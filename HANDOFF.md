# CreatorX 接续说明

核对日期：2026-11-27。后续接手前先核对 Git 状态、本机安装版与本地数据目录，不把文档记录当作当前运行事实。

## 项目入口

- GitHub：https://github.com/anjing-le/happy-creator-journey
- 本机仓库：`/Users/lvxianghe/project/anjing/happy-creator-journey`
- 本机正式安装：`~/Applications/CreatorX.app`（当前基线 `0.1.2`）
- 本机 Antigravity MCP 注册：`~/.gemini/config/mcp_config.json`（指向 `~/Applications/CreatorX.app/Contents/MacOS/creatorx mcp`）
- 阅读顺序：`AGENTS.md` → `design.md` → `desktop/ARCHITECTURE.md` → `desktop/AGENTS.md` → `desktop/CLI.md`。

## 已落地的核心设计（`0.1.2`）

1. **客户端零 CRUD 界面**：移除所有添加、编辑、删除、搜索框与内联表单。前端只做两件事：**纯只读样板间画册**（按 `提示词/Skill`、`文/图/视频`、`#Tag` 切换浏览，自动监听 `stateToken` 无感热刷新）与**纯净创作台**（图文／视频对话＋勾选引用已有样板＋右侧只读作品展示）。
2. **最简受控数据模型**：基础类型仅 `prompt`｜`skill` 两种；方向仅 `text`｜`image`｜`video` 三种；Tag 通过 `tags.json` 受控表逐一维护（未先登记的 Tag 拒绝挂载，重命名／删除自动级联更新）。
3. **内置 `creatorx mcp` ＋ CLI 双通道**：任何支持 MCP 或 CLI 的 Agent（Antigravity、Codex、Claude Code 等）均可脱离窗口进程直接维护 Tag、提示词、真实 Skill 文件夹和样板图片，共用文件锁与 `revision` 并发保护。

## 本机数据与待验证边界

- 旧版数据完整备份在 `~/Library/Application Support/cc.anjing.creatorx/archive-v0.0.17/`（41 条提示词、16 张参考图、1 个私人 Skill、1 条创作记录）；当前活动 `library/` 保持空库待逐条维护。
- 本机安装版无底板透明图标由 `desktop/scripts/macos-file-icon.swift` 在干净验签后本地设置，不进入公开安装包。
- 待验证项：OTA 自动重启后的窗口唤起异常待排查；鼠标侧键待真鼠实测；Windows 真机交互与平台代码签名／公证待推进。
