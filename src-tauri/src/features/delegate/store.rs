fn delegate_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("delegate")
        .join(DELEGATE_DB_FILE_NAME)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DelegateConversationSnapshot {
    delegate_id: String,
    kind: String,
    conversation_id: String,
    root_conversation_id: String,
    title: String,
    why: String,
    goal: String,
    todo: String,
    target_agent_id: String,
    status: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    archived_at: Option<String>,
    #[serde(default)]
    last_message_at: Option<String>,
    message_count: usize,
    step_count: usize,
    tool_call_count: usize,
    last_tool_name: String,
    cumulative_usage: ConversationCumulativeUsage,
}

#[derive(Debug, Clone, Default)]
struct DelegateConversationSnapshotCache {
    by_delegate_id: std::collections::HashMap<String, DelegateConversationSnapshot>,
    ordered_delegate_ids: Vec<String>,
    by_root_conversation_id: std::collections::HashMap<String, Vec<String>>,
}

fn delegate_snapshot_cache_store(
) -> &'static Mutex<std::collections::HashMap<String, DelegateConversationSnapshotCache>> {
    static CACHE: OnceLock<
        Mutex<std::collections::HashMap<String, DelegateConversationSnapshotCache>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn delegate_snapshot_cache_key(data_path: &PathBuf) -> String {
    delegate_store_db_path(data_path).to_string_lossy().to_string()
}

fn delegate_snapshot_cache_build(
    snapshots: Vec<DelegateConversationSnapshot>,
) -> DelegateConversationSnapshotCache {
    let mut by_delegate_id = std::collections::HashMap::new();
    for snapshot in snapshots {
        by_delegate_id.insert(snapshot.delegate_id.clone(), snapshot);
    }
    delegate_snapshot_cache_from_map(by_delegate_id)
}

fn delegate_snapshot_cache_from_map(
    by_delegate_id: std::collections::HashMap<String, DelegateConversationSnapshot>,
) -> DelegateConversationSnapshotCache {
    let mut ordered = by_delegate_id.keys().cloned().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        let left_snapshot = by_delegate_id.get(left);
        let right_snapshot = by_delegate_id.get(right);
        delegate_snapshot_sort_key(right_snapshot)
            .cmp(&delegate_snapshot_sort_key(left_snapshot))
    });
    let mut by_root_conversation_id =
        std::collections::HashMap::<String, Vec<String>>::new();
    for delegate_id in &ordered {
        let Some(snapshot) = by_delegate_id.get(delegate_id) else {
            continue;
        };
        if snapshot.root_conversation_id.trim().is_empty() {
            continue;
        }
        by_root_conversation_id
            .entry(snapshot.root_conversation_id.clone())
            .or_default()
            .push(delegate_id.clone());
    }
    DelegateConversationSnapshotCache {
        by_delegate_id,
        ordered_delegate_ids: ordered,
        by_root_conversation_id,
    }
}

fn delegate_snapshot_sort_key(
    snapshot: Option<&DelegateConversationSnapshot>,
) -> (String, String, String) {
    let Some(snapshot) = snapshot else {
        return (String::new(), String::new(), String::new());
    };
    let primary = snapshot
        .archived_at
        .as_deref()
        .or(snapshot.last_message_at.as_deref())
        .unwrap_or(snapshot.updated_at.as_str())
        .to_string();
    (primary, snapshot.updated_at.clone(), snapshot.delegate_id.clone())
}

fn delegate_store_open(data_path: &PathBuf) -> Result<Connection, String> {
    let path = delegate_store_db_path(data_path);
    let parent = path
        .parent()
        .ok_or_else(|| "委托数据库路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("创建委托目录失败: {err}"))?;
    let conn = Connection::open(&path)
        .map_err(|err| format!("打开委托数据库失败 ({}): {err}", path.display()))?;
    delegate_store_init(&conn)?;
    Ok(conn)
}

fn delegate_store_init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS delegate_record (
            delegate_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            parent_delegate_id TEXT,
            source_agent_id TEXT NOT NULL,
            target_agent_id TEXT NOT NULL,
            title TEXT NOT NULL,
            why TEXT NOT NULL,
            goal TEXT NOT NULL,
            todo TEXT NOT NULL,
            notify_assistant_when_done INTEGER NOT NULL,
            call_stack_json TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            delivered_at TEXT,
            completed_at TEXT
        );
        COMMIT;",
    )
    .map_err(|err| format!("初始化委托数据库失败: {err}"))?;
    delegate_store_migrate_why_goal_todo(conn)?;
    delegate_store_migrate_relax_legacy_department_columns(conn)?;
    delegate_store_migrate_snapshot_columns(conn)?;
    Ok(())
}

fn delegate_store_table_columns(conn: &Connection) -> Result<std::collections::HashSet<String>, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(delegate_record)")
        .map_err(|err| format!("读取委托表结构失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| format!("读取委托表字段失败: {err}"))?;
    let mut columns = std::collections::HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|err| format!("读取委托表字段失败: {err}"))?);
    }
    Ok(columns)
}

fn delegate_store_sql_expr_for_column(
    columns: &std::collections::HashSet<String>,
    preferred: &str,
    legacy: &[&str],
) -> String {
    let mut exprs = Vec::<String>::new();
    if columns.contains(preferred) {
        exprs.push(format!("NULLIF({preferred}, '')"));
    }
    for column in legacy {
        if columns.contains(*column) {
            exprs.push(format!("NULLIF({column}, '')"));
        }
    }
    if exprs.is_empty() {
        "''".to_string()
    } else {
        format!("COALESCE({}, '')", exprs.join(", "))
    }
}

