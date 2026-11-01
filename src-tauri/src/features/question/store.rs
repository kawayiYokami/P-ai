// ==================== 会话提问：SQLite 存储 ====================

fn question_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("question")
        .join(QUESTION_DB_FILE_NAME)
}

fn question_store_open(data_path: &PathBuf) -> Result<rusqlite::Connection, String> {
    let path = question_store_db_path(data_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Question db path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("创建问题数据目录失败：{err}"))?;
    let conn = rusqlite::Connection::open(&path)
        .map_err(|err| format!("打开问题数据库失败（{}）：{err}", path.display()))?;
    question_store_init(&conn)?;
    Ok(conn)
}

fn question_store_init(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS conversation_question (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            number INTEGER NOT NULL,
            title TEXT NOT NULL,
            background TEXT NOT NULL DEFAULT '',
            options_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_question_number
            ON conversation_question(conversation_id, number);
        CREATE TABLE IF NOT EXISTS question_next_number (
            conversation_id TEXT PRIMARY KEY,
            next_number INTEGER NOT NULL
        );
        COMMIT;",
    )
    .map_err(|err| format!("初始化问题数据库失败：{err}"))
}

fn question_row_to_entity(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationQuestion> {
    let options_json: String = row.get("options_json")?;
    let options = serde_json::from_str::<Vec<String>>(&options_json).unwrap_or_default();
    Ok(ConversationQuestion {
        id: row.get("id")?,
        conversation_id: row.get("conversation_id")?,
        number: row.get::<_, i64>("number")?.max(0) as u32,
        title: row.get("title")?,
        background: row.get("background")?,
        options,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn question_store_list(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Vec<ConversationQuestion>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, conversation_id, number, title, background, options_json, created_at, updated_at
             FROM conversation_question WHERE conversation_id = ?1 ORDER BY number ASC",
        )
        .map_err(|err| format!("准备问题查询失败：{err}"))?;
    let rows = stmt
        .query_map([conversation_id], question_row_to_entity)
        .map_err(|err| format!("查询问题列表失败：{err}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|err| format!("读取问题记录失败：{err}"))?);
    }
    Ok(items)
}

fn question_store_count(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<usize, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversation_question WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|err| format!("统计问题数量失败：{err}"))?;
    Ok(count.max(0) as usize)
}

fn question_store_get_by_number(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    number: u32,
) -> Result<Option<ConversationQuestion>, String> {
    conn.query_row(
        "SELECT id, conversation_id, number, title, background, options_json, created_at, updated_at
         FROM conversation_question WHERE conversation_id = ?1 AND number = ?2",
        rusqlite::params![conversation_id, number as i64],
        question_row_to_entity,
    )
    .optional()
    .map_err(|err| format!("读取问题记录失败：{err}"))
}

/// 分配下一个编号：`next_number` 游标与库内最大编号取大者。
/// 游标保证「删掉当前最大编号后也不复用」，最大编号保证游标被外部改坏时仍单调。
fn question_store_allocate_number(
    tx: &rusqlite::Transaction<'_>,
    conversation_id: &str,
) -> Result<u32, String> {
    let cursor: Option<i64> = tx
        .query_row(
            "SELECT next_number FROM question_next_number WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| format!("读取问题编号游标失败：{err}"))?;
    let max_existing: Option<i64> = tx
        .query_row(
            "SELECT MAX(number) FROM conversation_question WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|err| format!("读取问题最大编号失败：{err}"))?;
    let base = cursor
        .unwrap_or(1)
        .max(max_existing.unwrap_or(0).saturating_add(1))
        .max(1);
    let number = base as u32;
    tx.execute(
        "INSERT INTO question_next_number (conversation_id, next_number) VALUES (?1, ?2)
         ON CONFLICT(conversation_id) DO UPDATE SET next_number = excluded.next_number",
        rusqlite::params![conversation_id, (number as i64) + 1],
    )
    .map_err(|err| format!("更新问题编号游标失败：{err}"))?;
    Ok(number)
}

fn question_store_insert(
    conn: &mut rusqlite::Connection,
    conversation_id: &str,
    title: &str,
    background: &str,
    options: &[String],
) -> Result<ConversationQuestion, String> {
    let options_json = serde_json::to_string(options)
        .map_err(|err| format!("序列化问题选项失败：{err}"))?;
    let now = now_utc_rfc3339();
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启问题写入事务失败：{err}"))?;
    let number = question_store_allocate_number(&tx, conversation_id)?;
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO conversation_question
            (id, conversation_id, number, title, background, options_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            conversation_id,
            number as i64,
            title,
            background,
            options_json,
            now,
            now
        ],
    )
    .map_err(|err| format!("写入问题记录失败：{err}"))?;
    tx.commit()
        .map_err(|err| format!("提交问题写入事务失败：{err}"))?;
    Ok(ConversationQuestion {
        id,
        conversation_id: conversation_id.to_string(),
        number,
        title: title.to_string(),
        background: background.to_string(),
        options: options.to_vec(),
        created_at: now.clone(),
        updated_at: now,
    })
}

fn question_store_update(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    number: u32,
    title: &str,
    background: &str,
    options: &[String],
) -> Result<usize, String> {
    let options_json = serde_json::to_string(options)
        .map_err(|err| format!("序列化问题选项失败：{err}"))?;
    conn.execute(
        "UPDATE conversation_question
         SET title = ?1, background = ?2, options_json = ?3, updated_at = ?4
         WHERE conversation_id = ?5 AND number = ?6",
        rusqlite::params![
            title,
            background,
            options_json,
            now_utc_rfc3339(),
            conversation_id,
            number as i64
        ],
    )
    .map_err(|err| format!("更新问题记录失败：{err}"))
}

fn question_store_delete(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    number: u32,
) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM conversation_question WHERE conversation_id = ?1 AND number = ?2",
        rusqlite::params![conversation_id, number as i64],
    )
    .map_err(|err| format!("删除问题记录失败：{err}"))
}

/// 批量统计：一次性拿到所有会话的待答问题数，供会话列表渲染（避免逐条开关数据库）。
fn question_store_counts_by_conversation(
    conn: &rusqlite::Connection,
) -> Result<std::collections::HashMap<String, usize>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, COUNT(*) FROM conversation_question GROUP BY conversation_id",
        )
        .map_err(|err| format!("准备问题计数查询失败：{err}"))?;
    let rows = stmt
        .query_map([], |row| {
            let conversation_id: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((conversation_id, count.max(0) as usize))
        })
        .map_err(|err| format!("查询问题计数失败：{err}"))?;
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        let (conversation_id, count) = row.map_err(|err| format!("读取问题计数失败：{err}"))?;
        counts.insert(conversation_id, count);
    }
    Ok(counts)
}

/// 会话删除时连带清理其问题记录与编号游标。
fn question_store_delete_conversation(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM conversation_question WHERE conversation_id = ?1",
        [conversation_id],
    )
    .map_err(|err| format!("清理会话问题失败：{err}"))?;
    conn.execute(
        "DELETE FROM question_next_number WHERE conversation_id = ?1",
        [conversation_id],
    )
    .map_err(|err| format!("清理会话问题编号游标失败：{err}"))?;
    Ok(())
}
