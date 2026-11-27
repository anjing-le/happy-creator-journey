# CreatorX 客户端协作约束

- **零传统 CRUD 界面**：桌面前端不做任何添加、编辑、删除按钮、搜索输入框或管理表单；样板间仅为自动热同步的只读画册，创作台仅为对话输入（可勾选引用已有样板）与右侧作品只读展示。
- **Agent 原生维护（MCP ＋ CLI）**：所有提示词、Skill 文件夹、样板图和受控 Tag 统一通过内置 `creatorx mcp`（Stdio MCP Server）或本地 CLI JSON 接口维护；桌面端 IPC 不暴露资产写入命令。
- **最简数据模型**：基础类型严格只有 `prompt`（提示词）和 `skill`（文件夹）两种；方向严格只有 `text`（文）、`image`（图）、`video`（视频）三种；Tag 采用 `tags.json` 受控表逐一维护，未登记 Tag 拒绝挂载。
- **Creation Harness 与桌面规范**：保留 `snapshots/<seq>/` 只读快照锁、独立生图原图保护、可插拔 `AgentAdapter`（Codex 已打通，Antigravity 预留）、暖米白无字插画首页、自定义 `Choice` 弹层、macOS 隐藏三按钮与 `Command + [`／`]` 导航、静默 OTA 及本机安装后 Finder 透明图标脚本边界。
