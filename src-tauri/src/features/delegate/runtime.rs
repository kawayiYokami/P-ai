fn delegate_runtime_thread_build(
    delegate: &DelegateEntry,
    target_api_config_id: &str,
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
        conversation,
    }
}

fn delegate_runtime_thread_create(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
) -> Result<String, String> {
    let thread = delegate_runtime_thread_build(delegate, target_api_config_id);
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

fn delegate_runtime_thread_remove(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<Option<DelegateRuntimeThread>, String> {
    let mut guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    Ok(guard.remove(delegate_id.trim()))
}

fn delegate_runtime_thread_list(app_state: &AppState) -> Result<Vec<DelegateRuntimeThread>, String> {
    let guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    Ok(guard.values().cloned().collect::<Vec<_>>())
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
