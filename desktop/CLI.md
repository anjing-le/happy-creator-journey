# CreatorX CLI

CLI 与桌面界面共用同一份本地资产与样板间存储；无需启动窗口，也不需要联网。支持 macOS／Windows 的同一个应用可执行文件。

本机安装后：

```sh
creatorx schema
creatorx library ip
creatorx library list --kind skill --deliverable cards
creatorx library list --kind prompt --maturity showcase --track 计算机 --search 蜡笔
creatorx library get <id>
```

macOS 原始入口为 `~/Applications/CreatorX.app/Contents/MacOS/creatorx`（本机提供 `~/.local/bin/creatorx` 启动脚本）。Windows 使用安装目录的 `CreatorX.exe`；在 PowerShell 中将输出接到 `| Out-String` 或 `| ConvertFrom-Json`。

## 1. IP 底座读写

所有结果使用 JSON：成功为 `{"ok":true,"data":...}`，失败为 `{"ok":false,"error":...}`，失败退出码为 `1`。

```json
{
  "name": "安静",
  "visualPrompt": "蛋黄黄乱发、黑卫衣、圆脸点眼、粗蜡笔纸纹",
  "images": [],
  "voicePrompt": "计算机从业者视角，克制直白，先说结论",
  "voiceSample": "",
  "tracks": [
    { "name": "计算机", "positioning": "AI 工程与架构拆解" },
    { "name": "金融", "positioning": "基础金融常识与风险认知" }
  ],
  "revision": 0
}
```

```sh
creatorx library ip
creatorx library save-ip --file ip.json --if-revision 0
```

## 2. 样板间条目读写与删除

新风格提示词样板的最小 JSON：

```json
{
  "kind": "prompt",
  "name": "粗蜡笔架构拆解风",
  "deliverable": "cards",
  "category": "视觉画风",
  "track": "计算机",
  "maturity": "showcase",
  "sampleInput": "解释什么是 Creation Harness",
  "sampleOutput": "暖米白纸纹手绘分层架构卡",
  "prompt": "使用安静 IP 角色与粗颗粒蜡笔线条绘制架构对比……"
}
```

```sh
creatorx library save --file entry.json
creatorx library save --stdin < entry.json
creatorx library save --file edited-entry.json --if-revision 1
creatorx library delete <id> --if-revision 2
creatorx library add-category --name 视觉画风
```

界面与 CLI 共用进程间文件锁与 `revision` 修订冲突检查；旧修订写入或删除均会被拒绝。

## 3. 制式样板 Skill 文件夹导入

```sh
creatorx library import-skill --folder /absolute/path/my-skill --deliverable cards --track 金融 --maturity showcase
creatorx library skill-files <id>
creatorx library import-image --file /absolute/path/showcase.png
```

Skill 根目录必须包含 UTF-8 `SKILL.md`；其余脚本、参考资料和图片按原目录结构复制，导入过程绝不执行脚本。文件夹最多 512 个文件、总计 64 MB，单文件不超过 16 MB。`CREATORX_LIBRARY_DIR` 仅用于测试或隔离目录覆盖，不会污染个人库。
