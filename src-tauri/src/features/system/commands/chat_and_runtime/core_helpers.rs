fn inflight_chat_key(
    agent_id: &str,
    conversation_id: Option<&str>,
) -> String {
    match conversation_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(conversation_id) => format!("{}::{}", agent_id.trim(), conversation_id),
        None => agent_id.trim().to_string(),
    }
}

fn register_inflight_tool_abort_handle(
    state: &AppState,
    chat_key: &str,
    handle: AbortHandle,
) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(previous) = inflight.insert(chat_key.to_string(), handle) {
        previous.abort();
    }
    Ok(())
}

fn reset_inflight_completed_tool_history(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.insert(chat_key.to_string(), Vec::new());
    Ok(())
}

fn replace_inflight_completed_tool_history(
    state: &AppState,
    chat_key: &str,
    events: &[Value],
) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.insert(chat_key.to_string(), events.to_vec());
    Ok(())
}

fn inflight_completed_tool_history(
    state: &AppState,
    chat_key: &str,
) -> Result<Vec<Value>, String> {
    let inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    Ok(inflight.get(chat_key).cloned().unwrap_or_default())
}

fn clear_inflight_completed_tool_history(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn clear_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn abort_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(handle) = inflight.remove(chat_key) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn delegate_thread_chat_key(thread: &DelegateRuntimeThread) -> String {
    inflight_chat_key(
        &thread.target_agent_id,
        Some(&thread.conversation.id),
    )
}

fn abort_delegate_runtime_descendants_by_parent_session(
    state: &AppState,
    parent_chat_key: &str,
) -> Result<usize, String> {
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| thread.parent_chat_session_key.as_deref() == Some(parent_chat_key))
        .collect::<Vec<_>>();
    let mut aborted_count = 0usize;
    for thread in children {
        let child_chat_key = delegate_thread_chat_key(&thread);
        let aborted_chat = {
            let mut inflight = state
                .inflight_chat_abort_handles
                .lock()
                .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
            if let Some(handle) = inflight.remove(&child_chat_key) {
                handle.abort();
                true
            } else {
                false
            }
        };
        let aborted_tool = abort_inflight_tool_abort_handle(state, &child_chat_key)?;
        if aborted_chat || aborted_tool {
            aborted_count += 1;
            eprintln!(
                "[聊天] 已中止同步委托子会话: parent_session={}, child_session={}, delegate_id={}",
                parent_chat_key,
                child_chat_key,
                thread.delegate_id
            );
        }
        aborted_count += abort_delegate_runtime_descendants_by_parent_session(state, &child_chat_key)?;
    }
    Ok(aborted_count)
}

fn model_reply_has_visible_content(reply: &ModelReply) -> bool {
    !reply.assistant_text.trim().is_empty()
        || !reply.reasoning_standard.trim().is_empty()
        || !reply.reasoning_inline.trim().is_empty()
        || reply.assistant_provider_meta.is_some()
        || !reply.tool_history_events.is_empty()
        || reply.suppress_assistant_message
}

fn effective_prompt_tokens_from_provider(
    estimated_prompt_tokens: u64,
    trusted_input_tokens: Option<u64>,
) -> (u64, &'static str) {
    let estimated = estimated_prompt_tokens.max(1);
    let Some(provider) = trusted_input_tokens.filter(|value| *value > 0) else {
        return (estimated_prompt_tokens, "estimate_no_provider");
    };
    let gap = provider.abs_diff(estimated) as f64 / estimated as f64;
    if gap > 0.5 {
        return (provider.max(estimated_prompt_tokens), "max_large_gap");
    }
    (provider, "provider")
}

