use rusqlite::{OptionalExtension as _, TransactionBehavior};

const CHAT_METADATA_DB_FILE_NAME: &str = "chat_metadata.sqlite";
const CHAT_STORAGE_MIGRATION_KEY: &str = "v3_chat_metadata_sqlite";
pub(super) const USAGE_TRAIL_MIGRATION_KEY: &str = "usage_trail_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatStorageOperationDetail {
    conversation_id: String,
    expected_block_files: Vec<String>,
    #[serde(default)]
    replaced_block_files: Vec<String>,
    #[serde(default)]
    retired_block_files: Vec<String>,
    #[serde(default)]
    new_block_files: Vec<String>,
}

#[derive(Debug, Clone)]
struct ChatMetadataLocator {
    sequence: i64,
    item: MessageStoreIndexItem,
}

static CHAT_METADATA_WRITER_GATES: OnceLock<Mutex<std::collections::HashMap<String, std::sync::Weak<Mutex<()>>>>> = OnceLock::new();
static CHAT_METADATA_PUBLICATION_GATES: OnceLock<Mutex<std::collections::HashMap<String, std::sync::Weak<std::sync::RwLock<()>>>>> = OnceLock::new();

fn chat_metadata_store_with_writer_gate<T>(
    paths: &MessageStorePaths,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let key = format!(
        "{}:{}",
        chat_metadata_store_db_path(&paths.data_path).display(),
        paths.conversation_id
    );
    let gate = {
        let gates = CHAT_METADATA_WRITER_GATES.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
        let mut gates = gates.lock().unwrap_or_else(|poison| poison.into_inner());
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&key).and_then(std::sync::Weak::upgrade) {
            gate
        } else {
            let gate = std::sync::Arc::new(Mutex::new(()));
            gates.insert(key, std::sync::Arc::downgrade(&gate));
            gate
        }
    };
    let _guard = gate.lock().unwrap_or_else(|poison| poison.into_inner());
    operation()
}

fn chat_metadata_store_publication_gate(
    paths: &MessageStorePaths,
) -> std::sync::Arc<std::sync::RwLock<()>> {
    let key = format!(
        "{}:{}",
        chat_metadata_store_db_path(&paths.data_path).display(),
        paths.conversation_id
    );
    let gates = CHAT_METADATA_PUBLICATION_GATES
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut gates = gates.lock().unwrap_or_else(|poison| poison.into_inner());
    gates.retain(|_, gate| gate.strong_count() > 0);
    if let Some(gate) = gates.get(&key).and_then(std::sync::Weak::upgrade) {
        return gate;
    }
    let gate = std::sync::Arc::new(std::sync::RwLock::new(()));
    gates.insert(key, std::sync::Arc::downgrade(&gate));
    gate
}

fn chat_metadata_store_with_read_snapshot<T>(
    paths: &MessageStorePaths,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let gate = chat_metadata_store_publication_gate(paths);
    let _guard = gate.read().unwrap_or_else(|poison| poison.into_inner());
    operation()
}

fn chat_metadata_operation_root(paths: &MessageStorePaths, operation_id: &str) -> PathBuf {
    paths.shard_dir.join(".v3-operations").join(operation_id)
}

fn chat_metadata_copy_blocks(source: &PathBuf, target: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|err| format!("创建聊天存储操作目录失败，path={}，error={err}", target.display()))?;
    if !source.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(source).map_err(|err| format!("读取聊天块目录失败，path={}，error={err}", source.display()))? {
        let entry = entry.map_err(|err| format!("读取聊天块目录项失败: {err}"))?;
        let path = entry.path();
        if path.is_file() {
            fs::copy(&path, target.join(entry.file_name())).map_err(|err| format!("备份聊天块失败，path={}，error={err}", path.display()))?;
        }
    }
    Ok(())
}

fn chat_metadata_restore_blocks(paths: &MessageStorePaths, source: &PathBuf) -> Result<(), String> {
    if paths.blocks_dir.exists() {
        fs::remove_dir_all(&paths.blocks_dir).map_err(|err| format!("恢复聊天块前清理失败，path={}，error={err}", paths.blocks_dir.display()))?;
    }
    chat_metadata_copy_blocks(source, &paths.blocks_dir)
}


pub(super) fn chat_metadata_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_layout_chat_dir(data_path).join(CHAT_METADATA_DB_FILE_NAME)
}

fn chat_metadata_store_open(data_path: &PathBuf) -> Result<rusqlite::Connection, String> {
    let db_path = chat_metadata_store_db_path(data_path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!("创建聊天元数据数据库目录失败，path={}，error={err}", parent.display())
        })?;
    }
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|err| format!("打开聊天元数据数据库失败，path={}，error={err}", db_path.display()))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA busy_timeout=10000;

         CREATE TABLE IF NOT EXISTS chat_storage_migrations (
           migration_key TEXT PRIMARY KEY,
           state TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS conversation_metadata (
           conversation_id TEXT PRIMARY KEY,
           metadata_json TEXT NOT NULL,
           storage_revision INTEGER NOT NULL DEFAULT 0,
           updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS conversation_blocks (
           conversation_id TEXT NOT NULL,
           block_id INTEGER NOT NULL,
           block_file TEXT NOT NULL,
           byte_len INTEGER NOT NULL,
           message_count INTEGER NOT NULL,
           PRIMARY KEY (conversation_id, block_id),
           FOREIGN KEY(conversation_id) REFERENCES conversation_metadata(conversation_id) ON DELETE CASCADE
         );
         CREATE TABLE IF NOT EXISTS message_locator (
           conversation_id TEXT NOT NULL,
           sequence INTEGER NOT NULL,
           message_id TEXT NOT NULL,
           block_id INTEGER NOT NULL,
           byte_offset INTEGER NOT NULL,
           byte_len INTEGER NOT NULL,
           compaction_kind TEXT,
           role TEXT NOT NULL DEFAULT '',
           created_at TEXT NOT NULL DEFAULT '',
           PRIMARY KEY (conversation_id, sequence),
           UNIQUE (conversation_id, message_id),
           FOREIGN KEY(conversation_id) REFERENCES conversation_metadata(conversation_id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_message_locator_recent ON message_locator(conversation_id, sequence DESC);
         CREATE INDEX IF NOT EXISTS idx_message_locator_block ON message_locator(conversation_id, block_id, byte_offset);
         CREATE TABLE IF NOT EXISTS active_plan_records (
           conversation_id TEXT NOT NULL,
           plan_id TEXT NOT NULL,
           record_json TEXT NOT NULL,
           PRIMARY KEY (conversation_id, plan_id),
           FOREIGN KEY(conversation_id) REFERENCES conversation_metadata(conversation_id) ON DELETE CASCADE
         );
         CREATE TABLE IF NOT EXISTS storage_operations (
           operation_id TEXT PRIMARY KEY,
           conversation_id TEXT NOT NULL,
           before_revision INTEGER NOT NULL,
           after_revision INTEGER NOT NULL,
           state TEXT NOT NULL,
           detail_json TEXT NOT NULL,
           created_at TEXT NOT NULL,
           committed_at TEXT
         );
         CREATE TABLE IF NOT EXISTS usage_trail (
           bucket TEXT NOT NULL,
           conversation_id TEXT NOT NULL,
           agent_id TEXT NOT NULL DEFAULT '',
           department_id TEXT NOT NULL DEFAULT '',
           conversation_kind TEXT NOT NULL DEFAULT '',
           api_config_id TEXT NOT NULL DEFAULT '',
           provider_key TEXT NOT NULL DEFAULT '',
           provider_label TEXT NOT NULL DEFAULT '',
           model_name TEXT NOT NULL DEFAULT '',
           input_tokens INTEGER NOT NULL DEFAULT 0,
           output_tokens INTEGER NOT NULL DEFAULT 0,
           total_tokens INTEGER NOT NULL DEFAULT 0,
           cache_read_tokens INTEGER NOT NULL DEFAULT 0,
           cache_write_tokens INTEGER NOT NULL DEFAULT 0,
           reasoning_tokens INTEGER NOT NULL DEFAULT 0,
           updated_at TEXT NOT NULL,
           PRIMARY KEY (bucket, conversation_id, provider_key, model_name)
         );
         CREATE INDEX IF NOT EXISTS idx_usage_trail_bucket ON usage_trail(bucket);
         CREATE INDEX IF NOT EXISTS idx_usage_trail_conversation ON usage_trail(conversation_id);",
    )
    .map_err(|err| format!("初始化聊天元数据数据库失败: {err}"))?;
    for (column, definition) in [
        ("role", "TEXT NOT NULL DEFAULT ''"),
        ("created_at", "TEXT NOT NULL DEFAULT ''"),
    ] {
        let mut statement = conn.prepare("PRAGMA table_info(message_locator)")
            .map_err(|err| format!("读取聊天 locator schema 失败: {err}"))?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))
            .map_err(|err| format!("读取聊天 locator schema 列失败: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析聊天 locator schema 列失败: {err}"))?;
        if !columns.iter().any(|item| item == column) {
            conn.execute_batch(&format!("ALTER TABLE message_locator ADD COLUMN {column} {definition}"))
                .map_err(|err| format!("升级聊天 locator schema 失败，column={column}，error={err}"))?;
        }
    }
    Ok(conn)
}

fn chat_metadata_store_migration_is_completed(
    data_path: &PathBuf,
    migration_key: &str,
) -> Result<bool, String> {
    let path = chat_metadata_store_db_path(data_path);
    if !path.exists() {
        return Ok(false);
    }
    let conn = chat_metadata_store_open(data_path)?;
    conn.query_row(
        "SELECT state FROM chat_storage_migrations WHERE migration_key=?1",
        [migration_key],
        |row| row.get::<_, String>(0),
    )
    .map(|state| state == "completed")
    .or_else(|err| {
        if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
            Ok(false)
        } else {
            Err(err)
        }
    })
    .map_err(|err| format!("读取聊天元数据迁移状态失败: {err}"))
}

fn chat_metadata_store_mark_migration_completed(
    data_path: &PathBuf,
    migration_key: &str,
) -> Result<(), String> {
    let conn = chat_metadata_store_open(data_path)?;
    conn.execute(
        "INSERT INTO chat_storage_migrations(migration_key, state, updated_at) VALUES(?1, 'completed', ?2)
         ON CONFLICT(migration_key) DO UPDATE SET state='completed', updated_at=excluded.updated_at",
        rusqlite::params![migration_key, now_iso()],
    )
    .map_err(|err| format!("写入聊天元数据迁移状态失败，migration_key={migration_key}，error={err}"))?;
    Ok(())
}

fn chat_metadata_store_v3_conversation_migration_key(conversation_id: &str) -> String {
    format!("{CHAT_STORAGE_MIGRATION_KEY}:conversation:{conversation_id}")
}

pub(super) fn chat_metadata_store_is_ready(data_path: &PathBuf) -> Result<bool, String> {
    chat_metadata_store_migration_is_completed(data_path, CHAT_STORAGE_MIGRATION_KEY)
}

pub(super) fn chat_metadata_store_contains_conversation(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let conn = chat_metadata_store_open(data_path)?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_metadata WHERE conversation_id=?1)",
        [conversation_id],
        |row| row.get(0),
    )
    .map_err(|err| {
        format!(
            "确认 SQLite 会话是否存在失败，conversation_id={}，error={err}",
            conversation_id
        )
    })
}

fn chat_metadata_store_read_messages_for_locators(
    paths: &MessageStorePaths,
    locators: &[ChatMetadataLocator],
    cached: bool,
) -> Result<Vec<ChatMessage>, String> {
    let items = locators.iter().map(|locator| locator.item.clone()).collect::<Vec<_>>();
    if cached {
        read_jsonl_snapshot_messages_by_index_items_cached(&paths.messages_file, &items)
    } else {
        read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &items)
    }
}

fn chat_metadata_store_read_recent_page(
    paths: &MessageStorePaths,
    limit: usize,
    cached: bool,
) -> Result<MessageStoreLimitPage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let (locators, has_more) = chat_metadata_store_read_locator_page(paths, None, None, true, limit)?;
        Ok(MessageStoreLimitPage {
            messages: chat_metadata_store_read_messages_for_locators(paths, &locators, cached)?,
            has_more,
        })
    })
}

fn chat_metadata_store_read_message_by_id(
    paths: &MessageStorePaths,
    message_id: &str,
) -> Result<ChatMessage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let locator = chat_metadata_store_read_locator_by_id(paths, message_id)?
            .ok_or_else(|| format!("Message not found: {}", message_id.trim()))?;
        chat_metadata_store_read_messages_for_locators(paths, &[locator], false)?
            .pop()
            .ok_or_else(|| format!("Message not found: {}", message_id.trim()))
    })
}

fn chat_metadata_store_read_message_sequence(
    paths: &MessageStorePaths,
    message_id: &str,
) -> Result<Option<usize>, String> {
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return Ok(None);
    }
    chat_metadata_store_with_read_snapshot(paths, || {
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let sequence = conn
            .query_row(
                "SELECT sequence FROM message_locator WHERE conversation_id=?1 AND message_id=?2",
                rusqlite::params![paths.conversation_id, message_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|err| format!("读取 SQLite 消息序号失败: {err}"))?;
        sequence
            .map(|value| usize::try_from(value).map_err(|_| "SQLite 消息序号无效".to_string()))
            .transpose()
    })
}

fn chat_metadata_store_read_latest_compaction_message(
    paths: &MessageStorePaths,
) -> Result<Option<ChatMessage>, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let locator = conn
        .query_row(
            "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
             FROM message_locator
             WHERE conversation_id=?1 AND compaction_kind IS NOT NULL
             ORDER BY sequence DESC LIMIT 1",
            [&paths.conversation_id],
            |row| {
                Ok(ChatMetadataLocator {
                    sequence: row.get(0)?,
                    item: MessageStoreIndexItem {
                        message_id: row.get(1)?,
                        block_id: Some(row.get::<_, i64>(2)? as u32),
                        offset: row.get::<_, i64>(3)? as u64,
                        byte_len: row.get::<_, i64>(4)? as u64,
                        compaction_kind: row.get(5)?,
                        role: row.get(6)?,
                        created_at: row.get(7)?,
                    },
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取 SQLite 最新压缩消息失败: {err}"))?;
    locator
        .map(|locator| {
            chat_metadata_store_read_messages_for_locators(paths, &[locator], false)?
                .pop()
                .ok_or_else(|| "读取 SQLite 最新压缩消息为空".to_string())
        })
        .transpose()
    })
}

/// 轻量重算替换后的最新摘要标题：只读取摘要消息范围（compaction locator 索引 + 按需正文），
/// 不整读会话。`updated_messages` 为本次替换后的消息；磁盘上未被替换的摘要消息按原样参与计算。
pub(super) fn chat_metadata_store_recompute_latest_summary_title(
    paths: &MessageStorePaths,
    updated_messages: &[ChatMessage],
) -> Result<Option<String>, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let mut stmt = conn
            .prepare(
                "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
                 FROM message_locator
                 WHERE conversation_id=?1 AND compaction_kind IS NOT NULL
                 ORDER BY sequence DESC",
            )
            .map_err(|err| format!("查询 SQLite 摘要消息 locator 失败: {err}"))?;
        let locators = stmt
            .query_map([&paths.conversation_id], |row| {
                Ok(ChatMetadataLocator {
                    sequence: row.get(0)?,
                    item: MessageStoreIndexItem {
                        message_id: row.get(1)?,
                        block_id: Some(row.get::<_, i64>(2)? as u32),
                        offset: row.get::<_, i64>(3)? as u64,
                        byte_len: row.get::<_, i64>(4)? as u64,
                        compaction_kind: row.get(5)?,
                        role: row.get(6)?,
                        created_at: row.get(7)?,
                    },
                })
            })
            .map_err(|err| format!("读取 SQLite 摘要消息 locator 失败: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("读取 SQLite 摘要消息 locator 失败: {err}"))?;
        let replaced_by_id = updated_messages
            .iter()
            .map(|message| (message.id.trim().to_string(), message))
            .collect::<std::collections::HashMap<_, _>>();
        let mut candidates = Vec::<(i64, Option<String>)>::with_capacity(locators.len());
        for locator in &locators {
            let title = if let Some(updated) = replaced_by_id.get(locator.item.message_id.as_str()) {
                super::summary_context_message_title(updated)
            } else {
                chat_metadata_store_read_messages_for_locators(paths, std::slice::from_ref(locator), false)?
                    .first()
                    .and_then(super::summary_context_message_title)
            };
            candidates.push((locator.sequence, title));
        }
        for updated in updated_messages {
            if super::summary_context_message_title(updated).is_none() {
                continue;
            }
            let message_id = updated.id.trim();
            if locators
                .iter()
                .any(|locator| locator.item.message_id.as_str() == message_id)
            {
                continue;
            }
            let sequence = conn
                .query_row(
                    "SELECT sequence FROM message_locator WHERE conversation_id=?1 AND message_id=?2",
                    rusqlite::params![paths.conversation_id, message_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|err| format!("读取 SQLite 消息序号失败: {err}"))?
                .ok_or_else(|| {
                    format!(
                        "重算摘要标题失败：替换消息不在消息仓库，conversation_id={}，message_id={}",
                        paths.conversation_id,
                        message_id
                    )
                })?;
            candidates.push((sequence, super::summary_context_message_title(updated)));
        }
        Ok(candidates
            .into_iter()
            .filter_map(|(sequence, title)| title.map(|title| (sequence, title)))
            .max_by_key(|(sequence, _)| *sequence)
            .map(|(_, title)| title))
    })
}

fn chat_metadata_store_read_messages_before(
    paths: &MessageStorePaths,
    message_id: &str,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let anchor = chat_metadata_store_read_locator_by_id(paths, message_id)?
            .ok_or_else(|| format!("Message not found: {}", message_id.trim()))?;
        let (locators, has_more) = chat_metadata_store_read_locator_page(paths, None, Some(anchor.sequence), true, limit)?;
        Ok(MessageStoreLimitPage {
            messages: chat_metadata_store_read_messages_for_locators(paths, &locators, false)?,
            has_more,
        })
    })
}

fn chat_metadata_store_read_messages_after(
    paths: &MessageStorePaths,
    message_id: &str,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let anchor = chat_metadata_store_read_locator_by_id(paths, message_id)?
            .ok_or_else(|| format!("Message not found: {}", message_id.trim()))?;
        let (locators, has_more) = chat_metadata_store_read_locator_page(paths, Some(anchor.sequence), None, false, limit)?;
        Ok(MessageStoreLimitPage {
            messages: chat_metadata_store_read_messages_for_locators(paths, &locators, false)?,
            has_more,
        })
    })
}

fn chat_metadata_store_read_messages_after_all(
    paths: &MessageStorePaths,
    message_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let anchor = chat_metadata_store_read_locator_by_id(paths, message_id)?
            .ok_or_else(|| format!("Message not found: {}", message_id.trim()))?;
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let locators = chat_metadata_store_query_locators(&conn, &paths.conversation_id, "AND sequence>?2", &[&anchor.sequence])?;
        chat_metadata_store_read_messages_for_locators(paths, &locators, false)
    })
}

fn chat_metadata_store_latest_block_paths(
    paths: &MessageStorePaths,
    limit: usize,
) -> Result<Vec<PathBuf>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let mut statement = conn.prepare(
        "SELECT block_file FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id DESC LIMIT ?2",
    ).map_err(|err| format!("准备读取 SQLite 最新 block 路径失败: {err}"))?;
    let rows = statement.query_map(rusqlite::params![paths.conversation_id, limit.clamp(1, 8) as i64], |row| row.get::<_, String>(0))
        .map_err(|err| format!("读取 SQLite 最新 block 路径失败: {err}"))?;
    let mut paths_out = rows.map(|row| row.map(|file| paths.shard_dir.join(file)).map_err(|err| format!("解析 SQLite 最新 block 路径失败: {err}")))
        .collect::<Result<Vec<_>, _>>()?;
    paths_out.reverse();
    Ok(paths_out)
}

