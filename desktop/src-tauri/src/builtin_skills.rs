use crate::library::{DeliverableFormat, Entry, Kind, Maturity};

pub fn entries() -> Vec<Entry> {
    [
        (
            "cards-series",
            "卡片图文样板",
            "卡片图文",
            DeliverableFormat::Cards,
            "适用小红书／抖音 3:4 系列卡片：封面钩子卡 ＋ 分页重点卡 ＋ 发布短配文",
            include_str!("../../skills/cards-series/SKILL.md"),
        ),
        (
            "article-illustrated",
            "长文配图样板",
            "长文配图",
            DeliverableFormat::Article,
            "适用微信公众号深度长文：结构化 Markdown 定稿 ＋ 封面与概念插图锚点",
            include_str!("../../skills/article-illustrated/SKILL.md"),
        ),
        (
            "video-storyboard",
            "视频分镜样板",
            "视频创作",
            DeliverableFormat::Video,
            "适用抖音／小红书短视频：前 3 秒钩子口播稿 ＋ 逐镜画面／时长／关键帧蓝图",
            include_str!("../../skills/video-storyboard/SKILL.md"),
        ),
        (
            "image-single",
            "单张视觉图文",
            "卡片图文",
            DeliverableFormat::Cards,
            "适用单张海报／信息图：确认核心文案与构图后生成单张成图",
            include_str!("../../skills/image-single/SKILL.md"),
        ),
        (
            "image-revision",
            "原图受控修改",
            "卡片图文",
            DeliverableFormat::Cards,
            "基于已有槽位原图做局部文字或细节微调，保留全部历史版本",
            include_str!("../../skills/image-revision/SKILL.md"),
        ),
    ]
    .into_iter()
    .map(|(id, name, category, deliverable, sample_output, markdown)| Entry {
        schema_version: 3,
        id: format!("builtin:{id}"),
        kind: Kind::Skill,
        name: name.into(),
        prompt: String::new(),
        category: category.into(),
        deliverable,
        track: String::new(),
        maturity: Maturity::Showcase,
        sample_input: String::new(),
        sample_output: sample_output.into(),
        tags: vec![],
        notes: String::new(),
        origin_created_at: None,
        origin_updated_at: None,
        skill_markdown: markdown.into(),
        skill_digest: None,
        images: vec![],
        revision: 1,
        updated_at: 0,
        source: Some("CreatorX 内置样板".into()),
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
