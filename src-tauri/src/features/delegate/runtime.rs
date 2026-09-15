const DELEGATE_RECENT_THREAD_LIMIT: usize = 10;

static DELETED_DELEGATE_CONVERSATION_IDS: OnceLock<
    Mutex<std::collections::HashSet<String>>,
> = OnceLock::new();

/// 委托状态变更只发布脏标记事件；前端收到后按 conversationId 拉取快照，不依赖事件内容
fn emit_conversation_delegate_status_updated(
    app_state: &AppState,
    root_conversation_id: &str,
    delegate_id: &str,
    status: &str,
) {
    monitor_publish_changed(
        app_state,
        MonitorDomain::Delegate,
        status,
        root_conversation_id,
        delegate_id,
    );
}

fn deleted_delegate_conversation_ids(
) -> &'static Mutex<std::collections::HashSet<String>> {
    DELETED_DELEGATE_CONVERSATION_IDS.get_or_init(|| {
        Mutex::new(std::collections::HashSet::new())
    })
}

fn delegate_runtime_thread_is_deleted(delegate_id: &str) -> Result<bool, String> {
    let normalized_delegate_id = delegate_id.trim();
    if normalized_delegate_id.is_empty() {
        return Ok(false);
    }
    let deleted = deleted_delegate_conversation_ids()
        .lock()
        .map_err(|_| "Failed to lock deleted delegate conversation ids".to_string())?;
    Ok(deleted.contains(normalized_delegate_id))
}

fn delegate_conversation_store_write_if_not_deleted(
    app_state: &AppState,
    delegate_id: &str,
    conversation: &Conversation,
) -> Result<bool, String> {
    let normalized_delegate_id = delegate_id.trim();
    let deleted = deleted_delegate_conversation_ids()
        .lock()
        .map_err(|_| "Failed to lock deleted delegate conversation ids".to_string())?;
    if deleted.contains(normalized_delegate_id) {
        runtime_log_warn(format!(
            "[委托会话] 跳过，任务=写入已删除委托会话，delegate_id={}",
            normalized_delegate_id
        ));
        return Ok(false);
    }
    delegate_conversation_store_write(&app_state.data_path, conversation)?;
    Ok(true)
}

fn delegate_conversation_store_delete_with_tombstone(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<bool, String> {
    let normalized_delegate_id = delegate_id.trim();
    if normalized_delegate_id.is_empty() {
        return Err("delegateId 不能为空".to_string());
    }
    let mut deleted = deleted_delegate_conversation_ids()
        .lock()
        .map_err(|_| "Failed to lock deleted delegate conversation ids".to_string())?;
    deleted.insert(normalized_delegate_id.to_string());
    delegate_conversation_store_delete(&app_state.data_path, normalized_delegate_id)
}

fn delegate_parent_shell_workspace(
    app_state: &AppState,
    root_conversation_id: &str,
    parent_chat_session_key: Option<&str>,
) -> Option<Conversation> {
    if let Some(session_id) = parent_chat_session_key {
        if let Ok(Some(conversation)) = terminal_session_conversation(app_state, session_id) {
            if delegate_workspace_snapshot_from_conversation(&conversation).is_some() {
                return Some(conversation);
            }
        }
    }
    conversation_service_v2()
        .get_conversation_meta(app_state, root_conversation_id)
        .ok()
        .map(|conversation_meta| Conversation {
            id: conversation_meta.id,
            title: conversation_meta.title,
            agent_id: conversation_meta.agent_id,
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: conversation_meta.unread_count,
            conversation_kind: conversation_meta.conversation_kind,
            root_conversation_id: None,
            delegate_id: None,
            created_at: conversation_meta.created_at,
            updated_at: conversation_meta.updated_at,
            last_user_at: None,
            last_assistant_at: None,
            status: conversation_meta.status,
            archived_at: conversation_meta.archived_at,
            user_profile_snapshot: String::new(),
            preferred_api_config_id: conversation_meta.preferred_api_config_id,
            is_draft: false,
            auto_push_remote_contact_id: None,
            shell_workspace_path: conversation_meta.shell_workspace_path,
            shell_workspaces: conversation_meta.shell_workspaces,
            shell_autonomous_mode: conversation_meta.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&conversation_meta.shell_work_mode),
            shell_work_branch: conversation_meta.shell_work_branch.clone(),
            messages: Vec::new(),
            fast_request_turns: conversation_meta.fast_request_turns,
            current_todos: conversation_meta.current_todos,
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            cumulative_usage: ConversationCumulativeUsage::default(),
            active_goal: conversation_meta.active_goal,
            last_error: conversation_meta.last_error,
        })
        .filter(|conversation| delegate_workspace_snapshot_from_conversation(conversation).is_some())
}

