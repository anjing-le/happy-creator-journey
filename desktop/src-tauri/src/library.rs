use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

fn current_schema() -> u32 {
    2
}
fn default_category() -> String {
    String::new()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[serde(alias = "ip")]
    Prompt,
    #[serde(alias = "scene")]
    Skill,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    #[serde(default = "current_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    pub kind: Kind,
    pub name: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default = "default_category")]
    pub category: String,
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
    pub categories: Vec<String>,
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

pub struct LibraryStore {
    root: PathBuf,
    lock: Mutex<()>,
}
impl LibraryStore {
    fn lock(&self) -> Result<(std::sync::MutexGuard<'_, ()>, fs::File), String> {
        let guard = self.lock.lock().map_err(|_| "素材库正忙，请重试")?;
        let mut options = fs::OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(self.root.join(".lock"))
            .map_err(|_| "无法锁定素材库")?;
        file.lock().map_err(|_| "无法锁定素材库")?;
        Ok((guard, file))
    }

    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("entries")).map_err(|_| "无法创建素材目录")?;
        fs::create_dir_all(root.join("images")).map_err(|_| "无法创建图片目录")?;
        fs::create_dir_all(root.join("skills")).map_err(|_| "无法创建 Skills 目录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护素材目录")?;
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

    fn entry_path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err("素材标识无效".into());
        }
        Ok(self.root.join("entries").join(format!("{id}.json")))
    }

    fn validate(&self, entry: &Entry) -> Result<(), String> {
        if ![1, 2].contains(&entry.schema_version) {
            return Err("素材版本暂不支持".into());
        }
        if entry.name.trim().is_empty() || entry.name.chars().count() > 80 {
            return Err("请填写不超过 80 字的名称".into());
        }
        if entry.prompt.len() > 64_000 || entry.skill_markdown.len() > 256_000 {
            return Err("规则文档过长".into());
        }
        if entry.category.chars().count() > 80
            || entry.tags.len() > 20
            || entry.tags.iter().any(|tag| tag.chars().count() > 40)
            || entry.notes.len() > 16_000
        {
            return Err("分类、标签或备注过长".into());
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
        if entry.images.len() > 16 {
            return Err("每份素材最多保存 16 张样图".into());
        }
        if entry
            .source
            .as_ref()
            .is_some_and(|source| source.len() > 4096)
        {
            return Err("素材来源说明过长".into());
        }
        if entry.kind == Kind::Prompt && !entry.skill_markdown.is_empty()
            || entry.kind == Kind::Skill && !entry.prompt.is_empty()
        {
            return Err("提示词与 Skill 请分别保存".into());
        }
        for name in &entry.images {
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
                return Err("参考图片不存在".into());
            }
        }
        Ok(())
    }

    fn category_names(&self) -> Result<Vec<String>, String> {
        let path = self.root.join("categories.json");
        if !path.exists() {
            return Ok(vec![]);
        }
        let categories: Vec<String> =
            serde_json::from_slice(&fs::read(path).map_err(|_| "无法读取分类")?)
                .map_err(|_| "分类文件损坏")?;
        if categories.len() > 500
            || categories
                .iter()
                .any(|s| s.trim().is_empty() || s.chars().count() > 80)
        {
            return Err("分类文件格式无效".into());
        }
        Ok(categories)
    }
    pub fn add_category(&self, name: &str) -> Result<Vec<String>, String> {
        let _guard = self.lock()?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err("分类名需要 1–80 字".into());
        }
        let mut categories = self.category_names()?;
        if !categories.iter().any(|category| category == name) {
            if categories.len() >= 500 {
                return Err("分类数量过多".into());
            }
            categories.push(name.into());
            self.atomic_write(
                &self.root.join("categories.json"),
                &serde_json::to_vec_pretty(&categories).map_err(|_| "无法整理分类")?,
            )?;
        }
        Ok(categories)
    }

    pub fn move_category(&self, id: &str, revision: u64, category: &str) -> Result<Entry, String> {
        let _guard = self.lock()?;
        let path = self.entry_path(id)?;
        let mut entry: Entry = serde_json::from_slice(&fs::read(&path).map_err(|_| "素材不存在")?)
            .map_err(|_| "素材记录损坏")?;
        if entry.revision != revision {
            return Err("素材已被修改，请刷新后再移动".into());
        }
        let category = category.trim();
        if !category.is_empty() && !self.category_names()?.iter().any(|c| c == category) {
            return Err("分类不存在，请先新增分类".into());
        }
        entry.category = category.into();
        entry.revision = entry.revision.checked_add(1).ok_or("修订号无效")?;
        self.atomic_write(
            &path,
            &serde_json::to_vec_pretty(&entry).map_err(|_| "无法整理素材")?,
        )?;
        self.hydrate_skill(&mut entry)?;
        Ok(entry)
    }

    pub fn clear_categories(&self) -> Result<usize, String> {
        let _guard = self.lock()?;
        let mut count = 0;
        for file in fs::read_dir(self.root.join("entries")).map_err(|_| "无法读取素材")? {
            let path = file.map_err(|_| "无法读取素材")?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取素材")?)
                    .map_err(|_| "素材记录损坏")?;
            if !entry.category.is_empty() {
                entry.category.clear();
                entry.revision = entry.revision.checked_add(1).ok_or("修订号无效")?;
                self.atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&entry).map_err(|_| "无法整理素材")?,
                )?;
                count += 1;
            }
        }
        self.atomic_write(&self.root.join("categories.json"), b"[]")?;
        Ok(count)
    }

    pub fn snapshot_assets(
        &self,
        skill_id: &str,
        prompt_ids: &[String],
        destination: &std::path::Path,
    ) -> Result<(Option<Entry>, Vec<Entry>), String> {
        let _guard = self.lock()?;
        let read = |id: &str| -> Result<Entry, String> {
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(self.entry_path(id)?).map_err(|_| "素材不存在")?)
                    .map_err(|_| "素材记录损坏")?;
            self.hydrate_skill(&mut entry)?;
            self.validate(&entry)?;
            Ok(entry)
        };
        let skill = if skill_id.is_empty() {
            None
        } else {
            let entry = read(skill_id)?;
            if entry.kind != Kind::Skill {
                return Err("请选择真实 Skill 文件夹".into());
            }
            let folder = destination.join("skill");
            fs::create_dir_all(&folder).map_err(|_| "无法创建 Skill 快照")?;
            for (name, bytes) in Self::read_package(&self.root.join("skills").join(skill_id))? {
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
                return Err("附加素材应为提示词".into());
            }
            prompts.push(entry);
        }
        fs::create_dir_all(destination.join("references")).map_err(|_| "无法创建参考图目录")?;
        for entry in skill.iter().chain(prompts.iter()) {
            for name in &entry.images {
                self.atomic_write(
                    &destination.join("references").join(name),
                    &fs::read(self.root.join("images").join(name)).map_err(|_| "无法读取参考图")?,
                )?;
            }
        }
        Ok((skill, prompts))
    }

    pub fn list(&self) -> Result<Library, String> {
        let _guard = self.lock()?;
        let mut entries = Vec::new();
        for file in fs::read_dir(self.root.join("entries")).map_err(|_| "无法读取素材库")? {
            let file = file.map_err(|_| "无法读取素材")?;
            if file.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if !file.file_type().map_err(|_| "无法读取素材")?.is_file() {
                return Err("素材文件类型无效".into());
            }
            let mut entry: Entry =
                serde_json::from_slice(&fs::read(file.path()).map_err(|_| "无法读取素材")?)
                    .map_err(|_| "素材记录损坏，请保留文件后检查")?;
            self.hydrate_skill(&mut entry)?;
            self.validate(&entry)?;
            if self.entry_path(&entry.id)? != file.path() {
                return Err("素材文件与标识不一致".into());
            }
            entries.push(entry);
        }
        entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.name.cmp(&b.name)));
        let mut categories = self.category_names()?;
        for entry in &entries {
            if !entry.category.is_empty() && !categories.contains(&entry.category) {
                categories.push(entry.category.clone());
            }
        }
        Ok(Library {
            categories,
            entries,
            image_directory: self.root.join("images").to_string_lossy().into_owned(),
        })
    }

    fn atomic_write(&self, destination: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
        let temporary = destination.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary).map_err(|_| "无法保存素材")?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "无法保存素材")?;
            fs::rename(&temporary, destination).map_err(|_| "无法替换素材记录")
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
                    serde_json::from_slice(&fs::read(&metadata).map_err(|_| "无法读取素材")?)
                        .map_err(|_| "素材记录损坏，保留 Skill 备份后检查")?;
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
                // If the backup was never created, the original directory was not moved.
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

    fn scan_package(path: &std::path::Path) -> Result<Vec<(String, u64)>, String> {
        fn walk(
            root: &std::path::Path,
            path: &std::path::Path,
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

    pub(crate) fn read_package(path: &std::path::Path) -> Result<Vec<(String, Vec<u8>)>, String> {
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
            return Err("请先保存 Skill 文件夹".into());
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
        if entry.id.is_empty() {
            entry.id = Uuid::new_v4().to_string();
        }
        let path = self.entry_path(&entry.id)?;
        let actual_revision = if path.exists() {
            let old: Entry = serde_json::from_slice(&fs::read(&path).map_err(|_| "无法读取素材")?)
                .map_err(|_| "素材记录损坏")?;
            if old.kind != entry.kind {
                return Err("已有素材类型不能改变".into());
            }
            old.revision
        } else {
            0
        };
        if actual_revision != expected_revision {
            return Err("素材已在其他位置修改，请返回素材库重新打开".into());
        }
        if entry.kind == Kind::Prompt && (entry.prompt.trim().is_empty() || files.is_some()) {
            return Err("请填写提示词；文件夹属于 Skill".into());
        }
        if entry.kind == Kind::Skill && entry.skill_markdown.trim().is_empty() {
            return Err("请填写或导入 SKILL.md".into());
        }
        let mut package = None;
        if entry.kind == Kind::Skill {
            let current = self.skill_document(&entry.id)?;
            if current
                .as_ref()
                .map(|text| format!("{:x}", Sha256::digest(text.as_bytes())))
                != entry.skill_digest
            {
                return Err("SKILL.md 已在文件夹中修改，请返回素材库重新打开".into());
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
        entry.schema_version = 2;
        entry.name = entry.name.trim().into();
        entry.category = entry.category.trim().into();
        if !entry.category.is_empty() {
            let mut categories = self.category_names()?;
            if !categories.contains(&entry.category) {
                if categories.len() >= 500 {
                    return Err("分类数量过多".into());
                }
                categories.push(entry.category.clone());
                self.atomic_write(
                    &self.root.join("categories.json"),
                    &serde_json::to_vec_pretty(&categories).map_err(|_| "无法整理分类")?,
                )?;
            }
        }
        entry.tags = entry
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect();
        entry.tags.sort();
        entry.tags.dedup();
        entry.revision = actual_revision.checked_add(1).ok_or("素材修订号无效")?;
        entry.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效")?
            .as_secs();
        let mut record = entry.clone();
        record.skill_markdown.clear();
        record.skill_digest = None;
        let bytes = serde_json::to_vec_pretty(&record).map_err(|_| "无法整理素材")?;
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
        // Metadata is the commit point. An interrupted cleanup is retried on next open.
        if package.is_some() {
            let _ = self.recover_transactions();
        }
        self.hydrate_skill(&mut entry)?;
        Ok(entry)
    }

    pub fn import_image(&self, data_url: &str) -> Result<String, String> {
        if data_url.len() > 17_000_000 {
            return Err("单张图片请控制在 12 MB 以内".into());
        }
        let (header, encoded) = data_url.split_once(',').ok_or("图片格式无效")?;
        let extension = match header {
            "data:image/png;base64" => "png",
            "data:image/jpeg;base64" => "jpg",
            "data:image/webp;base64" => "webp",
            _ => return Err("支持 PNG、JPEG 与 WebP 图片".into()),
        };
        let bytes = STANDARD.decode(encoded).map_err(|_| "图片数据无效")?;
        if bytes.is_empty() || bytes.len() > 12 * 1024 * 1024 {
            return Err("图片为空或超过 12 MB".into());
        }
        let matches = match extension {
            "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "jpg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
            "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            _ => false,
        };
        if !matches {
            return Err("图片内容与格式不一致".into());
        }
        let name = format!("{:x}.{extension}", Sha256::digest(&bytes));
        let _guard = self.lock()?;
        let path = self.root.join("images").join(&name);
        if !path.exists() {
            self.atomic_write(&path, &bytes)?;
        }
        Ok(name)
    }
}

#[tauri::command]
pub async fn library_list(store: tauri::State<'_, LibraryStore>) -> Result<Library, String> {
    store.list()
}
#[tauri::command]
pub async fn library_save(
    store: tauri::State<'_, LibraryStore>,
    entry: Entry,
    expected_revision: u64,
    files: Option<Vec<ImportFile>>,
) -> Result<Entry, String> {
    store.save(entry, expected_revision, files)
}
#[tauri::command]
pub async fn library_import_image(
    store: tauri::State<'_, LibraryStore>,
    data_url: String,
) -> Result<String, String> {
    store.import_image(&data_url)
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
            schema_version: 1,
            id: String::new(),
            kind: Kind::Prompt,
            name: "安静".into(),
            prompt: "yellow hair, black hoodie".into(),
            category: default_category(),
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
    fn material_snapshot_allows_no_skill_and_preserves_selected_prompt() {
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().join("library")).unwrap();
        let saved = store.save(entry(), 0, None).unwrap();
        let target = root.path().join("snapshot");
        let (skill, prompts) = store
            .snapshot_assets("", std::slice::from_ref(&saved.id), &target)
            .unwrap();
        assert!(skill.is_none());
        assert!(!target.join("skill").exists());
        assert_eq!(prompts[0].prompt, saved.prompt);
        let (skill, prompts) = store
            .snapshot_assets("", &[], &root.path().join("empty"))
            .unwrap();
        assert!(skill.is_none());
        assert!(prompts.is_empty());
    }

    #[test]
    fn categories_clear_and_move_preserve_content_and_reject_stale_drag() {
        let root = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(root.path().into()).unwrap();
        let mut original = entry();
        original.category = "Old".into();
        let first = store.save(original, 0, None).unwrap();
        store.add_category("Empty").unwrap();
        assert_eq!(store.clear_categories().unwrap(), 1);
        let cleared = store.list().unwrap();
        assert!(cleared.categories.is_empty());
        assert_eq!(cleared.entries.len(), 1);
        assert_eq!(cleared.entries[0].prompt, first.prompt);
        assert_eq!(cleared.entries[0].images, first.images);
        assert!(cleared.entries[0].category.is_empty());
        store.add_category("New").unwrap();
        let current = &cleared.entries[0];
        let moved = store
            .move_category(&current.id, current.revision, "New")
            .unwrap();
        assert_eq!(moved.category, "New");
        assert_eq!(moved.prompt, first.prompt);
        assert!(store
            .move_category(&current.id, current.revision, "")
            .is_err());
        assert!(store
            .move_category(&moved.id, moved.revision, "Unknown")
            .is_err());
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
    fn interrupted_skill_swap_recovers_from_metadata_commit_point() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().into()).unwrap();
        let mut skill = entry();
        skill.kind = Kind::Skill;
        skill.prompt.clear();
        skill.skill_markdown = "# Original".into();
        let saved = store.save(skill, 0, None).unwrap();
        let package = store.skill_path(&saved.id).unwrap();
        let staged = format!(".stage-{}", Uuid::new_v4());
        let backup = format!(".previous-{}", Uuid::new_v4());
        fs::create_dir(dir.path().join("skills").join(&staged)).unwrap();
        fs::write(
            dir.path().join("skills").join(&staged).join("SKILL.md"),
            "# Uncommitted",
        )
        .unwrap();
        fs::rename(&package, dir.path().join("skills").join(&backup)).unwrap();
        fs::rename(dir.path().join("skills").join(&staged), &package).unwrap();
        let tx = SkillTransaction {
            id: saved.id.clone(),
            staged,
            backup: Some(backup),
            revision: 2,
        };
        let journal = dir
            .path()
            .join("skills")
            .join(format!(".transaction-{}.json", saved.id));
        fs::write(&journal, serde_json::to_vec(&tx).unwrap()).unwrap();
        let reopened = LibraryStore::new(dir.path().into()).unwrap();
        assert_eq!(
            reopened.list().unwrap().entries[0].skill_markdown,
            "# Original"
        );
        assert!(!journal.exists());
        let backup = format!(".previous-{}", Uuid::new_v4());
        fs::create_dir(dir.path().join("skills").join(&backup)).unwrap();
        let tx = SkillTransaction {
            id: saved.id.clone(),
            staged: format!(".stage-{}", Uuid::new_v4()),
            backup: Some(backup.clone()),
            revision: 1,
        };
        fs::write(&journal, serde_json::to_vec(&tx).unwrap()).unwrap();
        let reopened = LibraryStore::new(dir.path().into()).unwrap();
        assert_eq!(
            reopened.list().unwrap().entries[0].skill_markdown,
            "# Original"
        );
        assert!(!dir.path().join("skills").join(&backup).exists());
    }
    #[test]
    fn imports_preserve_bytes_and_deduplicate_but_reject_fake_formats() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().into()).unwrap();
        let bytes = include_bytes!("../icons/32x32.png");
        let data = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        let name = store.import_image(&data).unwrap();
        assert_eq!(name, store.import_image(&data).unwrap());
        assert_eq!(
            fs::read(dir.path().join("images").join(&name)).unwrap(),
            bytes
        );
        assert!(store.import_image("data:image/png;base64,YmFk").is_err());
        let mut bad = entry();
        bad.images = vec!["../../outside.png".into()];
        assert!(store.save(bad, 0, None).is_err());
        let mut bad = entry();
        bad.id = "../outside".into();
        assert!(store.save(bad, 0, None).is_err());
    }
}

#[tauri::command]
pub async fn library_add_category(
    store: tauri::State<'_, LibraryStore>,
    name: String,
) -> Result<Vec<String>, String> {
    store.add_category(&name)
}
#[tauri::command]
pub async fn library_move_category(
    store: tauri::State<'_, LibraryStore>,
    id: String,
    revision: u64,
    category: String,
) -> Result<Entry, String> {
    store.move_category(&id, revision, &category)
}