fn delegate_store_migrate_why_goal_todo(conn: &Connection) -> Result<(), String> {
    let columns = delegate_store_table_columns(conn)?;
    let has_new_columns =
        columns.contains("why") && columns.contains("goal") && columns.contains("todo");
    let has_legacy_columns = columns.contains("background")
        || columns.contains("instruction")
        || columns.contains("question")
        || columns.contains("specific_goal")
        || columns.contains("deliverable_requirement")
        || columns.contains("focus");
    if has_new_columns && !has_legacy_columns {
        return Ok(());
    }
    let why_expr = delegate_store_sql_expr_for_column(&columns, "why", &["background"]);
    let goal_expr = delegate_store_sql_expr_for_column(&columns, "goal", &["question", "instruction"]);
    let todo_expr =
        delegate_store_sql_expr_for_column(&columns, "todo", &["focus", "specific_goal", "deliverable_requirement"]);
    let sql = format!(
        "BEGIN;
        CREATE TABLE delegate_record_next (
            delegate_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            parent_delegate_id TEXT,
            source_department_id TEXT,
            target_department_id TEXT,
            source_agent_id TEXT NOT NULL,
            target_agent_id TEXT NOT NULL,
            title TEXT NOT NULL,
            why TEXT NOT NULL,
            goal TEXT NOT NULL,
            todo TEXT NOT NULL,
            notify_assistant_when_done INTEGER NOT NULL,
            call_stack_json TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            delivered_at TEXT,
            completed_at TEXT
        );
        INSERT INTO delegate_record_next (
            delegate_id, kind, conversation_id, parent_delegate_id,
            source_department_id, target_department_id, source_agent_id, target_agent_id,
            title, why, goal, todo,
            notify_assistant_when_done, call_stack_json, status, created_at, updated_at, delivered_at, completed_at
        )
        SELECT
            delegate_id, kind, conversation_id, parent_delegate_id,
            source_department_id, target_department_id, source_agent_id, target_agent_id,
            title, {why_expr}, {goal_expr}, {todo_expr},
            notify_assistant_when_done, call_stack_json, status, created_at, updated_at, delivered_at, completed_at
        FROM delegate_record;
        DROP TABLE delegate_record;
        ALTER TABLE delegate_record_next RENAME TO delegate_record;
        COMMIT;"
    );
    conn.execute_batch(&sql)
        .map_err(|err| format!("迁移委托字段 why/goal/todo 失败: {err}"))?;
    runtime_log_info(format!("[委托] 完成，任务=迁移字段why_goal_todo"));
    Ok(())
}

/// 放宽旧委托表遗留的部门列约束。
///
/// 部门已从运行时模型退场，新记录不再写这两列；但历史库中它们带着 NOT NULL，
/// 会强迫新记录继续填值。这里把这两列改为可空：旧值原样保留，新记录不再落值。
fn delegate_store_migrate_relax_legacy_department_columns(conn: &Connection) -> Result<(), String> {
    const LEGACY_DEPARTMENT_COLUMNS: [&str; 2] = ["source_department_id", "target_department_id"];
    let mut stmt = conn
        .prepare("PRAGMA table_info(delegate_record)")
        .map_err(|err| format!("读取委托表结构失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)? != 0,
            ))
        })
        .map_err(|err| format!("读取委托表字段失败: {err}"))?;
    let mut definitions = Vec::<String>::new();
    let mut names = Vec::<String>::new();
    let mut needs_relax = false;
    for row in rows {
        let (name, column_type, not_null, default_value, primary_key) =
            row.map_err(|err| format!("读取委托表字段失败: {err}"))?;
        let is_legacy_department = LEGACY_DEPARTMENT_COLUMNS.contains(&name.as_str());
        if is_legacy_department && not_null {
            needs_relax = true;
        }
        let mut definition = format!("{name} {column_type}");
        if primary_key {
            definition.push_str(" PRIMARY KEY");
        } else if not_null && !is_legacy_department {
            definition.push_str(" NOT NULL");
        }
        if let Some(default_value) = default_value {
            definition.push_str(&format!(" DEFAULT {default_value}"));
        }
        definitions.push(definition);
        names.push(name);
    }
    if !needs_relax {
        return Ok(());
    }
    let columns = names.join(", ");
    let sql = format!(
        "BEGIN;
        CREATE TABLE delegate_record_next (
            {}
        );
        INSERT INTO delegate_record_next ({columns}) SELECT {columns} FROM delegate_record;
        DROP TABLE delegate_record;
        ALTER TABLE delegate_record_next RENAME TO delegate_record;
        COMMIT;",
        definitions.join(",\n            ")
    );
    conn.execute_batch(&sql)
        .map_err(|err| format!("放宽委托表旧部门列约束失败: {err}"))?;
    runtime_log_info(format!("[委托] 完成，任务=放宽旧部门列约束"));
    Ok(())
}

fn delegate_store_migrate_snapshot_columns(conn: &Connection) -> Result<(), String> {
    let columns = delegate_store_table_columns(conn)?;
    let mut sql = Vec::<String>::new();
    if !columns.contains("snapshot_conversation_id") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_conversation_id TEXT NOT NULL DEFAULT ''"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_updated_at") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_updated_at TEXT NOT NULL DEFAULT ''"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_archived_at") {
        sql.push("ALTER TABLE delegate_record ADD COLUMN snapshot_archived_at TEXT".to_string());
    }
    if !columns.contains("snapshot_last_message_at") {
        sql.push("ALTER TABLE delegate_record ADD COLUMN snapshot_last_message_at TEXT".to_string());
    }
    if !columns.contains("snapshot_message_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_message_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_step_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_step_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_tool_call_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_tool_call_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_last_tool_name") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_last_tool_name TEXT NOT NULL DEFAULT ''"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_input_token_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_input_token_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_output_token_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_output_token_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_cache_read_token_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_cache_read_token_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_cache_write_token_count") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_cache_write_token_count INTEGER NOT NULL DEFAULT 0"
                .to_string(),
        );
    }
    if !columns.contains("snapshot_cumulative_usage_json") {
        sql.push(
            "ALTER TABLE delegate_record ADD COLUMN snapshot_cumulative_usage_json TEXT NOT NULL DEFAULT ''"
                .to_string(),
        );
    }
    sql.push(
        "CREATE INDEX IF NOT EXISTS idx_delegate_record_snapshot_updated_at ON delegate_record(snapshot_updated_at DESC)"
            .to_string(),
    );
    sql.push(
        "CREATE INDEX IF NOT EXISTS idx_delegate_record_snapshot_root_updated_at ON delegate_record(conversation_id, snapshot_updated_at DESC)"
            .to_string(),
    );
    if sql.is_empty() {
        return Ok(());
    }
    conn.execute_batch(&format!("BEGIN;\n{};\nCOMMIT;", sql.join(";\n")))
        .map_err(|err| format!("扩展委托快照字段失败: {err}"))?;
    Ok(())
}