fn chat_metadata_store_read_recent_blocks_page(
    paths: &MessageStorePaths,
    limit: usize,
    cached: bool,
) -> Result<MessageStoreLimitPage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let mut statement = conn.prepare(
            "SELECT block_id FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id DESC LIMIT ?2",
        ).map_err(|err| format!("准备读取 SQLite 最近 block 失败: {err}"))?;
        let rows = statement.query_map(rusqlite::params![paths.conversation_id, limit.clamp(1, 8) as i64], |row| row.get::<_, i64>(0))
            .map_err(|err| format!("读取 SQLite 最近 block 失败: {err}"))?;
        let mut block_ids = rows.map(|row| row.map(|value| value as u32).map_err(|err| format!("解析 SQLite 最近 block 失败: {err}")))
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = chat_metadata_store_read_block_count(paths)? > block_ids.len();
        block_ids.reverse();
        let mut locators = Vec::new();
        for block_id in block_ids {
            locators.extend(chat_metadata_store_read_locators_for_block(paths, block_id)?);
        }
        Ok(MessageStoreLimitPage {
            messages: chat_metadata_store_read_messages_for_locators(paths, &locators, cached)?,
            has_more,
        })
    })
}

fn chat_metadata_store_index_summary(
    paths: &MessageStorePaths,
) -> Result<MessageStoreIndexSummary, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let message_count = chat_metadata_store_message_count(paths)?;
    let visible_message_count = conn.query_row(
        "SELECT COUNT(*) FROM message_locator WHERE conversation_id=?1 AND lower(role) IN ('user', 'assistant', 'tool')",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).map_err(|err| format!("统计 SQLite 可见消息失败: {err}"))? as usize;
    let mut statement = conn.prepare(
        "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
         FROM message_locator WHERE conversation_id=?1 AND lower(role) IN ('user', 'assistant', 'tool')
         ORDER BY sequence DESC LIMIT 2",
    ).map_err(|err| format!("准备读取 SQLite 预览 locator 失败: {err}"))?;
    let rows = statement.query_map([&paths.conversation_id], |row| {
        Ok(ChatMetadataLocator {
            sequence: row.get(0)?,
            item: MessageStoreIndexItem {
                message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32),
                offset: row.get::<_, i64>(3)? as u64, byte_len: row.get::<_, i64>(4)? as u64,
                compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
            },
        })
    }).map_err(|err| format!("读取 SQLite 预览 locator 失败: {err}"))?;
    let mut preview_locators = rows.map(|row| row.map_err(|err| format!("解析 SQLite 预览 locator 失败: {err}"))).collect::<Result<Vec<_>, _>>()?;
    preview_locators.reverse();
    let preview_messages = chat_metadata_store_read_messages_for_locators(paths, &preview_locators, false)?;
    let preview_items = preview_messages.iter().map(|message| MessageStoreIndexPreviewItem {
        message_id: message.id.clone(), role: message.role.clone(), speaker_agent_id: message.speaker_agent_id.clone(),
        created_at: Some(message.created_at.clone()).filter(|value| !value.trim().is_empty()),
        text_preview: build_conversation_preview_text(message), has_image: message_store_message_has_image(message),
        has_pdf: message_store_message_has_pdf(message), has_audio: message_store_message_has_audio(message),
        has_attachment: conversation_message_has_attachment(message),
    }).collect::<Vec<_>>();
    let last = chat_metadata_store_read_locator_page(paths, None, None, true, 1)?.0.pop();
    let last_message = match last {
        Some(locator) => chat_metadata_store_read_messages_for_locators(paths, &[locator], false)?.pop(),
        None => None,
    };
    let mut first_user_text_preview = None;
    let mut user_statement = conn.prepare(
        "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
         FROM message_locator WHERE conversation_id=?1 AND lower(role)='user' ORDER BY sequence ASC LIMIT 32",
    ).map_err(|err| format!("准备读取 SQLite 首条用户消息失败: {err}"))?;
    let user_rows = user_statement.query_map([&paths.conversation_id], |row| {
        Ok(ChatMetadataLocator {
            sequence: row.get(0)?,
            item: MessageStoreIndexItem {
                message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32),
                offset: row.get::<_, i64>(3)? as u64, byte_len: row.get::<_, i64>(4)? as u64,
                compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
            },
        })
    }).map_err(|err| format!("读取 SQLite 首条用户消息失败: {err}"))?;
    let user_locators = user_rows.map(|row| row.map_err(|err| format!("解析 SQLite 首条用户消息失败: {err}"))).collect::<Result<Vec<_>, _>>()?;
    for message in chat_metadata_store_read_messages_for_locators(paths, &user_locators, false)? {
        if message.speaker_agent_id.as_deref().map(str::trim).filter(|value| !value.is_empty()) == Some(SYSTEM_PERSONA_ID) {
            continue;
        }
        let preview = build_conversation_preview_text(&message).trim().to_string();
        if !preview.is_empty() {
            first_user_text_preview = Some(preview);
            break;
        }
    }
    Ok(MessageStoreIndexSummary {
        message_count, visible_message_count,
        last_message_id: last_message.as_ref().map(|message| message.id.trim().to_string()).unwrap_or_default(),
        last_message_at: last_message.map(|message| message.created_at),
        first_user_text_preview, preview_items,
    })
    })
}

fn chat_metadata_store_chat_snapshot(
    paths: &MessageStorePaths,
) -> Result<MessageStoreChatSnapshot, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let active_message_count = chat_metadata_store_message_count(paths)?;
    let latest_for_role = |role: &str| -> Result<Option<ChatMessage>, String> {
        let locator = conn.query_row(
            "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
             FROM message_locator WHERE conversation_id=?1 AND lower(role)=lower(?2) ORDER BY sequence DESC LIMIT 1",
            rusqlite::params![paths.conversation_id, role],
            |row| Ok(ChatMetadataLocator {
                sequence: row.get(0)?,
                item: MessageStoreIndexItem {
                    message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32),
                    offset: row.get::<_, i64>(3)? as u64, byte_len: row.get::<_, i64>(4)? as u64,
                    compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
                },
            }),
        ).optional().map_err(|err| format!("读取 SQLite 最新 {role} 消息失败: {err}"))?;
        locator.map(|locator| chat_metadata_store_read_messages_for_locators(paths, &[locator], false)?.pop()
            .ok_or_else(|| format!("读取 SQLite 最新 {role} 消息为空"))).transpose()
    };
    Ok(MessageStoreChatSnapshot {
        latest_user: latest_for_role("user")?,
        latest_assistant: latest_for_role("assistant")?,
        active_message_count,
    })
    })
}

fn chat_metadata_store_read_meta(paths: &MessageStorePaths) -> Result<Option<ConversationShardMeta>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let raw = conn
        .query_row(
            "SELECT metadata_json FROM conversation_metadata WHERE conversation_id=?1",
            [&paths.conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("读取会话 SQLite metadata 失败，conversation_id={}，error={err}", paths.conversation_id))?;
    raw.map(|value| serde_json::from_str::<ConversationShardMeta>(&value)
        .map_err(|err| format!("解析会话 SQLite metadata 失败，conversation_id={}，error={err}", paths.conversation_id)))
        .transpose()
}

fn chat_metadata_store_read_index(paths: &MessageStorePaths) -> Result<Option<MessageStoreIndexFile>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_metadata WHERE conversation_id=?1)",
        [&paths.conversation_id],
        |row| row.get(0),
    ).map_err(|err| format!("确认 SQLite 会话存在失败: {err}"))?;
    if !exists {
        return Ok(None);
    }
    Ok(Some(MessageStoreIndexFile::new(
        MESSAGE_STORE_MANIFEST_VERSION,
        chat_metadata_store_query_locators(&conn, &paths.conversation_id, "", &[])?.into_iter().map(|row| row.item).collect(),
    )))
}

fn chat_metadata_store_query_locators(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    predicate: &str,
    values: &[&dyn rusqlite::ToSql],
) -> Result<Vec<ChatMetadataLocator>, String> {
    let sql = format!(
        "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
         FROM message_locator WHERE conversation_id=?1 {predicate} ORDER BY sequence ASC"
    );
    let mut params = Vec::<&dyn rusqlite::ToSql>::with_capacity(values.len() + 1);
    params.push(&conversation_id);
    params.extend_from_slice(values);
    let mut statement = conn.prepare(&sql).map_err(|err| format!("准备读取 SQLite locator 失败: {err}"))?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(ChatMetadataLocator {
            sequence: row.get(0)?,
            item: MessageStoreIndexItem {
                message_id: row.get(1)?,
                block_id: Some(row.get::<_, i64>(2)? as u32),
                offset: row.get::<_, i64>(3)? as u64,
                byte_len: row.get::<_, i64>(4)? as u64,
                compaction_kind: row.get(5)?,
                role: row.get(6)?,
                created_at: row.get(7)?,
            },
        })
    }).map_err(|err| format!("读取 SQLite locator 失败: {err}"))?;
    rows.map(|row| row.map_err(|err| format!("解析 SQLite locator 失败: {err}"))).collect()
}

fn chat_metadata_store_read_locator_by_id(
    paths: &MessageStorePaths,
    message_id: &str,
) -> Result<Option<ChatMetadataLocator>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let mut rows = chat_metadata_store_query_locators(
        &conn,
        &paths.conversation_id,
        "AND message_id=?2",
        &[&message_id],
    )?;
    Ok(rows.pop())
}

fn chat_metadata_store_read_locator_page(
    paths: &MessageStorePaths,
    after_sequence: Option<i64>,
    before_sequence: Option<i64>,
    descending: bool,
    limit: usize,
) -> Result<(Vec<ChatMetadataLocator>, bool), String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let predicate = match (after_sequence, before_sequence) {
        (Some(_), None) => "AND sequence>?2",
        (None, Some(_)) => "AND sequence<?2",
        (None, None) => "",
        (Some(_), Some(_)) => "AND sequence>?2 AND sequence<?3",
    };
    let sql = format!(
        "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
         FROM message_locator WHERE conversation_id=?1 {predicate}
         ORDER BY sequence {} LIMIT ?{}",
        if descending { "DESC" } else { "ASC" },
        match (after_sequence, before_sequence) { (Some(_), Some(_)) => 4, (_, Some(_)) | (Some(_), _) => 3, (None, None) => 2 },
    );
    let mut statement = conn.prepare(&sql).map_err(|err| format!("准备读取 SQLite locator 分页失败: {err}"))?;
    let requested = normalized_message_limit(limit).saturating_add(1) as i64;
    let after_value = after_sequence.unwrap_or_default();
    let before_value = before_sequence.unwrap_or_default();
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&paths.conversation_id];
    if after_sequence.is_some() { params.push(&after_value); }
    if before_sequence.is_some() { params.push(&before_value); }
    params.push(&requested);
    let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(ChatMetadataLocator {
            sequence: row.get(0)?,
            item: MessageStoreIndexItem {
                message_id: row.get(1)?,
                block_id: Some(row.get::<_, i64>(2)? as u32),
                offset: row.get::<_, i64>(3)? as u64,
                byte_len: row.get::<_, i64>(4)? as u64,
                compaction_kind: row.get(5)?,
                role: row.get(6)?,
                created_at: row.get(7)?,
            },
        })
    }).map_err(|err| format!("读取 SQLite locator 分页失败: {err}"))?;
    let mut rows = rows.map(|row| row.map_err(|err| format!("解析 SQLite locator 分页失败: {err}"))).collect::<Result<Vec<_>, _>>()?;
    let has_more = rows.len() > normalized_message_limit(limit);
    rows.truncate(normalized_message_limit(limit));
    if descending { rows.reverse(); }
    Ok((rows, has_more))
}

fn chat_metadata_store_read_locators_for_block(
    paths: &MessageStorePaths,
    block_id: u32,
) -> Result<Vec<ChatMetadataLocator>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    chat_metadata_store_query_locators(&conn, &paths.conversation_id, "AND block_id=?2", &[&(block_id as i64)])
}

fn chat_metadata_store_read_last_block_id(paths: &MessageStorePaths) -> Result<Option<u32>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    conn.query_row(
        "SELECT block_id FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id DESC LIMIT 1",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).optional().map(|value| value.map(|value| value as u32))
        .map_err(|err| format!("读取 SQLite 最新 block 失败: {err}"))
}

fn chat_metadata_store_message_count(paths: &MessageStorePaths) -> Result<usize, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    conn.query_row(
        "SELECT COUNT(*) FROM message_locator WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).map(|count| count as usize).map_err(|err| format!("统计 SQLite locator 失败: {err}"))
}

