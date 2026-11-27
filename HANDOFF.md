# CreatorX 接续说明

核对日期：2026-11-27。本文是接续摘要，不是完整聊天归档；后续先核对代码、安装版与真实数据，避免把记录当成当前运行事实。

## 项目与阅读入口

- GitHub：https://github.com/anjing-le/happy-creator-journey
- 本机仓库：`/Users/lvxianghe/project/anjing/happy-creator-journey`
- 本机安装：`~/Applications/CreatorX.app`；功能发布基线为 `creatorx-v0.0.17`。其后增加了本机图标定制脚本和说明，没有发布新二进制。
- 先读根目录 `AGENTS.md`、`desktop/AGENTS.md`、`design.md`，再按任务读 `desktop/README.md`、`desktop/CLI.md`、`desktop/icon-validation.md`。

## 已确认的设计

从真实成长与作品出发，探索并分享自媒体创作最佳实践。骨架与数据分离：流程、规范、通用 Skill 和工具接入可复用，创作者自己的 IP、素材、偏好、作品和过程独立保存。公共素材库是当前创作者的各工作区共用，不是联网公开内容库。

- 使用保持简单：表达意图、看结果、提修改、确认成品；复杂性放在技术内部。逐项讨论、逐项打磨，不自行扩张成大而全的平台。
- 首页上方三个无字插画入口为文、图、视频，下方整行素材库。暖米白、柔和低饱和配色；快速悬停不改变布局尺寸。
- 安静 IP：蛋黄黄乱发、黑卫衣、圆脸点眼、微笑腮红，粗蜡笔与纸纹。个人 IP 提示词和素材按选择使用，不强制所有人采用同一 IP。
- 应用内选择器共用自定义 `Choice` 弹层，避免系统原生 select／datalist。根目录 README 保持已认可图片展示，不增加独立正文；新图认可后再更新。

## 已实现与验证边界

- Tauri 2 + Rust，React + TypeScript + Vite；macOS 与 Windows 共用工程，已有 Apple Silicon、Intel Mac、Windows x64 安装包与更新签名验证。
- 文、图、视频各自保存 Codex 对话。连接本机已安装并登录的 Codex app-server，模型固定 `gpt-6.1-sol`／`medium`。可用 `CREATORX_CODEX_PATH` 指定可执行文件，不复制账号凭据。
- 新建图会话：左侧主会话明确意图，通过 `creatorx_generate_image` 动态工具派发独立生图子会话；右侧显示任务与图片，结果回传主会话。修改引用实际原图、创建关联子任务并保留原版。主会话讨论、独立出图、独立改图及实际文件已在隔离目录真实验证；不能把这次联调泛化成任意多任务并发均已验收。
- 动态工具使用 app-server 的 `experimentalApi`。旧图会话保留原线程与直接执行方式，不自动迁移历史。
- 输入框下方可选三个内置 Skill：单张图文、系列图文、图片修改；也可选私人库 Skill 和提示词。完整 Skill 文件夹、参考图与每轮选择按任务快照保留。
- 素材库有提示词／Skills、分类新增与拖拽归类、搜索及本地 CLI；界面与 CLI 共用存储、文件锁和修订检查。不要重新清空数据、自动添加用途标签或恢复旧筛选。
- Command + [ 后退、Command + ] 前进已在安装版实测；空白输入框聚焦不阻挡导航，未发送内容、编辑及弹层打开时保护页面。鼠标侧键 button 3／4 已实现并测试映射，用户表示稍后自行真鼠测试，尚无实鼠通过反馈。
- macOS 隐藏左上角三个按钮，Command + W 暂时隐藏窗口，Command + Q 完全退出；重新打开恢复窗口。Windows 保留原生窗口按钮。
- OTA 约每小时安静检查，只在发现新版后显示按钮；用户确认后下载验签安装。正在创作或编辑时阻止安装。**此前 OTA 自动重启后的窗口唤起异常仍待排查**，不能以本地安装成功声称完整 OTA 验收通过。
- Windows 原生 Codex 与桌面交互尚未真机验收；Apple 公证、Windows 代码签名尚未配置。更新签名与平台代码签名分别核对。

## 只在本机的内容

