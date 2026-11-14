# CreatorX

当前 **0.0.9**：文、图、视频分别进入 Codex 对话，固定 `gpt-6.1-sol`／`medium`，各自保存历史。打开后可直接交流，无需先选 Skill；聊天中可添加或更换 Skill、IP 提示词，按请求制作与修改。具体方法由实践逐步打磨。素材库的返回、提示词／Skills 在同一行；搜索和加号同一行，去除重复标题、引号装饰、用途标签和筛选。

```sh
cd desktop
npm ci
npm run desktop
```

本地 CLI 与界面共用同一份数据，无需启动窗口。命令发现、JSON 输入、修订检查和完整 Skill 导入见 [CLI 说明](CLI.md)。

```sh
creatorx schema
creatorx library list --kind prompt --search 蜡笔
creatorx library categories
```

macOS 数据位于 `~/Library/Application Support/cc.anjing.creatorx/library/`：`entries/` 保存记录，`images/` 保存原图，`skills/<id>/` 保存 Skill 的真实文件夹。Windows 使用系统应用数据目录。私人材料与 Hub 导出不随公共安装包分发。

验证：`npm run verify`、`npm test`、`cargo test --locked --manifest-path src-tauri/Cargo.toml`；打包后用 `node scripts/check-cli.mjs <应用可执行文件>` 检查真实 CLI。测试覆盖跨进程冲突、完整 Skill 资源保留、外部文档修改和中断保存恢复。

本机从私人 Hub 迁入的 41 条提示词与 16 张参考图保留。旧分类保持为空，并清除之前自动添加的用途标签，重新自行积累；导入完整的安静金融图文 Skill 试跑，不把个人 Skill 与素材嵌入公开安装包。原 Hub 未改动。

首页维持暖米白、无可见文字的四个插画入口；素材维护界面使用清晰文字。插画原图位于 `public/entrances/`，提示词见 [入口插画](entrance-prompts.md)。

OTA 保持独立签名、用户确认后下载验签；更新签名不等于 Apple 公证或 Windows 代码签名。此前 OTA 自动重启后的窗口唤起仍待排查，不能以本地构建安装代替完整 OTA 验收。图标的透明检查与各系统位置的视觉状态见 [图标验证](icon-validation.md)，未核对位置不宣称完成。

## 本机 Codex

使用已安装且登录的 Codex，通过 `codex app-server` 的 stdio 协议连接。CreatorX 不复制登录凭据，也不需要另配生图 API Key；模型与生成需要联网。可用 `CREATORX_CODEX_PATH` 指定原生 Codex 可执行文件。macOS 同时查找常见安装路径；Windows 需要 `codex.exe`，不自动执行 npm 的 `.cmd` 启动脚本。其他设备须自行安装和登录，不能继承开发者本机账号。

任务保存于应用数据目录 `creations/<id>/`：包含选定 Skill 与提示词的快照、参考图、对话及 `artifacts/` 原图。聊天中更换素材会创建新的快照，旧快照保留。旧版图文任务兼容归入“图”，保留原对话与图片。Codex 会话 ID 用于继续改稿；重启后保留任务记录。任务只能在工作区写入，不自动发布、推送或改原网站。未支持的交互审批不会自动同意。关闭应用会中断本客户端启动的 Codex 进程，重新打开后可继续原任务；正在创作时阻止 OTA 安装。

本机真实后端试跑已通过“文稿 → 原生出图 → 同会话改图”，保留两张实际图片。安装版已核对两版图片展示、重启后的同任务继续对话、新增空分类和拖拽移动；临时测试分类已清理，当前 42 份素材全部未分类。`codex.rs` 中的真实服务测试默认忽略；只有显式提供私有 `CREATORX_LIVE_CASE` 才运行，CI 不调用付费模型。普通测试覆盖任务恢复、未确认内容拦截、缺少实际图片时报告失败、分类移动的修订冲突及清空分类保留正文。

[0.0.8 Release](https://github.com/anjing-le/happy-creator-journey/releases/tag/creatorx-v0.0.8)、[双平台检查](https://github.com/anjing-le/happy-creator-journey/actions/runs/43617106501)、[三平台构建、Windows 发布版 CLI 与签名验证](https://github.com/anjing-le/happy-creator-journey/actions/runs/43617109128)均已完成。Windows 原生 Codex 连接、桌面交互与图标仍需真机验证；本轮本地安装不能代替 OTA 自动重启验收。

0.0.9 新增无 Skill 对话、三入口历史分离、可选素材切换和 Markdown 回复展示；空闲时停止轮询，仅执行中或切回窗口时刷新。视频入口已接入 Codex 对话，不代表已实现固定的视频生成或渲染后端。
