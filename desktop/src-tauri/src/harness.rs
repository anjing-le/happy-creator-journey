//! Creation Harness core: enforces immutable context snapshots, blueprint stage gates,
//! append-only slot artifact verification, and showcase promotion across pluggable local agents.
use crate::{
    adapters::{
        AdapterEvent, AdapterRegistry, AdapterStatus, Capability, EventSink, TurnRequest,
    },
    library::{DeliverableFormat, Entry, Kind, LibraryStore, Maturity},
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

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HarnessStage {
    #[default]
    Drafting,
    BlueprintReady,
    Rendering,
    Review,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactVersion {
    pub version: u32,
    pub path: String,
    pub task_id: String,
    #[serde(default)]
    pub note: String,
    pub created_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSlot {
    pub slot_id: String,
    pub label: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub visual_prompt: String,
    #[serde(default)]
    pub duration_sec: Option<u32>,
    #[serde(default = "default_slot_status")]
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub versions: Vec<ArtifactVersion>,
}

fn default_slot_status() -> String {
    "pending".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blueprint {
    #[serde(default)]
    pub summary_copy: String,
    #[serde(default)]
    pub body_markdown: String,
    #[serde(default)]
    pub slots: Vec<BlueprintSlot>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentsUpdate {
    #[serde(default = "default_true")]
    pub include_ip: bool,
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub skill_id: String,
    #[serde(default)]
    pub prompt_ids: Vec<String>,
}

fn default_true() -> bool {
    true
}
fn default_adapter() -> String {
    "codex".into()
}
fn default_deliverable() -> DeliverableFormat {
    DeliverableFormat::Cards
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationJob {
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub source_task_id: Option<String>,
    #[serde(default)]
    pub slot_id: Option<String>,
    #[serde(default)]
    pub revision_note: String,
    pub id: String,
    pub title: String,
    #[serde(default = "default_deliverable", alias = "mode")]
    pub deliverable: DeliverableFormat,
    #[serde(default = "default_adapter")]
    pub adapter_id: String,
    #[serde(default = "default_true")]
    pub include_ip: bool,
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub skill_id: String,
    #[serde(default)]
    pub skill_name: String,
    #[serde(default)]
    pub prompt_ids: Vec<String>,
    #[serde(default)]
    pub snapshot_seq: u32,
    #[serde(default)]
    pub stage: HarnessStage,
    #[serde(default)]
    pub blueprint_approved: bool,
    #[serde(default)]
    pub blueprint: Blueprint,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromoteShowcaseRequest {
    pub name: String,
    pub category: String,
    pub prompt: String,
    #[serde(default)]
    pub sample_input: String,
    #[serde(default)]
    pub sample_output: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub slot_ids: Vec<String>,
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

pub fn validate_slot_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 48
        || !trimmed.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
        })
    {
        return Err("槽位标识需为 1–48 位小写字母、数字、连字符或下划线（如 card-1、cover、shot-1）".into());
    }
    Ok(trimmed.to_string())
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

fn harness_dynamic_tools() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "creatorx_update_blueprint",
            "description": "整理或更新右侧工作台的结构化蓝图（卡片分页表 / 长文正文与插图锚点 / 视频口播与分镜表）。讨论定稿阶段调用此工具将结构化方案同步到右侧画布供用户确认。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "summaryCopy": { "type": "string", "description": "发布标题与短配文（卡片图文/视频）或文章导语摘要（长文）" },
                    "bodyMarkdown": { "type": "string", "description": "长文定稿 Markdown 正文（可用 ![说明](slot:slot-id) 标记插图位置）或视频完整口播逐字稿" },
                    "slots": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "slotId": { "type": "string", "description": "唯一槽位 ID，仅小写字母数字与连字符，如 card-1、cover、fig-1、shot-1" },
                                "label": { "type": "string", "description": "槽位显示标题，如 第 1 页·封面、封面图、镜头 1·开场钩子" },
                                "text": { "type": "string", "description": "卡片图内逐字准确文案 / 插图图注 / 该镜头对应台词" },
                                "visualPrompt": { "type": "string", "description": "画面主体、构图、留白或镜头运动描述" },
                                "durationSec": { "type": "integer", "description": "视频单镜预计秒数（图文可省略）" }
                            },
                            "required": ["slotId", "label"]
                        }
                    }
                },
                "required": ["slots"],
                "additionalProperties": false
            }
        }),
        json!({
            "type": "function",
            "name": "creatorx_render_slot",
            "description": "在蓝图已经过用户确认放行后，针对单个 slotId 派发独立生图或原图修订子任务，并将真实文件追加写入该槽位版本链。未获确认前请勿调用。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "slotId": { "type": "string", "description": "蓝图中已存在的 slotId" },
                    "revisionNote": { "type": "string", "description": "若是基于该槽位已有原图微调，说明本轮具体修改点；首次生成留空" }
                },
                "required": ["slotId"],
                "additionalProperties": false
            }
        }),
    ]
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

fn write_workspace_contract_files(root: &Path, job: &CreationJob) -> Result<(), String> {
    if job.parent_id.is_some() {
        return Ok(());
    }
    let dir = root.join(&job.id);
    atomic_save_file(&dir.join("draft.md"), job.blueprint.body_markdown.as_bytes())?;
    let contract = json!({
        "deliverable": job.deliverable,
        "approved": job.blueprint_approved,
        "summaryCopy": job.blueprint.summary_copy,
        "bodyMarkdown": job.blueprint.body_markdown,
        "slots": job.blueprint.slots.iter().map(|s| json!({
            "slotId": s.slot_id,
            "label": s.label,
            "text": s.text,
            "visualPrompt": s.visual_prompt,
            "durationSec": s.duration_sec
        })).collect::<Vec<_>>()
    });
    atomic_save_file(
        &dir.join("blueprint.json"),
        &serde_json::to_vec_pretty(&contract).map_err(|_| "无法序列化蓝图契约")?,
    )
}

