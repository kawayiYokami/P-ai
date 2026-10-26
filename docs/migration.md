# 迁移策略

> 更新于 2026-10-26

## 核心原则：迁移内聚，生产只保留最新

**旧数据读写必须封装在迁移模块内部，生产环境读写路径只认最新格式。**

- 每个存储的迁移逻辑自包含：旧格式的路径、读取、转换、写入全部收敛在迁移模块内，迁移完成后业务代码不再感知旧格式。
- 禁止拿生产读写路径去读写旧格式；禁止在业务代码里残留旧格式分支。
- 迁移必须幂等：重复调用直接跳过（已存在目标库/已记录版本号即跳过）。

以 `state` 迁移为范例（[state/migration.rs](../src-tauri/src/features/state/migration.rs)）：

> 旧 JSON（runtime_state.json / window_layouts.json / git_panel_repo_history.json）只在本模块被读取。迁移完成后业务代码不再感知旧格式。

## 各存储迁移

### 1. state：旧 JSON → state/state.sqlite

- 模块：[state/migration.rs](../src-tauri/src/features/state/migration.rs)
- 旧格式：`state/runtime_state.json`、`state/window_layouts.json`、`state/git_panel_repo_history.json`
- 流程：检测 `state.sqlite` 是否存在 → 存在则读版本号跳过；不存在则读旧 JSON → 写入 SQLite → 记录版本号
- 迁移版本：V4
- 迁移后业务状态读写统一走 `state_db_upsert_kv` / `state_db_get_kv`（`state/state_db.rs`），不再接触旧 JSON

### 2. chat message_store：V1 → V4

- 模块：[message_store/migration.rs](../src-tauri/src/features/chat/message_store/migration.rs) + [migration_v4.rs](../src-tauri/src/features/chat/message_store/migration_v4.rs)
- 演进：V1 单 conversation JSON → V2 manifest/meta/messages.jsonl/index/blocks → V3 明文块 + SQLite locator → V4 zstd 压缩块
- 旧格式读取封装为迁移专用函数：`migration_read_v1_conversation`、`migration_v1_to_v2_conversation`、`migration_v3_to_v4` 等，全部带 `migration_` 前缀，不进入生产读写路径
- 生产读写统一走 V4：`chat/chat_metadata.sqlite` 的 `message_locator` 定位 + `blocks/*.jsonl.zstd` 压缩块
- 用户侧迁移确认入口：[system/commands/config_and_persona/message_store_migration.rs](../src-tauri/src/features/system/commands/config_and_persona/message_store_migration.rs)

### 3. memory：legacy 位置迁移 + 库内迁移

- 模块：[memory/store/db.rs](../src-tauri/src/features/memory/store/db.rs)
- legacy 位置迁移：`memory_store_open` 打开时若目标 `memory/memory_store.db` 不存在，检查旧位置 `data_path.parent()/memory_store.db`，存在则 rename（失败回退 copy+remove）到目标位置
- 库内迁移：`memory_no` 回填、`owner_agent_id` 列补加、FTS 重建/回填、`profile_memory_link` 表初始化

### 4. task：列迁移

- 模块：[task/migration.rs](../src-tauri/src/features/task/migration.rs)
- 方式：`task_store_apply_migrations` 事务内按需 `RENAME COLUMN` / `ADD COLUMN`（`task_store_rename_column_if_needed` / `task_store_add_column_if_missing`）
- 内容：列重命名/新增、空 `conversation_id` → system、legacy trigger → cron 归一、一次性已触发任务 → completed

### 5. delegate：列迁移

- 模块：[delegate/store.rs](../src-tauri/src/features/delegate/store.rs)
- 内容：`delegate_store_migrate_why_goal_todo`（旧列 background/instruction/question/specific_goal/deliverable_requirement/focus → why/goal/todo 重建）、`delegate_store_migrate_snapshot_columns`（snapshot 系列列补加）

### 6. config：归一化（非存储迁移）

- 模块：[config/storage_and_stt.rs](../src-tauri/src/features/config/storage_and_stt.rs) `normalize_app_config`
- 在加载后、保存前执行，属于配置归一化而非数据迁移：
  1. api_configs 与 api_providers 均为空 → 回退默认配置
  2. legacy api_configs → providers 结构迁移（`migrate_legacy_api_configs_into_providers`）
  3. 从 providers 展开 api_configs（`expand_api_configs_from_providers`）
  4. selected/assistant_department/vision/stt 的 api_config_id 重映射（`remap_legacy_api_config_id_to_endpoint`）
  5. 无效或空 id 清空；不支持的格式回退；语言/字体/端口/更新方式等字段归一化

## 已废弃形态

以下内容不再存在，任何新代码不得为其编写迁移：

- `app_data.json` 全量 AppData（agents/conversations/archived 一体 JSON）——该文件只是路径锚点占位，已改名为 `config_mark`，从不读写内容
- 记忆库在根目录的 legacy 形态、WAL checkpoint 手工迁移等为「从未发布形态」编写的迁移逻辑
