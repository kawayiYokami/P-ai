// ==================== 聊天索引库损坏自修复 ====================
//
// SQLite 的坏页无法原地删改：改动坏页所在的 b-tree 页本身会再次报 malformed，
// 连 DELETE 都过不去。所以修复只能是换库文件——建一份新库，按会话搬运仍可读的行，
// 把读不出来的会话按块文件重建索引行，校验通过后再原子替换原库。
//
// 块文件被视为完好真相：这里只读块文件，不改写、不重建块文件本身。

const CHAT_METADATA_REPAIR_TEMP_TAG: &str = "repair";
const CHAT_METADATA_REPAIR_BACKUP_TAG: &str = "broken";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChatStorageHealth {
    pub conversation_id: String,
    pub corrupted: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChatStorageRepairOutcome {
    pub rebuilt_conversation_ids: Vec<String>,
    pub rebuilt_message_count: usize,
    pub balanced_conversation_count: usize,
    /// 索引读不出来、块文件又救不回来的会话，逐条带原因报给用户。
    pub failed_conversations: Vec<ChatStorageRepairFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChatStorageRepairFailure {
    pub conversation_id: String,
    pub detail: String,
}

fn chat_metadata_sidecar_path(db_path: &PathBuf, suffix: &str) -> PathBuf {
    let mut raw = db_path.clone().into_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

/// 只读探活：按会话读一遍块索引与消息定位，能把坏页读出来就够了。
pub(super) fn chat_metadata_store_probe_conversation(
    data_path: &PathBuf,
    conversation_id: &str,
) -> ChatStorageHealth {
    let mut health = ChatStorageHealth {
        conversation_id: conversation_id.to_string(),
        corrupted: false,
        detail: String::new(),
    };
    // 探活是一次真实的库访问，必须排在整库修复之后：
    // 否则修复正把库文件改名换位的瞬间，探活可能读到不存在的路径，得出错误的「健康」。
    let library_gate = chat_metadata_library_gate(data_path);
    let _library_guard = library_gate
        .read()
        .unwrap_or_else(|poison| poison.into_inner());
    let db_path = chat_metadata_store_db_path(data_path);
    if !db_path.exists() {
        return health;
    }
    let conn = match chat_metadata_store_open(data_path) {
        Ok(conn) => conn,
        Err(err) => {
            health.corrupted = true;
            health.detail = err;
            return health;
        }
    };
    if let Err(err) = chat_metadata_store_probe_conversation_rows(&conn, conversation_id) {
        health.corrupted = true;
        health.detail = err;
    }
    health
}

fn chat_metadata_store_probe_conversation_rows(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<(), String> {
    let mut block_statement = conn
        .prepare(
            "SELECT block_id, block_file, byte_len, message_count FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
        )
        .map_err(|err| format!("准备读取会话块索引失败: {err}"))?;
    let block_rows = block_statement
        .query_map([conversation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|err| format!("读取会话块索引失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析会话块索引失败: {err}"))?;
    drop(block_rows);

    let mut locator_statement = conn
        .prepare(
            "SELECT sequence, message_id, block_id, byte_offset, byte_len FROM message_locator WHERE conversation_id=?1 ORDER BY sequence ASC",
        )
        .map_err(|err| format!("准备读取会话消息定位失败: {err}"))?;
    let locator_rows = locator_statement
        .query_map([conversation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|err| format!("读取会话消息定位失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析会话消息定位失败: {err}"))?;
    drop(locator_rows);
    Ok(())
}

/// 整库重建修复。`conversation_id` 是用户点按钮的会话，只用于日志与结果标注；
/// 坏页可能牵连同表其他行，所以搬运按全库进行，读不出来的会话一律按块文件重建。
pub(super) fn chat_metadata_store_repair_conversation(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<ChatStorageRepairOutcome, String> {
    // 库级写锁：整个修复期间独占该库，所有写入排在后面。
    // 只锁目标会话是不够的，别的会话照样会往旧库提交，换库后那批写入就丢了。
    let library_gate = chat_metadata_library_gate(data_path);
    let _library_guard = library_gate
        .write()
        .unwrap_or_else(|poison| poison.into_inner());
    let db_path = chat_metadata_store_db_path(data_path);
    if !db_path.exists() {
        return Err("聊天索引库不存在，无需修复".to_string());
    }
    runtime_log_info(format!(
        "[聊天索引修复] 开始，conversation_id={conversation_id}，path={}",
        db_path.display()
    ));
    // 源库直接打开，不走 chat_metadata_store_open：那条路会执行建表语句，
    // 在坏库上可能连打开都失败，修复需要尽量连上去读。
    let source = rusqlite::Connection::open(&db_path).map_err(|err| {
        format!(
            "打开聊天索引库失败，path={}，error={err}",
            db_path.display()
        )
    })?;
    source
        .execute_batch("PRAGMA busy_timeout=10000;")
        .map_err(|err| format!("设置聊天索引库等待超时失败: {err}"))?;

    let temp_path = db_path.with_file_name(format!(
        "{CHAT_METADATA_DB_FILE_NAME}.{CHAT_METADATA_REPAIR_TEMP_TAG}.{}",
        Uuid::new_v4()
    ));
    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }
    let mut target = rusqlite::Connection::open(&temp_path).map_err(|err| {
        format!(
            "创建修复用临时库失败，path={}，error={err}",
            temp_path.display()
        )
    })?;
    target
        .execute_batch(CHAT_METADATA_SCHEMA_SQL_PREFIX)
        .map_err(|err| format!("初始化修复用临时库失败: {err}"))?;

    let repaired = chat_metadata_store_repair_into(data_path, &source, &mut target);
    let outcome = match repaired {
        Ok(outcome) => outcome,
        Err(err) => {
            drop(target);
            chat_metadata_remove_db_files(&temp_path);
            return Err(err);
        }
    };

    // 收干 WAL 再关：不留下 -wal/-shm 一起搬。
    let _ = target.query_row("PRAGMA journal_mode=DELETE", [], |row| {
        row.get::<_, String>(0)
    });
    drop(target);

    let verified = chat_metadata_store_verify_repaired_db(&temp_path);
    if let Err(err) = verified {
        chat_metadata_remove_db_files(&temp_path);
        return Err(err);
    }
    drop(source);

    let backup_path = match chat_metadata_store_backup_existing_db(&db_path) {
        Ok(path) => path,
        Err(err) => {
            // 备份阶段失败时旧库仍在原位（失败内部已尝试回滚），清掉临时库再报错。
            chat_metadata_remove_db_files(&temp_path);
            return Err(err);
        }
    };
    if let Err(err) = fs::rename(&temp_path, &db_path) {
        // 回滚必须把 -wal/-shm 一起搬回原名，否则主库回来了、预写文件还留在备份名下，
        // 那批已提交但未 checkpoint 的事务就凭空消失了。
        let restore_error = chat_metadata_restore_backup_db(&backup_path, &db_path).err();
        chat_metadata_remove_db_files(&temp_path);
        let mut message = format!(
            "替换聊天索引库失败，temp={}，target={}，error={err}",
            temp_path.display(),
            db_path.display()
        );
        if let Some(restore_error) = restore_error {
            message.push_str(&format!("；回滚旧库也失败：{restore_error}"));
        }
        return Err(message);
    }

    // 库文件换了，块文件缓存里的路径可能指向旧布局，整体清空重读。
    retain_message_store_block_file_cache_paths(&std::collections::HashSet::new());
    runtime_log_info(format!(
        "[聊天索引修复] 完成，conversation_id={conversation_id}，重建会话={}，重建消息={}，备份={}",
        outcome.rebuilt_conversation_ids.len(),
        outcome.rebuilt_message_count,
        backup_path.display()
    ));
    Ok(outcome)
}

fn chat_metadata_remove_db_files(db_path: &PathBuf) {
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(chat_metadata_sidecar_path(db_path, "-wal"));
    let _ = fs::remove_file(chat_metadata_sidecar_path(db_path, "-shm"));
}

/// 把备份库连同它的 -wal/-shm 一起搬回原位置。
fn chat_metadata_restore_backup_db(backup_path: &PathBuf, db_path: &PathBuf) -> Result<(), String> {
    fs::rename(backup_path, db_path).map_err(|err| {
        format!(
            "恢复旧库失败，from={}，to={}，error={err}",
            backup_path.display(),
            db_path.display()
        )
    })?;
    for suffix in ["-wal", "-shm"] {
        let mut raw = backup_path.clone().into_os_string();
        raw.push(suffix);
        let from = PathBuf::from(raw);
        if !from.exists() {
            continue;
        }
        let to = chat_metadata_sidecar_path(db_path, suffix);
        fs::rename(&from, &to).map_err(|err| {
            format!(
                "恢复旧库预写文件失败，from={}，to={}，error={err}",
                from.display(),
                to.display()
            )
        })?;
    }
    Ok(())
}

fn chat_metadata_store_verify_repaired_db(temp_path: &PathBuf) -> Result<(), String> {
    let conn = rusqlite::Connection::open(temp_path).map_err(|err| {
        format!(
            "校验修复结果失败：无法打开临时库，path={}，error={err}",
            temp_path.display()
        )
    })?;
    let check: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|err| format!("校验修复结果失败：完整性检查无法执行，error={err}"))?;
    if check != "ok" {
        return Err(format!("校验修复结果失败：重建后的库仍不完整，detail={check}"));
    }
    Ok(())
}

/// 旧库改名留档，连同 -wal/-shm 一起挪走，避免新库继承旧预写文件。
fn chat_metadata_store_backup_existing_db(db_path: &PathBuf) -> Result<PathBuf, String> {
    let stamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let backup_name = format!(
        "{CHAT_METADATA_DB_FILE_NAME}.{CHAT_METADATA_REPAIR_BACKUP_TAG}-{stamp}"
    );
    let backup_path = db_path.with_file_name(&backup_name);
    fs::rename(db_path, &backup_path).map_err(|err| {
        format!(
            "备份损坏的聊天索引库失败，path={}，error={err}",
            db_path.display()
        )
    })?;
    for suffix in ["-wal", "-shm"] {
        let from = chat_metadata_sidecar_path(db_path, suffix);
        if from.exists() {
            let mut raw = backup_path.clone().into_os_string();
            raw.push(suffix);
            let to = PathBuf::from(raw);
            if let Err(err) = fs::rename(&from, &to) {
                // 预写文件留在原名上极危险：新库搬进来后会和一个属于旧库的 -wal 配对，
                // 旧帧可能被套到新库上。这里必须原地放弃并回滚。
                let restore_error = chat_metadata_restore_backup_db(&backup_path, db_path).err();
                let mut message = format!(
                    "迁移旧库预写文件失败，from={}，error={err}",
                    from.display()
                );
                if let Some(restore_error) = restore_error {
                    message.push_str(&format!("；回滚旧库也失败：{restore_error}"));
                }
                return Err(message);
            }
        }
    }
    Ok(backup_path)
}

fn chat_metadata_store_repair_into(
    data_path: &PathBuf,
    source: &rusqlite::Connection,
    target: &mut rusqlite::Connection,
) -> Result<ChatStorageRepairOutcome, String> {
    let metas = {
        let mut statement = source
            .prepare(
                "SELECT conversation_id, metadata_json, storage_revision, updated_at FROM conversation_metadata",
            )
            .map_err(|err| format!("准备读取会话元数据失败: {err}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|err| format!("读取会话元数据失败: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析会话元数据失败: {err}"))?;
        rows
    };

    let transaction = target
        .transaction()
        .map_err(|err| format!("开启修复事务失败: {err}"))?;
    let mut rebuilt_conversation_ids = Vec::<String>::new();
    let mut rebuilt_message_count = 0usize;
    let mut balanced_conversation_count = 0usize;
    let mut failed_conversations = Vec::<ChatStorageRepairFailure>::new();

    for (conversation_id, metadata_json, storage_revision, updated_at) in &metas {
        transaction
            .execute(
                "INSERT OR REPLACE INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at) VALUES(?1, ?2, ?3, ?4)",
                rusqlite::params![conversation_id, metadata_json, storage_revision, updated_at],
            )
            .map_err(|err| format!("写入会话元数据失败，conversation_id={conversation_id}，error={err}"))?;

        let blocks = chat_metadata_source_blocks(source, conversation_id);
        let locators = chat_metadata_source_locators(source, conversation_id);
        let (blocks, locators) = match (blocks, locators) {
            (Ok(blocks), Ok(locators)) => (blocks, locators),
            (blocks, locators) => {
                let detail = blocks
                    .err()
                    .or_else(|| locators.err())
                    .unwrap_or_else(|| "未知原因".to_string());
                runtime_log_warn(format!(
                    "[聊天索引修复] 会话索引行不可读，改按块文件重建，conversation_id={conversation_id}，error={detail}"
                ));
                // 单个会话救不回来不该让整库修复失败：那会让用户唯一的自救通道永久不可用。
                match chat_metadata_rebuild_conversation_rows(
                    data_path,
                    conversation_id,
                    &transaction,
                    &mut rebuilt_message_count,
                ) {
                    Ok(()) => rebuilt_conversation_ids.push(conversation_id.clone()),
                    Err(rebuild_error) => {
                        runtime_log_warn(format!(
                            "[聊天索引修复] 会话重建失败，跳过该会话索引，conversation_id={conversation_id}，error={rebuild_error}"
                        ));
                        failed_conversations.push(ChatStorageRepairFailure {
                            conversation_id: conversation_id.clone(),
                            detail: rebuild_error,
                        });
                    }
                }
                continue;
            }
        };

        for (block_id, block_file, byte_len, message_count) in &blocks {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO conversation_blocks(conversation_id, block_id, block_file, byte_len, message_count) VALUES(?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![conversation_id, block_id, block_file, byte_len, message_count],
                )
                .map_err(|err| format!("写入会话块索引失败，conversation_id={conversation_id}，error={err}"))?;
        }
        for locator in &locators {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        conversation_id,
                        locator.sequence,
                        locator.message_id,
                        locator.block_id,
                        locator.byte_offset,
                        locator.byte_len,
                        locator.compaction_kind,
                        locator.role,
                        locator.created_at
                    ],
                )
                .map_err(|err| format!("写入会话消息定位失败，conversation_id={conversation_id}，error={err}"))?;
        }
        // 计划记录与用量统计是派生数据，读不出来只影响对应条目，不牵连会话索引。
        match chat_metadata_source_active_plans(source, conversation_id) {
            Ok(rows) => {
                for (plan_id, record_json) in rows {
                    transaction
                        .execute(
                            "INSERT OR REPLACE INTO active_plan_records(conversation_id, plan_id, record_json) VALUES(?1, ?2, ?3)",
                            rusqlite::params![conversation_id, plan_id, record_json],
                        )
                        .map_err(|err| format!("写入会话计划记录失败，conversation_id={conversation_id}，error={err}"))?;
                }
            }
            Err(err) => runtime_log_warn(format!(
                "[聊天索引修复] 会话计划记录不可读，跳过，conversation_id={conversation_id}，error={err}"
            )),
        }
        balanced_conversation_count += 1;
    }

    let migrations = chat_metadata_source_rows(
        source,
        "SELECT migration_key, state, updated_at FROM chat_storage_migrations",
        3,
    )?;
    for row in &migrations {
        transaction
            .execute(
                "INSERT OR REPLACE INTO chat_storage_migrations(migration_key, state, updated_at) VALUES(?1, ?2, ?3)",
                rusqlite::params![&row[0], &row[1], &row[2]],
            )
            .map_err(|err| format!("写入迁移状态失败: {err}"))?;
    }

    let operations = chat_metadata_source_rows(
        source,
        "SELECT operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at, committed_at FROM storage_operations",
        8,
    )?;
    for row in &operations {
        transaction
            .execute(
                "INSERT OR REPLACE INTO storage_operations(operation_id, conversation_id, before_revision, after_revision, state, detail_json, created_at, committed_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![&row[0], &row[1], &row[2], &row[3], &row[4], &row[5], &row[6], &row[7]],
            )
            .map_err(|err| format!("写入存储操作记录失败: {err}"))?;
    }

    match chat_metadata_source_usage_trail(source) {
        Ok(rows) => {
            for row in rows {
                transaction
                    .execute(
                        "INSERT OR REPLACE INTO usage_trail(bucket, conversation_id, agent_id, department_id, conversation_kind, api_config_id, provider_key, provider_label, model_name, input_tokens, output_tokens, total_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                        rusqlite::params![
                            row.bucket,
                            row.conversation_id,
                            row.agent_id,
                            row.department_id,
                            row.conversation_kind,
                            row.api_config_id,
                            row.provider_key,
                            row.provider_label,
                            row.model_name,
                            row.input_tokens,
                            row.output_tokens,
                            row.total_tokens,
                            row.cache_read_tokens,
                            row.cache_write_tokens,
                            row.reasoning_tokens,
                            row.updated_at
                        ],
                    )
                    .map_err(|err| format!("写入用量统计失败: {err}"))?;
            }
        }
        Err(err) => runtime_log_warn(format!("[聊天索引修复] 用量统计不可读，跳过，error={err}")),
    }

    transaction
        .commit()
        .map_err(|err| format!("提交修复事务失败: {err}"))?;
    Ok(ChatStorageRepairOutcome {
        rebuilt_conversation_ids,
        rebuilt_message_count,
        balanced_conversation_count,
        failed_conversations,
    })
}

fn chat_metadata_source_rows(
    source: &rusqlite::Connection,
    sql: &str,
    column_count: usize,
) -> Result<Vec<Vec<rusqlite::types::Value>>, String> {
    let mut statement = source
        .prepare(sql)
        .map_err(|err| format!("准备读取修复源数据失败，sql={sql}，error={err}"))?;
    let rows = statement
        .query_map([], |row| {
            let mut values = Vec::with_capacity(column_count);
            for index in 0..column_count {
                values.push(row.get::<_, rusqlite::types::Value>(index)?);
            }
            Ok(values)
        })
        .map_err(|err| format!("读取修复源数据失败，sql={sql}，error={err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析修复源数据失败，sql={sql}，error={err}"))?;
    Ok(rows)
}

struct ChatMetadataSourceLocator {
    sequence: i64,
    message_id: String,
    block_id: i64,
    byte_offset: i64,
    byte_len: i64,
    compaction_kind: Option<String>,
    role: String,
    created_at: String,
}

struct ChatMetadataSourceUsageRow {
    bucket: String,
    conversation_id: String,
    agent_id: String,
    department_id: String,
    conversation_kind: String,
    api_config_id: String,
    provider_key: String,
    provider_label: String,
    model_name: String,
    input_tokens: i64,
    output_tokens: i64,
    total_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    updated_at: String,
}

fn chat_metadata_source_blocks(
    source: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Vec<(i64, String, i64, i64)>, String> {
    let mut statement = source
        .prepare(
            "SELECT block_id, block_file, byte_len, message_count FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
        )
        .map_err(|err| format!("准备读取会话块索引失败: {err}"))?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|err| format!("读取会话块索引失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析会话块索引失败: {err}"))?;
    Ok(rows)
}

fn chat_metadata_source_locators(
    source: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Vec<ChatMetadataSourceLocator>, String> {
    let mut statement = source
        .prepare(
            "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at FROM message_locator WHERE conversation_id=?1 ORDER BY sequence ASC",
        )
        .map_err(|err| format!("准备读取会话消息定位失败: {err}"))?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(ChatMetadataSourceLocator {
                sequence: row.get::<_, i64>(0)?,
                message_id: row.get::<_, String>(1)?,
                block_id: row.get::<_, i64>(2)?,
                byte_offset: row.get::<_, i64>(3)?,
                byte_len: row.get::<_, i64>(4)?,
                compaction_kind: row.get::<_, Option<String>>(5)?,
                role: row.get::<_, String>(6)?,
                created_at: row.get::<_, String>(7)?,
            })
        })
        .map_err(|err| format!("读取会话消息定位失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析会话消息定位失败: {err}"))?;
    Ok(rows)
}

fn chat_metadata_source_active_plans(
    source: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut statement = source
        .prepare("SELECT plan_id, record_json FROM active_plan_records WHERE conversation_id=?1")
        .map_err(|err| format!("准备读取会话计划记录失败: {err}"))?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("读取会话计划记录失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析会话计划记录失败: {err}"))?;
    Ok(rows)
}

fn chat_metadata_source_usage_trail(
    source: &rusqlite::Connection,
) -> Result<Vec<ChatMetadataSourceUsageRow>, String> {
    let mut statement = source
        .prepare(
            "SELECT bucket, conversation_id, agent_id, department_id, conversation_kind, api_config_id, provider_key, provider_label, model_name, input_tokens, output_tokens, total_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, updated_at FROM usage_trail",
        )
        .map_err(|err| format!("准备读取用量统计失败: {err}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(ChatMetadataSourceUsageRow {
                bucket: row.get(0)?,
                conversation_id: row.get(1)?,
                agent_id: row.get(2)?,
                department_id: row.get(3)?,
                conversation_kind: row.get(4)?,
                api_config_id: row.get(5)?,
                provider_key: row.get(6)?,
                provider_label: row.get(7)?,
                model_name: row.get(8)?,
                input_tokens: row.get(9)?,
                output_tokens: row.get(10)?,
                total_tokens: row.get(11)?,
                cache_read_tokens: row.get(12)?,
                cache_write_tokens: row.get(13)?,
                reasoning_tokens: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })
        .map_err(|err| format!("读取用量统计失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析用量统计失败: {err}"))?;
    Ok(rows)
}

/// 索引行读不出来时，按块文件重放该会话的块索引与消息定位。
/// 全程包在保存点里：两类行要么都写进去，要么都不留，避免会话重建失败后留下半个索引。
fn chat_metadata_rebuild_conversation_rows(
    data_path: &PathBuf,
    conversation_id: &str,
    transaction: &rusqlite::Transaction<'_>,
    rebuilt_message_count: &mut usize,
) -> Result<(), String> {
    const SAVEPOINT: &str = "chat_metadata_rebuild_conversation";
    transaction
        .execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(|err| format!("开启会话索引重建保存点失败，conversation_id={conversation_id}，error={err}"))?;
    let result = chat_metadata_rebuild_conversation_rows_inner(
        data_path,
        conversation_id,
        transaction,
        rebuilt_message_count,
    );
    match result {
        Ok(()) => {
            transaction
                .execute_batch(&format!("RELEASE {SAVEPOINT}"))
                .map_err(|err| format!("释放会话索引重建保存点失败，conversation_id={conversation_id}，error={err}"))?;
            Ok(())
        }
        Err(err) => {
            if let Err(rollback_err) = transaction
                .execute_batch(&format!("ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"))
            {
                return Err(format!(
                    "{err}；回滚会话索引重建保存点失败：{rollback_err}"
                ));
            }
            Err(err)
        }
    }
}

fn chat_metadata_rebuild_conversation_rows_inner(
    data_path: &PathBuf,
    conversation_id: &str,
    transaction: &rusqlite::Transaction<'_>,
    rebuilt_message_count: &mut usize,
) -> Result<(), String> {
    let paths = message_store_paths(data_path, conversation_id)?;
    let rebuilt = rebuild_message_store_snapshot_from_blocks(&paths).map_err(|err| {
        format!("按块文件重建会话索引失败，conversation_id={conversation_id}，error={err}")
    })?;
    if rebuilt.index.items.is_empty() {
        return Err(format!(
            "按块文件重建会话索引失败：会话没有可读取的块文件，conversation_id={conversation_id}，path={}",
            paths.blocks_dir.display()
        ));
    }

    let mut counts = std::collections::BTreeMap::<u32, i64>::new();
    for item in &rebuilt.index.items {
        let block_id = item.block_id.ok_or_else(|| {
            format!(
                "按块文件重建会话索引失败：索引项缺少块号，conversation_id={conversation_id}，message_id={}",
                item.message_id
            )
        })?;
        *counts.entry(block_id).or_default() += 1;
    }
    for (block_id, message_count) in &counts {
        let (block_file, byte_len) = chat_metadata_existing_block_file(&paths, *block_id)?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO conversation_blocks(conversation_id, block_id, block_file, byte_len, message_count) VALUES(?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![conversation_id, *block_id as i64, block_file, byte_len, message_count],
            )
            .map_err(|err| format!("重建会话块索引失败，conversation_id={conversation_id}，error={err}"))?;
    }

    for (sequence, item) in rebuilt.index.items.iter().enumerate() {
        transaction
            .execute(
                "INSERT OR REPLACE INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    conversation_id,
                    sequence as i64,
                    item.message_id,
                    item.block_id.unwrap_or(0) as i64,
                    item.offset as i64,
                    item.byte_len as i64,
                    item.compaction_kind,
                    item.role,
                    item.created_at
                ],
            )
            .map_err(|err| format!("重建会话消息定位失败，conversation_id={conversation_id}，error={err}"))?;
    }
    *rebuilt_message_count += rebuilt.index.items.len();
    Ok(())
}

fn chat_metadata_existing_block_file(
    paths: &MessageStorePaths,
    block_id: u32,
) -> Result<(String, i64), String> {
    for extension in ["jsonl.zstd", "jsonl"] {
        let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.{extension}");
        let path = paths.shard_dir.join(&block_file);
        if let Ok(metadata) = fs::metadata(&path) {
            return Ok((block_file, metadata.len() as i64));
        }
    }
    Err(format!(
        "重建会话块索引失败：块文件缺失，conversation_id={}，block_id={}",
        paths.conversation_id, block_id
    ))
}

#[cfg(test)]
mod chat_metadata_repair_tests {
    use super::*;

    const TEST_CONVERSATION_ID: &str = "conversation-repair";
    const PAGE_SIZE: usize = 4096;

    fn test_message(id: &str, text_len: usize) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: "user".to_string(),
            created_at: "2026-10-23T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: format!("{id}-{}", "x".repeat(text_len)),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn test_conversation(id: &str, message_count: usize) -> Conversation {
        Conversation {
            id: id.to_string(),
            title: format!("fake-{id}"),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-10-23T00:00:00Z".to_string(),
            updated_at: "2026-10-23T00:00:00Z".to_string(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_worktree_path: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages: (0..message_count)
                .map(|index| test_message(&format!("m{index:03}"), 64))
                .collect(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None,
            team: TeamState::default(),
            last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    /// 用生产同一条重建逻辑给会话补齐索引行，得到一份健康基线。
    fn seed_index_rows(data_path: &PathBuf, conversation_id: &str) {
        let mut conn = chat_metadata_store_open(data_path).expect("open db");
        let transaction = conn.transaction().expect("begin seed transaction");
        let mut count = 0usize;
        chat_metadata_rebuild_conversation_rows(
            data_path,
            conversation_id,
            &transaction,
            &mut count,
        )
        .expect("seed index rows");
        transaction.commit().expect("commit seed");
    }

    /// 造一份纯假的现场：每个会话一块到多块，块文件按真实格式落盘，索引行用生产逻辑补齐。
    fn prepare_fake_fleet(
        conversation_count: usize,
        message_count: usize,
    ) -> (PathBuf, PathBuf, Vec<String>) {
        let root = std::env::temp_dir().join(format!(
            "easy-call-chat-metadata-repair-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let mut conversation_ids = Vec::new();
        for index in 0..conversation_count {
            let conversation_id = format!("fake-conversation-{index:03}");
            let paths = message_store_paths(&data_path, &conversation_id).expect("paths");
            let conversation = test_conversation(&conversation_id, message_count);
            write_jsonl_snapshot_directory_shard(&paths, &conversation).expect("write shard");
            let meta = read_conversation_shard_meta(&paths.meta_file).expect("read meta");
            let conn = chat_metadata_store_open(&data_path).expect("open db");
            conn.execute(
                "INSERT INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at) VALUES(?1, ?2, ?3, ?4)",
                rusqlite::params![
                    &conversation_id,
                    serde_json::to_string(&meta).expect("serialize meta"),
                    (index + 1) as i64,
                    "2026-10-23T00:00:00Z"
                ],
            )
            .expect("insert metadata");
            drop(conn);
            seed_index_rows(&data_path, &conversation_id);
            conversation_ids.push(conversation_id);
        }
        (root, data_path, conversation_ids)
    }

    /// 把库文件里包含指定字样的那一页打坏，制造真实坏页。
    fn corrupt_page_containing(db_path: &PathBuf, needle: &str) {
        {
            let data_path = db_path.parent().expect("db parent").to_path_buf();
            let conn = chat_metadata_store_open(&data_path).expect("open db");
            let _ = conn.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get::<_, String>(0));
        }
        let raw = fs::read(db_path).expect("read db");
        let offset = raw
            .windows(needle.len())
            .position(|window| window == needle.as_bytes())
            .expect("needle should exist in db file");
        let page_start = offset / PAGE_SIZE * PAGE_SIZE;
        let page_end = (page_start + 200).min(raw.len());
        let mut bytes = raw;
        for byte in bytes.iter_mut().take(page_end).skip(page_start) {
            *byte = 0xFF;
        }
        fs::write(db_path, bytes).expect("write corrupted db");
    }

    fn insert_marker_block_row(data_path: &PathBuf, conversation_id: &str) -> String {
        let marker = format!("blocks/marker-{}.jsonl.zstd", Uuid::new_v4());
        let conn = chat_metadata_store_open(data_path).expect("open db");
        conn.execute(
            "INSERT INTO conversation_blocks(conversation_id, block_id, block_file, byte_len, message_count) VALUES(?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![conversation_id, -1_i64, &marker, 7_i64, 1_i64],
        )
        .expect("insert marker row");
        marker
    }

    #[test]
    fn chat_metadata_repair_should_rebuild_unreadable_conversation_from_blocks() {
        let (root, data_path, conversation_ids) = prepare_fake_fleet(3, 30);
        let target = conversation_ids[1].clone();
        let marker = insert_marker_block_row(&data_path, &target);
        let db_path = chat_metadata_store_db_path(&data_path);
        corrupt_page_containing(&db_path, &marker);

        let health = chat_metadata_store_probe_conversation(&data_path, &target);
        assert!(health.corrupted, "坏页应被探活识别");
        assert!(
            health.detail.contains("malformed"),
            "探活详情应带上底层原因：{}",
            health.detail
        );

        let outcome =
            chat_metadata_store_repair_conversation(&data_path, &target).expect("坏页修复应成功");
        assert!(
            outcome.rebuilt_conversation_ids.contains(&target),
            "坏页会话必须重建，实际={:?}",
            outcome.rebuilt_conversation_ids
        );
        assert!(outcome.failed_conversations.is_empty());

        let conn = chat_metadata_store_open(&data_path).expect("open repaired db");
        let check: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check");
        assert_eq!(check, "ok");

        for conversation_id in &conversation_ids {
            let probe = chat_metadata_store_probe_conversation(&data_path, conversation_id);
            assert!(
                !probe.corrupted,
                "修复后会话应完全可读：{conversation_id}，{}",
                probe.detail
            );
            let paths = message_store_paths(&data_path, conversation_id).expect("paths");
            let expected_blocks = fs::read_dir(&paths.blocks_dir)
                .expect("read blocks dir")
                .filter_map(Result::ok)
                .count();
            let block_rows: i64 = conn
                .query_row(
                    "SELECT count(*) FROM conversation_blocks WHERE conversation_id=?1",
                    [conversation_id],
                    |row| row.get(0),
                )
                .expect("count blocks");
            assert_eq!(block_rows as usize, expected_blocks);
            let locator_rows: i64 = conn
                .query_row(
                    "SELECT count(*) FROM message_locator WHERE conversation_id=?1",
                    [conversation_id],
                    |row| row.get(0),
                )
                .expect("count locators");
            assert_eq!(locator_rows, 30);
            let loaded = chat_store_read_rewind_slice(&paths, "m015")
                .unwrap_or_else(|err| panic!("会话 {conversation_id} 撤回读取失败：{err}"));
            assert!(loaded.is_some());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn chat_metadata_repair_should_not_touch_silently_missing_rows() {
        let (root, data_path, conversation_ids) = prepare_fake_fleet(2, 10);
        let target = conversation_ids[0].clone();
        {
            let conn = chat_metadata_store_open(&data_path).expect("open db");
            conn.execute(
                "DELETE FROM conversation_blocks WHERE conversation_id=?1",
                [&target],
            )
            .expect("delete blocks");
            conn.execute(
                "DELETE FROM message_locator WHERE conversation_id=?1",
                [&target],
            )
            .expect("delete locators");
        }

        let outcome =
            chat_metadata_store_repair_conversation(&data_path, &target).expect("repair");
        assert!(
            outcome.rebuilt_conversation_ids.is_empty(),
            "行只是缺失、读得出来时不该重建，实际={:?}",
            outcome.rebuilt_conversation_ids
        );
        let conn = chat_metadata_store_open(&data_path).expect("open db");
        let block_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM conversation_blocks WHERE conversation_id=?1",
                [&target],
                |row| row.get(0),
            )
            .expect("count blocks");
        assert_eq!(block_rows, 0, "只有读不出来才重建");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn chat_metadata_repair_should_report_unrecoverable_conversation_without_aborting() {
        let (root, data_path, conversation_ids) = prepare_fake_fleet(3, 10);
        let target = conversation_ids[1].clone();
        let marker = insert_marker_block_row(&data_path, &target);
        let db_path = chat_metadata_store_db_path(&data_path);
        corrupt_page_containing(&db_path, &marker);
        // 块文件也一起坏掉：这个会话救不回来。
        let paths = message_store_paths(&data_path, &target).expect("paths");
        for entry in fs::read_dir(&paths.blocks_dir).expect("read blocks dir").filter_map(Result::ok) {
            fs::write(entry.path(), "not a jsonl block\n").expect("write broken block");
        }

        let outcome =
            chat_metadata_store_repair_conversation(&data_path, &target).expect("整库修复仍应完成");
        assert!(
            outcome
                .failed_conversations
                .iter()
                .any(|item| item.conversation_id == target && item.detail.contains("块文件")),
            "救不回来的会话要逐条报出来：{:?}",
            outcome.failed_conversations
        );
        // 其他会话不受牵连，库本身完整。
        let conn = chat_metadata_store_open(&data_path).expect("open repaired db");
        let check: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check");
        assert_eq!(check, "ok");
        for conversation_id in conversation_ids.iter().filter(|id| **id != target) {
            let probe = chat_metadata_store_probe_conversation(&data_path, conversation_id);
            assert!(!probe.corrupted, "其他会话应完好：{conversation_id}");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn chat_metadata_rebuild_should_rollback_partial_rows_when_locator_write_fails() {
        let (root, data_path, conversation_ids) = prepare_fake_fleet(1, 3);
        let target = conversation_ids[0].clone();
        {
            let conn = chat_metadata_store_open(&data_path).expect("open db");
            conn.execute("DELETE FROM conversation_blocks WHERE conversation_id=?1", [&target])
                .expect("delete blocks");
            conn.execute("DELETE FROM message_locator WHERE conversation_id=?1", [&target])
                .expect("delete locators");
            // 让第二条定位行写入必然失败，制造「块行已写、定位行没写完」的中断现场。
            conn.execute_batch(
                "CREATE TRIGGER test_block_locator BEFORE INSERT ON message_locator
                 WHEN NEW.sequence = 1 BEGIN SELECT RAISE(ABORT, 'injected locator failure'); END;",
            )
            .expect("create trigger");
        }

        {
            let mut conn = chat_metadata_store_open(&data_path).expect("open db");
            let transaction = conn.transaction().expect("begin");
            let mut count = 0usize;
            let error = chat_metadata_rebuild_conversation_rows(
                &data_path,
                &target,
                &transaction,
                &mut count,
            )
            .expect_err("注入失败应让重建报错");
            assert!(error.contains("重建会话消息定位失败"), "错误应指明定位行写入失败：{error}");
            transaction.commit().expect("commit rolled-back transaction");
        }

        let conn = chat_metadata_store_open(&data_path).expect("open db");
        for table in ["conversation_blocks", "message_locator"] {
            let rows: i64 = conn
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE conversation_id=?1"),
                    [&target],
                    |row| row.get(0),
                )
                .expect("count rows");
            assert_eq!(rows, 0, "{table} 里不该留下重建失败的残行");
        }
        drop(conn);

        // 去掉注入后应能正常重建。
        {
            let conn = chat_metadata_store_open(&data_path).expect("open db");
            conn.execute_batch("DROP TRIGGER test_block_locator").expect("drop trigger");
        }
        let mut conn = chat_metadata_store_open(&data_path).expect("open db");
        let transaction = conn.transaction().expect("begin");
        let mut count = 0usize;
        chat_metadata_rebuild_conversation_rows(&data_path, &target, &transaction, &mut count)
            .expect("重试应成功");
        transaction.commit().expect("commit");
        assert_eq!(count, 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn chat_metadata_restore_backup_should_bring_back_wal_sidecars() {
        let (root, data_path, _conversation_ids) = prepare_fake_fleet(1, 3);
        let db_path = chat_metadata_store_db_path(&data_path);
        let backup_path = chat_metadata_store_backup_existing_db(&db_path).expect("backup");
        assert!(!db_path.exists(), "备份后原位置应已让出");
        // 模拟换库失败后的现场：主库与预写文件都停在备份名下。
        for suffix in ["-wal", "-shm"] {
            let mut raw = backup_path.clone().into_os_string();
            raw.push(suffix);
            fs::write(PathBuf::from(raw), b"sidecar").expect("write sidecar");
        }

        chat_metadata_restore_backup_db(&backup_path, &db_path).expect("restore");
        assert!(db_path.exists(), "旧库应回到原位");
        for suffix in ["-wal", "-shm"] {
            assert!(
                chat_metadata_sidecar_path(&db_path, suffix).exists(),
                "预写文件 {suffix} 必须搬回原名"
            );
            let mut raw = backup_path.clone().into_os_string();
            raw.push(suffix);
            assert!(!PathBuf::from(raw).exists(), "备份名下不该残留 {suffix}");
        }
        let _ = fs::remove_dir_all(root);
    }
}

