# 数据存储架构

> 更新于 2026-08-23
> 本文档描述当前版本（HEAD=d48ed949）的真实存储形态。旧版文档描述的 `app_data.json` AppData 全量 JSON（agents/conversations/archived 一体）形态已不存在，仅残留于迁移读取路径。

## 目录布局

应用根目录 `app_root` 由 `app_root_from_data_path(data_path)` 以 `config_mark` 锚点反推（`runtime_state.rs`）：

- `data_path = config_dir/config_mark`，`config_mark` 是空占位锚点文件（从不读写内容）
- 若 `config_mark` 父目录名恰为 `config`（便携模式），取父目录的父目录为 `app_root`
- 否则（标准安装），取 `config_mark` 的父目录本身为 `app_root`

两种运行时形态（`runtime_state.rs`）：

| 模式 | 触发条件 | app_root |
|---|---|---|
| 便携 | exe 同级存在 `PORTABLE` 标记 | `exe_dir/data/` |
| 标准 | 默认 | `ProjectDirs::from("ai","easycall","p-ai").config_dir()` |

### 布局总表（相对 app_root）

| 路径 | 形态 | 说明 |
|---|---|---|
| `app_config.toml` | TOML | 顶层配置（热键、供应商、工具开关等） |
| `config_mark` | 占位文件 | 路径锚点，从不读写内容 |
| `config/agents.json` | JSON | 代理配置 |
| `state/state.sqlite` | SQLite | 全局状态总库（V4，runtime_state 表为现役落点） |
| `state/runtime_state.json` 等旧 JSON | JSON | 仅迁移读取，V4 起不再写入 |
| `chat/chat_metadata.sqlite` | SQLite | 聊天元数据、消息定位索引、用量统计 |
| `chat/conversations/<id>/` | 分片目录 | 会话正文（见下文） |
| `backups/` | 目录 | 迁移/导出备份，按 `backups/<时间戳>/` 组织 |
| `memory/memory_store.db` | SQLite | 记忆库 |
| `task/task_store.db` | SQLite | 任务库 |
| `delegate/delegate_store.db` | SQLite | 委托库 |
| `avatars/` | 目录 | 代理头像（webp） |
| `media/` | 目录 | 媒体文件（`<sha256>.<ext>`） |
| `exports/` | 目录 | 导出产物 |
| `llm-workspace/` | 目录 | Shell / Skills / MCP 工作区 |
| `llm-workspace/downloads/` | 目录 | 下载落点 |

## 1. app_config.toml

顶层字段（`types_config.rs`）：

- 标量：`hotkey`、`uiLanguage`、`uiFont`、`codeFont`、`uiSizeScale`（别名 `uiSizePreset`）、`webAccessPort`、`webAccessEnabled`、`webAccessPassword`、`githubUpdateMethod`、`skippedGithubUpdateVersion`、`recordHotkey`、`recordBackgroundWakeEnabled`、`minRecordSeconds`、`maxRecordSeconds`、`toolMaxIterations`、`llmRoundLogCapacity`、`messageNotificationEnabled`、`messageNotificationSoundEnabled`、`desktopOperationNoticeEnabled`、`desktopOperateEnabled`、`selectedApiConfigId`、`expertApiConfigId`（别名 `chatApiConfigId`；旧键 `assistantDepartmentApiConfigId`）、`visionApiConfigId`、`toolReviewApiConfigId`、`sttApiConfigId`、`imageGenerationModelId`、`sttAutoSend`、`terminalShellKind`、`simpleSetupMode`
- 数组：`shellWorkspaces[]`、`mcpServers[]`、`remoteImChannels[]`、`providerNonStreamBaseUrls[]`、`apiProviders[]`、`imageProviders[]`、`apiConfigs[]`

> 旧配置里的 `departments[]` 已不是本应用的字段，只在 V5 数据迁移里被读取一次（`agent_org_migration.rs`）。

## 2. config/agents.json

