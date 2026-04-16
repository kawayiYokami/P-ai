#[tauri::command]
fn switch_active_conversation_snapshot(
    input: SwitchActiveConversationSnapshotInput,
    state: State<'_, AppState>,
) -> Result<SwitchActiveConversationSnapshotOutput, String> {
    const SWITCH_SNAPSHOT_RECENT_LIMIT: usize = 50;
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let data_before = data.clone();
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let requested_conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let effective_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| data.assistant_department_agent_id.clone());
    let target_idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;
    let target_conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    ensure_unarchived_conversation_not_organizing(state.inner(), &target_conversation_id)?;

    let mut changed = normalized_changed || department_changed;
    for (_idx, conversation) in data.conversations.iter_mut().enumerate() {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        let target_status = "active";
        if conversation.status.trim() != target_status {
            conversation.status = target_status.to_string();
            changed = true;
        }
    }

    let conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    let all_messages = data
        .conversations
        .get(target_idx)
        .map(|item| item.messages.clone())
        .ok_or_else(|| "Unarchived conversation messages not found.".to_string())?;
    let total_messages = all_messages.len();
    let start = total_messages.saturating_sub(SWITCH_SNAPSHOT_RECENT_LIMIT);
    let mut messages = all_messages[start..].to_vec();
    let has_more_history = start > 0;
    let unarchived_conversations =
        collect_unarchived_conversation_summaries(state.inner(), &app_config, &data);

    if changed {
        persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    }
    drop(guard);

    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);

    Ok(SwitchActiveConversationSnapshotOutput {
        conversation_id: conversation_id.clone(),
        messages,
        has_more_history,
        current_todo: data
            .conversations
            .get(target_idx)
            .and_then(conversation_current_todo_text),
        current_todos: data
            .conversations
            .get(target_idx)
            .map(|conversation| conversation.current_todos.clone())
            .unwrap_or_default(),
        unarchived_conversations,
    })
}

#[tauri::command]
fn set_conversation_plan_mode(
    input: SetConversationPlanModeInput,
    state: State<'_, AppState>,
) -> Result<SetConversationPlanModeOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }

    let current_enabled =
        get_conversation_plan_mode_enabled(state.inner(), conversation_id).unwrap_or(false);
    if current_enabled == input.plan_mode_enabled {
        return Ok(SetConversationPlanModeOutput {
            conversation_id: conversation_id.to_string(),
            plan_mode_enabled: input.plan_mode_enabled,
        });
    }

    set_conversation_plan_mode_enabled(state.inner(), conversation_id, input.plan_mode_enabled)?;
    runtime_log_info(format!(
        "[计划模式] 完成，任务=切换会话运行时计划模式，会话ID={}，状态={}",
        conversation_id,
        if input.plan_mode_enabled { "开启" } else { "关闭" }
    ));

    Ok(SetConversationPlanModeOutput {
        conversation_id: conversation_id.to_string(),
        plan_mode_enabled: input.plan_mode_enabled,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationInput {
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    department_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationOutput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameUnarchivedConversationInput {
    conversation_id: String,
    title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameUnarchivedConversationOutput {
    conversation_id: String,
    title: String,
}

#[tauri::command]
fn create_unarchived_conversation(
    input: CreateUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<CreateUnarchivedConversationOutput, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let before_conversations = data.conversations.clone();
    let requested_department_id = input
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let department = if let Some(department_id) = requested_department_id {
        department_by_id(&app_config, department_id)
            .ok_or_else(|| format!("Department '{department_id}' not found."))?
    } else {
        assistant_department(&app_config)
            .ok_or_else(|| "No assistant department configured.".to_string())?
    };
    let api_config_id = input
        .api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| department_primary_api_config_id(department));
    let agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            department
                .agent_ids
                .iter()
                .find(|id| !id.trim().is_empty())
                .cloned()
        })
        .unwrap_or_else(|| data.assistant_department_agent_id.clone());
    let conversation_title = input
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    for conversation in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        conversation.status = "active".to_string();
    }
    let conversation = build_foreground_chat_conversation_record(
        &state.data_path,
        &data,
        &api_config_id,
        &agent_id,
        &department.id,
        conversation_title,
    );
    let conversation_id = conversation.id.clone();
    data.conversations.push(conversation);
    if data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        data.main_conversation_id = Some(conversation_id.clone());
    }
    let overview_payload =
        build_unarchived_conversation_overview_payload(state.inner(), &app_config, &data);
    persist_conversation_set_delta(state.inner(), &before_conversations, &data.conversations)?;
    let chat_index_before = state_read_chat_index_cached(state.inner())?;
    let chat_index = build_chat_index_file(&data.conversations);
    if chat_index_before != chat_index {
        state_write_chat_index_cached(state.inner(), &chat_index)?;
    }
    if data.main_conversation_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
        != state_read_runtime_state_cached(&state)?
            .main_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        state_write_runtime_state_cached(&state, &build_runtime_state_file(&data))?;
    }
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);

    Ok(CreateUnarchivedConversationOutput { conversation_id })
}