pub(super) fn chat_metadata_store_list_chat_index(
    data_path: &PathBuf,
) -> Result<Option<Vec<ChatIndexConversationItem>>, String> {
    let conn = chat_metadata_store_open(data_path)?;
    let mut stmt = conn.prepare("SELECT metadata_json FROM conversation_metadata")
        .map_err(|err| format!("准备读取 SQLite 会话列表失败: {err}"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| format!("读取 SQLite 会话列表失败: {err}"))?;
    let mut items = Vec::new();
    for row in rows {
        let raw = row.map_err(|err| format!("读取 SQLite 会话 metadata 失败: {err}"))?;
        let meta = serde_json::from_str::<ConversationShardMeta>(&raw)
            .map_err(|err| format!("解析 SQLite 会话 metadata 失败: {err}"))?;
        items.push(ChatIndexConversationItem {
            id: meta.id().to_string(),
            updated_at: meta.updated_at().to_string(),
            status: meta.status().to_string(),
            archived_at: meta.archived_at().map(ToOwned::to_owned),
        });
    }
    Ok(Some(items))
}

fn chat_metadata_store_read_active_plans(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Option<Vec<ActivePlanRecord>>, String> {
    let conn = chat_metadata_store_open(data_path)?;
    let mut stmt = conn.prepare(
        "SELECT record_json FROM active_plan_records WHERE conversation_id=?1 ORDER BY rowid DESC",
    ).map_err(|err| format!("准备读取 SQLite 活动计划失败: {err}"))?;
    let rows = stmt.query_map([conversation_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("读取 SQLite 活动计划失败: {err}"))?;
    let mut out = Vec::new();
    for row in rows {
        let raw = row.map_err(|err| format!("读取 SQLite 活动计划记录失败: {err}"))?;
        out.push(serde_json::from_str(&raw).map_err(|err| format!("解析 SQLite 活动计划记录失败: {err}"))?);
    }
    Ok(Some(out))
}

fn chat_metadata_store_append_active_plan(
    paths: &MessageStorePaths,
    record: &ActivePlanRecord,
) -> Result<(), String> {
    chat_metadata_store_with_writer_gate(paths, || {
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let raw = serde_json::to_string(record)
            .map_err(|err| format!("序列化 SQLite 活动计划失败: {err}"))?;
        conn.execute(
            "INSERT INTO active_plan_records(conversation_id, plan_id, record_json) VALUES (?1, ?2, ?3)",
            rusqlite::params![paths.conversation_id, record.plan_id, raw],
        )
        .map_err(|err| format!("写入 SQLite 活动计划失败: {err}"))?;
        Ok(())
    })
}

fn chat_metadata_store_complete_active_plan_by_path(
    paths: &MessageStorePaths,
    normalized_path: &str,
    completion_text: Option<&str>,
) -> Result<bool, String> {
    chat_metadata_store_with_writer_gate(paths, || {
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let matched = {
            let mut stmt = conn
                .prepare(
                    "SELECT rowid, record_json FROM active_plan_records
                     WHERE conversation_id=?1 ORDER BY rowid DESC",
                )
                .map_err(|err| format!("准备读取 SQLite 活动计划失败: {err}"))?;
            let rows = stmt
                .query_map([&paths.conversation_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|err| format!("读取 SQLite 活动计划失败: {err}"))?;
            let mut matched = None;
            for row in rows {
                let (rowid, raw) = row.map_err(|err| format!("读取 SQLite 活动计划记录失败: {err}"))?;
                let record = serde_json::from_str::<ActivePlanRecord>(&raw)
                    .map_err(|err| format!("解析 SQLite 活动计划记录失败: {err}"))?;
                if record.status.trim() == ACTIVE_PLAN_STATUS_IN_PROGRESS
                    && record.path.trim().eq_ignore_ascii_case(normalized_path)
                {
                    matched = Some((rowid, record));
                    break;
                }
            }
            matched
        };
        let Some((rowid, mut record)) = matched else {
            return Ok(false);
        };
        record.status = ACTIVE_PLAN_STATUS_COMPLETED.to_string();
        record.completed_at = Some(now_iso());
        record.completion_text = completion_text
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let raw = serde_json::to_string(&record)
            .map_err(|err| format!("序列化 SQLite 活动计划失败: {err}"))?;
        let updated = conn
            .execute(
                "UPDATE active_plan_records SET record_json=?1 WHERE rowid=?2",
                rusqlite::params![raw, rowid],
            )
            .map_err(|err| format!("更新 SQLite 活动计划失败: {err}"))?;
        if updated != 1 {
            return Err(format!("更新 SQLite 活动计划失败：记录不存在，rowid={rowid}"));
        }
        Ok(true)
    })
}

fn chat_metadata_store_delete_conversation_unlocked(paths: &MessageStorePaths) -> Result<bool, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let deleted = conn.execute(
        "DELETE FROM conversation_metadata WHERE conversation_id=?1",
        [&paths.conversation_id],
    ).map_err(|err| format!("删除 SQLite 会话 metadata 失败，conversation_id={}，error={err}", paths.conversation_id))?;
    Ok(deleted > 0)
}

fn chat_metadata_store_with_delete_gate<T>(
    paths: &MessageStorePaths,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    chat_metadata_store_with_writer_gate(paths, || {
        let publication_gate = chat_metadata_store_publication_gate(paths);
        let _publication_guard = publication_gate.write().unwrap_or_else(|poison| poison.into_inner());
        operation()
    })
}

fn chat_metadata_store_write_meta_only(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
) -> Result<(), String> {
    chat_metadata_store_with_writer_gate(paths, || {
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let raw = serde_json::to_string(meta).map_err(|err| format!("序列化 SQLite metadata 失败: {err}"))?;
        let updated = conn.execute(
            "UPDATE conversation_metadata
             SET metadata_json=?1, storage_revision=storage_revision+1, updated_at=?2
             WHERE conversation_id=?3",
            rusqlite::params![raw, meta.updated_at(), paths.conversation_id],
        ).map_err(|err| format!("更新 SQLite metadata 失败: {err}"))?;
        if updated != 1 {
            return Err(format!("写入 SQLite 会话 metadata 失败：会话不存在，conversation_id={}", paths.conversation_id));
        }
        Ok(())
    })
}

fn chat_metadata_store_write_result(
    conn: &rusqlite::Connection,
    paths: &MessageStorePaths,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let (message_count, last_message_id) = conn.query_row(
        "SELECT COUNT(*), COALESCE((SELECT message_id FROM message_locator WHERE conversation_id=?1 ORDER BY sequence DESC LIMIT 1), '')
         FROM message_locator WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    ).map_err(|err| format!("读取 SQLite 会话发布结果失败: {err}"))?;
    let total_bytes = conn.query_row(
        "SELECT COALESCE(SUM(byte_len), 0) FROM conversation_blocks WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).map_err(|err| format!("读取 SQLite block 字节数失败: {err}"))?;
    let revision = conn.query_row(
        "SELECT storage_revision FROM conversation_metadata WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).map_err(|err| format!("读取 SQLite storage revision 失败: {err}"))?;
    Ok(MessageStoreDirectorySnapshotWrite {
        manifest: MessageStoreManifest::jsonl_snapshot_ready_for_messages(
            message_count as usize,
            last_message_id.clone(),
            total_bytes as u64,
            revision as u64,
        ),
        message_count: message_count as usize,
        last_message_id,
    })
}

fn chat_metadata_store_reconcile_block_tail(
    paths: &MessageStorePaths,
    block_id: u32,
    expected_plain_len: u64,
    label: &str,
) -> Result<u64, String> {
    let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
    let block_path = paths.shard_dir.join(&block_file);
    let raw = std::fs::read(&block_path).map_err(|err| {
        format!(
            "{label}：读取块文件失败，path={}，error={err}",
            block_path.display()
        )
    })?;
    let keep_len = zstd_validate_tail_for_append(&raw, expected_plain_len as usize)?;
    if keep_len < raw.len() {
        runtime_log_warn(format!(
            "[消息存储] 追加前清理孤儿/损坏尾部，conversation_id={}，block_id={}，path={}，file_len={}，keep_len={}",
            paths.conversation_id,
            block_id,
            block_path.display(),
            raw.len(),
            keep_len
        ));
        fs::OpenOptions::new()
            .write(true)
            .open(&block_path)
            .map_err(|err| {
                format!(
                    "{label}：打开块文件失败（清理孤儿），path={}，error={err}",
                    block_path.display()
                )
            })?
            .set_len(keep_len as u64)
            .map_err(|err| {
                format!(
                    "{label}：截断块文件失败（清理孤儿），path={}，error={err}",
                    block_path.display()
                )
            })?;
    }
    Ok(keep_len as u64)
}

fn chat_metadata_store_publish_blocks(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
    changed_blocks: &[JsonlSnapshotConversationBlock],
    retired_block_ids: &[u32],
    locator_rows: &[(i64, MessageStoreIndexItem)],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let publication_gate = chat_metadata_store_publication_gate(paths);
    let _publication_guard = publication_gate.write().unwrap_or_else(|poison| poison.into_inner());
    let mut conn = chat_metadata_store_open(&paths.data_path)?;
        let before_revision = conn.query_row(
            "SELECT storage_revision FROM conversation_metadata WHERE conversation_id=?1",
            [&paths.conversation_id],
            |row| row.get::<_, i64>(0),
        ).optional().map_err(|err| format!("读取 SQLite storage revision 失败: {err}"))?
            .ok_or_else(|| format!("SQLite 会话不存在，conversation_id={}", paths.conversation_id))?;
        let operation_id = Uuid::new_v4().to_string();
        let operation_root = chat_metadata_operation_root(paths, &operation_id);
        let mut replaced_files = changed_blocks.iter().map(|block| block.block_file.clone()).collect::<Vec<_>>();
        replaced_files.sort();
        replaced_files.dedup();
        let retired_files = retired_block_ids.iter()
            .map(|block_id| format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"))
            .filter(|file| !replaced_files.contains(file))
            .collect::<Vec<_>>();
        let mut expected_block_files = conn
            .prepare(
                "SELECT block_file FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
            )
            .map_err(|err| format!("准备读取 SQLite 当前 block 失败: {err}"))?
            .query_map([&paths.conversation_id], |row| row.get::<_, String>(0))
            .map_err(|err| format!("读取 SQLite 当前 block 失败: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析 SQLite 当前 block 失败: {err}"))?;
        let existing_block_files = expected_block_files.clone();
        expected_block_files.retain(|file| {
            !replaced_files.contains(file) && !retired_files.contains(file)
        });
        expected_block_files.extend(replaced_files.iter().cloned());
        expected_block_files.sort();
        expected_block_files.dedup();
        let detail = ChatStorageOperationDetail {
            conversation_id: paths.conversation_id.clone(),
            expected_block_files,
            replaced_block_files: replaced_files.clone(),
            retired_block_files: retired_files.clone(),
            new_block_files: replaced_files.iter()
                .filter(|file| !existing_block_files.contains(file))
                .cloned()
                .collect(),
        };
        let detail_json = serde_json::to_string(&detail).map_err(|err| format!("序列化 SQLite 存储操作失败: {err}"))?;
        conn.execute(
            "INSERT INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at)
             VALUES(?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
            rusqlite::params![operation_id, paths.conversation_id, before_revision, before_revision + 1, detail_json, now_iso()],
        ).map_err(|err| format!("登记 SQLite 存储操作失败: {err}"))?;

        for file in replaced_files.iter().chain(retired_files.iter()) {
            let source = paths.shard_dir.join(file);
            if source.exists() {
                let backup = operation_root.join("old").join(file);
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent).map_err(|err| format!("创建 SQLite 操作备份目录失败，error={err}"))?;
                }
                fs::copy(&source, &backup).map_err(|err| format!("备份 SQLite 受影响 block 失败，path={}，error={err}", source.display()))?;
            }
        }
        for block in changed_blocks {
            let staged = operation_root.join("new").join(&block.block_file);
            write_jsonl_snapshot_block_v4_atomic(&staged, &block.content)?;
        }
        for block in changed_blocks {
            let staged = operation_root.join("new").join(&block.block_file);
            let compressed = fs::read(&staged)
                .map_err(|err| format!("读取 SQLite staged block 失败，path={}，error={err}", staged.display()))?;
            write_message_store_bytes_atomic(
                &paths.shard_dir.join(&block.block_file),
                "jsonl.zstd.tmp",
                &compressed,
                "V4 压缩块",
            )?;
        }

        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| format!("开启 SQLite block 发布事务失败: {err}"))?;
        let meta_json = serde_json::to_string(meta).map_err(|err| format!("序列化 SQLite metadata 失败: {err}"))?;
        let updated = transaction.execute(
            "UPDATE conversation_metadata SET metadata_json=?1, storage_revision=?2, updated_at=?3
             WHERE conversation_id=?4 AND storage_revision=?5",
            rusqlite::params![meta_json, before_revision + 1, meta.updated_at(), paths.conversation_id, before_revision],
        ).map_err(|err| format!("发布 SQLite metadata 失败: {err}"))?;
        if updated != 1 {
            return Err(format!("发布 SQLite metadata revision 冲突，conversation_id={}", paths.conversation_id));
        }
        let mut affected_ids = changed_blocks.iter().map(|block| block.block_id).collect::<Vec<_>>();
        affected_ids.extend_from_slice(retired_block_ids);
        affected_ids.sort();
        affected_ids.dedup();
        for block_id in affected_ids {
            transaction.execute(
                "DELETE FROM message_locator WHERE conversation_id=?1 AND block_id=?2",
                rusqlite::params![paths.conversation_id, block_id as i64],
            ).map_err(|err| format!("清理 SQLite 受影响 locator 失败: {err}"))?;
            transaction.execute(
                "DELETE FROM conversation_blocks WHERE conversation_id=?1 AND block_id=?2",
                rusqlite::params![paths.conversation_id, block_id as i64],
            ).map_err(|err| format!("清理 SQLite 受影响 block 失败: {err}"))?;
        }
        for block in changed_blocks {
            let staged = operation_root.join("new").join(&block.block_file);
            let compressed_len = fs::metadata(&staged)
                .map_err(|err| format!("读取 SQLite staged block 元数据失败，path={}，error={err}", staged.display()))?
                .len();
            transaction.execute(
                "INSERT INTO conversation_blocks(conversation_id, block_id, block_file, byte_len, message_count)
                 VALUES(?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![paths.conversation_id, block.block_id as i64, block.block_file, compressed_len as i64, block.index_items.len() as i64],
            ).map_err(|err| format!("写入 SQLite block 失败: {err}"))?;
        }
        for (sequence, item) in locator_rows {
            let block_id = item.block_id.ok_or_else(|| format!("SQLite locator 缺少 block id，message_id={}", item.message_id))?;
            transaction.execute(
                "INSERT INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![paths.conversation_id, sequence, item.message_id, block_id as i64, item.offset as i64, item.byte_len as i64, item.compaction_kind, item.role, item.created_at],
            ).map_err(|err| format!("写入 SQLite locator 失败: {err}"))?;
        }
        transaction.execute(
            "UPDATE storage_operations SET state='committed', committed_at=?1 WHERE operation_id=?2",
            rusqlite::params![now_iso(), operation_id],
        ).map_err(|err| format!("标记 SQLite 存储操作已提交失败: {err}"))?;
        transaction.commit().map_err(|err| format!("提交 SQLite block 发布事务失败: {err}"))?;

        let mut cleaned = true;
        for file in retired_files {
            let path = paths.shard_dir.join(&file);
            if path.exists() && fs::remove_file(&path).is_err() { cleaned = false; }
        }
        if operation_root.exists() && fs::remove_dir_all(&operation_root).is_err() { cleaned = false; }
        if cleaned {
            if let Err(err) = conn.execute("DELETE FROM storage_operations WHERE operation_id=?1 AND state='committed'", [&operation_id]) {
                runtime_log_info(format!("[消息存储] 清理已提交 SQLite operation 延后，operation_id={}，error={err}", operation_id));
            }
        }
    chat_metadata_store_write_result(&conn, paths)
}

fn chat_metadata_store_append_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if messages.is_empty() {
        return Err(format!("追加 SQLite 消息失败：消息为空，conversation_id={}", paths.conversation_id));
    }
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    chat_metadata_store_with_writer_gate(paths, || {
        let count = chat_metadata_store_message_count(paths)?;
        let last_block_id = chat_metadata_store_read_last_block_id(paths)?;
        let tail_rows = match last_block_id {
            Some(block_id) => chat_metadata_store_read_locators_for_block(paths, block_id)?,
            None => Vec::new(),
        };
        let mut known_ids = std::collections::HashSet::<String>::new();
        for message in messages {
            let id = message.id.trim();
            if id.is_empty() || !known_ids.insert(id.to_string()) || chat_metadata_store_read_locator_by_id(paths, id)?.is_some() {
                return Err(format!("追加 SQLite 消息失败：消息 ID 无效或重复，conversation_id={}，message_id={}", paths.conversation_id, message.id));
            }
        }
        let entries = messages.iter().map(|message| (meta, message)).collect::<Vec<_>>();
        // 追加前先对账尾块（torn/孤儿截断），否则整块解压读取会失败
        if let Some(tail_row) = tail_rows.last() {
            if let Some(block_id) = last_block_id {
                chat_metadata_store_reconcile_block_tail(
                    paths,
                    block_id,
                    tail_row.item.offset + tail_row.item.byte_len,
                    "追加 SQLite 消息失败",
                )?;
            }
        }
        // 只读尾行消息用于块规划，避免全读尾块（物理追加场景不需要旧行内容）
        let tail_last_message = match tail_rows.last() {
            Some(row) => read_jsonl_snapshot_messages_by_index_items(
                &paths.messages_file,
                std::slice::from_ref(&row.item),
            )?
            .into_iter()
            .next(),
            None => None,
        };
        let plan = plan_appended_message_blocks(tail_last_message.as_ref(), &entries);
        let existing_block_count = if last_block_id.is_some() { chat_metadata_store_read_block_count(paths)? } else { 0 };
        let total_block_count = existing_block_count + plan.groups.len() - usize::from(plan.continue_last_block && last_block_id.is_some());
        // 物理追加快速路径：延续尾块 + 单组新消息 + 非 slim（活跃会话普通追加）
        if plan.continue_last_block && plan.groups.len() == 1 {
            let block_id = last_block_id.ok_or_else(|| "追加 SQLite 消息失败：缺少尾 block".to_string())?;
            let should_slim = should_slim_conversation_block(
                meta.status.trim() == "archived",
                block_id as usize,
                total_block_count,
            );
            if !should_slim {
                let tail_row = tail_rows
                    .last()
                    .cloned()
                    .ok_or_else(|| "追加 SQLite 消息失败：尾 block 缺少消息".to_string())?;
                // 物理追加只插入新行 locator（不重插旧行），sequence 从尾行之后顺延
                let first_sequence = tail_rows.last().map(|row| row.sequence + 1).unwrap_or(count as i64);
                return chat_metadata_store_append_physical(
                    paths,
                    &shard_meta,
                    block_id,
                    &tail_row,
                    &plan.groups[0],
                    tail_rows.len(),
                    first_sequence,
                );
            }
        }
        let tail_messages = read_jsonl_snapshot_messages_by_index_items(
            &paths.messages_file,
            &tail_rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>(),
        )?;
        let mut changed_blocks = Vec::<JsonlSnapshotConversationBlock>::new();
        let mut rows = Vec::<(i64, MessageStoreIndexItem)>::new();
        let mut next_sequence = if plan.continue_last_block {
            tail_rows.first().map(|row| row.sequence).unwrap_or(count as i64)
        } else { count as i64 };
        if plan.continue_last_block {
            let block_id = last_block_id.ok_or_else(|| "追加 SQLite 消息失败：缺少尾 block".to_string())?;
            let mut merged = tail_messages;
            merged.extend(plan.groups.first().cloned().unwrap_or_default());
            let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
            let refs = ConversationBlockMessageRefs {
                block_id,
                block_file,
                messages: merged.iter().collect(),
            };
            let should_slim = should_slim_conversation_block(
                meta.status.trim() == "archived",
                block_id as usize,
                total_block_count,
            );
            let block = if should_slim {
                build_jsonl_snapshot_conversation_block_v4(&refs, true)?
            } else {
                // V4 尾部重建：解压整块 → 复用目标行之前的明文前缀，只重序列化新追加的消息
                let block_path = paths.shard_dir.join(&refs.block_file);
                let plain = read_jsonl_snapshot_block_plain_v4(&block_path)?;
                let prefix_items = tail_rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>();
                let prefix_end = prefix_items.last().map(|item| item.offset as usize + item.byte_len as usize).unwrap_or(0);
                let prefix_bytes = &plain.as_bytes()[..prefix_end.min(plain.len())];
                let new_messages = plan.groups.first().cloned().unwrap_or_default();
                let tail_refs = ConversationBlockMessageRefs {
                    block_id,
                    block_file: refs.block_file.clone(),
                    messages: new_messages.iter().collect(),
                };
                build_jsonl_snapshot_conversation_block_tail_v4(
                    &tail_refs,
                    prefix_bytes,
                    &prefix_items,
                    &new_messages,
                )?
            };
            for item in &block.index_items { rows.push((next_sequence, item.clone())); next_sequence += 1; }
            changed_blocks.push(block);
        }
        let first_new_group_index = usize::from(plan.continue_last_block);
        let new_block_ids = chat_metadata_store_allocate_new_block_ids(
            paths,
            last_block_id,
            plan.groups.len().saturating_sub(first_new_group_index),
        )?;
        for (index, group) in plan.groups.iter().enumerate().skip(first_new_group_index) {
            let block_id = new_block_ids[index - first_new_group_index];
            let refs = ConversationBlockMessageRefs {
                block_id,
                block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"),
                messages: group.iter().collect(),
            };
            let block = build_jsonl_snapshot_conversation_block_v4(&refs, should_slim_conversation_block(meta.status.trim() == "archived", block_id as usize, total_block_count))?;
            for item in &block.index_items { rows.push((next_sequence, item.clone())); next_sequence += 1; }
            changed_blocks.push(block);
        }
        chat_metadata_store_publish_blocks(paths, &shard_meta, &changed_blocks, &[], &rows)
    })
}

/// 物理追加：新消息直接写尾块文件尾，成功后更新 SQLite。
///
/// 适用场景：延续尾块 + 单组新消息 + 非 slim（活跃会话普通追加：用户消息、远程联系人消息）。
/// 与替换（全量重建）分开：追加不读旧数据、不重建，只在文件尾追加明文行；offset 从尾行末尾顺延。
/// 崩溃一致性：先写文件、后提交 SQLite；文件尾残留的孤儿字节（上次追加写成功但 SQLite 未提交）
/// 在下次追加前截断。不登记 storage_operations（追加是幂等自愈的，recover 机制不感知）。
fn chat_metadata_store_append_physical(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
    block_id: u32,
    tail_row: &ChatMetadataLocator,
    new_messages: &[ChatMessage],
    existing_block_message_count: usize,
    first_sequence: i64,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let publication_gate = chat_metadata_store_publication_gate(paths);
    let _publication_guard = publication_gate.write().unwrap_or_else(|poison| poison.into_inner());

    // 1. 序列化新消息为多行组明文，offset 从尾行末尾顺延（组级坐标）
    let mut new_plain = String::new();
    let mut items = Vec::<MessageStoreIndexItem>::with_capacity(new_messages.len());
    let mut offset = tail_row.item.offset + tail_row.item.byte_len;
    for message in new_messages {
        let lines = split_message_into_group_lines(message)?;
        let group_len = lines.iter().map(|line| line.as_bytes().len() as u64).sum::<u64>();
        let item = message_store_index_item_for_message_in_block(
            message,
            Some(block_id),
            offset,
            group_len,
        );
        if item.compaction_kind.is_none() && message_store_compaction_kind(message).is_some() {
            return Err(format!(
                "追加 SQLite 消息失败：追加消息带压缩边界但未开新块，message_id={}",
                message.id
            ));
        }
        for line in &lines {
            new_plain.push_str(line);
        }
        items.push(item);
        offset = offset.checked_add(group_len).ok_or_else(|| {
            format!(
                "追加 SQLite 消息失败：block offset 溢出，conversation_id={}，block_id={}",
                paths.conversation_id, block_id
            )
        })?;
    }

    // 2. 文件：追加前扫帧对账（torn/孤儿截断）+ 新组压帧 append
    let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
    let block_path = paths.shard_dir.join(&block_file);
    let expected_plain_len = tail_row.item.offset + tail_row.item.byte_len;
    let keep_len = chat_metadata_store_reconcile_block_tail(
        paths,
        block_id,
        expected_plain_len,
        "追加 SQLite 消息失败",
    )?;
    let new_frame = zstd_compress_frame(new_plain.as_bytes())?;
    {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&block_path)
            .map_err(|err| {
                format!(
                    "追加 SQLite 消息失败：打开块文件失败（追加），path={}，error={err}",
                    block_path.display()
                )
            })?;
        file.write_all(&new_frame).map_err(|err| {
            format!(
                "追加 SQLite 消息失败：追加写入块文件失败，path={}，error={err}",
                block_path.display()
            )
        })?;
    }

    // 3. SQLite 事务：更新块元数据（byte_len=压缩文件长度）、插入组级 locator、递增 revision
    let mut conn = chat_metadata_store_open(&paths.data_path)?;
    let before_revision = conn
        .query_row(
            "SELECT storage_revision FROM conversation_metadata WHERE conversation_id=?1",
            [&paths.conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| format!("读取 SQLite storage revision 失败: {err}"))?
        .ok_or_else(|| format!("SQLite 会话不存在，conversation_id={}", paths.conversation_id))?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("开启 SQLite 追加事务失败: {err}"))?;
    let new_message_count = existing_block_message_count + new_messages.len();
    let new_byte_len = keep_len as u64 + new_frame.len() as u64;
    let updated_block = transaction
        .execute(
            "UPDATE conversation_blocks SET byte_len=?1, message_count=?2 WHERE conversation_id=?3 AND block_id=?4",
            rusqlite::params![new_byte_len as i64, new_message_count as i64, paths.conversation_id, block_id as i64],
        )
        .map_err(|err| format!("更新 SQLite block 失败: {err}"))?;
    if updated_block != 1 {
        return Err(format!(
            "追加 SQLite 消息失败：block 不存在，conversation_id={}，block_id={}",
            paths.conversation_id, block_id
        ));
    }
    let meta_json = serde_json::to_string(meta).map_err(|err| format!("序列化 SQLite metadata 失败: {err}"))?;
    let updated = transaction
        .execute(
            "UPDATE conversation_metadata SET metadata_json=?1, storage_revision=?2, updated_at=?3
             WHERE conversation_id=?4 AND storage_revision=?5",
            rusqlite::params![
                meta_json,
                before_revision + 1,
                meta.updated_at(),
                paths.conversation_id,
                before_revision
            ],
        )
        .map_err(|err| format!("发布 SQLite metadata 失败: {err}"))?;
    if updated != 1 {
        return Err(format!(
            "追加 SQLite 消息失败：metadata revision 冲突，conversation_id={}",
            paths.conversation_id
        ));
    }
    let mut sequence = first_sequence;
    for item in &items {
        transaction
            .execute(
                "INSERT INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    paths.conversation_id,
                    sequence,
                    item.message_id,
                    block_id as i64,
                    item.offset as i64,
                    item.byte_len as i64,
                    item.compaction_kind,
                    item.role,
                    item.created_at
                ],
            )
            .map_err(|err| format!("写入 SQLite locator 失败: {err}"))?;
        sequence += 1;
    }
    transaction.commit().map_err(|err| format!("提交 SQLite 追加事务失败: {err}"))?;
    chat_metadata_store_write_result(&conn, paths)
}

/// 组内追加子行（D14：工具事件/正文累积不再走 replace）：
/// 目标组必须是块尾组（帧追加的物理约束）。追加前扫帧对账（torn/孤儿截断），
/// 新行明文压 zstd 帧 append 到文件尾；locator 只更新目标组的 byte_len
/// （offset 不变、message_count 不变、sequence 不变）；块 byte_len 更新为压缩文件长度。
///
/// 前置条件：`target_row` 是块内最后一个 locator（调用方保证），`new_lines` 是
/// 目标组新增的子行明文（工具行或正文行，含换行符）。调用方还需保证目标组形态
/// 与新增行匹配（工具事件追加工具行、正文累积追加正文行）。
fn chat_metadata_store_append_line_to_group_physical(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
    block_id: u32,
    target_row: &ChatMetadataLocator,
    new_lines: &[String],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let publication_gate = chat_metadata_store_publication_gate(paths);
    let _publication_guard = publication_gate.write().unwrap_or_else(|poison| poison.into_inner());

    // 1. 序列化新增子行为明文，校验目标组在块尾（组尾 = 块明文尾）
    let mut new_plain = String::new();
    for line in new_lines {
        new_plain.push_str(line);
    }
    if new_plain.is_empty() {
        return Err(format!(
            "组内追加子行失败：新增行为空，conversation_id={}，block_id={}，message_id={}",
            paths.conversation_id, block_id, target_row.item.message_id
        ));
    }

    // 2. 文件：追加前扫帧对账（torn/孤儿截断）+ 新行压帧 append
    let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
    let block_path = paths.shard_dir.join(&block_file);
    let expected_plain_len = target_row.item.offset + target_row.item.byte_len;
    let raw = std::fs::read(&block_path).map_err(|err| {
        format!(
            "组内追加子行失败：读取块文件失败，path={}，error={err}",
            block_path.display()
        )
    })?;
    let keep_len = zstd_validate_tail_for_append(&raw, expected_plain_len as usize)?;
    if keep_len < raw.len() {
        runtime_log_warn(format!(
            "[消息存储] 组内追加前清理孤儿/损坏尾部，conversation_id={}，block_id={}，path={}，file_len={}，keep_len={}",
            paths.conversation_id,
            block_id,
            block_path.display(),
            raw.len(),
            keep_len
        ));
        fs::OpenOptions::new()
            .write(true)
            .open(&block_path)
            .map_err(|err| {
                format!(
                    "组内追加子行失败：打开块文件失败（清理孤儿），path={}，error={err}",
                    block_path.display()
                )
            })?
            .set_len(keep_len as u64)
            .map_err(|err| {
                format!(
                    "组内追加子行失败：截断块文件失败（清理孤儿），path={}，error={err}",
                    block_path.display()
                )
            })?;
    }
    let new_frame = zstd_compress_frame(new_plain.as_bytes())?;
    {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&block_path)
            .map_err(|err| {
                format!(
                    "组内追加子行失败：打开块文件失败（追加），path={}，error={err}",
                    block_path.display()
                )
            })?;
        file.write_all(&new_frame).map_err(|err| {
            format!(
                "组内追加子行失败：追加写入块文件失败，path={}，error={err}",
                block_path.display()
            )
        })?;
    }

    // 3. SQLite 事务：更新目标组 locator byte_len（offset 不变）+ 块 byte_len + revision
    let mut conn = chat_metadata_store_open(&paths.data_path)?;
    let before_revision = conn
        .query_row(
            "SELECT storage_revision FROM conversation_metadata WHERE conversation_id=?1",
            [&paths.conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| format!("读取 SQLite storage revision 失败: {err}"))?
        .ok_or_else(|| format!("SQLite 会话不存在，conversation_id={}", paths.conversation_id))?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("开启 SQLite 组内追加事务失败: {err}"))?;
    let new_locator_byte_len = target_row.item.byte_len + new_plain.as_bytes().len() as u64;
    let updated_locator = transaction
        .execute(
            "UPDATE message_locator SET byte_len=?1 WHERE conversation_id=?2 AND sequence=?3",
            rusqlite::params![
                new_locator_byte_len as i64,
                paths.conversation_id,
                target_row.sequence
            ],
        )
        .map_err(|err| format!("更新 SQLite locator byte_len 失败: {err}"))?;
    if updated_locator != 1 {
        return Err(format!(
            "组内追加子行失败：locator 不存在，conversation_id={}，sequence={}",
            paths.conversation_id, target_row.sequence
        ));
    }
    let new_block_byte_len = keep_len as u64 + new_frame.len() as u64;
    let updated_block = transaction
        .execute(
            "UPDATE conversation_blocks SET byte_len=?1 WHERE conversation_id=?2 AND block_id=?3",
            rusqlite::params![
                new_block_byte_len as i64,
                paths.conversation_id,
                block_id as i64
            ],
        )
        .map_err(|err| format!("更新 SQLite block byte_len 失败: {err}"))?;
    if updated_block != 1 {
        return Err(format!(
            "组内追加子行失败：block 不存在，conversation_id={}，block_id={}",
            paths.conversation_id, block_id
        ));
    }
    let meta_json = serde_json::to_string(meta).map_err(|err| format!("序列化 SQLite metadata 失败: {err}"))?;
    let updated = transaction
        .execute(
            "UPDATE conversation_metadata SET metadata_json=?1, storage_revision=?2, updated_at=?3
             WHERE conversation_id=?4 AND storage_revision=?5",
            rusqlite::params![
                meta_json,
                before_revision + 1,
                meta.updated_at(),
                paths.conversation_id,
                before_revision
            ],
        )
        .map_err(|err| format!("发布 SQLite metadata 失败: {err}"))?;
    if updated != 1 {
        return Err(format!(
            "组内追加子行失败：metadata revision 冲突，conversation_id={}",
            paths.conversation_id
        ));
    }
    transaction.commit().map_err(|err| format!("提交 SQLite 组内追加事务失败: {err}"))?;
    chat_metadata_store_write_result(&conn, paths)
}

fn chat_metadata_store_read_block_count(paths: &MessageStorePaths) -> Result<usize, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    conn.query_row(
        "SELECT COUNT(*) FROM conversation_blocks WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).map(|value| value as usize).map_err(|err| format!("统计 SQLite block 失败: {err}"))
}

fn chat_metadata_store_read_locator_at_sequence(
    paths: &MessageStorePaths,
    sequence: i64,
) -> Result<Option<ChatMetadataLocator>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let mut rows = chat_metadata_store_query_locators(
        &conn,
        &paths.conversation_id,
        "AND sequence=?2",
        &[&sequence],
    )?;
    Ok(rows.pop())
}

