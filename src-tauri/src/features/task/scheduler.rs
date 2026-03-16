fn task_resolve_default_session(state: &AppState) -> Result<(String, String, String, String, String), String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let app_config = read_config(&state.config_path)?;
    let selected_api = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config configured for task dispatch.".to_string())?;
    let mut data = state_read_app_data_cached(state)?;
    let mut changed = ensure_default_agent(&mut data);
    let before_conversation_count = data.conversations.len();
    let agent_id = if data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user && !a.is_built_in_system)
    {
        data.assistant_department_agent_id.clone()
    } else {
        data.agents
            .iter()
            .find(|a| !a.is_built_in_user && !a.is_built_in_system)
            .map(|a| a.id.clone())
            .ok_or_else(|| "No assistant agent configured for task dispatch.".to_string())?
    };
    let conversation_idx = ensure_active_conversation_index(&mut data, &selected_api.id, &agent_id);
    let conversation_id = data
        .conversations
        .get(conversation_idx)
        .map(|item| item.id.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "No active main conversation configured for task dispatch.".to_string())?;
    changed = changed || data.conversations.len() != before_conversation_count;
    let department_id = department_for_agent_id(&app_config, &agent_id)
        .map(|item| item.id.clone())
        .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
    if changed {
        state_write_app_data_cached(state, &data)?;
    }
    drop(guard);
    Ok((
        selected_api.id.clone(),
        department_id,
        agent_id.clone(),
        conversation_id.clone(),
        inflight_chat_key(&agent_id, Some(&conversation_id)),
    ))
}

fn task_is_chat_busy(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    Ok(inflight.contains_key(chat_key))
}

fn task_enqueue_dispatch(state: &AppState, task_id: &str) -> Result<bool, String> {
    let mut queue = state
        .task_dispatch_queue
        .lock()
        .map_err(|_| "Failed to lock task dispatch queue".to_string())?;
    if queue.iter().any(|item| item.task_id == task_id) {
        return Ok(false);
    }
    queue.push_back(TaskDispatchQueueItem {
        task_id: task_id.to_string(),
        queued_at: now_iso(),
    });
    Ok(true)
}

fn task_dequeue_dispatch(state: &AppState, chat_key: &str) -> Result<Option<TaskDispatchQueueItem>, String> {
    let mut queue = state
        .task_dispatch_queue
        .lock()
        .map_err(|_| "Failed to lock task dispatch queue".to_string())?;
    if task_is_chat_busy(state, chat_key)? {
        return Ok(None);
    }
    Ok(queue.pop_front())
}

async fn task_try_dispatch_or_enqueue(state: &AppState, task: &TaskEntry) -> Result<(), String> {
    let (_api_id, _department_id, _agent_id, _conversation_id, chat_key) =
        task_resolve_default_session(state)?;
    if task_is_chat_busy(state, &chat_key)? {
        let queued = task_enqueue_dispatch(state, &task.task_id)?;
        if queued {
            task_store_insert_run_log(
                &state.data_path,
                &task.task_id,
                "queued",
                &format!(
                    "聊天繁忙，已排队等待分发，taskId={}，title={}，chatKey={}",
                    task.task_id,
                    task.title.trim(),
                    chat_key
                ),
            )?;
        }
        return Ok(());
    }
    task_dispatch_due_task(state, task).await
}

pub(crate) async fn task_process_dispatch_queue(state: &AppState) -> Result<(), String> {
    let (_api_id, _department_id, _agent_id, _conversation_id, chat_key) =
        task_resolve_default_session(state)?;
    let Some(item) = task_dequeue_dispatch(state, &chat_key)? else {
        return Ok(());
    };
    let task = task_store_get_task(&state.data_path, &item.task_id)?;
    if task.completion_state != TASK_STATE_ACTIVE {
        return Ok(());
    }
    task_store_insert_run_log(
        &state.data_path,
        &task.task_id,
        "dequeued",
        &format!(
            "从队列恢复分发，queuedAt={}，taskId={}，title={}",
            item.queued_at,
            task.task_id,
            task.title.trim()
        ),
    )?;
    task_dispatch_due_task(state, &task).await
}

