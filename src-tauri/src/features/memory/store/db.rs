fn memory_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("memory")
        .join(MEMORY_DB_FILE_NAME)
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

// ========== apply_pragmas_and_create_schema ==========
fn apply_pragmas_and_create_schema(conn: &Connection) -> Result<(), String> {
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
            owner_agent_id TEXT,
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
    Ok(())
}

// ========== migrate_owner_agent_col ==========
fn migrate_owner_agent_col(conn: &Connection) -> Result<(), String> {
    let has_owner_agent_col: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('memory_record') WHERE name='owner_agent_id'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if has_owner_agent_col == 0 {
        conn.execute_batch(
            "ALTER TABLE memory_record ADD COLUMN owner_agent_id TEXT;
             CREATE INDEX IF NOT EXISTS idx_memory_owner_agent_id ON memory_record(owner_agent_id);",
        )
        .map_err(|err| format!("Migrate memory_record owner_agent_id failed: {err}"))?;
    } else {
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_memory_owner_agent_id ON memory_record(owner_agent_id);")
            .map_err(|err| format!("Ensure idx_memory_owner_agent_id failed: {err}"))?;
    }
    Ok(())
}

// ========== migrate_memory_fts ==========
fn migrate_memory_fts(conn: &Connection) -> Result<(), String> {
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
    Ok(())
}

// ========== repopulate_fts_if_needed ==========
fn repopulate_fts_if_needed(conn: &Connection) -> Result<(), String> {
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

fn memory_store_init_schema(conn: &Connection) -> Result<(), String> {
    apply_pragmas_and_create_schema(conn)?;
    migrate_owner_agent_col(conn)?;
    migrate_memory_fts(conn)?;
    repopulate_fts_if_needed(conn)?;

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