`AgentsFile { agents: [AgentProfile] }`（`app_data_layout.rs`）。

`AgentProfile` 字段（`types_chat.rs`）：`id`、`name`、`systemPrompt`、`tools[]`（ApiToolConfig）、`createdAt`、`updatedAt`、`avatarPath`、`avatarUpdatedAt`、`isBuiltInUser`、`isBuiltInSystem`、`privateMemoryEnabled`、`memoryRecallMode`、`source`、`scope`。

读写入口：`read_agents_shard` / `write_agents_shard`（`app_data_layout.rs`）。

## 3. state/state.sqlite

PRAGMA：busy_timeout=10000、WAL、synchronous=NORMAL、foreign_keys=ON（`state_db.rs`）。

| 表 | 关键列 | 说明 |
|---|---|---|
| `state_migration` | `version` PK, `migrated_at` | 迁移版本 |
| `runtime_state` | `key` PK, `value` | 全局状态 k/v，runtime_state.json 的现役落点 |
| `image_text_cache` | `hash`, `model_api_id`, `media_type`, `description`, `text`, `updated_at` | PK(hash, model_api_id, media_type, description)；超限按 updated_at 淘汰 |
| `pdf_text_cache` | `file_hash` PK, `file_path`, `file_name`, `extracted_text`, `total_pages`, `extracted_pages`, `is_truncated`, `conversation_ids`, `created_at`, `updated_at` | PDF 文本缓存 |
| `pdf_image_cache` | `file_hash` PK, `file_path`, `file_name`, `total_pages`, `rendered_pages`, `dpi`, `images_json`, `conversation_ids`, `created_at`, `updated_at` | PDF 渲染缓存 |
| `remote_im_contacts` | `id` PK, `channel_id`, `platform`, `remote_contact_type`, `remote_contact_id`, `config_json` | 远程 IM 联系人 |
| `remote_im_group_members` | `contact_id`, `user_id`, `nickname`, `card`, `display_name`, `updated_at` | PK(contact_id, user_id) |
| `remote_im_contact_checkpoints` | `contact_id` PK, `checkpoint_json` | 联系人断点 |
| `window_layouts` | `window_label` PK, `width`/`height`/`x`/`y`, `maximized` | 窗口布局 |
| `git_repo_history` | `repo_key` PK, `history_json` | git 面板历史 |

V4 迁移把 `runtime_state.json` / `window_layouts.json` / `git_panel_repo_history.json` 一次性迁入 SQLite，迁移后业务不再读旧 JSON（`state/migration.rs`）。

## 4. chat/chat_metadata.sqlite

PRAGMA：WAL、synchronous=NORMAL、foreign_keys=ON、busy_timeout=10000（`sqlite.rs`）。

| 表 | 关键列 | 说明 |
|---|---|---|
| `chat_storage_migrations` | `migration_key` PK, `state`, `updated_at` | 迁移状态 |
| `conversation_metadata` | `conversation_id` PK, `metadata_json`, `storage_revision` DEFAULT 0, `updated_at` | meta.json 全量内容的库内镜像 |
| `conversation_blocks` | `conversation_id`, `block_id`, `block_file`, `byte_len`, `message_count` | PK(conversation_id, block_id)；FK CASCADE |
| `message_locator` | `conversation_id`, `sequence`, `message_id`, `block_id`, `byte_offset`, `byte_len`, `compaction_kind`, `role`, `created_at` | PK(conversation_id, sequence)；UNIQUE(conversation_id, message_id)；正文定位权威 |
| `active_plan_records` | `conversation_id`, `plan_id`, `record_json` | PK(conversation_id, plan_id) |
| `storage_operations` | `operation_id` PK, `conversation_id`, `before_revision`, `after_revision`, `state`, `detail_json`, `created_at`, `committed_at` | 追加/替换/截断/splice 原子接口追踪 |
| `usage_trail` | `bucket`, `conversation_id`, `agent_id`, `department_id`（历史列，已不再读写）, `conversation_kind`, `api_config_id`, `provider_key`, `provider_label`, `model_name`, `input_tokens`, `output_tokens`, `total_tokens`, `cache_read_tokens`, `cache_write_tokens`, `reasoning_tokens`, `updated_at` | PK(bucket, conversation_id, provider_key, model_name)；用量统计 |