fn task_is_due(entry: &TaskEntry, now: OffsetDateTime) -> bool {
    if entry.completion_state != TASK_STATE_ACTIVE {
        return false;
    }
    if entry.trigger.run_at.is_none() {
        return if let Some(last) = entry.last_triggered_at.as_deref().and_then(parse_iso) {
            now >= last + time::Duration::seconds(TASK_IMMEDIATE_RETRY_SECONDS)
        } else {
            true
        };
    }
    if entry.trigger.every_minutes.unwrap_or(0) == 0 {
        return entry
            .trigger
            .run_at
            .as_deref()
            .and_then(parse_iso)
            .map(|run_at| now >= run_at && entry.last_triggered_at.is_none())
            .unwrap_or(false);
    }
    let Some(run_at) = entry.trigger.run_at.as_deref().and_then(parse_iso) else {
        return false;
    };
    if now < run_at {
        return false;
    }
    let Some(end_at) = entry.trigger.end_at.as_deref().and_then(parse_iso) else {
        return false;
    };
    if now > end_at {
        return false;
    }
    let every = i64::from(entry.trigger.every_minutes.unwrap_or(0));
    if every <= 0 {
        return false;
    }
    if let Some(last) = entry.last_triggered_at.as_deref().and_then(parse_iso) {
        let next = last + time::Duration::minutes(every);
        next <= end_at && now >= next
    } else {
        true
    }
}

fn task_priority_time_rank(entry: &TaskEntry) -> i128 {
    if entry.trigger.run_at.is_none() {
        return i128::MIN;
    }
    entry
        .trigger
        .run_at
        .as_deref()
        .and_then(parse_iso)
        .map(|value| value.unix_timestamp_nanos())
        .unwrap_or(i128::MAX)
}

fn task_priority_rank(entry: &TaskEntry, now: OffsetDateTime) -> (i32, i128, i64) {
    let due_weight = if task_is_due(entry, now) { 0 } else { 1 };
    (due_weight, task_priority_time_rank(entry), entry.order_index)
}

fn task_scheduler_refresh_current_tracked(data_path: &PathBuf) -> Result<Option<String>, String> {
    let conn = task_store_open(data_path)?;
    let mut stmt = conn
        .prepare("SELECT * FROM task_record WHERE completion_state = ?1 ORDER BY order_index ASC")
        .map_err(|err| format!("Prepare tracked task query failed: {err}"))?;
    let rows = stmt
        .query_map(params![TASK_STATE_ACTIVE], |row| task_row_to_entry(row, None))
        .map_err(|err| format!("Query tracked tasks failed: {err}"))?;
    let mut active = Vec::new();
    for row in rows {
        active.push(row.map_err(|err| format!("Read tracked task failed: {err}"))?);
    }
    let now = now_utc();
    active.sort_by_key(|item| task_priority_rank(item, now));
    let tracked = active.first().map(|item| item.task_id.clone());
    task_store_set_runtime_state(
        &conn,
        TASK_RUNTIME_CURRENT_TRACKED_KEY,
        tracked.as_deref().unwrap_or(""),
    )?;
    Ok(tracked)
}

fn task_build_board_snapshot(data_path: &PathBuf) -> Result<TaskBoardSnapshot, String> {
    let tracked = task_scheduler_refresh_current_tracked(data_path)?;
    let tasks = task_store_list_tasks(data_path)?;
    let tracked_task = tracked
        .as_deref()
        .filter(|task_id| !task_id.trim().is_empty())
        .and_then(|task_id| tasks.iter().find(|item| item.task_id == task_id).cloned());
    Ok(TaskBoardSnapshot {
        current_tracked_task_id: tracked_task.as_ref().map(|item| item.task_id.clone()),
        tracked_task,
        tasks: tasks.into_iter().take(TASK_MAX_BOARD_ITEMS).collect(),
    })
}

