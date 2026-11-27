# CreatorX 客户端架构（`0.1.2` Agent 原生 MCP × 零 CRUD 客户端）

## 1. 模块边界

- **`mcp.rs` ＋ `cli.rs`（唯一写入口）**：直接内置在 `creatorx` 二进制中，对外提供标准 MCP Stdio Server（`creatorx mcp`）与本地 JSON CLI。负责受控 Tag 表（`tags.json`）增删改查、提示词与 Skill 文件夹导入更新、样板图去重导入及带 `revision` 校验的删除。
- **`library.rs`（共享存储与锁）**：使用进程间文件锁 `.lock` 与乐观锁 `revision` 保护本地 `library/`。向桌面端仅暴露**纯只读查询命令**（`library_list`、`library_state_token`、`library_skill_files`、`library_open_skill_folder`），桌面界面无任何写资产接口。
- **`harness.rs` ＋ `adapters/`（创作治具与本地引擎适配）**：冻结每轮勾选资产快照 `snapshots/<seq>/`，通过独立子任务生成或修改图片并校验文件头、保护原版不被覆盖；`CodexAdapter` 已接通本机 `codex app-server`，`AntigravityAdapter` 预留本地槽位。
- **前端 `Showcase.tsx` ＆ `Creation.tsx`（零 CRUD 视图）**：无添加、编辑、删除、搜索框等传统管理表单；可见状态下自动比对 `stateToken`，外部 Agent 通过 MCP 更新资产后界面自动刷新。

## 2. MCP 工具清单（`creatorx mcp`）

| 工具名 | 职责 |
| --- | --- |
| `creatorx_tag_list` | 读取受控维护的全部 Tag 列表 |
| `creatorx_tag_add` | 登记一个新受控 Tag（给资产挂标签前必须先登记） |
| `creatorx_tag_rename` | 重命名受控 Tag 并自动同步所有关联资产 |
| `creatorx_tag_delete` | 删除受控 Tag 并自动从所有关联资产移除 |
| `creatorx_asset_list` | 按 `kind`（`prompt`｜`skill`）、`modality`（`text`｜`image`｜`video`）、`tag` 或关键词查询样板 |
| `creatorx_asset_get` | 读取单条资产完整正文、Skill 文件列表与当前 `revision` |
| `creatorx_image_import` | 导入本机 PNG／JPEG／WebP 样板图或参考图（SHA-256 去重），返回图片标识 |
| `creatorx_prompt_save` | 新建或带 `expectedRevision` 更新提示词样板 |
| `creatorx_skill_import` | 新建或带 `expectedRevision` 整夹导入真实 Skill 文件夹（含 `SKILL.md`，不执行脚本） |
| `creatorx_skill_update` | 更新已有 Skill 的元数据、标签、样板图或 `SKILL.md`（保留其余资源文件） |
| `creatorx_asset_delete` | 带 `expectedRevision` 安全删除单条资产 |
