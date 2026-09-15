#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MigrationV1ToV2Failure {
    ConversationSkipped(String),
    SystemFailure(String),
}

impl MigrationV1ToV2Failure {
    fn into_message(self) -> String {
        match self {
            Self::ConversationSkipped(message) | Self::SystemFailure(message) => message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MigrationV2ToV3Failure {
    ConversationSkipped(String),
    SystemFailure(String),
}

fn migration_v2_io_failure(
    operation: &str,
    path: &PathBuf,
    error: std::io::Error,
) -> MigrationV2ToV3Failure {
    let message = format!(
        "{operation}，path={}，error={error}",
        path.display()
    );
    if error.kind() == std::io::ErrorKind::NotFound {
        MigrationV2ToV3Failure::ConversationSkipped(message)
    } else {
        MigrationV2ToV3Failure::SystemFailure(message)
    }
}

fn migration_v2_parse_failure(
    operation: &str,
    path: &PathBuf,
    error: impl std::fmt::Display,
) -> MigrationV2ToV3Failure {
    MigrationV2ToV3Failure::ConversationSkipped(format!(
        "{operation}，path={}，error={error}",
        path.display()
    ))
}

fn migration_v2_path_exists(
    path: &PathBuf,
) -> Result<bool, MigrationV2ToV3Failure> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(migration_v2_io_failure("检查 V2 文件失败", path, error)),
    }
}

pub(super) fn migration_read_v1_conversation(
    path: &PathBuf,
) -> Result<Conversation, MigrationV1ToV2Failure> {
    let raw = fs::read_to_string(path).map_err(|err| {
        let message = format!(
            "读取 V1 会话文件失败，path={}，error={err}",
            path.display()
        );
        if err.kind() == std::io::ErrorKind::NotFound {
            MigrationV1ToV2Failure::ConversationSkipped(message)
        } else {
            MigrationV1ToV2Failure::SystemFailure(message)
        }
    })?;
    serde_json::from_str::<Conversation>(&raw).map_err(|err| {
        MigrationV1ToV2Failure::ConversationSkipped(format!(
            "解析 V1 会话文件失败，path={}，error={err}",
            path.display()
        ))
    })
}

/// 独立的 V1→V2 转换入口。
///
/// 这里不调用生产持久化入口；只读取传入的 V1 `Conversation`，构造 V2
/// manifest/meta/index/blocks，并按文件顺序发布。`dry_run` 只做格式和内容
/// 校验，不创建任何目标文件。
pub(super) fn migration_v1_to_v2_conversation(
    paths: &MessageStorePaths,
    conversation: &Conversation,
    dry_run: bool,
) -> Result<(), String> {
    migration_v1_to_v2_conversation_classified(paths, conversation, dry_run)
        .map_err(MigrationV1ToV2Failure::into_message)
}

pub(super) fn migration_v1_to_v2_conversation_classified(
    paths: &MessageStorePaths,
    conversation: &Conversation,
    dry_run: bool,
) -> Result<(), MigrationV1ToV2Failure> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    let conversation_id = normalized_conversation.id.trim();
    if conversation_id.is_empty() || conversation_id != paths.conversation_id {
        return Err(MigrationV1ToV2Failure::ConversationSkipped(format!(
            "V1 会话 ID 与目标路径不一致，conversation_id={}，path_id={}",
            conversation_id, paths.conversation_id
        )));
    }
    let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&normalized_conversation)
        .map_err(MigrationV1ToV2Failure::ConversationSkipped)?;
    let expected_last_message_id = normalized_conversation
        .messages
        .last()
        .map(|message| message.id.trim().to_string())
        .unwrap_or_default();
    if blocks.message_count != normalized_conversation.messages.len()
        || blocks.last_message_id != expected_last_message_id
    {
        return Err(MigrationV1ToV2Failure::ConversationSkipped(format!(
            "V1→V2 构建结果不一致，conversation_id={}，expected_count={}，actual_count={}，expected_last={}，actual_last={}",
            paths.conversation_id,
            normalized_conversation.messages.len(),
            blocks.message_count,
            expected_last_message_id,
            blocks.last_message_id
        )));
    }
    if dry_run {
        return Ok(());
    }
    let manifest = MessageStoreManifest::jsonl_snapshot_building(&normalized_conversation)
        .jsonl_snapshot_ready(blocks.total_bytes, 1);
    let meta = ConversationShardMeta::from_conversation(&normalized_conversation);
    if migration_v1_to_v2_target_matches(paths, &manifest, &meta, &blocks) {
        return Ok(());
    }
    write_conversation_shard_meta_atomic(&paths.meta_file, &meta)
        .map_err(MigrationV1ToV2Failure::SystemFailure)?;
    migration_write_v2_blocks(paths, &blocks)
        .map_err(MigrationV1ToV2Failure::SystemFailure)?;
    write_message_store_index_atomic(&paths.index_file, &blocks.index)
        .map_err(MigrationV1ToV2Failure::SystemFailure)?;
    write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
        .map_err(MigrationV1ToV2Failure::SystemFailure)?;
    Ok(())
}

