//! Native Model Context Protocol (MCP) stdio server built directly into the CreatorX binary.
//! Allows Antigravity, Codex, Claude Code, or any MCP-compatible Agent to maintain
//! CreatorX's controlled Tags, Prompts, Skills, and Showcase images without any GUI CRUD forms.
use crate::library::{Entry, ImportFile, Kind, LibraryStore, Modality};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

pub fn default_library_root() -> Result<PathBuf, String> {
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

fn parse_modality(value: &str) -> Result<Modality, String> {
    match value {
        "text" => Ok(Modality::Text),
        "image" => Ok(Modality::Image),
        "video" => Ok(Modality::Video),
        _ => Err("modality 只能是 text（文）、image（图）或 video（视频）".into()),
    }
}

fn tools_schema() -> Value {
    json!({
        "tools": [
            {
                "name": "creatorx_tag_list",
                "description": "读取 CreatorX 样板间当前受控维护的全部 Tag 列表。",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "creatorx_tag_add",
                "description": "新增一个受控 Tag（如 安静、粗蜡笔、计算机、金融）。给资产挂载 Tag 前须先确保该 Tag 已存在。",
                "inputSchema": {
                    "type": "object",
                    "properties": { "name": { "type": "string", "description": "标签名，1-32 字，无需带 #" } },
                    "required": ["name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_tag_rename",
                "description": "重命名一个已有受控 Tag，并自动同步更新所有关联提示词与 Skill 的标签。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "from": { "type": "string" },
                        "to": { "type": "string" }
                    },
                    "required": ["from", "to"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_tag_delete",
                "description": "删除一个受控 Tag，并自动从所有关联资产移除该标签（保留资产本身）。",
                "inputSchema": {
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_asset_list",
                "description": "列出样板间中的资产。基础类型只有 prompt（提示词）和 skill（Skill 文件夹），方向只有 text（文）、image（图）、video（视频）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["prompt", "skill"] },
                        "modality": { "type": "string", "enum": ["text", "image", "video"] },
                        "tag": { "type": "string" },
                        "search": { "type": "string" }
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_asset_get",
                "description": "根据资产 ID 读取完整详情（含 prompt 或 SKILL.md、样板图列表与当前 revision）。修改或删除前先调用此工具获取 revision。",
                "inputSchema": {
                    "type": "object",
                    "properties": { "id": { "type": "string" } },
                    "required": ["id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_image_import",
                "description": "将本地 PNG/JPEG/WebP 图片导入样板间图片库（按内容 SHA-256 去重），返回可填入资产 images 数组的图片标识文件名。",
                "inputSchema": {
                    "type": "object",
                    "properties": { "filePath": { "type": "string", "description": "本机图片绝对路径" } },
                    "required": ["filePath"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_prompt_save",
                "description": "新建或更新一条提示词（prompt）样板。新建时省略 id；更新已有提示词时必须传入 id 与当前 expectedRevision。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "更新时传入已有 ID；新建留空" },
                        "expectedRevision": { "type": "integer", "description": "更新时传入读取到的当前 revision；新建为 0 或省略" },
                        "modality": { "type": "string", "enum": ["text", "image", "video"], "description": "方向：text(文) / image(图) / video(视频)" },
                        "name": { "type": "string" },
                        "prompt": { "type": "string", "description": "提示词完整正文" },
                        "sample": { "type": "string", "description": "文字样板示例或效果简述（可选）" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "已登记的受控 Tag 列表" },
                        "images": { "type": "array", "items": { "type": "string" }, "description": "creatorx_image_import 返回的样板图文件名列表" },
                        "notes": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["modality", "name", "prompt"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_skill_import",
                "description": "导入或整夹更新一个真实 Skill 文件夹（根目录必须含 UTF-8 SKILL.md，保留脚本与参考资源且不执行脚本）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "更新已有 Skill 时传入 ID；新建省略" },
                        "expectedRevision": { "type": "integer", "description": "更新已有 Skill 时传入当前 revision；新建省略" },
                        "folderPath": { "type": "string", "description": "本机 Skill 文件夹绝对路径" },
                        "modality": { "type": "string", "enum": ["text", "image", "video"] },
                        "name": { "type": "string" },
                        "sample": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "images": { "type": "array", "items": { "type": "string" } },
                        "notes": { "type": "string" },
                        "source": { "type": "string" }
                    },
                    "required": ["folderPath", "modality"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_skill_update",
                "description": "在不重新导入整个文件夹的情况下，更新已有 Skill 的元数据、标签、样板图或 SKILL.md 正文（保留文件夹内其他资源文件）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "expectedRevision": { "type": "integer" },
                        "modality": { "type": "string", "enum": ["text", "image", "video"] },
                        "name": { "type": "string" },
                        "skillMarkdown": { "type": "string" },
                        "sample": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "images": { "type": "array", "items": { "type": "string" } },
                        "notes": { "type": "string" }
                    },
                    "required": ["id", "expectedRevision"],
                    "additionalProperties": false
                }
            },
            {
                "name": "creatorx_asset_delete",
                "description": "按 ID 与当前 expectedRevision 安全删除一条提示词或 Skill。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "expectedRevision": { "type": "integer" }
                    },
                    "required": ["id", "expectedRevision"],
                    "additionalProperties": false
                }
            }
        ]
    })
}

pub fn call_tool(store: &LibraryStore, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "creatorx_tag_list" => Ok(json!({ "tags": store.tag_list()? })),
        "creatorx_tag_add" => {
            let tag = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or("缺少参数 name")?;
            Ok(json!({ "tags": store.tag_add(tag)? }))
        }
        "creatorx_tag_rename" => {
            let from = args
                .get("from")
                .and_then(Value::as_str)
                .ok_or("缺少参数 from")?;
            let to = args
                .get("to")
                .and_then(Value::as_str)
                .ok_or("缺少参数 to")?;
            Ok(json!({ "tags": store.tag_rename(from, to)? }))
        }
        "creatorx_tag_delete" => {
            let tag = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or("缺少参数 name")?;
            Ok(json!({ "tags": store.tag_delete(tag)? }))
        }
        "creatorx_asset_list" => {
            let kind = args.get("kind").and_then(Value::as_str);
            let modality = args
                .get("modality")
                .and_then(Value::as_str)
                .map(parse_modality)
                .transpose()?;
            let tag = args
                .get("tag")
                .and_then(Value::as_str)
                .map(|t| t.trim().trim_start_matches('#').to_string())
                .filter(|t| !t.is_empty());
            let search = args
                .get("search")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let lib = store.list()?;
            let items: Vec<_> = lib
                .entries
                .into_iter()
                .filter(|e| {
                    let ok_kind = kind.is_none_or(|k| (k == "prompt") == (e.kind == Kind::Prompt));
                    ok_kind
                        && modality.is_none_or(|m| e.modality == m)
                        && tag.as_ref().is_none_or(|t| e.tags.contains(t))
                        && (search.is_empty()
                            || format!(
                                "{} {} {} {} {} {}",
                                e.name,
                                e.prompt,
                                e.skill_markdown,
                                e.sample,
                                e.notes,
                                e.tags.join(" ")
                            )
                            .to_lowercase()
                            .contains(&search))
                })
                .collect();
            Ok(json!({
                "count": items.len(),
                "imageDirectory": lib.image_directory,
                "tags": lib.tags,
                "entries": items
            }))
        }
        "creatorx_asset_get" => {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or("缺少参数 id")?;
            let entry = store
                .list()?
                .entries
                .into_iter()
                .find(|e| e.id == id)
                .ok_or_else(|| format!("未找到资产：{id}"))?;
            let files = if entry.kind == Kind::Skill {
                store.skill_files(&entry.id)?
            } else {
                vec![]
            };
            Ok(json!({ "entry": entry, "skillFiles": files }))
        }
        "creatorx_image_import" => {
            let file_path = args
                .get("filePath")
                .and_then(Value::as_str)
                .ok_or("缺少参数 filePath")?;
            let name = store.import_image_file(std::path::Path::new(file_path))?;
            Ok(json!({ "image": name }))
        }
        "creatorx_prompt_save" => {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let expected = args
                .get("expectedRevision")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if !id.is_empty() && expected == 0 {
                return Err("更新已有提示词必须传入当前 expectedRevision（>= 1）".into());
            }
            let modality = parse_modality(
                args.get("modality")
                    .and_then(Value::as_str)
                    .ok_or("缺少参数 modality")?,
            )?;
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or("缺少参数 name")?
                .to_string();
            let prompt = args
                .get("prompt")
                .and_then(Value::as_str)
                .ok_or("缺少参数 prompt")?
                .to_string();
            let tags: Vec<String> = args
                .get("tags")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let images: Vec<String> = args
                .get("images")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let entry = Entry {
                schema_version: 5,
                id,
                kind: Kind::Prompt,
                modality,
                name,
                prompt,
                sample: args
                    .get("sample")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                tags,
                notes: args
                    .get("notes")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                origin_created_at: None,
                origin_updated_at: None,
                skill_markdown: String::new(),
                skill_digest: None,
                images,
                revision: 0,
                updated_at: 0,
                source: args.get("source").and_then(Value::as_str).map(str::to_string),
            };
            serde_json::to_value(store.save(entry, expected, None)?).map_err(|e| e.to_string())
        }
        "creatorx_skill_import" => {
            let folder = PathBuf::from(
                args.get("folderPath")
                    .and_then(Value::as_str)
                    .ok_or("缺少参数 folderPath")?,
            );
            if std::fs::symlink_metadata(&folder)
                .map_err(|_| "无法读取文件夹")?
                .file_type()
                .is_symlink()
            {
                return Err("Skill 文件夹不能是符号链接".into());
            }
            let raw_files = LibraryStore::read_package(&folder)?;
            let doc = raw_files
                .iter()
                .find(|(n, _)| n == "SKILL.md")
                .ok_or("Skill 文件夹根目录必须包含 SKILL.md")?;
            let markdown =
                String::from_utf8(doc.1.clone()).map_err(|_| "SKILL.md 必须是 UTF-8 编码")?;
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let expected = args
                .get("expectedRevision")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if !id.is_empty() && expected == 0 {
                return Err("更新已有 Skill 必须传入当前 expectedRevision（>= 1）".into());
            }
            let modality = parse_modality(
                args.get("modality")
                    .and_then(Value::as_str)
                    .ok_or("缺少参数 modality")?,
            )?;
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    folder
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                });
            let tags: Vec<String> = args
                .get("tags")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let images: Vec<String> = args
                .get("images")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let existing_digest = if !id.is_empty() {
                store
                    .list()?
                    .entries
                    .into_iter()
                    .find(|e| e.id == id)
                    .and_then(|e| e.skill_digest)
            } else {
                None
            };
            let entry = Entry {
                schema_version: 5,
                id,
                kind: Kind::Skill,
                modality,
                name,
                prompt: String::new(),
                sample: args
                    .get("sample")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                tags,
                notes: args
                    .get("notes")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                origin_created_at: None,
                origin_updated_at: None,
                skill_markdown: markdown,
                skill_digest: existing_digest,
                images,
                revision: 0,
                updated_at: 0,
                source: args.get("source").and_then(Value::as_str).map(str::to_string),
            };
            let import_files = raw_files
                .into_iter()
                .map(|(path, bytes)| ImportFile {
                    path,
                    base64: STANDARD.encode(bytes),
                })
                .collect();
            serde_json::to_value(store.save(entry, expected, Some(import_files))?)
                .map_err(|e| e.to_string())
        }
        "creatorx_skill_update" => {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or("缺少参数 id")?;
            let expected = args
                .get("expectedRevision")
                .and_then(Value::as_u64)
                .ok_or("缺少参数 expectedRevision")?;
            let mut existing = store
                .list()?
                .entries
                .into_iter()
                .find(|e| e.id == id)
                .ok_or_else(|| format!("未找到 Skill：{id}"))?;
            if existing.kind != Kind::Skill {
                return Err("该资产不是 Skill".into());
            }
            if let Some(m) = args.get("modality").and_then(Value::as_str) {
                existing.modality = parse_modality(m)?;
            }
            if let Some(n) = args.get("name").and_then(Value::as_str) {
                existing.name = n.into();
            }
            if let Some(md) = args.get("skillMarkdown").and_then(Value::as_str) {
                existing.skill_markdown = md.into();
            }
            if let Some(s) = args.get("sample").and_then(Value::as_str) {
                existing.sample = s.into();
            }
            if let Some(notes) = args.get("notes").and_then(Value::as_str) {
                existing.notes = notes.into();
            }
            if let Some(arr) = args.get("tags").and_then(Value::as_array) {
                existing.tags = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
            }
            if let Some(arr) = args.get("images").and_then(Value::as_array) {
                existing.images = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
            }
            serde_json::to_value(store.save(existing, expected, None)?).map_err(|e| e.to_string())
        }
        "creatorx_asset_delete" => {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or("缺少参数 id")?;
            let expected = args
                .get("expectedRevision")
                .and_then(Value::as_u64)
                .ok_or("缺少参数 expectedRevision")?;
            store.delete_entry(id, expected)?;
            Ok(json!({ "deleted": id }))
        }
        _ => Err(format!("未知的 CreatorX MCP 工具：{name}")),
    }
}

pub fn handle_rpc_message(store: &LibraryStore, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").unwrap_or(&Value::Null);

    let result = match method {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05");
            let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&requested) {
                requested
            } else {
                "2024-11-05"
            };
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "creatorx",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": "CreatorX 样板间资产维护服务：基础类型仅 prompt（提示词）与 skill（Skill 文件夹）两种，方向仅 text（文）、image（图）、video（视频）三种。Tag 采用受控表管理，挂载新 Tag 前请先调用 creatorx_tag_add。更新或删除已有资产须先读取最新 revision。"
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_schema()),
        "tools/call" => {
            let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params.get("arguments").unwrap_or(&Value::Null);
            match call_tool(store, tool_name, arguments) {
                Ok(data) => Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
                    }],
                    "isError": false
                })),
                Err(err) => Ok(json!({
                    "content": [{ "type": "text", "text": err }],
                    "isError": true
                })),
            }
        }
        _ => Err((-32601, format!("Method not found: {method}"))),
    };

    Some(match result {
        Ok(res) => json!({ "jsonrpc": "2.0", "id": id, "result": res }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    })
}

