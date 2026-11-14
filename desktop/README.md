# CreatorX

当前 **0.0.7**：素材库只分提示词和 Skills。提示词支持分类树、搜索、用途筛选、复制、编辑和参考图；Skill 按真实文件夹保存，可整夹导入、查看文件、编辑 SKILL.md 或打开本地目录。目前不执行 Skill，也未接入 AI 创作。

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

本轮按用户要求，旧的两份本机样例已移入废纸篓。随后从私人 Hub 迁入 41 条提示词、4 个分类及 16 张参考图，逐项核对正文、名称、来源时间与图片字节；重复迁入无新增。用途标签根据标题整理，可修改，不代表效果验证。原 Hub 数据未改动。

首页维持暖米白、无可见文字的四个插画入口；素材维护界面使用清晰文字。插画原图位于 `public/entrances/`，提示词见 [入口插画](entrance-prompts.md)。

OTA 保持独立签名、用户确认后下载验签；更新签名不等于 Apple 公证或 Windows 代码签名。此前 OTA 自动重启后的窗口唤起仍待排查，不能以本地构建安装代替完整 OTA 验收。图标的透明检查与各系统位置的视觉状态见 [图标验证](icon-validation.md)，未核对位置不宣称完成。

本机安装版已核对 41 条提示词、4 个分类和参考图，中文全文搜索、用途筛选、一键复制与 Skill 文件夹导入预览通过；临时预览已取消，Skills 仍为 0。`creatorx schema` 及安装版 CLI 读写／修订保护已验证。

[0.0.7 Release](https://github.com/anjing-le/happy-creator-journey/releases/tag/creatorx-v0.0.7)、[双平台检查](https://github.com/anjing-le/happy-creator-journey/actions/runs/43611197800)、[三平台构建、Windows 发布版 CLI 与签名验证](https://github.com/anjing-le/happy-creator-journey/actions/runs/43612700017)均已完成。Windows 桌面交互与图标显示仍未作真机视觉验收。
