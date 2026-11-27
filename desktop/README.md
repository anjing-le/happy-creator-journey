# CreatorX 客户端说明

当前版本 **0.1.0**：已完成 **Creation Harness（创作治具）＋ 样板间 ＋ 三大交付制式** 横向重构。

- **首页**：上方并排 **图文**、**视频** 两块无字安静 IP 蜡笔插画入口，下方横跨整行为 **样板间** 无字插画入口。
- **样板间与 IP 底座**：维护唯一 **IP 底座**（角色标准、参考图、人设口吻与内容赛道）、**制式 Skills**（`卡片图文`／`长文配图`／`视频创作`）与 **风格提示词**（`视觉画风`／`写作母版`／`脚本钩子`）。支持按赛道与成熟度（`样板`／`草稿`）筛选，画廊封面直接展示真实成品图或文字样板，支持一键 **“用此样板去创作”**。
- **Creation Harness 创作台**：
  - **图文**入口内置切换 **卡片图文**（小红书／抖音多图卡片＋短配文）与 **长文配图**（微信公众号 Markdown 长文＋内嵌封面／插图槽位）；
  - **视频**入口提供 **视频创作**（口播逐字稿＋分镜镜头卡＋关键帧参考图／分镜提示词包）；
  - 四道硬门禁由 Rust `harness.rs` 强制执行：① `snapshots/<seq>/` 只读上下文快照锁；② `blueprint.json`／`draft.md` 结构化蓝图确认门禁；③ `artifacts/<slot-id>/v<N>.<ext>` 追加式真实图像文件硬校验（单槽位微调自动引用上一版原图，绝不覆盖旧版）；④ 满意作品一键提炼回写样板间。
- **可插拔本地 Agent 适配层**：`adapters/codex.rs` 连接本机已登录 `codex app-server`（固定 `gpt-6.1-sol`／`medium`），`adapters/antigravity.rs` 预留本机 Antigravity 标准适配接口；客户端不托管或复制第三方账号凭据。

模块边界与磁盘沙盒契约见 [ARCHITECTURE.md](ARCHITECTURE.md)，命令行接口见 [CLI.md](CLI.md)。

## 本地开发与检查

```sh
cd desktop
npm ci
npm run verify && npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run desktop
```

打包后可用 `node scripts/check-cli.mjs <应用可执行文件>` 验证独立进程 CLI 的 JSON 输出、IP 底座、分类、删除与修订冲突保护。

## 本机数据与图标边界

- macOS 应用数据位于 `~/Library/Application Support/cc.anjing.creatorx/`：
  - `library/`：当前活动资产库（从零干净启动，按新结构逐步沉淀）；
  - `creations/`：当前活动创作任务沙盒；
  - `archive-v0.0.17/`：重构前归档保留的本机历史数据备份（41 条提示词、16 张参考图、1 个私人 Skill 文件夹与 1 条历史创作记录），未删除、不公开。
- macOS 隐藏左上角三个原生窗口按钮，`Command + W` 隐藏窗口保活、`Command + Q` 完全退出；全局支持 `Command + [`／`]` 与鼠标侧键 `button 3/4` 导航。
- macOS 本机退出后的无底板图标通过 `scripts/macos-file-icon.swift` 设置 Finder 自定义图标（详见 [icon-validation.md](icon-validation.md)）。该元数据会影响严格代码签名校验，**仅用于本机安装目录**，不进入 CI／OTA 发行包；覆盖安装后重新执行即可恢复。
- **待实测与待排查边界**：OTA 自动重启后的窗口唤起异常仍待排查；鼠标侧键待真鼠核对；Windows 真机原生 Codex 交互、Apple 公证与 Windows 代码签名尚未配置；固定视频渲染后端（如 Remotion）按后续真实 SOP 接入。