- `role` / `created_at` 两列是启动期 ALTER 补加（`sqlite.rs`）
- `bucket` 为本地时区小时桶 `YYYY-MM-DDTHH:00:00`，按凌晨 4 点分界（`usage_trail.rs`）

## 5. chat/conversations/<conversation_id>/

分片路径构造（`paths.rs`），文件常量（`paths.rs`）：

| 文件/目录 | 内容 |
|---|---|
| `manifest.json` | `MessageStoreManifest`（`manifest.rs`）：`version`(=1)、`messageStoreKind`(conversationJson\|jsonlSnapshot)、`migrationState`(none\|building\|ready\|failed\|rollback)、`sourceConversationRevision`、`sourceMessageCount`、`lastMessageId`、`messagesJsonlBytes`、`messagesIndexRevision`、`updatedAt` |
| `meta.json` | `ConversationPersistMeta`（`meta.rs`）：`metaSchemaVersion`、`id`、`title`、`agentId`、`boundConversationId`、`parentConversationId`、`childConversationIds`、`forkMessageCursor`、`unreadCount`、`conversationKind`、`rootConversationId`、`delegateId`、`createdAt`、`updatedAt`、`lastUserAt`、`lastAssistantAt`、`status`、`userProfileSnapshot`、`shellWorkspacePath`、`shellWorkspaces`、`shellAutonomousMode`、`shellWorkMode`、`archivedAt`、`currentTodos`、`memoryRecallTable`、`planModeEnabled`、`preferredApiConfigId`、`isDraft`、`autoPushRemoteContactId`、`cumulativeUsage`、`activeGoal`、`fastRequestTurns`、`lastMessageId`、`lastMessageAt`、`messageCount`、`bodyMessageCount`、`bodyTextLength`、`hasAssistantReply`、`hasContextCompactionMessage`、`latestSummaryTitle`、`previewMessages` |
| `messages.idx.json` | `MessageStoreIndexFile`（`index.rs`）：`version`(=1)、`items[]`，每项 `messageId`、`blockId`、`offset`、`byteLen`；`compactionKind`/`role`/`createdAt` 为运行时字段、序列化时清空 |
| `blocks/000000.jsonl` | V3 明文块：每行一个消息 JSON |
| `blocks/000000.jsonl.zstd` | V4 压缩块：整块单帧 zstd + 原子写 |
| `messages.jsonl` | V2 遗留单文件；V3/V4 写入快照后被删除 |
| `active_plans.jsonl` | 遗留文件名常量；现役活动计划存 `chat_metadata.sqlite.active_plan_records` |
| `blobs/` | 遗留二进制目录 |

消息行 JSON = `ChatMessage`（`types_chat.rs`）：`id`、`role`、`createdAt`、`speakerAgentId`、`parts[]`（`MessagePart` 带 `type` 标签：text/image/audio/attachment）、`extraTextBlocks`、`providerMeta`、`toolCall`、`mcpCall`、`memeAnnotations`。

存储演进：V1 单 conversation JSON → V2 manifest/meta/messages.jsonl/index/blocks → V3 明文块 + SQLite locator → V4 zstd 压缩块。`message_locator` 表是 V3+ 正文定位权威，`messages.idx.json` 是块内定位索引。

## 6. memory/memory_store.db

PRAGMA：WAL、synchronous=NORMAL、foreign_keys=ON、temp_store=MEMORY（`db.rs`）。

