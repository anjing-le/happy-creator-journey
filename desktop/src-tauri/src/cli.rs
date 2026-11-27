//! Local CLI & MCP entrypoint sharing the exact same locked local store as the desktop viewer.
use crate::{
    library::LibraryStore,
    mcp::{call_tool, default_library_root, run_stdio},
};
use serde_json::{json, Value};
use std::io::{Read, Write};

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

fn schema() -> Value {
    json!({
        "name": "CreatorX Agent Interface (MCP + CLI)",
        "version": env!("CARGO_PKG_VERSION"),
        "mcp": {
            "command": "creatorx mcp",
            "transport": "stdio (JSON-RPC 2.0)"
        },
        "commands": [
            {"usage": "mcp", "effect": "start Model Context Protocol stdio server for Antigravity / Codex / Claude Code"},
            {"usage": "schema", "effect": "read-only command and data schema"},
            {"usage": "tag list", "effect": "read controlled tag registry"},
            {"usage": "tag add --name NAME", "effect": "register a controlled tag"},
            {"usage": "tag rename --from OLD --to NEW", "effect": "rename a controlled tag across all entries"},
            {"usage": "tag delete --name NAME", "effect": "delete a controlled tag and strip from entries"},
            {"usage": "library list [--kind prompt|skill] [--modality text|image|video] [--tag TAG] [--search TEXT]", "effect": "read local showcase entries"},
            {"usage": "library get ID", "effect": "read complete entry, skill files, and current revision"},
            {"usage": "library save (--stdin | --file JSON) [--if-revision N]", "effect": "create/update entry (tags must be registered in tag list)"},
            {"usage": "library delete ID --if-revision N", "effect": "delete entry with revision check"},
            {"usage": "library import-skill --folder PATH --modality text|image|video [--name NAME] [--tags TAG1,TAG2]", "effect": "import complete Skill folder locally without executing scripts"},
            {"usage": "library import-image --file PATH", "effect": "copy and deduplicate showcase/reference image bytes locally"},
            {"usage": "library skill-files ID", "effect": "read relative package file paths"}
        ],
        "types": {
            "kind": "prompt | skill (only 2 base types)",
            "modality": "text | image | video (only 3 directions)",
            "tags": "controlled string[] maintained via tag add/rename/delete"
        }
    })
}

pub fn execute(args: &[String], input: &mut dyn Read) -> Result<Value, String> {
    if args.is_empty() || ["schema", "--help", "-h"].contains(&args[0].as_str()) {
        return Ok(schema());
    }
    let store = LibraryStore::new(default_library_root()?)?;

    if args[0] == "tag" {
        let sub = args.get(1).map(String::as_str).ok_or("Missing tag command")?;
        return match sub {
            "list" => call_tool(&store, "creatorx_tag_list", &json!({})),
            "add" => {
                let name = option(args, "--name")?.ok_or("Missing --name")?;
                call_tool(&store, "creatorx_tag_add", &json!({ "name": name }))
            }
            "rename" => {
                let from = option(args, "--from")?.ok_or("Missing --from")?;
                let to = option(args, "--to")?.ok_or("Missing --to")?;
                call_tool(&store, "creatorx_tag_rename", &json!({ "from": from, "to": to }))
            }
            "delete" => {
                let name = option(args, "--name")?.ok_or("Missing --name")?;
                call_tool(&store, "creatorx_tag_delete", &json!({ "name": name }))
            }
            _ => Err("Unsupported tag command; use schema".into()),
        };
    }

    if args[0] != "library" {
        return Err("Use schema or mcp".into());
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

    match command {
        "tags" => Ok(json!(store.tag_list()?)),
        "list" => call_tool(
            &store,
            "creatorx_asset_list",
            &json!({
                "kind": option(args, "--kind")?,
                "modality": option(args, "--modality")?,
                "tag": option(args, "--tag")?,
                "search": option(args, "--search")?,
            }),
        ),
        "get" => {
            let id = args.get(2).ok_or("Missing entry ID")?;
            let res = call_tool(&store, "creatorx_asset_get", &json!({ "id": id }))?;
            Ok(res["entry"].clone())
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
            let entry: crate::library::Entry =
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
            call_tool(
                &store,
                "creatorx_asset_delete",
                &json!({ "id": id, "expectedRevision": revision }),
            )
        }
        "import-skill" => {
            let folder = option(args, "--folder")?.ok_or("Missing --folder")?;
            let modality = option(args, "--modality")?.unwrap_or_else(|| "image".into());
            let tags: Vec<String> = option(args, "--tags")?
                .map(|raw| {
                    raw.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            call_tool(
                &store,
                "creatorx_skill_import",
                &json!({
                    "folderPath": folder,
                    "modality": modality,
                    "name": option(args, "--name")?,
                    "tags": tags
                }),
            )
        }
        "import-image" => {
            let file = option(args, "--file")?.ok_or("Missing --file")?;
            let res = call_tool(&store, "creatorx_image_import", &json!({ "filePath": file }))?;
            Ok(json!({ "name": res["image"] }))
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
    if args[0] == "mcp" {
        return Some(run_stdio());
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
