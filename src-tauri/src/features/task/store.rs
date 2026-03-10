fn task_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path).join("task").join(TASK_DB_FILE_NAME)
}

fn task_store_open(data_path: &PathBuf) -> Result<Connection, String> {
    let path = task_store_db_path(data_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Task db path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("Create task dir failed: {err}"))?;
    let conn = Connection::open(&path)
        .map_err(|err| format!("Open task db failed ({}): {err}", path.display()))?;
    task_store_init(&conn)?;
    Ok(conn)
}

fn task_store_init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS task_record (
            task_id TEXT PRIMARY KEY,
            order_index INTEGER NOT NULL,
            title TEXT NOT NULL,
            cause TEXT NOT NULL,
            goal TEXT NOT NULL,
            flow TEXT NOT NULL,
            todos_json TEXT NOT NULL,
            status_summary TEXT NOT NULL,
            completion_state TEXT NOT NULL,
            completion_conclusion TEXT NOT NULL,
            progress_notes_json TEXT NOT NULL,
            stage_key TEXT NOT NULL,
            stage_updated_at TEXT,
            trigger_kind TEXT NOT NULL,
            run_at TEXT,
            every_minutes INTEGER,
            end_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_triggered_at TEXT,
            completed_at TEXT
        );
        CREATE TABLE IF NOT EXISTS task_runtime_state (
            state_key TEXT PRIMARY KEY,
            state_value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS task_run_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            triggered_at TEXT NOT NULL,
            outcome TEXT NOT NULL,
            note TEXT NOT NULL
        );
        COMMIT;",
    )
    .map_err(|err| format!("Init task db failed: {err}"))?;
    let _ = conn.execute(
        "ALTER TABLE task_record ADD COLUMN end_at TEXT",
        [],
    );
    Ok(())
}

fn task_normalize_run_at(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("task.trigger.runAt must not be empty".to_string());
    }
    let parsed = parse_iso(trimmed)
        .ok_or_else(|| "task.trigger.runAt must be RFC3339 with timezone offset, for example 2026-03-10T09:30:00+08:00".to_string())?;
    parsed
        .replace_nanosecond(0)
        .map_err(|err| format!("Normalize task.trigger.runAt failed: {err}"))?
        .format(&Rfc3339)
        .map_err(|err| format!("Format task.trigger.runAt failed: {err}"))
}

fn task_normalize_end_at(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("task.trigger.endAt must not be empty".to_string());
    }
    let parsed = parse_iso(trimmed)
        .ok_or_else(|| "task.trigger.endAt must be RFC3339 with timezone offset, for example 2026-03-10T10:30:00+08:00".to_string())?;
    parsed
        .replace_nanosecond(0)
        .map_err(|err| format!("Normalize task.trigger.endAt failed: {err}"))?
        .format(&Rfc3339)
        .map_err(|err| format!("Format task.trigger.endAt failed: {err}"))
}

fn task_trigger_from_input(input: &TaskTriggerInput) -> Result<TaskTriggerInput, String> {
    let run_at = input.run_at.as_deref().map(str::trim).unwrap_or("");
    let every_minutes = input.every_minutes.unwrap_or(0);
    let end_at = input.end_at.as_deref().map(str::trim).unwrap_or("");
    if run_at.is_empty() {
        if every_minutes > 0 {
            return Err("task.trigger.runAt is required when task.trigger.everyMinutes is set".to_string());
        }
        if !end_at.is_empty() {
            return Err("task.trigger.endAt requires task.trigger.runAt".to_string());
        }
        return Ok(TaskTriggerInput {
            run_at: None,
            every_minutes: None,
            end_at: None,
        });
    }
    let normalized_run_at = task_normalize_run_at(run_at)?;
    if every_minutes == 0 {
        if !end_at.is_empty() {
            return Err("task.trigger.endAt is only supported when task.trigger.everyMinutes is set".to_string());
        }
        return Ok(TaskTriggerInput {
            run_at: Some(normalized_run_at),
            every_minutes: None,
            end_at: None,
        });
    }
    if end_at.is_empty() {
        return Err("task.trigger.endAt is required when task.trigger.everyMinutes is set".to_string());
    }
    let normalized_end_at = task_normalize_end_at(end_at)?;
    let run_dt = parse_iso(&normalized_run_at)
        .ok_or_else(|| "task.trigger.runAt normalization failed".to_string())?;
    let end_dt = parse_iso(&normalized_end_at)
        .ok_or_else(|| "task.trigger.endAt normalization failed".to_string())?;
    if end_dt <= run_dt {
        return Err("task.trigger.endAt must be later than task.trigger.runAt".to_string());
    }
    Ok(TaskTriggerInput {
        run_at: Some(normalized_run_at),
        every_minutes: Some(every_minutes),
        end_at: Some(normalized_end_at),
    })
}

