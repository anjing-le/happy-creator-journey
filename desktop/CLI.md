# CreatorX CLI

CLI 与桌面界面共用同一份本地样板间存储；无需启动窗口，也不需要联网。支持 macOS／Windows 同一个应用可执行文件。

```sh
creatorx schema
creatorx library tags
creatorx library list --kind prompt --modality image --tag 安静
creatorx library get <id>
```

macOS 入口为 `~/Applications/CreatorX.app/Contents/MacOS/creatorx`（本机脚本 `~/.local/bin/creatorx`）。所有命令输出 JSON：成功为 `{"ok":true,"data":...}`，失败为 `{"ok":false,"error":...}`（退出码 `1`）。

## 1. 保存与更新提示词样板

```json
{
  "kind": "prompt",
  "modality": "image",
  "name": "安静粗蜡笔角色设定",
  "prompt": "蛋黄黄乱发、黑卫衣、圆脸点眼、微笑腮红，暖米白纸纹粗蜡笔线条……",
  "sample": "安静站在白板旁讲解技术概念的手绘样板",
  "tags": ["安静", "粗蜡笔"],
  "images": []
}
```

```sh
creatorx library save --file entry.json
creatorx library save --stdin < entry.json
creatorx library save --file edited-entry.json --if-revision 1
creatorx library delete <id> --if-revision 2
```

更新与删除必须携带当前 `--if-revision`，旧修订会被拒绝覆盖。

## 2. 导入细粒度 Skill 文件夹与样板图片

```sh
creatorx library import-skill --folder /absolute/path/my-skill --modality image --tags 安静,单图
creatorx library skill-files <id>
creatorx library import-image --file /absolute/path/sample.png
```

Skill 文件夹根目录必须包含 UTF-8 `SKILL.md`，其余参考资料与脚本按原目录结构复制，导入绝不执行脚本。`CREATORX_LIBRARY_DIR` 可在测试时指定隔离目录。
