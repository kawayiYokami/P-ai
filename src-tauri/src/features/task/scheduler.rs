fn task_conversation_available_for_dispatch(conversation: &Conversation) -> bool {
    conversation.summary.trim().is_empty() && !conversation_is_delegate(conversation)
}

#[derive(Debug, Clone)]
struct TaskResolvedConversation {
    conversation_id: String,
    target_scope: String,
    fallback_to_main: bool,
}

#[derive(Debug, Clone)]
struct TaskDispatchSessionResolved {
    model_config_id: String,
    department_id: String,
    agent_id: String,
    conversation_id: String,
    target_scope: String,
    fallback_to_main: bool,
}

#[derive(Debug, Clone)]
struct TaskDispatchCandidate {
    task: TaskRecordStored,
    session: TaskDispatchSessionResolved,
}

fn task_scope_for_conversation(conversation: &Conversation) -> &'static str {
    if conversation_is_remote_im_contact(conversation) {
        TASK_TARGET_SCOPE_CONTACT
    } else {
        TASK_TARGET_SCOPE_DESKTOP
    }
}

fn task_scope_for_missing_conversation(
    data: &AppData,
    requested_conversation_id: &str,
    stored_target_scope: &str,
) -> &'static str {
    if data.remote_im_contacts.iter().any(|contact| {
        contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            == Some(requested_conversation_id)
    }) {
        return TASK_TARGET_SCOPE_CONTACT;
    }
    task_target_scope_normalized(stored_target_scope)
}

fn task_resolve_main_dispatch_conversation_id(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
    fallback_to_main: bool,
) -> Result<TaskResolvedConversation, String> {
    let conversation_idx = ensure_main_conversation_index(data, api_config_id, agent_id);
    let conversation_id = data
        .conversations
        .get(conversation_idx)
        .map(|item| item.id.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "No active main conversation configured for task dispatch.".to_string())?;
    Ok(TaskResolvedConversation {
        conversation_id,
        target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
        fallback_to_main,
    })
}

fn task_resolve_dispatch_conversation(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
    requested_conversation_id: Option<&str>,
    stored_target_scope: &str,
) -> Result<Option<TaskResolvedConversation>, String> {
    if let Some(requested) = requested_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let requested_scope = task_scope_for_missing_conversation(data, requested, stored_target_scope);
        if let Some(conversation) = data
            .conversations
            .iter()
            .find(|item| item.id == requested && task_conversation_available_for_dispatch(item))
        {
            return Ok(Some(TaskResolvedConversation {
                conversation_id: conversation.id.clone(),
                target_scope: task_scope_for_conversation(conversation).to_string(),
                fallback_to_main: false,
            }));
        }
        if requested_scope == TASK_TARGET_SCOPE_CONTACT {
            return Ok(None);
        }
        return task_resolve_main_dispatch_conversation_id(data, api_config_id, agent_id, true).map(Some);
    }

    if task_target_scope_normalized(stored_target_scope) == TASK_TARGET_SCOPE_CONTACT {
        return Ok(None);
    }
    task_resolve_main_dispatch_conversation_id(data, api_config_id, agent_id, false).map(Some)
}

fn task_resolve_dispatch_session(
    state: &AppState,
    task: &TaskRecordStored,
) -> Result<Option<TaskDispatchSessionResolved>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let app_config = read_config(&state.config_path)?;
    let selected_api = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config configured for task dispatch.".to_string())?;
    let mut data = state_read_app_data_cached(state)?;
    let mut changed = false;
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
    let requested_conversation_id = task
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let resolved = task_resolve_dispatch_conversation(
        &mut data,
        &selected_api.id,
        &agent_id,
        requested_conversation_id,
        &task.target_scope,
    )?;
    changed = changed || data.conversations.len() != before_conversation_count;
    let department_id = department_for_agent_id(&app_config, &agent_id)
        .map(|item| item.id.clone())
        .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
    if changed {
        state_write_app_data_cached(state, &data)?;
    }
    drop(guard);
    let Some(resolved) = resolved else {
        return Ok(None);
    };
    Ok(Some(TaskDispatchSessionResolved {
        model_config_id: selected_api.id.clone(),
        department_id,
        agent_id,
        conversation_id: resolved.conversation_id,
        target_scope: resolved.target_scope,
        fallback_to_main: resolved.fallback_to_main,
    }))
}