fn task_trigger_kind_from_fields(run_at: Option<&str>, every_minutes: Option<u32>) -> &'static str {
    if run_at.is_none() {
        "immediate"
    } else if every_minutes.unwrap_or(0) > 0 {
        "every"
    } else {
        "start"
    }
}

fn task_completion_state_normalized(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        TASK_STATE_ACTIVE | TASK_STATE_COMPLETED | TASK_STATE_FAILED_COMPLETED => Ok(normalized),
        _ => Err("task.completionState must be active, completed, or failed_completed".to_string()),
    }
}

fn task_list_to_json(items: &[String]) -> Result<String, String> {
    serde_json::to_string(items).map_err(|err| format!("Serialize task todos failed: {err}"))
}

fn task_notes_to_json(items: &[TaskProgressNote]) -> Result<String, String> {
    serde_json::to_string(items).map_err(|err| format!("Serialize task notes failed: {err}"))
}

fn task_list_from_json(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn task_notes_from_json(raw: &str) -> Vec<TaskProgressNote> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn task_append_progress_note(notes: &mut Vec<TaskProgressNote>, note: &str) {
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return;
    }
    notes.push(TaskProgressNote { at: now_iso(), note: trimmed.to_string() });
}

fn task_store_get_runtime_state(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT state_value FROM task_runtime_state WHERE state_key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|err| format!("Read task runtime state failed: {err}"))
}

fn task_store_set_runtime_state(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO task_runtime_state (state_key, state_value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(state_key) DO UPDATE SET state_value = excluded.state_value, updated_at = excluded.updated_at",
        params![key, value, now_iso()],
    )
    .map_err(|err| format!("Write task runtime state failed: {err}"))?;
    Ok(())
}

fn task_compute_next_run_at_raw(
    run_at: Option<&str>,
    every_minutes: Option<u32>,
    end_at: Option<&str>,
    last_triggered_at: Option<&str>,
    completion_state: &str,
) -> Option<String> {
    if completion_state != TASK_STATE_ACTIVE {
        return None;
    }
    if run_at.is_none() {
        return Some(now_iso());
    }
    if every_minutes.unwrap_or(0) == 0 {
        return run_at.map(ToOwned::to_owned);
    }
    let base = if let Some(last) = last_triggered_at.and_then(parse_iso) {
        last
    } else if let Some(start) = run_at.and_then(parse_iso) {
        start
    } else {
        return None;
    };
    let every = i64::from(every_minutes.unwrap_or(0));
    if every <= 0 {
        return None;
    }
    let next = base + time::Duration::minutes(every);
    if let Some(end_dt) = end_at.and_then(parse_iso) {
        if next > end_dt {
            return None;
        }
    }
    next.format(&Rfc3339).ok()
}

