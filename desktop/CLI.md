# CreatorX MCP ＆ CLI 说明

CreatorX 界面不提供任何传统的增删改查表单；所有 Tag、提示词、Skill 文件夹与样板图片均由 Agent 通过内置 **MCP Server** 或 **本地 CLI** 维护。无论桌面窗口是否打开均可直接调用，桌面窗口打开时会自动热同步最新内容。

## 1. 接入任意 Agent 的 MCP 配置（推荐）

在 Antigravity（`~/.gemini/config/mcp_config.json`）、Codex 或 Claude Code 中配置：

```json
{
  "mcpServers": {
    "creatorx": {
      "command": "/Users/lvxianghe/Applications/CreatorX.app/Contents/MacOS/creatorx",
      "args": ["mcp"]
    }
  }
}
```

Windows 将 `command` 指向安装目录的 `CreatorX.exe` 即可。内置 MCP 服务同时兼容换行分隔 JSON-RPC 2.0 与 `Content-Length` 帧格式。

## 2. 本地 CLI 命令

```sh
creatorx schema

# 1) 逐个维护受控 Tag
creatorx tag list
creatorx tag add --name 安静
creatorx tag rename --from 旧名 --to 新名
creatorx tag delete --name 旧名

# 2) 导入样板图并保存提示词（kind 仅 prompt|skill，modality 仅 text|image|video）
creatorx library import-image --file /absolute/path/sample.png
creatorx library save --file entry.json
creatorx library save --file edited.json --if-revision 1

# 3) 导入真实 Skill 文件夹与查询删除
creatorx library import-skill --folder /absolute/path/my-skill --modality image --tags 安静
creatorx library list --kind prompt --modality image --tag 安静
creatorx library get <id>
creatorx library delete <id> --if-revision 2
```
