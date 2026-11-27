use crate::library::{Entry, Kind, Modality};

pub fn entries() -> Vec<Entry> {
    [
        (
            "image-single",
            "单张生图",
            Modality::Image,
            vec!["生图", "单图"],
            "根据确认的文案、构图与参考图生成单张图片，核对图内文字与角色一致性。",
            include_str!("../../skills/image-single/SKILL.md"),
        ),
        (
            "image-series",
            "系列套图",
            Modality::Image,
            vec!["生图", "套图"],
            "保持同一画幅、纸面质感与角色设定，按页依次生成系列图片。",
            include_str!("../../skills/image-series/SKILL.md"),
        ),
        (
            "image-revision",
            "原图微调",
            Modality::Image,
            vec!["生图", "改图"],
            "引用上一版真实原图只改指定细节，另存新版本并完整保留旧版。",
            include_str!("../../skills/image-revision/SKILL.md"),
        ),
    ]
    .into_iter()
    .map(|(id, name, modality, tags, sample, markdown)| Entry {
        schema_version: 4,
        id: format!("builtin:{id}"),
        kind: Kind::Skill,
        modality,
        name: name.into(),
        prompt: String::new(),
        sample: sample.into(),
        tags: tags.into_iter().map(str::to_string).collect(),
        notes: String::new(),
        origin_created_at: None,
        origin_updated_at: None,
        skill_markdown: markdown.into(),
        skill_digest: None,
        images: vec![],
        revision: 1,
        updated_at: 0,
        source: Some("CreatorX 内置原子 Skill".into()),
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
