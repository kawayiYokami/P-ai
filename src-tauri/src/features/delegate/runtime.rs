const DELEGATE_RECENT_THREAD_LIMIT: usize = 10;

fn delegate_runtime_thread_build(
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    parent_chat_session_key: Option<String>,
) -> DelegateRuntimeThread {
    let mut conversation = build_conversation_record(
        target_api_config_id,
        &delegate.target_agent_id,
        &delegate.title,
        CONVERSATION_KIND_DELEGATE,
        Some(delegate.conversation_id.clone()),
        Some(delegate.delegate_id.clone()),
    );
    // 委托线程的唯一运行时标识直接使用 delegate_id，避免任何“猜当前会话”的路径。
    conversation.id = delegate.delegate_id.clone();
    conversation.created_at = delegate.created_at.clone();
    conversation.updated_at = delegate.updated_at.clone();
    conversation.last_user_at = None;
    conversation.last_assistant_at = None;
    DelegateRuntimeThread {
        delegate_id: delegate.delegate_id.clone(),
        root_conversation_id: delegate.conversation_id.clone(),
        target_agent_id: delegate.target_agent_id.clone(),
        title: delegate.title.clone(),
        call_stack: delegate.call_stack.clone(),
        parent_chat_session_key,
        archived_at: None,
        conversation,
    }
}

fn delegate_runtime_thread_create(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    parent_chat_session_key: Option<String>,
) -> Result<String, String> {
    let thread =
        delegate_runtime_thread_build(delegate, target_api_config_id, parent_chat_session_key);
    let thread_id = thread.delegate_id.clone();
    let mut guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    guard.insert(thread_id.clone(), thread);
    Ok(thread_id)
}

fn delegate_runtime_thread_get(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<Option<DelegateRuntimeThread>, String> {
    let guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    Ok(guard.get(delegate_id.trim()).cloned())
}

fn delegate_runtime_thread_modify<T, F>(
    app_state: &AppState,
    delegate_id: &str,
    modify: F,
) -> Result<T, String>
where
    F: FnOnce(&mut DelegateRuntimeThread) -> Result<T, String>,
{
    let mut guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    let thread = guard
        .get_mut(delegate_id.trim())
        .ok_or_else(|| format!("未找到委托线程，delegateId={delegate_id}"))?;
    modify(thread)
}

fn delegate_runtime_thread_list(app_state: &AppState) -> Result<Vec<DelegateRuntimeThread>, String> {
    let guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    Ok(guard.values().cloned().collect::<Vec<_>>())
}

fn delegate_recent_thread_list(app_state: &AppState) -> Result<Vec<DelegateRuntimeThread>, String> {
    let guard = app_state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
    Ok(guard.iter().cloned().collect::<Vec<_>>())
}

fn delegate_runtime_thread_archive(
    app_state: &AppState,
    delegate_id: &str,
    archived_at: &str,
) -> Result<(), String> {
    let mut active = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    let mut recent = app_state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
    let Some(mut thread) = active.remove(delegate_id.trim()) else {
        return Ok(());
    };
    thread.archived_at = Some(archived_at.to_string());
    recent.retain(|item| item.delegate_id != thread.delegate_id);
    recent.push_front(thread);
    while recent.len() > DELEGATE_RECENT_THREAD_LIMIT {
        recent.pop_back();
    }
    Ok(())
}

fn delegate_runtime_thread_get_any(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<Option<DelegateRuntimeThread>, String> {
    if let Some(thread) = delegate_runtime_thread_get(app_state, delegate_id)? {
        return Ok(Some(thread));
    }
    let recent = app_state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
    Ok(recent
        .iter()
        .find(|thread| thread.delegate_id == delegate_id.trim())
        .cloned())
}

fn delegate_runtime_thread_conversation_get(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<Option<Conversation>, String> {
    Ok(
        delegate_runtime_thread_get(app_state, delegate_id)?
            .map(|thread| thread.conversation),
    )
}

fn delegate_runtime_thread_conversation_get_any(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<Option<Conversation>, String> {
    Ok(
        delegate_runtime_thread_get_any(app_state, delegate_id)?
            .map(|thread| thread.conversation),
    )
}

fn delegate_runtime_thread_conversation_update(
    app_state: &AppState,
    delegate_id: &str,
    conversation: Conversation,
) -> Result<(), String> {
    delegate_runtime_thread_modify(app_state, delegate_id, move |thread| {
        thread.conversation = conversation;
        Ok(())
    })
}