fn task_conversation_is_busy(state: &AppState, conversation_id: &str) -> Result<bool, String> {
    Ok(get_conversation_runtime_state(state, conversation_id)? != MainSessionState::Idle)
}

fn task_conversation_last_message_is_system_persona(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let data = state_read_app_data_cached(state)?;
    Ok(data
        .conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)
        .and_then(|conversation| conversation.messages.last())
        .and_then(|message| message.speaker_agent_id.as_deref())
        .map(str::trim)
        == Some(SYSTEM_PERSONA_ID))
}

fn task_is_due(entry: &TaskRecordStored, now: OffsetDateTime) -> bool {
    if entry.completion_state != TASK_STATE_ACTIVE {
        return false;
    }
    if entry.trigger.run_at_utc.is_none() {
        if let Some(end_at_utc) = entry.trigger.end_at_utc.as_deref().and_then(parse_rfc3339_time) {
            if now > end_at_utc {
                return false;
            }
        }
        return if let Some(last) = entry.last_triggered_at_utc.as_deref().and_then(parse_rfc3339_time) {
            now >= last + time::Duration::seconds(TASK_IMMEDIATE_RETRY_SECONDS)
        } else {
            true
        };
    }
    if entry.trigger.every_minutes.unwrap_or(0.0) <= 0.0 {
        return entry
            .trigger
            .run_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
            .map(|run_at_utc| now >= run_at_utc && entry.last_triggered_at_utc.is_none())
            .unwrap_or(false);
    }
    let Some(run_at_utc) = entry.trigger.run_at_utc.as_deref().and_then(parse_rfc3339_time) else {
        return false;
    };
    if now < run_at_utc {
        return false;
    }
    let Some(end_at_utc) = entry.trigger.end_at_utc.as_deref().and_then(parse_rfc3339_time) else {
        return false;
    };
    if now > end_at_utc {
        return false;
    }
    let Some(every) = entry
        .trigger
        .every_minutes
        .and_then(task_every_minutes_to_duration)
    else {
        return false;
    };
    if let Some(last) = entry.last_triggered_at_utc.as_deref().and_then(parse_rfc3339_time) {
        let next = last + every;
        next <= end_at_utc && now >= next
    } else {
        true
    }
}

fn task_build_board_snapshot(data_path: &PathBuf) -> Result<TaskBoardSnapshot, String> {
    let tasks = task_store_list_tasks(data_path)?;
    Ok(TaskBoardSnapshot {
        tasks: tasks
            .into_iter()
            .filter(|item| item.completion_state == TASK_STATE_ACTIVE)
            .take(TASK_MAX_BOARD_ITEMS)
            .collect(),
    })
}

fn build_hidden_task_board_block(state: &AppState) -> Option<String> {
    let snapshot = task_build_board_snapshot(&state.data_path).ok()?;
    if snapshot.tasks.is_empty() {
        return None;
    }
    let mut lines = Vec::<String>::new();
    lines.push(format!("currentLocalTime: {}", now_local_rfc3339()));
    lines.push("timeFormatNote: all task times below use local RFC3339 with timezone offset; copy the same format directly when writing runAtLocal".to_string());
    lines.push(format!("activeTaskCount: {}", snapshot.tasks.len()));
    for (idx, task) in snapshot.tasks.iter().enumerate() {
        let task_no = idx + 1;
        lines.push(format!("task[{task_no}].id: {}", task.task_id));
        lines.push(format!("task[{task_no}].goal: {}", task.goal.trim()));
        if !task.todo.trim().is_empty() {
            lines.push(format!("task[{task_no}].how: {}", task.todo.trim()));
        }
        if !task.why.trim().is_empty() {
            lines.push(format!("task[{task_no}].why: {}", task.why.trim()));
        }
        if let Some(run_at_local) = task.trigger.run_at_local.as_deref() {
            lines.push(format!("task[{task_no}].runAtLocal: {}", run_at_local));
        }
        if let Some(end_at_local) = task.trigger.end_at_local.as_deref() {
            lines.push(format!("task[{task_no}].endAtLocal: {}", end_at_local));
        }
        if let Some(next_run_at_local) = task.trigger.next_run_at_local.as_deref() {
            lines.push(format!("task[{task_no}].nextRunAtLocal: {}", next_run_at_local));
        }
    }
    Some(prompt_xml_block("task board", lines.join("\n")))
}

