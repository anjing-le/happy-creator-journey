# CreatorX

当前 **0.0.15**：文、图、视频分别进入 Codex 对话，固定 `gpt-6.1-sol`／`medium`，各自保存历史。打开后可直接交流，无需先选 Skill；聊天中可添加或更换 Skill、IP 提示词，按请求制作与修改。具体方法由实践逐步打磨。素材库的返回、提示词／Skills 在同一行；搜索和加号同一行，去除重复标题、引号装饰、用途标签和筛选。

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

首页上方约七成为三张无字创作插画，下方约三成为横跨整行的素材库区域，整体铺满窗口，仅留细窄分隔。区域尺寸固定，悬停仅保留轻微插画位移缩放与透明度反馈，避免快速来回切换时反复改变布局；保留键盘聚焦与减少动态效果支持。插画原图位于 `public/entrances/`，提示词见 [入口插画](entrance-prompts.md)。

macOS 隐藏左上角关闭、最小化与全屏按钮，顶部仍可拖动，窗口仍可缩放。Command + W 暂时隐藏窗口，进程继续运行；重新打开恢复原窗口与状态。Command + Q 完全退出。Windows 保留原生窗口按钮。

OTA 约每小时后台检查，唤醒或切回窗口时补查已到期任务。默认不显示更新按钮，只在确认发现可用新版时出现；检查中、无新版、后台失败均不显示，也不弹窗打断。失败在当前进程内退避，重新启动可重试。保持独立签名、用户确认后下载验签；更新签名不等于 Apple 公证或 Windows 代码签名。此前 OTA 自动重启后的窗口唤起仍待排查，不能以本地构建安装代替完整 OTA 验收。图标的透明检查与各系统位置的视觉状态见 [图标验证](icon-validation.md)，未核对位置不宣称完成。

## 本机 Codex

使用已安装且登录的 Codex，通过 `codex app-server` 的 stdio 协议连接。CreatorX 不复制登录凭据，也不需要另配生图 API Key；模型与生成需要联网。可用 `CREATORX_CODEX_PATH` 指定原生 Codex 可执行文件。macOS 同时查找常见安装路径；Windows 需要 `codex.exe`，不自动执行 npm 的 `.cmd` 启动脚本。其他设备须自行安装和登录，不能继承开发者本机账号。

任务保存于应用数据目录 `creations/<id>/`：包含选定 Skill 与提示词的快照、参考图、对话及 `artifacts/` 原图。聊天中更换素材会创建新的快照，旧快照保留。旧版图文任务兼容归入“图”，保留原对话与图片。Codex 会话 ID 用于继续改稿；重启后保留任务记录。任务只能在工作区写入，不自动发布、推送或改原网站。未支持的交互审批不会自动同意。关闭应用会中断本客户端启动的 Codex 进程，重新打开后可继续原任务；正在创作时阻止 OTA 安装。

本机真实后端试跑已通过“文稿 → 原生出图 → 同会话改图”，保留两张实际图片。安装版已核对两版图片展示、重启后的同任务继续对话、新增空分类和拖拽移动；临时测试分类已清理，当前 42 份素材全部未分类。`codex.rs` 中的真实服务测试默认忽略；只有显式提供私有 `CREATORX_LIVE_CASE` 才运行，CI 不调用付费模型。普通测试覆盖任务恢复、未确认内容拦截、缺少实际图片时报告失败、分类移动的修订冲突及清空分类保留正文。

[0.0.15 Release](https://github.com/anjing-le/happy-creator-journey/releases/tag/creatorx-v0.0.15)、[双平台检查](https://github.com/anjing-le/happy-creator-journey/actions/runs/44902019094)、[三平台构建、Windows 发布版 CLI 与签名验证](https://github.com/anjing-le/happy-creator-journey/actions/runs/44902022377)均已完成。Windows 原生 Codex 连接、桌面交互与图标仍需真机验证；本轮本地安装不能代替 OTA 自动重启验收。

0.0.9 已在安装版实测文、图、视频三个入口不选择 Skill 的真实回复，以及聊天中添加 Skill 后读取其实际名称；重启恢复选择，旧任务的两张图片正常显示。联调会话已从客户端移出、对应 Codex 测试会话已归档。支持可选素材切换和 Markdown 回复展示；空闲时停止轮询，仅执行中或切回窗口时刷新。视频入口已接入 Codex 对话，不代表已实现固定的视频生成或渲染后端。

0.0.13 安装版已核对“上三、下整行”的首页、素材库空白处点击与默认隐藏更新按钮。后台定时检查的到期计算、失败退避、时钟异常和重启重试策略自动测试通过。

0.0.14 已在 macOS 安装版核对三个窗口按钮隐藏；Command + W 前后为同一进程，重新打开保留素材库页面及搜索内容，返回首页后仍无窗口按钮。

## 生图任务工作区

新建“图”会话采用左侧主 Codex 对话、右侧任务与作品。主会话通过 `creatorx_generate_image` 动态工具把明确的生成意图交给独立子会话；子会话继承本轮素材快照、使用原生生图工具，返回实际图片和状态。修改创建关联原任务的新子会话并复制实际参考图，保留旧版。生成失败、连接中断或缺少图片均明确记录，不把文本回复作为出图成功。

输入框下方可选单张图文、系列图文、图片修改三个内置 Skill，以及私人素材库的 Skill。内置 Skill 是 `skills/` 下的真实文件夹，随源码维护、按任务复制，不写入私人库。文与视频仍保留独立对话，本轮先完善图片生成。旧图会话继续原线程直接执行，保留历史；新建图会话使用主会话与子任务分工。

动态工具接口属于 Codex app-server 实验能力，启动时协商 `experimentalApi`；以本机实际支持为准，不支持时报告错误。参考 [官方动态工具协议](https://learn.chatgpt.com/docs/app-server#dynamic-tool-calls-experimental)。

0.0.15 本机验证：左右工作区、已存两版作品显示、输入框下方三种内置 Skill 与私人 Skill 选择均已核对。隔离目录内真实完成“仅讨论不派发 → 主会话调用工具 → 独立生图会话 → 关联原图的独立修改会话 → 结果回传”；两张实际图片已读取检查，原版字节保持一致，主会话 ID 保持不变。测试任务未进入个人作品目录，对应 Codex 测试会话已归档。自动测试覆盖旧任务兼容、内置 Skill 快照不污染私人库、拒绝非当前主会话派发和缺少图片时报错。