fn delegate_call_stack_to_json(items: &[String]) -> Result<String, String> {
    serde_json::to_string(items).map_err(|err| format!("序列化委托调用栈失败: {err}"))
}

fn delegate_call_stack_from_json(raw: &str) -> Vec<String> {
    match serde_json::from_str(raw) {
        Ok(items) => items,
        Err(err) => {
            runtime_log_error(format!(
                "[委托] 解析调用栈失败，raw={}, error={}",
                raw,
                err
            ));
            Vec::new()
        }
    }
}

fn delegate_row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<DelegateEntry> {
    Ok(DelegateEntry {
        delegate_id: row.get("delegate_id")?,
        kind: row.get("kind")?,
        conversation_id: row.get("conversation_id")?,
        parent_delegate_id: row.get("parent_delegate_id")?,
        source_agent_id: row.get("source_agent_id")?,
        target_agent_id: row.get("target_agent_id")?,
        title: row.get("title")?,
        why: row.get("why")?,
        goal: row.get("goal")?,
        todo: row.get("todo")?,
        notify_assistant_when_done: row.get::<_, i64>("notify_assistant_when_done")? != 0,
        call_stack: delegate_call_stack_from_json(&row.get::<_, String>("call_stack_json")?),
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        delivered_at: row.get("delivered_at")?,
        completed_at: row.get("completed_at")?,
    })
}

fn delegate_snapshot_row_to_entry(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DelegateConversationSnapshot> {
    let cumulative_usage_json = row.get::<_, String>("cumulative_usage_json")?;
    let cumulative_usage = if cumulative_usage_json.trim().is_empty() {
        ConversationCumulativeUsage {
            input_tokens: row.get::<_, i64>("input_token_count")? as u64,
            output_tokens: row.get::<_, i64>("output_token_count")? as u64,
            cache_read_tokens: row.get::<_, i64>("cache_read_token_count")? as u64,
            cache_write_tokens: row.get::<_, i64>("cache_write_token_count")? as u64,
            ..ConversationCumulativeUsage::default()
        }
    } else {
        serde_json::from_str::<ConversationCumulativeUsage>(&cumulative_usage_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                cumulative_usage_json.len(),
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })?
    };
    Ok(DelegateConversationSnapshot {
        delegate_id: row.get("delegate_id")?,
        kind: row.get("kind")?,
        conversation_id: row.get("conversation_id")?,
        root_conversation_id: row.get("root_conversation_id")?,
        title: row.get("title")?,
        why: row.get("why")?,
        goal: row.get("goal")?,
        todo: row.get("todo")?,
        target_agent_id: row.get("target_agent_id")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
        archived_at: row.get("archived_at")?,
        last_message_at: row.get("last_message_at")?,
        message_count: row.get::<_, i64>("message_count")? as usize,
        step_count: row.get::<_, i64>("step_count")? as usize,
        tool_call_count: row.get::<_, i64>("tool_call_count")? as usize,
        last_tool_name: row.get("last_tool_name")?,
        cumulative_usage,
    })
}

fn delegate_snapshot_stats_from_conversation(
    conversation: &Conversation,
) -> (usize, usize, String, ConversationCumulativeUsage) {
    let cumulative_usage = if conversation.cumulative_usage.is_empty() {
        ConversationCumulativeUsage {
            input_tokens: delegate_snapshot_token_count(&conversation.messages),
            ..ConversationCumulativeUsage::default()
        }
    } else {
        conversation.cumulative_usage.clone()
    };
    let mut request_count = 0usize;
    let mut tool_call_count = 0usize;
    let mut last_tool_name = String::new();
    for message in &conversation.messages {
        if message.role != "assistant" || delegate_snapshot_compaction_kind(message).is_some() {
            continue;
        }
        let events = normalize_message_tool_history_events(message, MessageToolHistoryView::Display);
        let assistant_tool_request_count = events
            .iter()
            .filter(|event| event.role == "assistant")
            .count();
        let mut assistant_tool_call_count = 0usize;
        let mut assistant_last_tool_name = String::new();
        for event in &events {
            if event.role != "assistant" {
                continue;
            }
            for call in &event.tool_calls {
                assistant_tool_call_count = assistant_tool_call_count.saturating_add(1);
                if let Some(name) = call
                    .tool_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    assistant_last_tool_name = name.to_string();
                }
            }
        }
        let has_final_text = delegate_snapshot_text_message_has_content(message);
        if assistant_tool_request_count == 0 || has_final_text {
            request_count = request_count.saturating_add(1);
        }
        request_count = request_count.saturating_add(assistant_tool_request_count);
        tool_call_count = tool_call_count.saturating_add(assistant_tool_call_count);
        if !assistant_last_tool_name.is_empty() {
            last_tool_name = assistant_last_tool_name;
        }
    }
    (request_count, tool_call_count, last_tool_name, cumulative_usage)
}

fn delegate_snapshot_effective_prompt_tokens(message: &ChatMessage) -> Option<u64> {
    message
        .provider_meta
        .as_ref()?
        .get("effectivePromptTokens")?
        .as_u64()
}

fn delegate_snapshot_compaction_kind(message: &ChatMessage) -> Option<String> {
    let kind = message
        .provider_meta
        .as_ref()?
        .get("message_meta")
        .or_else(|| message.provider_meta.as_ref()?.get("messageMeta"))?
        .get("kind")?
        .as_str()?
        .trim();
    match kind {
        "context_compaction" => Some("context_compaction".to_string()),
        "summary_context_seed" => Some("summary_context_seed".to_string()),
        _ => None,
    }
}

fn delegate_snapshot_text_message_has_content(message: &ChatMessage) -> bool {
    !render_prompt_message_text(message).trim().is_empty()
        || message.extra_text_blocks.iter().any(|item| !item.trim().is_empty())
}

