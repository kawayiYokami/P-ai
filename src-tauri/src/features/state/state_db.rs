// ==================== state.sqlite 存储层 ====================
// V4：state 目录全部数据统一迁入 state/state.sqlite。
// 表结构：
//   state_migration(version INTEGER PRIMARY KEY, migrated_at TEXT)  迁移版本
//   runtime_state(key TEXT PRIMARY KEY, value TEXT)                 全局状态 k/v
//   image_text_cache(hash, model_api_id, media_type, description, text, updated_at)
//   pdf_text_cache(file_hash PRIMARY KEY, ...)
//   pdf_image_cache(file_hash PRIMARY KEY, ...)
//   remote_im_contacts(id TEXT PRIMARY KEY, channel_id, platform, ..., config_json)
//   remote_im_group_members(contact_id, user_id, nickname, card, display_name, updated_at)
//   remote_im_contact_groups(id, channel_id, name, sort_order, created_at, updated_at)  渠道内联系人自定义分组
//   remote_im_contact_checkpoints(contact_id TEXT PRIMARY KEY, checkpoint_json)
//   window_layouts(window_label TEXT PRIMARY KEY, width, height, x, y, maximized)
//   git_repo_history(repo_key TEXT PRIMARY KEY, history_json)

const STATE_DB_FILE_NAME: &str = "state.sqlite";

/// 初始化段互斥：`PRAGMA journal_mode=WAL` 切换不调用 busy handler，
/// 首次并发打开同一数据库时可能直接报 database is locked，串行化建表段规避。
static STATE_DB_INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn state_db_path(data_path: &PathBuf) -> PathBuf {
    app_layout_state_dir(data_path).join(STATE_DB_FILE_NAME)
}