fn build_hidden_task_board_block(state: &AppState) -> Option<String> {
    let snapshot = task_build_board_snapshot(&state.data_path).ok()?;
    let tracked = snapshot.tracked_task?;
    let mut lines = Vec::<String>::new();
    lines.push("[HIDDEN TASK BOARD]".to_string());
    lines.push(format!("currentLocalTime: {}", now_local_time_rfc3339()));
    lines.push("timeFormatNote: all task times below use local RFC3339 with timezone offset; copy the same format directly when writing runAt".to_string());
    lines.push(format!("trackedTaskId: {}", tracked.task_id));
    lines.push(format!("title: {}", tracked.title.trim()));
    if !tracked.goal.trim().is_empty() {
        lines.push(format!("goal: {}", tracked.goal.trim()));
    }
    if !tracked.cause.trim().is_empty() {
        lines.push(format!("cause: {}", tracked.cause.trim()));
    }
    if !tracked.flow.trim().is_empty() {
        lines.push(format!("flow: {}", tracked.flow.trim()));
    }
    if !tracked.status_summary.trim().is_empty() {
        lines.push(format!("statusSummary: {}", tracked.status_summary.trim()));
    }
    if !tracked.todos.is_empty() {
        lines.push(format!("todos: {}", tracked.todos.join(" | ")));
    }
    if let Some(run_at) = tracked.trigger.run_at.as_deref() {
        lines.push(format!("runAt: {}", format_message_time_rfc3339_local(run_at)));
    }
    if let Some(end_at) = tracked.trigger.end_at.as_deref() {
        lines.push(format!("endAt: {}", format_message_time_rfc3339_local(end_at)));
    }
    if let Some(next_run_at) = tracked.trigger.next_run_at.as_deref() {
        lines.push(format!("nextRunAt: {}", format_message_time_rfc3339_local(next_run_at)));
    }
    let other_tasks = snapshot
        .tasks
        .iter()
        .filter(|item| item.task_id != tracked.task_id)
        .map(|item| format!("{} ({})", item.title.trim(), item.completion_state))
        .collect::<Vec<_>>();
    if !other_tasks.is_empty() {
        lines.push(format!("otherTasks: {}", other_tasks.join("; ")));
    }
    Some(lines.join("\n"))
}

fn build_task_trigger_hidden_prompt(task: &TaskEntry) -> String {
    let mut lines = Vec::<String>::new();
    lines.push(format!("任务提醒：{}", task.title.trim()));
    if !task.cause.trim().is_empty() {
        lines.push(format!("起因：{}", task.cause.trim()));
    }
    if !task.goal.trim().is_empty() {
        lines.push(format!("目标：{}", task.goal.trim()));
    }
    if !task.flow.trim().is_empty() {
        lines.push(format!("流程：{}", task.flow.trim()));
    }
    if !task.status_summary.trim().is_empty() {
        lines.push(format!("当前状态：{}", task.status_summary.trim()));
    }
    if !task.todos.is_empty() {
        lines.push(format!("待办：{}", task.todos.join(" | ")));
    }
    lines.push("请立刻继续推进这个任务；如果确实受阻，请写明失败结论并完成任务，不要将它悬置。".to_string());
    lines.join("\n")
}

fn build_task_trigger_provider_meta(task: &TaskEntry) -> Value {
    serde_json::json!({
        "messageKind": "task_trigger",
        "hiddenPromptText": build_task_trigger_hidden_prompt(task),
        "taskTrigger": {
            "taskId": task.task_id,
            "title": task.title.trim(),
            "cause": task.cause.trim(),
            "goal": task.goal.trim(),
            "flow": task.flow.trim(),
            "statusSummary": task.status_summary.trim(),
            "todos": task.todos,
            "runAt": task.trigger.run_at,
            "endAt": task.trigger.end_at,
            "nextRunAt": task.trigger.next_run_at,
            "everyMinutes": task.trigger.every_minutes,
        }
    })
}