Git 保存工程、设计、内置 Skill、公开图片与提示词、检查及发布脚本、已整理的进度；没有上传完整聊天原文，也没有上传下列私人运行状态：

- `~/Library/Application Support/cc.anjing.creatorx/library/`：截至核对时，41 条提示词、1 个完整私人 Skill、16 张参考图；原 Hub 未改动。
- 同级 `creations/`：1 条实际创作记录，没有运行中任务。不要公开正文、原图、素材快照或 Codex 会话内容。
- `desktop/.local/`：被 Git 忽略的联调结果、本机备份、验证记录。真实联调测试会话已归档，测试作品未混入个人作品列表。
- 本机 Codex 登录、独立 updater 签名私钥与其他凭据。只检查必要配置状态，不打印、写入公共文件或提交秘密；换机器不会自动继承这些配置和数据。

本机退出图标已参考 FinanceX 设置持久 Finder 自定义图标。系统解析与实际访达在启动、完全退出后均无底板；Dock 自动读取超时，实际 Dock 画面没有独立验收，用户随后反馈“我看是弄好了”。该操作添加 FinderInfo／资源分叉，会使严格代码签名检查报错；干净备份校验通过，`Contents/` 内容未变。**只用于本机定制，不进入发行包、不在应用启动时自动修改自身**。OTA 覆盖安装后需重设。脚本为 `desktop/scripts/macos-file-icon.swift`，详细限制和还原方式见图标验证文档；干净备份在 `desktop/.local/icon-fix-20261127/`。

## 已讨论但未落地

- 视频入口目前是 Codex 对话，没有完成固定的视频生成／渲染后端。Remotion 等仍是候选，不要把脚本或方案展示成已生成视频。
- 已调研官方 Seedance 2.5 `sd25-pe` Skill（0.1.1）：仅整理视频提示词，不提交视频生成任务；适合素材职责、角色对应和镜头／动作描述。官方包当时已校验摘要，只有 `SKILL.md`，未安装、未接入 CreatorX。
- 原版 `sd25-pe` 要求每个会话首次触发时自动升级自身；与任务固定快照有差异，建议由我们控制版本更新。该建议尚未实现，不要擅自全局安装或修改官方 Skill。
- 官方指南：https://docs.volcengine.com/docs/ark/seedance-2-5-prompt-guide?lang=zh 。后续重新核对当前官方版本，不依赖本文硬编码模型能力或安装地址。

## 工程与交付习惯

- 主要入口：`desktop/src/Workspace.tsx`（页面与素材库）、`Creation.tsx`（创作）、`Choice.tsx`（选择器）、`navigation.ts`、`App.tsx`（更新）、`src-tauri/src/codex.rs`（会话与子任务）、`library.rs`（存储与 CLI）、`lib.rs`（窗口）。
- 按改动运行合适检查：在 `desktop/` 执行 `npm run verify`、`npm test`，Rust 改动运行 `cargo test --locked --manifest-path src-tauri/Cargo.toml`。付费模型联调默认忽略，须显式选择隔离目录与测试案例，不能污染私人库。
- 本机只保留一个正式 CreatorX 安装入口。替换前检查未保存编辑和运行中任务；干净安装先验签，需要时再做本机图标定制，不能把定制后的严格签名错误误诊为资源或二进制被篡改。
- 每轮说明实际改动、验证范围和未验证项，更新精简设计与进度，同步 GitHub。源代码提交、CI、安装包、公开 Release、OTA 与本机实测分别核对。
- Git author／committer／GitHub 发布账号使用 `anjing-le`。远端主分支会被 Star Growth 工作流更新：提交／推送前 fetch 并检查差异，只合并已核对的变化，不强推。只有 main 推送成功后才能创建并推送新版本 tag；不移动已有 tag。
- 创建版本 tag `creatorx-vX.Y.Z` 会触发发布工作流，先生成完整 draft，验证全部平台包、签名和更新清单后再公开。不要为纯文档或本机图标定制无故增加二进制版本。
- 接续先阅读与核对，简短汇报已理解的现状和待验证项；已有明确需求就继续执行，尚无具体需求时等待用户指定下一项。不要自行重构、接入新模型或安装候选 Skill。
