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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Ip,
    Scene,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub schema_version: u32,
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub prompt: String,
    pub skill_markdown: String,
    pub images: Vec<String>,
    pub revision: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub entries: Vec<Entry>,
    pub image_directory: String,
}

pub struct LibraryStore {
    root: PathBuf,
    lock: Mutex<()>,
}
impl LibraryStore {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("entries")).map_err(|_| "无法创建素材目录")?;
        fs::create_dir_all(root.join("images")).map_err(|_| "无法创建图片目录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护素材目录")?;
        }
        Ok(Self {
            root,
            lock: Mutex::new(()),
        })
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
        if entry.schema_version != 1 {
            return Err("素材版本暂不支持".into());
        }
        if entry.name.trim().is_empty() || entry.name.chars().count() > 80 {
            return Err("请填写不超过 80 字的名称".into());
        }
        if entry.prompt.len() > 64_000 || entry.skill_markdown.len() > 256_000 {
            return Err("规则文档过长".into());
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
        if entry.kind == Kind::Ip && !entry.skill_markdown.is_empty()
            || entry.kind == Kind::Scene && !entry.prompt.is_empty()
        {
            return Err("角色提示词与场景 Skill 请分别保存".into());
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

    pub fn list(&self) -> Result<Library, String> {
        let _guard = self.lock.lock().map_err(|_| "素材库正忙，请重试")?;
        let mut entries = Vec::new();
        for file in fs::read_dir(self.root.join("entries")).map_err(|_| "无法读取素材库")? {
            let file = file.map_err(|_| "无法读取素材")?;
            if file.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if !file.file_type().map_err(|_| "无法读取素材")?.is_file() {
                return Err("素材文件类型无效".into());
            }
            let entry: Entry =
                serde_json::from_slice(&fs::read(file.path()).map_err(|_| "无法读取素材")?)
                    .map_err(|_| "素材记录损坏，请保留文件后检查")?;
            self.validate(&entry)?;
            if self.entry_path(&entry.id)? != file.path() {
                return Err("素材文件与标识不一致".into());
            }
            entries.push(entry);
        }
        entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.name.cmp(&b.name)));
        Ok(Library {
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

    pub fn save(&self, mut entry: Entry, expected_revision: u64) -> Result<Entry, String> {
        let _guard = self.lock.lock().map_err(|_| "素材库正忙，请重试")?;
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
        entry.name = entry.name.trim().into();
        entry.revision = actual_revision.checked_add(1).ok_or("素材修订号无效")?;
        entry.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效")?
            .as_secs();
        self.atomic_write(
            &path,
            &serde_json::to_vec_pretty(&entry).map_err(|_| "无法整理素材")?,
        )?;
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
        let _guard = self.lock.lock().map_err(|_| "素材库正忙，请重试")?;
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
) -> Result<Entry, String> {
    store.save(entry, expected_revision)
}
#[tauri::command]
pub async fn library_import_image(
    store: tauri::State<'_, LibraryStore>,
    data_url: String,
) -> Result<String, String> {
    store.import_image(&data_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry() -> Entry {
        Entry {
            schema_version: 1,
            id: String::new(),
            kind: Kind::Ip,
            name: "安静".into(),
            prompt: "yellow hair, black hoodie".into(),
            skill_markdown: String::new(),
            images: vec![],
            revision: 0,
            updated_at: 0,
            source: None,
        }
    }
    #[test]
    fn edits_survive_reopening_and_stale_writes_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().into()).unwrap();
        let first = store.save(entry(), 0).unwrap();
        let mut next = first.clone();
        next.name = "新的名字".into();
        store.save(next, first.revision).unwrap();
        assert!(store.save(first, 1).is_err());
        let reopened = LibraryStore::new(dir.path().into())
            .unwrap()
            .list()
            .unwrap();
        assert_eq!(reopened.entries[0].name, "新的名字");
        assert_eq!(reopened.entries[0].revision, 2);
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
        assert!(store.save(bad, 0).is_err());
        let mut bad = entry();
        bad.id = "../outside".into();
        assert!(store.save(bad, 0).is_err());
    }
}