enum Framing {
    Newline,
    ContentLength,
}

fn read_next_message(reader: &mut impl BufRead) -> std::io::Result<Option<(Value, Framing)>> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("Content-Length:")
            .or_else(|| trimmed.strip_prefix("content-length:"))
        {
            let length: usize = rest
                .trim()
                .parse()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            loop {
                line.clear();
                let h = reader.read_line(&mut line)?;
                if h == 0 || line.trim().is_empty() {
                    break;
                }
            }
            let mut buf = vec![0u8; length];
            reader.read_exact(&mut buf)?;
            let val: Value = serde_json::from_slice(&buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            return Ok(Some((val, Framing::ContentLength)));
        }
        let val: Value = serde_json::from_str(trimmed)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        return Ok(Some((val, Framing::Newline)));
    }
}

pub fn run_stdio() -> i32 {
    let store = match default_library_root().and_then(LibraryStore::new) {
        Ok(s) => s,
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "CreatorX MCP init error: {err}");
            return 1;
        }
    };
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    while let Ok(Some((msg, framing))) = read_next_message(&mut reader) {
        if let Some(reply) = handle_rpc_message(&store, &msg) {
            let serialized = serde_json::to_string(&reply).unwrap_or_default();
            let write_res = match framing {
                Framing::Newline => writeln!(writer, "{serialized}").and_then(|_| writer.flush()),
                Framing::ContentLength => write!(
                    writer,
                    "Content-Length: {}\r\n\r\n{}",
                    serialized.len(),
                    serialized
                )
                .and_then(|_| writer.flush()),
            };
            if write_res.is_err() {
                break;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_roundtrip_maintains_controlled_tags_prompts_and_skills() {
        let dir = tempfile::tempdir().unwrap();
        let store = LibraryStore::new(dir.path().join("library")).unwrap();

        let init = handle_rpc_message(
            &store,
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");

        let tools = handle_rpc_message(&store, &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(tools["result"]["tools"].as_array().unwrap().len() >= 10);

        // Unregistered tag returns isError = true
        let rejected = handle_rpc_message(
            &store,
            &json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"creatorx_prompt_save","arguments":{
                    "modality":"image","name":"安静设定","prompt":"黄乱发黑卫衣","tags":["安静"]
                }}
            }),
        )
        .unwrap();
        assert_eq!(rejected["result"]["isError"], true);

        // Register tag then save prompt
        let add_tag = handle_rpc_message(
            &store,
            &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"creatorx_tag_add","arguments":{"name":"安静"}}}),
        )
        .unwrap();
        assert_eq!(add_tag["result"]["isError"], false);

        let saved = handle_rpc_message(
            &store,
            &json!({
                "jsonrpc":"2.0","id":5,"method":"tools/call",
                "params":{"name":"creatorx_prompt_save","arguments":{
                    "modality":"image","name":"安静设定","prompt":"黄乱发黑卫衣","tags":["安静"]
                }}
            }),
        )
        .unwrap();
        assert_eq!(saved["result"]["isError"], false);
        assert_eq!(store.list().unwrap().entries.len(), 1);
    }
}
