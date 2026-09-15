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
            conversation_id TEXT,
            department_id TEXT,
            agent_id TEXT,
            target_scope TEXT NOT NULL DEFAULT 'desktop',
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
            stage_updated_at_utc TEXT,
            trigger_kind TEXT NOT NULL,
            run_at_utc TEXT,
            cron_expression TEXT,
            every_minutes INTEGER,
            end_at_utc TEXT,
            created_at_utc TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL,
            last_triggered_at_utc TEXT,
            completed_at_utc TEXT
        );
        CREATE TABLE IF NOT EXISTS task_runtime_state (
            state_key TEXT PRIMARY KEY,
            state_value TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS task_run_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            triggered_at_utc TEXT NOT NULL,
            outcome TEXT NOT NULL,
            note TEXT NOT NULL
        );
        COMMIT;",
    )
    .map_err(|err| format!("Init task db failed: {err}"))?;
    task_store_apply_migrations(conn)
}

fn task_normalize_run_at(value: &str) -> Result<String, String> {
    normalize_rfc3339_to_utc_storage("task.trigger.run_at", value)
}

fn task_normalize_end_at(value: &str) -> Result<String, String> {
    normalize_rfc3339_to_utc_storage("task.trigger.end_at", value)
}

// ========== 任务时间边界：输入 local，入库 utc ==========
fn task_trigger_from_local_input(input: &TaskTriggerInputLocal) -> Result<TaskTriggerStored, String> {
    let run_at = input
        .run_at
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let cron_expression = input
        .cron_expression
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let end_at = input
        .end_at
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if run_at.is_empty() {
        return Err("task.trigger.run_at is required".to_string());
    }
    let normalized_run_at_utc = task_normalize_run_at(run_at)?;
    let normalized_cron_expression = if !cron_expression.is_empty() {
        Some(task_normalize_cron_expression(cron_expression)?)
    } else if let Some(legacy_every_minutes) = input.legacy_every_minutes {
        task_exact_cron_expression_from_legacy_every_minutes(
            Some(normalized_run_at_utc.as_str()),
            legacy_every_minutes,
        )
    } else {
        None
    };
    let normalized_legacy_every_minutes = if normalized_cron_expression.is_some() {
        None
    } else {
        input
            .legacy_every_minutes
            .and_then(task_legacy_every_minutes_normalized)
    };
    let normalized_end_at_utc = if end_at.is_empty() {
        None
    } else {
        Some(task_normalize_end_at(end_at)?)
    };
    let run_dt = parse_rfc3339_time(&normalized_run_at_utc)
        .ok_or_else(|| "task.trigger.run_at normalization failed".to_string())?;
    if let Some(normalized_end_at_utc) = normalized_end_at_utc.as_deref() {
        let end_dt = parse_rfc3339_time(normalized_end_at_utc)
            .ok_or_else(|| "task.trigger.end_at normalization failed".to_string())?;
        if end_dt <= run_dt {
            return Err("task.trigger.end_at must be later than task.trigger.run_at".to_string());
        }
    }
    Ok(TaskTriggerStored {
        run_at_utc: Some(normalized_run_at_utc),
        cron_expression: normalized_cron_expression,
        legacy_every_minutes: normalized_legacy_every_minutes,
        end_at_utc: normalized_end_at_utc,
        next_run_at_utc: None,
    })
}