async fn task_dispatch_due_task(state: &AppState, task: &TaskEntry) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    task_store_mark_triggered(&state.data_path, &task.task_id)?;

    // 获取会话信息
    let (_api_id, department_id, agent_id, conversation_id, _) = task_resolve_default_session(state)?;

    // 构造任务消息
    let task_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text {
            text: build_task_trigger_hidden_prompt(task),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(build_task_trigger_provider_meta(task)),
        tool_call: None,
        mcp_call: None,
    };

    // 创建事件并入队
    let event = ChatPendingEvent {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::Task,
        messages: vec![task_message],
        activate_assistant: true,
        session_info: ChatSessionInfo {
            department_id,
            agent_id,
        },
        sender_info: None,
    };

    let trigger_label = if task.trigger.run_at.is_none() {
        "immediate"
    } else if task.trigger.every_minutes.unwrap_or(0) > 0 {
        "repeat"
    } else {
        "once"
    };
    let todo_count = task.todos.len();

    // 入队
    match enqueue_chat_event(state, event) {
        Ok(_) => {
            // 异步触发出队处理
            trigger_chat_queue_processing(state);

            let duration_ms = started_at.elapsed().as_millis();
            task_store_insert_run_log(
                &state.data_path,
                &task.task_id,
                "queued",
                &format!(
                    "任务已入队，title={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，everyMinutes={}，durationMs={}",
                    task.title.trim(),
                    conversation_id,
                    trigger_label,
                    todo_count,
                    task.trigger.run_at.is_some(),
                    task.trigger.every_minutes.unwrap_or(0),
                    duration_ms
                ),
            )?;
            Ok(())
        }
        Err(err) => {
            let duration_ms = started_at.elapsed().as_millis();
            task_store_insert_run_log(
                &state.data_path,
                &task.task_id,
                "failed",
                &format!(
                    "任务入队失败，title={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，everyMinutes={}，durationMs={}，error={}",
                    task.title.trim(),
                    conversation_id,
                    trigger_label,
                    todo_count,
                    task.trigger.run_at.is_some(),
                    task.trigger.every_minutes.unwrap_or(0),
                    duration_ms,
                    err
                ),
            )?;
            Err(err)
        }
    }
}

async fn task_scheduler_tick(state: &AppState) -> Result<(), String> {
    task_process_dispatch_queue(state).await?;
    let _ = task_scheduler_refresh_current_tracked(&state.data_path)?;
    let tasks = task_store_list_tasks(&state.data_path)?;
    let now = now_utc();
    let mut due_tasks = tasks
        .into_iter()
        .filter(|item| task_is_due(item, now))
        .collect::<Vec<_>>();
    due_tasks.sort_by_key(|item| task_priority_rank(item, now));
    if let Some(task) = due_tasks.into_iter().next() {
        task_try_dispatch_or_enqueue(state, &task).await?;
    }
    Ok(())
}

fn start_task_scheduler(state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let tick_started_at = std::time::Instant::now();
            if let Err(err) = task_scheduler_tick(&state).await {
                let queue_len = state
                    .task_dispatch_queue
                    .lock()
                    .map(|queue| queue.len())
                    .unwrap_or(0);
                eprintln!(
                    "[任务调度] 调度轮询失败，error={}，durationMs={}，queueLen={}，dataPath={}",
                    err,
                    tick_started_at.elapsed().as_millis(),
                    queue_len,
                    state.data_path.to_string_lossy()
                );
            }
            tokio::time::sleep(std::time::Duration::from_secs(TASK_SCHEDULER_INTERVAL_SECONDS)).await;
        }
    });
}

