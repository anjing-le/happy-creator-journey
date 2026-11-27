# CreatorX 客户端说明

当前版本 **0.1.1**：基于**“底层原子样板（文／图／视频 ＋ `#Tag`）× 上层组合创作（图文／视频）”**的极简 Creation Harness 客户端。

- **首页**：上方并排 **图文**、**视频** 两块无字蜡笔插画入口，下方整行为 **样板间** 无字插画入口。
- **样板间**：所有资产同构维护——只有 **载体（`提示词`／`Skill` 文件夹）**、**原子能力（`文`／`图`／`视频`）**、**样板展示（样板图或输出文字样例）** 与 **`#Tags`（如 `#安静`、`#计算机`、`#金融`，支持一键点选已有标签防打散）**。内置 `单张生图`、`系列套图`、`原图微调` 三个精简原子 Skill。
- **创作台（图文 ／ 视频）**：左侧与本地 Agent 对话，按 `文 / 图 / 视频` 和 `#Tag` 快速勾选本轮要引用的提示词与细粒度 Skill（自动冻结进 `snapshots/<seq>/`）；右侧展示定稿文稿与独立生图／微调版本流（保留全部历史原图，支持一键加 `#Tag` 存回样板间）。
- **本地 Agent 适配**：`CodexAdapter` 已接通本机 `codex app-server`（`gpt-6.1-sol`／`medium`），`AntigravityAdapter` 预留本地接口扩展。详见 [ARCHITECTURE.md](ARCHITECTURE.md) 与 [CLI.md](CLI.md)。

## 本地开发与验证

```sh
cd desktop
npm ci
npm run verify && npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run desktop
```

依据：[0.1.1 Release](https://github.com/anjing-le/happy-creator-journey/releases/tag/creatorx-v0.1.1)、[双平台检查](https://github.com/anjing-le/happy-creator-journey/actions/runs/46015916749)、[三平台构建与签名验证](https://github.com/anjing-le/happy-creator-journey/actions/runs/46015922544)。

## 本机数据与边界说明

- `~/Library/Application Support/cc.anjing.creatorx/archive-v0.0.17/` 保留重构前归档的 41 条旧提示词、16 张参考图、1 个私人 Skill 与 1 条历史创作记录；当前 `library/` 与 `creations/` 保持干净起步。
- 本机安装版 `~/Applications/CreatorX.app` 经严格签名校验后，使用 `scripts/macos-file-icon.swift` 设置本地持久 Finder 透明图标（详见 [icon-validation.md](icon-validation.md)，不进入 CI／OTA 包）。
- 待验证项保持如实记录：OTA 自动重启后的窗口唤起异常待排查；鼠标侧键待真鼠核对；Windows 真机原生 Codex 交互与平台公证／代码签名待后续推进。