fn delegate_snapshot_token_count(messages: &[ChatMessage]) -> u64 {
    let mut total = 0u64;
    let mut latest_segment_usage = None::<u64>;
    for message in messages {
        if let Some(value) = delegate_snapshot_effective_prompt_tokens(message) {
            latest_segment_usage = Some(value);
        }
        if delegate_snapshot_compaction_kind(message).is_some() {
            if let Some(value) = latest_segment_usage.take() {
                total = total.saturating_add(value);
            }
        }
    }
    if let Some(value) = latest_segment_usage {
        total = total.saturating_add(value);
    }
    total
}

fn delegate_snapshot_from_entry_and_conversation(
    entry: &DelegateEntry,
    conversation: &Conversation,
) -> DelegateConversationSnapshot {
    let delegate_id = conversation
        .delegate_id
        .clone()
        .unwrap_or_else(|| conversation.id.clone());
    let (step_count, tool_call_count, last_tool_name, cumulative_usage) =
        delegate_snapshot_stats_from_conversation(conversation);
    DelegateConversationSnapshot {
        delegate_id: delegate_id.clone(),
        kind: entry.kind.clone(),
        conversation_id: conversation.id.clone(),
        root_conversation_id: conversation
            .root_conversation_id
            .clone()
            .unwrap_or_else(|| entry.conversation_id.clone()),
        title: if entry.title.trim().is_empty() {
            conversation.title.clone()
        } else {
            entry.title.clone()
        },
        why: entry.why.clone(),
        goal: entry.goal.clone(),
        todo: entry.todo.clone(),
        target_agent_id: if entry.target_agent_id.trim().is_empty() {
            conversation.agent_id.clone()
        } else {
            entry.target_agent_id.clone()
        },
        status: entry.status.clone(),
        created_at: entry.created_at.clone(),
        updated_at: conversation.updated_at.clone(),
        completed_at: entry.completed_at.clone(),
        archived_at: conversation.archived_at.clone(),
        last_message_at: conversation.messages.last().map(|message| message.created_at.clone()),
        message_count: conversation.messages.len(),
        step_count,
        tool_call_count,
        last_tool_name,
        cumulative_usage,
    }
}

fn delegate_snapshot_from_entry(
    entry: &DelegateEntry,
    existing: Option<&DelegateConversationSnapshot>,
) -> DelegateConversationSnapshot {
    DelegateConversationSnapshot {
        delegate_id: entry.delegate_id.clone(),
        kind: entry.kind.clone(),
        conversation_id: existing
            .map(|value| value.conversation_id.clone())
            .unwrap_or_else(|| entry.delegate_id.clone()),
        root_conversation_id: entry.conversation_id.clone(),
        title: entry.title.clone(),
        why: entry.why.clone(),
        goal: entry.goal.clone(),
        todo: entry.todo.clone(),
        target_agent_id: entry.target_agent_id.clone(),
        status: entry.status.clone(),
        created_at: entry.created_at.clone(),
        updated_at: entry.updated_at.clone(),
        completed_at: entry.completed_at.clone(),
        archived_at: existing.and_then(|value| value.archived_at.clone()),
        last_message_at: existing.and_then(|value| value.last_message_at.clone()),
        message_count: existing.map(|value| value.message_count).unwrap_or(0),
        step_count: existing.map(|value| value.step_count).unwrap_or(0),
        tool_call_count: existing.map(|value| value.tool_call_count).unwrap_or(0),
        last_tool_name: existing
            .map(|value| value.last_tool_name.clone())
            .unwrap_or_default(),
        cumulative_usage: existing
            .map(|value| value.cumulative_usage.clone())
            .unwrap_or_default(),
    }
}

fn delegate_snapshot_store_read(
    data_path: &PathBuf,
    delegate_id: &str,
) -> Result<Option<DelegateConversationSnapshot>, String> {
    let conn = delegate_store_open(data_path)?;
    conn.query_row(
        "SELECT
            delegate_id,
            kind,
            snapshot_conversation_id AS conversation_id,
            conversation_id AS root_conversation_id,
            title,
            why,
            goal,
            todo,
            target_agent_id,
            status,
            created_at,
            snapshot_updated_at AS updated_at,
            completed_at,
            snapshot_archived_at AS archived_at,
            snapshot_last_message_at AS last_message_at,
            snapshot_message_count AS message_count,
            snapshot_step_count AS step_count,
            snapshot_tool_call_count AS tool_call_count,
            snapshot_last_tool_name AS last_tool_name,
            snapshot_input_token_count AS input_token_count,
            snapshot_output_token_count AS output_token_count,
            snapshot_cache_read_token_count AS cache_read_token_count,
            snapshot_cache_write_token_count AS cache_write_token_count,
            snapshot_cumulative_usage_json AS cumulative_usage_json
        FROM delegate_record
        WHERE delegate_id=?1 AND snapshot_conversation_id!=''",
        params![delegate_id.trim()],
        delegate_snapshot_row_to_entry,
    )
    .optional()
    .map_err(|err| format!("读取委托快照失败: {err}"))
}

fn delegate_snapshot_store_list_from_db(
    data_path: &PathBuf,
) -> Result<Vec<DelegateConversationSnapshot>, String> {
    let conn = delegate_store_open(data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT
                delegate_id,
                kind,
                snapshot_conversation_id AS conversation_id,
                conversation_id AS root_conversation_id,
                title,
                why,
                goal,
                todo,
                target_agent_id,
                status,
                created_at,
                snapshot_updated_at AS updated_at,
                completed_at,
                snapshot_archived_at AS archived_at,
                snapshot_last_message_at AS last_message_at,
                snapshot_message_count AS message_count,
                snapshot_step_count AS step_count,
                snapshot_tool_call_count AS tool_call_count,
                snapshot_last_tool_name AS last_tool_name,
                snapshot_input_token_count AS input_token_count,
                snapshot_output_token_count AS output_token_count,
                snapshot_cache_read_token_count AS cache_read_token_count,
                snapshot_cache_write_token_count AS cache_write_token_count,
                snapshot_cumulative_usage_json AS cumulative_usage_json
            FROM delegate_record
            WHERE snapshot_conversation_id!=''
            ORDER BY COALESCE(snapshot_archived_at, snapshot_last_message_at, snapshot_updated_at) DESC,
                     snapshot_updated_at DESC,
                     delegate_id DESC",
        )
        .map_err(|err| format!("准备读取委托快照列表失败: {err}"))?;
    let rows = stmt
        .query_map([], delegate_snapshot_row_to_entry)
        .map_err(|err| format!("读取委托快照列表失败: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析委托快照列表失败: {err}"))
}

