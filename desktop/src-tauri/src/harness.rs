//! Minimal Creation Harness:
//! 1. Assembles immutable atomic asset snapshots (`snapshots/<seq>/`) for the chosen local Agent.
//! 2. Delegates image generation & revision to isolated worker sessions, verifying real image bytes and preserving every prior version.
//! 3. Saves verified results + prompts + tags straight back into the Showcase.
use crate::{
    adapters::{
        AdapterEvent, AdapterRegistry, AdapterStatus, Capability, EventSink, TurnRequest,
    },
    library::LibraryStore,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentsUpdate {
    #[serde(default)]
    pub skill_id: String,
    #[serde(default)]
    pub prompt_ids: Vec<String>,
}

fn default_mode() -> String {
    "image_text".into()
}
fn default_adapter() -> String {
    "codex".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationJob {
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub source_task_id: Option<String>,
    pub id: String,
    pub title: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_adapter")]
    pub adapter_id: String,
    #[serde(default)]
    pub skill_id: String,
    #[serde(default)]
    pub skill_name: String,
    #[serde(default)]
    pub prompt_ids: Vec<String>,
    #[serde(default)]
    pub snapshot_seq: u32,
    #[serde(default)]
    pub draft_markdown: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub activity: String,
    #[serde(default)]
    pub updated_at: u64,
}

impl CreationJob {
    pub fn busy(&self) -> bool {
        ["starting", "running"].contains(&self.status.as_str())
    }
}

struct Runtime {
    _lock: fs::File,
    root: PathBuf,
    jobs: Mutex<HashMap<String, CreationJob>>,
    adapters: AdapterRegistry,
    operation: Mutex<()>,
    dispatch: Mutex<()>,
}

#[derive(Clone)]
pub struct HarnessEngine(Arc<Runtime>);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn validate_image_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.is_empty() {
        return None;
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("webp")
    } else {
        None
    }
}

fn image_delegation_tools() -> Vec<Value> {
    vec![json!({
        "type": "function",
        "name": "creatorx_generate_image",
        "description": "当用户明确要求生图或改图时，将完整视觉意图与准确图内文字交给独立生图子会话执行，返回真实图片文件与任务状态。纯讨论或改文阶段不要调用。若基于已有生图任务修改，传入同一主会话下的 sourceTaskId；新图留空。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "intent": { "type": "string", "description": "完整、明确的画面构图、角色特征、图内逐字准确文案及修改点" },
                "sourceTaskId": { "type": "string", "description": "需要基于原图微调的已完成生图子任务 ID；生成新图留空" }
            },
            "required": ["intent"],
            "additionalProperties": false
        }
    })]
}

fn atomic_save_file(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| "无法创建任务子目录")?;
    }
    let temp = dest.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let res = (|| {
        let mut file = options.open(&temp).map_err(|_| "无法写入任务文件")?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "无法落盘任务文件")?;
        fs::rename(&temp, dest).map_err(|_| "无法提交任务文件")
    })();
    if res.is_err() {
        let _ = fs::remove_file(&temp);
    }
    res.map_err(String::from)
}

fn save_job_record(root: &Path, job: &CreationJob) -> Result<(), String> {
    let dir = root.join(&job.id);
    fs::create_dir_all(&dir).map_err(|_| "无法创建任务目录")?;
    let payload = serde_json::to_vec_pretty(job).map_err(|_| "无法序列化任务")?;
    atomic_save_file(&dir.join("job.json"), &payload)
}

impl EventSink for HarnessEngine {
    fn on_event(&self, event: AdapterEvent) {
        self.process_adapter_event(event);
    }

    fn on_tool_call(
        &self,
        thread_id: &str,
        turn_id: &str,
        tool: &str,
        arguments: &Value,
    ) -> Result<Value, String> {
        match tool {
            "creatorx_generate_image" | "creatorx_render_slot" => {
                self.tool_delegate_image(thread_id, turn_id, arguments)
            }
            _ => Err(format!("不支持的 Harness 工具：{tool}")),
        }
    }
}