fn task_trigger_kind_from_fields(
    run_at_utc: Option<&str>,
    cron_expression: Option<&str>,
    legacy_every_minutes: Option<f64>,
) -> &'static str {
    if run_at_utc.is_none() {
        "legacy_immediate"
    } else if cron_expression.map(str::trim).filter(|value| !value.is_empty()).is_some() {
        "cron"
    } else if legacy_every_minutes.is_some() {
        "legacy_every_minutes"
    } else {
        "once"
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

fn task_notes_to_json(items: &[TaskProgressNoteStored]) -> Result<String, String> {
    serde_json::to_string(items).map_err(|err| format!("Serialize task notes failed: {err}"))
}

fn task_list_from_json(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn task_notes_from_json(raw: &str) -> Vec<TaskProgressNoteStored> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn task_row_to_record_stored(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecordStored> {
    let completion_state: String = row.get("completion_state")?;
    let run_at_utc: Option<String> = row.get("run_at_utc")?;
    let cron_expression: Option<String> = row.get("cron_expression")?;
    let every_minutes: Option<f64> = row.get("every_minutes")?;
    let end_at_utc: Option<String> = row.get("end_at_utc")?;
    let last_triggered_at_utc: Option<String> = row.get("last_triggered_at_utc")?;
    let normalized_cron_expression = cron_expression
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            every_minutes.and_then(|value| {
                task_exact_cron_expression_from_legacy_every_minutes(run_at_utc.as_deref(), value)
            })
        });
    Ok(TaskRecordStored {
        task_id: row.get("task_id")?,
        conversation_id: Some(task_normalize_bound_conversation_id(
            row.get::<_, Option<String>>("conversation_id")?.as_deref(),
        )),
        agent_id: row
            .get::<_, Option<String>>("agent_id")?
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        target_scope: task_target_scope_normalized(&row.get::<_, String>("target_scope")?).to_string(),
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
        stage_updated_at_utc: row.get("stage_updated_at_utc")?,
        trigger: TaskTriggerStored {
            run_at_utc: run_at_utc.clone(),
            cron_expression: normalized_cron_expression.clone(),
            legacy_every_minutes: every_minutes.and_then(task_legacy_every_minutes_normalized),
            end_at_utc: end_at_utc.clone(),
            next_run_at_utc: task_compute_next_run_at_utc_raw(
                run_at_utc.as_deref(),
                normalized_cron_expression.as_deref(),
                every_minutes.and_then(task_legacy_every_minutes_normalized),
                end_at_utc.as_deref(),
                last_triggered_at_utc.as_deref(),
                &completion_state,
            ),
        },
        created_at_utc: row.get("created_at_utc")?,
        updated_at_utc: row.get("updated_at_utc")?,
        last_triggered_at_utc,
        completed_at_utc: row.get("completed_at_utc")?,
    })
}

// ========== 任务时间边界：读库 utc，对外输出 local ==========
fn task_store_list_task_records(data_path: &PathBuf) -> Result<Vec<TaskRecordStored>, String> {
    let conn = task_store_open(data_path)?;
    let mut stmt = conn
        .prepare("SELECT * FROM task_record ORDER BY order_index ASC")
        .map_err(|err| format!("Prepare list task records failed: {err}"))?;
    let rows = stmt
        .query_map([], task_row_to_record_stored)
        .map_err(|err| format!("Query list task records failed: {err}"))?;
    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row.map_err(|err| format!("Read task record row failed: {err}"))?);
    }
    Ok(tasks)
}

fn task_store_get_task_record(data_path: &PathBuf, task_id: &str) -> Result<TaskRecordStored, String> {
    let conn = task_store_open(data_path)?;
    conn.query_row(
        "SELECT * FROM task_record WHERE task_id = ?1",
        params![task_id],
        task_row_to_record_stored,
    )
    .map_err(|err| format!("Get task record failed: {err}"))
}

fn task_store_list_tasks(data_path: &PathBuf) -> Result<Vec<TaskEntry>, String> {
    let tasks = task_store_list_task_records(data_path)?;
    Ok(tasks.iter().map(task_entry_view_from_stored).collect())
}