fn delegate_snapshot_store_is_empty(data_path: &PathBuf) -> Result<bool, String> {
    let conn = delegate_store_open(data_path)?;
    let count = conn
        .query_row(
            "SELECT COUNT(*) FROM delegate_record WHERE snapshot_conversation_id!=''",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("统计委托快照失败: {err}"))?;
    Ok(count == 0)
}

fn delegate_snapshot_store_upsert_db(
    data_path: &PathBuf,
    snapshot: &DelegateConversationSnapshot,
) -> Result<(), String> {
    let conn = delegate_store_open(data_path)?;
    let cumulative_usage_json = serde_json::to_string(&snapshot.cumulative_usage)
        .map_err(|err| format!("序列化委托快照累计用量失败，delegate_id={}，error={err}", snapshot.delegate_id))?;
    let affected = conn.execute(
        "UPDATE delegate_record SET
            snapshot_conversation_id=?2,
            snapshot_updated_at=?3,
            snapshot_archived_at=?4,
            snapshot_last_message_at=?5,
            snapshot_message_count=?6,
            snapshot_step_count=?7,
            snapshot_tool_call_count=?8,
            snapshot_last_tool_name=?9,
            snapshot_input_token_count=?10,
            snapshot_output_token_count=?11,
            snapshot_cache_read_token_count=?12,
            snapshot_cache_write_token_count=?13,
            snapshot_cumulative_usage_json=?14
        WHERE delegate_id=?1",
        params![
            snapshot.delegate_id,
            snapshot.conversation_id,
            snapshot.updated_at,
            snapshot.archived_at,
            snapshot.last_message_at,
            snapshot.message_count as i64,
            snapshot.step_count as i64,
            snapshot.tool_call_count as i64,
            snapshot.last_tool_name,
            snapshot.cumulative_usage.input_tokens as i64,
            snapshot.cumulative_usage.output_tokens as i64,
            snapshot.cumulative_usage.cache_read_tokens as i64,
            snapshot.cumulative_usage.cache_write_tokens as i64,
            cumulative_usage_json,
        ],
    )
    .map_err(|err| format!("写入委托快照失败，delegate_id={}，error={err}", snapshot.delegate_id))?;
    if affected == 0 {
        return Err(format!(
            "写入委托快照失败，delegate_id={}，reason=delegate_record_missing",
            snapshot.delegate_id
        ));
    }
    Ok(())
}

fn delegate_snapshot_store_delete_db(
    data_path: &PathBuf,
    delegate_id: &str,
) -> Result<bool, String> {
    let conn = delegate_store_open(data_path)?;
    let affected = conn.execute(
        "UPDATE delegate_record SET
            snapshot_conversation_id='',
            snapshot_updated_at='',
            snapshot_archived_at=NULL,
            snapshot_last_message_at=NULL,
            snapshot_message_count=0,
            snapshot_step_count=0,
            snapshot_tool_call_count=0,
            snapshot_last_tool_name='',
            snapshot_input_token_count=0,
            snapshot_output_token_count=0,
            snapshot_cache_read_token_count=0,
            snapshot_cache_write_token_count=0,
            snapshot_cumulative_usage_json=''
        WHERE delegate_id=?1 AND snapshot_conversation_id!=''",
        params![delegate_id.trim()],
    )
    .map_err(|err| format!("删除委托快照失败，delegate_id={}，error={err}", delegate_id.trim()))?;
    Ok(affected > 0)
}

fn delegate_snapshot_cache_list(
    data_path: &PathBuf,
) -> Result<Vec<DelegateConversationSnapshot>, String> {
    delegate_snapshot_cache_ensure_loaded(data_path)?;
    let cache = delegate_snapshot_cache_store()
        .lock()
        .map_err(|_| "Failed to lock delegate snapshot cache".to_string())?;
    let Some(snapshot_cache) = cache.get(&delegate_snapshot_cache_key(data_path)) else {
        return Ok(Vec::new());
    };
    Ok(snapshot_cache
        .ordered_delegate_ids
        .iter()
        .filter_map(|delegate_id| snapshot_cache.by_delegate_id.get(delegate_id).cloned())
        .collect())
}

fn delegate_snapshot_cache_list_by_root(
    data_path: &PathBuf,
    root_conversation_id: &str,
) -> Result<Vec<DelegateConversationSnapshot>, String> {
    delegate_snapshot_cache_ensure_loaded(data_path)?;
    let cache = delegate_snapshot_cache_store()
        .lock()
        .map_err(|_| "Failed to lock delegate snapshot cache".to_string())?;
    let Some(snapshot_cache) = cache.get(&delegate_snapshot_cache_key(data_path)) else {
        return Ok(Vec::new());
    };
    let Some(delegate_ids) = snapshot_cache
        .by_root_conversation_id
        .get(root_conversation_id.trim())
    else {
        return Ok(Vec::new());
    };
    Ok(delegate_ids
        .iter()
        .filter_map(|delegate_id| snapshot_cache.by_delegate_id.get(delegate_id).cloned())
        .collect())
}

fn delegate_snapshot_cache_get(
    data_path: &PathBuf,
    delegate_id: &str,
) -> Result<Option<DelegateConversationSnapshot>, String> {
    delegate_snapshot_cache_ensure_loaded(data_path)?;
    let cache = delegate_snapshot_cache_store()
        .lock()
        .map_err(|_| "Failed to lock delegate snapshot cache".to_string())?;
    Ok(cache
        .get(&delegate_snapshot_cache_key(data_path))
        .and_then(|snapshot_cache| snapshot_cache.by_delegate_id.get(delegate_id.trim()).cloned()))
}

