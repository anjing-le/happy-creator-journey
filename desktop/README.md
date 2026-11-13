# CreatorX

当前仅为空白客户端与 OTA 更新入口，创作功能随后逐步设计。

```sh
cd desktop
npm ci
npm run desktop
```

`npm run verify` 检查前端和发布配置，`npm test` 检查更新策略。

发布使用独立的 CreatorX 更新签名密钥；私钥保存在仓库之外，不复用 macos-x 密钥。GitHub Releases 提供签名安装包与 latest.json。后台仅在启动且距上次成功检查超过一小时时检查；有新版只点亮入口，用户点击“更新并重启”才下载、验签和安装。开发热重载不代表 OTA。

更新签名与操作系统代码签名是两件事。当前 macOS 包使用 ad hoc 签名，未做 Apple 公证；Windows 未配置代码签名证书。正式对外分发前需完善相应身份与信任链。

光标使用本仓库原创的简洁箭头与手形，保留原生文字／缩放光标；不打包 EnglishX／FinanceX 仅供个人预览的第三方光标素材。

## 当前验证

2026-11-13：本机已从 0.0.1 通过线上更新入口下载、验签、安装并重启至 0.0.2；升级后窗口与版本显示正常，再次检查显示“已是最新版本”。正常包验签通过，损坏包及被篡改的签名受信注释被拒绝。

[双平台检查](https://github.com/anjing-le/happy-creator-journey/actions/runs/43442225254)和[三平台打包与签名验证](https://github.com/anjing-le/happy-creator-journey/actions/runs/43442233737)通过。Apple Silicon、Intel Mac、Windows x64 安装包已在 [0.0.2 Release](https://github.com/anjing-le/happy-creator-journey/releases/tag/creatorx-v0.0.2) 提供；Windows 真机安装与交互尚未验收。