fn build_task_trigger_hidden_prompt(task: &TaskRecordStored) -> String {
    let mut lines = Vec::<String>::new();
    let goal = task_goal_from_legacy_fields(&task.title, &task.goal);
    let why = task_why_from_legacy_record(task);
    let todo = task_todo_from_legacy_fields(&task.status_summary, &task.todos);
    lines.push(format!("task_id: {}", task.task_id.trim()));
    lines.push(format!("target: {}", goal.trim()));
    if !todo.trim().is_empty() {
        lines.push(format!("how: {}", todo.trim()));
    }
    if !why.trim().is_empty() {
        lines.push(format!("why: {}", why.trim()));
    }
    if let Some(run_at_utc) = task.trigger.run_at_utc.as_deref() {
        lines.push(format!(
            "start_at: {}",
            format_utc_storage_time_to_local_rfc3339(run_at_utc)
        ));
    }
    if let Some(end_at_utc) = task.trigger.end_at_utc.as_deref() {
        lines.push(format!(
            "end_at: {}",
            format_utc_storage_time_to_local_rfc3339(end_at_utc)
        ));
    }
    if let Some(every_minutes) = task.trigger.every_minutes.filter(|value| *value > 0.0) {
        lines.push(format!("every: {}", every_minutes));
    }
    lines.push(String::new());
    lines.push("请立刻继续推进这个任务，直到任务全部成功或者明确无法完成。".to_string());
    lines.push("不管成功与否，最终都必须调用 task 工具使任务 complete。".to_string());
    lines.push(format!(
        "成功时调用：{{\"action\":\"complete\",\"task_id\":\"{}\",\"completion_state\":\"completed\",\"completion_conclusion\":\"<简洁说明最终结果>\"}}",
        task.task_id.trim()
    ));
    lines.push(format!(
        "失败或明确无法完成时调用：{{\"action\":\"complete\",\"task_id\":\"{}\",\"completion_state\":\"failed_completed\",\"completion_conclusion\":\"<简洁说明失败原因或阻塞点>\"}}",
        task.task_id.trim()
    ));
    format!("<task_remind>\n{}\n</task_remind>", lines.join("\n"))
}

fn build_task_trigger_provider_meta(task: &TaskRecordStored) -> Value {
    let trigger_view = task_trigger_view_from_stored(&task.trigger);
    let goal = task_goal_from_legacy_fields(&task.title, &task.goal);
    let why = task_why_from_legacy_record(task);
    let todo = task_todo_from_legacy_fields(&task.status_summary, &task.todos);
    serde_json::json!({
        "messageKind": "task_trigger",
        "hiddenPromptText": build_task_trigger_hidden_prompt(task),
        "taskTrigger": {
            "taskId": task.task_id,
            "goal": goal.trim(),
            "how": todo.trim(),
            "why": why.trim(),
            "runAtLocal": trigger_view.run_at_local,
            "endAtLocal": trigger_view.end_at_local,
            "nextRunAtLocal": trigger_view.next_run_at_local,
            "everyMinutes": trigger_view.every_minutes,
        }
    })
}