fn delegate_snapshot_cache_write(
    data_path: &PathBuf,
    snapshot: DelegateConversationSnapshot,
) -> Result<(), String> {
    delegate_snapshot_store_upsert_db(data_path, &snapshot)?;
    let mut cache = delegate_snapshot_cache_store()
        .lock()
        .map_err(|_| "Failed to lock delegate snapshot cache".to_string())?;
    let key = delegate_snapshot_cache_key(data_path);
    if let Some(snapshot_cache) = cache.get_mut(&key) {
        // 启动首次读取前允许缓存尚未装载；写路径只能更新已装载缓存，绝不能隐式触发真相层重建。
        snapshot_cache
            .by_delegate_id
            .insert(snapshot.delegate_id.clone(), snapshot);
        *snapshot_cache = delegate_snapshot_cache_from_map(snapshot_cache.by_delegate_id.clone());
    }
    Ok(())
}

fn delegate_snapshot_cache_delete(
    data_path: &PathBuf,
    delegate_id: &str,
) -> Result<bool, String> {
    let deleted = delegate_snapshot_store_delete_db(data_path, delegate_id)?;
    let mut cache = delegate_snapshot_cache_store()
        .lock()
        .map_err(|_| "Failed to lock delegate snapshot cache".to_string())?;
    let key = delegate_snapshot_cache_key(data_path);
    if let Some(snapshot_cache) = cache.get_mut(&key) {
        // 删除路径与写路径同理：只维护已装载缓存，不在运行期补触发首次重建。
        snapshot_cache.by_delegate_id.remove(delegate_id.trim());
        *snapshot_cache = delegate_snapshot_cache_from_map(snapshot_cache.by_delegate_id.clone());
    }
    Ok(deleted)
}

fn delegate_snapshot_cache_ensure_loaded(data_path: &PathBuf) -> Result<(), String> {
    let key = delegate_snapshot_cache_key(data_path);
    let mut cache = delegate_snapshot_cache_store()
        .lock()
        .map_err(|_| "Failed to lock delegate snapshot cache".to_string())?;
    if cache.contains_key(&key) {
        return Ok(());
    }
    runtime_log_debug("[委托快照] 首次读取检查开始".to_string());
    // 首次读取是唯一允许触发空表重建的入口；普通写路径、错误路径都不允许绕到这里补扫真相层。
    // 这里故意只看当前目录型正文仓库。旧格式是否存在、何时迁移，都只能由迁移服务单独负责，
    // 运行期列表与快照缓存绝不能为了“看见旧数据”再去碰旧格式。
    if delegate_snapshot_store_is_empty(data_path)? {
        delegate_snapshot_store_rebuild_from_truth_if_empty(data_path)?;
    }
    let snapshots = delegate_snapshot_store_list_from_db(data_path)?;
    let snapshot_cache = delegate_snapshot_cache_build(snapshots);
    cache.insert(key, snapshot_cache);
    runtime_log_debug("[委托快照] 首次读取检查完成".to_string());
    Ok(())
}

fn delegate_snapshot_store_rebuild_from_truth_if_empty(data_path: &PathBuf) -> Result<(), String> {
    if !delegate_snapshot_store_is_empty(data_path)? {
        return Ok(());
    }
    runtime_log_debug("[委托快照] 快照表为空，开始重建".to_string());
    let conversations = delegate_conversation_store_list(data_path)?;
    let conversation_count = conversations.len();
    for conversation in conversations {
        let delegate_id = conversation
            .delegate_id
            .clone()
            .unwrap_or_else(|| conversation.id.clone());
        let entry = delegate_store_get_delegate(data_path, &delegate_id)?;
        let snapshot = delegate_snapshot_from_entry_and_conversation(&entry, &conversation);
        delegate_snapshot_store_upsert_db(data_path, &snapshot)?;
    }
    runtime_log_debug(format!(
        "[委托快照] 快照表重建完成，conversation_count={}",
        conversation_count
    ));
    Ok(())
}

fn delegate_snapshot_store_sync_from_conversation(
    data_path: &PathBuf,
    conversation: &Conversation,
) -> Result<(), String> {
    let delegate_id = conversation
        .delegate_id
        .clone()
        .unwrap_or_else(|| conversation.id.clone());
    let entry = delegate_store_get_delegate(data_path, &delegate_id)?;
    let snapshot = delegate_snapshot_from_entry_and_conversation(&entry, conversation);
    delegate_snapshot_cache_write(data_path, snapshot)
}

fn delegate_snapshot_store_sync_from_entry(
    data_path: &PathBuf,
    entry: &DelegateEntry,
) -> Result<(), String> {
    let existing = delegate_snapshot_store_read(data_path, &entry.delegate_id)?;
    let snapshot = delegate_snapshot_from_entry(entry, existing.as_ref());
    delegate_snapshot_cache_write(data_path, snapshot)
}

fn delegate_store_create_delegate(
    data_path: &PathBuf,
    input: &DelegateCreateInput,
) -> Result<DelegateEntry, String> {
    if input.conversation_id.trim().is_empty() {
        return Err("delegate.conversationId 不能为空".to_string());
    }
    if input.source_agent_id.trim().is_empty() || input.target_agent_id.trim().is_empty() {
        return Err("委托 source/target agent 不能为空".to_string());
    }
    let title = input.title.trim();
    if title.is_empty() {
        return Err("delegate.title 不能为空".to_string());
    }
    let goal = input.goal.trim();
    if goal.is_empty() {
        return Err("delegate.goal 不能为空".to_string());
    }
    let conn = delegate_store_open(data_path)?;
    let delegate_id = format!("delegate-{}", Uuid::new_v4());
    let now = now_iso();
    conn.execute(
        "INSERT INTO delegate_record (
            delegate_id, kind, conversation_id, parent_delegate_id,
            source_agent_id, target_agent_id,
            title, why, goal, todo,
            notify_assistant_when_done, call_stack_json, status, created_at, updated_at, delivered_at, completed_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL)",
        params![
            delegate_id,
            input.kind.trim(),
            input.conversation_id.trim(),
            input.parent_delegate_id.as_deref(),
            input.source_agent_id.trim(),
            input.target_agent_id.trim(),
            title,
            input.why.trim(),
            goal,
            input.todo.trim(),
            if input.notify_assistant_when_done { 1 } else { 0 },
            delegate_call_stack_to_json(&input.call_stack)?,
            DELEGATE_STATUS_DELIVERED,
            now,
            now,
            now,
        ],
    )
    .map_err(|err| format!("创建委托记录失败: {err}"))?;
    let entry = conn.query_row(
        "SELECT * FROM delegate_record WHERE delegate_id = ?1",
        params![delegate_id],
        delegate_row_to_entry,
    )
    .map_err(|err| format!("读取委托记录失败: {err}"))?;
    delegate_snapshot_store_sync_from_entry(data_path, &entry)?;
    Ok(entry)
}

