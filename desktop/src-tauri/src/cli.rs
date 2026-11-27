//! Local CLI: invokes the same store as the desktop app without launching a webview.
use crate::library::{Entry, ImportFile, Kind, LibraryStore, Modality};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::PathBuf,
};

fn default_root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CREATORX_LIBRARY_DIR") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is missing")?);
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is missing")?)
        .join("Library/Application Support");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(
            PathBuf::from(std::env::var_os("HOME").ok_or("HOME is missing")?).join(".local/share"),
        );
    Ok(base.join("cc.anjing.creatorx/library"))
}

fn option(args: &[String], key: &str) -> Result<Option<String>, String> {
    if let Some(index) = args.iter().position(|arg| arg == key) {
        return args
            .get(index + 1)
            .filter(|s| !s.starts_with("--"))
            .cloned()
            .map(Some)
            .ok_or(format!("{key} requires a value"));
    }
    Ok(None)
}

fn parse_modality(value: &str) -> Result<Modality, String> {
    match value {
        "text" => Ok(Modality::Text),
        "image" => Ok(Modality::Image),
        "video" => Ok(Modality::Video),
        _ => Err("modality must be text, image, or video".into()),
    }
}

fn schema() -> Value {
    json!({
        "name":"CreatorX CLI",
        "version":env!("CARGO_PKG_VERSION"),
        "commands":[
            {"usage":"schema","effect":"read-only command and data schema"},
            {"usage":"library list [--kind prompt|skill] [--modality text|image|video] [--tag TAG] [--search TEXT]","effect":"read local showcase entries"},
            {"usage":"library tags","effect":"read deduplicated tags across all entries"},
            {"usage":"library get ID","effect":"read complete entry and prompt/SKILL.md"},
            {"usage":"library save (--stdin | --file JSON) [--if-revision N]","effect":"create/update entry; updates require current revision; Skill saves create/update a real folder"},
            {"usage":"library delete ID --if-revision N","effect":"delete entry with revision check"},
            {"usage":"library import-skill --folder PATH [--name NAME] [--modality text|image|video] [--tags TAG1,TAG2]","effect":"copy complete folder locally; scripts are never executed"},
            {"usage":"library skill-files ID","effect":"read relative package file paths"},
            {"usage":"library import-image --file PATH","effect":"copy and deduplicate showcase/reference image bytes locally"}
        ],
        "entry":{
            "kind":"prompt|skill",
            "modality":"text|image|video",
            "name":"required",
            "prompt":"required for prompt",
            "skillMarkdown":"SKILL.md for skill",
            "sample":"sample text output or showcase summary",
            "tags":"string[] (e.g. [\"安静\", \"粗蜡笔\", \"计算机\"])",
            "notes":"string",
            "images":"stored image hashes",
            "id":"empty for create",
            "revision":"read before updating"
        },
        "limits":{"skillFiles":512,"skillTotalMB":64,"skillFileMB":16},
        "storage":"same application data as GUI; CREATORX_LIBRARY_DIR overrides CLI storage for tests",
        "output":"JSON: ok/data or ok/error"
    })
}

