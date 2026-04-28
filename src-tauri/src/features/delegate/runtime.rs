const DELEGATE_RECENT_THREAD_LIMIT: usize = 10;

fn delegate_parent_shell_workspace(
    app_state: &AppState,
    root_conversation_id: &str,
    parent_chat_session_key: Option<&str>,
) -> Option<Conversation> {
    if let Some(session_id) = parent_chat_session_key {
        if let Ok(Some(conversation)) = terminal_session_conversation(app_state, session_id) {
            if conversation
                .shell_workspace_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            {
                return Some(conversation);
            }
        }
    }
    state_read_conversation_cached(app_state, root_conversation_id)
        .ok()
        .filter(|conversation| {
            conversation
                .shell_workspace_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
        })
}

fn delegate_runtime_thread_build(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    parent_chat_session_key: Option<String>,
) -> DelegateRuntimeThread {
    let mut conversation = build_conversation_record(
        target_api_config_id,
        &delegate.target_agent_id,
        &delegate.target_department_id,
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
    if let Some(parent_workspace) = delegate_parent_shell_workspace(
        app_state,
        &delegate.conversation_id,
        parent_chat_session_key.as_deref(),
    ) {
        conversation.shell_workspace_path = parent_workspace.shell_workspace_path;
        conversation.shell_workspaces = parent_workspace.shell_workspaces;
    }
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
    let thread = delegate_runtime_thread_build(
        app_state,
        delegate,
        target_api_config_id,
        parent_chat_session_key,
    );
    let thread_id = thread.delegate_id.clone();
    delegate_conversation_store_write(&app_state.data_path, &thread.conversation)?;
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

fn delegate_runtime_thread_apply_persisted_conversation(
    mut thread: DelegateRuntimeThread,
    app_state: &AppState,
) -> Result<DelegateRuntimeThread, String> {
    if let Some(conversation) = delegate_conversation_store_read(&app_state.data_path, &thread.delegate_id)?
    {
        thread.conversation = conversation;
    }
    Ok(thread)
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
    guard
        .values()
        .cloned()
        .map(|thread| delegate_runtime_thread_apply_persisted_conversation(thread, app_state))
        .collect()
}

fn delegate_recent_thread_list(app_state: &AppState) -> Result<Vec<DelegateRuntimeThread>, String> {
    let guard = app_state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
    guard
        .iter()
        .cloned()
        .map(|thread| delegate_runtime_thread_apply_persisted_conversation(thread, app_state))
        .collect()
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
    thread.conversation.archived_at = Some(archived_at.to_string());
    thread.conversation.updated_at = archived_at.to_string();
    delegate_conversation_store_write(&app_state.data_path, &thread.conversation)?;
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
        return delegate_runtime_thread_apply_persisted_conversation(thread, app_state).map(Some);
    }
    let recent_thread = {
        let recent = app_state
            .delegate_recent_threads
            .lock()
            .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
        recent
            .iter()
            .find(|thread| thread.delegate_id == delegate_id.trim())
            .cloned()
    };
    if let Some(thread) = recent_thread {
        return delegate_runtime_thread_apply_persisted_conversation(thread, app_state).map(Some);
    }
    if let Some(conversation) = delegate_conversation_store_read(&app_state.data_path, delegate_id)? {
        let root_conversation_id = conversation.root_conversation_id.clone().unwrap_or_default();
        let delegate_id = conversation
            .delegate_id
            .clone()
            .unwrap_or_else(|| conversation.id.clone());
        return Ok(Some(DelegateRuntimeThread {
            delegate_id,
            root_conversation_id,
            target_agent_id: conversation.agent_id.clone(),
            title: conversation.title.clone(),
            call_stack: Vec::new(),
            parent_chat_session_key: None,
            archived_at: conversation.archived_at.clone(),
            conversation,
        }));
    }
    Ok(None)
}

fn delegate_runtime_thread_conversation_get(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<Option<Conversation>, String> {
    if let Some(conversation) = delegate_conversation_store_read(&app_state.data_path, delegate_id)? {
        return Ok(Some(conversation));
    }
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
    delegate_conversation_store_write(&app_state.data_path, &conversation)?;
    let mut active = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    if let Some(thread) = active.get_mut(delegate_id.trim()) {
        thread.conversation = conversation;
        return Ok(());
    }
    drop(active);
    let mut recent = app_state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
    if let Some(thread) = recent
        .iter_mut()
        .find(|thread| thread.delegate_id == delegate_id.trim())
    {
        thread.conversation = conversation;
    }
    Ok(())
}

fn delegate_persisted_conversation_list(app_state: &AppState) -> Result<Vec<Conversation>, String> {
    delegate_conversation_store_list(&app_state.data_path)
}