fn state_db_open(data_path: &PathBuf) -> Result<rusqlite::Connection, String> {
    let db_path = state_db_path(data_path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!("创建 state 数据库目录失败，path={}，error={err}", parent.display())
        })?;
    }
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|err| format!("打开 state 数据库失败，path={}，error={err}", db_path.display()))?;
    let _init_guard = STATE_DB_INIT_LOCK.lock().map_err(|_| "锁定 state 数据库初始化失败".to_string())?;
    conn.execute_batch(
        "PRAGMA busy_timeout=10000;
         PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;

         CREATE TABLE IF NOT EXISTS state_migration (
           version INTEGER PRIMARY KEY,
           migrated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS runtime_state (
           key TEXT PRIMARY KEY,
           value TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS image_text_cache (
           hash TEXT NOT NULL,
           model_api_id TEXT NOT NULL,
           media_type TEXT NOT NULL DEFAULT '',
           description TEXT NOT NULL DEFAULT '',
           text TEXT NOT NULL,
           updated_at TEXT NOT NULL,
           PRIMARY KEY (hash, model_api_id, media_type, description)
         );

         CREATE TABLE IF NOT EXISTS pdf_text_cache (
           file_hash TEXT PRIMARY KEY,
           file_path TEXT NOT NULL,
           file_name TEXT NOT NULL,
           extracted_text TEXT NOT NULL,
           total_pages INTEGER NOT NULL,
           extracted_pages INTEGER NOT NULL,
           is_truncated INTEGER NOT NULL,
           conversation_ids TEXT NOT NULL,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS pdf_image_cache (
           file_hash TEXT PRIMARY KEY,
           file_path TEXT NOT NULL,
           file_name TEXT NOT NULL,
           total_pages INTEGER NOT NULL,
           rendered_pages INTEGER NOT NULL,
           dpi INTEGER NOT NULL,
           images_json TEXT NOT NULL,
           conversation_ids TEXT NOT NULL,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS remote_im_contacts (
           id TEXT PRIMARY KEY,
           channel_id TEXT NOT NULL,
           platform TEXT NOT NULL,
           remote_contact_type TEXT NOT NULL,
           remote_contact_id TEXT NOT NULL,
           config_json TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_remote_im_contacts_channel ON remote_im_contacts(channel_id);

         CREATE TABLE IF NOT EXISTS remote_im_group_members (
           contact_id TEXT NOT NULL,
           user_id TEXT NOT NULL,
           nickname TEXT NOT NULL DEFAULT '',
           card TEXT NOT NULL DEFAULT '',
           display_name TEXT NOT NULL DEFAULT '',
           updated_at TEXT,
           PRIMARY KEY (contact_id, user_id)
         );

         CREATE TABLE IF NOT EXISTS remote_im_contact_groups (
           id TEXT PRIMARY KEY,
           channel_id TEXT NOT NULL,
           name TEXT NOT NULL,
           sort_order INTEGER NOT NULL DEFAULT 0,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_remote_im_contact_groups_channel ON remote_im_contact_groups(channel_id);

         CREATE TABLE IF NOT EXISTS remote_im_contact_checkpoints (
           contact_id TEXT PRIMARY KEY,
           checkpoint_json TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS window_layouts (
           window_label TEXT PRIMARY KEY,
           width INTEGER,
           height INTEGER,
           x INTEGER,
           y INTEGER,
           maximized INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS git_repo_history (
           repo_key TEXT PRIMARY KEY,
           history_json TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS global_short_id_sequences (
           domain TEXT PRIMARY KEY,
           next_val INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS timers (
           timer_id TEXT PRIMARY KEY,
           conversation_id TEXT,
           message TEXT NOT NULL,
           dispatch_mode TEXT NOT NULL,
           enabled INTEGER NOT NULL DEFAULT 1,
           run_at_utc TEXT NOT NULL,
           cron_expression TEXT,
           end_at_utc TEXT,
           next_run_at_utc TEXT,
           last_triggered_at_utc TEXT,
           created_at_utc TEXT NOT NULL
         );

         CREATE INDEX IF NOT EXISTS idx_timers_active_next_run
         ON timers(enabled, next_run_at_utc);

         CREATE INDEX IF NOT EXISTS idx_timers_conversation
         ON timers(conversation_id);",
    )
    .map_err(|err| format!("初始化 state 数据库表结构失败，error={err}"))?;
    Ok(conn)
}

fn state_db_upsert_kv(
    data_path: &PathBuf,
    key: &str,
    value_json: &str,
) -> Result<(), String> {
    let conn = state_db_open(data_path)?;
    conn.execute(
        "INSERT INTO runtime_state(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value_json],
    )
    .map_err(|err| format!("写入 state k/v 失败，key={key}，error={err}"))?;
    Ok(())
}

fn state_db_get_kv(data_path: &PathBuf, key: &str) -> Result<Option<String>, String> {
    let conn = state_db_open(data_path)?;
    let mut stmt = conn
        .prepare("SELECT value FROM runtime_state WHERE key=?1")
        .map_err(|err| format!("准备 state k/v 查询失败，key={key}，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![key])
        .map_err(|err| format!("查询 state k/v 失败，key={key}，error={err}"))?;
    if let Some(row) = rows.next().map_err(|err| format!("读取 state k/v 行失败，key={key}，error={err}"))? {
        let value: String = row
            .get(0)
            .map_err(|err| format!("解析 state k/v 值失败，key={key}，error={err}"))?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

fn state_db_get_migration_version(data_path: &PathBuf) -> Result<Option<u32>, String> {
    let conn = state_db_open(data_path)?;
    let mut stmt = conn
        .prepare("SELECT version FROM state_migration ORDER BY version DESC LIMIT 1")
        .map_err(|err| format!("准备 state 迁移版本查询失败，error={err}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|err| format!("查询 state 迁移版本失败，error={err}"))?;
    if let Some(row) = rows.next().map_err(|err| format!("读取 state 迁移版本失败，error={err}"))? {
        let version: u32 = row
            .get(0)
            .map_err(|err| format!("解析 state 迁移版本失败，error={err}"))?;
        Ok(Some(version))
    } else {
        Ok(None)
    }
}

/// image_text_cache 超出上限时按 updated_at 淘汰最旧条目。
fn state_db_trim_image_text_cache(data_path: &PathBuf) -> Result<(), String> {
    let conn = state_db_open(data_path)?;
    conn.execute(
        "DELETE FROM image_text_cache WHERE rowid NOT IN (
           SELECT rowid FROM image_text_cache
           ORDER BY updated_at DESC, rowid DESC
           LIMIT ?1
         )",
        rusqlite::params![MAX_IMAGE_TEXT_CACHE_ENTRIES as i64],
    )
    .map_err(|err| format!("淘汰 image_text_cache 过期条目失败，error={err}"))?;
    Ok(())
}