fn delegate_store_get_delegate(data_path: &PathBuf, delegate_id: &str) -> Result<DelegateEntry, String> {
    let conn = delegate_store_open(data_path)?;
    conn.query_row(
        "SELECT * FROM delegate_record WHERE delegate_id = ?1",
        params![delegate_id.trim()],
        delegate_row_to_entry,
    )
    .map_err(|err| format!("读取委托记录失败: {err}"))
}

fn delegate_store_delete_terminal_delegate(data_path: &PathBuf, delegate_id: &str) -> Result<bool, String> {
    let conn = delegate_store_open(data_path)?;
    let affected = conn.execute(
        "DELETE FROM delegate_record WHERE delegate_id = ?1 AND status IN (?2, ?3)",
        params![delegate_id.trim(), DELEGATE_STATUS_COMPLETED, DELEGATE_STATUS_FAILED],
    )
    .map_err(|err| format!("删除已终结委托记录失败，delegate_id={}，error={err}", delegate_id.trim()))?;
    Ok(affected > 0)
}

fn delegate_store_update_status(
    data_path: &PathBuf,
    delegate_id: &str,
    status: &str,
) -> Result<DelegateEntry, String> {
    let conn = delegate_store_open(data_path)?;
    let now = now_iso();
    let completed_at = if status == DELEGATE_STATUS_COMPLETED || status == DELEGATE_STATUS_FAILED {
        Some(now.clone())
    } else {
        None
    };
    let affected = conn.execute(
        "UPDATE delegate_record
         SET status = ?2, updated_at = ?3, completed_at = COALESCE(?4, completed_at)
         WHERE delegate_id = ?1",
        params![delegate_id.trim(), status.trim(), now, completed_at],
    )
    .map_err(|err| format!("更新委托状态失败: {err}"))?;
    if affected == 0 {
        return Err(format!(
            "更新委托状态失败：未找到委托 {}",
            delegate_id.trim()
        ));
    }
    runtime_log_info(format!(
        "[委托] 更新状态成功，delegate_id={}, status={}, completed_at={}",
        delegate_id.trim(),
        status.trim(),
        completed_at.as_deref().unwrap_or("-")
    ));
    let entry = delegate_store_get_delegate(data_path, delegate_id)?;
    delegate_snapshot_store_sync_from_entry(data_path, &entry)?;
    Ok(entry)
}

