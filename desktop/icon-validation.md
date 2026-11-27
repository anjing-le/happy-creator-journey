# 透明主体应用图标

来源：用户提供的安静头像。原图、imagegen 透明蒙版、最终透明母图及提示词保存在 `../assets/creatorx-icon/`。

- imagegen 提取外围透明轮廓；`scripts/export-icon.swift` 将原图内部 RGB 恢复到蒙版内，边缘保留透明抗锯齿，避免混入原背景。主体内部原有浅色蜡笔纹理保留；纸张底板、黑色角部及外围绿色背景去除。
- 透明 PNG 经 Tauri icon 导出桌面 PNG、ICNS、ICO。仅复制桌面资源，不分发会铺底的移动端资源。
- macOS 启动时用 AppKit `NSApplication.applicationIconImage` 直接加载透明 PNG。该接口只控制运行中的 Dock 图标；退出后的入口与访达仍由安装包 ICNS 和系统渲染决定。[Apple 文档](https://developer.apple.com/documentation/appkit/nsapplication/applicationiconimage)

## 检查方式

```sh
uv run --with pillow==13.0.0 python desktop/scripts/verify-icon-alpha.py
uv run --with pillow==13.0.0 python desktop/scripts/verify-icon-alpha.py --bundle /Users/lvxianghe/Applications/CreatorX.app
```

脚本解码全部 PNG、ICO 各尺寸及 ICNS 各尺寸，检查真实 Alpha=0 的外围；安装包检查还比对 ICNS 原字节，并确认运行时 PNG 原字节存在于实际可执行文件中。图像透明检查不能代替系统位置的视觉验收。

## 0.0.6 验证状态

| 范围 | 状态 |
| --- | --- |
| 透明母图与多尺寸资源 | 已通过 Alpha 检查，主体内部 840591 个像素使用原图 RGB。 |
| Rust 编译与原素材保存测试 | 已通过。 |
| 安装包与已安装应用 | 本机 0.0.7 已安装；ICNS 与源码一致，运行时 PNG 原字节存在于可执行文件，Alpha 检查通过。 |
| macOS 运行中的 Dock | 自动化读取 Dock 超时，待实际显示核对。 |
| 退出后的启动入口与访达 | 待实际显示核对。 |
| Windows 实机 | 当前无 Windows 桌面可用；ICO 透明度及跨平台构建与实际系统显示分别记录。 |

未验证位置不宣称无底板；系统自动附加的显示样式也不能靠源 PNG 的 Alpha 检查来判断。

## 2026-11-27 本机退出图标修复

本机 macOS 27.0.1 在 CreatorX 退出后，会给透明 ICNS 合成灰色圆角底板。已从 `NSWorkspace.icon(forFile:)` 读取系统解析结果复现，原 PNG／ICNS 的透明通道没有丢失。

参考 FinanceX 当前的 `scripts/package-preview.py` 与 `scripts/icon-raster.swift`，给已安装 CreatorX 设置 `NSWorkspace.setIcon(_:forFile:options:)` 自定义文件图标。运行中的 Dock 继续沿用现有透明 PNG；不改主体图片、不改应用内容文件。

```sh
mkdir -p desktop/.local
swiftc desktop/scripts/macos-file-icon.swift -o desktop/.local/macos-file-icon
# 先备份干净安装并验证其代码签名，然后只对本机安装路径定制。
desktop/.local/macos-file-icon apply "$HOME/Applications/CreatorX.app" desktop/src-tauri/icons/icon.png
desktop/.local/macos-file-icon inspect "$HOME/Applications/CreatorX.app" desktop/.local/system-icon.png
```

| 检查位置 | 实际结果 |
| --- | --- |
| 退出后的系统解析图标 | 已导出并检查，灰色底板消失，外围保留透明。 |
| 实际访达图标 | 已打开应用目录查看；启动并 Command + Q 退出后再次查看，仍无底板。 |
| 实际 Dock 截图 | Dock 自动读取超时，尚未完成该位置的视觉验收。系统解析结果不能冒充 Dock 截图。 |
| 客户端运行 | 本机已正常打开并完全退出。 |
| 图像资源与安装内容 | 透明资源检查通过；与干净备份逐文件比较，`Contents/` 内容未改变。 |
| 签名 | 干净备份严格校验通过；定制后由于 FinderInfo／资源分叉，严格校验会报错，与 FinanceX 本机定制方式相同。 |

**适用范围**：只用于本机定制。不得把图标元数据并入发行包、OTA 包或公证产物，不在应用启动时自动修改自身。OTA 替换整个 `.app` 后需要重新定制；本次未发布新的二进制版本，仍为 0.0.17。

本机干净安装备份、前后系统图标、Alpha 检查和验证记录保存在忽略目录 `desktop/.local/icon-fix-20261127/`，没有公开发布。`restore APP` 可清除自定义图标；恢复严格签名应从干净安装包或保留的完整备份恢复应用，再校验。个人素材与创作任务在独立应用数据目录，不受图标定制影响。
