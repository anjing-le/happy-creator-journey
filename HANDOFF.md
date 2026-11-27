# CreatorX 接续说明

核对日期：2026-11-27。本文是接续摘要；后续接手前须先核对当前 Git 状态、本机安装版本与本地应用数据目录，不把文档记录当作当前运行事实。

## 项目与阅读入口

- GitHub：https://github.com/anjing-le/happy-creator-journey
- 本机仓库：`/Users/lvxianghe/project/anjing/happy-creator-journey`
- 本机正式安装：`~/Applications/CreatorX.app`（当前基线 `0.1.0`）
- 阅读顺序：根目录 `AGENTS.md` → `design.md` → `desktop/ARCHITECTURE.md` → `desktop/AGENTS.md`，再按任务读 `desktop/README.md`、`desktop/CLI.md`、`desktop/icon-validation.md`。

## 已落地架构（`0.1.0` Creation Harness 横向底座）

CreatorX 明确定位为构建在本地 Agent 之上的 **Creation Harness（创作治具／编排约束台）**，坚持**可复用骨架＋创作者私有数据分离**：

1. **首页与三大交付制式**：
   - 首页上方两块无字蜡笔插画为 **图文**、**视频**，下方横跨整行为 **样板间**（暖米白配色、无可见文字、统一 `Choice` 自定义弹层）。
   - “图文”工作台内置切换 **`卡片图文`**（小红书／抖音多图卡片＋短配文）与 **`长文配图`**（微信公众号 Markdown 长文＋内嵌插图槽位）；“视频”工作台提供 **`视频创作`**（口播逐字稿＋结构化分镜镜头卡＋关键帧／分镜提示词包）。
2. **样板间与 IP 底座（`library.rs` ＋ `Showcase.tsx`）**：
   - 支持唯一 **IP 底座**（`ip.json`：角色视觉标准、参考图、表达口吻、赛道列表如计算机／金融／音乐，带 `revision` 乐观锁）；
   - 支持 **制式 Skills**（含 5 套内置样板与个人文件夹 Skill）与 **风格提示词**，按赛道和成熟度（`样板`／`草稿`）筛选，画廊封面优先展示成品图或文字样板，支持一键“用此样板去创作”；
   - GUI 与本地 `creatorx` CLI 共用存储、跨进程文件锁与修订冲突检查。
3. **Harness 四道硬门禁与可插拔 Agent（`harness.rs` ＋ `adapters/`）**：
   - ① `snapshots/<seq>/` 只读快照锁（默认挂载 IP 底座与选定样板文件／参考图，切换挂载递增快照序号，不改历史快照）；
   - ② `blueprint.json`／`draft.md` 沙盒文件契约与 `blueprintApproved` 阶段门禁（未确认放行前拦截自主批量槽位生图）；
   - ③ `artifacts/<slot-id>/v<N>.<ext>` 追加式版本链与真实图像文件头硬校验（单槽位微调自动引用上一版原图，绝不覆盖历史版本）；
   - ④ `promote_to_showcase` 成品一键提炼回写样板间；
   - `CodexAdapter` 已接通本机 `codex app-server`（固定 `gpt-6.1-sol`／`medium`），`AntigravityAdapter` 已预留标准本机适配槽位（零凭据托管）。

## 只在本机的私有数据边界

- `~/Library/Application Support/cc.anjing.creatorx/archive-v0.0.17/`：重构前归档保留的本机旧数据备份（41 条提示词、16 张参考图、1 个私人 Skill 文件夹、1 条历史创作记录），原封未删，供后续挑选有效资产迁入新样板间。
- 同级 `library/` 与 `creations/`：重构后从零干净启动的活动目录。严禁公开私人素材、作品原图或本地凭据。
- 本机安装版 `~/Applications/CreatorX.app` 在干净构建验签后，使用 `desktop/scripts/macos-file-icon.swift` 设置本地持久 Finder 透明图标。该操作添加 Finder 元数据，会使严格代码签名检查报错，**仅限本机定制，绝不放入 CI／OTA 发行包**。

## 待实测与未完成边界（切勿误标为已完成）

- **真实资产积累与联调打磨**：横向骨架与自动化单测／CLI 检验已完成；下一步由用户在新样板间录入“安静”IP 底座与精选样板，并用真实创作逐项打磨卡片图文、长文配图与视频分镜手感。
- **OTA 自动重启窗口唤起**：此前记录的 OTA 自动重启后窗口唤起异常仍待专项排查，本机直接覆盖安装不能替代完整 OTA 验收。
- **硬件与跨平台验收**：鼠标侧键 `button 3/4` 待真鼠反馈；Windows 原生 `codex.exe` 真机交互尚未实测；Apple 公证与 Windows 代码签名尚未配置。
- **视频渲染后端与候选 Skill**：固定视频渲染后端（如 Remotion）尚未接入；Seedance `sd25-pe` 仅有调研结论，未安装接入。

## 工程交付习惯

- 主要源码：`desktop/src/Workspace.tsx`、`Showcase.tsx`、`Creation.tsx`、`Choice.tsx`、`library.ts`、`navigation.ts`；后端 `desktop/src-tauri/src/library.rs`、`harness.rs`、`adapters/mod.rs`、`adapters/codex.rs`、`adapters/antigravity.rs`、`cli.rs`、`lib.rs`。
- 提交／推送前执行 `git fetch` 检查远端 Star Growth 工作流提交，只合并已核对变化，不强推；Git 身份与 GitHub 发布账号统一使用 `anjing-le`。