fn task_row_to_entry(row: &rusqlite::Row<'_>, current_tracked_task_id: Option<&str>) -> rusqlite::Result<TaskEntry> {
    let task_id: String = row.get("task_id")?;
    let completion_state: String = row.get("completion_state")?;
    let run_at: Option<String> = row.get("run_at")?;
    let every_minutes: Option<u32> = row.get("every_minutes")?;
    let end_at: Option<String> = row.get("end_at")?;
    let last_triggered_at: Option<String> = row.get("last_triggered_at")?;
    Ok(TaskEntry {
        task_id: task_id.clone(),
        order_index: row.get("order_index")?,
        title: row.get("title")?,
        cause: row.get("cause")?,
        goal: row.get("goal")?,
        flow: row.get("flow")?,
        todos: task_list_from_json(&row.get::<_, String>("todos_json")?),
        status_summary: row.get("status_summary")?,
        completion_state: completion_state.clone(),
        completion_conclusion: row.get("completion_conclusion")?,
        progress_notes: task_notes_from_json(&row.get::<_, String>("progress_notes_json")?),
        stage_key: row.get("stage_key")?,
        stage_updated_at: row.get("stage_updated_at")?,
        trigger: TaskTrigger {
            run_at: run_at.clone(),
            every_minutes,
            end_at: end_at.clone(),
            next_run_at: task_compute_next_run_at_raw(
                run_at.as_deref(),
                every_minutes,
                end_at.as_deref(),
                last_triggered_at.as_deref(),
                &completion_state,
            ),
        },
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        last_triggered_at,
        completed_at: row.get("completed_at")?,
        current_tracked: current_tracked_task_id == Some(task_id.as_str()),
    })
}

fn task_store_list_tasks(data_path: &PathBuf) -> Result<Vec<TaskEntry>, String> {
    let conn = task_store_open(data_path)?;
    let current = task_store_get_runtime_state(&conn, TASK_RUNTIME_CURRENT_TRACKED_KEY)?;
    let mut stmt = conn
        .prepare("SELECT * FROM task_record ORDER BY order_index ASC")
        .map_err(|err| format!("Prepare list tasks failed: {err}"))?;
    let rows = stmt
        .query_map([], |row| task_row_to_entry(row, current.as_deref()))
        .map_err(|err| format!("Query list tasks failed: {err}"))?;
    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row.map_err(|err| format!("Read task row failed: {err}"))?);
    }
    Ok(tasks)
}

fn task_store_get_task(data_path: &PathBuf, task_id: &str) -> Result<TaskEntry, String> {
    let conn = task_store_open(data_path)?;
    let current = task_store_get_runtime_state(&conn, TASK_RUNTIME_CURRENT_TRACKED_KEY)?;
    conn.query_row(
        "SELECT * FROM task_record WHERE task_id = ?1",
        params![task_id],
        |row| task_row_to_entry(row, current.as_deref()),
    )
    .map_err(|err| format!("Get task failed: {err}"))
}

fn task_store_next_order_index(conn: &Connection) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(order_index), 0) FROM task_record",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value + 1)
    .map_err(|err| format!("Read task order index failed: {err}"))
}

fn task_store_create_task(data_path: &PathBuf, input: &TaskCreateInput) -> Result<TaskEntry, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("task.title is required".to_string());
    }
    let trigger = task_trigger_from_input(&input.trigger)?;
    let conn = task_store_open(data_path)?;
    let task_id = format!("task-{}", Uuid::new_v4());
    let now = now_iso();
    let order_index = task_store_next_order_index(&conn)?;
    let todos = input
        .todos
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    conn.execute(
        "INSERT INTO task_record (
            task_id, order_index, title, cause, goal, flow, todos_json, status_summary,
            completion_state, completion_conclusion, progress_notes_json, stage_key, stage_updated_at,
            trigger_kind, run_at, every_minutes, end_at, created_at, updated_at, last_triggered_at, completed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '', ?10, '', NULL, ?11, ?12, ?13, ?14, ?15, ?16, NULL, NULL)",
        params![
            task_id,
            order_index,
            title,
            input.cause.trim(),
            input.goal.trim(),
            input.flow.trim(),
            task_list_to_json(&todos)?,
            input.status_summary.trim(),
            TASK_STATE_ACTIVE,
            task_notes_to_json(&Vec::<TaskProgressNote>::new())?,
            task_trigger_kind_from_fields(trigger.run_at.as_deref(), trigger.every_minutes),
            trigger.run_at.as_deref(),
            trigger.every_minutes,
            trigger.end_at.as_deref(),
            now,
            now,
        ],
    )
    .map_err(|err| format!("Create task failed: {err}"))?;
    task_scheduler_refresh_current_tracked(data_path)?;
    task_store_get_task(data_path, &task_id)
}

