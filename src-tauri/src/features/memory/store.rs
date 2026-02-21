use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::collections::{HashMap as StdHashMap, HashSet as StdHashSet};

const MEMORY_DB_FILE_NAME: &str = "memory_store.db";
const LEGACY_APP_DATA_MEMORIES_MIGRATION_KEY: &str = "legacy_app_data_memories_migrated_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreImportStats {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreProviderSyncReport {
    status: String,
    old_provider_id: Option<String>,
    new_provider_id: String,
    deleted: usize,
    added: usize,
    batch_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreRebuildReport {
    memory_rows: usize,
    memory_fts_rows: usize,
    note_rows: usize,
    note_fts_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreHealthReport {
    status: String,
    memory_rows: usize,
    memory_fts_rows: usize,
    note_rows: usize,
    note_fts_rows: usize,
    orphan_memory_tag_rows: usize,
    orphan_note_tag_rows: usize,
    repaired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreBackupResult {
    path: String,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct MemoryDraftInput {
    memory_type: String,
    judgment: String,
    reasoning: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyMemoryJsonItem {
    #[serde(default, alias = "memoryType")]
    memory_type: String,
    #[serde(default, alias = "content")]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default, alias = "keywords")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyMemoryJsonPayload {
    #[serde(default)]
    memories: Vec<LegacyMemoryJsonItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryLegacyMigrationReport {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
    source_path: String,
    archived_path: String,
}

fn memory_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("memory")
        .join(MEMORY_DB_FILE_NAME)
}

fn memory_store_legacy_app_data_migrated_path(data_path: &PathBuf) -> PathBuf {
    let parent = data_path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join("app_data.memories.migrated.json")
}

fn memory_store_migrate_legacy_app_data_memories(
    data_path: &PathBuf,
) -> Result<Option<MemoryLegacyMigrationReport>, String> {
    let conn = memory_store_open(data_path)?;
    let migrated = memory_store_get_runtime_state(&conn, LEGACY_APP_DATA_MEMORIES_MIGRATION_KEY)?;
    if migrated.as_deref() == Some("done") {
        return Ok(None);
    }

    if !data_path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(data_path)
        .map_err(|err| format!("Read app_data.json failed ({}): {err}", data_path.display()))?;
    let payload: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|err| format!("Parse app_data.json failed ({}): {err}", data_path.display()))?;
    let Some(memories_value) = payload.get("memories").cloned() else {
        return Ok(None);
    };
    let incoming = if memories_value.is_array() {
        serde_json::from_value::<Vec<LegacyMemoryJsonItem>>(memories_value)
            .map_err(|err| format!("Parse app_data.memories array failed: {err}"))?
    } else {
        serde_json::from_value::<LegacyMemoryJsonPayload>(memories_value)
            .map(|p| p.memories)
            .map_err(|err| format!("Parse app_data.memories payload failed: {err}"))?
    };

    let mut drafts = Vec::<MemoryDraftInput>::new();
    for item in &incoming {
        let judgment = clean_text(item.judgment.trim());
        if judgment.is_empty() {
            continue;
        }
        let tags = normalize_memory_keywords(&item.tags);
        if tags.is_empty() {
            continue;
        }
        let memory_type = memory_store_normalize_memory_type(&item.memory_type)?;
        let reasoning = clean_text(item.reasoning.trim());
        drafts.push(MemoryDraftInput {
            memory_type,
            judgment,
            reasoning,
            tags,
        });
    }

    let before = memory_store_count(data_path)?;
    let (results, total_count) = memory_store_upsert_drafts(data_path, &drafts)?;
    let imported_count = drafts.len();
    let created_count = total_count.saturating_sub(before);
    let merged_count = results.iter().filter(|r| r.saved).count().saturating_sub(created_count);

    let mut archived_path = memory_store_legacy_app_data_migrated_path(data_path);
    if archived_path.exists() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        archived_path = archived_path.with_file_name(format!("app_data.memories.migrated.{now}.json"));
    }
    let archived_body = serde_json::to_string_pretty(&incoming)
        .map_err(|err| format!("Serialize migrated memories snapshot failed: {err}"))?;
    fs::write(&archived_path, archived_body).map_err(|err| {
        format!(
            "Write migrated memories snapshot failed ({}): {err}",
            archived_path.display()
        )
    })?;

    memory_store_set_runtime_state(&conn, LEGACY_APP_DATA_MEMORIES_MIGRATION_KEY, "done")?;
    Ok(Some(MemoryLegacyMigrationReport {
        imported_count,
        created_count,
        merged_count,
        total_count,
        source_path: data_path.to_string_lossy().to_string(),
        archived_path: archived_path.to_string_lossy().to_string(),
    }))
}

fn memory_store_open(data_path: &PathBuf) -> Result<Connection, String> {
    let db_path = memory_store_db_path(data_path);
    if !db_path.exists() {
        let legacy_parent = data_path
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| PathBuf::from("."));
        let legacy_db = legacy_parent.join(MEMORY_DB_FILE_NAME);
        if legacy_db.exists() {
            if let Some(new_parent) = db_path.parent() {
                fs::create_dir_all(new_parent).map_err(|err| {
                    format!("Create memory db dir failed ({}): {err}", new_parent.display())
                })?;
            }
            fs::rename(&legacy_db, &db_path).or_else(|_| {
                fs::copy(&legacy_db, &db_path)
                    .map(|_| ())
                    .and_then(|_| fs::remove_file(&legacy_db))
            }).map_err(|err| {
                format!(
                    "Migrate legacy memory db failed ({} -> {}): {err}",
                    legacy_db.display(),
                    db_path.display()
                )
            })?;
        }
    }
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create memory db dir failed ({}): {err}", parent.display()))?;
    }
    let conn = Connection::open(&db_path)
        .map_err(|err| format!("Open memory db failed ({}): {err}", db_path.display()))?;
    memory_store_init_schema(&conn)?;
    Ok(conn)
}

fn memory_store_normalize_memory_type(raw: &str) -> Result<String, String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "knowledge" | "skill" | "emotion" | "event" => Ok(normalized),
        "task" => Err("memory_type 'task' is not supported in this build".to_string()),
        "" => Ok("knowledge".to_string()),
        _ => Err(format!(
            "invalid memory_type '{raw}', expected one of: knowledge/skill/emotion/event"
        )),
    }
}