fn chat_metadata_store_read_block_ids_after(
    paths: &MessageStorePaths,
    block_id: u32,
) -> Result<Vec<u32>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let mut statement = conn.prepare(
        "SELECT block_id FROM conversation_blocks WHERE conversation_id=?1 AND block_id>?2 ORDER BY block_id ASC",
    ).map_err(|err| format!("准备读取 SQLite 后续 block 失败: {err}"))?;
    let rows = statement.query_map(rusqlite::params![paths.conversation_id, block_id as i64], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("读取 SQLite 后续 block 失败: {err}"))?;
    rows.map(|row| row.map(|value| value as u32).map_err(|err| format!("解析 SQLite 后续 block 失败: {err}")))
        .collect()
}

fn chat_metadata_store_read_all_block_ids(paths: &MessageStorePaths) -> Result<Vec<u32>, String> {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let mut statement = conn.prepare(
        "SELECT block_id FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
    ).map_err(|err| format!("准备读取 SQLite block 列表失败: {err}"))?;
    let rows = statement.query_map([&paths.conversation_id], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("读取 SQLite block 列表失败: {err}"))?;
    rows.map(|row| row.map(|value| value as u32).map_err(|err| format!("解析 SQLite block 列表失败: {err}")))
        .collect()
}

fn chat_metadata_store_splice_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    start_index: usize,
    delete_count: usize,
    inserted_messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if start_index == 0 && delete_count == 0 && chat_metadata_store_message_count(paths)? == 0 {
        return chat_metadata_store_append_messages(paths, meta, inserted_messages);
    }
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    chat_metadata_store_with_writer_gate(paths, || {
        let count = chat_metadata_store_message_count(paths)?;
        if start_index > count || delete_count > count.saturating_sub(start_index) {
            return Err(format!("拼接 SQLite 消息失败：范围越界，conversation_id={}，start_index={start_index}，delete_count={delete_count}，message_count={count}", paths.conversation_id));
        }
        if start_index == count && delete_count == 0 {
            return chat_metadata_store_append_messages_unlocked(paths, meta, inserted_messages);
        }
        let start_locator = chat_metadata_store_read_locator_at_sequence(paths, start_index as i64)?
            .ok_or_else(|| format!("拼接 SQLite 消息失败：起始位置不存在，conversation_id={}", paths.conversation_id))?;
        let start_block_id = start_locator.item.block_id.ok_or_else(|| "拼接 SQLite 消息失败：起始 locator 缺少 block id".to_string())?;
        let start_block_rows = chat_metadata_store_read_locators_for_block(paths, start_block_id)?;
        let first_sequence = start_block_rows.first().map(|row| row.sequence)
            .ok_or_else(|| format!("拼接 SQLite 消息失败：起始 block 为空，conversation_id={}", paths.conversation_id))?;
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let suffix_rows = chat_metadata_store_query_locators(&conn, &paths.conversation_id, "AND sequence>=?2", &[&first_sequence])?;
        let mut suffix_messages = chat_metadata_store_read_messages_for_locators(paths, &suffix_rows, false)?;
        let relative_start = start_index.saturating_sub(first_sequence as usize);
        if relative_start > suffix_messages.len() || delete_count > suffix_messages.len().saturating_sub(relative_start) {
            return Err(format!("拼接 SQLite 消息失败：受影响 block 范围不一致，conversation_id={}", paths.conversation_id));
        }
        for message in inserted_messages {
            let id = message.id.trim();
            if id.is_empty() {
                return Err(format!("拼接 SQLite 消息失败：插入消息 ID 为空，conversation_id={}", paths.conversation_id));
            }
            if suffix_messages.iter().enumerate().filter(|(index, _)| *index < relative_start || *index >= relative_start + delete_count)
                .any(|(_, existing)| existing.id.trim() == id) {
                return Err(format!("拼接 SQLite 消息失败：消息 ID 冲突，conversation_id={}，message_id={id}", paths.conversation_id));
            }
            if let Some(existing) = chat_metadata_store_read_locator_by_id(paths, id)? {
                if existing.sequence < start_index as i64 || existing.sequence >= (start_index + delete_count) as i64 {
                    return Err(format!("拼接 SQLite 消息失败：消息 ID 冲突，conversation_id={}，message_id={id}", paths.conversation_id));
                }
            }
        }
        suffix_messages.splice(relative_start..relative_start + delete_count, inserted_messages.iter().cloned());
        let raw_blocks = split_messages_into_conversation_blocks(&suffix_messages);
        let old_block_ids = suffix_rows.iter().filter_map(|row| row.item.block_id).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>();
        let rebuilt_block_ids = chat_metadata_store_allocate_rebuilt_block_ids(
            paths,
            &old_block_ids,
            raw_blocks.len(),
        )?;
        let all_block_count = chat_metadata_store_read_block_count(paths)?;
        let prefix_block_count = all_block_count.saturating_sub(old_block_ids.len());
        let mut changed_blocks = Vec::new();
        let mut rows = Vec::new();
        let mut sequence = first_sequence;
        let force_slim_closed_remote_blocks = meta.conversation_kind
            == CONVERSATION_KIND_REMOTE_IM_CONTACT
            && inserted_messages.iter().any(|message| message_store_compaction_kind(message).is_some());
        for (offset, source) in raw_blocks.iter().enumerate() {
            let block_id = rebuilt_block_ids[offset];
            let refs = ConversationBlockMessageRefs {
                block_id,
                block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"),
                messages: source.messages.clone(),
            };
            let absolute_block_index = prefix_block_count + offset;
            let total_rebuilt_block_count = prefix_block_count + raw_blocks.len();
            let should_slim = should_slim_spliced_block(
                force_slim_closed_remote_blocks,
                meta.status.trim() == "archived",
                absolute_block_index,
                total_rebuilt_block_count,
            );
            let block = build_jsonl_snapshot_conversation_block_v4(
                &refs,
                should_slim,
            )?;
            for item in &block.index_items {
                rows.push((sequence, item.clone()));
                sequence += 1;
            }
            changed_blocks.push(block);
        }
        chat_metadata_store_publish_blocks(paths, &shard_meta, &changed_blocks, &old_block_ids, &rows)
    })
}

fn should_slim_spliced_block(
    force_slim_closed_remote_blocks: bool,
    archived_conversation: bool,
    block_index: usize,
    block_count: usize,
) -> bool {
    if force_slim_closed_remote_blocks {
        return block_index < block_count.saturating_sub(1);
    }
    should_slim_conversation_block(archived_conversation, block_index, block_count)
}

#[cfg(test)]
#[test]
fn remote_wake_splice_should_slim_every_closed_block_immediately() {
    assert!(should_slim_spliced_block(true, false, 0, 2));
    assert!(!should_slim_spliced_block(true, false, 1, 2));
    assert!(!should_slim_spliced_block(false, false, 0, 2));
}

#[cfg(test)]
#[test]
fn remote_wake_splice_should_preserve_trigger_in_new_block_and_slim_old_block() {
    let root = std::env::temp_dir().join(format!("eca-remote-wake-splice-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let message = |id: &str, role: &str, text: &str| ChatMessage {
        id: id.to_string(),
        role: role.to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text {
            text: text.to_string(),
            reasoning_content: Some("不应保留的思维链".to_string()),
        }],
        extra_text_blocks: vec!["工具细节".to_string()],
        provider_meta: Some(serde_json::json!({"large": true})),
        tool_call: Some(vec![
            serde_json::json!({"type": "tool_call", "name": "tool"}),
            serde_json::json!({"type": "tool_result", "name": "tool"}),
        ]),
        mcp_call: None,
        meme_annotations: None,
    };
    let old_assistant = message("old-assistant", "assistant", "旧回答");
    let old_user = message("old-user", "user", "旧问题");
    let trigger = message("trigger", "user", "现在需要回答");
    let following = message("following", "user", "同批后续消息");
    let conversation = Conversation {
        id: "remote-wake-conversation".to_string(),
        title: "remote wake".to_string(),
        agent_id: DEFAULT_AGENT_ID.to_string(),
        bound_conversation_id: None,
        parent_conversation_id: None,
        child_conversation_ids: Vec::new(),
        fork_message_cursor: None,
        unread_count: 0,
        conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
        root_conversation_id: None,
        delegate_id: None,
        created_at: now_iso(),
        updated_at: now_iso(),
        last_user_at: None,
        last_assistant_at: None,
        status: "active".to_string(),
        user_profile_snapshot: String::new(),
        shell_workspace_path: None,
        shell_workspaces: Vec::new(),
        shell_autonomous_mode: false,
        shell_work_mode: default_shell_work_mode(),
        shell_work_branch: String::new(),
        archived_at: None,
        messages: vec![
            old_assistant.clone(),
            old_user.clone(),
            trigger.clone(),
            following.clone(),
        ],
        fast_request_turns: Vec::new(),
        current_todos: Vec::new(),
        memory_recall_table: Vec::new(),
        plan_mode_enabled: false,
        preferred_api_config_id: None,
        auto_push_remote_contact_id: None,
        active_goal: None, last_error: None,
        cumulative_usage: ConversationCumulativeUsage::default(),
        is_draft: false,
    };
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write fixture");
    chat_metadata_store_run_v3_migration(&data_path).expect("migrate v3");
    migration_v3_to_v4(&data_path, None).expect("migrate v4");
    let mut summary = message("wake-summary", "user", "远程唤醒上下文");
    summary.provider_meta = Some(serde_json::json!({
        "message_meta": { "kind": "context_compaction" }
    }));
    let after = Conversation {
        messages: vec![
            old_assistant,
            old_user,
            summary.clone(),
            trigger.clone(),
            following.clone(),
        ],
        ..conversation
    };
    chat_metadata_store_splice_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&after),
        2,
        1,
        &[summary.clone(), trigger.clone()],
    )
    .expect("splice remote wake");
    let old = chat_metadata_store_read_message_by_id(&paths, "old-assistant")
        .expect("read slimmed old assistant");
    assert!(old.tool_call.is_none());
    assert!(old.extra_text_blocks.is_empty());
    let summary_sequence = chat_metadata_store_read_message_sequence(&paths, "wake-summary")
        .expect("read summary sequence")
        .expect("summary sequence");
    assert_eq!(summary_sequence, 2);
    let following_after = chat_metadata_store_read_message_by_id(&paths, "following")
        .expect("read following message");
    assert_eq!(following_after.id, "following");
    assert_eq!(
        chat_metadata_store_compaction_segment(&paths, None)
            .expect("current block")
            .messages
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        vec!["wake-summary", "trigger", "following"]
    );
    let _ = fs::remove_dir_all(root);
}