fn task_store_update_task(data_path: &PathBuf, input: &TaskUpdateInput) -> Result<TaskEntry, String> {
    let existing = task_store_get_task(data_path, &input.task_id)?;
    if existing.completion_state != TASK_STATE_ACTIVE {
        return Err("Only active tasks can be updated".to_string());
    }
    let trigger = if let Some(trigger) = &input.trigger {
        task_trigger_from_input(trigger)?
    } else {
        TaskTriggerInput {
            run_at: existing.trigger.run_at.clone(),
            every_minutes: existing.trigger.every_minutes,
            end_at: existing.trigger.end_at.clone(),
        }
    };
    let mut notes = existing.progress_notes.clone();
    if let Some(note) = &input.append_note {
        task_append_progress_note(&mut notes, note);
    }
    let title = input.title.as_deref().unwrap_or(&existing.title).trim().to_string();
    if title.is_empty() {
        return Err("task.title cannot be empty".to_string());
    }
    let todos = input
        .todos
        .clone()
        .unwrap_or(existing.todos.clone())
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let stage_key = input.stage_key.as_deref().unwrap_or(&existing.stage_key).trim().to_string();
    let stage_updated_at = if input.stage_key.is_some() || input.append_note.is_some() {
        Some(now_iso())
    } else {
        existing.stage_updated_at.clone()
    };
    let conn = task_store_open(data_path)?;
    conn.execute(
        "UPDATE task_record SET
            title = ?2,
            cause = ?3,
            goal = ?4,
            flow = ?5,
            todos_json = ?6,
            status_summary = ?7,
            progress_notes_json = ?8,
            stage_key = ?9,
            stage_updated_at = ?10,
            trigger_kind = ?11,
            run_at = ?12,
            every_minutes = ?13,
            end_at = ?14,
            updated_at = ?15
         WHERE task_id = ?1",
        params![
            input.task_id,
            title,
            input.cause.as_deref().unwrap_or(&existing.cause).trim(),
            input.goal.as_deref().unwrap_or(&existing.goal).trim(),
            input.flow.as_deref().unwrap_or(&existing.flow).trim(),
            task_list_to_json(&todos)?,
            input.status_summary.as_deref().unwrap_or(&existing.status_summary).trim(),
            task_notes_to_json(&notes)?,
            stage_key,
            stage_updated_at,
            task_trigger_kind_from_fields(trigger.run_at.as_deref(), trigger.every_minutes),
            trigger.run_at.as_deref(),
            trigger.every_minutes,
            trigger.end_at.as_deref(),
            now_iso(),
        ],
    )
    .map_err(|err| format!("Update task failed: {err}"))?;
    task_scheduler_refresh_current_tracked(data_path)?;
    task_store_get_task(data_path, &input.task_id)
}