fn delegate_store_interrupt_unfinished_remote_replies(data_path: &PathBuf) -> Result<Vec<String>, String> {
    let conn = delegate_store_open(data_path)?;
    let mut statement = conn
        .prepare(
            "SELECT delegate_id FROM delegate_record
             WHERE kind IN ('remote_im_reply', 'remote_im_departure_reflection') AND status = ?1",
        )
        .map_err(|err| format!("读取未完成远程委托失败: {err}"))?;
    let delegate_ids = statement
        .query_map(params![DELEGATE_STATUS_DELIVERED], |row| row.get::<_, String>(0))
        .map_err(|err| format!("读取未完成远程委托失败: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("读取未完成远程委托失败: {err}"))?;
    drop(statement);
    if delegate_ids.is_empty() {
        return Ok(delegate_ids);
    }
    let now = now_iso();
    conn.execute(
            "UPDATE delegate_record
             SET status = ?1, updated_at = ?2, completed_at = COALESCE(completed_at, ?2)
             WHERE kind IN ('remote_im_reply', 'remote_im_departure_reflection') AND status = ?3",
            params![DELEGATE_STATUS_FAILED, now, DELEGATE_STATUS_DELIVERED],
        )
        .map_err(|err| format!("恢复远程委托状态失败: {err}"))?;
    for delegate_id in &delegate_ids {
        let entry = delegate_store_get_delegate(data_path, delegate_id)?;
        delegate_snapshot_store_sync_from_entry(data_path, &entry)?;
    }
    Ok(delegate_ids)
}

#[cfg(test)]
mod delegate_store_tests {
    use super::*;
    use uuid::Uuid;

    fn test_delegate_input() -> DelegateCreateInput {
        DelegateCreateInput {
            kind: "delegate".to_string(),
            conversation_id: "root-conversation".to_string(),
            parent_delegate_id: None,
            source_agent_id: "source-agent".to_string(),
            target_agent_id: DEFAULT_AGENT_ID.to_string(),
            title: "委托标题".to_string(),
            why: "委托原因".to_string(),
            goal: "委托目标".to_string(),
            todo: "委托待办".to_string(),
            notify_assistant_when_done: false,
            call_stack: Vec::new(),
        }
    }

    fn test_delegate_conversation(entry: &DelegateEntry) -> Conversation {
        let mut conversation = build_conversation_record(
            "",
            &entry.target_agent_id,
            &entry.title,
            CONVERSATION_KIND_DELEGATE,
            Some(entry.conversation_id.clone()),
            Some(entry.delegate_id.clone()),
        );
        conversation.id = entry.delegate_id.clone();
        conversation.created_at = entry.created_at.clone();
        conversation.updated_at = entry.updated_at.clone();
        conversation.messages = vec![ChatMessage {
            id: "message-1".to_string(),
            role: "assistant".to_string(),
            created_at: entry.updated_at.clone(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "hello".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }];
        conversation
    }

    #[test]
    fn delegate_store_migrate_why_goal_todo_should_prefer_new_fields_and_drop_legacy_columns() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "CREATE TABLE delegate_record (
                delegate_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                parent_delegate_id TEXT,
                source_department_id TEXT NOT NULL,
                target_department_id TEXT NOT NULL,
                source_agent_id TEXT NOT NULL,
                target_agent_id TEXT NOT NULL,
                title TEXT NOT NULL,
                why TEXT NOT NULL,
                goal TEXT NOT NULL,
                todo TEXT NOT NULL,
                background TEXT NOT NULL,
                question TEXT NOT NULL,
                focus TEXT NOT NULL,
                notify_assistant_when_done INTEGER NOT NULL,
                call_stack_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                delivered_at TEXT,
                completed_at TEXT
            );
            INSERT INTO delegate_record (
                delegate_id, kind, conversation_id, parent_delegate_id,
                source_department_id, target_department_id, source_agent_id, target_agent_id,
                title, why, goal, todo, background, question, focus,
                notify_assistant_when_done, call_stack_json, status, created_at, updated_at, delivered_at, completed_at
            ) VALUES (
                'delegate-a', 'delegate', 'conversation-a', NULL,
                'source-dept', 'target-dept', 'source-agent', 'target-agent',
                'title-a', 'new why', 'new goal', '', 'old why', 'old goal', 'old todo',
                0, '[]', 'delivered', '2026-06-11T00:00:00Z', '2026-06-11T00:00:00Z', NULL, NULL
            );",
        )
        .expect("seed legacy delegate table");

        delegate_store_migrate_why_goal_todo(&conn).expect("migrate delegate fields");

        let columns = delegate_store_table_columns(&conn).expect("read migrated columns");
        assert!(columns.contains("why"));
        assert!(columns.contains("goal"));
        assert!(columns.contains("todo"));
        assert!(!columns.contains("background"));
        assert!(!columns.contains("question"));
        assert!(!columns.contains("focus"));

        let (why, goal, todo): (String, String, String) = conn
            .query_row(
                "SELECT why, goal, todo FROM delegate_record WHERE delegate_id = 'delegate-a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read migrated row");
        assert_eq!(why, "new why");
        assert_eq!(goal, "new goal");
        assert_eq!(todo, "old todo");
    }

    #[test]
    fn delegate_store_create_delegate_should_sync_snapshot_after_cache_bootstrap() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-snapshot-create-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let snapshots = delegate_snapshot_cache_list(&data_path).expect("bootstrap empty cache");
        assert!(snapshots.is_empty());

        let entry =
            delegate_store_create_delegate(&data_path, &test_delegate_input()).expect("create delegate");
        let snapshot = delegate_snapshot_store_read(&data_path, &entry.delegate_id)
            .expect("read snapshot")
            .expect("snapshot exists");

        assert_eq!(snapshot.delegate_id, entry.delegate_id);
        assert_eq!(snapshot.root_conversation_id, entry.conversation_id);
        assert_eq!(snapshot.target_agent_id, entry.target_agent_id);
        assert_eq!(snapshot.status, entry.status);
        assert_eq!(snapshot.message_count, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delegate_store_create_delegate_should_bootstrap_and_sync_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-snapshot-create-bootstrap-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");

        let entry =
            delegate_store_create_delegate(&data_path, &test_delegate_input()).expect("create delegate");
        let snapshot = delegate_snapshot_store_read(&data_path, &entry.delegate_id)
            .expect("read snapshot")
            .expect("snapshot exists");

        assert_eq!(snapshot.delegate_id, entry.delegate_id);
        assert_eq!(snapshot.status, DELEGATE_STATUS_DELIVERED);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unfinished_remote_delegate_recovery_should_include_departure_reflection() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-remote-recovery-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let mut reply_input = test_delegate_input();
        reply_input.kind = "remote_im_reply".to_string();
        let reply = delegate_store_create_delegate(&data_path, &reply_input)
            .expect("create remote reply");
        let mut reflection_input = test_delegate_input();
        reflection_input.kind = "remote_im_departure_reflection".to_string();
        let reflection = delegate_store_create_delegate(&data_path, &reflection_input)
            .expect("create departure reflection");

        let interrupted = delegate_store_interrupt_unfinished_remote_replies(&data_path)
            .expect("interrupt unfinished remote delegates");

        assert!(interrupted.contains(&reply.delegate_id));
        assert!(interrupted.contains(&reflection.delegate_id));
        assert_eq!(
            delegate_store_get_delegate(&data_path, &reflection.delegate_id)
                .expect("read reflection")
                .status,
            DELEGATE_STATUS_FAILED
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delegate_conversation_write_should_not_bootstrap_snapshot_cache() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-snapshot-no-cache-bootstrap-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let entry =
            delegate_store_create_delegate(&data_path, &test_delegate_input()).expect("create delegate");
        let conversation = test_delegate_conversation(&entry);

        delegate_conversation_store_write(&data_path, &conversation).expect("write conversation");

        let cache_key = delegate_snapshot_cache_key(&data_path);
        let cache = delegate_snapshot_cache_store()
            .lock()
            .expect("lock snapshot cache");
        assert!(!cache.contains_key(&cache_key));
        drop(cache);

        let snapshots = delegate_snapshot_cache_list(&data_path).expect("load snapshot cache");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].delegate_id, entry.delegate_id);
        assert_eq!(snapshots[0].message_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delegate_conversation_delete_should_remove_empty_delegate_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-snapshot-delete-empty-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let entry =
            delegate_store_create_delegate(&data_path, &test_delegate_input()).expect("create delegate");

        let deleted = delegate_conversation_store_delete(&data_path, &entry.delegate_id)
            .expect("delete empty delegate");
        let snapshot = delegate_snapshot_store_read(&data_path, &entry.delegate_id)
            .expect("read snapshot");

        assert!(deleted);
        assert!(snapshot.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delegate_snapshot_cache_list_should_rebuild_from_truth_when_table_is_empty() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-snapshot-rebuild-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");

        let entry =
            delegate_store_create_delegate(&data_path, &test_delegate_input()).expect("create delegate");
        let conversation = test_delegate_conversation(&entry);
        delegate_conversation_store_write(&data_path, &conversation).expect("write conversation");
        delegate_snapshot_store_delete_db(&data_path, &entry.delegate_id)
            .expect("delete snapshot row");

        let snapshots = delegate_snapshot_cache_list(&data_path).expect("load snapshot cache");

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].delegate_id, entry.delegate_id);
        assert_eq!(snapshots[0].message_count, 1);
        let _ = fs::remove_dir_all(root);
    }
}