fn migration_v1_to_v2_target_matches(
    paths: &MessageStorePaths,
    manifest: &MessageStoreManifest,
    meta: &ConversationShardMeta,
    blocks: &JsonlSnapshotConversationBlocks,
) -> bool {
    let stored_manifest = migration_read_v2_manifest(paths).ok();
    let stored_meta = migration_read_v2_meta(paths).ok();
    let stored_index = migration_read_v2_index(paths).ok();
    stored_manifest.as_ref().is_some_and(|stored| {
        stored.should_read_jsonl()
            && stored.source_message_count() == manifest.source_message_count()
            && stored.last_message_id() == manifest.last_message_id()
            && stored.messages_jsonl_bytes() == manifest.messages_jsonl_bytes()
    })
        && stored_meta.as_ref() == Some(meta)
        && stored_index.as_ref().is_some_and(|stored| {
            stored.persistent_view().items == blocks.index.persistent_view().items
        })
        && migration_rebuild_v2_snapshot(paths, &blocks.index)
            .is_ok_and(|rebuilt| {
                rebuilt.index.persistent_view().items == blocks.index.persistent_view().items
                    && rebuilt.total_bytes == blocks.total_bytes
            })
}

fn migration_write_v2_blocks(
    paths: &MessageStorePaths,
    blocks: &JsonlSnapshotConversationBlocks,
) -> Result<(), String> {
    fs::create_dir_all(&paths.blocks_dir).map_err(|err| {
        format!(
            "迁移创建 V2 block 目录失败，conversation_id={}，path={}，error={err}",
            paths.conversation_id,
            paths.blocks_dir.display()
        )
    })?;
    for block in &blocks.blocks {
        write_jsonl_snapshot_atomic(&paths.shard_dir.join(&block.block_file), &block.content)?;
    }
    // V1→V2 只发布新的 manifest/meta/index/block 内容，不清理已有 V2
    // artifact。旧文件可能仍是后续 V2→V3 的迁移源，且保留它们便于失败后
    // 重试和人工取证；未被新 index 引用的 block 不影响 current store 读取。
    Ok(())
}

// ==================== 独立 V2→V3 迁移读取/写入 ====================
//
// 这些函数故意不调用 chat_store_*、运行时缓存或生产恢复逻辑。
// 它们只把磁盘上的 V2 文件解析成迁移所需的中间数据，再以单事务写入 V3。

const MIGRATION_V3_DB_FILE_NAME: &str = "chat_metadata.sqlite";
const MIGRATION_V3_COMPLETED_KEY: &str = "v3_chat_metadata_sqlite";

fn migration_v3_db_path(data_path: &PathBuf) -> PathBuf {
    app_layout_chat_dir(data_path).join(MIGRATION_V3_DB_FILE_NAME)
}

fn migration_open_v3_database(data_path: &PathBuf) -> Result<rusqlite::Connection, String> {
    let db_path = migration_v3_db_path(data_path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "迁移创建 V3 数据库目录失败，path={}，error={err}",
                parent.display()
            )
        })?;
    }
    let conn = rusqlite::Connection::open(&db_path).map_err(|err| {
        format!(
            "迁移打开 V3 数据库失败，path={}，error={err}",
            db_path.display()
        )
    })?;
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
         );",
    )
    .map_err(|err| format!("迁移初始化 V3 数据库失败: {err}"))?;
    Ok(conn)
}

fn migration_v3_is_completed(data_path: &PathBuf, migration_key: &str) -> Result<bool, String> {
    if !migration_v3_db_path(data_path).exists() {
        return Ok(false);
    }
    let conn = migration_open_v3_database(data_path)?;
    conn.query_row(
        "SELECT state FROM chat_storage_migrations WHERE migration_key=?1",
        [migration_key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|state| state.as_deref() == Some("completed"))
    .map_err(|err| format!("迁移读取 V3 状态失败，migration_key={migration_key}，error={err}"))
}

fn migration_v3_mark_completed(data_path: &PathBuf, migration_key: &str) -> Result<(), String> {
    let conn = migration_open_v3_database(data_path)?;
    conn.execute(
        "INSERT INTO chat_storage_migrations(migration_key, state, updated_at) VALUES(?1, 'completed', ?2)
         ON CONFLICT(migration_key) DO UPDATE SET state='completed', updated_at=excluded.updated_at",
        rusqlite::params![migration_key, now_iso()],
    )
    .map_err(|err| {
        format!("迁移写入 V3 状态失败，migration_key={migration_key}，error={err}")
    })?;
    Ok(())
}

fn migration_v3_contains_conversation(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let conn = migration_open_v3_database(data_path)?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_metadata WHERE conversation_id=?1)",
        [conversation_id],
        |row| row.get::<_, bool>(0),
    )
    .map_err(|err| {
        format!(
            "迁移确认 V3 会话失败，conversation_id={conversation_id}，error={err}"
        )
    })
}