fn task_store_get_task(data_path: &PathBuf, task_id: &str) -> Result<TaskEntry, String> {
    let task = task_store_get_task_record(data_path, task_id)?;
    Ok(task_entry_view_from_stored(&task))
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
    let goal = input.goal.trim();
    if goal.is_empty() {
        return Err("task.goal is required".to_string());
    }
    let trigger = task_trigger_from_local_input(&input.trigger)?;
    let conn = task_store_open(data_path)?;
    let task_id = format!("task-{}", Uuid::new_v4());
    let now_utc = now_utc_rfc3339();
    let order_index = task_store_next_order_index(&conn)?;
    let conversation_id = task_normalize_bound_conversation_id(input.conversation_id.as_deref());
    let target_scope = task_target_scope_normalized(
        input
            .target_scope
            .as_deref()
            .unwrap_or(TASK_TARGET_SCOPE_DESKTOP),
    );
    let todos = task_legacy_todos_from_todo(&input.todo);
    conn.execute(
        "INSERT INTO task_record (
            task_id, conversation_id, agent_id, target_scope, order_index, title, cause, goal, flow, todos_json, status_summary,
            completion_state, completion_conclusion, progress_notes_json, stage_key, stage_updated_at_utc,
            trigger_kind, run_at_utc, cron_expression, every_minutes, end_at_utc, created_at_utc, updated_at_utc,
            last_triggered_at_utc, completed_at_utc
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, '', ?13, '', NULL, ?14, ?15, ?16, ?17, ?18, ?19, ?20, NULL, NULL)",
        params![
            task_id,
            conversation_id,
            input.agent_id.as_deref().map(str::trim).filter(|value| !value.is_empty()),
            target_scope,
            order_index,
            task_legacy_title_from_goal(goal),
            task_legacy_cause_from_why(&input.why),
            task_legacy_goal_from_goal(goal),
            task_legacy_flow_from_why(&input.why),
            task_list_to_json(&todos)?,
            task_legacy_status_summary_from_todo(&input.todo),
            TASK_STATE_ACTIVE,
            task_notes_to_json(&Vec::<TaskProgressNoteStored>::new())?,
            task_trigger_kind_from_fields(
                trigger.run_at_utc.as_deref(),
                trigger.cron_expression.as_deref(),
                trigger.legacy_every_minutes,
            ),
            trigger.run_at_utc.as_deref(),
            trigger.cron_expression.as_deref(),
            trigger.legacy_every_minutes,
            trigger.end_at_utc.as_deref(),
            now_utc,
            now_utc,
        ],
    )
    .map_err(|err| format!("Create task failed: {err}"))?;
    task_store_get_task(data_path, &task_id)
}

fn task_store_update_task(data_path: &PathBuf, input: &TaskUpdateInput) -> Result<TaskEntry, String> {
    let existing = task_store_get_task_record(data_path, &input.task_id)?;
    if existing.completion_state != TASK_STATE_ACTIVE {
        return Err("Only active tasks can be updated".to_string());
    }
    let trigger = if let Some(trigger_input) = &input.trigger {
        task_trigger_from_local_input(trigger_input)?
    } else {
        existing.trigger.clone()
    };
    if !task_record_is_one_time(&existing) && task_trigger_is_one_time(&trigger) {
        return Err("定时任务不能切换为一次性任务".to_string());
    }
    let next_goal = input
        .goal
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| task_goal_from_legacy_fields(&existing.title, &existing.goal));
    if next_goal.trim().is_empty() {
        return Err("task.goal cannot be empty".to_string());
    }
    let next_why = input
        .why
        .as_deref()
        .map(str::trim)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| task_why_from_legacy_record(&existing));
    let next_todo = input
        .todo
        .as_deref()
        .map(str::trim)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| task_todo_from_legacy_fields(&existing.status_summary, &existing.todos));
    let conversation_id = task_normalize_bound_conversation_id(
        input
            .conversation_id
            .as_deref()
            .or(existing.conversation_id.as_deref()),
    );
    let agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(existing.agent_id.clone());
    let target_scope = input
        .target_scope
        .as_deref()
        .map(task_target_scope_normalized)
        .unwrap_or_else(|| task_target_scope_normalized(&existing.target_scope))
        .to_string();
    let existing_notes_json = task_notes_to_json(&existing.progress_notes)?;
    let existing_stage_key = existing.stage_key.clone();
    let existing_stage_updated_at_utc = existing.stage_updated_at_utc.clone();
    let conn = task_store_open(data_path)?;
    conn.execute(
        "UPDATE task_record SET
            conversation_id = ?2,
            agent_id = ?3,
            target_scope = ?4,
            title = ?5,
            cause = ?6,
            goal = ?7,
            flow = ?8,
            todos_json = ?9,
            status_summary = ?10,
            progress_notes_json = ?11,
            stage_key = ?12,
            stage_updated_at_utc = ?13,
            trigger_kind = ?14,
            run_at_utc = ?15,
            cron_expression = ?16,
            every_minutes = ?17,
            end_at_utc = ?18,
            updated_at_utc = ?19
         WHERE task_id = ?1",
        params![
            input.task_id,
            conversation_id,
            agent_id.as_deref(),
            target_scope,
            task_legacy_title_from_goal(&next_goal),
            task_legacy_cause_from_why(&next_why),
            task_legacy_goal_from_goal(&next_goal),
            task_legacy_flow_from_why(&next_why),
            task_list_to_json(&task_legacy_todos_from_todo(&next_todo))?,
            task_legacy_status_summary_from_todo(&next_todo),
            existing_notes_json,
            existing_stage_key,
            existing_stage_updated_at_utc,
            task_trigger_kind_from_fields(
                trigger.run_at_utc.as_deref(),
                trigger.cron_expression.as_deref(),
                trigger.legacy_every_minutes,
            ),
            trigger.run_at_utc.as_deref(),
            trigger.cron_expression.as_deref(),
            trigger.legacy_every_minutes,
            trigger.end_at_utc.as_deref(),
            now_utc_rfc3339(),
        ],
    )
    .map_err(|err| format!("Update task failed: {err}"))?;
    task_store_get_task(data_path, &input.task_id)
}