| 表 | 关键列 | 说明 |
|---|---|---|
| `memory_record` | `id` PK, `memory_no` UNIQUE, `memory_type` DEFAULT 'knowledge', `judgment`, `reasoning`, `owner_agent_id`, `strength`, `is_active`, `memory_scope`, `useful_count`, `useful_score`, `last_recalled_at`, `last_decay_at`, `created_at`, `updated_at` | `memory_type` 合法值 knowledge/skill/emotion/event |
| `global_tag` | `id` PK, `name` UNIQUE | |
| `memory_tag_rel` | `memory_id`, `tag_id` | PK(memory_id, tag_id)；FK 双向 CASCADE |
| `profile_memory_link` | `id` PK, `memory_id` UNIQUE, `source` DEFAULT 'auto', `created_at`, `updated_at` | |
| `note_index_record` | `source_id` PK, `note_short_id` UNIQUE, `file_id`, `source_file_path`, `heading_h1`~`heading_h6`, `total_lines`, `updated_at` | 笔记索引 |
| `note_tag_rel` | `source_id`, `tag_id` | PK(source_id, tag_id) |
| `embedding_provider` | `provider_id` PK, `dimension`, `model_name`, `is_active`, `created_at`, `updated_at` | |
| `kb_runtime_state` | `key` PK, `value` | 键：active_index_provider_id、embedding_api_config_id、rerank_api_config_id、rebuild_status 等 |
| `memory_fts` | FTS5(`item_id` UNINDEXED, `judgment`) | |
| `note_fts` | FTS5(`item_id` UNINDEXED, `tags`) | |

索引：`idx_memory_updated_at`、`idx_memory_scope_active`、`idx_memory_useful_score`、`idx_memory_tag_tag_id`、`idx_profile_memory_updated_at`、`idx_note_updated_at`、`idx_note_file_id`、`idx_note_tag_tag_id`。

## 7. task/task_store.db

`task_record`（`store.rs`）：

`task_id` PK、`conversation_id`、`department_id`（历史列，已不再读写）、`agent_id`、`target_scope` DEFAULT 'desktop'、`order_index`、`title`、`cause`、`goal`、`flow`、`todos_json`、`status_summary`、`completion_state`、`completion_conclusion`、`progress_notes_json`、`stage_key`、`stage_updated_at_utc`、`trigger_kind`、`run_at_utc`、`cron_expression`、`every_minutes`、`end_at_utc`、`created_at_utc`、`updated_at_utc`、`last_triggered_at_utc`、`completed_at_utc`。

辅助表：`task_runtime_state`（`state_key` PK, `state_value`, `updated_at_utc`）、`task_run_log`（`id` PK AUTOINCREMENT, `task_id`, `triggered_at_utc`, `outcome`, `note`）。

## 8. delegate/delegate_store.db

`delegate_record`（`store.rs`）：

- 基础列：`delegate_id` PK、`kind`、`conversation_id`、`parent_delegate_id`、`source_department_id`、`target_department_id`（两列均为历史列，已放宽为可空且不再写入）、`source_agent_id`、`target_agent_id`、`title`、`why`、`goal`、`todo`、`notify_assistant_when_done`、`call_stack_json`、`status`、`created_at`、`updated_at`、`delivered_at`、`completed_at`
- 迁移追加的快照列：`snapshot_conversation_id`、`snapshot_updated_at`、`snapshot_archived_at`、`snapshot_last_message_at`、`snapshot_message_count`、`snapshot_step_count`、`snapshot_tool_call_count`、`snapshot_last_tool_name`、`snapshot_input_token_count`、`snapshot_output_token_count`、`snapshot_cache_read_token_count`、`snapshot_cache_write_token_count`、`snapshot_cumulative_usage_json`

`why`/`goal`/`todo` 由旧列（`background`/`instruction`/`question`/`specific_goal`/`deliverable_requirement`/`focus`）迁移重建而来。

## 9. 序列化约定

所有结构体使用 `#[serde(rename_all = "camelCase")]`，Rust 下划线命名在 JSON/TOML 中变为驼峰。`config_mark` 与 `PORTABLE` 均为无扩展名标记文件。
