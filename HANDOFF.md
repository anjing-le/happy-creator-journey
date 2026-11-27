# CreatorX 接续说明

核对日期：2026-11-27。后续接手前先核对 Git 状态、本机安装版与本地数据目录，不把文档记录当作当前运行事实。

## 项目入口

- GitHub：https://github.com/anjing-le/happy-creator-journey
- 本机仓库：`/Users/lvxianghe/project/anjing/happy-creator-journey`
- 本机正式安装：`~/Applications/CreatorX.app`（当前基线 `0.1.1`）
- 阅读顺序：`AGENTS.md` → `design.md` → `desktop/ARCHITECTURE.md` → `desktop/AGENTS.md`，按任务查阅 `desktop/README.md`、`desktop/CLI.md`、`desktop/icon-validation.md`。

## 已确认并落地的极简架构（`0.1.1`）

1. **不做新 Agent，只做极简 Creation Harness**：连接本机已登录的 Agent（`CodexAdapter` 已打通 `codex app-server`，`AntigravityAdapter` 预留本地槽位），零凭据托管；保持可复用骨架与个人私有数据分离。
2. **去掉“IP 底座”与多重分类表单，回归底层原子积木**：
   - 最底层的创作能力只有 **生文（`文`）**、**生图（`图`）**、**生视频（`视频`）**；
   - 样板间每条资产只含 4 要素：**载体（`提示词`／`Skill` 文件夹）**、**原子方向（`文`／`图`／`视频`）**、**样板展示（样板图或文字样例）**、**`#Tags`（如 `#安静`、`#计算机`、`#金融`，编辑器内置历史标签点选复用）**；
   - 内置仅保留 3 个细粒度生图原子 Skill：`单张生图`、`系列套图`、`原图微调`。
3. **上层组合创作台（图文 ／ 视频）**：
   - 首页上方为 **图文**、**视频** 两块无字蜡笔插画，下方整行为 **样板间**；
   - 创作台左侧自然交流，按 `文 / 图 / 视频` 与 `#Tag` 快速勾选组合提示词和细粒度 Skill（自动冻结只读快照 `snapshots/<seq>/`）；右侧呈现 `draft.md` 定稿文稿与生图／微调版本卡片（校验真实图片文件头、改图引用真实原图且不覆盖旧版，支持一键加 `#Tag` 沉淀回样板间）。

## 本机数据与待验证边界

- 旧版数据已安全备份在 `~/Library/Application Support/cc.anjing.creatorx/archive-v0.0.17/`（41 条提示词、16 张参考图、1 个私人 Skill、1 条创作记录）；当前 `library/` 从零干净积累。
- 本机安装版退出透明图标通过 `desktop/scripts/macos-file-icon.swift` 本地定制，不进入公开安装包。
- 待验证项：OTA 自动重启后的窗口唤起异常待排查；鼠标侧键待真鼠实测；Windows 真机交互、平台代码签名／公证及视频渲染细粒度工具按后续实际创作逐步推进。
