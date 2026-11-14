# CreatorX CLI

CLI 和桌面界面使用同一份本地素材库；无需启动窗口，也不需要联网。当前支持 macOS／Windows 的同一个应用可执行文件。

本机安装后：

```sh
creatorx schema
creatorx library list --kind prompt --search 蜡笔
creatorx library categories
creatorx library get <id>
```

macOS 原始入口为 `~/Applications/CreatorX.app/Contents/MacOS/creatorx`。本机提供 `~/.local/bin/creatorx` 启动脚本。Windows 使用安装目录的 `CreatorX.exe`；在 PowerShell 中将输出接到 `| Out-String` 或 `| ConvertFrom-Json`，等待 CLI 结束并接收 JSON。

## 写入与修改

所有结果使用 JSON：成功为 `{"ok":true,"data":...}`，失败为 `{"ok":false,"error":...}`，失败退出码为 1。正文通过 JSON 文件或标准输入传入。

新提示词的最小 JSON：

```json
{"kind":"prompt","name":"文章整理","prompt":"将以下材料整理为清晰的文章大纲：\n[材料]","category":"","tags":["写作"]}
```

```sh
creatorx library save --file entry.json
creatorx library save --stdin < entry.json
creatorx library add-category --name 写作
```

更新前先 `library get <id>`，取得 `data` 中的完整记录，修改所需字段，再传给 `save --file`，同时提供当前修订号：

```sh
creatorx library save --file edited-entry.json --if-revision 3
```

界面与 CLI 共用进程间文件锁和修订检查；旧修订会被拒绝覆盖。切回素材库时自动重新读取，也可以点击“刷新”。分类、标签、备注、来源和原始时间随记录保存；提示词正文不被自动改写。分类为空表示未分类；界面可新增分类，并将素材拖入目标分类。

`creatorx library clear-categories` 会清空所有分类及素材的分类归属，保留素材、参考图、Skill 文件和原始时间，逐条增加修订号。它是明确的本地数据修改，执行前应按需要备份。

## Skill 文件夹

```sh
creatorx library import-skill --folder /absolute/path/my-skill
creatorx library skill-files <id>
creatorx library import-image --file /absolute/path/reference.png
```

Skill 根目录必须有 UTF-8 `SKILL.md`。其余脚本、参考资料和图片按原目录结构复制；不执行脚本。文件夹最多 512 个文件、64 MB，单文件不超过 16 MB。不导入符号链接和跨平台不兼容的文件名。当前保留内容与目录结构，不将本机依赖环境打包，也不承诺保留 Unix 可执行权限。

界面可以整夹导入、编辑 `SKILL.md`、查看文件列表和打开本地文件夹。直接在文件夹修改 `SKILL.md` 后，旧界面修订也会拒绝覆盖该文档。`CREATORX_LIBRARY_DIR` 仅用于让 CLI 指向隔离目录，测试不会写入个人库。

## 从 Hub 迁入

`scripts/import-hub.py` 接收私有 API 导出目录（`items.json` 和 `categories.json`），通过 CLI 导入提示词、分类和参考图。保留正文、名称、来源及原始时间；以 Hub ID 生成稳定标识。重复执行不重复导入，正文、名称、分类或参考图与源导出不同时报告冲突，不覆盖；本地标签与备注保留。

```sh
python3 desktop/scripts/import-hub.py --client /path/to/creatorx --snapshot /private/hub-export
```

源导出、图片和迁移核对报告留在私有目录，不进入公共仓库；迁移不会删除或修改原 Hub 数据。