fn task_store_complete_task(data_path: &PathBuf, input: &TaskCompleteInput) -> Result<TaskEntry, String> {
    let existing = task_store_get_task_record(data_path, &input.task_id)?;
    if existing.completion_state != TASK_STATE_ACTIVE {
        return Err("Task is already completed".to_string());
    }
    let completion_state = task_completion_state_normalized(&input.completion_state)?;
    if completion_state == TASK_STATE_ACTIVE {
        return Err("Complete task cannot keep completionState=active".to_string());
    }
    let now_utc = now_utc_rfc3339();
    let existing_status_summary = existing.status_summary.clone();
    let existing_notes_json = task_notes_to_json(&existing.progress_notes)?;
    let conn = task_store_open(data_path)?;
    conn.execute(
        "UPDATE task_record SET
            completion_state = ?2,
            completion_conclusion = ?3,
            status_summary = ?4,
            progress_notes_json = ?5,
            completed_at_utc = ?6,
            updated_at_utc = ?7
         WHERE task_id = ?1",
        params![
            input.task_id,
            completion_state,
            input.completion_conclusion.trim(),
            existing_status_summary,
            existing_notes_json,
            now_utc,
            now_utc,
        ],
    )
    .map_err(|err| format!("Complete task failed: {err}"))?;
    task_store_get_task(data_path, &input.task_id)
}

fn task_store_complete_one_time_dispatch(
    data_path: &PathBuf,
    task_id: &str,
) -> Result<bool, String> {
    let normalized_task_id = task_id.trim();
    if normalized_task_id.is_empty() {
        return Err("task.taskId is required".to_string());
    }
    let conn = task_store_open(data_path)?;
    let now_utc = now_utc_rfc3339();
    let affected = conn
        .execute(
            "UPDATE task_record
             SET completion_state = ?2,
                 last_triggered_at_utc = CASE
                     WHEN last_triggered_at_utc IS NULL OR TRIM(last_triggered_at_utc) = '' THEN ?3
                     ELSE last_triggered_at_utc
                 END,
                 completed_at_utc = CASE
                     WHEN completed_at_utc IS NULL OR TRIM(completed_at_utc) = '' THEN ?3
                     ELSE completed_at_utc
                 END,
                 updated_at_utc = ?3
             WHERE task_id = ?1
               AND completion_state = ?4
               AND (cron_expression IS NULL OR TRIM(cron_expression) = '')
               AND every_minutes IS NULL",
            params![
                normalized_task_id,
                TASK_STATE_COMPLETED,
                now_utc,
                TASK_STATE_ACTIVE,
            ],
        )
        .map_err(|err| format!("Complete one-time task dispatch failed: {err}"))?;
    Ok(affected > 0)
}