fn chat_metadata_store_append_messages_unlocked(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if messages.is_empty() {
        return Err(format!("追加 SQLite 消息失败：消息为空，conversation_id={}", paths.conversation_id));
    }
    let count = chat_metadata_store_message_count(paths)?;
    let last_block_id = chat_metadata_store_read_last_block_id(paths)?;
    let tail_rows = match last_block_id { Some(block_id) => chat_metadata_store_read_locators_for_block(paths, block_id)?, None => Vec::new() };
    let tail_messages = chat_metadata_store_read_messages_for_locators(paths, &tail_rows, false)?;
    let mut known_ids = std::collections::HashSet::<String>::new();
    for message in messages {
        let id = message.id.trim();
        if id.is_empty() || !known_ids.insert(id.to_string()) || chat_metadata_store_read_locator_by_id(paths, id)?.is_some() {
            return Err(format!("追加 SQLite 消息失败：消息 ID 无效或重复，conversation_id={}，message_id={}", paths.conversation_id, message.id));
        }
    }
    let entries = messages.iter().map(|message| (meta, message)).collect::<Vec<_>>();
    let plan = plan_appended_message_blocks(tail_messages.last(), &entries);
    let existing_block_count = if last_block_id.is_some() { chat_metadata_store_read_block_count(paths)? } else { 0 };
    let total_block_count = existing_block_count + plan.groups.len() - usize::from(plan.continue_last_block && last_block_id.is_some());
    let mut changed_blocks = Vec::<JsonlSnapshotConversationBlock>::new();
    let mut rows = Vec::<(i64, MessageStoreIndexItem)>::new();
    let mut next_sequence = if plan.continue_last_block { tail_rows.first().map(|row| row.sequence).unwrap_or(count as i64) } else { count as i64 };
    if plan.continue_last_block {
        let block_id = last_block_id.ok_or_else(|| "追加 SQLite 消息失败：缺少尾 block".to_string())?;
        let mut merged = tail_messages;
        merged.extend(plan.groups.first().cloned().unwrap_or_default());
        let refs = ConversationBlockMessageRefs { block_id, block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"), messages: merged.iter().collect() };
        let block = build_jsonl_snapshot_conversation_block_v4(&refs, should_slim_conversation_block(meta.status.trim() == "archived", block_id as usize, total_block_count))?;
        for item in &block.index_items { rows.push((next_sequence, item.clone())); next_sequence += 1; }
        changed_blocks.push(block);
    }
    let first_new_group_index = usize::from(plan.continue_last_block);
    let new_block_ids = chat_metadata_store_allocate_new_block_ids(
        paths,
        last_block_id,
        plan.groups.len().saturating_sub(first_new_group_index),
    )?;
    for (index, group) in plan.groups.iter().enumerate().skip(first_new_group_index) {
        let block_id = new_block_ids[index - first_new_group_index];
        let refs = ConversationBlockMessageRefs { block_id, block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"), messages: group.iter().collect() };
        let block = build_jsonl_snapshot_conversation_block_v4(&refs, should_slim_conversation_block(meta.status.trim() == "archived", block_id as usize, total_block_count))?;
        for item in &block.index_items { rows.push((next_sequence, item.clone())); next_sequence += 1; }
        changed_blocks.push(block);
    }
    chat_metadata_store_publish_blocks(paths, &ConversationShardMeta::from_persist_meta(meta), &changed_blocks, &[], &rows)
}

fn chat_metadata_store_replace_message(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    message: &ChatMessage,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    chat_metadata_store_with_writer_gate(paths, || {
        let locator = chat_metadata_store_read_locator_by_id(paths, message.id.trim())?
            .ok_or_else(|| format!("替换 SQLite 消息失败：消息不存在，conversation_id={}，message_id={}", paths.conversation_id, message.id))?;
        let block_id = locator.item.block_id.ok_or_else(|| "替换 SQLite 消息失败：locator 缺少 block id".to_string())?;
        let block_rows = chat_metadata_store_read_locators_for_block(paths, block_id)?;
        let block_count = chat_metadata_store_read_block_count(paths)?;
        let should_slim = should_slim_conversation_block(
            meta.status.trim() == "archived",
            block_id as usize,
            block_count,
        );
        if !should_slim {
            // 尾部重建：解压整块 → 目标行之前的明文前缀原样复用，只重新序列化目标行及之后
            let position = block_rows.iter().position(|row| row.item.message_id.trim() == message.id.trim())
                .ok_or_else(|| format!("替换 SQLite 消息失败：block 中找不到消息，conversation_id={}，message_id={}", paths.conversation_id, message.id))?;
            let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
            let block_path = paths.shard_dir.join(&block_file);
            let plain = read_jsonl_snapshot_block_plain_v4(&block_path)?;
            let target_offset = usize::try_from(block_rows[position].item.offset).map_err(|_| {
                format!(
                    "替换 SQLite 消息失败：消息偏移超出 usize 范围，conversation_id={}，message_id={}，offset={}",
                    paths.conversation_id,
                    message.id.trim(),
                    block_rows[position].item.offset
                )
            })?;
            if target_offset > plain.len() {
                return Err(format!(
                    "替换 SQLite 消息失败：消息偏移超出块明文长度，conversation_id={}，message_id={}，offset={}，plain_len={}，path={}",
                    paths.conversation_id,
                    message.id.trim(),
                    target_offset,
                    plain.len(),
                    block_path.display()
                ));
            }
            let prefix_bytes = &plain.as_bytes()[0..target_offset];
            let prefix_items = block_rows[..position].iter().map(|row| row.item.clone()).collect::<Vec<_>>();
            let tail_items = block_rows[position..].iter().map(|row| row.item.clone()).collect::<Vec<_>>();
            let mut tail_messages = read_jsonl_snapshot_messages_by_index_items(
                &paths.messages_file,
                &tail_items,
            )?;
            tail_messages[0] = message.clone();
            let refs = ConversationBlockMessageRefs {
                block_id,
                block_file,
                messages: tail_messages.iter().collect(),
            };
            let rebuilt = build_jsonl_snapshot_conversation_block_tail_v4(
                &refs,
                prefix_bytes,
                &prefix_items,
                &tail_messages,
            )?;
            let rows = rebuilt.index_items.iter().enumerate()
                .map(|(index, item)| (block_rows[index].sequence, item.clone()))
                .collect::<Vec<_>>();
            return chat_metadata_store_publish_blocks(paths, &shard_meta, &[rebuilt], &[], &rows);
        }
        let mut messages = read_jsonl_snapshot_messages_by_index_items(
            &paths.messages_file,
            &block_rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>(),
        )?;
        let position = messages.iter().position(|item| item.id.trim() == message.id.trim())
            .ok_or_else(|| format!("替换 SQLite 消息失败：block 中找不到消息，conversation_id={}，message_id={}", paths.conversation_id, message.id))?;
        messages[position] = message.clone();
        let refs = ConversationBlockMessageRefs {
            block_id,
            block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"),
            messages: messages.iter().collect(),
        };
        let rebuilt = build_jsonl_snapshot_conversation_block_v4(
            &refs,
            should_slim_conversation_block(meta.status.trim() == "archived", block_id as usize, block_count),
        )?;
        let rows = rebuilt.index_items.iter().enumerate()
            .map(|(index, item)| (block_rows[index].sequence, item.clone()))
            .collect::<Vec<_>>();
        chat_metadata_store_publish_blocks(paths, &shard_meta, &[rebuilt], &[], &rows)
    })
}

fn chat_metadata_store_replace_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if messages.is_empty() {
        return Err(format!("批量替换 SQLite 消息失败：消息为空，conversation_id={}", paths.conversation_id));
    }
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    chat_metadata_store_with_writer_gate(paths, || {
        let mut replacements_by_block = std::collections::BTreeMap::<u32, Vec<&ChatMessage>>::new();
        let mut message_ids = std::collections::HashSet::<String>::new();
        for message in messages {
            let message_id = message.id.trim();
            if message_id.is_empty() || !message_ids.insert(message_id.to_string()) {
                return Err(format!("批量替换 SQLite 消息失败：消息 ID 为空或重复，conversation_id={}，message_id={}", paths.conversation_id, message.id));
            }
            let locator = chat_metadata_store_read_locator_by_id(paths, message_id)?
                .ok_or_else(|| format!("批量替换 SQLite 消息失败：消息不存在，conversation_id={}，message_id={message_id}", paths.conversation_id))?;
            let block_id = locator.item.block_id
                .ok_or_else(|| format!("批量替换 SQLite 消息失败：locator 缺少 block id，message_id={message_id}"))?;
            replacements_by_block.entry(block_id).or_default().push(message);
        }
        let block_count = chat_metadata_store_read_block_count(paths)?;
        let mut changed_blocks = Vec::with_capacity(replacements_by_block.len());
        let mut rows = Vec::new();
        for (block_id, replacements) in replacements_by_block {
            let block_rows = chat_metadata_store_read_locators_for_block(paths, block_id)?;
            let mut block_messages = read_jsonl_snapshot_messages_by_index_items(
                &paths.messages_file,
                &block_rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>(),
            )?;
            for replacement in replacements {
                let position = block_messages.iter().position(|message| message.id.trim() == replacement.id.trim())
                    .ok_or_else(|| format!("批量替换 SQLite 消息失败：block 中找不到消息，conversation_id={}，message_id={}", paths.conversation_id, replacement.id))?;
                block_messages[position] = replacement.clone();
            }
            let refs = ConversationBlockMessageRefs {
                block_id,
                block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"),
                messages: block_messages.iter().collect(),
            };
            let rebuilt = build_jsonl_snapshot_conversation_block_v4(
                &refs,
                should_slim_conversation_block(meta.status.trim() == "archived", block_id as usize, block_count),
            )?;
            for (index, item) in rebuilt.index_items.iter().enumerate() {
                rows.push((block_rows[index].sequence, item.clone()));
            }
            changed_blocks.push(rebuilt);
        }
        chat_metadata_store_publish_blocks(paths, &shard_meta, &changed_blocks, &[], &rows)
    })
}

fn chat_metadata_store_truncate_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    keep_count: usize,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    chat_metadata_store_with_writer_gate(paths, || {
        let count = chat_metadata_store_message_count(paths)?;
        if keep_count > count {
            return Err(format!("截断 SQLite 消息失败：保留数量超过当前消息数，conversation_id={}，keep_count={keep_count}，message_count={count}", paths.conversation_id));
        }
        if keep_count == 0 {
            return chat_metadata_store_publish_blocks(paths, &shard_meta, &[], &chat_metadata_store_read_all_block_ids(paths)?, &[]);
        }
        let last = chat_metadata_store_read_locator_at_sequence(paths, keep_count as i64 - 1)?
            .ok_or_else(|| format!("截断 SQLite 消息失败：保留位置不存在，conversation_id={}", paths.conversation_id))?;
        let block_id = last.item.block_id.ok_or_else(|| "截断 SQLite 消息失败：locator 缺少 block id".to_string())?;
        let block_rows = chat_metadata_store_read_locators_for_block(paths, block_id)?;
        let kept_rows = block_rows.iter().filter(|row| row.sequence < keep_count as i64).cloned().collect::<Vec<_>>();
        let retired = chat_metadata_store_read_block_ids_after(paths, block_id)?;
        if kept_rows.len() == block_rows.len() {
            return chat_metadata_store_publish_blocks(paths, &shard_meta, &[], &retired, &[]);
        }
        let kept_messages = read_jsonl_snapshot_messages_by_index_items(
            &paths.messages_file,
            &kept_rows.iter().map(|row| row.item.clone()).collect::<Vec<_>>(),
        )?;
        let block_count = chat_metadata_store_read_block_count(paths)?;
        let refs = ConversationBlockMessageRefs {
            block_id,
            block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd"),
            messages: kept_messages.iter().collect(),
        };
        let rebuilt = build_jsonl_snapshot_conversation_block_v4(
            &refs,
            should_slim_conversation_block(meta.status.trim() == "archived", block_id as usize, block_count.saturating_sub(retired.len())),
        )?;
        let rows = rebuilt.index_items.iter().enumerate()
            .map(|(index, item)| (kept_rows[index].sequence, item.clone()))
            .collect::<Vec<_>>();
        chat_metadata_store_publish_blocks(paths, &shard_meta, &[rebuilt], &retired, &rows)
    })
}

#[cfg(test)]
fn chat_metadata_store_import_v2_conversation(paths: &MessageStorePaths) -> Result<(), String> {
    migration_v2_to_v3_conversation(paths).map_err(|failure| match failure {
        MigrationV2ToV3Failure::ConversationSkipped(err)
        | MigrationV2ToV3Failure::SystemFailure(err) => err,
    })
}

#[cfg(test)]
pub(super) fn chat_metadata_store_run_v3_migration(data_path: &PathBuf) -> Result<(), String> {
    migration_v2_to_v3(data_path, None)
}

fn chat_metadata_store_compaction_segment(
    paths: &MessageStorePaths,
    boundary_message_id: Option<&str>,
) -> Result<MessageStoreCompactionSegment, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let (start_locator, end_sequence) = match boundary_message_id {
        Some(message_id) => {
            let locator = chat_metadata_store_read_locator_by_id(paths, message_id)?
                .ok_or_else(|| format!("Compaction boundary not found: {}", message_id.trim()))?;
            if locator.item.compaction_kind.is_none() {
                return Err(format!("Message is not a compaction boundary: {}", message_id.trim()));
            }
            let previous = conn.query_row(
                "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
                 FROM message_locator WHERE conversation_id=?1 AND compaction_kind IS NOT NULL AND sequence<?2 ORDER BY sequence DESC LIMIT 1",
                rusqlite::params![paths.conversation_id, locator.sequence],
                |row| Ok(ChatMetadataLocator { sequence: row.get(0)?, item: MessageStoreIndexItem {
                    message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32), offset: row.get::<_, i64>(3)? as u64,
                    byte_len: row.get::<_, i64>(4)? as u64, compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
                }}),
            ).optional().map_err(|err| format!("读取 SQLite 前一压缩边界失败: {err}"))?;
            (previous, Some(locator.sequence))
        }
        None => {
            let latest = conn.query_row(
            "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
             FROM message_locator WHERE conversation_id=?1 AND compaction_kind IS NOT NULL ORDER BY sequence DESC LIMIT 1",
            [&paths.conversation_id],
            |row| Ok(ChatMetadataLocator { sequence: row.get(0)?, item: MessageStoreIndexItem {
                message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32), offset: row.get::<_, i64>(3)? as u64,
                byte_len: row.get::<_, i64>(4)? as u64, compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
            }}),
            ).optional().map_err(|err| format!("读取 SQLite 最新压缩边界失败: {err}"))?;
            (latest, None)
        }
    };
    let start_sequence = start_locator.as_ref().map(|locator| locator.sequence).unwrap_or(0);
    let previous_boundary = match start_locator.as_ref() {
        Some(start) => conn.query_row(
            "SELECT message_id FROM message_locator WHERE conversation_id=?1 AND compaction_kind IS NOT NULL AND sequence<?2 ORDER BY sequence DESC LIMIT 1",
            rusqlite::params![paths.conversation_id, start.sequence], |row| row.get::<_, String>(0),
        ).optional().map_err(|err| format!("读取 SQLite 更早压缩边界失败: {err}"))?,
        None => None,
    };
    let predicate = if end_sequence.is_some() { "AND sequence>=?2 AND sequence<?3" } else { "AND sequence>=?2" };
    let mut values: Vec<&dyn rusqlite::ToSql> = vec![&start_sequence];
    if let Some(end) = end_sequence.as_ref() { values.push(end); }
    let locators = chat_metadata_store_query_locators(&conn, &paths.conversation_id, predicate, &values)?;
    Ok(MessageStoreCompactionSegment {
        messages: chat_metadata_store_read_messages_for_locators(paths, &locators, false)?,
        boundary_message_id: start_locator.as_ref().map(|locator| locator.item.message_id.trim().to_string()),
        previous_boundary_message_id: previous_boundary,
        has_previous_segment: start_sequence > 0,
    })
    })
}

fn chat_metadata_store_compaction_segment_before_anchor(
    paths: &MessageStorePaths,
    anchor_message_id: &str,
) -> Result<MessageStoreCompactionSegment, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let anchor = chat_metadata_store_read_locator_by_id(paths, anchor_message_id)?
            .ok_or_else(|| format!("Message not found: {}", anchor_message_id.trim()))?;
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let anchor_is_compaction = anchor.item.compaction_kind.is_some();
        let (start_locator, end_sequence) = if anchor_is_compaction {
            let previous = conn.query_row(
                "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
                 FROM message_locator WHERE conversation_id=?1 AND compaction_kind IS NOT NULL AND sequence<?2 ORDER BY sequence DESC LIMIT 1",
                rusqlite::params![paths.conversation_id, anchor.sequence],
                |row| Ok(ChatMetadataLocator { sequence: row.get(0)?, item: MessageStoreIndexItem {
                    message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32), offset: row.get::<_, i64>(3)? as u64,
                    byte_len: row.get::<_, i64>(4)? as u64, compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
                }}),
            ).optional().map_err(|err| format!("读取 SQLite 前一压缩边界失败: {err}"))?;
            (previous, Some(anchor.sequence))
        } else {
            let previous_compaction = conn.query_row(
                "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
                 FROM message_locator WHERE conversation_id=?1 AND compaction_kind IS NOT NULL AND sequence<?2 ORDER BY sequence DESC LIMIT 1",
                rusqlite::params![paths.conversation_id, anchor.sequence],
                |row| Ok(ChatMetadataLocator { sequence: row.get(0)?, item: MessageStoreIndexItem {
                    message_id: row.get(1)?, block_id: Some(row.get::<_, i64>(2)? as u32), offset: row.get::<_, i64>(3)? as u64,
                    byte_len: row.get::<_, i64>(4)? as u64, compaction_kind: row.get(5)?, role: row.get(6)?, created_at: row.get(7)?,
                }}),
            ).optional().map_err(|err| format!("读取 SQLite 前一压缩边界失败: {err}"))?;
            (previous_compaction, Some(anchor.sequence))
        };
        let start_sequence = start_locator.as_ref().map(|locator| locator.sequence).unwrap_or(0);
        let previous_boundary = match start_locator.as_ref() {
            Some(start) => conn.query_row(
                "SELECT message_id FROM message_locator WHERE conversation_id=?1 AND compaction_kind IS NOT NULL AND sequence<?2 ORDER BY sequence DESC LIMIT 1",
                rusqlite::params![paths.conversation_id, start.sequence], |row| row.get::<_, String>(0),
            ).optional().map_err(|err| format!("读取 SQLite 更早压缩边界失败: {err}"))?,
            None => None,
        };
        let locators = chat_metadata_store_query_locators(&conn, &paths.conversation_id, "AND sequence>=?2 AND sequence<?3", &[&start_sequence, end_sequence.as_ref().unwrap()])?;
        Ok(MessageStoreCompactionSegment {
            messages: chat_metadata_store_read_messages_for_locators(paths, &locators, false)?,
            boundary_message_id: start_locator.as_ref().map(|locator| locator.item.message_id.trim().to_string()),
            previous_boundary_message_id: previous_boundary,
            has_previous_segment: start_sequence > 0,
        })
    })
}

fn chat_metadata_store_block_page(
    paths: &MessageStorePaths,
    requested_block_id: Option<u32>,
) -> Result<MessageStoreBlockPage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let mut statement = conn.prepare(
        "SELECT block_id, message_count FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
    ).map_err(|err| format!("准备读取 SQLite block 页面失败: {err}"))?;
    let rows = statement.query_map([&paths.conversation_id], |row| Ok((row.get::<_, i64>(0)? as u32, row.get::<_, i64>(1)? as usize)))
        .map_err(|err| format!("读取 SQLite block 页面失败: {err}"))?;
    let block_rows = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("解析 SQLite block 页面失败: {err}"))?;
    if block_rows.is_empty() {
        return Ok(MessageStoreBlockPage { blocks: Vec::new(), selected_block_id: requested_block_id.unwrap_or(0), messages: Vec::new(), has_prev_block: false, has_next_block: false });
    }
    let latest_id = block_rows.last().map(|(block_id, _)| *block_id).unwrap_or(0);
    let mut blocks = Vec::with_capacity(block_rows.len());
    for (block_id, message_count) in &block_rows {
        let mut locator_statement = conn.prepare(
            "SELECT message_id, created_at FROM message_locator WHERE conversation_id=?1 AND block_id=?2 ORDER BY sequence ASC",
        ).map_err(|err| format!("准备读取 SQLite block 边界失败: {err}"))?;
        let locators = locator_statement.query_map(rusqlite::params![paths.conversation_id, *block_id as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|err| format!("读取 SQLite block 边界失败: {err}"))?
            .collect::<Result<Vec<_>, _>>().map_err(|err| format!("解析 SQLite block 边界失败: {err}"))?;
        let first = locators.first().cloned().unwrap_or_default();
        let last = locators.last().cloned().unwrap_or_default();
        blocks.push(MessageStoreBlockSummary {
            block_id: *block_id, message_count: *message_count, first_message_id: first.0, last_message_id: last.0,
            first_created_at: Some(first.1).filter(|value| !value.trim().is_empty()),
            last_created_at: Some(last.1).filter(|value| !value.trim().is_empty()), is_latest: *block_id == latest_id,
        });
    }
    let selected_block_id = requested_block_id.filter(|block_id| blocks.iter().any(|block| block.block_id == *block_id)).unwrap_or(latest_id);
    let selected_index = blocks.iter().position(|block| block.block_id == selected_block_id)
        .ok_or_else(|| format!("SQLite 会话块不存在，conversation_id={}，block_id={selected_block_id}", paths.conversation_id))?;
    let locators = chat_metadata_store_read_locators_for_block(paths, selected_block_id)?;
    Ok(MessageStoreBlockPage {
        blocks, selected_block_id, messages: chat_metadata_store_read_messages_for_locators(paths, &locators, true)?,
        has_prev_block: selected_index > 0, has_next_block: selected_index + 1 < block_rows.len(),
    })
    })
}