#[derive(Debug, Clone)]
struct DelegateWorkspaceSnapshot {
    shell_workspace_path: Option<String>,
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    shell_autonomous_mode: bool,
    shell_work_mode: String,
    shell_work_branch: String,
}

fn delegate_workspace_snapshot_from_conversation(
    conversation: &Conversation,
) -> Option<DelegateWorkspaceSnapshot> {
    let has_locked_root = conversation
        .shell_workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let has_workspaces = !conversation.shell_workspaces.is_empty();
    if !has_locked_root
        && !has_workspaces
        && !conversation.shell_autonomous_mode
        && normalize_shell_work_mode_text(&conversation.shell_work_mode) == SHELL_WORK_MODE_DIRECTORY
    {
        return None;
    }
    Some(DelegateWorkspaceSnapshot {
        shell_workspace_path: conversation.shell_workspace_path.clone(),
        shell_workspaces: conversation.shell_workspaces.clone(),
        shell_autonomous_mode: conversation.shell_autonomous_mode,
        shell_work_mode: normalize_shell_work_mode_text(&conversation.shell_work_mode),
        shell_work_branch: conversation.shell_work_branch.clone(),
    })
}

fn delegate_capture_workspace_snapshot(
    app_state: &AppState,
    root_conversation_id: &str,
    parent_chat_session_key: Option<&str>,
) -> Option<DelegateWorkspaceSnapshot> {
    let snapshot = delegate_parent_shell_workspace(app_state, root_conversation_id, parent_chat_session_key)
        .and_then(|conversation| delegate_workspace_snapshot_from_conversation(&conversation));
    runtime_log_debug(format!(
        "[委托工作目录] 捕获快照 conversation_id={} parent_chat_session_key={} shell_workspace_path={} shell_workspaces={} shell_autonomous_mode={}",
        root_conversation_id,
        parent_chat_session_key.unwrap_or(""),
        snapshot
            .as_ref()
            .and_then(|value| value.shell_workspace_path.as_deref())
            .unwrap_or(""),
        snapshot
            .as_ref()
            .map(|value| value.shell_workspaces.iter().map(|item| item.path.clone()).collect::<Vec<_>>().join(" | "))
            .unwrap_or_default(),
        snapshot
            .as_ref()
            .map(|value| value.shell_autonomous_mode)
            .unwrap_or(false)
    ));
    snapshot
}

