fn task_conversation_meta_available_for_dispatch(
    conversation_meta: &ConversationMetaView,
) -> bool {
    conversation_meta.status.trim() != "archived"
        && conversation_meta
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE
        && conversation_meta.id.trim() != SYSTEM_NOTIFICATION_CONVERSATION_ID
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_SYSTEM_NOTIFICATION
}

#[derive(Debug, Clone)]
struct TaskResolvedConversation {
    conversation_id: String,
    target_scope: String,
    system_task: bool,
}

#[derive(Debug, Clone)]
struct TaskDispatchSessionResolved {
    model_config_id: String,
    agent_id: String,
    conversation_id: String,
    target_scope: String,
    system_task: bool,
}

#[derive(Debug, Clone)]
struct TaskDispatchCandidate {
    task: TaskRecordStored,
    session: TaskDispatchSessionResolved,
}

#[derive(Debug, Clone)]
struct TaskDispatchSkipContext {
    request_id: String,
    dispatch_id: String,
    task_goal: String,
    conversation_id: String,
    trigger_label: String,
    todo_count: usize,
    has_run_at: bool,
    cron_expression: String,
    duration_ms: u128,
    target_scope: String,
    system_task: bool,
}

fn task_target_scope_for_conversation_meta(
    conversation_meta: &ConversationMetaView,
) -> &'static str {
    if conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT {
        TASK_TARGET_SCOPE_CONTACT
    } else {
        TASK_TARGET_SCOPE_DESKTOP
    }
}

fn task_resolve_dispatch_conversation(
    state: &AppState,
    requested_conversation_id: Option<&str>,
) -> Result<Option<TaskResolvedConversation>, String> {
    let requested = task_normalize_bound_conversation_id(requested_conversation_id);
    if task_conversation_id_is_system_notification(&requested) {
        task_ensure_system_notification_conversation(state)?;
        return Ok(Some(TaskResolvedConversation {
            conversation_id: SYSTEM_NOTIFICATION_CONVERSATION_ID.to_string(),
            target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
            system_task: true,
        }));
    }

    if let Ok(conversation_meta) = conversation_service_v2().get_conversation_meta(state, &requested) {
        if task_conversation_meta_available_for_dispatch(&conversation_meta) {
            return Ok(Some(TaskResolvedConversation {
                conversation_id: conversation_meta.id.to_string(),
                target_scope: task_target_scope_for_conversation_meta(&conversation_meta)
                    .to_string(),
                system_task: false,
            }));
        }
    }
    Ok(None)
}

fn task_resolve_dispatch_session(
    state: &AppState,
    task: &TaskRecordStored,
) -> Result<Option<TaskDispatchSessionResolved>, String> {
    let requested_conversation_id = task
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let resolved = task_resolve_dispatch_conversation(state, requested_conversation_id)?;
    let Some(resolved) = resolved else {
        return Ok(None);
    };
    let existing_agent_id = task
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let agent_id = if let Some(agent_id) = existing_agent_id {
        task_validate_agent_for_write(state, agent_id)?
    } else if resolved.system_task {
        task_default_agent_for_write(state)?
    } else if let Some(agent_id) =
        task_agent_from_conversation_for_write(state, &resolved.conversation_id)?
    {
        agent_id
    } else {
        task_default_agent_for_write(state)?
    };
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let app_config = runtime_snapshot.config.clone();
    let agent = runtime_agent_by_id(&runtime_snapshot, &agent_id)
        .ok_or_else(|| format!("任务绑定人格不存在：{agent_id}"))?;
    let model_config_id = agent_primary_chat_api_config_id(&app_config, agent)
        .ok_or_else(|| format!("任务绑定人格没有可用模型：{agent_id}"))?;
    Ok(Some(TaskDispatchSessionResolved {
        model_config_id,
        agent_id,
        conversation_id: resolved.conversation_id,
        target_scope: resolved.target_scope,
        system_task: resolved.system_task,
    }))
}