fn chat_metadata_store_query_block_locators_before(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    block_id: u32,
    before_sequence: Option<i64>,
    limit: usize,
) -> Result<(Vec<ChatMetadataLocator>, bool), String> {
    let normalized_limit = normalized_message_limit(limit);
    let requested = normalized_limit.saturating_add(1) as i64;
    let block_id_value = block_id as i64;
    let (sql, values): (&str, Vec<&dyn rusqlite::ToSql>) =
        if let Some(before_sequence) = before_sequence.as_ref() {
            (
                "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
                 FROM message_locator
                 WHERE conversation_id=?1 AND block_id=?2 AND sequence<?3
                 ORDER BY sequence DESC LIMIT ?4",
                vec![&conversation_id, &block_id_value, before_sequence, &requested],
            )
        } else {
            (
                "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
                 FROM message_locator
                 WHERE conversation_id=?1 AND block_id=?2
                 ORDER BY sequence DESC LIMIT ?3",
                vec![&conversation_id, &block_id_value, &requested],
            )
        };
    let mut statement = conn
        .prepare(sql)
        .map_err(|err| format!("准备反向读取 SQLite block 消息失败: {err}"))?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok(ChatMetadataLocator {
                sequence: row.get(0)?,
                item: MessageStoreIndexItem {
                    message_id: row.get(1)?,
                    block_id: Some(row.get::<_, i64>(2)? as u32),
                    offset: row.get::<_, i64>(3)? as u64,
                    byte_len: row.get::<_, i64>(4)? as u64,
                    compaction_kind: row.get(5)?,
                    role: row.get(6)?,
                    created_at: row.get(7)?,
                },
            })
        })
        .map_err(|err| format!("反向读取 SQLite block 消息失败: {err}"))?;
    let mut locators = rows
        .map(|row| row.map_err(|err| format!("解析 SQLite block 消息失败: {err}")))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = locators.len() > normalized_limit;
    locators.truncate(normalized_limit);
    locators.reverse();
    Ok((locators, has_more))
}

fn chat_metadata_store_read_block_messages_before(
    paths: &MessageStorePaths,
    requested_block_id: Option<u32>,
    before_message_id: Option<&str>,
    limit: usize,
) -> Result<MessageStoreBlockMessagePage, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let before_message_id = before_message_id
            .map(str::trim)
            .filter(|message_id| !message_id.is_empty());
        let anchor = before_message_id
            .map(|message_id| {
                chat_metadata_store_read_locator_by_id(paths, message_id)?.ok_or_else(|| {
                    format!("Message not found: {message_id}")
                })
            })
            .transpose()?;
        let anchor_block_id = anchor
            .as_ref()
            .and_then(|locator| locator.item.block_id);
        if let (Some(requested), Some(anchor_block_id)) =
            (requested_block_id, anchor_block_id)
        {
            if requested != anchor_block_id {
                return Err(format!(
                    "block 反向读取锚点不属于目标块：conversation_id={}，block_id={}，anchor_block_id={}",
                    paths.conversation_id, requested, anchor_block_id
                ));
            }
        }
        let selected_block_id = requested_block_id
            .or(anchor_block_id)
            .or(chat_metadata_store_read_last_block_id(paths)?)
            .unwrap_or(0);
        let conn = chat_metadata_store_open(&paths.data_path)?;
        if requested_block_id.is_some() {
            let block_exists = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversation_blocks WHERE conversation_id=?1 AND block_id=?2)",
                    rusqlite::params![paths.conversation_id, selected_block_id as i64],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|err| format!("确认 SQLite block 存在失败: {err}"))?;
            if !block_exists {
                return Err(format!(
                    "SQLite 会话块不存在，conversation_id={}，block_id={selected_block_id}",
                    paths.conversation_id
                ));
            }
        }
        let anchor_sequence = anchor.as_ref().map(|locator| locator.sequence);
        let (locators, has_more) = chat_metadata_store_query_block_locators_before(
            &conn,
            &paths.conversation_id,
            selected_block_id,
            anchor_sequence,
            limit,
        )?;
        Ok(MessageStoreBlockMessagePage {
            selected_block_id,
            messages: chat_metadata_store_read_messages_for_locators(paths, &locators, false)?,
            has_more,
        })
    })
}

fn chat_metadata_store_count_block_messages(
    paths: &MessageStorePaths,
    before_message_id: &str,
) -> Result<usize, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let before_message_id = before_message_id.trim();
        if before_message_id.is_empty() {
            return Err("读取 SQLite block 消息计数失败：缺少触发消息 ID".to_string());
        }
        let anchor = chat_metadata_store_read_locator_by_id(paths, before_message_id)?
            .ok_or_else(|| format!("Message not found: {before_message_id}"))?;
        let selected_block_id = anchor.item.block_id.unwrap_or(0);
        let conn = chat_metadata_store_open(&paths.data_path)?;
        let count = conn
            .query_row(
                "SELECT COUNT(*) FROM message_locator
                 WHERE conversation_id=?1 AND block_id=?2",
                rusqlite::params![paths.conversation_id, selected_block_id as i64],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| format!("读取 SQLite block 消息计数失败: {err}"))?;
        Ok(count.max(0) as usize)
    })
}

fn chat_metadata_store_status(paths: &MessageStorePaths) -> Result<Option<MessageStoreStatus>, String> {
    if chat_metadata_store_read_meta(paths)?.is_none() {
        return Ok(None);
    }
    let conn = chat_metadata_store_open(&paths.data_path)?;
    let (count, last_message_id) = conn.query_row(
        "SELECT COUNT(*), COALESCE((SELECT message_id FROM message_locator WHERE conversation_id=?1 ORDER BY sequence DESC LIMIT 1), '')
         FROM message_locator WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    ).map_err(|err| format!("读取 SQLite 会话状态失败: {err}"))?;
    Ok(Some(MessageStoreStatus {
        message_count: count as usize,
        last_message_id,
    }))
}

fn chat_metadata_store_read_conversation(paths: &MessageStorePaths) -> Result<Option<Conversation>, String> {
    chat_metadata_store_with_read_snapshot(paths, || {
        let Some(meta) = chat_metadata_store_read_meta(paths)? else {
            return Ok(None);
        };
        let index = chat_metadata_store_read_index(paths)?
            .ok_or_else(|| format!("SQLite 会话缺少 locator，conversation_id={}", paths.conversation_id))?;
        let messages = read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &index.items)?;
        Ok(Some(meta.into_conversation(messages)))
    })
}