fn delegate_runtime_thread_build(
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    workspace_snapshot: Option<DelegateWorkspaceSnapshot>,
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
    if let Some(workspace_snapshot) = workspace_snapshot {
        conversation.shell_workspace_path = workspace_snapshot.shell_workspace_path;
        conversation.shell_workspaces = workspace_snapshot.shell_workspaces;
        conversation.shell_autonomous_mode = workspace_snapshot.shell_autonomous_mode;
        conversation.shell_work_mode = workspace_snapshot.shell_work_mode;
        conversation.shell_work_branch =
            normalize_shell_work_branch_text(&workspace_snapshot.shell_work_branch);
    }
    runtime_log_info(format!(
        "[委托工作目录] 写入子代理 delegate_id={} shell_workspace_path={} shell_workspaces={} shell_autonomous_mode={}",
        delegate.delegate_id,
        conversation.shell_workspace_path.as_deref().unwrap_or(""),
        conversation
            .shell_workspaces
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>()
            .join(" | "),
        conversation.shell_autonomous_mode
    ));
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
    workspace_snapshot: Option<DelegateWorkspaceSnapshot>,
    parent_chat_session_key: Option<String>,
) -> Result<String, String> {
    if delegate_runtime_thread_is_deleted(&delegate.delegate_id)? {
        return Err(format!(
            "委托会话已删除，delegateId={}",
            delegate.delegate_id
        ));
    }
    if task_conversation_id_is_system_notification(&delegate.conversation_id) {
        task_ensure_system_notification_conversation(app_state)?;
    } else {
        conversation_service_v2()
            .get_conversation_meta(app_state, &delegate.conversation_id)
            .ok()
            .filter(|conversation_meta| {
                conversation_meta.status.trim() != "archived"
                    && conversation_meta
                        .archived_at
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE
                    && conversation_meta.conversation_kind.trim()
                        != CONVERSATION_KIND_SYSTEM_NOTIFICATION
            })
            .ok_or_else(|| {
                format!(
                    "委托绑定会话不存在，delegateId={}，conversationId={}",
                    delegate.delegate_id, delegate.conversation_id
                )
            })?;
    }
    let thread = delegate_runtime_thread_build(
        delegate,
        target_api_config_id,
        workspace_snapshot,
        parent_chat_session_key,
    );
    let thread_id = thread.delegate_id.clone();
    if !delegate_conversation_store_write_if_not_deleted(
        app_state,
        &thread.delegate_id,
        &thread.conversation,
    )? {
        return Err(format!("委托会话已删除，delegateId={}", thread.delegate_id));
    }
    let mut guard = app_state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    guard.insert(thread_id.clone(), thread);
    drop(guard);
    emit_conversation_delegate_status_updated(
        app_state,
        &delegate.conversation_id,
        &thread_id,
        DELEGATE_STATUS_RUNNING,
    );
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
    match delegate_conversation_store_read(&app_state.data_path, &thread.delegate_id) {
        Ok(Some(conversation)) => {
            thread.conversation = conversation;
        }
        Ok(None) => {}
        Err(err) => {
            runtime_log_warn(format!(
                "[委托会话] 警告，任务=读取持久化委托会话失败，delegate_id={}，error={}",
                thread.delegate_id, err
            ));
        }
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
    Ok(guard.values().cloned().collect())
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
    let Some(mut thread) = active.remove(delegate_id.trim()) else {
        drop(active);
        let Some(mut conversation) = delegate_conversation_store_read(&app_state.data_path, delegate_id)? else {
            return Ok(());
        };
        conversation.archived_at = Some(archived_at.to_string());
        conversation.updated_at = archived_at.to_string();
        return delegate_conversation_store_write_if_not_deleted(app_state, delegate_id, &conversation).map(|_| ());
    };
    drop(active);
    if delegate_runtime_thread_is_deleted(&thread.delegate_id)? {
        return Ok(());
    }
    if let Some(persisted) =
        delegate_conversation_store_read(&app_state.data_path, &thread.delegate_id)?
    {
        thread.conversation = persisted;
    }
    thread.archived_at = Some(archived_at.to_string());
    thread.conversation.archived_at = Some(archived_at.to_string());
    thread.conversation.updated_at = archived_at.to_string();
    delegate_conversation_store_write_if_not_deleted(
        app_state,
        &thread.delegate_id,
        &thread.conversation,
    )?;
    let mut recent = app_state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
    recent.retain(|item| item.delegate_id != thread.delegate_id);
    recent.push_front(thread);
    while recent.len() > DELEGATE_RECENT_THREAD_LIMIT {
        recent.pop_back();
    }
    Ok(())
}

fn abort_delegate_runtime_thread(
    app_state: &AppState,
    delegate_id: &str,
    reason: &str,
) -> Result<bool, String> {
    let normalized_delegate_id = delegate_id.trim();
    if normalized_delegate_id.is_empty() {
        return Err("delegateId 不能为空".to_string());
    }
    let thread = delegate_runtime_thread_get(app_state, normalized_delegate_id)?;
    let Some(thread) = thread else {
        return Ok(false);
    };
    // 委托被中断时其调用方不会走到正常释放分支，这里补一次深度回忆内存索引释放。
    deep_recall_index_release(normalized_delegate_id);
    let chat_key = delegate_thread_chat_key(&thread);
    let aborted_chat = {
        let mut inflight = app_state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(handle) = inflight.remove(&chat_key) {
            handle.abort();
            true
        } else {
            false
        }
    };
    let aborted_tool = abort_inflight_tool_abort_handle(app_state, &chat_key)?;
    let descendant_count = abort_delegate_runtime_descendants_by_parent_session(app_state, &chat_key)?;
    if let Err(err) = clear_conversation_queue(
        app_state,
        &thread.conversation.id,
        "委托已被打断，队列消息已清理",
    ) {
        runtime_log_error(format!(
            "[委托会话] 清理队列失败: delegate_id={}, error={}",
            normalized_delegate_id, err
        ));
    }
    if let Err(err) = release_conversation_processing_claim(app_state, &thread.conversation.id) {
        runtime_log_error(format!(
            "[委托会话] 释放处理声明失败: delegate_id={}, error={}",
            normalized_delegate_id, err
        ));
    }
    if let Err(err) =
        set_conversation_runtime_state_and_emit(
            app_state,
            &thread.conversation.id,
            MainSessionState::Idle,
        )
    {
        runtime_log_error(format!(
            "[委托会话] 重置运行态失败: delegate_id={}, error={}",
            normalized_delegate_id, err
        ));
    }
    clear_inflight_completed_tool_history(app_state, &chat_key)?;
    let archived_at = now_iso();
    delegate_runtime_thread_archive(app_state, normalized_delegate_id, &archived_at)?;
    delegate_store_update_status(
        &app_state.data_path,
        normalized_delegate_id,
        DELEGATE_STATUS_FAILED,
    )?;
    emit_conversation_delegate_status_updated(
        app_state,
        &thread.root_conversation_id,
        normalized_delegate_id,
        DELEGATE_STATUS_FAILED,
    );
    runtime_log_info(format!(
        "[委托会话] 已打断: delegate_id={}, chat_key={}, reason={}, aborted_chat={}, aborted_tool={}, descendant_count={}",
        normalized_delegate_id,
        chat_key,
        reason,
        aborted_chat,
        aborted_tool,
        descendant_count
    ));
    Ok(true)
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

fn delegate_runtime_thread_conversation_mutation_lock(
    app_state: &AppState,
    delegate_id: &str,
) -> Arc<Mutex<()>> {
    static LOCKS: std::sync::OnceLock<Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>> =
        std::sync::OnceLock::new();
    let key = format!(
        "{}::{}",
        app_state.data_path.to_string_lossy(),
        delegate_id.trim()
    );
    let locks = LOCKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut locks = match locks.lock() {
        Ok(locks) => locks,
        Err(poisoned) => {
            runtime_log_warn("[委托会话] 变更锁表中毒，已恢复".to_string());
            poisoned.into_inner()
        }
    };
    locks
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn delegate_runtime_thread_conversation_update_unlocked(
    app_state: &AppState,
    delegate_id: &str,
    conversation: Conversation,
) -> Result<(), String> {
    if !delegate_conversation_store_write_if_not_deleted(
        app_state,
        delegate_id,
        &conversation,
    )? {
        return Ok(());
    }
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

fn delegate_runtime_thread_conversation_update(
    app_state: &AppState,
    delegate_id: &str,
    conversation: Conversation,
) -> Result<(), String> {
    let mutation_lock = delegate_runtime_thread_conversation_mutation_lock(app_state, delegate_id);
    let _guard = match mutation_lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_warn(format!(
                "[委托会话] 变更锁中毒，已恢复，delegate_id={}",
                delegate_id
            ));
            poisoned.into_inner()
        }
    };
    delegate_runtime_thread_conversation_update_unlocked(app_state, delegate_id, conversation)
}

// 群聊长度门改写（默认禁用，保留待重新启用）。
#[allow(dead_code)]
fn delegate_runtime_thread_append_fast_request(
    app_state: &AppState,
    delegate_id: &str,
    turn: FastRequestTurn,
) -> Result<bool, String> {
    let normalized_delegate_id = delegate_id.trim();
    if normalized_delegate_id.is_empty() {
        return Err("delegateId 不能为空".to_string());
    }
    let mutation_lock =
        delegate_runtime_thread_conversation_mutation_lock(app_state, normalized_delegate_id);
    let _guard = match mutation_lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_warn(format!(
                "[委托会话] 杂务追加锁中毒，已恢复，delegate_id={}",
                normalized_delegate_id
            ));
            poisoned.into_inner()
        }
    };
    let Some(mut conversation) =
        delegate_runtime_thread_conversation_get_any(app_state, normalized_delegate_id)?
    else {
        return Ok(false);
    };
    if conversation
        .fast_request_turns
        .iter()
        .any(|existing| existing.id == turn.id)
    {
        return Ok(false);
    }
    conversation.fast_request_turns.push(turn);
    conversation.updated_at = now_iso();
    delegate_runtime_thread_conversation_update_unlocked(
        app_state,
        normalized_delegate_id,
        conversation,
    )?;
    Ok(true)
}