fn migration_read_v2_manifest(
    paths: &MessageStorePaths,
) -> Result<MessageStoreManifest, MigrationV2ToV3Failure> {
    let raw = fs::read_to_string(&paths.manifest_file).map_err(|err| {
        migration_v2_io_failure("迁移读取 V2 manifest 失败", &paths.manifest_file, err)
    })?;
    let manifest = serde_json::from_str::<MessageStoreManifest>(&raw).map_err(|err| {
        migration_v2_parse_failure("迁移解析 V2 manifest 失败", &paths.manifest_file, err)
    })?;
    manifest.validate_version(&paths.manifest_file).map_err(|err| {
        migration_v2_parse_failure("迁移校验 V2 manifest 失败", &paths.manifest_file, err)
    })?;
    Ok(manifest)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MigrationV2Status {
    pub(super) ready: bool,
    pub(super) meta_present: bool,
    pub(super) message_count: usize,
    pub(super) title: String,
    pub(super) migration_state: String,
    pub(super) message_store_kind: String,
}

pub(super) fn migration_read_v2_status(
    paths: &MessageStorePaths,
) -> Result<Option<MigrationV2Status>, MigrationV2ToV3Failure> {
    if !migration_v2_path_exists(&paths.manifest_file)? {
        return Ok(None);
    }
    let manifest = migration_read_v2_manifest(paths)?;
    let (meta_present, title) = if migration_v2_path_exists(&paths.meta_file)? {
        (
            true,
            migration_read_v2_meta(paths)?
                .title()
                .to_string(),
        )
    } else {
        (false, String::new())
    };
    Ok(Some(MigrationV2Status {
        ready: manifest.should_read_jsonl(),
        meta_present,
        message_count: manifest.source_message_count(),
        title,
        migration_state: manifest.migration_state_label().to_string(),
        message_store_kind: manifest.store_kind_label().to_string(),
    }))
}

fn migration_read_v2_meta(
    paths: &MessageStorePaths,
) -> Result<ConversationShardMeta, MigrationV2ToV3Failure> {
    let raw = fs::read_to_string(&paths.meta_file).map_err(|err| {
        migration_v2_io_failure("迁移读取 V2 meta 失败", &paths.meta_file, err)
    })?;
    let meta = serde_json::from_str::<ConversationShardMeta>(&raw).map_err(|err| {
        migration_v2_parse_failure("迁移解析 V2 meta 失败", &paths.meta_file, err)
    })?;
    validate_conversation_shard_meta_id(paths, &meta).map_err(|err| {
        migration_v2_parse_failure("迁移校验 V2 meta 失败", &paths.meta_file, err)
    })?;
    Ok(meta)
}

fn migration_read_v2_index(
    paths: &MessageStorePaths,
) -> Result<MessageStoreIndexFile, MigrationV2ToV3Failure> {
    let raw = fs::read_to_string(&paths.index_file).map_err(|err| {
        migration_v2_io_failure("迁移读取 V2 index 失败", &paths.index_file, err)
    })?;
    let index = serde_json::from_str::<MessageStoreIndexFile>(&raw).map_err(|err| {
        migration_v2_parse_failure("迁移解析 V2 index 失败", &paths.index_file, err)
    })?;
    validate_message_store_index_file(&paths.index_file, &index).map_err(|err| {
        migration_v2_parse_failure("迁移校验 V2 index 失败", &paths.index_file, err)
    })?;
    Ok(index.with_position_lookup())
}

fn migration_read_v2_active_plans(
    paths: &MessageStorePaths,
) -> Result<Vec<ActivePlanRecord>, MigrationV2ToV3Failure> {
    let raw = match fs::read_to_string(&paths.active_plans_file) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(migration_v2_io_failure(
                "迁移读取 V2 active_plans 失败",
                &paths.active_plans_file,
                error,
            ));
        }
    };
    let mut records = Vec::new();
    for (line_number, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<ActivePlanRecord>(line).map_err(|err| {
            migration_v2_parse_failure(
                &format!("迁移解析 V2 active_plan 失败，line={}", line_number + 1),
                &paths.active_plans_file,
                err,
            )
        })?;
        if record.path.trim().is_empty() {
            continue;
        }
        records.push(record);
    }
    Ok(records)
}

fn migration_rebuild_v2_snapshot(
    paths: &MessageStorePaths,
    index: &MessageStoreIndexFile,
) -> Result<RebuiltReadyMessageStoreSnapshot, MigrationV2ToV3Failure> {
    let mut block_ids = index
        .items
        .iter()
        .filter_map(|item| item.block_id)
        .collect::<Vec<_>>();
    block_ids.sort_unstable();
    block_ids.dedup();
    let mut items = Vec::with_capacity(index.items.len());
    let mut total_bytes = 0_u64;
    let mut last_message_id = String::new();
    for block_id in block_ids {
        // V2→V3 迁移读 V2 明文块（.jsonl），不随生产切 .jsonl.zstd
        let block_path = paths
            .shard_dir
            .join(MESSAGE_STORE_BLOCKS_DIR_NAME)
            .join(format!("{block_id:06}.jsonl"));
        let raw = fs::read_to_string(&block_path).map_err(|err| {
            migration_v2_io_failure("迁移读取 V2 block 失败", &block_path, err)
        })?;
        let report = verify_jsonl_snapshot_content(&raw, usize::MAX, "").map_err(|err| {
            migration_v2_parse_failure(
                &format!("迁移校验 V2 block 失败，block_id={block_id}"),
                &block_path,
                err,
            )
        })?;
        if report.message_count == 0 {
            return Err(MigrationV2ToV3Failure::ConversationSkipped(format!(
                "迁移校验 V2 block 失败，conversation_id={}，block_id={}，error=空 block",
                paths.conversation_id, block_id
            )));
        }
        let block_len = fs::metadata(&block_path).map_err(|err| {
            migration_v2_io_failure("迁移读取 V2 block 长度失败", &block_path, err)
        })?.len();
        total_bytes = total_bytes.checked_add(block_len).ok_or_else(|| {
            MigrationV2ToV3Failure::ConversationSkipped(format!(
                "迁移统计 V2 block 字节数溢出，conversation_id={}",
                paths.conversation_id
            ))
        })?;
        last_message_id = report.last_message_id;
        items.extend(report.index.items.into_iter().map(|mut item| {
            item.block_id = Some(block_id);
            item
        }));
    }
    Ok(RebuiltReadyMessageStoreSnapshot {
        message_count: items.len(),
        last_message_id,
        total_bytes,
        index: MessageStoreIndexFile::new(MESSAGE_STORE_MANIFEST_VERSION, items),
    })
}

