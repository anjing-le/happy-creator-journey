use crate::library::Entry;
use serde_json::json;

pub fn entries() -> Vec<Entry> {
    [
        (
            "image-single",
            "单张图文",
            include_str!("../../skills/image-single/SKILL.md"),
        ),
        (
            "image-series",
            "系列图文",
            include_str!("../../skills/image-series/SKILL.md"),
        ),
        (
            "image-revision",
            "图片修改",
            include_str!("../../skills/image-revision/SKILL.md"),
        ),
    ]
    .into_iter()
    .map(|(id, name, markdown)| {
        serde_json::from_value(json!({
            "id":format!("builtin:{id}"), "kind":"skill", "name":name,
            "skillMarkdown":markdown, "source":"CreatorX 内置"
        }))
        .expect("valid bundled skill")
    })
    .collect()
}

pub fn get(id: &str) -> Option<Entry> {
    entries().into_iter().find(|entry| entry.id == id)
}

#[tauri::command]
pub fn creation_builtin_skills() -> Vec<Entry> {
    entries()
}
