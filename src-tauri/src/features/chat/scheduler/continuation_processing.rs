fn goal_continue_turn_for_conversation(
    conversation: &Conversation,
    goal_id: &str,
) -> usize {
    conversation
        .messages
        .iter()
        .filter(|message| {
            message
                .provider_meta
                .as_ref()
                .and_then(|meta| meta.get("messageKind"))
                .and_then(Value::as_str)
                .map(str::trim)
                == Some("goal_continue")
                && message
                    .provider_meta
                    .as_ref()
                    .and_then(|meta| meta.get("goalId"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    == Some(goal_id)
        })
        .count()
        + 1
}

fn build_goal_continue_message(
    goal: &ConversationGoalState,
    goal_turn: usize,
    now: String,
) -> ChatMessage {
    let hidden_prompt_text = render_goal_continue_hidden_prompt(goal, &now);
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now,
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text {
            text: GOAL_CONTINUE_DISPLAY_TEXT.to_string(),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "messageKind": "goal_continue",
            "goalId": goal.goal_id,
            "goalTurn": goal_turn,
            "objective": goal.objective,
            "status": goal.status,
            "startedAt": goal.started_at,
            "hiddenPromptText": hidden_prompt_text,
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

fn maybe_enqueue_goal_continue_after_idle(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() || !conversation_is_idle_for_goal_fallback(state, conversation_id)? {
        return Ok(false);
    }
    if goal_continue_is_suppressed(state, conversation_id)? {
        return Ok(false);
    }
    let conversation_meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(conversation_meta)
            if conversation_meta.status.trim() != "archived"
                && conversation_meta
                    .archived_at
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none() => conversation_meta,
        _ => return Ok(false),
    };
    let Some(goal) = conversation_meta
        .active_goal
        .as_ref()
        .filter(|goal| conversation_goal_is_active(goal))
        .cloned()
    else {
        return Ok(false);
    };
    let conversation = conversation_service_v2().get_conversation_prompt_context(state, conversation_id)?;
    if matches!(
        runtime_tool_origin_scope_from_conversation(state, &conversation),
        RuntimeToolOriginScope::RemoteGroup | RuntimeToolOriginScope::RemoteUnknown
    ) {
        runtime_log_warn(format!(
            "[目标续跑] 跳过，任务=goal_continue，conversation_id={}，goal_id={}，原因=远程群聊及其来源会话禁止 Goal",
            conversation_id, goal.goal_id
        ));
        return Ok(false);
    }
    let goal_turn = goal_continue_turn_for_conversation(&conversation, &goal.goal_id);
    let now = now_iso();
    let event_id = format!("goal-continue-{}", Uuid::new_v4());
    let request_id = format!("goal-continue-request-{}", Uuid::new_v4());
    let message = build_goal_continue_message(&goal, goal_turn, now.clone());
    let mut runtime_context = runtime_context_new("goal_continue", "active_goal");
    runtime_context.request_id = Some(request_id);
    runtime_context.dispatch_id = Some(format!("goal-continue-dispatch-{}", Uuid::new_v4()));
    runtime_context.origin_conversation_id = Some(conversation_id.to_string());
    runtime_context.target_conversation_id = Some(conversation_id.to_string());
    runtime_context.root_conversation_id = conversation
        .root_conversation_id
        .clone()
        .or_else(|| Some(conversation_id.to_string()));
    runtime_context.executor_agent_id = Some(conversation.agent_id.clone());
    let event = ChatPendingEvent {
        id: event_id,
        conversation_id: conversation_id.to_string(),
        created_at: now,
        source: ChatEventSource::System,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![message],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: conversation.agent_id,
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    let ingress = ingress_chat_event(state, event)?;
    runtime_log_info(format!(
        "[目标续跑] 开始，任务=goal_continue，conversation_id={}，goal_id={}，goal_turn={}",
        conversation_id, goal.goal_id, goal_turn
    ));
    trigger_chat_event_after_ingress_with_delay(state, ingress, std::time::Duration::from_secs(1));
    Ok(true)
}

async fn process_claimed_conversation_batch(
    state: &AppState,
    conversation_id: &str,
    events: Vec<ChatPendingEvent>,
) -> Result<(), String> {
    let result = process_conversation_batch(state, conversation_id, events).await;
    if let Err(release_err) = release_conversation_processing_claim(state, conversation_id) {
        runtime_log_error(format!(
            "[聊天调度] 释放会话处理声明失败: conversation_id={}, error={}",
            conversation_id, release_err
        ));
    }
    emit_chat_queue_snapshot(state);
    trigger_pending_guided_queue_processing(state);
    if conversation_has_guided_queue_events(state, conversation_id).unwrap_or(false) {
        trigger_guided_queue_processing(state, conversation_id);
    } else if conversation_has_pending_queue_events(state, conversation_id).unwrap_or(false) {
        trigger_chat_queue_processing(state);
    } else if result.is_ok()
        && maybe_enqueue_goal_continue_after_idle(state, conversation_id).unwrap_or(false)
    {
        trigger_chat_queue_processing(state);
    } else if result.is_ok()
        && maybe_enqueue_overdue_task_after_idle(state, conversation_id).unwrap_or(false)
    {
        trigger_chat_queue_processing(state);
    } else {
        trigger_chat_queue_processing(state);
    }
    result
}

fn resolve_activation_reason(runtime_context: &RuntimeContext) -> String {
    let dispatch_reason = runtime_context
        .dispatch_reason
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    match dispatch_reason {
        "user_send" => "user_send",
        "task_due" => "task_due",
        "remote_im_followup" => "remote_im_followup",
        "active_goal" => "active_goal",
        "context_compaction_followup" => "context_compaction_followup",
        "after_auto_compaction" | "after_tool_continue_compaction" => {
            "context_compaction_followup"
        }
        "guided_queue" => "guided_queue",
        "delegate_continue" => "delegate_continue",
        _ => "queue_dispatch",
    }
    .to_string()
}

async fn process_guided_queue_after_round(
    state: &AppState,
    conversation_id: &str,
    guided_events: Vec<ChatPendingEvent>,
) -> Result<bool, String> {
    if guided_events.is_empty() {
        return Ok(false);
    }
    let guided_event_count = guided_events.len();
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[引导投送] 开始，任务=process_guided_queue_after_round，conversation_id={}，event_count={}",
        conversation_id,
        guided_event_count
    ));
    process_conversation_batch(state, conversation_id, guided_events).await?;
    runtime_log_info(format!(
        "[引导投送] 完成，任务=process_guided_queue_after_round，conversation_id={}，event_count={}，duration_ms={}",
        conversation_id,
        guided_event_count,
        started_at.elapsed().as_millis()
    ));
    Ok(true)
}

async fn process_guided_queue_when_idle(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let guided_events = claim_guided_queue_events_for_conversation(state, conversation_id)?;
    if guided_events.is_empty() {
        return Ok(());
    }
    emit_chat_queue_snapshot(state);

    let result = process_guided_queue_after_round(state, conversation_id, guided_events).await;
    if let Err(release_err) = release_conversation_processing_claim(state, conversation_id) {
        runtime_log_warn(format!(
            "[引导投送] 失败，任务=release_guided_claim，conversation_id={}，error={}",
            conversation_id, release_err
        ));
    }
    if let Err(state_err) =
        set_conversation_runtime_state_and_emit(state, conversation_id, MainSessionState::Idle)
    {
        runtime_log_warn(format!(
            "[引导投送] 失败，任务=restore_guided_runtime_state，conversation_id={}，error={}",
            conversation_id, state_err
        ));
    }
    emit_chat_queue_snapshot(state);
    trigger_pending_guided_queue_processing(state);
    if conversation_has_guided_queue_events(state, conversation_id).unwrap_or(false) {
        trigger_guided_queue_processing(state, conversation_id);
    } else if result.is_ok()
        && maybe_enqueue_overdue_task_after_idle(state, conversation_id).unwrap_or(false)
    {
        trigger_chat_queue_processing(state);
    } else {
        trigger_chat_queue_processing(state);
    }
    result.map(|_| ())
}

pub(crate) fn trigger_guided_queue_processing(state: &AppState, conversation_id: &str) {
    let state_clone = state.clone();
    let conversation_id = conversation_id.to_string();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = process_guided_queue_when_idle(&state_clone, &conversation_id).await {
            runtime_log_warn(format!(
                "[引导投送] 失败，任务=trigger_guided_queue_processing，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
    });
}

fn trigger_pending_guided_queue_processing(state: &AppState) {
    let conversation_ids = lock_conversation_runtime_slots(state)
        .map(|slots| {
            slots
                .iter()
                .filter(|(_, slot)| slot.pending_queue.iter().any(|event| event.queue_mode == ChatQueueMode::Guided))
                .map(|(conversation_id, _)| conversation_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for conversation_id in conversation_ids {
        trigger_guided_queue_processing(state, &conversation_id);
    }
}