fn delegate_runtime_thread_conversation_append_if_absent(
    app_state: &AppState,
    delegate_id: &str,
    message: ChatMessage,
) -> Result<bool, String> {
    // 委托线程当前以单个 Conversation 文档持久化，没有独立消息仓库可做 message-by-id append。
    // 所有 update/append 共享同一个 per-delegate 变更锁，避免并发读改写互相覆盖。
    let mutation_lock = delegate_runtime_thread_conversation_mutation_lock(app_state, delegate_id);
    let _guard = match mutation_lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_warn(format!(
                "[委托会话] 追加锁中毒，已恢复，delegate_id={}",
                delegate_id
            ));
            poisoned.into_inner()
        }
    };
    let Some(mut conversation) = delegate_runtime_thread_conversation_get_any(app_state, delegate_id)?
    else {
        return Err(format!("委托会话不存在，delegate_id={delegate_id}"));
    };
    if conversation.messages.iter().any(|item| item.id == message.id) {
        return Ok(false);
    }
    conversation.messages.push(message);
    conversation.updated_at = now_iso();
    delegate_runtime_thread_conversation_update_unlocked(app_state, delegate_id, conversation)?;
    Ok(true)
}

fn delegate_runtime_thread_conversation_delete(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<bool, String> {
    let normalized_delegate_id = delegate_id.trim();
    if normalized_delegate_id.is_empty() {
        return Err("delegateId 不能为空".to_string());
    }
    let active_thread = {
        let mut active = app_state
            .delegate_runtime_threads
            .lock()
            .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
        active.remove(normalized_delegate_id)
    };
    if let Some(thread) = active_thread.as_ref() {
        let chat_key = delegate_thread_chat_key(thread);
        let aborted_chat = {
            let mut inflight = app_state
                .inflight_chat_abort_handles
                .lock()
                .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
            if let Some(handle) = inflight.remove(&chat_key) {
                handle.abort();
                true
            } else {
                false
            }
        };
        let aborted_tool = abort_inflight_tool_abort_handle(app_state, &chat_key)?;
        let descendant_count = abort_delegate_runtime_descendants_by_parent_session(app_state, &chat_key)?;
        clear_inflight_completed_tool_history(app_state, &chat_key)?;
        runtime_log_info(format!(
            "[委托会话] 完成，任务=删除前中止委托调度，delegate_id={}，chat_key={}，aborted_chat={}，aborted_tool={}，descendant_count={}",
            normalized_delegate_id,
            chat_key,
            aborted_chat,
            aborted_tool,
            descendant_count
        ));
    }
    {
        let mut recent = app_state
            .delegate_recent_threads
            .lock()
            .map_err(|_| "Failed to lock recent delegate runtime threads".to_string())?;
        recent.retain(|thread| thread.delegate_id != normalized_delegate_id);
    }
    delegate_conversation_store_delete_with_tombstone(app_state, normalized_delegate_id)
}

fn delegate_runtime_thread_conversation_delete_by_root(
    app_state: &AppState,
    root_conversation_id: &str,
) -> Result<usize, String> {
    let normalized_root_conversation_id = root_conversation_id.trim();
    if normalized_root_conversation_id.is_empty() {
        return Ok(0);
    }
    let mut delegate_ids = std::collections::BTreeSet::<String>::new();
    for thread in delegate_runtime_thread_list(app_state)? {
        if thread.root_conversation_id.trim() == normalized_root_conversation_id {
            delegate_ids.insert(thread.delegate_id);
        }
    }
    for thread in delegate_recent_thread_list(app_state)? {
        if thread.root_conversation_id.trim() == normalized_root_conversation_id {
            delegate_ids.insert(thread.delegate_id);
        }
    }
    for snapshot in delegate_persisted_snapshot_list_by_root(app_state, normalized_root_conversation_id)? {
        delegate_ids.insert(snapshot.delegate_id);
    }

    let mut deleted_count = 0usize;
    for delegate_id in delegate_ids {
        if delegate_runtime_thread_conversation_delete(app_state, &delegate_id)? {
            deleted_count = deleted_count.saturating_add(1);
        }
    }
    Ok(deleted_count)
}

fn delegate_persisted_conversation_summary_list(
    app_state: &AppState,
) -> Result<Vec<DelegateConversationSnapshot>, String> {
    delegate_snapshot_cache_list(&app_state.data_path)
}

fn delegate_persisted_snapshot_list_by_root(
    app_state: &AppState,
    root_conversation_id: &str,
) -> Result<Vec<DelegateConversationSnapshot>, String> {
    delegate_snapshot_cache_list_by_root(&app_state.data_path, root_conversation_id)
}