fn task_store_fail_active_task(
    data_path: &PathBuf,
    task_id: &str,
    conclusion: &str,
    log_note: &str,
) -> Result<bool, String> {
    let normalized_task_id = task_id.trim();
    if normalized_task_id.is_empty() {
        return Err("task.taskId is required".to_string());
    }
    let conn = task_store_open(data_path)?;
    let now_utc = now_utc_rfc3339();
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|err| format!("Begin task fail transaction failed: {err}"))?;
    let result = (|| -> Result<bool, String> {
        let affected = conn
            .execute(
                "UPDATE task_record
                 SET completion_state = ?2,
                     completion_conclusion = ?3,
                     completed_at_utc = ?4,
                     updated_at_utc = ?4
                 WHERE task_id = ?1 AND completion_state = ?5",
                params![
                    normalized_task_id,
                    TASK_STATE_FAILED_COMPLETED,
                    conclusion.trim(),
                    now_utc.as_str(),
                    TASK_STATE_ACTIVE,
                ],
            )
            .map_err(|err| format!("Mark task failed failed: {err}"))?;
        if affected > 0 {
            conn.execute(
                "INSERT INTO task_run_log (task_id, triggered_at_utc, outcome, note) VALUES (?1, ?2, ?3, ?4)",
                params![normalized_task_id, now_utc.as_str(), "failed", log_note.trim()],
            )
            .map_err(|err| format!("Insert task failed run log failed: {err}"))?;
        }
        Ok(affected > 0)
    })();
    match result {
        Ok(value) => {
            conn.execute_batch("COMMIT;")
                .map_err(|err| format!("Commit task fail transaction failed: {err}"))?;
            Ok(value)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(err)
        }
    }
}

fn task_store_delete_task(data_path: &PathBuf, task_id: &str) -> Result<(), String> {
    let normalized_task_id = task_id.trim();
    if normalized_task_id.is_empty() {
        return Err("task.taskId is required".to_string());
    }

    task_store_get_task_record(data_path, normalized_task_id)?;

    let conn = task_store_open(data_path)?;
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|err| format!("Begin task delete transaction failed: {err}"))?;

    let delete_result = (|| -> Result<(), String> {
        conn.execute(
            "DELETE FROM task_run_log WHERE task_id = ?1",
            params![normalized_task_id],
        )
        .map_err(|err| format!("Delete task run logs failed: {err}"))?;

        let affected = conn
            .execute(
                "DELETE FROM task_record WHERE task_id = ?1",
                params![normalized_task_id],
            )
            .map_err(|err| format!("Delete task failed: {err}"))?;

        if affected == 0 {
            return Err("Task not found".to_string());
        }
        Ok(())
    })();

    match delete_result {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|err| format!("Commit task delete transaction failed: {err}"))?,
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(err);
        }
    }

    Ok(())
}

fn task_store_mark_triggered(data_path: &PathBuf, task_id: &str) -> Result<(), String> {
    let conn = task_store_open(data_path)?;
    let now_utc = now_utc_rfc3339();
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|err| format!("Begin task trigger transaction failed: {err}"))?;
    let result = (|| -> Result<(), String> {
        let affected = conn
            .execute(
                "UPDATE task_record SET last_triggered_at_utc = ?2, updated_at_utc = ?2 WHERE task_id = ?1",
                params![task_id, now_utc.as_str()],
            )
            .map_err(|err| format!("Mark task triggered failed: {err}"))?;
        if affected == 0 {
            return Err("Task not found".to_string());
        }
        let updated = conn
            .query_row(
                "SELECT * FROM task_record WHERE task_id = ?1",
                params![task_id],
                task_row_to_record_stored,
            )
            .map_err(|err| format!("Read triggered task failed: {err}"))?;
        let should_complete = updated.completion_state == TASK_STATE_ACTIVE
            && updated.trigger.end_at_utc.is_some()
            && updated.trigger.next_run_at_utc.is_none()
            && (updated.trigger.cron_expression.is_some()
                || updated.trigger.legacy_every_minutes.is_some());
        if should_complete {
            conn.execute(
                "UPDATE task_record
                 SET completion_state = ?2,
                     completed_at_utc = CASE
                         WHEN completed_at_utc IS NULL OR TRIM(completed_at_utc) = '' THEN ?3
                         ELSE completed_at_utc
                     END,
                     updated_at_utc = ?3
                 WHERE task_id = ?1 AND completion_state = ?4",
                params![
                    task_id,
                    TASK_STATE_COMPLETED,
                    now_utc.as_str(),
                    TASK_STATE_ACTIVE,
                ],
            )
            .map_err(|err| format!("Auto complete ended recurring task failed: {err}"))?;
            conn.execute(
                "INSERT INTO task_run_log (task_id, triggered_at_utc, outcome, note) VALUES (?1, ?2, ?3, ?4)",
                params![task_id, now_utc.as_str(), "completed", "任务达到结束时间，已自动完成"],
            )
            .map_err(|err| format!("Insert task auto complete run log failed: {err}"))?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|err| format!("Commit task trigger transaction failed: {err}"))?,
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(err);
        }
    }
    Ok(())
}