impl HarnessEngine {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Self::with_registry(root, AdapterRegistry::new())
    }

    pub(crate) fn with_registry(root: PathBuf, adapters: AdapterRegistry) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|_| "无法创建创作目录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护创作目录")?;
        }
        let lock = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(root.join(".lock"))
            .map_err(|_| "无法锁定创作目录")?;
        lock.try_lock()
            .map_err(|_| "已有 CreatorX 正在管理创作任务，请使用已打开的客户端")?;
        let mut jobs = HashMap::new();
        for file in fs::read_dir(&root).map_err(|_| "无法读取创作目录")? {
            let path = file.map_err(|_| "无法读取任务")?.path().join("job.json");
            if !path.is_file() {
                continue;
            }
            if let Ok(mut job) = fs::read(path)
                .map_err(|_| ())
                .and_then(|bytes| serde_json::from_slice::<CreationJob>(&bytes).map_err(|_| ()))
            {
                if Uuid::parse_str(&job.id).is_err() {
                    continue;
                }
                if job.busy() {
                    job.status = "interrupted".into();
                    job.activity = "上次客户端关闭，可在原任务继续".into();
                    save_job_record(&root, &job)?;
                }
                jobs.insert(job.id.clone(), job);
            }
        }
        Ok(Self(Arc::new(Runtime {
            _lock: lock,
            root,
            jobs: Mutex::new(jobs),
            adapters,
            operation: Mutex::new(()),
            dispatch: Mutex::new(()),
        })))
    }

    pub fn adapters_status(&self, verify_id: Option<&str>) -> Vec<AdapterStatus> {
        self.0.adapters.list_statuses(verify_id)
    }

    pub fn list(&self) -> Result<Vec<CreationJob>, String> {
        let jobs = self.0.jobs.lock().map_err(|_| "任务正忙")?;
        let mut list: Vec<_> = jobs.values().cloned().collect();
        list.sort_by(|a, b| a.updated_at.cmp(&b.updated_at).then(a.id.cmp(&b.id)));
        Ok(list)
    }

    pub fn get(&self, id: &str) -> Result<CreationJob, String> {
        self.0
            .jobs
            .lock()
            .map_err(|_| "任务正忙")?
            .get(id)
            .cloned()
            .ok_or_else(|| "创作任务不存在".into())
    }

    fn put(&self, mut job: CreationJob) -> Result<(), String> {
        job.updated_at = now_secs();
        let mut jobs = self.0.jobs.lock().map_err(|_| "任务正忙")?;
        save_job_record(&self.0.root, &job)?;
        jobs.insert(job.id.clone(), job);
        Ok(())
    }

    pub fn create(
        &self,
        store: &LibraryStore,
        mode: &str,
        adapter_id: &str,
        skill_id: &str,
        prompt_ids: Vec<String>,
        brief: String,
    ) -> Result<CreationJob, String> {
        if !["image_text", "video"].contains(&mode) {
            return Err("创作方向需为 图文(image_text) 或 视频(video)".into());
        }
        if brief.trim().is_empty() || brief.len() > 50_000 || prompt_ids.len() > 16 {
            return Err("请填写创作想法（不超过 5 万字节，最多同时引用 16 条提示词）".into());
        }
        let adapter_id = if adapter_id.trim().is_empty() {
            "codex"
        } else {
            adapter_id.trim()
        };
        let adapter = self.0.adapters.get(adapter_id)?;
        if !adapter
            .status(false)
            .capabilities
            .contains(&Capability::Planning)
        {
            return Err("所选本地 Agent 适配器不支持创作对话".into());
        }

        let id = Uuid::new_v4().to_string();
        let dir = self.0.root.join(&id);
        fs::create_dir(&dir).map_err(|_| "无法创建任务目录")?;
        let snap_dir = dir.join("snapshots/1");
        let manifest = match store.snapshot_assets(skill_id, &prompt_ids, &snap_dir) {
            Ok(m) => m,
            Err(e) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(e);
            }
        };

        let agents_rule = "# CreatorX Creation Harness 沙盒规则\n\
            1. 仅在本任务目录内工作，不修改外部仓库或应用样板间，不对外发布或 git push。\n\
            2. 本轮只读资产快照位于 `snapshots/<最新序号>/`（含用户引用的提示词 `prompts.md`、细粒度 `skill/SKILL.md` 及 `references/` 参考图）。\n\
            3. 成稿文字或脚本可同步写入 `draft.md` 便于右侧展示与复制；生图或改图通过 `creatorx_generate_image` 交给独立子会话执行，每次保留历史版本。\n";
        if let Err(e) = atomic_save_file(&dir.join("AGENTS.md"), agents_rule.as_bytes()) {
            let _ = fs::remove_dir_all(&dir);
            return Err(e);
        }

        let job = CreationJob {
            parent_id: None,
            source_task_id: None,
            id: id.clone(),
            title: brief
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or(if mode == "video" { "视频创作" } else { "图文创作" })
                .chars()
                .take(40)
                .collect(),
            mode: mode.into(),
            adapter_id: adapter_id.to_string(),
            skill_id: skill_id.into(),
            skill_name: manifest
                .skill
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            prompt_ids,
            snapshot_seq: 1,
            draft_markdown: String::new(),
            thread_id: None,
            turn_id: None,
            status: "idle".into(),
            messages: vec![],
            images: vec![],
            error: None,
            activity: String::new(),
            updated_at: now_secs(),
        };
        self.put(job)?;
        self.continue_chat(&id, brief, None, None)
    }

    pub fn continue_chat(
        &self,
        id: &str,
        text: String,
        attachments: Option<(&LibraryStore, AttachmentsUpdate)>,
        adapter_override: Option<String>,
    ) -> Result<CreationJob, String> {
        if text.trim().is_empty() || text.len() > 100_000 {
            return Err("请输入不超过 10 万字节的交流内容".into());
        }
        let _op = self
            .0
            .operation
            .lock()
            .map_err(|_| "正在提交任务，请稍候")?;
        let mut job = self.get(id)?;
        if job.parent_id.is_some() {
            return Err("生图子任务不支持直接聊天，请在主会话继续或点击微调".into());
        }
        if job.busy() {
            return Err("当前任务正在执行，请等待完成或停止后再发送".into());
        }
        if let Some(next_adapter) = adapter_override.filter(|s| !s.trim().is_empty()) {
            if next_adapter != job.adapter_id {
                self.0.adapters.get(&next_adapter)?;
                job.adapter_id = next_adapter;
                job.thread_id = None;
            }
        }
        if let Some((store, next)) = attachments {
            if next.prompt_ids.len() > 16 {
                return Err("最多同时引用 16 条提示词".into());
            }
            let seq = job.snapshot_seq.checked_add(1).ok_or("快照次数超限")?;
            let snap_dir = self.0.root.join(&job.id).join(format!("snapshots/{seq}"));
            let manifest = store.snapshot_assets(&next.skill_id, &next.prompt_ids, &snap_dir)?;
            job.skill_id = next.skill_id;
            job.skill_name = manifest
                .skill
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_default();
            job.prompt_ids = next.prompt_ids;
            job.snapshot_seq = seq;
        }

        let adapter = self.0.adapters.get(&job.adapter_id)?;
        job.status = "starting".into();
        job.error = None;
        job.turn_id = None;
        job.activity = format!("正在连接 {}", job.adapter_id);
        job.messages.push(Message {
            id: Uuid::new_v4().to_string(),
            role: "user".into(),
            text: text.clone(),
        });
        self.put(job.clone())?;

        let dir = self.0.root.join(&job.id);
        let snap_dir = dir.join(format!("snapshots/{}", job.snapshot_seq));
        let prompts_md = snap_dir.join("prompts.md");
        let skill_md = snap_dir.join("skill/SKILL.md");
        let mode_label = if job.mode == "video" { "视频" } else { "图文" };

        let dev_instructions = format!(
            "你在 CreatorX 的「{mode_label}」创作台协助用户组合生文、生图与视频脚本创作。\n\
            1. 保持简洁直接：用户还在讨论选题或打磨文字时，只做文字交流与定稿整理，不要擅自提前出图；当形成完整定稿文案／长文／分镜脚本时，可将其同步写入本任务根目录 `draft.md` 方便右侧展示复制。\n\
            2. 本轮生效的资产快照目录为 `{}`：若存在 `prompts.md`（用户勾选的 #Tag 角色、画风或文风提示词）及 `references/` 参考图，请读取并严格沿用；若存在 `skill/SKILL.md`，按其细粒度职责执行。\n\
            3. 当用户明确要求生成图片或修改已有图片时，调用 `creatorx_generate_image` 把完整意图与逐字文案交给独立生图子会话执行（修改已有图片时传入对应的 `sourceTaskId`），等待真实文件结果返回后再汇报；不把脚本或计划冒充已生成的图片或视频文件。",
            snap_dir.display()
        );

        let context_header = format!(
            "[本轮挂载资产索引] 快照=#{}，提示词文件={}，Skill={}",
            job.snapshot_seq,
            if prompts_md.is_file() {
                prompts_md.to_string_lossy().into_owned()
            } else {
                "未勾选".into()
            },
            if skill_md.is_file() {
                format!("{} ({})", job.skill_name, skill_md.display())
            } else {
                "未选择".into()
            }
        );

        let req = TurnRequest {
            thread_id: job.thread_id.clone(),
            cwd: dir,
            developer_instructions: dev_instructions,
            user_messages: vec![context_header, text],
            skill_attachment: skill_md
                .is_file()
                .then(|| (job.skill_name.clone(), skill_md)),
            dynamic_tools: image_delegation_tools(),
        };

        match adapter.start_turn(req, Arc::new(self.clone())) {
            Ok(started) => {
                let mut jobs = self.0.jobs.lock().map_err(|_| "任务正忙")?;
                if let Some(current) = jobs.get_mut(id) {
                    current.thread_id = Some(started.thread_id);
                    if current.turn_id.is_none() {
                        current.turn_id = started.turn_id;
                    }
                    if current.status == "starting" {
                        current.status = "running".into();
                        current.activity = "正在回复…".into();
                    }
                    current.updated_at = now_secs();
                    save_job_record(&self.0.root, current)?;
                }
                drop(jobs);
                self.get(id)
            }
            Err(err) => {
                let mut current = self.get(id)?;
                current.status = "error".into();
                current.error = Some(err.clone());
                current.activity = "连接未完成".into();
                self.put(current)?;
                Err(err)
            }
        }
    }

    fn tool_delegate_image(
        &self,
        thread_id: &str,
        turn_id: &str,
        arguments: &Value,
    ) -> Result<Value, String> {
        let parent = self
            .list()?
            .into_iter()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
            .ok_or("找不到主创作会话")?;
        if parent.parent_id.is_some()
            || !parent.busy()
            || parent.turn_id.as_deref() != Some(turn_id)
        {
            return Err("仅当前执行中的主会话可派发生图子任务".into());
        }
        let intent = arguments
            .get("intent")
            .and_then(Value::as_str)
            .unwrap_or("");
        let source_task_id = arguments
            .get("sourceTaskId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);

        let worker_id = self.spawn_image_worker(&parent.id, source_task_id, intent.to_string())?;
        let started = std::time::Instant::now();
        loop {
            let worker = self.get(&worker_id)?;
            if !worker.busy() {
                return Ok(json!({
                    "ok": worker.status == "completed" && worker.error.is_none(),
                    "taskId": worker_id,
                    "status": worker.status,
                    "error": worker.error,
                    "images": worker.images.iter().map(|p| self.0.root.join(&worker_id).join(p)).collect::<Vec<_>>(),
                }));
            }
            if started.elapsed() > Duration::from_secs(1800) {
                let _ = self.cancel(&worker_id);
                return Err("生图等待超时，已请求停止；请检查任务状态，不自动重试".into());
            }
            if !self.get(&parent.id)?.busy() {
                let _ = self.cancel(&worker_id);
                return Err("主会话已停止，已同步停止生图子任务".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    #[cfg(test)]
    pub fn spawn_image_worker_for_test(
        &self,
        parent_id: &str,
        source_task_id: Option<String>,
        intent: String,
    ) -> Result<CreationJob, String> {
        let worker_id = self.spawn_image_worker(parent_id, source_task_id, intent)?;
        self.get(&worker_id)
    }

    fn spawn_image_worker(
        &self,
        parent_id: &str,
        source_task_id: Option<String>,
        intent: String,
    ) -> Result<String, String> {
        if intent.trim().is_empty() || intent.len() > 100_000 {
            return Err("请填写明确的生图或改图说明".into());
        }
        let _guard = self.0.dispatch.lock().map_err(|_| "生图派发正忙")?;
        let parent = self.get(parent_id)?;
        if parent.parent_id.is_some() {
            return Err("不能从子任务再次派发生图任务".into());
        }
        if self
            .list()?
            .iter()
            .filter(|j| j.parent_id.as_deref() == Some(parent_id) && j.busy())
            .count()
            >= 3
        {
            return Err("已有 3 个生图任务正在执行，请等待结果".into());
        }
        let adapter = self.0.adapters.get(&parent.adapter_id)?;
        let caps = adapter.status(false).capabilities;

        let source_job = source_task_id
            .as_deref()
            .map(|sid| self.get(sid))
            .transpose()?;
        if let Some(src) = &source_job {
            if src.parent_id.as_deref() != Some(parent_id) || src.images.is_empty() || src.busy() {
                return Err("只能基于当前创作下已完成且包含真实图片的任务进行微调".into());
            }
            if !caps.contains(&Capability::ImageRevision) {
                return Err("当前所选本地 Agent 不支持原图微调".into());
            }
        } else if !caps.contains(&Capability::ImageGeneration) {
            return Err("当前所选本地 Agent 不支持原生生图".into());
        }

        let worker_id = Uuid::new_v4().to_string();
        let worker_dir = self.0.root.join(&worker_id);
        fs::create_dir_all(&worker_dir).map_err(|_| "无法创建生图子任务目录")?;

        let parent_dir = self.0.root.join(&parent.id);
        let snap_src = parent_dir.join(format!("snapshots/{}", parent.snapshot_seq));
        if snap_src.is_dir() {
            for (rel, bytes) in LibraryStore::read_package(&snap_src)? {
                atomic_save_file(&worker_dir.join(&rel), &bytes)?;
            }
        }
        let agents_path = parent_dir.join("AGENTS.md");
        if agents_path.is_file() {
            let _ = fs::copy(&agents_path, worker_dir.join("AGENTS.md"));
        }

        let mut prompt_body = format!(
            "你是独立生图子会话，直接执行以下视觉意图并读取检查实际生成的图片文件；不得再次派生代理，不覆盖任何旧版文件。\n\
            完整意图：\n{}\n",
            intent.trim()
        );
        if worker_dir.join("prompts.md").is_file() {
            prompt_body.push_str(
                "\n请读取本目录 `prompts.md` 及 `references/` 中的参考图，严格保持角色与画风一致。",
            );
        }
        if let Some(src) = &source_job {
            let prev_dir = worker_dir.join("previous");
            fs::create_dir_all(&prev_dir).map_err(|_| "无法准备上一版原图目录")?;
            for rel in &src.images {
                let filename = Path::new(rel).file_name().ok_or("原图文件名无效")?;
                let dest = prev_dir.join(filename);
                fs::copy(self.0.root.join(&src.id).join(rel), &dest)
                    .map_err(|_| "无法读取上一版真实原图")?;
                prompt_body.push_str(&format!(
                    "\n【基于真实原图微调】必须引用实际上一版图片 `{}` 进行修改，保持其余细节稳定。",
                    dest.display()
                ));
            }
        }
        prompt_body.push_str(
            "\n必须调用原生 image_gen / image_generation 产出真实图片文件；不可用外部付费 API 或代码绘图冒充。",
        );

        let skill_md = worker_dir.join("skill/SKILL.md");
        let worker = CreationJob {
            parent_id: Some(parent.id.clone()),
            source_task_id: source_job.map(|s| s.id),
            id: worker_id.clone(),
            title: intent.trim().chars().take(42).collect(),
            mode: parent.mode.clone(),
            adapter_id: parent.adapter_id.clone(),
            skill_id: parent.skill_id.clone(),
            skill_name: parent.skill_name.clone(),
            prompt_ids: parent.prompt_ids.clone(),
            snapshot_seq: parent.snapshot_seq,
            draft_markdown: String::new(),
            thread_id: None,
            turn_id: None,
            status: "starting".into(),
            messages: vec![],
            images: vec![],
            error: None,
            activity: "正在生成图片…".into(),
            updated_at: now_secs(),
        };
        self.put(worker)?;

        let req = TurnRequest {
            thread_id: None,
            cwd: worker_dir,
            developer_instructions: "你是受控生图子会话，直接调用原生生图工具完成图片并核对，不修改外部目录。".into(),
            user_messages: vec![prompt_body],
            skill_attachment: skill_md
                .is_file()
                .then(|| (parent.skill_name.clone(), skill_md)),
            dynamic_tools: vec![],
        };

        match adapter.start_turn(req, Arc::new(self.clone())) {
            Ok(started) => {
                let mut jobs = self.0.jobs.lock().map_err(|_| "任务正忙")?;
                if let Some(w) = jobs.get_mut(&worker_id) {
                    w.thread_id = Some(started.thread_id);
                    if w.turn_id.is_none() {
                        w.turn_id = started.turn_id;
                    }
                    if w.status == "starting" {
                        w.status = "running".into();
                    }
                    save_job_record(&self.0.root, w)?;
                }
                Ok(worker_id)
            }
            Err(err) => {
                let mut w = self.get(&worker_id)?;
                w.status = "error".into();
                w.error = Some(err.clone());
                self.put(w)?;
                Err(err)
            }
        }
    }

    pub(crate) fn process_adapter_event(&self, event: AdapterEvent) {
        match event {
            AdapterEvent::Disconnected { error } => {
                if let Ok(mut jobs) = self.0.jobs.lock() {
                    for job in jobs.values_mut().filter(|j| j.busy()) {
                        job.status = "interrupted".into();
                        job.error = Some(error.clone());
                        let _ = save_job_record(&self.0.root, job);
                    }
                }
            }
            AdapterEvent::TurnStarted { thread_id, turn_id } => {
                if let Ok(mut jobs) = self.0.jobs.lock() {
                    if let Some(job) = jobs
                        .values_mut()
                        .find(|j| j.thread_id.as_deref() == Some(&thread_id))
                    {
                        job.turn_id = Some(turn_id);
                        job.status = "running".into();
                        let _ = save_job_record(&self.0.root, job);
                    }
                }
            }
            AdapterEvent::Activity { thread_id, text } => {
                if let Ok(mut jobs) = self.0.jobs.lock() {
                    if let Some(job) = jobs
                        .values_mut()
                        .find(|j| j.thread_id.as_deref() == Some(&thread_id))
                    {
                        job.activity = text;
                    }
                }
            }
            AdapterEvent::MessageDelta {
                thread_id,
                item_id,
                delta,
            } => {
                if let Ok(mut jobs) = self.0.jobs.lock() {
                    if let Some(job) = jobs
                        .values_mut()
                        .find(|j| j.thread_id.as_deref() == Some(&thread_id))
                    {
                        if let Some(m) = job.messages.iter_mut().find(|m| m.id == item_id) {
                            if m.text.len() < 200_000 {
                                m.text.push_str(&delta);
                            }
                        } else {
                            job.messages.push(Message {
                                id: item_id,
                                role: "assistant".into(),
                                text: delta,
                            });
                        }
                    }
                }
            }
            AdapterEvent::MessageCompleted {
                thread_id,
                item_id,
                text,
            } => {
                if let Ok(mut jobs) = self.0.jobs.lock() {
                    if let Some(job) = jobs
                        .values_mut()
                        .find(|j| j.thread_id.as_deref() == Some(&thread_id))
                    {
                        if let Some(m) = job.messages.iter_mut().find(|m| m.id == item_id) {
                            m.text = text;
                        } else {
                            job.messages.push(Message {
                                id: item_id,
                                role: "assistant".into(),
                                text,
                            });
                        }
                        let _ = save_job_record(&self.0.root, job);
                    }
                }
            }
            AdapterEvent::ImageGenerated {
                thread_id,
                saved_path,
                base64_data,
            } => {
                self.record_generated_image(&thread_id, saved_path, base64_data);
            }
            AdapterEvent::TurnCompleted {
                thread_id,
                status,
                error,
            } => {
                self.complete_turn(&thread_id, status, error);
            }
        }
    }

    fn record_generated_image(
        &self,
        thread_id: &str,
        saved_path: Option<PathBuf>,
        base64_data: Option<String>,
    ) {
        let raw = (|| -> Result<(Vec<u8>, &'static str), String> {
            let bytes = if let Some(path) = saved_path {
                fs::read(path).map_err(|_| "无法读取生成的图片文件".to_string())?
            } else if let Some(data) = base64_data {
                let payload = data
                    .split_once(',')
                    .map(|(_, b)| b)
                    .unwrap_or(data.as_str());
                STANDARD
                    .decode(payload)
                    .map_err(|_| "生成图片编码无效".to_string())?
            } else {
                return Err("生图事件未返回可读取的图片文件".into());
            };
            let ext =
                validate_image_magic(&bytes).ok_or_else(|| "生图结果未通过有效图片文件头校验".to_string())?;
            Ok((bytes, ext))
        })();

        let Ok(mut jobs) = self.0.jobs.lock() else {
            return;
        };
        let Some(job) = jobs
            .values_mut()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
        else {
            return;
        };
        match raw {
            Err(err) => {
                job.error = Some(err);
                let _ = save_job_record(&self.0.root, job);
            }
            Ok((bytes, ext)) => {
                let next_idx = job.images.len() + 1;
                let rel = format!("artifacts/v{next_idx}-{}.{ext}", Uuid::new_v4());
                let dest = self.0.root.join(&job.id).join(&rel);
                if dest.exists() {
                    job.error = Some("检测到重复文件名，拒绝覆盖旧图".into());
                    return;
                }
                if let Err(err) = atomic_save_file(&dest, &bytes) {
                    job.error = Some(err);
                    return;
                }
                job.images.push(rel);
                job.updated_at = now_secs();
                let _ = save_job_record(&self.0.root, job);
            }
        }
    }

    fn complete_turn(&self, thread_id: &str, status: String, error: Option<String>) {
        let Ok(mut jobs) = self.0.jobs.lock() else {
            return;
        };
        let Some(job) = jobs
            .values_mut()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
        else {
            return;
        };
        job.status = status;
        if let Some(err) = error {
            job.error = Some(err);
        }
        if job.parent_id.is_none() {
            if let Ok(draft) = fs::read_to_string(self.0.root.join(&job.id).join("draft.md")) {
                if !draft.trim().is_empty() {
                    job.draft_markdown = draft.chars().take(200_000).collect();
                }
            }
        } else if job.status == "completed" && job.images.is_empty() {
            job.status = "error".into();
            job.error.get_or_insert_with(|| {
                "本轮未收到真实图片文件，尚未完成出图".into()
            });
        }
        job.activity = if job.status == "completed" {
            "已完成".into()
        } else {
            "未完成，可查看提示继续".into()
        };
        job.updated_at = now_secs();
        let _ = save_job_record(&self.0.root, job);
    }

    pub fn cancel(&self, id: &str) -> Result<CreationJob, String> {
        let job = self.get(id)?;
        if !job.busy() {
            return Ok(job);
        }
        for child in self
            .list()?
            .into_iter()
            .filter(|j| j.parent_id.as_deref() == Some(id) && j.busy())
        {
            let _ = self.cancel(&child.id);
        }
        let thread = job.thread_id.as_ref().ok_or("任务尚在连接，请稍候")?;
        let turn = job.turn_id.as_ref().ok_or("任务尚在启动，请稍候")?;
        let adapter = self.0.adapters.get(&job.adapter_id)?;
        adapter.cancel_turn(thread, turn)?;
        self.get(id)
    }

    pub fn open_artifact_file(&self, id: &str, path: &str) -> Result<(), String> {
        self.get(id)?;
        let relative = Path::new(path);
        if path.contains('\\')
            || path.contains(':')
            || !relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return Err("只支持打开当前任务沙盒内的文件".into());
        }
        let root = self
            .0
            .root
            .join(id)
            .canonicalize()
            .map_err(|_| "任务目录不存在")?;
        let file = root
            .join(relative)
            .canonicalize()
            .map_err(|_| "文件尚未生成或路径无效")?;
        let extension = file
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !file.starts_with(&root)
            || !file.is_file()
            || ![
                "md", "json", "txt", "png", "jpg", "jpeg", "webp", "pdf", "mp4", "webm", "mov",
                "wav", "mp3",
            ]
            .contains(&extension.as_str())
        {
            return Err("不能打开此类型或任务以外的文件".into());
        }
        #[cfg(target_os = "macos")]
        let mut command = Command::new("/usr/bin/open");
        #[cfg(windows)]
        let mut command = Command::new("explorer.exe");
        #[cfg(not(any(target_os = "macos", windows)))]
        let mut command = Command::new("xdg-open");
        command.arg(file).spawn().map_err(|_| "无法打开作品文件")?;
        Ok(())
    }

    pub fn root_directory(&self) -> String {
        self.0.root.to_string_lossy().into_owned()
    }

    pub fn shutdown(&self) {
        self.0.adapters.shutdown_all();
    }
}

#[tauri::command]
pub async fn adapter_list(
    engine: tauri::State<'_, HarnessEngine>,
    verify_id: Option<String>,
) -> Result<Vec<AdapterStatus>, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || Ok(engine.adapters_status(verify_id.as_deref())))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn creation_list(
    engine: tauri::State<'_, HarnessEngine>,
) -> Result<Vec<CreationJob>, String> {
    engine.list()
}

#[tauri::command]
pub async fn creation_directory(
    engine: tauri::State<'_, HarnessEngine>,
) -> Result<String, String> {
    Ok(engine.root_directory())
}

#[tauri::command]
pub async fn creation_start(
    app: tauri::AppHandle,
    engine: tauri::State<'_, HarnessEngine>,
    mode: String,
    adapter_id: Option<String>,
    skill_id: Option<String>,
    prompt_ids: Vec<String>,
    brief: String,
) -> Result<CreationJob, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine.create(
            app.state::<LibraryStore>().inner(),
            &mode,
            adapter_id.as_deref().unwrap_or("codex"),
            &skill_id.unwrap_or_default(),
            prompt_ids,
            brief,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn creation_continue(
    app: tauri::AppHandle,
    engine: tauri::State<'_, HarnessEngine>,
    id: String,
    text: String,
    attachments: Option<AttachmentsUpdate>,
    adapter_id: Option<String>,
) -> Result<CreationJob, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<LibraryStore>();
        engine.continue_chat(
            &id,
            text,
            attachments.map(|a| (store.inner(), a)),
            adapter_id,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn creation_cancel(
    engine: tauri::State<'_, HarnessEngine>,
    id: String,
) -> Result<CreationJob, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.cancel(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn creation_open_file(
    engine: tauri::State<'_, HarnessEngine>,
    id: String,
    path: String,
) -> Result<(), String> {
    engine.open_artifact_file(&id, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{AgentAdapter, StartedTurn};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockAdapter {
        calls: AtomicUsize,
        produce_image: bool,
    }

    impl MockAdapter {
        fn new(produce_image: bool) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                produce_image,
            }
        }
    }

    impl AgentAdapter for MockAdapter {
        fn id(&self) -> &'static str {
            "mock"
        }
        fn status(&self, _verify_live: bool) -> AdapterStatus {
            AdapterStatus {
                id: "mock".into(),
                display_name: "Mock Local Agent".into(),
                available: true,
                capabilities: vec![
                    Capability::Planning,
                    Capability::ImageGeneration,
                    Capability::ImageRevision,
                    Capability::WorkspaceExec,
                ],
                detail: None,
            }
        }
        fn start_turn(
            &self,
            req: TurnRequest,
            sink: Arc<dyn EventSink>,
        ) -> Result<StartedTurn, String> {
            let seq = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let thread_id = req
                .thread_id
                .unwrap_or_else(|| format!("mock-thread-{seq}"));
            let turn_id = format!("mock-turn-{seq}");
            let is_worker = req.dynamic_tools.is_empty();
            let produce = self.produce_image;
            let cwd = req.cwd.clone();
            let bg_thread_id = thread_id.clone();
            let bg_turn_id = turn_id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                sink.on_event(AdapterEvent::TurnStarted {
                    thread_id: bg_thread_id.clone(),
                    turn_id: bg_turn_id,
                });
                if is_worker && produce {
                    let png = include_bytes!("../icons/32x32.png");
                    let saved = cwd.join("generated.png");
                    fs::write(&saved, png).unwrap();
                    sink.on_event(AdapterEvent::ImageGenerated {
                        thread_id: bg_thread_id.clone(),
                        saved_path: Some(saved),
                        base64_data: None,
                    });
                }
                sink.on_event(AdapterEvent::MessageCompleted {
                    thread_id: bg_thread_id.clone(),
                    item_id: format!("msg-{seq}"),
                    text: "处理完成".into(),
                });
                sink.on_event(AdapterEvent::TurnCompleted {
                    thread_id: bg_thread_id,
                    status: "completed".into(),
                    error: None,
                });
            });
            Ok(StartedTurn {
                thread_id,
                turn_id: Some(turn_id),
            })
        }
        fn cancel_turn(&self, _thread_id: &str, _turn_id: &str) -> Result<(), String> {
            Ok(())
        }
        fn shutdown(&self) {}
    }

    fn wait_idle(engine: &HarnessEngine, id: &str) -> CreationJob {
        for _ in 0..100 {
            let job = engine.get(id).unwrap();
            if !job.busy() {
                return job;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("job {id} did not settle");
    }

    #[test]
    fn harness_preserves_tag_snapshots_and_original_images_on_revision() {
        use crate::library::{Entry, Kind, Modality};
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().join("library")).unwrap();
        store.tag_add("安静").unwrap();
        store.tag_add("生图").unwrap();
        let anjing_prompt = store
            .save(
                Entry {
                    schema_version: 5,
                    id: String::new(),
                    kind: Kind::Prompt,
                    modality: Modality::Image,
                    name: "安静角色视觉".into(),
                    prompt: "蛋黄黄乱发、黑卫衣、粗蜡笔纸纹".into(),
                    sample: String::new(),
                    tags: vec!["安静".into(), "生图".into()],
                    notes: String::new(),
                    origin_created_at: None,
                    origin_updated_at: None,
                    skill_markdown: String::new(),
                    skill_digest: None,
                    images: vec![],
                    revision: 0,
                    updated_at: 0,
                    source: None,
                },
                0,
                None,
            )
            .unwrap();

        let registry = AdapterRegistry::with_adapters(vec![Arc::new(MockAdapter::new(true))]);
        let engine = HarnessEngine::with_registry(root.path().join("creations"), registry).unwrap();

        let created = engine
            .create(
                &store,
                "image_text",
                "mock",
                "builtin:image-single",
                vec![anjing_prompt.id.clone()],
                "做一期零拷贝图文".into(),
            )
            .unwrap();
        let parent = wait_idle(&engine, &created.id);
        assert_eq!(parent.snapshot_seq, 1);
        let snap1 = root
            .path()
            .join("creations")
            .join(&parent.id)
            .join("snapshots/1/prompts.md");
        assert!(snap1.is_file());

        engine
            .continue_chat(
                &parent.id,
                "准备出套图".into(),
                Some((
                    &store,
                    AttachmentsUpdate {
                        skill_id: "builtin:image-series".into(),
                        prompt_ids: vec![anjing_prompt.id],
                    },
                )),
                None,
            )
            .unwrap();
        let parent = wait_idle(&engine, &parent.id);
        assert_eq!(parent.snapshot_seq, 2);
        assert!(snap1.is_file());

        let v1_worker = engine
            .spawn_image_worker_for_test(&parent.id, None, "画封面：安静在黑板前".into())
            .unwrap();
        let v1_done = wait_idle(&engine, &v1_worker.id);
        assert_eq!(v1_done.images.len(), 1);
        let v1_file = root
            .path()
            .join("creations")
            .join(&v1_done.id)
            .join(&v1_done.images[0]);
        let v1_bytes = fs::read(&v1_file).unwrap();

        let v2_worker = engine
            .spawn_image_worker_for_test(
                &parent.id,
                Some(v1_done.id.clone()),
                "把标题改短一点".into(),
            )
            .unwrap();
        let v2_done = wait_idle(&engine, &v2_worker.id);
        assert_eq!(v2_done.source_task_id.as_deref(), Some(v1_done.id.as_str()));
        assert_eq!(v2_done.images.len(), 1);
        assert_eq!(fs::read(&v1_file).unwrap(), v1_bytes);
    }

    #[test]
    fn missing_worker_image_fails_honestly() {
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().join("library")).unwrap();
        let registry = AdapterRegistry::with_adapters(vec![Arc::new(MockAdapter::new(false))]);
        let engine = HarnessEngine::with_registry(root.path().join("creations"), registry).unwrap();

        let created = engine
            .create(&store, "image_text", "mock", "", vec![], "测试".into())
            .unwrap();
        let parent = wait_idle(&engine, &created.id);
        let worker = engine
            .spawn_image_worker_for_test(&parent.id, None, "生一张图".into())
            .unwrap();
        let failed = wait_idle(&engine, &worker.id);
        assert_eq!(failed.status, "error");
        assert!(failed.error.unwrap_or_default().contains("真实图片文件"));
    }
}