pub fn execute(args: &[String], input: &mut dyn Read) -> Result<Value, String> {
    if args.is_empty() || ["schema", "--help", "-h"].contains(&args[0].as_str()) {
        return Ok(schema());
    }
    if args[0] != "library" {
        return Err("Use schema to discover supported commands".into());
    }
    let command = args
        .get(1)
        .map(String::as_str)
        .ok_or("Missing library command")?;
    let allowed: &[&str] = match command {
        "list" => &["--kind", "--modality", "--tag", "--search"],
        "save" => &["--stdin", "--file", "--if-revision"],
        "delete" => &["--if-revision"],
        "import-skill" => &["--folder", "--name", "--modality", "--tags"],
        "tags" | "get" | "skill-files" => &[],
        "import-image" => &["--file"],
        _ => return Err("Unsupported command; use schema".into()),
    };
    let mut index = 2;
    while index < args.len() {
        if args[index].starts_with("--") {
            if !allowed.contains(&args[index].as_str()) {
                return Err(format!("Unsupported option {}", args[index]));
            }
            index += if args[index] == "--stdin" { 1 } else { 2 };
        } else if ["get", "skill-files", "delete"].contains(&command) && index == 2 {
            index += 1;
        } else {
            return Err(format!("Unexpected argument {}", args[index]));
        }
    }
    let store = LibraryStore::new(default_root()?)?;
    match command {
        "tags" => Ok(json!(store.list()?.tags)),
        "list" => {
            let kind = option(args, "--kind")?;
            if kind
                .as_ref()
                .is_some_and(|k| !["prompt", "skill"].contains(&k.as_str()))
            {
                return Err("kind must be prompt or skill".into());
            }
            let modality = option(args, "--modality")?
                .as_deref()
                .map(parse_modality)
                .transpose()?;
            let tag = option(args, "--tag")?
                .map(|t| t.trim().trim_start_matches('#').to_string());
            let search = option(args, "--search")?.unwrap_or_default().to_lowercase();
            let entries: Vec<_> = store
                .list()?
                .entries
                .into_iter()
                .filter(|e| {
                    let matches_kind = kind
                        .as_ref()
                        .is_none_or(|k| (k == "prompt") == (e.kind == Kind::Prompt));
                    matches_kind
                        && modality.is_none_or(|m| e.modality == m)
                        && tag.as_ref().is_none_or(|t| e.tags.iter().any(|et| et == t))
                        && format!(
                            "{} {} {} {} {} {}",
                            e.name,
                            e.prompt,
                            e.skill_markdown,
                            e.sample,
                            e.notes,
                            e.tags.join(" ")
                        )
                        .to_lowercase()
                        .contains(&search)
                })
                .collect();
            Ok(json!({"entries":entries,"count":entries.len()}))
        }
        "get" => {
            let id = args.get(2).ok_or("Missing entry ID")?;
            serde_json::to_value(
                store
                    .list()?
                    .entries
                    .into_iter()
                    .find(|entry| &entry.id == id)
                    .ok_or("Entry not found")?,
            )
            .map_err(|e| e.to_string())
        }
        "save" => {
            let file = option(args, "--file")?;
            let stdin = args.iter().any(|arg| arg == "--stdin");
            if stdin == file.is_some() {
                return Err("Choose exactly one of --stdin or --file".into());
            }
            let bytes = if let Some(path) = file {
                std::fs::read(path).map_err(|_| "Cannot read input file")?
            } else {
                let mut bytes = Vec::new();
                input
                    .take(2_000_001)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "Cannot read stdin")?;
                bytes
            };
            if bytes.len() > 2_000_000 {
                return Err("Input JSON is too large".into());
            }
            let entry: Entry =
                serde_json::from_slice(&bytes).map_err(|e| format!("Invalid entry JSON: {e}"))?;
            let expected = option(args, "--if-revision")?;
            if !entry.id.is_empty() && expected.is_none() {
                return Err("Updates require --if-revision".into());
            }
            let revision = expected
                .unwrap_or_else(|| "0".into())
                .parse()
                .map_err(|_| "Invalid revision")?;
            serde_json::to_value(store.save(entry, revision, None)?).map_err(|e| e.to_string())
        }
        "delete" => {
            let id = args.get(2).ok_or("Missing entry ID")?;
            let revision: u64 = option(args, "--if-revision")?
                .ok_or("Deletes require --if-revision")?
                .parse()
                .map_err(|_| "Invalid revision")?;
            store.delete_entry(id, revision)?;
            Ok(json!({"deleted": id}))
        }
        "import-skill" => {
            let path = PathBuf::from(option(args, "--folder")?.ok_or("Missing --folder")?);
            if std::fs::symlink_metadata(&path)
                .map_err(|_| "Cannot read folder")?
                .file_type()
                .is_symlink()
            {
                return Err("Folder cannot be a symbolic link".into());
            }
            let files = LibraryStore::read_package(&path)?;
            let doc = files
                .iter()
                .find(|(name, _)| name == "SKILL.md")
                .ok_or("Folder root must contain SKILL.md")?;
            if doc.1.len() > 256_000 {
                return Err("SKILL.md is too large".into());
            }
            let markdown =
                String::from_utf8(doc.1.clone()).map_err(|_| "SKILL.md must be UTF-8")?;
            let name = option(args, "--name")?.unwrap_or_else(|| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            let modality = option(args, "--modality")?
                .as_deref()
                .map(parse_modality)
                .transpose()?
                .unwrap_or(Modality::Image);
            let tags: Vec<String> = option(args, "--tags")?
                .map(|raw| raw.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let entry: Entry = serde_json::from_value(json!({
                "kind":"skill",
                "modality":modality,
                "name":name,
                "skillMarkdown":markdown,
                "tags":tags
            }))
            .map_err(|e| e.to_string())?;
            let files = files
                .into_iter()
                .map(|(path, bytes)| ImportFile {
                    path,
                    base64: STANDARD.encode(bytes),
                })
                .collect();
            serde_json::to_value(store.save(entry, 0, Some(files))?).map_err(|e| e.to_string())
        }
        "import-image" => {
            let path = PathBuf::from(option(args, "--file")?.ok_or("Missing --file")?);
            if std::fs::metadata(&path)
                .map_err(|_| "Cannot read image")?
                .len()
                > 12 * 1024 * 1024
            {
                return Err("Image exceeds 12 MB".into());
            }
            let bytes = std::fs::read(path).map_err(|_| "Cannot read image")?;
            Ok(json!({"name":store.import_image_bytes(&bytes)?}))
        }
        "skill-files" => Ok(json!(
            store.skill_files(args.get(2).ok_or("Missing entry ID")?)?
        )),
        _ => Err("Unsupported command".into()),
    }
}

pub fn run_if_requested() -> Option<i32> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return None;
    }
    #[cfg(target_os = "windows")]
    unsafe {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetStdHandle(kind: u32) -> *mut std::ffi::c_void;
            fn AttachConsole(process: u32) -> i32;
        }
        let handle = GetStdHandle((-11i32) as u32);
        if handle.is_null() || handle as isize == -1 {
            AttachConsole(u32::MAX);
        }
    }
    match execute(&args, &mut std::io::stdin()) {
        Ok(data) => {
            let _ = writeln!(std::io::stdout(), "{}", json!({"ok":true,"data":data}));
            Some(0)
        }
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "{}", json!({"ok":false,"error":error}));
            Some(1)
        }
    }
}
