#[cfg(test)]
fn resolve_unarchived_conversation_index_with_fallback(
    data: &mut AppData,
    state: &AppState,
    app_config: &AppConfig,
    effective_agent_id: &str,
    requested_conversation_id: Option<&str>,
) -> Result<usize, String> {
    if let Some(conversation_id) = requested_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(idx) = data.conversations.iter().position(|item| {
            item.id == conversation_id
                && conversation_visible_in_foreground_lists(item)
        }) {
            return Ok(idx);
        }
        runtime_log_info(format!(
            "[存储] 解析对话索引 请求的会话不存在 终止本次读取 会话='{}' 人格='{}'",
            conversation_id, effective_agent_id
        ));
        return Err(format!(
            "Requested conversation not found: {conversation_id}"
        ));
    }

    if let Some(existing_idx) = main_conversation_index(data, state, effective_agent_id)? {
        return Ok(existing_idx);
    }

    if let Some(existing_idx) = latest_active_conversation_index(data, "", effective_agent_id) {
        return Ok(existing_idx);
    }

    let api_config = resolve_selected_api_config(app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;
    Ok(ensure_active_conversation_index(
        data,
        state,
        &api_config.id,
        effective_agent_id,
    ))
}

fn ensure_chat_store_conversation_readable(
    state: &AppState,
    conversation_id: &str,
    store_paths: &message_store::MessageStorePaths,
) -> Result<(), String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    // 新建会话可能仍在 pending 队列尚未落盘：先同步发布到 V4 存储，保证读侧就绪。
    publish_pending_new_conversation_if_needed(
        state,
        normalized_conversation_id,
        store_paths,
    )?;
    let initial_status = message_store::chat_store_read_status(store_paths);
    if matches!(initial_status, Ok(Some(_))) {
        return Ok(());
    }
    if let Err(err) = &initial_status {
        runtime_log_warn(format!(
            "[存储] 仓库状态首次读取失败，将在会话锁内重试 原因={}",
            err
        ));
    }
    with_conversation_mutation(
        state,
        normalized_conversation_id,
        "ensure_chat_store_conversation_readable",
        || match message_store::chat_store_read_status(store_paths) {
            Ok(Some(_)) => Ok(()),
            // 会话尚未落盘（persist 排队中或已删除）按空仓库处理：V4 读取接口对
            // 空仓库返回空数据，读写方各自按“无消息”语义收敛，不再拒绝。
            Ok(None) => Ok(()),
            Err(err) => Err(format!(
                "读取会话消息仓库状态失败，conversation_id={normalized_conversation_id}，error={err}"
            )),
        },
    )
}

fn build_foreground_conversation_snapshot_from_conversation(
    state: &AppState,
    conversation: &Conversation,
    recent_limit: usize,
) -> Result<ForegroundConversationSnapshotCore, String> {
    let (messages, has_more_history) = build_foreground_snapshot_recent_messages(
        state,
        conversation,
        recent_limit,
    )?;
    Ok(ForegroundConversationSnapshotCore {
        conversation_id: conversation.id.clone(),
        messages,
        last_message_id: conversation.messages.last().map(|message| message.id.clone()),
        has_more_history,
        runtime_state: unarchived_conversation_runtime_state(state, &conversation.id),
        current_todo: conversation_current_todo_text(conversation),
        current_todos: conversation.current_todos.clone(),
        preferred_api_config_id: conversation.preferred_api_config_id.clone(),
        active_goal: goal_active_goal_from_conversation(conversation),
    })
}