fn task_dispatch_block_reason(
    state: &AppState,
    _conversation_id: &str,
) -> Result<Option<&'static str>, String> {
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let claims = lock_conversation_processing_claims(state)?;
    let running_count = claims.len();
    if running_count >= CHAT_CONCURRENCY_LIMIT {
        return Ok(Some("chat_concurrency_limit"));
    }
    Ok(None)
}

fn task_scheduler_notify_changed(state: &AppState) {
    state.task_scheduler_notify.notify_one();
}

fn task_trigger_label(task: &TaskRecordStored) -> &'static str {
    if task
        .trigger
        .cron_expression
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        "cron"
    } else if task.trigger.legacy_every_minutes.is_some() {
        "legacy_every_minutes"
    } else {
        "once"
    }
}

fn task_dispatch_todo_count(task: &TaskRecordStored) -> usize {
    task_legacy_todos_from_todo(&task_todo_from_legacy_fields(&task.status_summary, &task.todos)).len()
}

fn build_task_trigger_hidden_prompt(task: &TaskRecordStored) -> String {
    let goal = task_goal_from_legacy_fields(&task.title, &task.goal);
    let why = task_why_from_legacy_record(task);
    let todo = task_todo_from_legacy_fields(&task.status_summary, &task.todos);
    let lines = if why.trim().is_empty() && todo.trim().is_empty() {
        vec![
            "背景：用户希望你能独立完成任务达成目标".to_string(),
            format!("目标：{}", goal.trim()),
            "要求：一直持续工作，直到达成目标，最后在当前会话进行工作汇报。".to_string(),
        ]
    } else {
        vec![
            format!("背景：{}", why.trim()),
            format!("目标：{}", goal.trim()),
            format!("要求：{}", todo.trim()),
        ]
    };
    format!("<task_remind>\n{}\n</task_remind>", lines.join("\n"))
}

fn build_task_trigger_provider_meta(task: &TaskRecordStored) -> Value {
    serde_json::json!({
        "messageKind": "task_trigger",
        "hiddenPromptText": build_task_trigger_hidden_prompt(task),
        "taskTrigger": {
            "taskId": task.task_id.trim(),
            "runAt": task.trigger.run_at_utc.as_deref().map(format_utc_storage_time_to_local_rfc3339),
            "nextRunAt": task.trigger.next_run_at_utc.as_deref().map(format_utc_storage_time_to_local_rfc3339),
            "cronExpression": task.trigger.cron_expression.as_deref().map(str::trim).filter(|value| !value.is_empty()),
            "endAt": task.trigger.end_at_utc.as_deref().map(format_utc_storage_time_to_local_rfc3339),
        }
    })
}

