# CreatorX

当前仅为空白客户端与 OTA 更新入口，创作功能随后逐步设计。

```sh
cd desktop
npm ci
npm run desktop
```

`npm run verify` 检查前端和发布配置，`npm test` 检查更新策略。

发布使用独立的 CreatorX 更新签名密钥；私钥保存在仓库之外，不复用 macos-x 密钥。GitHub Releases 提供签名安装包与 latest.json。后台仅在启动且距上次成功检查超过一小时时检查；有新版只点亮入口，用户点击“更新并重启”才下载、验签和安装。开发热重载不代表 OTA。

更新签名与操作系统代码签名是两件事。当前自签应用包不宣称经过 Apple 公证或 Windows 代码签名。正式对外分发前需完善相应身份与信任链。

光标使用本仓库原创的简洁箭头与手形，保留原生文字／缩放光标；不打包 EnglishX／FinanceX 仅供个人预览的第三方光标素材。