#[tauri::command]
fn rename_unarchived_conversation(
    input: RenameUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<RenameUnarchivedConversationOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let next_title = clean_text(input.title.trim());
    if next_title.is_empty() {
        return Err("会话标题不能为空".to_string());
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let runtime = state_read_runtime_state_cached(&state)?;
    let main_conversation_id = runtime
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if conversation_id == main_conversation_id {
        drop(guard);
        return Err("主会话暂不支持改名".to_string());
    }
    ensure_unarchived_conversation_not_organizing(state.inner(), conversation_id)?;

    let mut conversation = state_read_conversation_cached(&state, conversation_id)?;
    if !conversation.summary.trim().is_empty()
        || !conversation_visible_in_foreground_lists(&conversation)
    {
        drop(guard);
        return Err("未找到可改名的会话".to_string());
    }

    if conversation.title.trim() == next_title {
        drop(guard);
        return Ok(RenameUnarchivedConversationOutput {
            conversation_id: conversation_id.to_string(),
            title: next_title,
        });
    }

    conversation.title = next_title.clone();
    state_write_conversation_cached(&state, &conversation)?;
    let data = state_read_app_data_cached(&state)?;
    let overview_payload = build_unarchived_conversation_overview_payload(state.inner(), &app_config, &data);
    drop(guard);

    runtime_log_info(format!(
        "[会话] 完成，任务=重命名会话，conversation_id={}，title={}",
        conversation_id, next_title
    ));
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);

    Ok(RenameUnarchivedConversationOutput {
        conversation_id: conversation_id.to_string(),
        title: next_title,
    })
}

