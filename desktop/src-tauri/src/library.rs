use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

fn current_schema() -> u32 {
    5
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[serde(alias = "ip")]
    Prompt,
    #[serde(alias = "scene")]
    Skill,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Modality {
    #[serde(alias = "writing", alias = "article")]
    Text,
    #[default]
    #[serde(alias = "cards", alias = "general")]
    Image,
    Video,
}

impl Modality {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "文",
            Self::Image => "图",
            Self::Video => "视频",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(default = "current_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    pub kind: Kind,
    #[serde(default)]
    pub modality: Modality,
    pub name: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default, alias = "sampleOutput")]
    pub sample: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub origin_created_at: Option<String>,
    #[serde(default)]
    pub origin_updated_at: Option<String>,
    #[serde(default)]
    pub skill_markdown: String,
    #[serde(default)]
    pub skill_digest: Option<String>,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub entries: Vec<Entry>,
    pub image_directory: String,
    pub tags: Vec<String>,
    pub state_token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    pub skill: Option<Entry>,
    pub prompts: Vec<Entry>,
    pub references: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportFile {
    pub(crate) path: String,
    pub(crate) base64: String,
}

#[derive(Serialize, Deserialize)]
struct SkillTransaction {
    id: String,
    staged: String,
    backup: Option<String>,
    revision: u64,
}

pub fn normalize_tag(raw: &str) -> Result<String, String> {
    let clean = raw.trim().trim_start_matches('#').trim();
    if clean.is_empty() || clean.chars().count() > 32 || clean.chars().any(char::is_control) {
        return Err("标签名称需为 1–32 个有效字符".into());
    }
    Ok(clean.to_string())
}

pub struct LibraryStore {
    root: PathBuf,
    lock: Mutex<()>,
}

impl LibraryStore {
    fn lock(&self) -> Result<(std::sync::MutexGuard<'_, ()>, fs::File), String> {
        let guard = self.lock.lock().map_err(|_| "样板间正忙，请重试")?;
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(self.root.join(".lock"))
            .map_err(|_| "无法锁定样板间")?;
        file.lock().map_err(|_| "无法锁定样板间")?;
        Ok((guard, file))
    }

    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("entries")).map_err(|_| "无法创建样板目录")?;
        fs::create_dir_all(root.join("images")).map_err(|_| "无法创建图片目录")?;
        fs::create_dir_all(root.join("skills")).map_err(|_| "无法创建 Skills 目录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护样板目录")?;
        }
        let store = Self {
            root,
            lock: Mutex::new(()),
        };
        {
            let _guard = store.lock()?;
            store.recover_transactions()?;
        }
        Ok(store)
    }

    fn now_secs() -> Result<u64, String> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效".into())
            .map(|d| d.as_secs())
    }

    fn entry_path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err("资产标识无效".into());
        }
        Ok(self.root.join("entries").join(format!("{id}.json")))
    }

    fn read_tags_unlocked(&self) -> Result<Vec<String>, String> {
        let path = self.root.join("tags.json");
        if !path.exists() {
            return Ok(vec![]);
        }
        let raw: Vec<String> =
            serde_json::from_slice(&fs::read(path).map_err(|_| "无法读取标签表")?)
                .map_err(|_| "标签表文件损坏")?;
        if raw.len() > 500 {
            return Err("标签数量超限".into());
        }
        let mut out = Vec::new();
        for t in raw {
            let norm = normalize_tag(&t)?;
            if !out.contains(&norm) {
                out.push(norm);
            }
        }
        Ok(out)
    }

    fn write_tags_unlocked(&self, tags: &[String]) -> Result<(), String> {
        self.atomic_write(
            &self.root.join("tags.json"),
            &serde_json::to_vec_pretty(tags).map_err(|_| "无法保存标签表")?,
        )
    }

    pub fn tag_list(&self) -> Result<Vec<String>, String> {
        let _guard = self.lock()?;
        self.read_tags_unlocked()
    }

    pub fn tag_add(&self, name: &str) -> Result<Vec<String>, String> {
        let _guard = self.lock()?;
        let norm = normalize_tag(name)?;
        let mut tags = self.read_tags_unlocked()?;
        if !tags.contains(&norm) {
            if tags.len() >= 500 {
                return Err("受控标签最多维护 500 个".into());
            }
            tags.push(norm);
            self.write_tags_unlocked(&tags)?;
        }
        Ok(tags)
    }

    pub fn tag_rename(&self, from: &str, to: &str) -> Result<Vec<String>, String> {
        let _guard = self.lock()?;
        let from_norm = normalize_tag(from)?;
        let to_norm = normalize_tag(to)?;
        let mut tags = self.read_tags_unlocked()?;
        let Some(pos) = tags.iter().position(|t| t == &from_norm) else {
            return Err(format!("标签「{from_norm}」不存在"));
        };
        if from_norm == to_norm {
            return Ok(tags);
        }
        if tags.contains(&to_norm) {
            tags.remove(pos);
        } else {
            tags[pos] = to_norm.clone();
        }
        let now = Self::now_secs()?;
        for file in fs::read_dir(self.root.join("entries")).map_err(|_| "无法读取样板")? {
            let path = file.map_err(|_| "无法读取样板")?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取样板")?)
                    .map_err(|_| "样板记录损坏")?;
            if entry.tags.contains(&from_norm) {
                let mut seen = std::collections::HashSet::new();
                entry.tags = entry
                    .tags
                    .into_iter()
                    .map(|t| if t == from_norm { to_norm.clone() } else { t })
                    .filter(|t| seen.insert(t.clone()))
                    .collect();
                entry.revision = entry.revision.checked_add(1).ok_or("修订号无效")?;
                entry.updated_at = now;
                self.atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&entry).map_err(|_| "无法更新关联样板")?,
                )?;
            }
        }
        self.write_tags_unlocked(&tags)?;
        Ok(tags)
    }

    pub fn tag_delete(&self, name: &str) -> Result<Vec<String>, String> {
        let _guard = self.lock()?;
        let norm = normalize_tag(name)?;
        let mut tags = self.read_tags_unlocked()?;
        let before = tags.len();
        tags.retain(|t| t != &norm);
        if tags.len() == before {
            return Err(format!("标签「{norm}」不存在"));
        }
        let now = Self::now_secs()?;
        for file in fs::read_dir(self.root.join("entries")).map_err(|_| "无法读取样板")? {
            let path = file.map_err(|_| "无法读取样板")?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取样板")?)
                    .map_err(|_| "样板记录损坏")?;
            let old_len = entry.tags.len();
            entry.tags.retain(|t| t != &norm);
            if entry.tags.len() != old_len {
                entry.revision = entry.revision.checked_add(1).ok_or("修订号无效")?;
                entry.updated_at = now;
                self.atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&entry).map_err(|_| "无法更新关联样板")?,
                )?;
            }
        }
        self.write_tags_unlocked(&tags)?;
        Ok(tags)
    }

    fn validate_image_refs(&self, images: &[String]) -> Result<(), String> {
        if images.len() > 16 {
            return Err("每份样板最多保存 16 张图片".into());
        }
        for name in images {
            let (hash, extension) = name.rsplit_once('.').ok_or("图片标识无效")?;
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                || !["png", "jpg", "webp"].contains(&extension)
            {
                return Err("图片标识无效".into());
            }
            let path = self.root.join("images").join(name);
            if !path.is_file()
                || fs::symlink_metadata(&path)
                    .map_err(|_| "无法读取图片")?
                    .file_type()
                    .is_symlink()
            {
                return Err("样板图片不存在，请先导入图片再引用".into());
            }
        }
        Ok(())
    }

    fn validate(&self, entry: &Entry) -> Result<(), String> {
        if !(1..=5).contains(&entry.schema_version) {
            return Err("资产版本暂不支持".into());
        }
        if entry.name.trim().is_empty() || entry.name.chars().count() > 80 {
            return Err("请填写不超过 80 字的名称".into());
        }
        if entry.prompt.len() > 64_000 || entry.skill_markdown.len() > 256_000 {
            return Err("正文文档过长".into());
        }
        if entry.sample.len() > 32_000 {
            return Err("样板示例文字过长".into());
        }
        if entry.tags.len() > 24 || entry.notes.len() > 16_000 {
            return Err("标签或备注过长（最多 24 个标签）".into());
        }
        for tag in &entry.tags {
            normalize_tag(tag)?;
        }
        if entry
            .origin_created_at
            .as_ref()
            .is_some_and(|s| s.len() > 100)
            || entry
                .origin_updated_at
                .as_ref()
                .is_some_and(|s| s.len() > 100)
        {
            return Err("来源时间无效".into());
        }
        if entry
            .source
            .as_ref()
            .is_some_and(|source| source.len() > 4096)
        {
            return Err("来源说明过长".into());
        }
        if entry.kind == Kind::Prompt && !entry.skill_markdown.is_empty()
            || entry.kind == Kind::Skill && !entry.prompt.is_empty()
        {
            return Err("提示词与 Skill 请分别保存".into());
        }
        self.validate_image_refs(&entry.images)
    }

    pub fn delete_entry(&self, id: &str, expected_revision: u64) -> Result<(), String> {
        let _guard = self.lock()?;
        let path = self.entry_path(id)?;
        if !path.is_file() {
            return Err("样板不存在".into());
        }
        let entry: Entry = serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取样板")?)
            .map_err(|_| "样板记录损坏")?;
        if entry.revision != expected_revision {
            return Err("样板已在其他位置修改，请先重新读取最新 revision 再删除".into());
        }
        fs::remove_file(&path).map_err(|_| "无法删除样板记录")?;
        if entry.kind == Kind::Skill {
            let dir = self.skill_path(id)?;
            if dir.exists() {
                fs::remove_dir_all(dir).map_err(|_| "无法清理 Skill 文件夹")?;
            }
        }
        Ok(())
    }

    pub fn snapshot_assets(
        &self,
        skill_id: &str,
        prompt_ids: &[String],
        destination: &Path,
    ) -> Result<SnapshotManifest, String> {
        let _guard = self.lock()?;
        fs::create_dir_all(destination).map_err(|_| "无法创建快照目录")?;
        let read = |id: &str| -> Result<Entry, String> {
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(self.entry_path(id)?).map_err(|_| "样板不存在")?)
                    .map_err(|_| "样板记录损坏")?;
            self.hydrate_skill(&mut entry)?;
            self.validate(&entry)?;
            Ok(entry)
        };
        let skill = if skill_id.is_empty() {
            None
        } else {
            let entry = if skill_id.starts_with("builtin:") {
                crate::builtin_skills::get(skill_id).ok_or("内置 Skill 不存在")?
            } else {
                read(skill_id)?
            };
            if entry.kind != Kind::Skill {
                return Err("请选择真实 Skill 文件夹".into());
            }
            let folder = destination.join("skill");
            fs::create_dir_all(&folder).map_err(|_| "无法创建 Skill 快照")?;
            let files = if skill_id.starts_with("builtin:") {
                vec![("SKILL.md".into(), entry.skill_markdown.as_bytes().to_vec())]
            } else {
                Self::read_package(&self.root.join("skills").join(skill_id))?
            };
            for (name, bytes) in files {
                let path = folder.join(name);
                fs::create_dir_all(path.parent().ok_or("路径无效")?)
                    .map_err(|_| "无法创建资源目录")?;
                self.atomic_write(&path, &bytes)?;
            }
            Some(entry)
        };

        let mut prompts = Vec::new();
        for id in prompt_ids {
            let entry = read(id)?;
            if entry.kind != Kind::Prompt {
                return Err("附加引用应为提示词".into());
            }
            prompts.push(entry);
        }

        let refs_dir = destination.join("references");
        fs::create_dir_all(&refs_dir).map_err(|_| "无法创建参考图目录")?;
        let mut references = Vec::new();
        for entry in skill.iter().chain(prompts.iter()) {
            for name in &entry.images {
                if !references.iter().any(|existing| existing == name) {
                    self.atomic_write(
                        &refs_dir.join(name),
                        &fs::read(self.root.join("images").join(name))
                            .map_err(|_| "无法读取参考图")?,
                    )?;
                    references.push(name.clone());
                }
            }
        }

        if !prompts.is_empty() {
            let mut doc = String::from("# 本轮引用的提示词样板\n\n");
            for item in &prompts {
                let tag_line = if item.tags.is_empty() {
                    String::new()
                } else {
                    format!(
                        "（{}）",
                        item.tags
                            .iter()
                            .map(|t| format!("#{t}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                };
                doc.push_str(&format!(
                    "## [{}] {}{}\n{}\n",
                    item.modality.label(),
                    item.name,
                    tag_line,
                    item.prompt.trim()
                ));
                if !item.sample.trim().is_empty() {
                    doc.push_str(&format!("\n样板示例：\n{}\n", item.sample.trim()));
                }
                if !item.images.is_empty() {
                    doc.push_str("\n关联参考图：\n");
                    for img in &item.images {
                        doc.push_str(&format!("- references/{img}\n"));
                    }
                }
                doc.push('\n');
            }
            self.atomic_write(&destination.join("prompts.md"), doc.as_bytes())?;
        }

        let manifest = SnapshotManifest {
            skill,
            prompts,
            references,
        };
        self.atomic_write(
            &destination.join("materials.json"),
            &serde_json::to_vec_pretty(&json!(manifest)).map_err(|_| "无法整理快照清单")?,
        )?;
        Ok(manifest)
    }

    fn compute_state_token(tags: &[String], entries: &[Entry]) -> String {
        let mut hasher = Sha256::new();
        for t in tags {
            hasher.update(t.as_bytes());
            hasher.update(b"|");
        }
        hasher.update(b"##");
        for e in entries {
            hasher.update(e.id.as_bytes());
            hasher.update(b":");
            hasher.update(e.revision.to_le_bytes());
            hasher.update(b":");
            hasher.update(e.updated_at.to_le_bytes());
            if let Some(d) = &e.skill_digest {
                hasher.update(d.as_bytes());
            }
            hasher.update(b";");
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn state_token(&self) -> Result<String, String> {
        Ok(self.list()?.state_token)
    }

    pub fn list(&self) -> Result<Library, String> {
        let _guard = self.lock()?;
        let mut entries = Vec::new();
        for file in fs::read_dir(self.root.join("entries")).map_err(|_| "无法读取样板间")? {
            let file = file.map_err(|_| "无法读取样板")?;
            if file.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if !file.file_type().map_err(|_| "无法读取样板")?.is_file() {
                return Err("样板文件类型无效".into());
            }
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(file.path()).map_err(|_| "无法读取样板")?)
                    .map_err(|_| "样板记录损坏，请保留文件后检查")?;
            self.hydrate_skill(&mut entry)?;
            self.validate(&entry)?;
            if self.entry_path(&entry.id)? != file.path() {
                return Err("样板文件与标识不一致".into());
            }
            entries.push(entry);
        }
        entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.name.cmp(&b.name)));
        let mut tags = self.read_tags_unlocked()?;
        for entry in &entries {
            for tag in &entry.tags {
                if !tag.is_empty() && !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
        }
        let state_token = Self::compute_state_token(&tags, &entries);
        Ok(Library {
            tags,
            entries,
            image_directory: self.root.join("images").to_string_lossy().into_owned(),
            state_token,
        })
    }

    fn atomic_write(&self, destination: &Path, bytes: &[u8]) -> Result<(), String> {
        let temporary = destination.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary).map_err(|_| "无法保存样板")?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "无法保存样板")?;
            fs::rename(&temporary, destination).map_err(|_| "无法替换样板记录")
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(String::from)
    }

    fn recover_transactions(&self) -> Result<(), String> {
        let skills = self.root.join("skills");
        for file in fs::read_dir(&skills).map_err(|_| "无法检查 Skill 保存状态")? {
            let file = file.map_err(|_| "无法检查 Skill 保存状态")?;
            let name = file.file_name().to_string_lossy().into_owned();
            if !name.starts_with(".transaction-") || !name.ends_with(".json") {
                continue;
            }
            if !file
                .file_type()
                .map_err(|_| "无法检查 Skill 保存状态")?
                .is_file()
            {
                return Err("Skill 保存日志类型无效".into());
            }
            let tx: SkillTransaction = serde_json::from_slice(
                &fs::read(file.path()).map_err(|_| "无法读取 Skill 保存日志")?,
            )
            .map_err(|_| "Skill 保存日志损坏，请保留文件后检查")?;
            let metadata = self.entry_path(&tx.id)?;
            if name != format!(".transaction-{}.json", tx.id) {
                return Err("Skill 保存日志标识无效".into());
            }
            for (value, prefix) in std::iter::once((&tx.staged, ".stage-"))
                .chain(tx.backup.iter().map(|value| (value, ".previous-")))
            {
                if value
                    .strip_prefix(prefix)
                    .is_none_or(|s| Uuid::parse_str(s).is_err())
                {
                    return Err("Skill 保存日志路径无效".into());
                }
            }
            let committed = if metadata.is_file() {
                let entry: Entry =
                    serde_json::from_slice(&fs::read(&metadata).map_err(|_| "无法读取样板")?)
                        .map_err(|_| "样板记录损坏，保留 Skill 备份后检查")?;
                entry.revision >= tx.revision
            } else {
                false
            };
            let directory = self.skill_path(&tx.id)?;
            let staged = skills.join(&tx.staged);
            let backup = tx.backup.as_ref().map(|name| skills.join(name));
            if committed {
                if !directory.is_dir() {
                    return Err("已保存的 Skill 文件夹缺失，请保留备份后检查".into());
                }
                if let Some(backup) = &backup {
                    if backup.exists() {
                        fs::remove_dir_all(backup).map_err(|_| "无法整理 Skill 旧版本")?;
                    }
                }
            } else if let Some(backup) = &backup {
                if backup.exists() {
                    if directory.exists() {
                        fs::remove_dir_all(&directory).map_err(|_| "无法恢复 Skill 文件夹")?;
                    }
                    fs::rename(backup, &directory).map_err(|_| "无法恢复 Skill 旧版本")?;
                }
            } else if directory.exists() {
                fs::remove_dir_all(&directory).map_err(|_| "无法撤回未完成的 Skill")?;
            }
            if staged.exists() {
                fs::remove_dir_all(staged).map_err(|_| "无法整理 Skill 暂存")?;
            }
            fs::remove_file(file.path()).map_err(|_| "无法清理 Skill 保存日志")?;
        }
        Ok(())
    }

    fn skill_path(&self, id: &str) -> Result<PathBuf, String> {
        self.entry_path(id)?;
        let path = self.root.join("skills").join(id);
        if path.exists()
            && fs::symlink_metadata(&path)
                .map_err(|_| "无法读取 Skill")?
                .file_type()
                .is_symlink()
        {
            return Err("Skill 目录不能是符号链接".into());
        }
        Ok(path)
    }

    fn skill_document(&self, id: &str) -> Result<Option<String>, String> {
        let path = self.skill_path(id)?.join("SKILL.md");
        if !path.exists() {
            return Ok(None);
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| "无法读取 SKILL.md")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 256_000 {
            return Err("SKILL.md 必须是不超过 256 KB 的普通文件".into());
        }
        Ok(Some(
            fs::read_to_string(path).map_err(|_| "SKILL.md 必须是 UTF-8 文本")?,
        ))
    }

    fn hydrate_skill(&self, entry: &mut Entry) -> Result<(), String> {
        if entry.kind == Kind::Skill {
            if let Some(document) = self.skill_document(&entry.id)? {
                entry.skill_markdown = document;
                entry.skill_digest = Some(format!(
                    "{:x}",
                    Sha256::digest(entry.skill_markdown.as_bytes())
                ));
            }
        }
        Ok(())
    }

    fn valid_relative_path(path: &str) -> bool {
        if path.is_empty()
            || path.len() > 1024
            || path.contains('\\')
            || path.contains(':')
            || path.chars().any(char::is_control)
        {
            return false;
        }
        let parts: Vec<_> = path.split('/').collect();
        parts.len() <= 16
            && parts.iter().all(|part| {
                let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
                !part.is_empty()
                    && *part != "."
                    && *part != ".."
                    && !part.ends_with([' ', '.'])
                    && !part.chars().any(|c| "<>\"|?*".contains(c))
                    && !["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                    && !(stem.len() == 4
                        && (stem.starts_with("COM") || stem.starts_with("LPT"))
                        && stem.as_bytes()[3].is_ascii_digit())
            })
    }

    fn scan_package(path: &Path) -> Result<Vec<(String, u64)>, String> {
        fn walk(
            root: &Path,
            path: &Path,
            files: &mut Vec<(String, u64)>,
            total: &mut u64,
            depth: usize,
        ) -> Result<(), String> {
            if depth > 16 {
                return Err("Skill 目录层级过深".into());
            }
            for file in fs::read_dir(path).map_err(|_| "无法读取 Skill 文件夹")? {
                let file = file.map_err(|_| "无法读取 Skill 文件")?;
                let metadata =
                    fs::symlink_metadata(file.path()).map_err(|_| "无法读取 Skill 文件")?;
                if metadata.file_type().is_symlink() {
                    return Err("Skill 文件夹不能包含符号链接".into());
                }
                if metadata.is_dir() {
                    walk(root, &file.path(), files, total, depth + 1)?;
                } else if metadata.is_file() {
                    *total += metadata.len();
                    if metadata.len() > 16 * 1024 * 1024
                        || *total > 64 * 1024 * 1024
                        || files.len() >= 512
                    {
                        return Err("Skill 最多 512 个文件、总计 64 MB，单文件不超过 16 MB".into());
                    }
                    let relative = file
                        .path()
                        .strip_prefix(root)
                        .map_err(|_| "Skill 路径无效")?
                        .to_string_lossy()
                        .replace('\\', "/");
                    if !LibraryStore::valid_relative_path(&relative) {
                        return Err("Skill 文件名不兼容 macOS／Windows".into());
                    }
                    files.push((relative, metadata.len()));
                } else {
                    return Err("Skill 只能包含普通文件和目录".into());
                }
            }
            Ok(())
        }
        let mut files = Vec::new();
        walk(path, path, &mut files, &mut 0, 0)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(files)
    }

    pub(crate) fn read_package(path: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut total = 0usize;
        Self::scan_package(path)?
            .into_iter()
            .map(|(name, _)| {
                let bytes = fs::read(path.join(&name)).map_err(|_| "无法读取 Skill 文件")?;
                total += bytes.len();
                if bytes.len() > 16 * 1024 * 1024 || total > 64 * 1024 * 1024 {
                    return Err("Skill 在读取期间变大，请重新导入".into());
                }
                Ok((name, bytes))
            })
            .collect()
    }

    fn decode_package(files: Vec<ImportFile>) -> Result<Vec<(String, Vec<u8>)>, String> {
        if files.is_empty() || files.len() > 512 {
            return Err("请选择包含 SKILL.md 的文件夹，最多 512 个文件".into());
        }
        let mut decoded = Vec::new();
        let mut names = std::collections::HashSet::new();
        let mut total = 0usize;
        for file in files {
            if !Self::valid_relative_path(&file.path) || !names.insert(file.path.to_lowercase()) {
                return Err("Skill 路径无效或文件名重复".into());
            }
            if file.base64.len() > 23_000_000 {
                return Err("Skill 单文件不超过 16 MB".into());
            }
            let bytes = STANDARD
                .decode(file.base64)
                .map_err(|_| "Skill 文件编码无效")?;
            total += bytes.len();
            if bytes.len() > 16 * 1024 * 1024 || total > 64 * 1024 * 1024 {
                return Err("Skill 总计不超过 64 MB，单文件不超过 16 MB".into());
            }
            decoded.push((file.path, bytes));
        }
        if !decoded.iter().any(|(name, _)| name == "SKILL.md") {
            return Err("Skill 文件夹根目录需要 SKILL.md".into());
        }
        Ok(decoded)
    }

    pub fn skill_files(&self, id: &str) -> Result<Vec<String>, String> {
        let _guard = self.lock()?;
        let path = self.skill_path(id)?;
        if !path.is_dir() {
            return Ok(vec![]);
        }
        Ok(Self::scan_package(&path)?
            .into_iter()
            .map(|(name, _)| name)
            .collect())
    }

    pub fn open_skill_folder(&self, id: &str) -> Result<(), String> {
        let path = self.skill_path(id)?;
        if !path.is_dir() {
            return Err("Skill 文件夹不存在".into());
        }
        #[cfg(target_os = "macos")]
        let status = std::process::Command::new("/usr/bin/open")
            .arg(&path)
            .status();
        #[cfg(target_os = "windows")]
        let status = std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .map(|_| ());
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let status = std::process::Command::new("xdg-open").arg(&path).status();
        #[cfg(not(target_os = "windows"))]
        if !status.map_err(|_| "无法打开 Skill 文件夹")?.success() {
            return Err("文件管理器未能打开 Skill 文件夹".into());
        }
        #[cfg(target_os = "windows")]
        status.map_err(|_| "无法打开 Skill 文件夹")?;
        Ok(())
    }

    pub fn save(
        &self,
        mut entry: Entry,
        expected_revision: u64,
        files: Option<Vec<ImportFile>>,
    ) -> Result<Entry, String> {
        let _guard = self.lock()?;
        self.validate(&entry)?;
        let registered_tags = self.read_tags_unlocked()?;
        let mut normalized_tags = Vec::new();
        for raw in &entry.tags {
            let norm = normalize_tag(raw)?;
            if !registered_tags.contains(&norm) {
                return Err(format!(
                    "标签「{norm}」尚未登记；请先调用 tag_add 创建该标签后再使用，确保 Tag 逐一受控维护"
                ));
            }
            if !normalized_tags.contains(&norm) {
                normalized_tags.push(norm);
            }
        }
        entry.tags = normalized_tags;

        if entry.id.is_empty() {
            entry.id = Uuid::new_v4().to_string();
        }
        let path = self.entry_path(&entry.id)?;
        let actual_revision = if path.exists() {
            let old: Entry = serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取样板")?)
                .map_err(|_| "样板记录损坏")?;
            if old.kind != entry.kind {
                return Err("已有资产基础类型（prompt/skill）不能改变".into());
            }
            old.revision
        } else {
            0
        };
        if actual_revision != expected_revision {
            return Err("样板已在其他位置修改，请先读取最新 revision 再保存".into());
        }
        if entry.kind == Kind::Prompt && (entry.prompt.trim().is_empty() || files.is_some()) {
            return Err("提示词须填写 prompt 正文；文件夹属于 Skill".into());
        }
        if entry.kind == Kind::Skill && entry.skill_markdown.trim().is_empty() {
            return Err("Skill 须包含非空 SKILL.md".into());
        }
        let mut package = None;
        if entry.kind == Kind::Skill {
            let current = self.skill_document(&entry.id)?;
            if current.is_some()
                && entry.skill_digest.is_some()
                && current
                    .as_ref()
                    .map(|text| format!("{:x}", Sha256::digest(text.as_bytes())))
                    != entry.skill_digest
            {
                return Err("SKILL.md 已在磁盘被外部修改，请重新读取后再保存".into());
            }
            let directory = self.skill_path(&entry.id)?;
            let mut contents = if let Some(files) = files {
                Self::decode_package(files)?
            } else if directory.is_dir() {
                Self::read_package(&directory)?
            } else {
                vec![]
            };
            contents.retain(|(name, _)| name != "SKILL.md");
            contents.push(("SKILL.md".into(), entry.skill_markdown.as_bytes().to_vec()));
            let staged = self
                .root
                .join("skills")
                .join(format!(".stage-{}", Uuid::new_v4()));
            fs::create_dir(&staged).map_err(|_| "无法暂存 Skill")?;
            let write = (|| {
                for (name, bytes) in contents {
                    let file = staged.join(&name);
                    fs::create_dir_all(file.parent().ok_or("Skill 路径无效")?)
                        .map_err(|_| "无法创建 Skill 子目录")?;
                    self.atomic_write(&file, &bytes)?;
                }
                Ok::<_, String>(())
            })();
            if let Err(error) = write {
                let _ = fs::remove_dir_all(&staged);
                return Err(error);
            }
            package = Some((staged, directory));
        }
        entry.schema_version = 5;
        entry.name = entry.name.trim().into();
        entry.revision = actual_revision.checked_add(1).ok_or("修订号无效")?;
        entry.updated_at = Self::now_secs()?;
        let mut record = entry.clone();
        record.skill_markdown.clear();
        record.skill_digest = None;
        let bytes = serde_json::to_vec_pretty(&record).map_err(|_| "无法整理样板")?;
        if let Some((staged, directory)) = &package {
            let backup = directory
                .exists()
                .then(|| format!(".previous-{}", Uuid::new_v4()));
            let tx = SkillTransaction {
                id: entry.id.clone(),
                staged: staged
                    .file_name()
                    .ok_or("Skill 暂存路径无效")?
                    .to_string_lossy()
                    .into_owned(),
                backup: backup.clone(),
                revision: entry.revision,
            };
            let journal = self
                .root
                .join("skills")
                .join(format!(".transaction-{}.json", entry.id));
            if let Err(error) = self.atomic_write(
                &journal,
                &serde_json::to_vec(&tx).map_err(|_| "无法记录 Skill 保存状态")?,
            ) {
                let _ = fs::remove_dir_all(staged);
                return Err(error);
            }
            let swap = (|| {
                if let Some(backup) = &backup {
                    fs::rename(directory, self.root.join("skills").join(backup))
                        .map_err(|_| "无法替换 Skill 文件夹，请关闭占用它的程序")?;
                }
                fs::rename(staged, directory).map_err(|_| "无法保存 Skill 文件夹")?;
                Ok::<_, String>(())
            })();
            if let Err(error) = swap {
                self.recover_transactions()?;
                return Err(error);
            }
        }
        if let Err(error) = self.atomic_write(&path, &bytes) {
            if package.is_some() {
                self.recover_transactions()?;
            }
            return Err(error);
        }
        if package.is_some() {
            let _ = self.recover_transactions();
        }
        self.hydrate_skill(&mut entry)?;
        Ok(entry)
    }

    pub fn import_image_bytes(&self, bytes: &[u8]) -> Result<String, String> {
        if bytes.is_empty() || bytes.len() > 12 * 1024 * 1024 {
            return Err("图片为空或超过 12 MB".into());
        }
        let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "jpg"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "webp"
        } else {
            return Err("支持 PNG、JPEG 与 WebP 图片".into());
        };
        let name = format!("{:x}.{extension}", Sha256::digest(bytes));
        let _guard = self.lock()?;
        let path = self.root.join("images").join(&name);
        if !path.exists() {
            self.atomic_write(&path, bytes)?;
        }
        Ok(name)
    }

    pub fn import_image_file(&self, path: &Path) -> Result<String, String> {
        let meta = fs::metadata(path).map_err(|_| "无法读取图片文件")?;
        if !meta.is_file() || meta.len() > 12 * 1024 * 1024 {
            return Err("图片必须是 12 MB 以内的普通文件".into());
        }
        let bytes = fs::read(path).map_err(|_| "无法读取图片字节")?;
        self.import_image_bytes(&bytes)
    }
}

