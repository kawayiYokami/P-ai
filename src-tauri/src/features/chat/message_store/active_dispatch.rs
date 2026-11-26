// ==================== 调度运行态（意外重启恢复用） ====================
//
// 只记两件事：哪个会话在调度、它此刻还排着哪些消息。
// 进调度与队列变动各写一次，收口删一次；执行过程（流式、工具、整理上下文）零写库。

#[derive(Debug, Clone)]
pub(super) struct ActiveDispatchRecord {
    pub(super) conversation_id: String,
    pub(super) pending_queue_json: String,
}

pub(super) fn active_dispatch_upsert(
    data_path: &PathBuf,
    conversation_id: &str,
    pending_queue_json: &str,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("会话 id 为空，无法记录调度运行态".to_string());
    }
    let conn = chat_metadata_store_open(data_path)?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO active_dispatches(conversation_id, pending_queue_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT(conversation_id) DO UPDATE SET
             pending_queue_json=excluded.pending_queue_json,
             updated_at=excluded.updated_at",
        rusqlite::params![conversation_id, pending_queue_json, now],
    )
    .map_err(|err| format!("写入调度运行态失败: {err}"))?;
    Ok(())
}

pub(super) fn active_dispatch_remove(data_path: &PathBuf, conversation_id: &str) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(());
    }
    let conn = chat_metadata_store_open(data_path)?;
    conn.execute(
        "DELETE FROM active_dispatches WHERE conversation_id=?1",
        rusqlite::params![conversation_id],
    )
    .map_err(|err| format!("清除调度运行态失败: {err}"))?;
    Ok(())
}

pub(super) fn active_dispatch_list_all(data_path: &PathBuf) -> Result<Vec<ActiveDispatchRecord>, String> {
    let conn = chat_metadata_store_open(data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, pending_queue_json FROM active_dispatches ORDER BY updated_at ASC",
        )
        .map_err(|err| format!("准备读取调度运行态失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ActiveDispatchRecord {
                conversation_id: row.get(0)?,
                pending_queue_json: row.get(1)?,
            })
        })
        .map_err(|err| format!("读取调度运行态失败: {err}"))?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|err| format!("读取调度运行态记录失败: {err}"))?);
    }
    Ok(records)
}