fn build_foreground_conversation_snapshot_from_meta_view(
    state: &AppState,
    conversation_meta: &ConversationMetaView,
    recent_limit: usize,
) -> Result<ForegroundConversationSnapshotCore, String> {
    let conversation_id = conversation_meta.id.to_string();
    let paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
    ensure_chat_store_conversation_readable(state, &conversation_id, &paths)?;
    let (messages, has_more_history) = if let Some(page) =
        message_store::chat_store_read_recent_messages_page_cached(&paths, recent_limit)?
    {
        if let Err(err) = conversation_service_v2().retain_message_store_block_cache_whitelist(state) {
            runtime_log_warn(format!(
                "[存储] 警告 任务=retain_message_store_block_cache_whitelist 原因={}",
                err
            ));
        }
        (page.messages, page.has_more)
    } else {
        let messages = message_store::chat_store_read_all_messages(&paths)?.unwrap_or_default();
        let total_messages = messages.len();
        let start = total_messages.saturating_sub(recent_limit);
        (messages[start..].to_vec(), start > 0)
    };
    let last_message_id = conversation_meta
        .last_message_id
        .clone()
        .or_else(|| messages.last().map(|message| message.id.clone()));
    Ok(ForegroundConversationSnapshotCore {
        conversation_id: conversation_id.clone(),
        messages,
        last_message_id,
        has_more_history,
        runtime_state: unarchived_conversation_runtime_state(state, &conversation_id),
        current_todo: conversation_current_todo_text_from_items(&conversation_meta.current_todos),
        current_todos: conversation_meta.current_todos.clone(),
        preferred_api_config_id: conversation_meta.preferred_api_config_id.clone(),
        active_goal: conversation_meta.active_goal.clone(),
    })
}

fn build_foreground_snapshot_recent_messages(
    state: &AppState,
    conversation: &Conversation,
    recent_limit: usize,
) -> Result<(Vec<ChatMessage>, bool), String> {
    let paths = message_store::message_store_paths(&state.data_path, &conversation.id)?;
    if let Some(page) =
        message_store::chat_store_read_recent_messages_page_cached(&paths, recent_limit)?
    {
        if let Err(err) = conversation_service_v2().retain_message_store_block_cache_whitelist(state) {
            runtime_log_warn(format!(
                "[存储] 警告 任务=retain_message_store_block_cache_whitelist 会话={} 原因={}",
                conversation.title,
                err
            ));
        }
        return Ok((page.messages, page.has_more));
    }
    let total_messages = conversation.messages.len();
    let start = total_messages.saturating_sub(recent_limit);
    Ok((conversation.messages[start..].to_vec(), start > 0))
}

fn provider_usage_prompt_tokens(usage: &Value) -> u64 {
    usage
        .get("promptTokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|item| u64::try_from(item).ok()))
        })
        .unwrap_or(0)
}

fn emit_provider_context_usage_update_from_conversation(
    state: &AppState,
    conversation: &Conversation,
    usage: &Value,
) {
    let prompt_tokens = provider_usage_prompt_tokens(usage);
    if prompt_tokens == 0 {
        return;
    }
    let app_config = match state_read_config_cached(state) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=推送真实上下文用量 会话={} 原因={}",
                conversation.title,
                err
            ));
            return;
        }
    };
    let Some(api_config) =
        resolve_selected_api_config(&app_config, conversation.preferred_api_config_id.as_deref())
    else {
        return;
    };
    let context_window_tokens = api_config.context_window_tokens.max(1);
    let context_usage_ratio = prompt_tokens as f64 / f64::from(context_window_tokens);
    let context_usage_percent = context_usage_ratio
        .mul_add(100.0, 0.0)
        .round()
        .clamp(0.0, 100.0) as u32;
    let message = serde_json::json!({
        "conversationId": conversation.id,
        "providerPromptTokens": prompt_tokens,
        "contextUsagePercent": context_usage_percent,
        "contextUsageRatio": context_usage_ratio,
        "contextWindowTokens": context_window_tokens,
        "source": "provider_prompt_tokens",
        "eventReason": "provider_request_usage",
    })
    .to_string();
    emit_assistant_delta_app_event(
        state,
        &conversation.id,
        &AssistantDeltaEvent {
            delta: String::new(),
            kind: Some("context_usage_update".to_string()),
            request_id: None,
            activation_id: None,
            phase_id: None,
            reason: Some("provider_request_usage".to_string()),
            tool_name: None,
            tool_call_id: None,
            tool_status: None,
            tool_args: None,
            message: Some(message),
            stream_cache: None,
        },
    );
}