async fn task_dispatch_due_task(
    state: &AppState,
    task: &TaskRecordStored,
    session: &TaskDispatchSessionResolved,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    task_store_mark_triggered(&state.data_path, &task.task_id)?;
    if let Some(requested) = task
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if session.fallback_to_main {
            eprintln!(
                "[任务调度] 原会话不可用，回退到主会话: task_id={}, requested_conversation_id={}, fallback_conversation_id={}",
                task.task_id,
                requested,
                session.conversation_id
            );
        } else {
            eprintln!(
                "[任务调度] 会话{}的任务{}，投递中",
                session.conversation_id,
                task.task_id
            );
        }
    }

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
    let event_id = Uuid::new_v4().to_string();
    let request_id = format!("task-dispatch-{}", Uuid::new_v4());
    let mut runtime_context = runtime_context_new(
        "task_trigger",
        if session.fallback_to_main {
            "task_due_fallback_to_main"
        } else {
            "task_due"
        },
    );
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = task
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    runtime_context.target_conversation_id = Some(session.conversation_id.clone());
    runtime_context.root_conversation_id = runtime_context
        .origin_conversation_id
        .clone()
        .or_else(|| Some(session.conversation_id.clone()));
    runtime_context.executor_agent_id = Some(session.agent_id.clone());
    runtime_context.executor_department_id = Some(session.department_id.clone());
    runtime_context.model_config_id = Some(session.model_config_id.clone());
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: session.conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::Task,
        messages: vec![task_message],
        activate_assistant: true,
        session_info: ChatSessionInfo {
            department_id: session.department_id.clone(),
            agent_id: session.agent_id.clone(),
        },
        runtime_context: Some(runtime_context.clone()),
        sender_info: None,
    };

    let trigger_label = if task.trigger.run_at_utc.is_none() {
        "immediate"
    } else if task.trigger.every_minutes.unwrap_or(0.0) > 0.0 {
        "repeat"
    } else {
        "once"
    };
    let todo_count = task_legacy_todos_from_todo(&task_todo_from_legacy_fields(&task.status_summary, &task.todos)).len();
    let task_goal = task_goal_from_legacy_fields(&task.title, &task.goal);

    // 入队
    match ingress_chat_event(state, event) {
        Ok(ingress) => {
            // 异步触发处理：直写或排队由 ingress 判定，排队仅在确实滞留时通知前端。
            let (outcome, note_prefix) = match &ingress {
                ChatEventIngress::Direct(_) => ("sent", "任务已发送"),
                ChatEventIngress::Queued { .. } => ("queued", "任务已入队"),
            };
            trigger_chat_event_after_ingress(state, ingress);

            let duration_ms = started_at.elapsed().as_millis();
            task_store_insert_run_log(
                &state.data_path,
                &task.task_id,
                outcome,
                &format!(
                    "{}，requestId={}，dispatchId={}，goal={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，everyMinutes={}，durationMs={}，targetScope={}，fallbackToMain={}",
                    note_prefix,
                    request_id,
                    event_id,
                    task_goal.trim(),
                    session.conversation_id,
                    trigger_label,
                    todo_count,
                    task.trigger.run_at_utc.is_some(),
                    task.trigger.every_minutes.unwrap_or(0.0),
                    duration_ms
                    ,
                    session.target_scope,
                    session.fallback_to_main
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
                    "任务发送失败，requestId={}，dispatchId={}，goal={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，everyMinutes={}，durationMs={}，targetScope={}，fallbackToMain={}，error={}",
                    request_id,
                    event_id,
                    task_goal.trim(),
                    session.conversation_id,
                    trigger_label,
                    todo_count,
                    task.trigger.run_at_utc.is_some(),
                    task.trigger.every_minutes.unwrap_or(0.0),
                    duration_ms,
                    session.target_scope,
                    session.fallback_to_main,
                    err
                ),
            )?;
            Err(err)
        }
    }
}

fn task_build_dispatch_candidates(
    state: &AppState,
    tasks: Vec<TaskRecordStored>,
    now: OffsetDateTime,
) -> Result<Vec<TaskDispatchCandidate>, String> {
    let mut due_tasks = tasks
        .into_iter()
        .filter(|item| task_is_due(item, now))
        .collect::<Vec<_>>();
    due_tasks.sort_by_key(|item| item.order_index);

    let mut used_conversation_ids = std::collections::HashSet::<String>::new();
    let mut candidates = Vec::<TaskDispatchCandidate>::new();
    for task in due_tasks {
        let Some(session) = task_resolve_dispatch_session(state, &task)? else {
            continue;
        };
        if used_conversation_ids.insert(session.conversation_id.clone()) {
            candidates.push(TaskDispatchCandidate { task, session });
        }
    }
    Ok(candidates)
}

async fn task_scheduler_tick(state: &AppState) -> Result<(), String> {
    let tasks = task_store_list_task_records(&state.data_path)?;
    let now = now_utc();
    let candidates = task_build_dispatch_candidates(state, tasks, now)?;
    for candidate in candidates {
        if task_conversation_is_busy(state, &candidate.session.conversation_id)? {
            continue;
        }
        if task_conversation_last_message_is_system_persona(state, &candidate.session.conversation_id)? {
            continue;
        }
        task_dispatch_due_task(state, &candidate.task, &candidate.session).await?;
    }
    Ok(())
}

fn start_task_scheduler(state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let tick_started_at = std::time::Instant::now();
            if let Err(err) = task_scheduler_tick(&state).await {
                eprintln!(
                    "[任务调度] 调度轮询失败，error={}，durationMs={}，dataPath={}",
                    err,
                    tick_started_at.elapsed().as_millis(),
                    state.data_path.to_string_lossy()
                );
            }
            tokio::time::sleep(std::time::Duration::from_secs(TASK_SCHEDULER_INTERVAL_SECONDS)).await;
        }
    });
}


