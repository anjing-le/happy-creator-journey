# CreatorX 客户端说明

当前版本 **0.1.2**：**零传统 CRUD 界面 ＋ 内置 `creatorx mcp` Agent 原生维护**的极简 Creation Harness 客户端。

- **只读样板画册**：仅含 2 种基础类型（`提示词`、`Skill`）、3 个方向（`文`、`图`、`视频`）与受控 `#Tag` 切换浏览；无任何增删改查输入框或管理按钮，外部 Agent 更新本地库后窗口自动无感刷新。
- **内置 MCP ＆ CLI**：同一个 `creatorx` 二进制提供 `creatorx mcp` 标准 Stdio 服务（见 [CLI.md](CLI.md) 与 [ARCHITECTURE.md](ARCHITECTURE.md)），Antigravity、Codex、Claude Code 等任意 Agent 均可直接维护受控 Tag、提示词、Skill 文件夹与样板图。
- **纯净创作台（图文 ／ 视频）**：左侧对话并勾选引用已有样板，后台冻结只读快照并保护历版原图；右侧纯粹展示作品与定稿内容。

```sh
cd desktop
npm ci
npm run verify && npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run desktop
```

本机历史数据备份于 `~/Library/Application Support/cc.anjing.creatorx/archive-v0.0.17/`，活动库保持干净；本机 Antigravity 已在 `~/.gemini/config/mcp_config.json` 配置 `creatorx` MCP 服务。待验证项保持如实记录：OTA 自动重启窗口唤起异常待排查、鼠标侧键待真鼠核对、Windows 真机交互与代码签名／公证待后续推进。