fn memory_store_init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA temp_store=MEMORY;",
    )
    .map_err(|err| format!("Apply memory db pragmas failed: {err}"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_record (
            id TEXT PRIMARY KEY,
            memory_type TEXT NOT NULL DEFAULT 'knowledge',
            judgment TEXT NOT NULL,
            reasoning TEXT NOT NULL DEFAULT '',
            strength INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            memory_scope TEXT NOT NULL DEFAULT 'public',
            useful_count INTEGER NOT NULL DEFAULT 0,
            useful_score REAL NOT NULL DEFAULT 0,
            last_recalled_at TEXT,
            last_decay_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS global_tag (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS memory_tag_rel (
            memory_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            PRIMARY KEY (memory_id, tag_id),
            FOREIGN KEY(memory_id) REFERENCES memory_record(id) ON DELETE CASCADE,
            FOREIGN KEY(tag_id) REFERENCES global_tag(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS note_index_record (
            source_id TEXT PRIMARY KEY,
            note_short_id INTEGER NOT NULL UNIQUE,
            file_id TEXT NOT NULL,
            source_file_path TEXT NOT NULL,
            heading_h1 TEXT,
            heading_h2 TEXT,
            heading_h3 TEXT,
            heading_h4 TEXT,
            heading_h5 TEXT,
            heading_h6 TEXT,
            total_lines INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS note_tag_rel (
            source_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            PRIMARY KEY (source_id, tag_id),
            FOREIGN KEY(source_id) REFERENCES note_index_record(source_id) ON DELETE CASCADE,
            FOREIGN KEY(tag_id) REFERENCES global_tag(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS embedding_provider (
            provider_id TEXT PRIMARY KEY,
            dimension INTEGER NOT NULL,
            model_name TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kb_runtime_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_memory_updated_at ON memory_record(updated_at);
        CREATE INDEX IF NOT EXISTS idx_memory_scope_active ON memory_record(memory_scope, is_active);
        CREATE INDEX IF NOT EXISTS idx_memory_useful_score ON memory_record(useful_score);
        CREATE INDEX IF NOT EXISTS idx_memory_tag_tag_id ON memory_tag_rel(tag_id);
        CREATE INDEX IF NOT EXISTS idx_note_updated_at ON note_index_record(updated_at);
        CREATE INDEX IF NOT EXISTS idx_note_file_id ON note_index_record(file_id);
        CREATE INDEX IF NOT EXISTS idx_note_tag_tag_id ON note_tag_rel(tag_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
            item_id UNINDEXED,
            judgment
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS note_fts USING fts5(
            item_id UNINDEXED,
            tags
        );",
    )
    .map_err(|err| format!("Initialize memory db schema failed: {err}"))?;

    // Migrate memory_fts: drop the old 2-column (tags+judgment) FTS table and recreate
    // as single-column. The judgment column stores concatenated "judgment + tags" text for BM25.
    let col_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('memory_fts') WHERE name IN ('tags','judgment')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if col_count == 2 {
        conn.execute_batch(
            "DROP TABLE IF EXISTS memory_fts;
             CREATE VIRTUAL TABLE memory_fts USING fts5(item_id UNINDEXED, judgment);",
        )
        .map_err(|err| format!("Migrate memory_fts (drop tags column) failed: {err}"))?;

        // Load all tags into jieba then repopulate FTS inline so data is never empty after migration.
        let tag_names: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT DISTINCT name FROM global_tag")
                .map_err(|err| format!("Migrate: list tags failed: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("Migrate: query tags failed: {err}"))?;
            let mut out = Vec::<String>::new();
            for row in rows {
                out.push(row.map_err(|err| format!("Migrate: read tag failed: {err}"))?);
            }
            out
        };
        memory_jieba_add_words(&tag_names);

        let memory_ids: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT id FROM memory_record")
                .map_err(|err| format!("Migrate: list memory ids failed: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("Migrate: query memory ids failed: {err}"))?;
            let mut out = Vec::<String>::new();
            for row in rows {
                out.push(row.map_err(|err| format!("Migrate: read memory id failed: {err}"))?);
            }
            out
        };
        for memory_id in &memory_ids {
            memory_store_sync_memory_fts(conn, memory_id)?;
        }
    }

    // If memory_fts is empty but memory_record has data, repopulate.
    let fts_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM memory_fts", [], |row| row.get(0))
        .unwrap_or(0);
    let mem_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get(0))
        .unwrap_or(0);
    if fts_count == 0 && mem_count > 0 {
        let tag_names: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT DISTINCT name FROM global_tag")
                .map_err(|err| format!("FTS repopulate: list tags failed: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("FTS repopulate: query tags failed: {err}"))?;
            let mut out = Vec::<String>::new();
            for row in rows {
                out.push(row.map_err(|err| format!("FTS repopulate: read tag failed: {err}"))?);
            }
            out
        };
        memory_jieba_add_words(&tag_names);

        let memory_ids: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT id FROM memory_record")
                .map_err(|err| format!("FTS repopulate: list memory ids failed: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("FTS repopulate: query memory ids failed: {err}"))?;
            let mut out = Vec::<String>::new();
            for row in rows {
                out.push(row.map_err(|err| format!("FTS repopulate: read memory id failed: {err}"))?);
            }
            out
        };
        for memory_id in &memory_ids {
            memory_store_sync_memory_fts(conn, memory_id)?;
        }
    }

    Ok(())
}

fn memory_store_set_runtime_state(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO kb_runtime_state(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .map_err(|err| format!("Set runtime state failed for '{key}': {err}"))?;
    Ok(())
}

fn memory_store_get_runtime_state(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM kb_runtime_state WHERE key=?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|err| format!("Get runtime state failed for '{key}': {err}"))
}

fn memory_store_provider_model_name(
    conn: &Connection,
    provider_id: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT model_name FROM embedding_provider WHERE provider_id=?1",
        params![provider_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|err| format!("Query provider model_name failed: {err}"))
}

fn memory_store_tag_id_by_name(conn: &Connection, tag: &str) -> Result<String, String> {
    let found = conn
        .query_row(
            "SELECT id FROM global_tag WHERE name=?1",
            params![tag],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("Lookup tag failed: {err}"))?;
    if let Some(id) = found {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO global_tag(id, name) VALUES (?1, ?2)",
        params![id, tag],
    )
    .map_err(|err| format!("Insert tag failed: {err}"))?;
    Ok(id)
}

fn memory_store_sync_tags(conn: &Connection, memory_id: &str, tags: &[String]) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_tag_rel WHERE memory_id=?1",
        params![memory_id],
    )
    .map_err(|err| format!("Delete memory_tag_rel failed: {err}"))?;

    for kw in tags {
        let tag_id = memory_store_tag_id_by_name(conn, kw)?;
        conn.execute(
            "INSERT OR IGNORE INTO memory_tag_rel(memory_id, tag_id) VALUES (?1, ?2)",
            params![memory_id, tag_id],
        )
        .map_err(|err| format!("Insert memory_tag_rel failed: {err}"))?;
    }
    Ok(())
}

fn memory_store_sync_memory_fts(conn: &Connection, memory_id: &str) -> Result<(), String> {
    let judgment = conn
        .query_row(
            "SELECT judgment FROM memory_record WHERE id=?1",
            params![memory_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("Load memory judgment failed: {err}"))?
        .unwrap_or_default();

    let mut tag_stmt = conn
        .prepare(
            "SELECT gt.name
             FROM memory_tag_rel rel
             JOIN global_tag gt ON gt.id = rel.tag_id
             WHERE rel.memory_id=?1",
        )
        .map_err(|err| format!("Prepare load memory tags failed: {err}"))?;
    let tag_rows = tag_stmt
        .query_map(params![memory_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("Query memory tags failed: {err}"))?;
    let mut tags = Vec::<String>::new();
    for row in tag_rows {
        tags.push(row.map_err(|err| format!("Read memory tag failed: {err}"))?);
    }
    let tags_text = tags.join(" ");
    let raw_fts_text = format!("{} {}", judgment.trim(), tags_text.trim())
        .trim()
        .to_string();
    // Keep FTS indexing tokenization aligned with query tokenization (jieba).
    let fts_doc = memory_tokenize_terms(&raw_fts_text, false).join(" ");

    conn.execute("DELETE FROM memory_fts WHERE item_id=?1", params![memory_id])
        .map_err(|err| format!("Delete memory_fts row failed: {err}"))?;
    conn.execute(
        "INSERT INTO memory_fts(item_id, judgment) VALUES (?1, ?2)",
        params![memory_id, fts_doc],
    )
    .map_err(|err| format!("Insert memory_fts row failed: {err}"))?;

    Ok(())
}

fn memory_store_ensure_jieba_tags(data_path: &PathBuf) {
    static LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if LOADED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if let Ok(conn) = memory_store_open(data_path) {
        if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT name FROM global_tag") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                let tags: Vec<String> = rows.filter_map(|r| r.ok()).collect();
                memory_jieba_add_words(&tags);
            }
        }
    }
    LOADED.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn memory_store_search_fts_bm25(
    data_path: &PathBuf,
    query_text: &str,
    limit: usize,
) -> Result<Vec<(String, f64)>, String> {
    if query_text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let terms = memory_tokenize_query_terms(query_text);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let fts_query = terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ");
    if fts_query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let conn = memory_store_open(data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT item_id, bm25(memory_fts, 1.0) AS score
             FROM memory_fts
             WHERE memory_fts MATCH ?1
             ORDER BY score ASC
             LIMIT ?2",
        )
        .map_err(|err| format!("Prepare memory_fts bm25 query failed: {err}"))?;

    let rows = stmt
        .query_map(params![fts_query, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(|err| format!("Query memory_fts bm25 failed: {err}"))?;

    let mut out = Vec::<(String, f64)>::new();
    for row in rows {
        out.push(row.map_err(|err| format!("Read memory_fts bm25 row failed: {err}"))?);
    }
    Ok(out)
}

fn memory_store_upsert_drafts(
    data_path: &PathBuf,
    drafts: &[MemoryDraftInput],
) -> Result<(Vec<MemorySaveUpsertItemResult>, usize), String> {
    if drafts.is_empty() {
        return Ok((Vec::new(), memory_store_count(data_path)?));
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin memory upsert transaction failed: {err}"))?;

    // Inject draft tags into jieba so judgment tokenization keeps these terms intact.
    let draft_tags: Vec<String> = drafts
        .iter()
        .flat_map(|d| d.tags.iter().cloned())
        .collect();
    memory_jieba_add_words(&draft_tags);

    let now = now_iso();
    let mut results = Vec::<MemorySaveUpsertItemResult>::new();
    for draft in drafts {
        let memory_type = memory_store_normalize_memory_type(&draft.memory_type)?;
        if memory_contains_sensitive(&draft.judgment, &draft.tags) {
            results.push(MemorySaveUpsertItemResult {
                saved: false,
                id: None,
                tags: None,
                updated_at: None,
                reason: Some("sensitive_rejected".to_string()),
            });
            continue;
        }

        let existing_id = tx
            .query_row(
                "SELECT id FROM memory_record WHERE lower(trim(judgment))=lower(trim(?1)) LIMIT 1",
                params![draft.judgment],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|err| format!("Lookup existing memory by judgment failed: {err}"))?;

        let memory_id = if let Some(id) = existing_id {
            tx.execute(
                "UPDATE memory_record SET memory_type=?1, judgment=?2, reasoning=?3, updated_at=?4 WHERE id=?5",
                params![memory_type, draft.judgment, draft.reasoning, now, id],
            )
            .map_err(|err| format!("Update memory_record failed: {err}"))?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO memory_record(id, memory_type, judgment, reasoning, strength, is_active, memory_scope, useful_count, useful_score, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, 1, 'public', 0, 0, ?5, ?6)",
                params![id, memory_type, draft.judgment, draft.reasoning, now, now],
            )
            .map_err(|err| format!("Insert memory_record failed: {err}"))?;
            id
        };

        memory_store_sync_tags(&tx, &memory_id, &draft.tags)?;
        memory_store_sync_memory_fts(&tx, &memory_id)?;

        results.push(MemorySaveUpsertItemResult {
            saved: true,
            id: Some(memory_id),
            tags: Some(draft.tags.clone()),
            updated_at: Some(now.clone()),
            reason: None,
        });
    }

    tx.commit()
        .map_err(|err| format!("Commit memory upsert transaction failed: {err}"))?;
    invalidate_memory_matcher_cache();

    let total = memory_store_count(data_path)?;
    Ok((results, total))
}

fn memory_store_count(data_path: &PathBuf) -> Result<usize, String> {
    let conn = memory_store_open(data_path)?;
    let count = conn
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_record failed: {err}"))?;
    Ok(count.max(0) as usize)
}

fn memory_store_list_memories(data_path: &PathBuf) -> Result<Vec<MemoryEntry>, String> {
    let conn = memory_store_open(data_path)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, memory_type, judgment, reasoning, created_at, updated_at
             FROM memory_record
             ORDER BY updated_at DESC",
        )
        .map_err(|err| format!("Prepare list memories failed: {err}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|err| format!("Query list memories failed: {err}"))?;

    let mut out = Vec::<MemoryEntry>::new();
    for row in rows {
        let (id, memory_type, judgment, reasoning, created_at, updated_at) =
            row.map_err(|err| format!("Read memory row failed: {err}"))?;
        let mut tag_stmt = conn
            .prepare(
                "SELECT t.name
                 FROM memory_tag_rel r
                 JOIN global_tag t ON t.id=r.tag_id
                 WHERE r.memory_id=?1
                 ORDER BY t.name ASC",
            )
            .map_err(|err| format!("Prepare list tags failed: {err}"))?;
        let tags_iter = tag_stmt
            .query_map(params![id.clone()], |tag_row| tag_row.get::<_, String>(0))
            .map_err(|err| format!("Query list tags failed: {err}"))?;
        let mut tags = Vec::<String>::new();
        for tag in tags_iter {
            tags.push(tag.map_err(|err| format!("Read tag row failed: {err}"))?);
        }

        out.push(MemoryEntry {
            id,
            memory_type,
            judgment,
            reasoning,
            tags,
            created_at,
            updated_at,
        });
    }

    Ok(out)
}

fn memory_store_delete_memory(data_path: &PathBuf, memory_id: &str) -> Result<(), String> {
    let target_id = memory_id.trim();
    if target_id.is_empty() {
        return Err("memory_id is required".to_string());
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin memory delete transaction failed: {err}"))?;

    tx.execute("DELETE FROM memory_fts WHERE item_id=?1", params![target_id])
        .map_err(|err| format!("Delete memory_fts failed: {err}"))?;
    let affected = tx
        .execute("DELETE FROM memory_record WHERE id=?1", params![target_id])
        .map_err(|err| format!("Delete memory_record failed: {err}"))?;
    if affected == 0 {
        return Err("Memory not found".to_string());
    }

    tx.commit()
        .map_err(|err| format!("Commit memory delete transaction failed: {err}"))?;
    invalidate_memory_matcher_cache();
    Ok(())
}

fn memory_store_import_memories(
    data_path: &PathBuf,
    incoming: &[MemoryEntry],
) -> Result<MemoryStoreImportStats, String> {
    let mut drafts = Vec::<MemoryDraftInput>::new();
    let mut imported_count = 0usize;
    for item in incoming {
        let judgment = clean_text(item.judgment.trim());
        if judgment.is_empty() {
            continue;
        }
        let tags = normalize_memory_keywords(&item.tags);
        if tags.is_empty() {
            continue;
        }
        let memory_type = memory_store_normalize_memory_type(&item.memory_type)?;
        let reasoning = clean_text(item.reasoning.trim());
        imported_count += 1;
        drafts.push(MemoryDraftInput {
            memory_type,
            judgment,
            reasoning,
            tags,
        });
    }

    let before = memory_store_count(data_path)?;
    let (results, total_count) = memory_store_upsert_drafts(data_path, &drafts)?;
    let created_count = total_count.saturating_sub(before);
    let merged_count = results.iter().filter(|r| r.saved).count().saturating_sub(created_count);

    Ok(MemoryStoreImportStats {
        imported_count,
        created_count,
        merged_count,
        total_count,
    })
}

fn memory_store_merge_drafts(data_path: &PathBuf, drafts: &[MemoryDraftInput]) -> Result<usize, String> {
    let (results, _) = memory_store_upsert_drafts(data_path, drafts)?;
    Ok(results.into_iter().filter(|r| r.saved).count())
}

fn memory_store_collect_doc_texts(conn: &Connection) -> Result<StdHashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT id, judgment FROM memory_record")
        .map_err(|err| format!("Prepare collect doc texts failed: {err}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|err| format!("Query collect doc texts failed: {err}"))?;
    let mut out = StdHashMap::<String, String>::new();
    for row in rows {
        let (id, text) = row.map_err(|err| format!("Read collect doc row failed: {err}"))?;
        out.insert(id, text);
    }
    Ok(out)
}

fn memory_store_normalize_model_id(raw: &str) -> Result<String, String> {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' || ch == '.' || ch == '/' || ch == ':' {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return Err("model_name is empty after normalization".to_string());
    }
    Ok(out)
}

fn memory_store_model_store_db_path(data_path: &PathBuf, model_name: &str) -> Result<PathBuf, String> {
    let norm = memory_store_normalize_model_id(model_name)?;
    Ok(app_root_from_data_path(data_path)
        .join("memory")
        .join(format!("{norm}_embedding_store.db")))
}

fn memory_store_provider_table(_provider_id: &str) -> Result<String, String> {
    Ok("memory_vector".to_string())
}

fn memory_store_open_provider_vector_db(
    data_path: &PathBuf,
    provider_id: &str,
) -> Result<Connection, String> {
    let main_conn = memory_store_open(data_path)?;
    let model = memory_store_provider_model_name(&main_conn, provider_id)?
        .ok_or_else(|| format!("provider '{provider_id}' has no model_name in embedding_provider"))?;
    let path = memory_store_model_store_db_path(data_path, &model)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create provider vector db dir failed ({}): {err}", parent.display()))?;
    }
    let conn = Connection::open(&path)
        .map_err(|err| format!("Open provider vector db failed ({}): {err}", path.display()))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         CREATE TABLE IF NOT EXISTS memory_vector (
           chunk_id TEXT PRIMARY KEY,
           embedding_json TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );",
    )
    .map_err(|err| format!("Init provider vector db failed ({}): {err}", path.display()))?;
    Ok(conn)
}

fn memory_store_provider_index_ids(conn: &Connection, table: &str) -> Result<StdHashSet<String>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT chunk_id FROM {table}"))
        .map_err(|err| format!("Prepare list provider index ids failed: {err}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| format!("Query list provider index ids failed: {err}"))?;
    let mut out = StdHashSet::<String>::new();
    for row in rows {
        out.insert(row.map_err(|err| format!("Read provider index id failed: {err}"))?);
    }
    Ok(out)
}

fn memory_store_sync_provider_index<F>(
    data_path: &PathBuf,
    new_provider_id: &str,
    model_name: &str,
    batch_size: usize,
    mut embedder: F,
) -> Result<MemoryStoreProviderSyncReport, String>
where
    F: FnMut(&[String]) -> Result<Vec<Vec<f32>>, String>,
{
    let mut conn = memory_store_open(data_path)?;

    let old_provider_id = memory_store_get_runtime_state(&conn, "active_index_provider_id")?;
    let provider_store_exists = memory_store_model_store_db_path(data_path, model_name)
        .map(|p| p.exists())
        .unwrap_or(false);
    if old_provider_id.as_deref() == Some(new_provider_id.trim()) && provider_store_exists {
        return Ok(MemoryStoreProviderSyncReport {
            status: "no_op".to_string(),
            old_provider_id,
            new_provider_id: new_provider_id.trim().to_string(),
            deleted: 0,
            added: 0,
            batch_count: 0,
        });
    }

    memory_store_set_runtime_state(&conn, "rebuild_status", "running")?;
    memory_store_set_runtime_state(&conn, "rebuild_trace_id", &Uuid::new_v4().to_string())?;
    memory_store_set_runtime_state(&conn, "rebuild_done_batches", "0")?;
    memory_store_set_runtime_state(&conn, "rebuild_total_batches", "0")?;
    memory_store_set_runtime_state(&conn, "rebuild_error", "")?;

    let sync_result = (|| {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| format!("Begin provider sync tx failed: {err}"))?;

        let doc_map = memory_store_collect_doc_texts(&tx)?;
        let doc_ids = doc_map.keys().cloned().collect::<StdHashSet<_>>();
        let table = memory_store_provider_table(new_provider_id)?;
        let vector_path = memory_store_model_store_db_path(data_path, model_name)?;
        if let Some(parent) = vector_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Create provider vector db dir failed ({}): {err}",
                    parent.display()
                )
            })?;
        }
        let vector_conn = Connection::open(&vector_path)
            .map_err(|err| format!("Open provider vector db failed ({}): {err}", vector_path.display()))?;
        vector_conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS memory_vector (
               chunk_id TEXT PRIMARY KEY,
               embedding_json TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );",
        )
        .map_err(|err| format!("Init provider vector db failed ({}): {err}", vector_path.display()))?;
        let index_ids = memory_store_provider_index_ids(&vector_conn, &table)?;

        let to_delete = index_ids
            .difference(&doc_ids)
            .cloned()
            .collect::<Vec<_>>();
        let mut to_add = doc_ids
            .difference(&index_ids)
            .cloned()
            .collect::<Vec<_>>();
        to_add.sort();
        let effective_batch = batch_size.max(1);
        let total_batches = if to_add.is_empty() { 0 } else { to_add.len().div_ceil(effective_batch) };
        memory_store_set_runtime_state(&tx, "rebuild_total_batches", &total_batches.to_string())?;

        let mut deleted = 0usize;
        for id in &to_delete {
            vector_conn
                .execute(&format!("DELETE FROM {table} WHERE chunk_id=?1"), params![id])
                .map_err(|err| format!("Delete provider vector row failed: {err}"))?;
            deleted += 1;
        }

        let mut added = 0usize;
        let mut batch_count = 0usize;
        let mut dimension: usize = 0;

        for chunk in to_add.chunks(effective_batch) {
            let texts = chunk
                .iter()
                .filter_map(|id| doc_map.get(id).cloned())
                .collect::<Vec<_>>();
            if texts.is_empty() {
                continue;
            }
            let vectors = embedder(&texts)?;
            if vectors.len() != texts.len() {
                return Err("Embedding output length mismatch".to_string());
            }
            if let Some(first) = vectors.first() {
                dimension = first.len();
            }

            for (idx, chunk_id) in chunk.iter().enumerate() {
                let embedding = vectors
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| "Missing embedding row".to_string())?;
                let embedding_json = serde_json::to_string(&embedding)
                    .map_err(|err| format!("Serialize embedding failed: {err}"))?;
                vector_conn
                    .execute(
                    &format!(
                        "INSERT INTO {table}(chunk_id, embedding_json, updated_at)
                         VALUES (?1, ?2, ?3)
                         ON CONFLICT(chunk_id) DO UPDATE SET embedding_json=excluded.embedding_json, updated_at=excluded.updated_at"
                    ),
                    params![chunk_id, embedding_json, now_iso()],
                )
                .map_err(|err| format!("Upsert provider embedding failed: {err}"))?;
                added += 1;
            }
            batch_count += 1;
            memory_store_set_runtime_state(&tx, "rebuild_done_batches", &batch_count.to_string())?;
        }

        tx.execute(
            "UPDATE embedding_provider SET is_active=0, updated_at=?1",
            params![now_iso()],
        )
        .map_err(|err| format!("Reset embedding provider active flag failed: {err}"))?;
        tx.execute(
            "INSERT INTO embedding_provider(provider_id, dimension, model_name, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?5)
             ON CONFLICT(provider_id) DO UPDATE SET dimension=excluded.dimension, model_name=excluded.model_name, is_active=1, updated_at=excluded.updated_at",
            params![new_provider_id.trim(), (dimension as i64), model_name.trim(), now_iso(), now_iso()],
        )
        .map_err(|err| format!("Upsert embedding provider failed: {err}"))?;
        memory_store_set_runtime_state(&tx, "active_index_provider_id", new_provider_id.trim())?;
        memory_store_set_runtime_state(&tx, "rebuild_status", "idle")?;
        memory_store_set_runtime_state(&tx, "rebuild_done_batches", &batch_count.to_string())?;
        tx.commit()
            .map_err(|err| format!("Commit provider sync tx failed: {err}"))?;

        Ok(MemoryStoreProviderSyncReport {
            status: "synced".to_string(),
            old_provider_id,
            new_provider_id: new_provider_id.trim().to_string(),
            deleted,
            added,
            batch_count,
        })
    })();

    if let Err(err) = sync_result.as_ref() {
        let _ = memory_store_set_runtime_state(&conn, "rebuild_status", "failed");
        let _ = memory_store_set_runtime_state(&conn, "rebuild_error", err);
    }

    sync_result
}

fn memory_store_rebuild_indexes(data_path: &PathBuf) -> Result<MemoryStoreRebuildReport, String> {
    // Inject all tags into jieba before rebuilding FTS so tokenization keeps known terms intact.
    {
        let conn = memory_store_open(data_path)?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT name FROM global_tag")
            .map_err(|err| format!("Prepare list all tags for jieba failed: {err}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| format!("Query all tags for jieba failed: {err}"))?;
        let mut all_tags = Vec::<String>::new();
        for row in rows {
            all_tags.push(row.map_err(|err| format!("Read tag for jieba failed: {err}"))?);
        }
        memory_jieba_add_words(&all_tags);
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin rebuild indexes tx failed: {err}"))?;

    tx.execute("DELETE FROM memory_fts", [])
        .map_err(|err| format!("Clear memory_fts failed: {err}"))?;
    tx.execute("DELETE FROM note_fts", [])
        .map_err(|err| format!("Clear note_fts failed: {err}"))?;

    {
        let mut memory_stmt = tx
            .prepare("SELECT id FROM memory_record")
            .map_err(|err| format!("Prepare list memory ids failed: {err}"))?;
        let memory_ids = memory_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| format!("Query list memory ids failed: {err}"))?;
        for id in memory_ids {
            let memory_id = id.map_err(|err| format!("Read memory id failed: {err}"))?;
            memory_store_sync_memory_fts(&tx, &memory_id)?;
        }
    }

    tx.execute(
        "INSERT INTO note_fts(item_id, tags)
         SELECT n.source_id, COALESCE(GROUP_CONCAT(t.name, ' '), '')
         FROM note_index_record n
         LEFT JOIN note_tag_rel r ON r.source_id = n.source_id
         LEFT JOIN global_tag t ON t.id = r.tag_id
         GROUP BY n.source_id",
        [],
    )
    .map_err(|err| format!("Rebuild note_fts failed: {err}"))?;

    tx.commit()
        .map_err(|err| format!("Commit rebuild indexes tx failed: {err}"))?;

    let conn = memory_store_open(data_path)?;
    let memory_rows = conn
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_record failed: {err}"))?
        .max(0) as usize;
    let memory_fts_rows = conn
        .query_row("SELECT COUNT(1) FROM memory_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_fts failed: {err}"))?
        .max(0) as usize;
    let note_rows = conn
        .query_row("SELECT COUNT(1) FROM note_index_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_index_record failed: {err}"))?
        .max(0) as usize;
    let note_fts_rows = conn
        .query_row("SELECT COUNT(1) FROM note_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_fts failed: {err}"))?
        .max(0) as usize;

    Ok(MemoryStoreRebuildReport {
        memory_rows,
        memory_fts_rows,
        note_rows,
        note_fts_rows,
    })
}

fn memory_store_health_check(
    data_path: &PathBuf,
    auto_repair: bool,
) -> Result<MemoryStoreHealthReport, String> {
    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin health check tx failed: {err}"))?;

    let memory_rows = tx
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_record failed: {err}"))?
        .max(0) as usize;
    let memory_fts_rows = tx
        .query_row("SELECT COUNT(1) FROM memory_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_fts failed: {err}"))?
        .max(0) as usize;
    let note_rows = tx
        .query_row("SELECT COUNT(1) FROM note_index_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_index_record failed: {err}"))?
        .max(0) as usize;
    let note_fts_rows = tx
        .query_row("SELECT COUNT(1) FROM note_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_fts failed: {err}"))?
        .max(0) as usize;

    let orphan_memory_tag_rows = tx
        .query_row(
            "SELECT COUNT(1)
             FROM memory_tag_rel r
             LEFT JOIN memory_record m ON m.id=r.memory_id
             LEFT JOIN global_tag t ON t.id=r.tag_id
             WHERE m.id IS NULL OR t.id IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("Count orphan memory_tag_rel failed: {err}"))?
        .max(0) as usize;
    let orphan_note_tag_rows = tx
        .query_row(
            "SELECT COUNT(1)
             FROM note_tag_rel r
             LEFT JOIN note_index_record n ON n.source_id=r.source_id
             LEFT JOIN global_tag t ON t.id=r.tag_id
             WHERE n.source_id IS NULL OR t.id IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("Count orphan note_tag_rel failed: {err}"))?
        .max(0) as usize;

    let mut repaired = false;
    if auto_repair {
        if orphan_memory_tag_rows > 0 {
            tx.execute(
                "DELETE FROM memory_tag_rel
                 WHERE memory_id NOT IN (SELECT id FROM memory_record)
                    OR tag_id NOT IN (SELECT id FROM global_tag)",
                [],
            )
            .map_err(|err| format!("Repair memory_tag_rel failed: {err}"))?;
            repaired = true;
        }
        if orphan_note_tag_rows > 0 {
            tx.execute(
                "DELETE FROM note_tag_rel
                 WHERE source_id NOT IN (SELECT source_id FROM note_index_record)
                    OR tag_id NOT IN (SELECT id FROM global_tag)",
                [],
            )
            .map_err(|err| format!("Repair note_tag_rel failed: {err}"))?;
            repaired = true;
        }
    }

    tx.commit()
        .map_err(|err| format!("Commit health check tx failed: {err}"))?;

    let needs_rebuild =
        memory_rows != memory_fts_rows || note_rows != note_fts_rows || orphan_memory_tag_rows > 0 || orphan_note_tag_rows > 0;
    if auto_repair && needs_rebuild {
        let _ = memory_store_rebuild_indexes(data_path)?;
        repaired = true;
    }

    let status = if needs_rebuild {
        if auto_repair {
            "repaired"
        } else {
            "warn"
        }
    } else {
        "ok"
    };

    let refreshed = if auto_repair && needs_rebuild {
        memory_store_rebuild_indexes(data_path).ok()
    } else {
        None
    };
    let (memory_fts_rows_final, note_fts_rows_final) = if let Some(report) = refreshed {
        (report.memory_fts_rows, report.note_fts_rows)
    } else {
        (memory_fts_rows, note_fts_rows)
    };

    Ok(MemoryStoreHealthReport {
        status: status.to_string(),
        memory_rows,
        memory_fts_rows: memory_fts_rows_final,
        note_rows,
        note_fts_rows: note_fts_rows_final,
        orphan_memory_tag_rows,
        orphan_note_tag_rows,
        repaired,
    })
}

fn memory_store_backup_db(data_path: &PathBuf, target_path: &PathBuf) -> Result<MemoryStoreBackupResult, String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create backup dir failed ({}): {err}", parent.display()))?;
    }
    let conn = memory_store_open(data_path)?;
    let target_sql = target_path.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{target_sql}';"))
        .map_err(|err| format!("Backup memory db via VACUUM INTO failed: {err}"))?;
    let bytes = fs::metadata(target_path)
        .map_err(|err| format!("Read backup file metadata failed: {err}"))?
        .len();
    Ok(MemoryStoreBackupResult {
        path: target_path.to_string_lossy().to_string(),
        bytes,
    })
}

fn memory_store_restore_db(data_path: &PathBuf, source_path: &PathBuf) -> Result<MemoryStoreBackupResult, String> {
    if !source_path.exists() {
        return Err(format!("Restore source not found: {}", source_path.display()));
    }
    let target = memory_store_db_path(data_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create memory db dir failed ({}): {err}", parent.display()))?;
    }
    let wal = PathBuf::from(format!("{}-wal", target.to_string_lossy()));
    let shm = PathBuf::from(format!("{}-shm", target.to_string_lossy()));
    if wal.exists() {
        let _ = fs::remove_file(&wal);
    }
    if shm.exists() {
        let _ = fs::remove_file(&shm);
    }
    fs::copy(source_path, &target)
        .map_err(|err| format!("Copy restore db failed: {err}"))?;
    let _ = memory_store_open(data_path)?;
    let _ = memory_store_rebuild_indexes(data_path)?;
    let bytes = fs::metadata(&target)
        .map_err(|err| format!("Read restored db metadata failed: {err}"))?
        .len();
    Ok(MemoryStoreBackupResult {
        path: target.to_string_lossy().to_string(),
        bytes,
    })
}

#[cfg(test)]
mod memory_store_tests {
    use super::*;

    fn temp_data_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("easy_call_ai_tests")
            .join(format!("{}_{}", name, Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp dir");
        root.join("app_data.json")
    }

    #[test]
    fn memory_store_crud_and_import_should_work() {
        let data_path = temp_data_path("memory_store_crud");

        let drafts = vec![MemoryDraftInput {
            memory_type: "knowledge".to_string(),
            judgment: "Alice likes rust".to_string(),
            reasoning: "用户偏好".to_string(),
            tags: vec!["alice".to_string(), "rust".to_string()],
        }];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("upsert drafts");
        assert_eq!(saved.len(), 1);
        assert!(saved[0].saved);
        assert_eq!(total, 1);

        let memories = memory_store_list_memories(&data_path).expect("list memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].tags, vec!["alice".to_string(), "rust".to_string()]);

        let stats = memory_store_import_memories(&data_path, &vec![MemoryEntry {
            id: String::new(),
            memory_type: "knowledge".to_string(),
            judgment: "Alice likes rust".to_string(),
            reasoning: "".to_string(),
            tags: vec!["backend".to_string(), "rust".to_string()],
            created_at: now_iso(),
            updated_at: now_iso(),
        }])
        .expect("import memories");
        assert_eq!(stats.imported_count, 1);
        assert_eq!(stats.total_count, 1);

        let memories_after = memory_store_list_memories(&data_path).expect("list memories after");
        assert_eq!(memories_after.len(), 1);
        assert!(memories_after[0].tags.contains(&"backend".to_string()));
    }

    #[test]
    fn memory_store_delete_should_remove_record_and_fts() {
        let data_path = temp_data_path("memory_store_delete");
        let drafts = vec![MemoryDraftInput {
            memory_type: "knowledge".to_string(),
            judgment: "删除测试样本".to_string(),
            reasoning: "delete".to_string(),
            tags: vec!["删除".to_string(), "样本".to_string()],
        }];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 1);
        let memory_id = saved[0].id.clone().expect("saved id");

        memory_store_delete_memory(&data_path, &memory_id).expect("delete memory");

        let memories = memory_store_list_memories(&data_path).expect("list memories");
        assert!(memories.is_empty());

        let conn = memory_store_open(&data_path).expect("open conn");
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM memory_fts WHERE item_id=?1",
                params![memory_id],
                |row| row.get(0),
            )
            .expect("count memory fts");
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn provider_sync_should_rebuild_and_switch() {
        let data_path = temp_data_path("provider_sync");
        let _ = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Use sqlite for truth source".to_string(),
                reasoning: "".to_string(),
                tags: vec!["sqlite".to_string()],
            }],
        )
        .expect("seed memory");

        let report = memory_store_sync_provider_index(
            &data_path,
            "openai_text_embedding_3_large",
            "text-embedding-3-large",
            16,
            |texts| {
                Ok(texts
                    .iter()
                    .map(|text| vec![text.len() as f32, 1.0, 2.0])
                    .collect::<Vec<_>>())
            },
        )
        .expect("sync provider");

        assert_eq!(report.status, "synced");
        assert_eq!(report.new_provider_id, "openai_text_embedding_3_large");
        assert_eq!(report.added, 1);

        let conn = memory_store_open(&data_path).expect("open conn");
        let active = memory_store_get_runtime_state(&conn, "active_index_provider_id")
            .expect("runtime state")
            .expect("active provider");
        assert_eq!(active, "openai_text_embedding_3_large");
    }

    #[test]
    fn provider_sync_should_support_noop_and_delete_diff() {
        let data_path = temp_data_path("provider_sync_diff");
        let (saved, _) = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "Document A".to_string(),
                reasoning: "".to_string(),
                tags: vec!["a".to_string()],
            }],
        )
        .expect("seed memory");
        let first_id = saved[0].id.clone().expect("saved id");

        let report1 = memory_store_sync_provider_index(
            &data_path,
            "provider_x",
            "model-x",
            8,
            |texts| Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect::<Vec<_>>()),
        )
        .expect("first sync");
        assert_eq!(report1.added, 1);

        let report2 = memory_store_sync_provider_index(
            &data_path,
            "provider_x",
            "model-x",
            8,
            |texts| Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect::<Vec<_>>()),
        )
        .expect("second sync");
        assert_eq!(report2.status, "no_op");

        let conn = memory_store_open(&data_path).expect("open conn");
        conn.execute(
            "DELETE FROM memory_record WHERE id=?1",
            params![first_id],
        )
        .expect("delete memory record");
        conn.execute("DELETE FROM memory_fts", []).expect("clear memory fts");

        let report3 = memory_store_sync_provider_index(
            &data_path,
            "provider_y",
            "model-y",
            8,
            |texts| Ok(texts.iter().map(|_| vec![0.4, 0.5, 0.6]).collect::<Vec<_>>()),
        )
        .expect("third sync");
        assert_eq!(report3.deleted, 0);
        assert_eq!(report3.added, 0);
    }

    #[test]
    fn rebuild_health_backup_restore_should_work() {
        let data_path = temp_data_path("rebuild_health_backup");
        let _ = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "skill".to_string(),
                judgment: "Backup target memory".to_string(),
                reasoning: "".to_string(),
                tags: vec!["backup".to_string()],
            }],
        )
        .expect("seed memory");

        let rebuild = memory_store_rebuild_indexes(&data_path).expect("rebuild indexes");
        assert_eq!(rebuild.memory_rows, rebuild.memory_fts_rows);

        let health = memory_store_health_check(&data_path, false).expect("health check");
        assert_eq!(health.status, "ok");

        let backup_path = data_path
            .parent()
            .expect("parent")
            .join("memory_store_backup.db");
        let backup = memory_store_backup_db(&data_path, &backup_path).expect("backup db");
        assert!(backup.bytes > 0);

        let restore = memory_store_restore_db(&data_path, &backup_path).expect("restore db");
        assert!(restore.bytes > 0);
    }

    #[test]
    fn bm25_search_should_return_ranked_results() {
        let data_path = temp_data_path("bm25_ranked");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "用户偏好深色主题的编辑器风格".to_string(),
                reasoning: "UI偏好".to_string(),
                tags: vec!["风格".to_string(), "编辑器".to_string(), "偏好".to_string()],
            },
            MemoryDraftInput {
                memory_type: "skill".to_string(),
                judgment: "写代码时风格偏简洁，不喜欢过度抽象".to_string(),
                reasoning: "编码习惯".to_string(),
                tags: vec!["风格".to_string(), "代码".to_string()],
            },
            MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "今天讨论了项目架构的风格问题".to_string(),
                reasoning: "事件记录".to_string(),
                tags: vec!["架构".to_string(), "风格".to_string()],
            },
        ];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 3);
        assert!(saved.iter().all(|s| s.saved));

        let hits = memory_store_search_fts_bm25(&data_path, "风格", 10).expect("search");
        assert!(!hits.is_empty());
        assert!(hits.len() <= 3);
        assert!(hits.iter().all(|(_, s)| s.is_finite()));
    }

    #[test]
    fn bm25_search_should_produce_non_zero_and_non_binary_scores() {
        let data_path = temp_data_path("bm25_score_independent");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Rust Rust Rust 适合做高性能后端架构".to_string(),
                reasoning: "高频词样本".to_string(),
                tags: vec!["偏好".to_string()],
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Rust 也常用于工具链开发".to_string(),
                reasoning: "低频词样本".to_string(),
                tags: vec!["习惯".to_string()],
            },
            MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "今天讨论的是数据库迁移方案".to_string(),
                reasoning: "无关样本".to_string(),
                tags: vec!["会议".to_string()],
            },
        ];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 3);
        assert!(saved.iter().all(|s| s.saved));

        let hits = memory_store_search_fts_bm25(&data_path, "Rust", 10).expect("search");
        assert!(
            hits.len() >= 2,
            "expected at least 2 bm25 hits, got {}",
            hits.len()
        );

        let abs_scores = hits.iter().map(|(_, s)| s.abs()).collect::<Vec<_>>();
        assert!(
            abs_scores.iter().all(|s| s.is_finite()),
            "bm25 contains non-finite values: {abs_scores:?}"
        );
        assert!(
            abs_scores.iter().any(|s| *s > 0.0),
            "bm25 all zero: {abs_scores:?}"
        );

        let unique = abs_scores
            .iter()
            .map(|s| format!("{s:.9}"))
            .collect::<std::collections::HashSet<_>>();
        assert!(
            unique.len() >= 2,
            "bm25 appears binary/discrete for this sample: {abs_scores:?}"
        );
    }

    #[test]
    fn bm25_should_hit_chinese_exact_fragment() {
        let data_path = temp_data_path("bm25_chinese_fragment");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "我最喜欢的角色是遥酱，她的语气很温柔。".to_string(),
                reasoning: "".to_string(),
                tags: vec!["遥酱".to_string(), "偏好".to_string()],
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "今天复习了Rust生命周期".to_string(),
                reasoning: "".to_string(),
                tags: vec!["rust".to_string()],
            },
        ];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 2);
        assert!(saved.iter().all(|s| s.saved));

        let hits = memory_store_search_fts_bm25(&data_path, "遥酱", 10).expect("search");
        assert!(
            !hits.is_empty(),
            "expected bm25 hit for exact chinese fragment, got empty"
        );
        assert!(
            hits.iter().any(|(_, score)| score.abs() > 0.0),
            "expected non-zero bm25 score for chinese fragment, got {hits:?}"
        );
    }

    #[test]
    fn legacy_app_data_memories_should_migrate_once() {
        let data_path = temp_data_path("legacy_app_data_memories_migration_once");
        let parent = data_path.parent().expect("parent").to_path_buf();
        let app_data_path = parent.join("app_data.json");
        fs::write(
            &app_data_path,
            r#"{
              "version": 1,
              "agents": [],
              "selectedAgentId": "default-agent",
              "userAlias": "tester",
              "responseStyleId": "concise",
              "conversations": [],
              "archivedConversations": [],
              "imageTextCache": [],
              "memories": [
                {"memoryType":"knowledge","content":"用户偏好简洁回答","reasoning":"历史记忆","keywords":["偏好","简洁"]},
                {"memoryType":"skill","judgment":"偏好 Rust","reasoning":"","tags":["rust","backend"]}
              ]
            }"#,
        )
        .expect("write legacy app_data.json");

        let report = memory_store_migrate_legacy_app_data_memories(&data_path)
            .expect("migrate legacy")
            .expect("should migrate");
        assert_eq!(report.imported_count, 2);
        assert!(app_data_path.exists(), "app_data should be kept untouched");
        assert!(
            PathBuf::from(&report.archived_path).exists(),
            "archived file should exist"
        );

        let memories = memory_store_list_memories(&data_path).expect("list memories");
        assert_eq!(memories.len(), 2);

        let report2 =
            memory_store_migrate_legacy_app_data_memories(&data_path).expect("migrate again");
        assert!(report2.is_none(), "migration should run only once");
    }

    #[test]
    fn task_memory_type_should_be_rejected() {
        let data_path = temp_data_path("task_type_reject");
        let result = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "task".to_string(),
                judgment: "Do something".to_string(),
                reasoning: "todo".to_string(),
                tags: vec!["todo".to_string()],
            }],
        );
        assert!(result.is_err());
    }
}
