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