struct MigrationV2ConversationSource {
    meta: ConversationShardMeta,
    index: MessageStoreIndexFile,
    rebuilt: RebuiltReadyMessageStoreSnapshot,
    plans: Vec<ActivePlanRecord>,
}

fn migration_load_v2_conversation_source(
    paths: &MessageStorePaths,
) -> Result<MigrationV2ConversationSource, MigrationV2ToV3Failure> {
    let manifest = migration_read_v2_manifest(paths)?;
    let ready = manifest.should_read_jsonl();
    let recoverable_building = manifest.store_kind_label() == "jsonlSnapshot"
        && manifest.migration_state_label() == "building";
    if !ready && !recoverable_building {
        return Err(MigrationV2ToV3Failure::ConversationSkipped(format!(
            "V2 会话不可迁移，conversation_id={}，kind={}，state={}",
            paths.conversation_id,
            manifest.store_kind_label(),
            manifest.migration_state_label()
        )));
    }
    let meta = migration_read_v2_meta(paths)?;
    let index = migration_read_v2_index(paths)?;
    let rebuilt = migration_rebuild_v2_snapshot(paths, &index)?;
    if index.persistent_view().items != rebuilt.index.persistent_view().items
        || manifest.source_message_count() != rebuilt.message_count
        || manifest.last_message_id().trim() != rebuilt.last_message_id
        || (ready && manifest.messages_jsonl_bytes() != rebuilt.total_bytes)
    {
        return Err(MigrationV2ToV3Failure::ConversationSkipped(format!(
            "V2 locator 与 block 不一致，conversation_id={}；保留原始文件供人工处理或显式重试",
            paths.conversation_id
        )));
    }
    let plans = migration_read_v2_active_plans(paths)?;
    Ok(MigrationV2ConversationSource {
        meta,
        index,
        rebuilt,
        plans,
    })
}

pub(super) fn migration_validate_v2_conversation(
    paths: &MessageStorePaths,
) -> Result<(), MigrationV2ToV3Failure> {
    migration_load_v2_conversation_source(paths).map(|_| ())
}

fn migration_v2_to_v3_conversation(
    paths: &MessageStorePaths,
) -> Result<(), MigrationV2ToV3Failure> {
    let MigrationV2ConversationSource {
        meta,
        index,
        rebuilt,
        plans,
    } = migration_load_v2_conversation_source(paths)?;
    let conn = migration_open_v3_database(&paths.data_path)
        .map_err(MigrationV2ToV3Failure::SystemFailure)?;
    let transaction = conn
        .unchecked_transaction()
        .map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("开启 V3 迁移事务失败: {err}")))?;
    let meta_json = serde_json::to_string(&meta)
        .map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("序列化 V2 metadata 失败: {err}")))?;
    transaction.execute(
        "INSERT INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at)
         VALUES(?1, ?2, 1, ?3)
         ON CONFLICT(conversation_id) DO UPDATE SET metadata_json=excluded.metadata_json, storage_revision=excluded.storage_revision, updated_at=excluded.updated_at",
        rusqlite::params![paths.conversation_id, meta_json, meta.updated_at()],
    ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("导入 V3 metadata 失败: {err}")))?;
    transaction.execute(
        "DELETE FROM conversation_blocks WHERE conversation_id=?1",
        [&paths.conversation_id],
    ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("清理 V3 block 失败: {err}")))?;
    transaction.execute(
        "DELETE FROM message_locator WHERE conversation_id=?1",
        [&paths.conversation_id],
    ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("清理 V3 locator 失败: {err}")))?;
    for block_id in index.items.iter().filter_map(|item| item.block_id).collect::<std::collections::BTreeSet<_>>() {
        // V2→V3 迁移读 V2 明文块（.jsonl），不随生产切 .jsonl.zstd
        let block_path = paths
            .shard_dir
            .join(MESSAGE_STORE_BLOCKS_DIR_NAME)
            .join(format!("{block_id:06}.jsonl"));
        let byte_len = fs::metadata(&block_path).map_err(|err| {
            migration_v2_io_failure("读取 V2 block 长度失败", &block_path, err)
        })?.len();
        let count = index.items.iter().filter(|item| item.block_id == Some(block_id)).count();
        transaction.execute(
            "INSERT INTO conversation_blocks(conversation_id, block_id, block_file, byte_len, message_count) VALUES(?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![paths.conversation_id, block_id as i64, format!("blocks/{block_id:06}.jsonl"), byte_len as i64, count as i64],
        ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("导入 V3 block 失败: {err}")))?;
    }
    for (sequence, item) in rebuilt.index.items.iter().enumerate() {
        let block_id = item.block_id.ok_or_else(|| MigrationV2ToV3Failure::ConversationSkipped(
            format!("V2 locator 缺少 block_id，conversation_id={}，message_id={}", paths.conversation_id, item.message_id)
        ))?;
        transaction.execute(
            "INSERT INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![paths.conversation_id, sequence as i64, item.message_id, block_id as i64, item.offset as i64, item.byte_len as i64, item.compaction_kind, item.role, item.created_at],
        ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("导入 V3 locator 失败: {err}")))?;
    }
    transaction.execute(
        "DELETE FROM active_plan_records WHERE conversation_id=?1",
        [&paths.conversation_id],
    ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("清理 V3 活动计划失败: {err}")))?;
    for record in plans {
        let raw = serde_json::to_string(&record)
            .map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("序列化 V2 活动计划失败: {err}")))?;
        transaction.execute(
            "INSERT INTO active_plan_records(conversation_id, plan_id, record_json) VALUES(?1, ?2, ?3)",
            rusqlite::params![paths.conversation_id, record.plan_id, raw],
        ).map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("导入 V3 活动计划失败: {err}")))?;
    }
    transaction
        .commit()
        .map_err(|err| MigrationV2ToV3Failure::SystemFailure(format!("提交 V3 迁移事务失败: {err}")))
}