fn task_store_mark_skipped(
    data_path: &PathBuf,
    task_id: &str,
    outcome: &str,
    note: &str,
) -> Result<(), String> {
    let conn = task_store_open(data_path)?;
    let now_utc = now_utc_rfc3339();
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|err| format!("Begin task skip transaction failed: {err}"))?;
    let result = (|| -> Result<(), String> {
        let affected = conn.execute(
            "UPDATE task_record SET last_triggered_at_utc = ?2, updated_at_utc = ?2 WHERE task_id = ?1",
            params![task_id, now_utc],
        )
        .map_err(|err| format!("Mark task skipped failed: {err}"))?;
        if affected == 0 {
            return Err("Task not found".to_string());
        }
        conn.execute(
            "INSERT INTO task_run_log (task_id, triggered_at_utc, outcome, note) VALUES (?1, ?2, ?3, ?4)",
            params![task_id, now_utc, outcome, note],
        )
        .map_err(|err| format!("Insert task skip run log failed: {err}"))?;
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("COMMIT;")
            .map_err(|err| format!("Commit task skip transaction failed: {err}"))?,
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(err);
        }
    }
    Ok(())
}

fn task_store_insert_run_log(data_path: &PathBuf, task_id: &str, outcome: &str, note: &str) -> Result<(), String> {
    let conn = task_store_open(data_path)?;
    conn.execute(
        "INSERT INTO task_run_log (task_id, triggered_at_utc, outcome, note) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, now_utc_rfc3339(), outcome, note],
    )
    .map_err(|err| format!("Insert task run log failed: {err}"))?;
    Ok(())
}

fn task_store_list_run_log_records(
    data_path: &PathBuf,
    task_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TaskRunLogStored>, String> {
    let conn = task_store_open(data_path)?;
    let capped = limit.clamp(1, 200);
    let sql_all =
        "SELECT id, task_id, triggered_at_utc, outcome, note FROM task_run_log ORDER BY id DESC LIMIT ?1";
    let sql_task = "SELECT id, task_id, triggered_at_utc, outcome, note FROM task_run_log WHERE task_id = ?1 ORDER BY id DESC LIMIT ?2";
    let mut out = Vec::new();
    if let Some(task_id) = task_id.filter(|value| !value.trim().is_empty()) {
        let mut stmt = conn
            .prepare(sql_task)
            .map_err(|err| format!("Prepare task run logs failed: {err}"))?;
        let rows = stmt
            .query_map(params![task_id.trim(), capped as i64], |row| {
                Ok(TaskRunLogStored {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    triggered_at_utc: row.get(2)?,
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
    let mut stmt = conn
        .prepare(sql_all)
        .map_err(|err| format!("Prepare task run logs failed: {err}"))?;
    let rows = stmt
        .query_map(params![capped as i64], |row| {
            Ok(TaskRunLogStored {
                id: row.get(0)?,
                task_id: row.get(1)?,
                triggered_at_utc: row.get(2)?,
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

fn task_store_list_run_logs(
    data_path: &PathBuf,
    task_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TaskRunLogEntry>, String> {
    let logs = task_store_list_run_log_records(data_path, task_id, limit)?;
    Ok(logs.iter().map(task_run_log_view_from_stored).collect())
}