#[tauri::command]
fn list_delegate_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<DelegateConversationSummary>, String> {
    let mut threads = delegate_runtime_thread_list(state.inner())?;
    threads.extend(delegate_recent_thread_list(state.inner())?);
    let mut summaries = threads
        .iter()
        .map(delegate_conversation_summary_from_runtime_thread)
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| {
        let bk = b
            .archived_at
            .as_deref()
            .or(b.last_message_at.as_deref())
            .unwrap_or(b.updated_at.as_str());
        let ak = a
            .archived_at
            .as_deref()
            .or(a.last_message_at.as_deref())
            .unwrap_or(a.updated_at.as_str());
        bk.cmp(ak).then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    Ok(summaries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationMessagesInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationRecentMessagesInput {
    conversation_id: String,
    #[serde(default = "default_recent_unarchived_message_limit")]
    limit: usize,
}

fn default_recent_unarchived_message_limit() -> usize {
    5
}

#[tauri::command]
fn get_unarchived_conversation_messages(
    input: GetUnarchivedConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let requested_messages = data
        .conversations
        .iter()
        .find(|c| c.summary.trim().is_empty() && conversation_visible_in_foreground_lists(c) && c.id == conversation_id)
        .map(|c| c.messages.clone());
    let fallback_messages = latest_active_conversation_index(&data, "", "")
        .and_then(|idx| data.conversations.get(idx))
        .map(|c| c.messages.clone());
    let mut messages = requested_messages
        .or(fallback_messages)
        .ok_or_else(|| "Unarchived conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[tauri::command]
fn get_unarchived_conversation_recent_messages(
    input: GetUnarchivedConversationRecentMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 50);
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let mut messages = data
        .conversations
        .iter()
        .find(|c| c.summary.trim().is_empty() && conversation_visible_in_foreground_lists(c) && c.id == conversation_id)
        .map(|c| {
            let total = c.messages.len();
            let start = total.saturating_sub(limit);
            c.messages[start..].to_vec()
        })
        .ok_or_else(|| "Unarchived conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetDelegateConversationMessagesInput {
    conversation_id: String,
}

#[tauri::command]
fn get_delegate_conversation_messages(
    input: GetDelegateConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let mut messages = delegate_runtime_thread_conversation_get_any(state.inner(), conversation_id)?
        .map(|conversation| conversation.messages.clone())
        .ok_or_else(|| "Delegate conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationOutput {
    deleted_conversation_id: String,
    active_conversation_id: String,
}

#[tauri::command]
fn delete_unarchived_conversation(
    input: DeleteUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<DeleteUnarchivedConversationOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[会话] 开始，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}",
        conversation_id
    ));
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let before_conversations = data.conversations.clone();
    let before_runtime = state_read_runtime_state_cached(&state)?;
    let deleted_is_main = data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        == Some(conversation_id);
    let before = data.conversations.len();
    data.conversations.retain(|conversation| {
        !(conversation.id == conversation_id
            && conversation.summary.trim().is_empty()
            && conversation_visible_in_foreground_lists(conversation))
    });
    if data.conversations.len() == before {
        drop(guard);
        runtime_log_info(format!(
            "[会话] 失败，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}，reason=not_found，duration_ms={}",
            conversation_id,
            started_at.elapsed().as_millis()
        ));
        return Err("Unarchived conversation not found.".to_string());
    }
    let _ = normalize_main_conversation_marker(&mut data, "");
    if !deleted_is_main {
        mark_tasks_as_session_lost(&state.data_path, conversation_id);
    }
    if deleted_is_main {
        let main_api_config_id = assistant_department(&app_config)
            .map(department_primary_api_config_id)
            .or_else(|| resolve_selected_api_config(&app_config, None).map(|item| item.id.clone()))
            .ok_or_else(|| "No API config available".to_string())?;
        let conversation = build_foreground_chat_conversation_record(
            &state.data_path,
            &data,
            &main_api_config_id,
            &data.assistant_department_agent_id,
            ASSISTANT_DEPARTMENT_ID,
            "",
        );
        let next_main_id = conversation.id.clone();
        data.conversations.push(conversation);
        data.main_conversation_id = Some(next_main_id);
    }
    let _ = normalize_single_active_main_conversation(&mut data);
    let active_conversation_id = data
        .conversations
        .iter()
        .find(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
                && conversation.status.trim() == "active"
        })
        .map(|conversation| conversation.id.clone())
        .or_else(|| {
            data.conversations
                .iter()
                .find(|conversation| {
                    conversation.summary.trim().is_empty() && conversation_visible_in_foreground_lists(conversation)
                })
                .map(|conversation| conversation.id.clone())
        })
        .unwrap_or_default();
    let overview_payload =
        build_unarchived_conversation_overview_payload(state.inner(), &app_config, &data);
    persist_conversation_set_delta(state.inner(), &before_conversations, &data.conversations)?;
    let chat_index_before = state_read_chat_index_cached(state.inner())?;
    let chat_index = build_chat_index_file(&data.conversations);
    if chat_index_before != chat_index {
        state_write_chat_index_cached(state.inner(), &chat_index)?;
    }
    if before_runtime.main_conversation_id != data.main_conversation_id {
        state_write_runtime_state_cached(state.inner(), &build_runtime_state_file(&data))?;
    }
    runtime_log_info(format!(
        "[会话] 完成，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}，duration_ms={}",
        conversation_id,
        started_at.elapsed().as_millis()
    ));
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);
    cleanup_pdf_session_memory_cache_for_conversation(conversation_id);
    Ok(DeleteUnarchivedConversationOutput {
        deleted_conversation_id: conversation_id.to_string(),
        active_conversation_id,
    })
}

#[tauri::command]
fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;

    let mut data = state_read_app_data_cached(&state)?;
    let data_before = data.clone();
    let _normalized_changed = normalize_single_active_main_conversation(&mut data);
    let _department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let requested_conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;

    let mut messages = data.conversations[idx].messages.clone();

    persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    drop(guard);
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[tauri::command]
fn mark_conversation_read(
    input: MarkConversationReadInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(());
    }
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut conversation = match state_read_conversation_cached(&state, conversation_id) {
        Ok(conversation) => conversation,
        Err(err) => {
            runtime_log_debug(format!(
                "[会话已读] 读取会话失败，conversation_id={}，error={}",
                conversation_id, err
            ));
            drop(guard);
            return Ok(());
        }
    };
    let next_last_read_message_id = conversation
        .messages
        .last()
        .map(|message| message.id.trim().to_string())
        .unwrap_or_default();
    if conversation.last_read_message_id.trim() == next_last_read_message_id {
        drop(guard);
        return Ok(());
    }
    conversation.last_read_message_id = next_last_read_message_id;
    state_write_conversation_cached(&state, &conversation)?;
    drop(guard);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesBeforeInput {
    session: SessionSelector,
    before_message_id: String,
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesBeforeOutput {
    messages: Vec<ChatMessage>,
    has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesAfterInput {
    session: SessionSelector,
    after_message_id: String,
    #[serde(default = "default_message_page_limit")]
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesAfterOutput {
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestConversationMessagesAfterAsyncInput {
    conversation_id: String,
    #[serde(default)]
    after_message_id: Option<String>,
    #[serde(default = "default_recent_unarchived_message_limit")]
    fallback_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestConversationMessagesAfterAsyncOutput {
    accepted: bool,
    request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMessagesAfterAsyncPayload {
    request_id: String,
    conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_message_id: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn default_message_page_limit() -> usize {
    100
}

fn resolve_unarchived_conversation_index_with_fallback(
    data: &mut AppData,
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
                && item.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(item)
        }) {
            return Ok(idx);
        }
        runtime_log_warn(format!(
            "[解析对话索引] 请求的conversation_id不存在，终止本次读取: '{}' (agent_id: '{}')",
            conversation_id, effective_agent_id
        ));
        return Err(format!(
            "Requested conversation not found: {conversation_id}"
        ));
    }

    if let Some(existing_idx) = main_conversation_index(data, effective_agent_id) {
        return Ok(existing_idx);
    }

    if let Some(existing_idx) = latest_active_conversation_index(data, "", effective_agent_id) {
        return Ok(existing_idx);
    }

    let api_config = resolve_selected_api_config(app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;
    Ok(ensure_active_conversation_index(
        data,
        &api_config.id,
        effective_agent_id,
    ))
}

fn resolve_unarchived_conversation_messages_after(
    state: &AppState,
    conversation_id: &str,
    after_message_id: Option<&str>,
    fallback_limit: usize,
) -> Result<(Vec<ChatMessage>, Option<String>), String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(state)?;
    let conversation = data
        .conversations
        .iter()
        .find(|item| {
            item.id == conversation_id
                && item.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(item)
        })
        .ok_or_else(|| format!("Unarchived conversation not found: {conversation_id}"))?;
    let messages = conversation.messages.clone();
    drop(guard);

    let trimmed_after = after_message_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (mut page, fallback_mode) = if let Some(after_id) = trimmed_after {
        if let Some(after_idx) = messages.iter().position(|item| item.id == after_id) {
            (messages[(after_idx + 1)..].to_vec(), None)
        } else {
            let start = messages.len().saturating_sub(fallback_limit);
            (messages[start..].to_vec(), Some("recent_limit".to_string()))
        }
    } else {
        let start = messages.len().saturating_sub(fallback_limit);
        (messages[start..].to_vec(), Some("recent_limit".to_string()))
    };
    materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
    Ok((page, fallback_mode))
}

#[tauri::command]
fn get_active_conversation_messages_before(
    input: GetActiveConversationMessagesBeforeInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationMessagesBeforeOutput, String> {
    let before_message_id = input.before_message_id.trim();
    if before_message_id.is_empty() {
        return Err("beforeMessageId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 100);

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let data_before = data.clone();
    let _normalized_changed = normalize_single_active_main_conversation(&mut data);
    let _department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.session.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;

    let messages = data.conversations[idx].messages.clone();
    persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    drop(guard);

    let before_idx = messages
        .iter()
        .position(|item| item.id == before_message_id)
        .ok_or_else(|| format!("beforeMessageId not found: {before_message_id}"))?;

    let start = before_idx.saturating_sub(limit);
    let has_more = start > 0;
    let mut page = messages[start..before_idx].to_vec();
    materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
    Ok(GetActiveConversationMessagesBeforeOutput {
        messages: page,
        has_more,
    })
}

#[tauri::command]
fn get_active_conversation_messages_after(
    input: GetActiveConversationMessagesAfterInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationMessagesAfterOutput, String> {
    let after_message_id = input.after_message_id.trim();
    if after_message_id.is_empty() {
        return Err("afterMessageId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 200);

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let data_before = data.clone();
    let _normalized_changed = normalize_single_active_main_conversation(&mut data);
    let _department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.session.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;

    let messages = data.conversations[idx].messages.clone();
    persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    drop(guard);

    let after_idx = messages
        .iter()
        .position(|item| item.id == after_message_id)
        .ok_or_else(|| format!("afterMessageId not found: {after_message_id}"))?;

    let end = (after_idx + 1 + limit).min(messages.len());
    let mut page = messages[(after_idx + 1)..end].to_vec();
    materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
    Ok(GetActiveConversationMessagesAfterOutput {
        messages: page,
    })
}


#[tauri::command]
fn request_conversation_messages_after_async(
    input: RequestConversationMessagesAfterAsyncInput,
    state: State<'_, AppState>,
) -> Result<RequestConversationMessagesAfterAsyncOutput, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let request_id = Uuid::new_v4().to_string();
    let after_message_id = input
        .after_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let fallback_limit = input.fallback_limit.clamp(1, 50);
    let state_clone = state.inner().clone();
    let request_id_clone = request_id.clone();
    eprintln!(
        "[聊天推送] 收到异步补消息请求: request_id={}, conversation_id={}, after_message_id={}, fallback_limit={}",
        request_id,
        conversation_id,
        after_message_id.as_deref().unwrap_or(""),
        fallback_limit
    );
    tauri::async_runtime::spawn(async move {
        let payload = match resolve_unarchived_conversation_messages_after(
            &state_clone,
            &conversation_id,
            after_message_id.as_deref(),
            fallback_limit,
        ) {
            Ok((messages, fallback_mode)) => {
                eprintln!(
                    "[聊天推送] 异步补消息完成: request_id={}, conversation_id={}, message_count={}, fallback_mode={}",
                    request_id_clone,
                    conversation_id,
                    messages.len(),
                    fallback_mode.as_deref().unwrap_or("")
                );
                ConversationMessagesAfterAsyncPayload {
                    request_id: request_id_clone.clone(),
                    conversation_id: conversation_id.clone(),
                    after_message_id: after_message_id.clone(),
                    messages,
                    fallback_mode,
                    error: None,
                }
            }
            Err(error) => {
                eprintln!(
                    "[聊天推送] 异步补消息失败: request_id={}, conversation_id={}, error={}",
                    request_id_clone, conversation_id, error
                );
                ConversationMessagesAfterAsyncPayload {
                    request_id: request_id_clone.clone(),
                    conversation_id: conversation_id.clone(),
                    after_message_id: after_message_id.clone(),
                    messages: Vec::new(),
                    fallback_mode: None,
                    error: Some(error),
                }
            }
        };
        let app_handle = match state_clone.app_handle.lock() {
            Ok(guard) => guard.as_ref().cloned(),
            Err(_) => None,
        };
        let Some(app_handle) = app_handle else {
            eprintln!(
                "[聊天推送] 异步补消息 emit 跳过: app_handle unavailable, request_id={}, conversation_id={}",
                request_id_clone, conversation_id
            );
            return;
        };
        match app_handle.emit("easy-call:conversation-messages-after-synced", &payload) {
            Ok(_) => eprintln!(
                "[聊天推送] 异步补消息 emit 成功: request_id={}, conversation_id={}",
                request_id_clone, conversation_id
            ),
            Err(err) => eprintln!(
                "[聊天推送] 异步补消息 emit 失败: request_id={}, conversation_id={}, error={}",
                request_id_clone, conversation_id, err
            ),
        }
    });

    Ok(RequestConversationMessagesAfterAsyncOutput {
        accepted: true,
        request_id,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationInput {
    session: SessionSelector,
    message_id: String,
    #[serde(default)]
    undo_apply_patch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationResult {
    removed_count: usize,
    remaining_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    recalled_user_message: Option<ChatMessage>,
}

fn validate_rewind_input(
    input: &RewindConversationInput,
    started_at: &std::time::Instant,
) -> Result<(String, String), String> {
    let message_id = input.message_id.trim().to_string();
    if message_id.is_empty() {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=validate_rewind_input，reason=message_id_empty，duration_ms={}",
            elapsed_ms
        ));
        return Err("messageId is required.".to_string());
    }

    let requested_agent_id = input.session.agent_id.trim().to_string();
    if requested_agent_id.is_empty() {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=validate_rewind_input，reason=agent_id_empty，duration_ms={}",
            elapsed_ms
        ));
        return Err("agentId is required.".to_string());
    }

    Ok((message_id, requested_agent_id))
}

fn resolve_rewind_target_conversation_index(
    data: &AppData,
    requested_agent_id: &str,
    requested_conversation_id: Option<&str>,
) -> Result<usize, String> {
    if let Some(conversation_id) = requested_conversation_id {
        data.conversations
            .iter()
            .position(|item| {
                item.id == conversation_id
                    && item.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(item)
            })
            .ok_or_else(|| {
                format!("Target conversation not found or unavailable, conversationId={conversation_id}")
            })
    } else {
        latest_active_conversation_index(data, "", requested_agent_id)
            .ok_or_else(|| "No conversation found for current agent.".to_string())
    }
}

fn persist_rewind_conversation_state(
    conversation: &mut Conversation,
    remove_from: usize,
) -> Result<(usize, usize), String> {
    let removed_count = conversation.messages.len().saturating_sub(remove_from);
    conversation.messages.truncate(remove_from);
    conversation.updated_at = now_iso();
    conversation.last_user_at = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.created_at.clone());
    conversation.last_assistant_at = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .map(|m| m.created_at.clone());
    conversation.last_context_usage_ratio = 0.0;
    conversation.last_effective_prompt_tokens = 0;
    Ok((removed_count, remove_from))
}

#[tauri::command]
fn rewind_conversation_from_message(
    input: RewindConversationInput,
    state: State<'_, AppState>,
) -> Result<RewindConversationResult, String> {
    let started_at = std::time::Instant::now();
    let (message_id, requested_agent_id) = validate_rewind_input(&input, &started_at)?;
    runtime_log_info(format!(
        "[会话撤回] 开始，任务=rewind_conversation_from_message，message_id={}，undo_apply_patch={}",
        message_id, input.undo_apply_patch
    ));

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| {
            format!(
                "Failed to lock state mutex at {}:{} {}: {err}",
                file!(),
                line!(),
                module_path!()
            )
        })?;

    let mut app_config = read_config(&state.config_path)?;

    let mut data = state_read_app_data_cached(&state)?;
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    if !runtime_data
        .agents
        .iter()
        .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=rewind_conversation_from_message，reason=agent_not_found，agent_id={}，duration_ms={}",
            requested_agent_id, elapsed_ms
        ));
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }

    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_rewind_target_conversation_index(&data, &requested_agent_id, requested_conversation_id)?;
    let (conversation_id, removed_count, remaining_count, mut recalled_user_message) = {
        let conversation = data
            .conversations
            .get_mut(idx)
            .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?;
        let conversation_id = conversation.id.clone();

        let remove_from = conversation
            .messages
            .iter()
            .position(|m| m.id == message_id && m.role == "user")
            .ok_or_else(|| "Target user message not found in active conversation.".to_string())?;
        runtime_log_info(format!(
            "[会话撤回] 命中目标，任务=rewind_conversation_from_message，conversation_id={}，remove_from={}，messages_total={}",
            conversation.id,
            remove_from,
            conversation.messages.len()
        ));

        let recalled_user_message = conversation.messages.get(remove_from).cloned();
        let removed_messages = conversation.messages[remove_from..].to_vec();
        if input.undo_apply_patch {
            runtime_log_info(format!(
                "[会话撤回] 开始工具逆向，任务=rewind_conversation_from_message，removed_messages={}，message_id={}",
                removed_messages.len(),
                message_id
            ));
            let undone_patch_count =
                match try_undo_apply_patch_from_removed_messages(state.inner(), &removed_messages) {
                    Ok(value) => value,
                    Err(err) => {
                        let elapsed_ms = started_at.elapsed().as_millis();
                        runtime_log_error(format!(
                            "[会话撤回] 失败，任务=rewind_conversation_from_message，stage=undo_apply_patch，message_id={}，duration_ms={}，error={}",
                            message_id, elapsed_ms, err
                        ));
                        return Err(err);
                    }
                };
            runtime_log_info(format!(
                "[会话撤回] 工具逆向处理，任务=rewind_conversation_from_message，patches={}，message_id={}",
                undone_patch_count, message_id
            ));
            if undone_patch_count > 0 {
                eprintln!(
                    "[会话撤回] 已执行 apply_patch 反向撤回: patches={}, message_id={}",
                    undone_patch_count,
                    message_id
                );
            }
        }
        let (removed_count, remaining_count) =
            persist_rewind_conversation_state(conversation, remove_from)?;
        (
            conversation_id,
            removed_count,
            remaining_count,
            recalled_user_message,
        )
    };
    if removed_count > 0 {
        persist_single_conversation_runtime_fast(&state, &data, &conversation_id)?;
    }
    drop(guard);

    if let Some(message) = recalled_user_message.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }
    let elapsed_ms = started_at.elapsed().as_millis();
    runtime_log_info(format!(
        "[会话撤回] 完成，任务=rewind_conversation_from_message，removed_count={}，remaining_count={}，duration_ms={}",
        removed_count, remaining_count, elapsed_ms
    ));

    Ok(RewindConversationResult {
        removed_count,
        remaining_count,
        recalled_user_message,
    })
}