fn merge_blueprint_payload(
    existing: &Blueprint,
    payload: &Value,
) -> Result<(Blueprint, bool), String> {
    let summary_copy = payload
        .get("summaryCopy")
        .and_then(Value::as_str)
        .unwrap_or(&existing.summary_copy)
        .chars()
        .take(16_000)
        .collect::<String>();
    let body_markdown = payload
        .get("bodyMarkdown")
        .and_then(Value::as_str)
        .unwrap_or(&existing.body_markdown)
        .chars()
        .take(200_000)
        .collect::<String>();
    let slots = if let Some(arr) = payload.get("slots").and_then(Value::as_array) {
        if arr.len() > 32 {
            return Err("单个创作任务最多支持 32 个槽位".into());
        }
        let mut seen = std::collections::HashSet::new();
        let mut merged = Vec::with_capacity(arr.len());
        for item in arr {
            let slot_id = validate_slot_id(
                item.get("slotId")
                    .and_then(Value::as_str)
                    .ok_or("蓝图槽位缺少 slotId")?,
            )?;
            if !seen.insert(slot_id.clone()) {
                return Err(format!("蓝图槽位 ID 重复：{slot_id}"));
            }
            let label = item
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or(&slot_id)
                .trim()
                .chars()
                .take(80)
                .collect::<String>();
            if label.is_empty() {
                return Err("蓝图槽位标题不能为空".into());
            }
            let text = item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(8_000)
                .collect::<String>();
            let visual_prompt = item
                .get("visualPrompt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(8_000)
                .collect::<String>();
            let duration_sec = item
                .get("durationSec")
                .and_then(Value::as_u64)
                .map(|n| n.min(600) as u32);
            let old = existing.slots.iter().find(|s| s.slot_id == slot_id);
            merged.push(BlueprintSlot {
                slot_id,
                label,
                text,
                visual_prompt,
                duration_sec,
                status: old
                    .map(|s| s.status.clone())
                    .unwrap_or_else(default_slot_status),
                error: old.and_then(|s| s.error.clone()),
                versions: old.map(|s| s.versions.clone()).unwrap_or_default(),
            });
        }
        merged
    } else {
        existing.slots.clone()
    };

    let content_changed = summary_copy != existing.summary_copy
        || body_markdown != existing.body_markdown
        || slots.len() != existing.slots.len()
        || slots.iter().zip(existing.slots.iter()).any(|(a, b)| {
            a.slot_id != b.slot_id
                || a.label != b.label
                || a.text != b.text
                || a.visual_prompt != b.visual_prompt
                || a.duration_sec != b.duration_sec
        });

    Ok((
        Blueprint {
            summary_copy,
            body_markdown,
            slots,
        },
        content_changed,
    ))
}

fn sync_disk_contract_if_modified(root: &Path, job: &mut CreationJob) {
    if job.parent_id.is_some() {
        return;
    }
    let dir = root.join(&job.id);
    if let Ok(draft) = fs::read_to_string(dir.join("draft.md")) {
        if !draft.trim().is_empty() && draft != job.blueprint.body_markdown {
            job.blueprint.body_markdown = draft.chars().take(200_000).collect();
        }
    }
    if let Ok(bytes) = fs::read(dir.join("blueprint.json")) {
        if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
            if let Ok((mut merged, changed)) = merge_blueprint_payload(&job.blueprint, &val) {
                if merged.body_markdown.trim().is_empty()
                    && !job.blueprint.body_markdown.trim().is_empty()
                {
                    merged.body_markdown = job.blueprint.body_markdown.clone();
                }
                if changed {
                    job.blueprint = merged;
                    if job.stage == HarnessStage::Drafting
                        && (!job.blueprint.slots.is_empty()
                            || !job.blueprint.body_markdown.trim().is_empty())
                    {
                        job.stage = HarnessStage::BlueprintReady;
                    }
                }
            }
        }
    }
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
            "creatorx_update_blueprint" => self.tool_update_blueprint(thread_id, turn_id, arguments),
            "creatorx_render_slot" | "creatorx_generate_image" => {
                self.tool_render_slot(thread_id, turn_id, arguments)
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
                    for slot in &mut job.blueprint.slots {
                        if slot.status == "running" {
                            slot.status = "pending".into();
                        }
                    }
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
        deliverable: DeliverableFormat,
        adapter_id: &str,
        include_ip: bool,
        track: String,
        skill_id: &str,
        prompt_ids: Vec<String>,
        brief: String,
    ) -> Result<CreationJob, String> {
        if deliverable == DeliverableFormat::General {
            return Err("请选择具体的创作制式：卡片图文、长文配图或视频创作".into());
        }
        if brief.trim().is_empty() || brief.len() > 50_000 || prompt_ids.len() > 16 {
            return Err("请填写创作想法（不超过 5 万字节，附加提示词最多 16 条）".into());
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
            return Err("所选 Agent 适配器不支持创作规划对话".into());
        }

        let id = Uuid::new_v4().to_string();
        let dir = self.0.root.join(&id);
        fs::create_dir(&dir).map_err(|_| "无法创建任务目录")?;
        let snapshot_dir = dir.join("snapshots/1");
        let manifest = match store.snapshot_context(
            include_ip,
            &track,
            skill_id,
            &prompt_ids,
            &snapshot_dir,
        ) {
            Ok(m) => m,
            Err(e) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(e);
            }
        };

        let agents_rule = "# CreatorX Creation Harness 沙盒契约\n\
            1. 仅在本任务目录工作，严禁修改应用素材库、原 Skill 源目录或其他仓库，不执行任何对外发布或 git push。\n\
            2. 本轮只读输入快照位于 `snapshots/<最新序号>/`（包含 `ip.md`、`prompts.md`、`skill/SKILL.md` 及 `references/` 参考图）。\n\
            3. 遵循三阶门禁：先对齐选题立意与真实素材 → 整理结构化蓝图（调用 `creatorx_update_blueprint` 或写入 `blueprint.json` 与 `draft.md`）供用户在右侧工作台确认 → 蓝图确认放行后才按槽位生成产物。\n\
            4. 产物文件必须写入 `artifacts/` 且不得覆盖任何历史版本。\n";
        if let Err(e) = atomic_save_file(&dir.join("AGENTS.md"), agents_rule.as_bytes()) {
            let _ = fs::remove_dir_all(&dir);
            return Err(e);
        }

        let job = CreationJob {
            parent_id: None,
            source_task_id: None,
            slot_id: None,
            revision_note: String::new(),
            id: id.clone(),
            title: brief
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or(deliverable.label())
                .chars()
                .take(40)
                .collect(),
            deliverable,
            adapter_id: adapter_id.to_string(),
            include_ip,
            track: track.trim().into(),
            skill_id: skill_id.into(),
            skill_name: manifest
                .skill
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            prompt_ids,
            snapshot_seq: 1,
            stage: HarnessStage::Drafting,
            blueprint_approved: false,
            blueprint: Blueprint::default(),
            thread_id: None,
            turn_id: None,
            status: "idle".into(),
            messages: vec![],
            images: vec![],
            error: None,
            activity: String::new(),
            updated_at: now_secs(),
        };
        write_workspace_contract_files(&self.0.root, &job)?;
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
            return Err("子渲染任务不支持直接对话，请在主会话或对应槽位操作".into());
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
                return Err("每次最多挂载 16 条风格提示词".into());
            }
            let seq = job.snapshot_seq.checked_add(1).ok_or("快照次数超限")?;
            let snap_dir = self.0.root.join(&job.id).join(format!("snapshots/{seq}"));
            let manifest = store.snapshot_context(
                next.include_ip,
                &next.track,
                &next.skill_id,
                &next.prompt_ids,
                &snap_dir,
            )?;
            job.include_ip = next.include_ip;
            job.track = next.track.trim().into();
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
        let skill_md = snap_dir.join("skill/SKILL.md");
        let format_guide = match job.deliverable {
            DeliverableFormat::Cards => {
                "当前交付制式：【卡片图文】（面向小红书／抖音多图卡片＋短配文）。\n\
                蓝图要求：`summaryCopy` 填写发布标题与短配文；`slots` 按页列出每张卡片（如 `card-1` 封面、`card-2` 要点页…），`text` 为图内逐字准确文字，`visualPrompt` 为画面构图与角色动作。"
            }
            DeliverableFormat::Article => {
                "当前交付制式：【长文配图】（面向微信公众号深度长文＋封面／文中插图）。\n\
                蓝图要求：`bodyMarkdown`（及 `draft.md`）填写完整结构化 Markdown 正文，文中需要配图处用 `![图注](slot:fig-1)` 占位；`slots` 列出 `cover`（封面图）与各 `fig-N` 插图的图注与画面构图说明。"
            }
            DeliverableFormat::Video => {
                "当前交付制式：【视频创作】（面向抖音／小红书短视频口播＋分镜）。\n\
                蓝图要求：`summaryCopy` 填写视频发布文案；`bodyMarkdown`（及 `draft.md`）填写完整口播／旁白逐字稿；`slots` 按镜头列出 `shot-1`..`shot-N`（`text` 为该镜台词，`visualPrompt` 为画面主体与运镜，`durationSec` 为秒数）。"
            }
            DeliverableFormat::General => "",
        };

        let dev_instructions = format!(
            "你在 CreatorX Creation Harness 中担任主创作协作引擎。\n\
            {format_guide}\n\
            硬门禁规则：\n\
            1. 优先读取当前生效快照目录 `{}`（若存在 `ip.md` 则严格遵循创作者 IP 底座与赛道定位；若存在 `prompts.md` 或 `skill/SKILL.md` 则遵循选定样板规范）。\n\
            2. 当选题与表达明确后，调用 `creatorx_update_blueprint`（或同步更新任务根目录 `blueprint.json` 与 `draft.md`）把结构化蓝图呈现在右侧工作台。\n\
            3. 当前蓝图放行状态：`blueprintApproved = {}`。若为 `false`，整理好蓝图后请提示用户在右侧工作台检查并点击「确认蓝图」或直接点槽位生成，切勿在未放行前调用 `creatorx_render_slot`。\n\
            4. 若蓝图已放行且用户明确要求在对话中生成或微调某个槽位图片，调用 `creatorx_render_slot` 交给独立子会话执行，等待真实文件结果再汇报。",
            snap_dir.display(),
            job.blueprint_approved
        );

        let context_header = format!(
            "[Harness 本轮上下文索引] 快照序号=#{}，目录={}，IP底座={}，赛道={}，样板Skill={}，蓝图已确认={}。",
            job.snapshot_seq,
            snap_dir.display(),
            if snap_dir.join("ip.md").is_file() { "已挂载(ip.md)" } else { "未配置/未挂载" },
            if job.track.is_empty() { "通用" } else { &job.track },
            if skill_md.is_file() { job.skill_name.as_str() } else { "未选择" },
            job.blueprint_approved
        );

        let req = TurnRequest {
            thread_id: job.thread_id.clone(),
            cwd: dir,
            developer_instructions: dev_instructions,
            user_messages: vec![context_header, text],
            skill_attachment: skill_md
                .is_file()
                .then(|| (job.skill_name.clone(), skill_md)),
            dynamic_tools: harness_dynamic_tools(),
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
                        current.activity = "正在思考与整理…".into();
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
                current.activity = "启动未成功".into();
                self.put(current)?;
                Err(err)
            }
        }
    }

    pub fn save_blueprint(
        &self,
        id: &str,
        incoming: Blueprint,
        approve: bool,
    ) -> Result<CreationJob, String> {
        let mut job = self.get(id)?;
        if job.parent_id.is_some() {
            return Err("子任务没有独立蓝图".into());
        }
        let val = serde_json::to_value(&incoming).map_err(|_| "蓝图数据无效")?;
        let (merged, changed) = merge_blueprint_payload(&job.blueprint, &val)?;
        if approve && merged.slots.is_empty() && merged.body_markdown.trim().is_empty() {
            return Err("请先至少添加一个卡片／插图／分镜槽位或正文，再确认蓝图".into());
        }
        job.blueprint = merged;
        if approve {
            job.blueprint_approved = true;
            if job.stage == HarnessStage::Drafting {
                job.stage = HarnessStage::BlueprintReady;
            }
        } else if changed && job.stage != HarnessStage::Review {
            job.blueprint_approved = false;
            if !job.blueprint.slots.is_empty() || !job.blueprint.body_markdown.trim().is_empty() {
                job.stage = HarnessStage::BlueprintReady;
            }
        }
        write_workspace_contract_files(&self.0.root, &job)?;
        self.put(job.clone())?;
        Ok(job)
    }

    fn tool_update_blueprint(
        &self,
        thread_id: &str,
        turn_id: &str,
        arguments: &Value,
    ) -> Result<Value, String> {
        let mut parent = self
            .list()?
            .into_iter()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
            .ok_or("找不到对应的主创作会话")?;
        if parent.parent_id.is_some()
            || !parent.busy()
            || parent.turn_id.as_deref() != Some(turn_id)
        {
            return Err("仅当前执行中的主会话可更新蓝图".into());
        }
        let (merged, changed) = merge_blueprint_payload(&parent.blueprint, arguments)?;
        parent.blueprint = merged;
        if changed && parent.stage != HarnessStage::Review {
            parent.blueprint_approved = false;
        }
        if parent.stage == HarnessStage::Drafting {
            parent.stage = HarnessStage::BlueprintReady;
        }
        parent.activity = "已同步结构化蓝图到右侧工作台".into();
        write_workspace_contract_files(&self.0.root, &parent)?;
        self.put(parent.clone())?;
        Ok(json!({
            "ok": true,
            "slotCount": parent.blueprint.slots.len(),
            "blueprintApproved": parent.blueprint_approved,
            "nextStep": "蓝图已呈现在右侧工作台；若尚未确认，请提示用户确认蓝图后再开始按槽位生成。"
        }))
    }

    fn tool_render_slot(
        &self,
        thread_id: &str,
        turn_id: &str,
        arguments: &Value,
    ) -> Result<Value, String> {
        let parent = self
            .list()?
            .into_iter()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
            .ok_or("主创作会话不存在")?;
        if parent.parent_id.is_some()
            || !parent.busy()
            || parent.turn_id.as_deref() != Some(turn_id)
        {
            return Err("仅当前执行中的主会话可派发槽位渲染".into());
        }
        if !parent.blueprint_approved {
            return Ok(json!({
                "ok": false,
                "gate": "blueprint_unapproved",
                "message": "蓝图门禁拦截：当前蓝图尚未经用户在右侧工作台点击「确认蓝图」放行。请先结束本轮回复并请用户确认蓝图。"
            }));
        }
        let slot_id = if let Some(s) = arguments.get("slotId").and_then(Value::as_str) {
            validate_slot_id(s)?
        } else if let Some(first) = parent.blueprint.slots.first() {
            first.slot_id.clone()
        } else {
            return Err("当前蓝图没有可渲染的槽位".into());
        };
        let note = arguments
            .get("revisionNote")
            .or_else(|| arguments.get("intent"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let worker_id = self.spawn_slot_worker(&parent.id, &slot_id, note, false)?;
        let started = std::time::Instant::now();
        loop {
            let worker = self.get(&worker_id)?;
            if !worker.busy() {
                let refreshed = self.get(&parent.id)?;
                let slot = refreshed
                    .blueprint
                    .slots
                    .iter()
                    .find(|s| s.slot_id == slot_id);
                return Ok(json!({
                    "ok": worker.status == "completed" && worker.error.is_none(),
                    "taskId": worker_id,
                    "slotId": slot_id,
                    "status": worker.status,
                    "error": worker.error,
                    "versions": slot.map(|s| &s.versions),
                }));
            }
            if started.elapsed() > Duration::from_secs(1800) {
                let _ = self.cancel(&worker_id);
                return Err("槽位生图等待超时，已请求停止；请检查状态，不自动重试".into());
            }
            if !self.get(&parent.id)?.busy() {
                let _ = self.cancel(&worker_id);
                return Err("主会话已停止，已同步停止子任务".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    pub fn render_slot_from_ui(
        &self,
        parent_id: &str,
        slot_id: &str,
        revision_note: String,
    ) -> Result<CreationJob, String> {
        let slot_id = validate_slot_id(slot_id)?;
        let _ = self.spawn_slot_worker(parent_id, &slot_id, revision_note, true)?;
        self.get(parent_id)
    }

    fn spawn_slot_worker(
        &self,
        parent_id: &str,
        slot_id: &str,
        revision_note: String,
        approve_from_ui: bool,
    ) -> Result<String, String> {
        let _guard = self.0.dispatch.lock().map_err(|_| "渲染派发正忙")?;
        let mut parent = self.get(parent_id)?;
        if parent.parent_id.is_some() {
            return Err("不能从子任务再次派发槽位任务".into());
        }
        if !approve_from_ui && !parent.blueprint_approved {
            return Err("请先确认蓝图后再执行槽位渲染".into());
        }
        if self
            .list()?
            .iter()
            .filter(|j| j.parent_id.as_deref() == Some(parent_id) && j.busy())
            .count()
            >= 3
        {
            return Err("最多同时执行 3 个槽位生成任务，请等待当前任务完成".into());
        }
        let adapter = self.0.adapters.get(&parent.adapter_id)?;
        let caps = adapter.status(false).capabilities;

        let slot_idx = parent
            .blueprint
            .slots
            .iter()
            .position(|s| s.slot_id == slot_id)
            .ok_or_else(|| format!("蓝图中不存在槽位 {slot_id}"))?;
        if parent.blueprint.slots[slot_idx].status == "running" {
            return Err("该槽位正在生成中，请勿重复提交".into());
        }

        let previous_version = parent.blueprint.slots[slot_idx].versions.last().cloned();
        if previous_version.is_some() {
            if !caps.contains(&Capability::ImageRevision) {
                return Err("当前所选 Agent 适配器不支持基于原图微调".into());
            }
        } else if !caps.contains(&Capability::ImageGeneration) {
            return Err("当前所选 Agent 适配器不支持原生生图".into());
        }

        if approve_from_ui {
            parent.blueprint_approved = true;
        }
        parent.stage = HarnessStage::Rendering;
        parent.blueprint.slots[slot_idx].status = "running".into();
        parent.blueprint.slots[slot_idx].error = None;
        let slot_snapshot = parent.blueprint.slots[slot_idx].clone();
        write_workspace_contract_files(&self.0.root, &parent)?;
        self.put(parent.clone())?;

        let worker_id = Uuid::new_v4().to_string();
        let worker_dir = self.0.root.join(&worker_id);
        fs::create_dir_all(&worker_dir).map_err(|_| "无法创建槽位渲染沙盒")?;

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
            "你是 CreatorX 独立槽位生图子会话，只负责渲染槽位 `{}`（{}）。\n\
            交付制式：{}\n\
            图内准确文案 / 镜头台词：\n{}\n\n\
            画面构图与视觉描述：\n{}\n",
            slot_snapshot.slot_id,
            slot_snapshot.label,
            parent.deliverable.label(),
            if slot_snapshot.text.trim().is_empty() {
                "（无强制图内文字）"
            } else {
                slot_snapshot.text.trim()
            },
            if slot_snapshot.visual_prompt.trim().is_empty() {
                "沿用 IP 底座与样板画风，简洁留白"
            } else {
                slot_snapshot.visual_prompt.trim()
            }
        );
        if worker_dir.join("ip.md").is_file() {
            prompt_body.push_str("\n请先读取本目录 `ip.md` 及 `references/` 中的标准角色参考图保持形象一致。");
        }
        if worker_dir.join("prompts.md").is_file() {
            prompt_body.push_str("\n请同时遵循本目录 `prompts.md` 中的风格样板要求。");
        }
        if let Some(prev) = &previous_version {
            let src_file = parent_dir.join(&prev.path);
            if !src_file.is_file() {
                return Err("找不到该槽位的上一版真实原图，拒绝凭空声称改图".into());
            }
            let ext = src_file
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("png");
            let dest_prev = worker_dir.join("previous").join(format!("v{}.{ext}", prev.version));
            atomic_save_file(
                &dest_prev,
                &fs::read(&src_file).map_err(|_| "无法读取上一版原图")?,
            )?;
            prompt_body.push_str(&format!(
                "\n\n【原图受控微调】必须引用实际上一版图片 `{}` 进行修改，保持其余构图与主体一致。本轮修改要求：\n{}",
                dest_prev.display(),
                if revision_note.trim().is_empty() {
                    "按最新槽位文案与构图微调细节"
                } else {
                    revision_note.trim()
                }
            ));
        } else if !revision_note.trim().is_empty() {
            prompt_body.push_str(&format!("\n补充生成说明：{}\n", revision_note.trim()));
        }
        prompt_body.push_str(
            "\n必须调用原生 image_gen / image_generation 工具产出真实图片文件并读取检查；不得使用外部付费 API 或代码绘图冒充。",
        );

        let skill_md = worker_dir.join("skill/SKILL.md");
        let worker = CreationJob {
            parent_id: Some(parent.id.clone()),
            source_task_id: previous_version.map(|v| v.task_id),
            slot_id: Some(slot_id.to_string()),
            revision_note: revision_note.trim().into(),
            id: worker_id.clone(),
            title: format!("{} · {}", slot_snapshot.label, parent.title),
            deliverable: parent.deliverable,
            adapter_id: parent.adapter_id.clone(),
            include_ip: parent.include_ip,
            track: parent.track.clone(),
            skill_id: parent.skill_id.clone(),
            skill_name: parent.skill_name.clone(),
            prompt_ids: parent.prompt_ids.clone(),
            snapshot_seq: parent.snapshot_seq,
            stage: HarnessStage::Rendering,
            blueprint_approved: true,
            blueprint: Blueprint::default(),
            thread_id: None,
            turn_id: None,
            status: "starting".into(),
            messages: vec![],
            images: vec![],
            error: None,
            activity: format!("正在生成 {}", slot_snapshot.label),
            updated_at: now_secs(),
        };
        self.put(worker)?;

        let req = TurnRequest {
            thread_id: None,
            cwd: worker_dir,
            developer_instructions: "你是 CreatorX 受控槽位生图子会话。直接按指令使用原生生图工具生成一张高质量图片并核对，不派生其他代理，不修改外部目录。".into(),
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
                let mut p = self.get(parent_id)?;
                if let Some(s) = p.blueprint.slots.iter_mut().find(|s| s.slot_id == slot_id) {
                    s.status = "error".into();
                    s.error = Some(err.clone());
                }
                self.put(p)?;
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
                        for slot in &mut job.blueprint.slots {
                            if slot.status == "running" {
                                slot.status = "error".into();
                                slot.error = Some(error.clone());
                            }
                        }
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
                    .map_err(|_| "生成图片的 Base64 数据无效".to_string())?
            } else {
                return Err("生图事件未返回可读取的图片文件".into());
            };
            let ext = validate_image_magic(&bytes)
                .ok_or_else(|| "生图结果未通过 PNG/JPEG/WebP 文件头硬校验".to_string())?;
            Ok((bytes, ext))
        })();

        let Ok(mut jobs) = self.0.jobs.lock() else {
            return;
        };
        let Some(worker_id) = jobs
            .values()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
            .map(|j| j.id.clone())
        else {
            return;
        };

        match raw {
            Err(err) => {
                if let Some(worker) = jobs.get_mut(&worker_id) {
                    worker.error = Some(err);
                    let _ = save_job_record(&self.0.root, worker);
                }
            }
            Ok((bytes, ext)) => {
                let worker_rel = format!("artifacts/{}.{ext}", Uuid::new_v4());
                if let Err(err) =
                    atomic_save_file(&self.0.root.join(&worker_id).join(&worker_rel), &bytes)
                {
                    if let Some(worker) = jobs.get_mut(&worker_id) {
                        worker.error = Some(err);
                    }
                    return;
                }
                let (parent_id_opt, slot_id_opt, note) = {
                    let worker = jobs.get_mut(&worker_id).unwrap();
                    worker.images.push(worker_rel.clone());
                    worker.updated_at = now_secs();
                    let _ = save_job_record(&self.0.root, worker);
                    (
                        worker.parent_id.clone(),
                        worker.slot_id.clone(),
                        worker.revision_note.clone(),
                    )
                };

                // Append-only write into parent job's slot version chain (`artifacts/<slot-id>/vN.<ext>`)
                if let (Some(parent_id), Some(slot_id)) = (parent_id_opt, slot_id_opt) {
                    if let Some(parent) = jobs.get_mut(&parent_id) {
                        if let Some(slot) = parent
                            .blueprint
                            .slots
                            .iter_mut()
                            .find(|s| s.slot_id == slot_id)
                        {
                            let next_ver = slot
                                .versions
                                .iter()
                                .map(|v| v.version)
                                .max()
                                .unwrap_or(0)
                                + 1;
                            let slot_rel = format!("artifacts/{slot_id}/v{next_ver}.{ext}");
                            let slot_dest = self.0.root.join(&parent_id).join(&slot_rel);
                            if slot_dest.exists() {
                                slot.error = Some("检测到同名历史版本文件，拒绝覆盖".into());
                                return;
                            }
                            if let Err(err) = atomic_save_file(&slot_dest, &bytes) {
                                slot.error = Some(err);
                                return;
                            }
                            slot.versions.push(ArtifactVersion {
                                version: next_ver,
                                path: slot_rel.clone(),
                                task_id: worker_id,
                                note,
                                created_at: now_secs(),
                            });
                            parent.images.push(slot_rel);
                            parent.updated_at = now_secs();
                            let _ = save_job_record(&self.0.root, parent);
                        }
                    }
                }
            }
        }
    }

    fn complete_turn(&self, thread_id: &str, status: String, error: Option<String>) {
        let Ok(mut jobs) = self.0.jobs.lock() else {
            return;
        };
        let Some(job_id) = jobs
            .values()
            .find(|j| j.thread_id.as_deref() == Some(thread_id))
            .map(|j| j.id.clone())
        else {
            return;
        };

        let (parent_id_opt, slot_id_opt, final_status, final_error) = {
            let job = jobs.get_mut(&job_id).unwrap();
            job.status = status;
            if let Some(err) = error {
                job.error = Some(err);
            }
            if job.parent_id.is_none() {
                sync_disk_contract_if_modified(&self.0.root, job);
            } else if job.status == "completed" && job.images.is_empty() {
                // Hard Gate #3: verbal completion without a verified image file is a failure.
                job.status = "error".into();
                job.error.get_or_insert_with(|| {
                    "产物硬校验未通过：子会话未产出真实图片文件，该槽位尚未完成".into()
                });
            }
            job.activity = if job.status == "completed" {
                "已完成".into()
            } else {
                "未完成，请查看提示".into()
            };
            job.updated_at = now_secs();
            let _ = write_workspace_contract_files(&self.0.root, job);
            let _ = save_job_record(&self.0.root, job);
            (
                job.parent_id.clone(),
                job.slot_id.clone(),
                job.status.clone(),
                job.error.clone(),
            )
        };

        if let (Some(parent_id), Some(slot_id)) = (parent_id_opt, slot_id_opt) {
            if let Some(parent) = jobs.get_mut(&parent_id) {
                if let Some(slot) = parent
                    .blueprint
                    .slots
                    .iter_mut()
                    .find(|s| s.slot_id == slot_id)
                {
                    if final_status == "completed" && final_error.is_none() {
                        slot.status = "completed".into();
                        slot.error = None;
                    } else {
                        slot.status = "error".into();
                        slot.error = final_error;
                    }
                }
                let any_running = parent.blueprint.slots.iter().any(|s| s.status == "running");
                let any_completed = parent
                    .blueprint
                    .slots
                    .iter()
                    .any(|s| !s.versions.is_empty());
                if !any_running && any_completed {
                    parent.stage = HarnessStage::Review;
                }
                parent.updated_at = now_secs();
                let _ = save_job_record(&self.0.root, parent);
            }
        }
    }

    pub fn promote_to_showcase(
        &self,
        store: &LibraryStore,
        job_id: &str,
        req: PromoteShowcaseRequest,
    ) -> Result<Entry, String> {
        let job = self.get(job_id)?;
        if job.parent_id.is_some() {
            return Err("请从主创作任务提炼样板".into());
        }
        let job_dir = self.0.root.join(&job.id);
        let mut image_hashes = Vec::new();
        for slot in &job.blueprint.slots {
            if !req.slot_ids.is_empty() && !req.slot_ids.contains(&slot.slot_id) {
                continue;
            }
            if let Some(latest) = slot.versions.last() {
                let bytes = fs::read(job_dir.join(&latest.path))
                    .map_err(|_| "无法读取成品原图以提炼样板")?;
                let hash_name = store.import_image_bytes(&bytes)?;
                if !image_hashes.contains(&hash_name) && image_hashes.len() < 16 {
                    image_hashes.push(hash_name);
                }
            }
        }
        let fallback_output = if !req.sample_output.trim().is_empty() {
            req.sample_output
        } else if !job.blueprint.body_markdown.trim().is_empty() {
            job.blueprint
                .body_markdown
                .chars()
                .take(4_000)
                .collect::<String>()
        } else {
            job.blueprint.summary_copy.clone()
        };
        let entry = Entry {
            schema_version: 3,
            id: String::new(),
            kind: Kind::Prompt,
            name: req.name.trim().into(),
            prompt: req.prompt,
            category: req.category.trim().into(),
            deliverable: job.deliverable,
            track: job.track.clone(),
            maturity: Maturity::Showcase,
            sample_input: if req.sample_input.trim().is_empty() {
                job.title.clone()
            } else {
                req.sample_input
            },
            sample_output: fallback_output,
            tags: vec![],
            notes: req.notes,
            origin_created_at: None,
            origin_updated_at: None,
            skill_markdown: String::new(),
            skill_digest: None,
            images: image_hashes,
            revision: 0,
            updated_at: 0,
            source: Some(format!("提炼自创作任务 {}", job.title)),
        };
        store.save(entry, 0, None)
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
            return Err("不能打开此类型或任务沙盒以外的文件".into());
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
    deliverable: DeliverableFormat,
    adapter_id: Option<String>,
    include_ip: Option<bool>,
    track: Option<String>,
    skill_id: Option<String>,
    prompt_ids: Vec<String>,
    brief: String,
) -> Result<CreationJob, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine.create(
            app.state::<LibraryStore>().inner(),
            deliverable,
            adapter_id.as_deref().unwrap_or("codex"),
            include_ip.unwrap_or(true),
            track.unwrap_or_default(),
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
pub async fn creation_save_blueprint(
    engine: tauri::State<'_, HarnessEngine>,
    id: String,
    blueprint: Blueprint,
    approve: bool,
) -> Result<CreationJob, String> {
    engine.save_blueprint(&id, blueprint, approve)
}

#[tauri::command]
pub async fn creation_render_slot(
    engine: tauri::State<'_, HarnessEngine>,
    id: String,
    slot_id: String,
    revision_note: Option<String>,
) -> Result<CreationJob, String> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        engine.render_slot_from_ui(&id, &slot_id, revision_note.unwrap_or_default())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn creation_promote_showcase(
    app: tauri::AppHandle,
    engine: tauri::State<'_, HarnessEngine>,
    id: String,
    request: PromoteShowcaseRequest,
) -> Result<Entry, String> {
    let store = app.state::<LibraryStore>();
    engine.promote_to_showcase(store.inner(), &id, request)
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
    use crate::library::IpFoundation;
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
                    text: "已完成本轮处理".into(),
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
            if !job.busy()
                && job
                    .blueprint
                    .slots
                    .iter()
                    .all(|s| s.status != "running")
            {
                return job;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("job {id} did not settle");
    }

    #[test]
    fn harness_enforces_immutable_snapshots_blueprint_gate_version_chain_and_showcase_promotion() {
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().join("library")).unwrap();
        store
            .save_ip(
                IpFoundation {
                    name: "安静".into(),
                    visual_prompt: "蛋黄黄乱发、黑卫衣、粗蜡笔纸纹".into(),
                    images: vec![],
                    voice_prompt: "直白克制".into(),
                    voice_sample: String::new(),
                    tracks: vec![],
                    revision: 0,
                    updated_at: 0,
                },
                0,
            )
            .unwrap();

        let registry = AdapterRegistry::with_adapters(vec![Arc::new(MockAdapter::new(true))]);
        let engine = HarnessEngine::with_registry(root.path().join("creations"), registry).unwrap();

        let created = engine
            .create(
                &store,
                DeliverableFormat::Cards,
                "mock",
                true,
                "计算机".into(),
                "builtin:cards-series",
                vec![],
                "拆解 Creation Harness 是什么".into(),
            )
            .unwrap();
        let job = wait_idle(&engine, &created.id);
        assert_eq!(job.snapshot_seq, 1);
        let snap1 = root
            .path()
            .join("creations")
            .join(&job.id)
            .join("snapshots/1/ip.md");
        assert!(snap1.is_file());

        // Switching attachments creates Snapshot #2 without overwriting Snapshot #1
        engine
            .continue_chat(
                &job.id,
                "补充一点细节".into(),
                Some((
                    &store,
                    AttachmentsUpdate {
                        include_ip: true,
                        track: "金融".into(),
                        skill_id: "builtin:image-single".into(),
                        prompt_ids: vec![],
                    },
                )),
                None,
            )
            .unwrap();
        let job = wait_idle(&engine, &job.id);
        assert_eq!(job.snapshot_seq, 2);
        assert!(snap1.is_file());
        assert!(root
            .path()
            .join("creations")
            .join(&job.id)
            .join("snapshots/2/skill/SKILL.md")
            .is_file());

        // Save unapproved blueprint -> internal render without approval must be rejected
        let bp = Blueprint {
            summary_copy: "Harness 核心三件事".into(),
            body_markdown: String::new(),
            slots: vec![BlueprintSlot {
                slot_id: "card-1".into(),
                label: "第 1 页 · 封面".into(),
                text: "不做新 Agent，做创作治具".into(),
                visual_prompt: "安静站在架构白板前".into(),
                duration_sec: None,
                status: "pending".into(),
                error: None,
                versions: vec![],
            }],
        };
        let saved = engine.save_blueprint(&job.id, bp, false).unwrap();
        assert!(!saved.blueprint_approved);
        assert!(engine
            .spawn_slot_worker(&job.id, "card-1", String::new(), false)
            .is_err());

        // Render from UI approves blueprint and produces v1.png; revising produces v2.png without overwriting v1.png
        engine
            .render_slot_from_ui(&job.id, "card-1", String::new())
            .unwrap();
        let after_v1 = wait_idle(&engine, &job.id);
        assert_eq!(after_v1.stage, HarnessStage::Review);
        assert_eq!(after_v1.blueprint.slots[0].versions.len(), 1);
        let v1_path = root
            .path()
            .join("creations")
            .join(&job.id)
            .join(&after_v1.blueprint.slots[0].versions[0].path);
        assert!(v1_path.is_file());

        engine
            .render_slot_from_ui(&job.id, "card-1", "标题加大一点".into())
            .unwrap();
        let after_v2 = wait_idle(&engine, &job.id);
        assert_eq!(after_v2.blueprint.slots[0].versions.len(), 2);
        let v2_path = root
            .path()
            .join("creations")
            .join(&job.id)
            .join(&after_v2.blueprint.slots[0].versions[1].path);
        assert!(v1_path.is_file(), "v1 must never be overwritten or deleted");
        assert!(v2_path.is_file());
        assert_ne!(v1_path, v2_path);

        // Showcase promotion writes verified image + metadata back into LibraryStore
        let promoted = engine
            .promote_to_showcase(
                &store,
                &job.id,
                PromoteShowcaseRequest {
                    name: "架构拆解封面样板".into(),
                    category: "视觉画风".into(),
                    prompt: "暖米白纸纹、粗蜡笔黑卫衣角色站白板旁".into(),
                    sample_input: String::new(),
                    sample_output: String::new(),
                    notes: "实测有效".into(),
                    slot_ids: vec!["card-1".into()],
                },
            )
            .unwrap();
        assert_eq!(promoted.maturity, Maturity::Showcase);
        assert_eq!(promoted.images.len(), 1);
        assert_eq!(store.list().unwrap().entries.len(), 1);
    }

    #[test]
    fn missing_worker_image_fails_artifact_verification_honestly() {
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().join("library")).unwrap();
        let registry = AdapterRegistry::with_adapters(vec![Arc::new(MockAdapter::new(false))]);
        let engine = HarnessEngine::with_registry(root.path().join("creations"), registry).unwrap();

        let created = engine
            .create(
                &store,
                DeliverableFormat::Article,
                "mock",
                false,
                String::new(),
                "builtin:article-illustrated",
                vec![],
                "写一篇架构文章".into(),
            )
            .unwrap();
        let job = wait_idle(&engine, &created.id);
        engine
            .save_blueprint(
                &job.id,
                Blueprint {
                    summary_copy: String::new(),
                    body_markdown: "# 标题\n\n![封面](slot:cover)".into(),
                    slots: vec![BlueprintSlot {
                        slot_id: "cover".into(),
                        label: "封面图".into(),
                        text: String::new(),
                        visual_prompt: "简洁插画".into(),
                        duration_sec: None,
                        status: "pending".into(),
                        error: None,
                        versions: vec![],
                    }],
                },
                true,
            )
            .unwrap();
        engine
            .render_slot_from_ui(&job.id, "cover", String::new())
            .unwrap();
        let failed = wait_idle(&engine, &job.id);
        assert_eq!(failed.blueprint.slots[0].status, "error");
        assert!(failed.blueprint.slots[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("未产出真实图片文件"));
    }
}