fn build_task_trigger_message(task: &TaskRecordStored) -> ChatMessage {
    let goal = task_goal_from_legacy_fields(&task.title, &task.goal);
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "system".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text {
            text: format!("任务提醒：{}", goal.trim()),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(build_task_trigger_provider_meta(task)),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

fn task_conversation_is_ready_for_immediate_dispatch(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    conversation_is_idle_for_goal_fallback(state, conversation_id)
}

fn task_enqueue_conversation_trigger(
    state: &AppState,
    task: &TaskRecordStored,
    session: &TaskDispatchSessionResolved,
) -> Result<ChatEventIngress, String> {
    let request_id = format!("task-dispatch-{}", Uuid::new_v4());
    let dispatch_id = format!("task-trigger-{}", Uuid::new_v4());
    let mut runtime_context = runtime_context_new("task_trigger", "task_due");
    runtime_context.request_id = Some(request_id);
    runtime_context.dispatch_id = Some(dispatch_id);
    runtime_context.origin_conversation_id = Some(session.conversation_id.clone());
    runtime_context.target_conversation_id = Some(session.conversation_id.clone());
    runtime_context.root_conversation_id = Some(session.conversation_id.clone());
    runtime_context.executor_agent_id = Some(session.agent_id.clone());
    runtime_context.model_config_id = Some(session.model_config_id.clone());
    let event = ChatPendingEvent {
        id: format!("task-event-{}", Uuid::new_v4()),
        conversation_id: session.conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::Task,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![build_task_trigger_message(task)],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: session.agent_id.clone(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    ingress_chat_event(state, event)
}

fn task_complete_one_time_dispatch_if_needed(
    state: &AppState,
    task: &TaskRecordStored,
) -> Result<(), String> {
    if !task_record_is_one_time(task) {
        return Ok(());
    }
    let changed = task_store_complete_one_time_dispatch(
        &state.data_path,
        &task.task_id,
    )?;
    if changed {
        runtime_log_info(format!(
            "[任务调度] 完成，任务=一次性任务已发起调度，task_id={}",
            task.task_id
        ));
        task_publish_changed_event(state, "completed", &task.task_id, None);
    }
    Ok(())
}

fn task_mark_dispatch_sent(state: &AppState, task: &TaskRecordStored) -> Result<(), String> {
    if task_record_is_one_time(task) {
        return task_complete_one_time_dispatch_if_needed(state, task);
    }
    task_store_mark_triggered(&state.data_path, &task.task_id)
}

fn task_mark_dispatch_skipped(
    state: &AppState,
    task: &TaskRecordStored,
    reason: &str,
    context: &TaskDispatchSkipContext,
) -> Result<(), String> {
    task_store_mark_skipped(
        &state.data_path,
        &task.task_id,
        "skipped",
        &format!(
            "任务已跳过，requestId={}，dispatchId={}，goal={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，cronExpression={}，durationMs={}，targetScope={}，systemTask={}，reason={}",
            context.request_id,
            context.dispatch_id,
            context.task_goal.trim(),
            context.conversation_id,
            context.trigger_label,
            context.todo_count,
            context.has_run_at,
            context.cron_expression,
            context.duration_ms,
            context.target_scope,
            context.system_task,
            reason
        ),
    )?;
    task_complete_one_time_dispatch_if_needed(state, task)
}

fn task_fail_missing_bound_conversation(
    state: &AppState,
    task: &TaskRecordStored,
) -> Result<(), String> {
    let conversation_id = task
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(SYSTEM_NOTIFICATION_CONVERSATION_ID);
    let changed = task_store_fail_active_task(
        &state.data_path,
        &task.task_id,
        TASK_BOUND_CONVERSATION_MISSING_CONCLUSION,
        &format!(
            "绑定会话不存在，任务已失败，taskId={}，conversationId={}",
            task.task_id,
            conversation_id
        ),
    )?;
    if changed {
        runtime_log_error(format!(
            "[任务调度] 失败，任务=绑定会话丢失，task_id={}，conversation_id={}",
            task.task_id,
            conversation_id
        ));
    }
    Ok(())
}

fn task_fail_unavailable_owner(
    state: &AppState,
    task: &TaskRecordStored,
    reason: &str,
) -> Result<(), String> {
    let changed = task_store_fail_active_task(
        &state.data_path,
        &task.task_id,
        TASK_BOUND_OWNER_UNAVAILABLE_CONCLUSION,
        &format!(
            "任务负责人不可用，任务已失败，taskId={}，reason={}",
            task.task_id,
            reason.trim()
        ),
    )?;
    if changed {
        runtime_log_error(format!(
            "[任务调度] 失败，任务=负责人不可用，task_id={}，reason={}",
            task.task_id,
            reason.trim()
        ));
    }
    Ok(())
}

fn task_is_due(entry: &TaskRecordStored, now: OffsetDateTime) -> bool {
    if entry.completion_state != TASK_STATE_ACTIVE {
        return false;
    }
    entry
        .trigger
        .next_run_at_utc
        .as_deref()
        .and_then(parse_rfc3339_time)
        .map(|next_run_at| now >= next_run_at)
        .unwrap_or(false)
}

fn task_build_board_snapshot(data_path: &PathBuf) -> Result<TaskBoardSnapshot, String> {
    let tasks = task_store_list_tasks(data_path)?;
    Ok(TaskBoardSnapshot {
        tasks: tasks
            .into_iter()
            .filter(|item| {
                item.completion_state == TASK_STATE_ACTIVE
                    && item
                        .conversation_id
                        .as_deref()
                        .map(task_conversation_id_is_system_notification)
                        != Some(true)
            })
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
    lines.push("timeFormatNote: all task times below use local RFC3339 with timezone offset; copy the same format directly when writing run_at or end_at".to_string());
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
        if let Some(run_at) = task.trigger.run_at.as_deref() {
            lines.push(format!("task[{task_no}].run_at: {}", run_at));
        }
        if let Some(cron_expression) = task.trigger.cron_expression.as_deref() {
            lines.push(format!("task[{task_no}].cron_expression: {}", cron_expression));
        }
        if let Some(end_at) = task.trigger.end_at.as_deref() {
            lines.push(format!("task[{task_no}].end_at: {}", end_at));
        }
        if let Some(next_run_at) = task.trigger.next_run_at.as_deref() {
            lines.push(format!("task[{task_no}].next_run_at: {}", next_run_at));
        }
    }
    Some(prompt_xml_block("task board", lines.join("\n")))
}

fn task_system_delegate_title(task: &TaskRecordStored) -> String {
    let goal = task_goal_from_legacy_fields(&task.title, &task.goal);
    let compact = goal.trim().chars().take(32).collect::<String>();
    if compact.trim().is_empty() {
        format!("系统任务：{}", task.task_id.trim())
    } else {
        format!("系统任务：{}", compact)
    }
}

fn build_system_task_delegate_instruction(task: &TaskRecordStored) -> String {
    format!(
        "{}\n\n这是系统任务，请在独立委托线程中完成，不要读取 `P-ai系统` 会话正文作为上下文。完成后直接汇报结果。",
        build_task_trigger_hidden_prompt(task),
    )
}

fn task_dispatch_system_delegate(
    state: &AppState,
    task: &TaskRecordStored,
    session: &TaskDispatchSessionResolved,
) -> Result<String, String> {
    task_ensure_system_notification_conversation(state)?;
    let title = task_system_delegate_title(task);
    let instruction = build_system_task_delegate_instruction(task);
    let delegate = delegate_create_record(
        state,
        DELEGATE_TOOL_KIND_DELEGATE,
        SYSTEM_NOTIFICATION_CONVERSATION_ID,
        None,
        &session.agent_id,
        &session.agent_id,
        &title,
        instruction,
        title.clone(),
        "完成系统任务，并直接汇报结果。".to_string(),
        false,
        vec![session.agent_id.clone()],
    )?;
    let delegate_id = delegate.delegate_id.clone();
    spawn_delegate_task(
        state.clone(),
        delegate,
        SYSTEM_NOTIFICATION_CONVERSATION_ID.to_string(),
        vec![session.model_config_id.clone()],
        None,
    );
    Ok(delegate_id)
}

async fn task_dispatch_due_task(
    state: &AppState,
    task: &TaskRecordStored,
    session: &TaskDispatchSessionResolved,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let trigger_label = task_trigger_label(task);
    let todo_count = task_dispatch_todo_count(task);
    let task_goal = task_goal_from_legacy_fields(&task.title, &task.goal);

    if session.system_task {
        let request_id = format!("task-dispatch-{}", Uuid::new_v4());
        let delegate_id = task_dispatch_system_delegate(state, task, session)?;
        task_mark_dispatch_sent(state, task)?;
        let duration_ms = started_at.elapsed().as_millis();
        task_store_insert_run_log(
            &state.data_path,
            &task.task_id,
            "sent",
            &format!(
                "系统任务已发起独立委托，requestId={}，delegateId={}，goal={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，cronExpression={}，durationMs={}，targetScope={}，systemTask=true",
                request_id,
                delegate_id,
                task_goal.trim(),
                SYSTEM_NOTIFICATION_CONVERSATION_ID,
                trigger_label,
                todo_count,
                task.trigger.run_at_utc.is_some(),
                task.trigger.cron_expression.as_deref().unwrap_or(""),
                duration_ms,
                session.target_scope
            ),
        )?;
        return Ok(());
    }

    if let Some(requested) = task
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_log_info(format!(
            "[任务调度] 会话{}的任务{}，投递中，requested_conversation_id={}",
            session.conversation_id,
            task.task_id,
            requested
        ));
    }
    let request_id = format!("task-dispatch-{}", Uuid::new_v4());
    let ingress = task_enqueue_conversation_trigger(state, task, session)?;
    let (sent, dispatch_kind) = match &ingress {
        ChatEventIngress::Direct(_) => (true, "direct"),
        ChatEventIngress::Queued { .. } => (true, "queued"),
    };
    if sent {
        task_mark_dispatch_sent(state, task)?;
    }
    let duration_ms = started_at.elapsed().as_millis();
    task_store_insert_run_log(
        &state.data_path,
        &task.task_id,
        "sent",
        &format!(
            "任务已投递原会话，requestId={}，goal={}，conversationId={}，trigger={}，todoCount={}，hasRunAt={}，cronExpression={}，durationMs={}，targetScope={}，systemTask=false，dispatchKind={}",
            request_id,
            task_goal.trim(),
            session.conversation_id,
            trigger_label,
            todo_count,
            task.trigger.run_at_utc.is_some(),
            task.trigger.cron_expression.as_deref().unwrap_or(""),
            duration_ms,
            session.target_scope,
            dispatch_kind
        ),
    )?;
    trigger_chat_event_after_ingress(state, ingress);
    Ok(())
}

fn task_skip_context_for_candidate_filter(
    task: &TaskRecordStored,
    session: &TaskDispatchSessionResolved,
) -> TaskDispatchSkipContext {
    TaskDispatchSkipContext {
        request_id: "task-candidate-skip".to_string(),
        dispatch_id: format!("task-skip-{}", Uuid::new_v4()),
        task_goal: task_goal_from_legacy_fields(&task.title, &task.goal),
        conversation_id: session.conversation_id.clone(),
        trigger_label: task_trigger_label(task).to_string(),
        todo_count: task_dispatch_todo_count(task),
        has_run_at: task.trigger.run_at_utc.is_some(),
        cron_expression: task.trigger.cron_expression.clone().unwrap_or_default(),
        duration_ms: 0,
        target_scope: session.target_scope.clone(),
        system_task: session.system_task,
    }
}

fn task_matches_conversation(task: &TaskRecordStored, conversation_id: &str) -> bool {
    task.conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        == Some(conversation_id.trim())
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

    let mut candidates = Vec::<TaskDispatchCandidate>::new();
    let mut used_conversation_ids = std::collections::HashSet::<String>::new();
    for task in due_tasks {
        let session = match task_resolve_dispatch_session(state, &task) {
            Ok(Some(session)) => session,
            Ok(None) => {
                task_fail_missing_bound_conversation(state, &task)?;
                continue;
            }
            Err(err) => {
                task_fail_unavailable_owner(state, &task, &err)?;
                continue;
            }
        };
        if !session.system_task
            && !task_conversation_is_ready_for_immediate_dispatch(state, &session.conversation_id)?
        {
            runtime_log_warn(format!(
                "[任务调度] 跳过，任务=会话忙碌等待收尾补检查，task_id={}，conversation_id={}",
                task.task_id, session.conversation_id
            ));
            continue;
        }
        if !session.system_task && !used_conversation_ids.insert(session.conversation_id.clone()) {
            continue;
        }
        if let Some(reason) = task_dispatch_block_reason(state, &session.conversation_id)? {
            let context = task_skip_context_for_candidate_filter(&task, &session);
            task_mark_dispatch_skipped(state, &task, reason, &context)?;
            continue;
        }
        candidates.push(TaskDispatchCandidate { task, session });
    }
    Ok(candidates)
}

fn task_due_dispatch_is_ready_now(
    state: &AppState,
    task: &TaskRecordStored,
) -> Result<bool, String> {
    if !task_is_due(task, now_utc()) {
        return Ok(false);
    }
    let Some(session) = task_resolve_dispatch_session(state, task)? else {
        return Ok(false);
    };
    if session.system_task {
        return Ok(true);
    }
    task_conversation_is_ready_for_immediate_dispatch(state, &session.conversation_id)
}

fn maybe_enqueue_overdue_task_after_idle(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() || !task_conversation_is_ready_for_immediate_dispatch(state, conversation_id)? {
        return Ok(false);
    }
    let tasks = task_store_list_task_records(&state.data_path)?;
    let now = now_utc();
    let candidates = task_build_dispatch_candidates(
        state,
        tasks.into_iter().filter(|task| task_matches_conversation(task, conversation_id)).collect(),
        now,
    )?;
    let Some(candidate) = candidates.into_iter().next() else {
        return Ok(false);
    };
    runtime_log_info(format!(
        "[任务调度] 开始，任务=会话收尾补发到点任务，task_id={}，conversation_id={}",
        candidate.task.task_id,
        conversation_id
    ));
    tauri::async_runtime::spawn({
        let state = state.clone();
        async move {
            if let Err(err) = task_dispatch_due_task(&state, &candidate.task, &candidate.session).await {
                runtime_log_warn(format!(
                    "[任务调度] 失败，任务=会话收尾补发到点任务，task_id={}，conversation_id={}，error={}",
                    candidate.task.task_id,
                    candidate.session.conversation_id,
                    err
                ));
            }
        }
    });
    Ok(true)
}

async fn task_scheduler_tick(state: &AppState) -> Result<(), String> {
    let tasks = task_store_list_task_records(&state.data_path)?;
    let now = now_utc();
    let candidates = task_build_dispatch_candidates(state, tasks, now)?;
    for candidate in candidates {
        task_dispatch_due_task(state, &candidate.task, &candidate.session).await?;
    }
    Ok(())
}

fn task_scheduler_next_wake_delay(state: &AppState) -> Result<Option<std::time::Duration>, String> {
    let tasks = task_store_list_task_records(&state.data_path)?;
    let now = now_utc();
    let mut next_future_due = None::<OffsetDateTime>;
    for task in tasks
        .iter()
        .filter(|task| task.completion_state == TASK_STATE_ACTIVE)
    {
        let Some(next_run_at) = task
            .trigger
            .next_run_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
        else {
            continue;
        };
        if next_run_at <= now {
            if task_due_dispatch_is_ready_now(state, task)? {
                return Ok(Some(std::time::Duration::ZERO));
            }
            continue;
        }
        next_future_due = Some(
            next_future_due
                .map(|current| current.min(next_run_at))
                .unwrap_or(next_run_at),
        );
    }
    let Some(next_due) = next_future_due else {
        return Ok(None);
    };
    let millis = (next_due - now).whole_milliseconds();
    let millis = u64::try_from(millis).unwrap_or(u64::MAX);
    Ok(Some(std::time::Duration::from_millis(millis)))
}

async fn task_scheduler_wait(state: &AppState) -> Result<(), String> {
    let fallback = std::time::Duration::from_secs(TASK_SCHEDULER_FALLBACK_SECONDS);
    let next_delay = task_scheduler_next_wake_delay(state)?;
    let wait_duration = next_delay.map(|delay| delay.min(fallback)).unwrap_or(fallback);
    if wait_duration.is_zero() {
        return Ok(());
    }

    let sleep = tokio::time::sleep(wait_duration);
    tokio::pin!(sleep);
    tokio::select! {
        _ = &mut sleep => {}
        _ = state.task_scheduler_notify.notified() => {}
    }
    Ok(())
}

fn start_task_scheduler(state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let tick_started_at = std::time::Instant::now();
            if let Err(err) = task_scheduler_tick(&state).await {
                runtime_log_error(format!(
                    "[任务调度] 调度轮询失败，error={}，durationMs={}，dataPath={}",
                    err,
                    tick_started_at.elapsed().as_millis(),
                    state.data_path.to_string_lossy()
                ));
            }
            if let Err(err) = task_scheduler_wait(&state).await {
                runtime_log_error(format!(
                    "[任务调度] 等待下一次触发失败，error={}，durationMs={}，dataPath={}",
                    err,
                    tick_started_at.elapsed().as_millis(),
                    state.data_path.to_string_lossy()
                ));
                tokio::time::sleep(std::time::Duration::from_secs(TASK_SCHEDULER_FALLBACK_SECONDS)).await;
            }
        }
    });
}