fn chat_metadata_store_current_block_files(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<std::collections::HashSet<String>, String> {
    let mut statement = conn
        .prepare(
            "SELECT block_file FROM conversation_blocks WHERE conversation_id=?1",
        )
        .map_err(|err| format!("准备读取 SQLite 当前 block 文件失败: {err}"))?;
    let rows = statement
        .query_map([conversation_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("读取 SQLite 当前 block 文件失败: {err}"))?;
    rows.collect::<Result<std::collections::HashSet<_>, _>>()
        .map_err(|err| format!("解析 SQLite 当前 block 文件失败: {err}"))
}

fn chat_metadata_store_next_available_block_id(
    paths: &MessageStorePaths,
    previous_block_id: Option<u32>,
) -> Result<u32, String> {
    let mut candidate = match previous_block_id {
        Some(block_id) => block_id.checked_add(1).ok_or_else(|| {
            format!(
                "V3 current block ID 已达上限，无法继续追加，conversation_id={}",
                paths.conversation_id
            )
        })?,
        None => 0,
    };
    while paths
        .blocks_dir
        .join(format!("{candidate:06}.jsonl.zstd"))
        .exists()
        || paths
            .blocks_dir
            .join(format!("{candidate:06}.jsonl"))
            .exists()
    {
        candidate = candidate.checked_add(1).ok_or_else(|| {
            format!(
                "会话 block ID 已达上限，无法分配 V3 block，conversation_id={}",
                paths.conversation_id
            )
        })?;
    }
    Ok(candidate)
}

fn chat_metadata_store_allocate_new_block_ids(
    paths: &MessageStorePaths,
    previous_block_id: Option<u32>,
    block_count: usize,
) -> Result<Vec<u32>, String> {
    let mut block_ids = Vec::with_capacity(block_count);
    let mut previous = previous_block_id;
    for _ in 0..block_count {
        let block_id = chat_metadata_store_next_available_block_id(paths, previous)?;
        block_ids.push(block_id);
        previous = Some(block_id);
    }
    Ok(block_ids)
}

fn chat_metadata_store_allocate_rebuilt_block_ids(
    paths: &MessageStorePaths,
    reusable_block_ids: &[u32],
    block_count: usize,
) -> Result<Vec<u32>, String> {
    let reusable_count = reusable_block_ids.len().min(block_count);
    let mut block_ids = reusable_block_ids[..reusable_count].to_vec();
    if reusable_count == block_count {
        return Ok(block_ids);
    }
    let new_block_ids = chat_metadata_store_allocate_new_block_ids(
        paths,
        reusable_block_ids.last().copied(),
        block_count - reusable_count,
    )?;
    block_ids.extend(new_block_ids);
    Ok(block_ids)
}

fn chat_metadata_store_remap_snapshot_blocks(
    blocks: &mut JsonlSnapshotConversationBlocks,
    block_ids: &[u32],
) -> Result<(), String> {
    if blocks.blocks.len() != block_ids.len() {
        return Err(format!(
            "V3 快照 block ID 数量不一致，blocks={}，block_ids={}",
            blocks.blocks.len(),
            block_ids.len()
        ));
    }
    for (block, block_id) in blocks.blocks.iter_mut().zip(block_ids.iter().copied()) {
        block.block_id = block_id;
        block.block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
        for item in &mut block.index_items {
            item.block_id = Some(block_id);
        }
    }
    let index_items = blocks.blocks.iter()
        .flat_map(|block| block.index_items.iter().cloned())
        .collect();
    blocks.index = MessageStoreIndexFile::new(blocks.index.version, index_items);
    Ok(())
}

fn chat_metadata_store_write_snapshot(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    chat_metadata_store_with_writer_gate(paths, || {
        chat_metadata_store_write_snapshot_unlocked(paths, conversation)
    })
}

fn chat_metadata_store_write_snapshot_unlocked(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let publication_gate = chat_metadata_store_publication_gate(paths);
    let _publication_guard = publication_gate.write().unwrap_or_else(|poison| poison.into_inner());
    let mut conn = chat_metadata_store_open(&paths.data_path)?;
    let existing_current_block_files =
        chat_metadata_store_current_block_files(&conn, &paths.conversation_id)?;
    let existing_current_block_ids = chat_metadata_store_read_all_block_ids(paths)?;
    let mut blocks = build_jsonl_snapshot_conversation_blocks_for_conversation_v4(conversation)?;
    // V1/V2 与 V3 历史上共用 blocks 目录。优先复用 SQLite current
    // 已管理的 block ID；新增 block 从上一个 current ID 向后查找，
    // 只跳过磁盘实际已占用的文件，避免覆盖未被 current 管理的旧 artifact。
    // 纯 V3 连续 ID 场景保持原编号。
    let rebuilt_block_ids = chat_metadata_store_allocate_rebuilt_block_ids(
        paths,
        &existing_current_block_ids,
        blocks.blocks.len(),
    )?;
    chat_metadata_store_remap_snapshot_blocks(&mut blocks, &rebuilt_block_ids)?;
    let meta = ConversationShardMeta::from_conversation(conversation);
    fs::create_dir_all(&paths.blocks_dir).map_err(|err| format!("创建 v3 会话块目录失败: {err}"))?;
    let operation_id = Uuid::new_v4().to_string();
    let operation_root = chat_metadata_operation_root(paths, &operation_id);
    let old_blocks_dir = operation_root.join("old");
    let new_blocks_dir = operation_root.join("new");
    chat_metadata_copy_blocks(&paths.blocks_dir, &old_blocks_dir)?;
    let expected_files = blocks.blocks.iter().map(|block| block.block_file.clone()).collect::<std::collections::HashSet<_>>();
    for block in &blocks.blocks {
        write_jsonl_snapshot_block_v4_atomic(&new_blocks_dir.join(&block.block_file), &block.content)?;
    }
    let retired_files = existing_current_block_files
        .difference(&expected_files)
        .cloned()
        .collect::<Vec<_>>();
    let detail = ChatStorageOperationDetail {
        conversation_id: paths.conversation_id.clone(),
        expected_block_files: expected_files.iter().cloned().collect(),
        replaced_block_files: expected_files.iter().cloned().collect(),
        retired_block_files: retired_files,
        new_block_files: expected_files.iter()
            .filter(|file| !existing_current_block_files.contains(*file))
            .cloned()
            .collect(),
    };
    let detail_json = serde_json::to_string(&detail).map_err(|err| format!("序列化 v3 存储操作失败: {err}"))?;
    let before_revision = conn.query_row(
        "SELECT storage_revision FROM conversation_metadata WHERE conversation_id=?1",
        [&paths.conversation_id],
        |row| row.get::<_, i64>(0),
    ).optional().map_err(|err| format!("读取 v3 会话 revision 失败: {err}"))?.unwrap_or(0);
    conn.execute(
        "INSERT INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at)
         VALUES(?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
        rusqlite::params![operation_id, paths.conversation_id, before_revision, before_revision + 1, detail_json, now_iso()],
    ).map_err(|err| format!("登记 v3 存储操作失败: {err}"))?;
    for block in &blocks.blocks {
        let staged = fs::read(new_blocks_dir.join(&block.block_file))
            .map_err(|err| format!("读取 v3 staged block 失败，block={}，error={err}", block.block_file))?;
        write_message_store_bytes_atomic(
            &paths.shard_dir.join(&block.block_file),
            "jsonl.zstd.tmp",
            &staged,
            "V4 压缩块",
        )?;
    }
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("开启 v3 发布事务失败: {err}"))?;
    let meta_json = serde_json::to_string(&meta).map_err(|err| format!("序列化 v3 metadata 失败: {err}"))?;
    transaction.execute(
        "INSERT INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(conversation_id) DO UPDATE SET metadata_json=excluded.metadata_json, storage_revision=excluded.storage_revision, updated_at=excluded.updated_at",
        rusqlite::params![paths.conversation_id, meta_json, before_revision + 1, meta.updated_at()],
    ).map_err(|err| format!("发布 v3 metadata 失败: {err}"))?;
    transaction.execute("DELETE FROM conversation_blocks WHERE conversation_id=?1", [&paths.conversation_id]).map_err(|err| format!("清理 v3 旧 block 失败: {err}"))?;
    transaction.execute("DELETE FROM message_locator WHERE conversation_id=?1", [&paths.conversation_id]).map_err(|err| format!("清理 v3 旧 locator 失败: {err}"))?;
    for block in &blocks.blocks {
        let staged = new_blocks_dir.join(&block.block_file);
        let compressed_len = fs::metadata(&staged)
            .map_err(|err| format!("读取 v3 staged block 元数据失败，block={}，error={err}", block.block_file))?
            .len();
        transaction.execute(
            "INSERT INTO conversation_blocks(conversation_id, block_id, block_file, byte_len, message_count) VALUES(?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![paths.conversation_id, block.block_id as i64, block.block_file, compressed_len as i64, block.index_items.len() as i64],
        ).map_err(|err| format!("发布 v3 block 失败: {err}"))?;
    }
    for (sequence, item) in blocks.index.items.iter().enumerate() {
        let block_id = item.block_id.ok_or_else(|| format!("v3 block locator 缺少 block id，message_id={}", item.message_id))?;
        transaction.execute(
            "INSERT INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![paths.conversation_id, sequence as i64, item.message_id, block_id as i64, item.offset as i64, item.byte_len as i64, item.compaction_kind, item.role, item.created_at],
        ).map_err(|err| format!("发布 v3 locator 失败: {err}"))?;
    }
    transaction.execute(
        "UPDATE storage_operations SET state='committed', committed_at=?1 WHERE operation_id=?2",
        rusqlite::params![now_iso(), operation_id],
    ).map_err(|err| format!("完成 v3 存储操作失败: {err}"))?;
    transaction.commit().map_err(|err| format!("提交 v3 发布事务失败: {err}"))?;
    cleanup_stale_conversation_block_files_by_names(
        paths,
        &expected_files,
        Some(&existing_current_block_files),
    )?;
    fs::remove_dir_all(&operation_root).map_err(|err| format!("清理 v3 存储操作目录失败，path={}，error={err}", operation_root.display()))?;
    conn.execute("DELETE FROM storage_operations WHERE operation_id=?1 AND state='committed'", [&operation_id])
        .map_err(|err| format!("清理已完成 v3 存储操作失败: {err}"))?;
    Ok(MessageStoreDirectorySnapshotWrite {
        manifest: MessageStoreManifest::jsonl_snapshot_ready_for_messages(
            blocks.message_count,
            blocks.last_message_id.clone(),
            blocks.total_bytes,
            (before_revision + 1) as u64,
        ),
        message_count: blocks.message_count,
        last_message_id: blocks.last_message_id,
    })
}

fn chat_metadata_store_drop_recover_operation(
    conn: &rusqlite::Connection,
    operation_id: &str,
    reason: &str,
) -> Result<(), String> {
    runtime_log_warn(format!(
        "[聊天存储恢复] 放弃，任务=恢复v3未完成操作，operation_id={}，reason={}",
        operation_id, reason
    ));
    conn.execute("DELETE FROM storage_operations WHERE operation_id=?1", [operation_id])
        .map_err(|err| format!("清理 v3 异常恢复操作记录失败: {err}"))?;
    Ok(())
}

fn chat_metadata_store_recover_operations(data_path: &PathBuf) -> Result<(), String> {
    let conn = chat_metadata_store_open(data_path)?;
    let mut statement = conn.prepare(
        "SELECT operation_id, conversation_id, before_revision, after_revision, state, detail_json
         FROM storage_operations ORDER BY created_at ASC",
    ).map_err(|err| format!("准备读取 v3 未完成操作失败: {err}"))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?,
        ))
    }).map_err(|err| format!("读取 v3 未完成操作失败: {err}"))?;
    let mut operations = Vec::new();
    for row in rows {
        operations.push(row.map_err(|err| format!("解析 v3 未完成操作失败: {err}"))?);
    }
    drop(statement);
    for (operation_id, conversation_id, _before_revision, _after_revision, state, detail_json) in operations {
        let detail: ChatStorageOperationDetail = match serde_json::from_str(&detail_json) {
            Ok(detail) => detail,
            Err(err) => {
                chat_metadata_store_drop_recover_operation(
                    &conn,
                    &operation_id,
                    &format!("解析操作详情失败：{err}"),
                )?;
                continue;
            }
        };
        if detail.conversation_id != conversation_id {
            chat_metadata_store_drop_recover_operation(
                &conn,
                &operation_id,
                &format!(
                    "操作会话ID不一致：record={}，detail={}",
                    conversation_id, detail.conversation_id
                ),
            )?;
            continue;
        }
        let paths = match message_store_paths(data_path, &conversation_id) {
            Ok(paths) => paths,
            Err(err) => {
                chat_metadata_store_drop_recover_operation(&conn, &operation_id, &err)?;
                continue;
            }
        };
        let publication_gate = chat_metadata_store_publication_gate(&paths);
        let _publication_guard = publication_gate.write().unwrap_or_else(|poison| poison.into_inner());
        let operation_root = chat_metadata_operation_root(&paths, &operation_id);
        if state == "committed" {
            for file in &detail.replaced_block_files {
                let staged = operation_root.join("new").join(file);
                if staged.exists() {
                    let content = fs::read_to_string(&staged).map_err(|err| format!("读取 v3 恢复 staged block 失败，path={}，error={err}", staged.display()))?;
                    write_jsonl_snapshot_atomic(&paths.shard_dir.join(file), &content)?;
                }
            }
            let expected_files = detail
                .expected_block_files
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>();
            let mut managed_files = expected_files.clone();
            managed_files.extend(detail.replaced_block_files.iter().cloned());
            managed_files.extend(detail.retired_block_files.iter().cloned());
            cleanup_stale_conversation_block_files_by_names(
                &paths,
                &expected_files,
                Some(&managed_files),
            )?;
        } else {
            for file in detail.replaced_block_files.iter().chain(detail.retired_block_files.iter()) {
                let backup = operation_root.join("old").join(file);
                let target = paths.shard_dir.join(file);
                if backup.exists() {
                    let content = fs::read_to_string(&backup).map_err(|err| format!("读取 v3 恢复旧 block 失败，path={}，error={err}", backup.display()))?;
                    write_jsonl_snapshot_atomic(&target, &content)?;
                } else if detail.new_block_files.contains(file) && target.exists() {
                    fs::remove_file(&target).map_err(|err| format!("删除未发布 v3 新 block 失败，path={}，error={err}", target.display()))?;
                }
            }
        }
        if operation_root.exists() {
            if let Err(err) = fs::remove_dir_all(&operation_root) {
                runtime_log_warn(format!(
                    "[聊天存储恢复] 跳过，任务=清理v3恢复操作目录，operation_id={}，path={}，异常={}，retry=保留操作记录",
                    operation_id,
                    operation_root.display(),
                    err
                ));
                continue;
            }
        }
        conn.execute("DELETE FROM storage_operations WHERE operation_id=?1", [&operation_id])
            .map_err(|err| format!("清理 v3 恢复操作记录失败: {err}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_migration_should_initialize_empty_chat_without_legacy_files() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-empty-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    chat_metadata_store_run_v3_migration(&data_path).expect("migrate empty chat storage");
    assert!(chat_metadata_store_is_ready(&data_path).expect("read migration state"));
    assert!(chat_metadata_store_db_path(&data_path).exists());
    let _ = fs::remove_dir_all(root);
}


#[cfg(test)]
#[test]
fn v3_chat_metadata_migration_should_import_v2_metadata_without_removing_v2_files() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-import-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let mut conversation = Conversation {
        id: "conv-v3-import".to_string(), title: "SQLite 会话".to_string(), agent_id: DEFAULT_AGENT_ID.to_string(), bound_conversation_id: None, parent_conversation_id: None, child_conversation_ids: Vec::new(), fork_message_cursor: None, unread_count: 0, conversation_kind: CONVERSATION_KIND_CHAT.to_string(), root_conversation_id: None, delegate_id: None, created_at: now_iso(), updated_at: now_iso(), last_user_at: None, last_assistant_at: None, status: "active".to_string(), user_profile_snapshot: String::new(), shell_workspace_path: None, shell_workspaces: Vec::new(), shell_autonomous_mode: false, shell_work_mode: default_shell_work_mode(), archived_at: None, messages: Vec::new(), fast_request_turns: Vec::new(), current_todos: Vec::new(), memory_recall_table: Vec::new(), plan_mode_enabled: false, preferred_api_config_id: None, auto_push_remote_contact_id: None, active_goal: None, last_error: None, cumulative_usage: ConversationCumulativeUsage::default(),
        shell_work_branch: String::new(),
        is_draft: false,
    };
    conversation.messages.push(ChatMessage {
        id: "migrate-message".to_string(),
        role: "assistant".to_string(),
        created_at: "2026-07-10T00:00:00Z".to_string(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text { text: "迁移消息".to_string(), reasoning_content: None }],
        extra_text_blocks: Vec::new(),
        provider_meta: None,
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    });
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write v2 fixture");
    forget_message_store_index_cache(&paths.index_file);
    assert!(paths.meta_file.exists() && paths.manifest_file.exists() && paths.index_file.exists());
    chat_metadata_store_import_v2_conversation(&paths).expect("import interrupted v3 fixture");
    let migration_key = chat_metadata_store_v3_conversation_migration_key(&conversation.id);
    chat_metadata_store_mark_migration_completed(&data_path, &migration_key)
        .expect("record conversation migration progress");
    assert!(!chat_metadata_store_is_ready(&data_path).expect("global migration remains pending"));
    chat_metadata_store_run_v3_migration(&data_path).expect("resume v3 fixture migration");
    let meta = chat_metadata_store_read_meta(&paths).expect("read SQLite metadata").expect("metadata exists");
    assert_eq!(meta.title(), "SQLite 会话");
    assert!(paths.meta_file.exists() && paths.manifest_file.exists() && paths.index_file.exists());
    assert!(chat_metadata_store_read_conversation(&paths).expect("read SQLite conversation").is_some());
    let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl");
    let block_path = paths.shard_dir.join(&block_file);
    let original_block = fs::read_to_string(&block_path).expect("read migrated block");
    let conn = chat_metadata_store_open(&data_path).expect("open SQLite metadata");
    let revision = conn.query_row(
        "SELECT storage_revision FROM conversation_metadata WHERE conversation_id=?1",
        [&conversation.id],
        |row| row.get::<_, i64>(0),
    ).expect("read revision");
    let operation_id = Uuid::new_v4().to_string();
    let detail = ChatStorageOperationDetail {
        conversation_id: conversation.id.clone(),
        expected_block_files: vec![block_file.clone()],
        replaced_block_files: vec![block_file],
        retired_block_files: Vec::new(),
        new_block_files: Vec::new(),
    };
    conn.execute(
        "INSERT INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at)
         VALUES(?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
        rusqlite::params![operation_id, conversation.id, revision, revision + 1, serde_json::to_string(&detail).expect("serialize operation"), now_iso()],
    ).expect("record pending operation before block preparation");
    chat_metadata_store_recover_operations(&data_path).expect("recover pending operation without backups");
    assert_eq!(fs::read_to_string(&block_path).expect("preserve old block"), original_block);
    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM storage_operations WHERE operation_id=?1",
        [&operation_id],
        |row| row.get(0),
    ).expect("count recovered operation");
    assert_eq!(remaining, 0);

    let insert_operation = |operation_id: &str, before_revision: i64, after_revision: i64, state: &str, detail: &ChatStorageOperationDetail| {
        conn.execute(
            "INSERT INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![operation_id, conversation.id, before_revision, after_revision, state, serde_json::to_string(detail).expect("serialize operation"), now_iso()],
        ).expect("record recovery operation");
    };
    let recovery_detail = |retired_block_files: Vec<String>| ChatStorageOperationDetail {
        conversation_id: conversation.id.clone(),
        expected_block_files: vec![format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl")],
        replaced_block_files: vec![format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl")],
        retired_block_files,
        new_block_files: Vec::new(),
    };

    let staged_operation_id = Uuid::new_v4().to_string();
    let staged_root = chat_metadata_operation_root(&paths, &staged_operation_id);
    let staged_file = staged_root.join("new").join(&format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl"));
    write_jsonl_snapshot_atomic(&staged_file, &original_block).expect("write staged block");
    insert_operation(&staged_operation_id, revision, revision + 1, "pending", &recovery_detail(Vec::new()));
    chat_metadata_store_recover_operations(&data_path).expect("recover staged block operation");
    assert_eq!(fs::read_to_string(&block_path).expect("read preserved block"), original_block);
    assert!(!staged_root.exists());

    let replaced_operation_id = Uuid::new_v4().to_string();
    let replaced_root = chat_metadata_operation_root(&paths, &replaced_operation_id);
    let backup_file = replaced_root.join("old").join(&format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl"));
    write_jsonl_snapshot_atomic(&backup_file, &original_block).expect("write old block backup");
    write_jsonl_snapshot_atomic(&block_path, "{\"broken\":true}\n").expect("replace physical block");
    insert_operation(&replaced_operation_id, revision, revision + 1, "pending", &recovery_detail(Vec::new()));
    chat_metadata_store_recover_operations(&data_path).expect("recover replaced block operation");
    assert_eq!(fs::read_to_string(&block_path).expect("restore old block"), original_block);

    let committed_operation_id = Uuid::new_v4().to_string();
    let committed_root = chat_metadata_operation_root(&paths, &committed_operation_id);
    let committed_file = committed_root.join("new").join(&format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl"));
    write_jsonl_snapshot_atomic(&committed_file, &original_block).expect("write committed staged block");
    let stale_block = paths.blocks_dir.join("000999.jsonl");
    write_jsonl_snapshot_atomic(&stale_block, "{\"stale\":true}\n").expect("write stale block");
    conn.execute(
        "UPDATE conversation_metadata SET storage_revision=?1 WHERE conversation_id=?2",
        rusqlite::params![revision + 1, conversation.id],
    ).expect("advance committed revision");
    insert_operation(
        &committed_operation_id,
        revision,
        revision + 1,
        "committed",
        &recovery_detail(vec![format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000999.jsonl")]),
    );
    chat_metadata_store_recover_operations(&data_path).expect("recover committed cleanup operation");
    assert_eq!(fs::read_to_string(&block_path).expect("keep committed block"), original_block);
    assert!(!stale_block.exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_recover_operations_should_drop_bad_operation_detail() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-bad-operation-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    chat_metadata_store_run_v3_migration(&data_path).expect("initialize v3 storage");
    let conn = chat_metadata_store_open(&data_path).expect("open SQLite metadata");
    let operation_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at)
         VALUES(?1, ?2, 0, 1, 'pending', ?3, ?4)",
        rusqlite::params![operation_id, "bad-operation-conversation", "{broken", now_iso()],
    ).expect("record bad operation");

    chat_metadata_store_recover_operations(&data_path).expect("drop bad operation detail");
    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM storage_operations WHERE operation_id=?1",
        [&operation_id],
        |row| row.get(0),
    ).expect("count bad operation");

    assert_eq!(remaining, 0);
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_recover_operations_should_keep_record_when_cleanup_fails() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-cleanup-retry-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    chat_metadata_store_run_v3_migration(&data_path).expect("initialize v3 storage");
    let conn = chat_metadata_store_open(&data_path).expect("open SQLite metadata");
    let conversation_id = "cleanup-retry-conversation";
    let paths = message_store_paths(&data_path, conversation_id).expect("paths");
    let operation_id = Uuid::new_v4().to_string();
    let operation_root = chat_metadata_operation_root(&paths, &operation_id);
    if let Some(parent) = operation_root.parent() {
        fs::create_dir_all(parent).expect("create operation parent");
    }
    fs::write(&operation_root, "not a directory").expect("block cleanup with file");
    let detail = ChatStorageOperationDetail {
        conversation_id: conversation_id.to_string(),
        expected_block_files: Vec::new(),
        replaced_block_files: Vec::new(),
        retired_block_files: Vec::new(),
        new_block_files: Vec::new(),
    };
    conn.execute(
        "INSERT INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at)
         VALUES(?1, ?2, 0, 1, 'pending', ?3, ?4)",
        rusqlite::params![
            operation_id,
            conversation_id,
            serde_json::to_string(&detail).expect("serialize operation"),
            now_iso()
        ],
    ).expect("record cleanup retry operation");

    chat_metadata_store_recover_operations(&data_path).expect("recover should keep retry record");
    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM storage_operations WHERE operation_id=?1",
        [&operation_id],
        |row| row.get(0),
    ).expect("count retained operation");

    assert_eq!(remaining, 1);
    let _ = fs::remove_file(operation_root);
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_migration_should_skip_building_conversation_without_writing_back() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-skip-building-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let message = |id: &str| ChatMessage {
        id: id.to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text { text: id.to_string(), reasoning_content: None }],
        extra_text_blocks: Vec::new(),
        provider_meta: None,
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    };
    let conversation = Conversation {
        id: "conv-v3-skip-building".to_string(), title: "跳过 building".to_string(), agent_id: DEFAULT_AGENT_ID.to_string(), bound_conversation_id: None, parent_conversation_id: None, child_conversation_ids: Vec::new(), fork_message_cursor: None, unread_count: 0, conversation_kind: CONVERSATION_KIND_CHAT.to_string(), root_conversation_id: None, delegate_id: None, created_at: now_iso(), updated_at: now_iso(), last_user_at: None, last_assistant_at: None, status: "active".to_string(), user_profile_snapshot: String::new(), shell_workspace_path: None, shell_workspaces: Vec::new(), shell_autonomous_mode: false, shell_work_mode: default_shell_work_mode(), archived_at: None, messages: vec![message("m1"), message("m2")], fast_request_turns: Vec::new(), current_todos: Vec::new(), memory_recall_table: Vec::new(), plan_mode_enabled: false, preferred_api_config_id: None, auto_push_remote_contact_id: None, active_goal: None, last_error: None, cumulative_usage: ConversationCumulativeUsage::default(),
        shell_work_branch: String::new(),
        is_draft: false,
    };
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write v2 fixture");
    let block_path = paths.blocks_dir.join("000000.jsonl");
    let mut block = fs::read_to_string(&block_path).expect("read block");
    block.push_str("{broken json line}\n");
    block.push_str(&encode_jsonl_snapshot_message(&message("m3")).expect("encode tail message"));
    fs::write(&block_path, block).expect("write interrupted block");
    let building = MessageStoreManifest::jsonl_snapshot_building(&conversation);
    write_message_store_manifest_atomic(&paths.manifest_file, &building).expect("write building manifest");

    chat_metadata_store_run_v3_migration(&data_path).expect("migrate v3");

    assert!(!chat_metadata_store_contains_conversation(&data_path, &conversation.id).expect("contains check"));
    assert!(fs::read_to_string(&block_path).expect("read untouched block").contains("broken json line"));
    assert_eq!(
        read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists")
            .migration_state,
        MessageStoreMigrationState::Building
    );
    let _ = fs::remove_dir_all(root);
}


#[cfg(test)]
#[test]
fn v3_chat_metadata_block_reader_should_stop_at_block_boundary() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-block-reader-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let message = |id: &str, role: &str| ChatMessage {
        id: id.to_string(), role: role.to_string(), created_at: now_iso(), speaker_agent_id: None,
        parts: vec![MessagePart::Text { text: id.to_string(), reasoning_content: None }], extra_text_blocks: Vec::new(), provider_meta: None, tool_call: None, mcp_call: None, meme_annotations: None,
    };
    let mut compaction = message("summary", "user");
    compaction.provider_meta = Some(serde_json::json!({
        "message_meta": { "kind": "context_compaction" }
    }));
    let conversation = Conversation {
        id: "conv-v3-block-reader".to_string(), title: "SQLite block reader".to_string(), agent_id: DEFAULT_AGENT_ID.to_string(), bound_conversation_id: None, parent_conversation_id: None, child_conversation_ids: Vec::new(), fork_message_cursor: None, unread_count: 0, conversation_kind: CONVERSATION_KIND_CHAT.to_string(), root_conversation_id: None, delegate_id: None, created_at: now_iso(), updated_at: now_iso(), last_user_at: None, last_assistant_at: None, status: "active".to_string(), user_profile_snapshot: String::new(), shell_workspace_path: None, shell_workspaces: Vec::new(), shell_autonomous_mode: false, shell_work_mode: default_shell_work_mode(), archived_at: None,
        shell_work_branch: String::new(),
        messages: vec![message("old", "user"), compaction, message("current-1", "user"), message("current-2", "assistant")],
        fast_request_turns: Vec::new(), current_todos: Vec::new(), memory_recall_table: Vec::new(), plan_mode_enabled: false, preferred_api_config_id: None, auto_push_remote_contact_id: None, active_goal: None, last_error: None, cumulative_usage: ConversationCumulativeUsage::default(),
        is_draft: false,
    };
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write v2 fixture");
    chat_metadata_store_run_v3_migration(&data_path).expect("migrate v3");

    let page = chat_metadata_store_read_block_messages_before(
        &paths,
        None,
        Some("current-2"),
        10,
    )
    .expect("read current block before anchor");

    assert_eq!(
        page.messages.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
        vec!["summary", "current-1"]
    );
    assert!(!page.has_more);

    let block_message_count = chat_metadata_store_count_block_messages(
        &paths,
        "current-2",
    )
    .expect("count current SQLite block messages");
    assert_eq!(block_message_count, 3);
    let _ = fs::remove_dir_all(root);
}


#[cfg(test)]
#[test]
fn v3_chat_metadata_physical_append_should_append_bytes_without_rebuilding() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-physical-append-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let message = |id: &str, role: &str| ChatMessage {
        id: id.to_string(), role: role.to_string(), created_at: now_iso(), speaker_agent_id: None,
        parts: vec![MessagePart::Text { text: id.to_string(), reasoning_content: None }], extra_text_blocks: Vec::new(), provider_meta: None, tool_call: None, mcp_call: None, meme_annotations: None,
    };
    let mut conversation = Conversation {
        id: "conv-v3-physical-append".to_string(), title: "physical append".to_string(), agent_id: DEFAULT_AGENT_ID.to_string(), bound_conversation_id: None, parent_conversation_id: None, child_conversation_ids: Vec::new(), fork_message_cursor: None, unread_count: 0, conversation_kind: CONVERSATION_KIND_CHAT.to_string(), root_conversation_id: None, delegate_id: None, created_at: now_iso(), updated_at: now_iso(), last_user_at: None, last_assistant_at: None, status: "active".to_string(), user_profile_snapshot: String::new(), shell_workspace_path: None, shell_workspaces: Vec::new(), shell_autonomous_mode: false, shell_work_mode: default_shell_work_mode(), archived_at: None, messages: Vec::new(), fast_request_turns: Vec::new(), current_todos: Vec::new(), memory_recall_table: Vec::new(), plan_mode_enabled: false, preferred_api_config_id: None, auto_push_remote_contact_id: None, active_goal: None, last_error: None, cumulative_usage: ConversationCumulativeUsage::default(),
        shell_work_branch: String::new(),
        is_draft: false,
    };
    conversation.messages.push(message("m1", "user"));
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write v2 fixture");
    chat_metadata_store_run_v3_migration(&data_path).expect("migrate v3");
    migration_v3_to_v4(&data_path, None).expect("migrate v4");

    let block_path = paths.shard_dir.join(format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl.zstd"));
    let before_bytes = fs::read(&block_path).expect("read block before append");
    let before_plain = zstd_decompress_block(&before_bytes).expect("decompress before append");

    let mut after_append = conversation.clone();
    after_append.messages.push(message("m2", "assistant"));
    chat_metadata_store_append_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&after_append),
        &after_append.messages[1..],
    )
    .expect("physical append");

    // 1. 块文件 = 旧压缩帧原样 + 新帧（帧追加，未重建前缀）
    let after_bytes = fs::read(&block_path).expect("read block after append");
    let after_plain = zstd_decompress_block(&after_bytes).expect("decompress after append");
    assert!(
        String::from_utf8_lossy(&after_plain).starts_with(&*String::from_utf8_lossy(&before_plain)),
        "帧追加应原样保留旧明文前缀"
    );
    assert!(after_bytes.len() > before_bytes.len());

    // 2. 读取一致
    let read_messages = chat_metadata_store_read_conversation(&paths)
        .expect("read conversation")
        .expect("conversation exists");
    assert_eq!(read_messages.messages.len(), 2);
    assert_eq!(read_messages.messages[1].id, "m2");

    // 3. locator 顺延：m2 offset = m1 末尾（明文坐标），sequence 连续
    let m1 = chat_metadata_store_read_locator_by_id(&paths, "m1").expect("read m1 locator").expect("m1 exists");
    let m2 = chat_metadata_store_read_locator_by_id(&paths, "m2").expect("read m2 locator").expect("m2 exists");
    assert_eq!(m2.item.offset, m1.item.offset + m1.item.byte_len);
    assert_eq!(m2.sequence, m1.sequence + 1);
    assert_eq!(after_plain.len() as u64, m2.item.offset + m2.item.byte_len);

    // 4. 再追加一条：仍然物理追加、sequence 继续顺延
    let mut after_append2 = after_append.clone();
    after_append2.messages.push(message("m3", "user"));
    chat_metadata_store_append_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&after_append2),
        &after_append2.messages[2..],
    )
    .expect("second physical append");
    let m3 = chat_metadata_store_read_locator_by_id(&paths, "m3").expect("read m3 locator").expect("m3 exists");
    assert_eq!(m3.sequence, m2.sequence + 1);
    assert_eq!(
        chat_metadata_store_read_conversation(&paths).expect("read conversation").expect("conversation").messages.len(),
        3
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_physical_append_should_truncate_orphan_tail_bytes() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-orphan-tail-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let message = |id: &str, role: &str| ChatMessage {
        id: id.to_string(), role: role.to_string(), created_at: now_iso(), speaker_agent_id: None,
        parts: vec![MessagePart::Text { text: id.to_string(), reasoning_content: None }], extra_text_blocks: Vec::new(), provider_meta: None, tool_call: None, mcp_call: None, meme_annotations: None,
    };
    let mut conversation = Conversation {
        id: "conv-v3-orphan-tail".to_string(), title: "orphan tail".to_string(), agent_id: DEFAULT_AGENT_ID.to_string(), bound_conversation_id: None, parent_conversation_id: None, child_conversation_ids: Vec::new(), fork_message_cursor: None, unread_count: 0, conversation_kind: CONVERSATION_KIND_CHAT.to_string(), root_conversation_id: None, delegate_id: None, created_at: now_iso(), updated_at: now_iso(), last_user_at: None, last_assistant_at: None, status: "active".to_string(), user_profile_snapshot: String::new(), shell_workspace_path: None, shell_workspaces: Vec::new(), shell_autonomous_mode: false, shell_work_mode: default_shell_work_mode(), archived_at: None, messages: Vec::new(), fast_request_turns: Vec::new(), current_todos: Vec::new(), memory_recall_table: Vec::new(), plan_mode_enabled: false, preferred_api_config_id: None, auto_push_remote_contact_id: None, active_goal: None, last_error: None, cumulative_usage: ConversationCumulativeUsage::default(),
        shell_work_branch: String::new(),
        is_draft: false,
    };
    conversation.messages.push(message("m1", "user"));
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write v2 fixture");
    chat_metadata_store_run_v3_migration(&data_path).expect("migrate v3");
    migration_v3_to_v4(&data_path, None).expect("migrate v4");

    // 模拟上次追加崩溃残留：块文件尾有未登记 locator 的孤儿字节（torn 帧）
    let block_path = paths.shard_dir.join(format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/000000.jsonl.zstd"));
    {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new().append(true).open(&block_path).expect("open block for orphan");
        file.write_all(b"\x28\xb5\x2f\xfd orphan-garbage").expect("append orphan bytes");
    }

    // 追加 m2：应截断孤儿后再物理追加
    let mut after_append = conversation.clone();
    after_append.messages.push(message("m2", "assistant"));
    chat_metadata_store_append_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&after_append),
        &after_append.messages[1..],
    )
    .expect("append after orphan");

    let after_bytes = fs::read(&block_path).expect("read block after orphan cleanup");
    let after_plain = zstd_decompress_block(&after_bytes).expect("decompress after orphan cleanup");
    let content = String::from_utf8(after_plain.clone()).expect("block utf8");
    assert!(!content.contains("orphan"), "孤儿字节应被截断");

    // 文件尾 = 最后登记组末尾（m2 offset+len，明文坐标），无残留
    let m2 = chat_metadata_store_read_locator_by_id(&paths, "m2").expect("read m2 locator").expect("m2 exists");
    assert_eq!(after_plain.len() as u64, m2.item.offset + m2.item.byte_len);

    // 读取一致
    let read_messages = chat_metadata_store_read_conversation(&paths)
        .expect("read conversation")
        .expect("conversation exists");
    assert_eq!(read_messages.messages.len(), 2);
    assert_eq!(read_messages.messages[1].id, "m2");
    let _ = fs::remove_dir_all(root);
}


