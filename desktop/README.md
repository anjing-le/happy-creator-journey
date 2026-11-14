# CreatorX

当前版本 **0.0.4**：首页上排是文、图片、视频三张插画，下排是整行素材库插画色块；悬停或键盘聚焦显示用途说明。先做素材库，创作执行随后逐步接入。

素材库区分角色 IP 与创作场景，默认展示参考图／样图套组，支持展开、编辑提示词或 Skill 文档，以及导入图片与 SKILL.md。Skill 当前仅保存文档快照，尚未导入完整包依赖或执行。个人素材保存在本机，不随公共安装包分发。

```sh
cd desktop
npm ci
npm run desktop
```

`npm run verify` 检查前端和发布配置，`npm test` 检查更新策略；`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` 验证素材保存重读、修订冲突与图片原样导入。

macOS 数据目录为 `~/Library/Application Support/cc.anjing.creatorx/library/`，包含 `entries/` 记录与 `images/` 原图。Windows 使用系统应用数据目录。编辑携带修订号，冲突时拒绝覆盖；移除图片引用不删除原始文件。

发布使用独立的 CreatorX 更新签名密钥；私钥保存在仓库之外，不复用 macos-x 密钥。GitHub Releases 提供签名安装包与 latest.json。后台仅在启动且距上次成功检查超过一小时时检查；有新版只点亮入口，用户点击“更新并重启”才下载、验签和安装。开发热重载不代表 OTA。

更新签名与操作系统代码签名是两件事。当前 macOS 包使用 ad hoc 签名，未做 Apple 公证；Windows 未配置代码签名证书。正式对外分发前需完善相应身份与信任链。

光标使用本仓库原创的简洁箭头与手形，保留原生文字／缩放光标；不打包 EnglishX／FinanceX 仅供个人预览的第三方光标素材。

## 当前验证

2026-11-14：前端、更新策略与素材保存测试通过。[双平台检查](https://github.com/anjing-le/happy-creator-journey/actions/runs/43599383341)及[三平台打包与签名验证](https://github.com/anjing-le/happy-creator-journey/actions/runs/43599416660)通过，安装包见 [0.0.4 Release](https://github.com/anjing-le/happy-creator-journey/releases/tag/creatorx-v0.0.4)。Windows 真机安装与交互尚未验收。

本机已通过 OTA 从 0.0.2 安装到 0.0.3，磁盘版本已核对。自动重启后窗口连接异常，结束进程后干净启动正常；重启唤窗仍待排查，不算完整 OTA 体验验收通过。原生客户端已核对首页四入口、安静图片与提示词展开、角色编辑保存、场景两张样图切换及 Skill 文档展开；保存后本地记录修订号已更新。

本机个人库先放入安静 IP 和金融知识图解样例，素材取自已有本地 Skill，保留原图与来源；没有把个人素材打进公共安装包。后续对着 UI 打磨，再确定完整 Skill 包、角色关联与创作执行。

0.0.4：首页四张插画由内置 imagegen 生成，原图位于 `public/entrances/`，完整提示词见 [入口插画提示词](entrance-prompts.md)。原生客户端已核对新版首页，浏览器预览已核对悬停／键盘聚焦说明。通过 OTA 安装后仍需干净启动才能重新连接窗口，重启唤窗问题继续保留；升级前后本机素材库五个文件的 SHA-256 全部一致。