fn migration_collect_v2_conversation_paths(
    data_path: &PathBuf,
) -> Result<Vec<MessageStorePaths>, String> {
    let conversation_dir = app_layout_chat_conversations_dir(data_path);
    let mut paths = Vec::new();
    match fs::metadata(&conversation_dir) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(format!(
                "V2 会话根路径不是目录，path={}",
                conversation_dir.display()
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(paths),
        Err(err) => {
            return Err(format!(
                "检查 V2 会话根目录失败，path={}，error={err}",
                conversation_dir.display()
            ));
        }
    }
    for entry in fs::read_dir(&conversation_dir)
        .map_err(|err| format!("枚举 V2 会话失败: {err}"))?
    {
        let entry = entry.map_err(|err| format!("读取 V2 会话目录项失败: {err}"))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            format!("读取 V2 会话目录项类型失败，path={}，error={err}", path.display())
        })?;
        if path.extension().and_then(|value| value.to_str()) == Some("json")
            || !file_type.is_dir()
        {
            continue;
        }
        let Some(id) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        paths.push(message_store_paths(data_path, id)?);
    }
    Ok(paths)
}

/// 独立的 V2→V3 批量迁移入口。
/// 单会话转换错误只记录并跳过；只有 SQLite/根目录等系统级错误由底层返回 Err。
/// progress 为可选逐会话进度回调：参数依次为（当前序号（从 1 起）、总数、会话 ID、会话标题、阶段名），
/// 在准备处理每个会话前调用一次。
pub(super) fn migration_v2_to_v3(
    data_path: &PathBuf,
    progress: Option<&dyn Fn(usize, usize, &str, &str, &str)>,
) -> Result<(), String> {
    let paths = migration_collect_v2_conversation_paths(data_path)?;
    let total = paths.len();
    let mut skipped_count = 0usize;
    for (index, paths) in paths.iter().enumerate() {
        if let Some(callback) = progress {
            let title = migration_read_v2_meta(paths)
                .map(|meta| meta.title().to_string())
                .unwrap_or_default();
            callback(index + 1, total, &paths.conversation_id, &title, "v2_to_v3");
        }
        let migration_key = format!(
            "{}:conversation:{}",
            MIGRATION_V3_COMPLETED_KEY, paths.conversation_id
        );
        if migration_v3_contains_conversation(data_path, &paths.conversation_id)? {
            if !migration_v3_is_completed(data_path, &migration_key)? {
                migration_v3_mark_completed(data_path, &migration_key)?;
            }
            continue;
        }
        match migration_v2_to_v3_conversation(paths) {
            Ok(()) => migration_v3_mark_completed(data_path, &migration_key)?,
            Err(MigrationV2ToV3Failure::ConversationSkipped(err)) => {
                skipped_count += 1;
                runtime_log_warn(format!(
                    "[聊天存储迁移] 跳过，任务=V2到V3会话迁移，conversation_id={}，异常={}",
                    paths.conversation_id, err
                ));
            }
            Err(MigrationV2ToV3Failure::SystemFailure(err)) => return Err(err),
        }
    }
    if skipped_count > 0 {
        runtime_log_warn(format!(
            "[聊天存储迁移] 完成，任务=V2到V3逐会话迁移，跳过会话数={}，source=保留原始文件供人工处理或显式重试",
            skipped_count
        ));
    }
    migration_v3_mark_completed(data_path, MIGRATION_V3_COMPLETED_KEY)
}

#[cfg(test)]
mod message_store_tests {
    use super::*;