fn task_store_complete_task(data_path: &PathBuf, input: &TaskCompleteInput) -> Result<TaskEntry, String> {
    let existing = task_store_get_task(data_path, &input.task_id)?;
    if existing.completion_state != TASK_STATE_ACTIVE {
        return Err("Task is already completed".to_string());
    }
    let completion_state = task_completion_state_normalized(&input.completion_state)?;
    if completion_state == TASK_STATE_ACTIVE {
        return Err("Complete task cannot keep completionState=active".to_string());
    }
    let mut notes = existing.progress_notes.clone();
    if let Some(note) = &input.append_note {
        task_append_progress_note(&mut notes, note);
    }
    let final_status = if input.status_summary.trim().is_empty() {
        existing.status_summary.trim().to_string()
    } else {
        input.status_summary.trim().to_string()
    };
    let now = now_iso();
    let conn = task_store_open(data_path)?;
    conn.execute(
        "UPDATE task_record SET
            completion_state = ?2,
            completion_conclusion = ?3,
            status_summary = ?4,
            progress_notes_json = ?5,
            completed_at = ?6,
            updated_at = ?7
         WHERE task_id = ?1",
        params![
            input.task_id,
            completion_state,
            input.completion_conclusion.trim(),
            final_status,
            task_notes_to_json(&notes)?,
            now,
            now,
        ],
    )
    .map_err(|err| format!("Complete task failed: {err}"))?;
    task_scheduler_refresh_current_tracked(data_path)?;
    task_store_get_task(data_path, &input.task_id)
}

fn task_store_mark_triggered(data_path: &PathBuf, task_id: &str) -> Result<(), String> {
    let conn = task_store_open(data_path)?;
    let now = now_iso();
    conn.execute(
        "UPDATE task_record SET last_triggered_at = ?2, updated_at = ?2 WHERE task_id = ?1",
        params![task_id, now],
    )
    .map_err(|err| format!("Mark task triggered failed: {err}"))?;
    Ok(())
}

fn task_store_insert_run_log(data_path: &PathBuf, task_id: &str, outcome: &str, note: &str) -> Result<(), String> {
    let conn = task_store_open(data_path)?;
    conn.execute(
        "INSERT INTO task_run_log (task_id, triggered_at, outcome, note) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, now_iso(), outcome, note],
    )
    .map_err(|err| format!("Insert task run log failed: {err}"))?;
    Ok(())
}


fn task_store_list_run_logs(
    data_path: &PathBuf,
    task_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TaskRunLogEntry>, String> {
    let conn = task_store_open(data_path)?;
    let capped = limit.clamp(1, 200);
    let sql_all = "SELECT id, task_id, triggered_at, outcome, note FROM task_run_log ORDER BY id DESC LIMIT ?1";
    let sql_task = "SELECT id, task_id, triggered_at, outcome, note FROM task_run_log WHERE task_id = ?1 ORDER BY id DESC LIMIT ?2";
    let mut out = Vec::new();
    if let Some(task_id) = task_id.filter(|v| !v.trim().is_empty()) {
        let mut stmt = conn.prepare(sql_task).map_err(|err| format!("Prepare task run logs failed: {err}"))?;
        let rows = stmt
            .query_map(params![task_id.trim(), capped as i64], |row| {
                Ok(TaskRunLogEntry {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    triggered_at: row.get(2)?,
                    outcome: row.get(3)?,
                    note: row.get(4)?,
                })
            })
            .map_err(|err| format!("Query task run logs failed: {err}"))?;
        for row in rows {
            out.push(row.map_err(|err| format!("Read task run log failed: {err}"))?);
        }
        return Ok(out);
    }
    let mut stmt = conn.prepare(sql_all).map_err(|err| format!("Prepare task run logs failed: {err}"))?;
    let rows = stmt
        .query_map(params![capped as i64], |row| {
            Ok(TaskRunLogEntry {
                id: row.get(0)?,
                task_id: row.get(1)?,
                triggered_at: row.get(2)?,
                outcome: row.get(3)?,
                note: row.get(4)?,
            })
        })
        .map_err(|err| format!("Query task run logs failed: {err}"))?;
    for row in rows {
        out.push(row.map_err(|err| format!("Read task run log failed: {err}"))?);
    }
    Ok(out)
}