#[tauri::command]
pub async fn library_list(store: tauri::State<'_, LibraryStore>) -> Result<Library, String> {
    store.list()
}
#[tauri::command]
pub async fn library_state_token(store: tauri::State<'_, LibraryStore>) -> Result<String, String> {
    store.state_token()
}
#[tauri::command]
pub async fn library_skill_files(
    store: tauri::State<'_, LibraryStore>,
    id: String,
) -> Result<Vec<String>, String> {
    store.skill_files(&id)
}
#[tauri::command]
pub async fn library_open_skill_folder(
    store: tauri::State<'_, LibraryStore>,
    id: String,
) -> Result<(), String> {
    store.open_skill_folder(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> Entry {
        Entry {
            schema_version: 5,
            id: String::new(),
            kind: Kind::Prompt,
            modality: Modality::Image,
            name: "安静角色设定".into(),
            prompt: "蛋黄黄乱发、黑卫衣、粗蜡笔纸纹".into(),
            sample: "安静站在白板旁微笑".into(),
            tags: vec![],
            notes: String::new(),
            origin_created_at: None,
            origin_updated_at: None,
            skill_markdown: String::new(),
            skill_digest: None,
            images: vec![],
            revision: 0,
            updated_at: 0,
            source: None,
        }
    }

    #[test]
    fn controlled_tags_enforce_registration_and_cascade_rename_and_delete() {
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().join("library")).unwrap();

        // Saving with an unregistered tag must fail
        let mut unregistered = entry();
        unregistered.tags = vec!["安静".into()];
        assert!(store.save(unregistered, 0, None).is_err());

        // Register tags explicitly one by one
        store.tag_add("#安静").unwrap();
        store.tag_add("计算机").unwrap();
        assert_eq!(store.tag_list().unwrap(), vec!["安静", "计算机"]);

        let mut item = entry();
        item.tags = vec!["#安静".into(), "计算机".into()];
        let saved = store.save(item, 0, None).unwrap();
        assert_eq!(saved.tags, vec!["安静", "计算机"]);
        let token_before = store.state_token().unwrap();

        // Renaming a tag updates controlled list and cascades to entries
        store.tag_rename("计算机", "技术").unwrap();
        assert_eq!(store.tag_list().unwrap(), vec!["安静", "技术"]);
        let after_rename = &store.list().unwrap().entries[0];
        assert_eq!(after_rename.tags, vec!["安静", "技术"]);
        assert_ne!(store.state_token().unwrap(), token_before);

        // Deleting a tag removes it from controlled list and strips from entries
        store.tag_delete("技术").unwrap();
        let after_delete = &store.list().unwrap().entries[0];
        assert_eq!(after_delete.tags, vec!["安静"]);
    }

    #[test]
    fn edits_survive_reopening_and_stale_writes_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().into()).unwrap();
        let first = store.save(entry(), 0, None).unwrap();
        let mut next = first.clone();
        next.name = "新的名字".into();
        store.save(next, first.revision, None).unwrap();
        assert!(store.save(first, 1, None).is_err());
        let reopened = LibraryStore::new(dir.path().into())
            .unwrap()
            .list()
            .unwrap();
        assert_eq!(reopened.entries[0].name, "新的名字");
        assert_eq!(reopened.entries[0].revision, 2);
    }

    #[test]
    fn skill_import_rejects_escaping_and_conflicting_paths_before_writes() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().join("library")).unwrap();
        for path in [
            "../outside",
            "/absolute",
            "assets/../../outside",
            "CON",
            "C:/outside",
            "a\\b",
        ] {
            let mut skill = entry();
            skill.kind = Kind::Skill;
            skill.prompt.clear();
            skill.skill_markdown = "# Test".into();
            let files = vec![
                ImportFile {
                    path: "SKILL.md".into(),
                    base64: STANDARD.encode(b"# Test"),
                },
                ImportFile {
                    path: path.into(),
                    base64: STANDARD.encode(b"bad"),
                },
            ];
            assert!(store.save(skill, 0, Some(files)).is_err(), "{path}");
            assert!(store.list().unwrap().entries.is_empty());
        }
        assert!(!dir.path().join("outside").exists());
    }

    #[test]
    fn imports_preserve_bytes_and_deduplicate_but_reject_fake_formats() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().into()).unwrap();
        let bytes = include_bytes!("../icons/32x32.png");
        let name = store.import_image_bytes(bytes).unwrap();
        assert_eq!(name, store.import_image_bytes(bytes).unwrap());
        assert_eq!(
            fs::read(dir.path().join("images").join(&name)).unwrap(),
            bytes
        );
        assert!(store.import_image_bytes(b"bad").is_err());
    }
}