    fn test_message(id: &str, role: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: format!("2026-04-24T00:00:0{}Z", id.len()),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: format!("message {id}"),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        }
    }

    fn test_compaction_message(id: &str, kind: &str) -> ChatMessage {
        let mut message = test_message(id, "assistant");
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": kind
            }
        }));
        message
    }

    fn test_conversation(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-a".to_string(),
            title: "会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages,
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    #[test]
    fn message_store_jsonl_verification_should_detect_compaction_kinds() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
            test_compaction_message("c2", "summary_context_seed"),
        ]);

        let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&conversation)
            .expect("build artifacts");

        assert_eq!(blocks.message_count, 4);
        assert_eq!(blocks.last_message_id, "c2");
        assert_eq!(blocks.index.items.len(), 4);
        assert_eq!(
            blocks
                .index
                .items
                .iter()
                .filter(|item| item.compaction_kind.is_some())
                .count(),
            2
        );
        assert!(blocks.blocks.iter().all(|block| block.content.ends_with('\n')));
    }

    #[test]
    fn message_store_jsonl_verification_should_reject_stale_last_message() {
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let content = encode_jsonl_snapshot_messages(&conversation.messages).expect("encode");
        let err = verify_jsonl_snapshot_content(&content, 1, "m2")
            .expect_err("stale last message should fail");

        assert!(err.contains("最后一条消息不一致"));
    }

    #[test]
    fn message_store_manifest_should_not_read_stale_jsonl_without_ready_state() {
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);

        assert!(!manifest.should_read_jsonl());
        assert!(!manifest.is_ready_directory_store());
        assert!(manifest
            .stale_jsonl_reason()
            .expect("stale reason")
            .contains("未处于 ready JSONL 状态"));
    }

    #[test]
    fn message_store_paths_should_extend_existing_chat_conversation_layout() {
        let data_path = PathBuf::from("E:/app/data/config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");

        assert!(paths
            .legacy_conversation_file
            .to_string_lossy()
            .ends_with("chat\\conversations\\conversation-a.json")
            || paths
                .legacy_conversation_file
                .to_string_lossy()
                .ends_with("chat/conversations/conversation-a.json"));
        assert!(paths
            .messages_file
            .to_string_lossy()
            .contains("chat"));
        assert!(paths
            .messages_file
            .to_string_lossy()
            .ends_with("conversation-a\\messages.jsonl")
            || paths
                .messages_file
                .to_string_lossy()
                .ends_with("conversation-a/messages.jsonl"));
    }

    #[test]
    fn message_store_manifest_should_round_trip_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-manifest-{}",
            Uuid::new_v4()
        ));
        let manifest_path = root.join("chat").join("conversations").join("conversation-a").join("manifest.json");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation)
            .jsonl_snapshot_ready(128, 2);

        write_message_store_manifest_atomic(&manifest_path, &manifest).expect("write manifest");
        let loaded = read_message_store_manifest(&manifest_path)
            .expect("read manifest")
            .expect("manifest exists");

        assert_eq!(loaded, manifest);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_manifest_should_reject_unsupported_version() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-manifest-version-{}",
            Uuid::new_v4()
        ));
        let manifest_path = root
            .join("chat")
            .join("conversations")
            .join("conversation-a")
            .join("manifest.json");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        manifest.version = MESSAGE_STORE_MANIFEST_VERSION + 1;

        write_message_store_manifest_atomic(&manifest_path, &manifest).expect("write manifest");
        let err = read_message_store_manifest(&manifest_path)
            .expect_err("unsupported manifest version should fail");

        assert!(err.contains("manifest 版本不支持"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_run_migration_should_write_manifest_jsonl_and_index() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-run-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "summary_context_seed"),
        ]);

        migration_v1_to_v2_conversation(&paths, &conversation, false)
            .expect("run migration");
        let manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");

        assert!(manifest.should_read_jsonl());
        assert!(paths.meta_file.exists());
        assert!(!paths.messages_file.exists());
        assert!(paths.blocks_dir.exists());
        assert!(paths.index_file.exists());
        let meta = read_conversation_shard_meta(&paths.meta_file).expect("read meta");
        assert_eq!(meta.id, conversation.id);
        let block_zero = paths.blocks_dir.join("000000.jsonl");
        let block_one = paths.blocks_dir.join("000001.jsonl");
        let report_zero = verify_jsonl_snapshot_file(&block_zero, 1, "m1").expect("verify block 0");
        let report_one = verify_jsonl_snapshot_file(&block_one, 1, "c1").expect("verify block 1");
        assert_eq!(report_zero.compaction_count, 0);
        assert_eq!(report_one.compaction_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v1_to_v2_should_be_idempotent() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v1-v2-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
        ]);

        migration_v1_to_v2_conversation(&paths, &conversation, false)
            .expect("first v1 to v2 migration");
        let first_manifest = fs::read_to_string(&paths.manifest_file).expect("first manifest");
        let first_meta = fs::read_to_string(&paths.meta_file).expect("first meta");
        let first_index = fs::read_to_string(&paths.index_file).expect("first index");
        let first_block = fs::read_to_string(paths.blocks_dir.join("000000.jsonl"))
            .expect("first block");

        migration_v1_to_v2_conversation(&paths, &conversation, false)
            .expect("second v1 to v2 migration");

        assert_eq!(fs::read_to_string(&paths.manifest_file).expect("second manifest"), first_manifest);
        assert_eq!(fs::read_to_string(&paths.meta_file).expect("second meta"), first_meta);
        assert_eq!(fs::read_to_string(&paths.index_file).expect("second index"), first_index);
        assert_eq!(
            fs::read_to_string(paths.blocks_dir.join("000000.jsonl")).expect("second block"),
            first_block
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v1_to_v2_should_preserve_unreferenced_legacy_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v1-v2-preserve-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        if let Some(parent) = paths.legacy_conversation_file.parent() {
            fs::create_dir_all(parent).expect("create conversation dir");
        }
        let legacy_v1 = serde_json::to_vec(&conversation).expect("serialize v1 source");
        fs::write(&paths.legacy_conversation_file, &legacy_v1).expect("write v1 source");
        fs::create_dir_all(&paths.blocks_dir).expect("create v2 blocks dir");
        let legacy_single_file = b"legacy-v2-single-file\n";
        fs::write(&paths.messages_file, legacy_single_file).expect("write legacy v2 single file");
        let unreferenced_block = paths.blocks_dir.join("999999.jsonl");
        let unreferenced_content = b"legacy-v2-unreferenced-block\n";
        fs::write(&unreferenced_block, unreferenced_content)
            .expect("write unreferenced v2 block");

        migration_v1_to_v2_conversation(&paths, &conversation, false)
            .expect("run v1 to v2 migration");

        assert_eq!(
            fs::read(&paths.legacy_conversation_file).expect("read v1 source after migration"),
            legacy_v1
        );
        assert_eq!(
            fs::read(&paths.messages_file).expect("read legacy v2 single file after migration"),
            legacy_single_file
        );
        assert_eq!(
            fs::read(&unreferenced_block).expect("read unreferenced v2 block after migration"),
            unreferenced_content
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v2_to_v3_should_preserve_sources_and_be_idempotent() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v2-v3-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
        ]);
        migration_v1_to_v2_conversation(&paths, &conversation, false)
            .expect("seed v2 fixture");
        fs::write(
            &paths.active_plans_file,
            concat!(
                r#"{"planId":"plan-a","sourceMessageId":"m1","status":"in_progress","path":"docs/plan.md","createdAt":"2026-08-21T00:00:00Z"}"#,
                "\n"
            ),
        )
        .expect("seed v2 active plan fixture");
        let source_files = [
            paths.manifest_file.clone(),
            paths.meta_file.clone(),
            paths.index_file.clone(),
            paths.active_plans_file.clone(),
            paths.blocks_dir.join("000000.jsonl"),
        ];
        let source_snapshots = source_files
            .iter()
            .map(|path| {
                let content = fs::read(path).expect("read v2 source before migration");
                let modified = fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .expect("read v2 source mtime before migration");
                (path.clone(), content, modified)
            })
            .collect::<Vec<_>>();

        migration_v2_to_v3(&data_path, None).expect("first v2 to v3 migration");
        migration_v2_to_v3(&data_path, None).expect("second v2 to v3 migration");

        for (path, content_before, modified_before) in source_snapshots {
            assert_eq!(
                fs::read(&path).expect("read v2 source after migration"),
                content_before,
                "V2→V3 不得改写源文件内容，path={}",
                path.display()
            );
            assert_eq!(
                fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .expect("read v2 source mtime after migration"),
                modified_before,
                "V2→V3 不得更新源文件 mtime，path={}",
                path.display()
            );
        }
        let stored = chat_metadata_store_read_conversation(&paths)
            .expect("read v3 conversation")
            .expect("v3 conversation exists");
        assert_eq!(stored.id, conversation.id);
        assert_eq!(
            stored.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            conversation
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(migration_v3_is_completed(&data_path, MIGRATION_V3_COMPLETED_KEY)
            .expect("global migration key"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v2_to_v3_should_not_overwrite_existing_v3_conversation() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v2-v3-existing-current-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let mut legacy = test_conversation(vec![test_message("m1", "user")]);
        legacy.title = "V2 旧标题".to_string();
        migration_v1_to_v2_conversation(&paths, &legacy, false).expect("seed v2");
        let mut current = legacy.clone();
        current.title = "V3 当前标题".to_string();
        chat_store_write_snapshot(&paths, &current).expect("seed existing V3 current");

        migration_v2_to_v3(&data_path, None).expect("run migration with existing V3 current");

        let stored = chat_metadata_store_read_conversation(&paths)
            .expect("read current conversation")
            .expect("current conversation exists");
        assert_eq!(stored.title, "V3 当前标题");
        let migration_key = format!(
            "{}:conversation:{}",
            MIGRATION_V3_COMPLETED_KEY, current.id
        );
        assert!(migration_v3_is_completed(&data_path, &migration_key)
            .expect("per-conversation migration key"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v2_to_v3_should_skip_bad_conversation_and_allow_explicit_retry() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v2-v3-skip-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let good_paths = message_store_paths(&data_path, "conversation-a").expect("good paths");
        let good = test_conversation(vec![test_message("m1", "user")]);
        migration_v1_to_v2_conversation(&good_paths, &good, false).expect("seed good v2");

        let mut bad = test_conversation(vec![test_message("bad-m1", "user")]);
        bad.id = "conversation-bad".to_string();
        let bad_paths = message_store_paths(&data_path, &bad.id).expect("bad paths");
        migration_v1_to_v2_conversation(&bad_paths, &bad, false).expect("seed bad v2");
        fs::write(&bad_paths.index_file, "{broken index").expect("corrupt bad index");

        migration_v2_to_v3(&data_path, None).expect("migrate with bad conversation");

        assert!(chat_metadata_store_contains_conversation(&data_path, &good.id)
            .expect("good contains"));
        assert!(!chat_metadata_store_contains_conversation(&data_path, &bad.id)
            .expect("bad contains"));
        assert!(migration_v3_is_completed(&data_path, MIGRATION_V3_COMPLETED_KEY)
            .expect("global migration key"));
        assert!(bad_paths.index_file.exists());

        migration_v1_to_v2_conversation(&bad_paths, &bad, false)
            .expect("repair bad V2 source");
        migration_v2_to_v3(&data_path, None).expect("explicitly retry repaired V2 source");
        assert!(chat_metadata_store_contains_conversation(&data_path, &bad.id)
            .expect("repaired conversation contains"));
        assert!(migration_v3_is_completed(&data_path, MIGRATION_V3_COMPLETED_KEY)
            .expect("global migration key completes after retry"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v2_to_v3_should_import_complete_building_source() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v2-v3-building-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        migration_v1_to_v2_conversation(&paths, &conversation, false).expect("seed v2");
        let building = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        write_message_store_manifest_atomic(&paths.manifest_file, &building)
            .expect("downgrade manifest to building");

        migration_v2_to_v3(&data_path, None).expect("migrate complete building source");

        assert!(chat_metadata_store_contains_conversation(&data_path, &conversation.id)
            .expect("V3 contains migrated building source"));
        let source_manifest = migration_read_v2_manifest(&paths).expect("read source manifest");
        assert_eq!(source_manifest.migration_state_label(), "building");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn independent_v2_to_v3_should_stop_on_system_io_failure() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-independent-v2-v3-system-failure-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        migration_v1_to_v2_conversation(&paths, &conversation, false).expect("seed v2");
        fs::remove_file(&paths.index_file).expect("remove index fixture");
        fs::create_dir(&paths.index_file).expect("replace index with unreadable directory");

        let err = migration_v2_to_v3(&data_path, None)
            .expect_err("system-level V2 read failure should stop migration");

        assert!(err.contains("迁移读取 V2 index 失败"));
        assert!(!migration_v3_is_completed(&data_path, MIGRATION_V3_COMPLETED_KEY)
            .expect("global migration key should remain incomplete"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_dry_run_should_not_write_files() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-dry-run-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);

        migration_v1_to_v2_conversation(&paths, &conversation, true)
            .expect("dry run migration");

        assert!(!paths.manifest_file.exists());
        assert!(!paths.messages_file.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_conversation_json_should_match_before_after_limit_semantics() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_message("m4", "assistant"),
        ]);
        let before = read_messages_before_from_slice(&conversation.messages, "m4", 2)
            .expect("before page");
        let after = read_messages_after_from_slice(&conversation.messages, "m1", 2)
            .expect("after page");

        assert_eq!(before.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["m2", "m3"]);
        assert!(before.has_more);
        assert_eq!(after.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["m2", "m3"]);
        assert!(after.has_more);
    }

    #[test]
    fn message_store_compaction_segment_should_use_compaction_boundaries() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m3", "user"),
            test_compaction_message("c2", "summary_context_seed"),
            test_message("m4", "assistant"),
        ]);
        let current = read_current_compaction_segment_from_slice(&conversation.messages)
            .expect("current segment");
        let previous = read_compaction_segment_before_from_slice(
            &conversation.messages,
            current.boundary_message_id.as_deref().expect("boundary"),
        )
            .expect("previous segment");

        assert_eq!(
            current.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c2", "m4"]
        );
        assert_eq!(current.boundary_message_id.as_deref(), Some("c2"));
        assert_eq!(current.previous_boundary_message_id.as_deref(), Some("c1"));
        assert!(current.has_previous_segment);
        assert_eq!(
            previous.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c1", "m3"]
        );
        assert_eq!(previous.boundary_message_id.as_deref(), Some("c1"));
        assert!(previous.has_previous_segment);
    }

    #[test]
    fn message_store_compaction_segment_without_boundary_should_return_whole_conversation() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
        ]);
        let current = read_current_compaction_segment_from_slice(&conversation.messages)
            .expect("current segment");

        assert_eq!(
            current.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
        assert_eq!(current.boundary_message_id, None);
        assert!(!current.has_previous_segment);
    }

    #[test]
    fn message_store_jsonl_verification_should_read_fixture_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-jsonl-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create temp dir");
        let messages_path = root.join("messages.jsonl");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
        ]);
        let content = encode_jsonl_snapshot_messages(&conversation.messages).expect("encode");
        fs::write(&messages_path, content).expect("write fixture");

        let report = verify_jsonl_snapshot_file(&messages_path, 3, "m2").expect("verify fixture");
        let rebuilt = rebuild_jsonl_snapshot_index_from_file(&messages_path).expect("rebuild index");

        assert_eq!(report.compaction_count, 1);
        assert_eq!(rebuilt.items.len(), 3);
        assert_eq!(rebuilt.items[1].compaction_kind.as_deref(), Some("context_compaction"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_verification_should_reject_half_line_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-half-line-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create temp dir");
        let messages_path = root.join("messages.jsonl");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut content = encode_jsonl_snapshot_messages(&conversation.messages).expect("encode");
        content.push_str("{\"kind\":\"message\"");
        fs::write(&messages_path, content).expect("write fixture");

        let err = verify_jsonl_snapshot_file(&messages_path, 2, "")
            .expect_err("half line should fail");

        assert!(err.contains("offset=") || err.contains("半行"));
        let _ = fs::remove_dir_all(root);
    }
}