#[cfg(test)]
#[test]
fn v3_chat_metadata_mutations_should_publish_only_sql_locator_and_blocks() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-mutations-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let mut conversation = Conversation {
        id: "conv-v3-mutations".to_string(), title: "SQLite mutation".to_string(), agent_id: DEFAULT_AGENT_ID.to_string(), bound_conversation_id: None, parent_conversation_id: None, child_conversation_ids: Vec::new(), fork_message_cursor: None, unread_count: 0, conversation_kind: CONVERSATION_KIND_CHAT.to_string(), root_conversation_id: None, delegate_id: None, created_at: now_iso(), updated_at: now_iso(), last_user_at: None, last_assistant_at: None, status: "active".to_string(), user_profile_snapshot: String::new(), shell_workspace_path: None, shell_workspaces: Vec::new(), shell_autonomous_mode: false, shell_work_mode: default_shell_work_mode(), archived_at: None, messages: Vec::new(), fast_request_turns: Vec::new(), current_todos: Vec::new(), memory_recall_table: Vec::new(), plan_mode_enabled: false, preferred_api_config_id: None, auto_push_remote_contact_id: None, active_goal: None, last_error: None, cumulative_usage: ConversationCumulativeUsage::default(),
        shell_work_branch: String::new(),
        is_draft: false,
    };
    let message = |id: &str, role: &str| ChatMessage {
        id: id.to_string(), role: role.to_string(), created_at: now_iso(), speaker_agent_id: None,
        parts: vec![MessagePart::Text { text: id.to_string(), reasoning_content: None }], extra_text_blocks: Vec::new(), provider_meta: None, tool_call: None, mcp_call: None, meme_annotations: None,
    };
    conversation.messages.push(message("m1", "user"));
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write v2 fixture");
    chat_metadata_store_run_v3_migration(&data_path).expect("migrate v3");
    migration_v3_to_v4(&data_path, None).expect("migrate v4");

    active_plan_append_in_progress(
        &data_path,
        &conversation.id,
        "m1",
        "docs/plan/v3.md",
    )
    .expect("append active plan through v3");
    assert_eq!(
        chat_metadata_store_read_active_plans(&data_path, &conversation.id)
            .expect("read active plans")
            .expect("v3 active plans")
            .len(),
        1
    );
    assert!(active_plan_complete_by_path(
        &data_path,
        &conversation.id,
        "docs/plan/v3.md",
        Some("done"),
    )
    .expect("complete active plan through v3"));
    let completed_plans = chat_metadata_store_read_active_plans(&data_path, &conversation.id)
        .expect("read completed plans")
        .expect("v3 completed plans");
    assert_eq!(completed_plans[0].status, ACTIVE_PLAN_STATUS_COMPLETED);
    assert_eq!(completed_plans[0].completion_text.as_deref(), Some("done"));

    let mut after_append = conversation.clone();
    after_append.messages.push(message("m2", "assistant"));
    chat_metadata_store_append_messages(&paths, &ConversationPersistMeta::from_conversation(&after_append), &after_append.messages[1..])
        .expect("append through v3");
    assert_eq!(chat_metadata_store_read_recent_page(&paths, 10, false).expect("recent").messages.len(), 2);
    assert!(paths.manifest_file.exists() && paths.index_file.exists() && paths.meta_file.exists());

    let mut replacement = after_append.messages[1].clone();
    replacement.parts = vec![MessagePart::Text { text: "replaced".to_string(), reasoning_content: None }];
    after_append.messages[1] = replacement.clone();
    chat_metadata_store_replace_message(&paths, &ConversationPersistMeta::from_conversation(&after_append), &replacement)
        .expect("replace through v3");
    assert_eq!(build_conversation_preview_text(&chat_metadata_store_read_message_by_id(&paths, "m2").expect("read replacement")), "replaced");

    let mut batch_replacements = after_append.messages.clone();
    batch_replacements[0].provider_meta = Some(serde_json::json!({ "batch": "first" }));
    batch_replacements[1].provider_meta = Some(serde_json::json!({ "batch": "second" }));
    after_append.messages = batch_replacements.clone();
    chat_metadata_store_replace_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&after_append),
        &batch_replacements,
    ).expect("batch replace through one v3 publish");
    assert_eq!(
        chat_metadata_store_read_message_by_id(&paths, "m1")
            .expect("read first batch replacement")
            .provider_meta,
        Some(serde_json::json!({ "batch": "first" }))
    );
    assert_eq!(
        chat_metadata_store_read_message_by_id(&paths, "m2")
            .expect("read second batch replacement")
            .provider_meta,
        Some(serde_json::json!({ "batch": "second" }))
    );

    let mut after_truncate = after_append.clone();
    after_truncate.messages.truncate(1);
    chat_metadata_store_truncate_messages(&paths, &ConversationPersistMeta::from_conversation(&after_truncate), 1)
        .expect("truncate through v3");
    assert_eq!(chat_metadata_store_read_recent_page(&paths, 10, false).expect("recent after truncate").messages.len(), 1);

    let after_splice = message("m3", "assistant");
    chat_metadata_store_splice_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&Conversation { messages: vec![after_splice.clone()], ..after_truncate.clone() }),
        0,
        1,
        std::slice::from_ref(&after_splice),
    ).expect("splice through v3");
    assert_eq!(chat_metadata_store_read_message_by_id(&paths, "m3").expect("read splice").id, "m3");
    assert!(chat_metadata_store_read_message_by_id(&paths, "m1").is_err());

    let concurrent_a = message("m4", "assistant");
    let concurrent_b = message("m5", "tool");
    let concurrent_meta = ConversationPersistMeta::from_conversation(&Conversation {
        messages: vec![after_splice.clone(), concurrent_a.clone(), concurrent_b.clone()],
        ..after_truncate.clone()
    });
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let first_paths = paths.clone();
    let first_meta = concurrent_meta.clone();
    let first_message = concurrent_a.clone();
    let first_barrier = std::sync::Arc::clone(&barrier);
    let first = std::thread::spawn(move || {
        first_barrier.wait();
        chat_metadata_store_append_messages(&first_paths, &first_meta, std::slice::from_ref(&first_message))
    });
    let second_paths = paths.clone();
    let second_meta = concurrent_meta.clone();
    let second_message = concurrent_b.clone();
    let second_barrier = std::sync::Arc::clone(&barrier);
    let second = std::thread::spawn(move || {
        second_barrier.wait();
        chat_metadata_store_append_messages(&second_paths, &second_meta, std::slice::from_ref(&second_message))
    });
    first.join().expect("join first append").expect("append first concurrent message");
    second.join().expect("join second append").expect("append second concurrent message");
    let mut concurrent_ids = chat_metadata_store_read_recent_page(&paths, 10, false)
        .expect("read concurrent append result")
        .messages
        .into_iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();
    concurrent_ids.sort();
    assert_eq!(concurrent_ids, vec!["m3", "m4", "m5"]);

    let mut summary = message("summary", "user");
    summary.provider_meta = Some(serde_json::json!({
        "message_meta": { "kind": "context_compaction" }
    }));
    let current_user = message("m6", "user");
    let after_compaction = Conversation {
        messages: vec![
            after_splice,
            concurrent_a,
            concurrent_b,
            summary.clone(),
            current_user.clone(),
        ],
        ..after_truncate
    };
    chat_metadata_store_append_messages(
        &paths,
        &ConversationPersistMeta::from_conversation(&after_compaction),
        &[summary, current_user],
    )
    .expect("append compaction and current user");

    let current_context = chat_metadata_store_compaction_segment(&paths, None)
        .expect("read current context");
    assert_eq!(
        current_context
            .messages
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        vec!["summary", "m6"]
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_migration_should_keep_legacy_conversation_file_without_blocking() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-legacy-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let legacy_dir = app_layout_chat_conversations_dir(&data_path);
    fs::create_dir_all(&legacy_dir).expect("create legacy chat dir");
    fs::write(legacy_dir.join("legacy.json"), "{}").expect("write legacy conversation");
    chat_metadata_store_run_v3_migration(&data_path)
        .expect("legacy must not block v3 startup migration");
    assert!(legacy_dir.join("legacy.json").exists());
    assert!(chat_metadata_store_is_ready(&data_path).expect("global v3 may advance"));
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_snapshot_should_wait_for_same_conversation_writer() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-snapshot-writer-gate-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    chat_metadata_store_run_v3_migration(&data_path).expect("initialize v3 storage");
    let conversation = Conversation {
        id: "snapshot-writer-gate-conversation".to_string(),
        title: "snapshot writer gate".to_string(),
        agent_id: DEFAULT_AGENT_ID.to_string(),
        bound_conversation_id: None,
        parent_conversation_id: None,
        child_conversation_ids: Vec::new(),
        fork_message_cursor: None,
        unread_count: 0,
        conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
        root_conversation_id: None,
        delegate_id: None,
        created_at: now_iso(),
        updated_at: now_iso(),
        last_user_at: None,
        last_assistant_at: None,
        status: "active".to_string(),
        user_profile_snapshot: String::new(),
        shell_workspace_path: None,
        shell_workspaces: Vec::new(),
        shell_autonomous_mode: false,
        shell_work_mode: default_shell_work_mode(),
        shell_work_branch: String::new(),
        archived_at: None,
        messages: Vec::new(),
        fast_request_turns: Vec::new(),
        current_todos: Vec::new(),
        memory_recall_table: Vec::new(),
        plan_mode_enabled: false,
        preferred_api_config_id: None,
        auto_push_remote_contact_id: None,
        active_goal: None, last_error: None,
        cumulative_usage: ConversationCumulativeUsage::default(),
        is_draft: false,
    };
    let paths = message_store_paths(&data_path, &conversation.id).expect("paths");
    let (writer_entered_tx, writer_entered_rx) = std::sync::mpsc::channel();
    let (release_writer_tx, release_writer_rx) = std::sync::mpsc::channel();
    let writer_paths = paths.clone();
    let writer = std::thread::spawn(move || {
        chat_metadata_store_with_writer_gate(&writer_paths, || {
            writer_entered_tx.send(()).expect("notify writer entered");
            release_writer_rx.recv().expect("release writer");
            Ok(())
        })
    });
    writer_entered_rx.recv().expect("wait writer entered");

    let snapshot_completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let snapshot_completed_for_thread = std::sync::Arc::clone(&snapshot_completed);
    let (snapshot_started_tx, snapshot_started_rx) = std::sync::mpsc::channel();
    let snapshot_paths = paths.clone();
    let snapshot_conversation = conversation.clone();
    let snapshot = std::thread::spawn(move || {
        snapshot_started_tx.send(()).expect("notify snapshot start");
        let result = chat_metadata_store_write_snapshot(&snapshot_paths, &snapshot_conversation);
        snapshot_completed_for_thread.store(true, std::sync::atomic::Ordering::Release);
        result
    });
    snapshot_started_rx.recv().expect("wait snapshot start");
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(!snapshot_completed.load(std::sync::atomic::Ordering::Acquire));

    release_writer_tx.send(()).expect("release writer");
    writer.join().expect("join writer").expect("writer result");
    snapshot.join().expect("join snapshot").expect("snapshot result");
    assert!(snapshot_completed.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(
        chat_metadata_store_read_conversation(&paths)
            .expect("read written snapshot")
            .expect("snapshot conversation exists")
            .id,
        conversation.id,
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_delete_should_wait_for_same_conversation_reader() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-delete-gate-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    chat_metadata_store_run_v3_migration(&data_path).expect("initialize v3 storage");
    let (reader_entered_tx, reader_entered_rx) = std::sync::mpsc::channel();
    let (release_reader_tx, release_reader_rx) = std::sync::mpsc::channel();
    let reader_data_path = data_path.clone();
    let reader = std::thread::spawn(move || {
        let reader_paths = message_store_paths(&reader_data_path, "delete-gate-conversation")
            .expect("reader paths");
        chat_metadata_store_with_read_snapshot(&reader_paths, || {
            reader_entered_tx.send(()).expect("notify reader entered");
            release_reader_rx.recv().expect("release reader");
            Ok(())
        })
    });
    reader_entered_rx.recv().expect("wait reader entered");

    let deletion_completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let deletion_completed_for_thread = std::sync::Arc::clone(&deletion_completed);
    let (delete_started_tx, delete_started_rx) = std::sync::mpsc::channel();
    let delete_data_path = data_path.clone();
    let deleter = std::thread::spawn(move || {
        let delete_paths = message_store_paths(&delete_data_path, "delete-gate-conversation")
            .expect("delete paths");
        delete_started_tx.send(()).expect("notify delete start");
        let result = delete_message_store_shard_artifacts(&delete_paths);
        deletion_completed_for_thread.store(true, std::sync::atomic::Ordering::Release);
        result
    });
    delete_started_rx.recv().expect("wait delete start");
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(!deletion_completed.load(std::sync::atomic::Ordering::Acquire));

    release_reader_tx.send(()).expect("release reader");
    reader.join().expect("join reader").expect("reader result");
    deleter.join().expect("join deleter").expect("delete result");
    assert!(deletion_completed.load(std::sync::atomic::Ordering::Acquire));
    let _ = fs::remove_dir_all(root);
}

#[cfg(test)]
#[test]
fn v3_chat_metadata_publication_gate_should_wait_only_for_same_conversation() {
    let root = std::env::temp_dir().join(format!("eca-chat-v3-publication-gate-{}", Uuid::new_v4()));
    let data_path = root.join("config_mark");
    let paths = message_store_paths(&data_path, "same-conversation").expect("same paths");
    let other_paths = message_store_paths(&data_path, "other-conversation").expect("other paths");
    let publication_gate = chat_metadata_store_publication_gate(&paths);
    let writer_guard = publication_gate.write().expect("lock publication writer");
    let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader_data_path = data_path.clone();
    let reader_started = std::sync::Arc::clone(&started);
    let reader_completed = std::sync::Arc::clone(&completed);
    let reader = std::thread::spawn(move || {
        let reader_paths = message_store_paths(&reader_data_path, "same-conversation").expect("reader paths");
        reader_started.store(true, std::sync::atomic::Ordering::Release);
        chat_metadata_store_with_read_snapshot(&reader_paths, || {
            reader_completed.store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        })
    });
    while !started.load(std::sync::atomic::Ordering::Acquire) {
        std::thread::yield_now();
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(!completed.load(std::sync::atomic::Ordering::Acquire));
    chat_metadata_store_with_read_snapshot(&other_paths, || Ok(())).expect("other conversation does not wait");
    drop(writer_guard);
    reader.join().expect("join reader").expect("same conversation waits then reads");
    assert!(completed.load(std::sync::atomic::Ordering::Acquire));
    let _ = fs::remove_dir_all(root);
}