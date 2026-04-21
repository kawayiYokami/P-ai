mod git_ghost_snapshot;

use git_ghost_snapshot::read_git_snapshot_record_from_provider_meta;
use git_ghost_snapshot::restore_main_workspace_from_git_ghost_snapshot;

#[tauri::command]
fn switch_active_conversation_snapshot(
    input: SwitchActiveConversationSnapshotInput,
    state: State<'_, AppState>,
) -> Result<SwitchActiveConversationSnapshotOutput, String> {
    let started_at = std::time::Instant::now();
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let target_idx = resolve_foreground_snapshot_target_index(&input, &app_config, &mut data)?;
    let target_conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    ensure_unarchived_conversation_not_organizing(state.inner(), &target_conversation_id)?;

    let mut changed = normalized_changed || department_changed;
    for conversation in data.conversations.iter_mut() {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        if conversation.status.trim() != "active" {
            conversation.status = "active".to_string();
            changed = true;
        }
    }

    let mut snapshot = build_foreground_conversation_snapshot_core(
        state.inner(),
        &data,
        target_idx,
        SWITCH_SNAPSHOT_RECENT_LIMIT,
    )?;
    let unarchived_conversations =
        collect_unarchived_conversation_summaries(state.inner(), &app_config, &data);

    if changed {
        let target_ids = foreground_conversation_ids(&data);
        persist_selected_conversations_and_runtime(
            &state,
            &data,
            &target_ids,
            "switch_active_conversation_snapshot",
        )?;
    }
    drop(guard);

    materialize_chat_message_parts_from_media_refs(&mut snapshot.messages, &state.data_path);
    runtime_log_info(format!(
        "[前台重型快照] 完成，conversation_id={}，message_count={}，has_more_history={}，summary_count={}，duration_ms={}",
        snapshot.conversation_id,
        snapshot.messages.len(),
        snapshot.has_more_history,
        unarchived_conversations.len(),
        started_at.elapsed().as_millis()
    ));

    Ok(SwitchActiveConversationSnapshotOutput {
        conversation_id: snapshot.conversation_id,
        messages: snapshot.messages,
        has_more_history: snapshot.has_more_history,
        runtime_state: snapshot.runtime_state,
        current_todo: snapshot.current_todo,
        current_todos: snapshot.current_todos,
        unarchived_conversations,
    })
}

fn resolve_foreground_snapshot_target_index(
    input: &SwitchActiveConversationSnapshotInput,
    app_config: &AppConfig,
    data: &mut AppData,
) -> Result<usize, String> {
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
    resolve_unarchived_conversation_index_with_fallback(
        data,
        app_config,
        &effective_agent_id,
        requested_conversation_id,
    )
}

fn build_foreground_conversation_snapshot_core(
    state: &AppState,
    data: &AppData,
    target_idx: usize,
    recent_limit: usize,
) -> Result<ForegroundConversationSnapshotCore, String> {
    let conversation = data
        .conversations
        .get(target_idx)
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    let total_messages = conversation.messages.len();
    let start = total_messages.saturating_sub(recent_limit);
    let messages = conversation.messages[start..].to_vec();
    Ok(ForegroundConversationSnapshotCore {
        conversation_id: conversation.id.clone(),
        messages,
        has_more_history: start > 0,
        runtime_state: unarchived_conversation_runtime_state(state, &conversation.id),
        current_todo: conversation_current_todo_text(conversation),
        current_todos: conversation.current_todos.clone(),
    })
}

#[tauri::command]
fn get_foreground_conversation_light_snapshot(
    input: ForegroundConversationLightSnapshotInput,
    state: State<'_, AppState>,
) -> Result<ForegroundConversationLightSnapshotOutput, String> {
    let started_at = std::time::Instant::now();
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let target_idx = resolve_foreground_snapshot_target_index(
        &SwitchActiveConversationSnapshotInput {
            conversation_id: input.conversation_id.clone(),
            agent_id: input.agent_id.clone(),
        },
        &app_config,
        &mut data,
    )?;
    let target_conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    ensure_unarchived_conversation_not_organizing(state.inner(), &target_conversation_id)?;

    let mut snapshot = build_foreground_conversation_snapshot_core(
        state.inner(),
        &data,
        target_idx,
        SWITCH_SNAPSHOT_RECENT_LIMIT,
    )?;
    drop(guard);

    materialize_chat_message_parts_from_media_refs(&mut snapshot.messages, &state.data_path);
    runtime_log_info(format!(
        "[前台轻量快照] 完成，conversation_id={}，message_count={}，has_more_history={}，duration_ms={}",
        snapshot.conversation_id,
        snapshot.messages.len(),
        snapshot.has_more_history,
        started_at.elapsed().as_millis()
    ));

    Ok(ForegroundConversationLightSnapshotOutput {
        conversation_id: snapshot.conversation_id,
        messages: snapshot.messages,
        has_more_history: snapshot.has_more_history,
        runtime_state: snapshot.runtime_state,
        current_todo: snapshot.current_todo,
        current_todos: snapshot.current_todos,
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
    #[serde(default)]
    copy_source_conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationOutput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeriveUnarchivedConversationFromSelectionInput {
    source_conversation_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeriveUnarchivedConversationFromSelectionOutput {
    conversation_id: String,
    title: String,
    #[serde(default)]
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliverUnarchivedConversationSelectionInput {
    source_conversation_id: String,
    target_conversation_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliverUnarchivedConversationSelectionOutput {
    target_conversation_id: String,
    delivered_count: usize,
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

fn trimmed_option(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn clone_chat_message_for_copied_conversation(message: &ChatMessage) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: message.role.clone(),
        created_at: message.created_at.clone(),
        speaker_agent_id: message.speaker_agent_id.clone(),
        parts: message.parts.clone(),
        extra_text_blocks: message.extra_text_blocks.clone(),
        provider_meta: message.provider_meta.clone(),
        tool_call: message.tool_call.clone(),
        mcp_call: message.mcp_call.clone(),
    }
}

fn clone_foreground_conversation_for_copy(
    source: &Conversation,
    agent_id: &str,
    department_id: &str,
    title: &str,
) -> Conversation {
    let now = now_iso();
    let mut conversation = source.clone();
    conversation.id = Uuid::new_v4().to_string();
    conversation.title = if title.trim().is_empty() {
        source.title.clone()
    } else {
        title.trim().to_string()
    };
    conversation.agent_id = agent_id.trim().to_string();
    conversation.department_id = department_id.trim().to_string();
    conversation.bound_conversation_id = None;
    conversation.parent_conversation_id = Some(source.id.clone());
    conversation.child_conversation_ids = Vec::new();
    conversation.last_read_message_id = String::new();
    conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
    conversation.root_conversation_id = None;
    conversation.delegate_id = None;
    conversation.status = "active".to_string();
    conversation.summary = String::new();
    conversation.archived_at = None;
    conversation.created_at = now.clone();
    conversation.updated_at = now.clone();
    conversation.last_context_usage_ratio = 0.0;
    conversation.last_effective_prompt_tokens = 0;
    conversation.messages = source
        .messages
        .iter()
        .map(clone_chat_message_for_copied_conversation)
        .collect::<Vec<_>>();
    conversation.fork_message_cursor = conversation
        .messages
        .last()
        .map(|message| message.id.clone());
    conversation.last_read_message_id = conversation
        .messages
        .last()
        .map(|message| message.id.clone())
        .unwrap_or_default();
    conversation
}

fn build_derived_conversation_title(
    source_title: &str,
    first_selected_ordinal: usize,
    source_is_main_conversation: bool,
) -> String {
    let base_title = source_title.trim();
    let prefix = if source_is_main_conversation {
        "主会话"
    } else if base_title.is_empty() {
        "未命名会话"
    } else {
        base_title
    };
    format!("{prefix}[派生自第{first_selected_ordinal}条对话]")
}

fn collect_selected_messages_for_derive(
    source: &Conversation,
    selected_message_ids: &[String],
) -> (Vec<ChatMessage>, usize) {
    let selected_ids = selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut selected_messages = Vec::new();
    let mut visible_ordinal = 0usize;
    let mut first_selected_ordinal = 0usize;
    for message in &source.messages {
        if archive_pipeline_is_context_compaction_message(message) {
            continue;
        }
        visible_ordinal += 1;
        if !selected_ids.contains(message.id.trim()) {
            continue;
        }
        if first_selected_ordinal == 0 {
            first_selected_ordinal = visible_ordinal;
        }
        selected_messages.push(message.clone());
    }
    (selected_messages, first_selected_ordinal)
}

fn derive_conversation_settings_agent_id(
    data: &AppData,
    department: &DepartmentConfig,
    requested_agent_id: &str,
) -> String {
    let normalized_requested_agent_id = requested_agent_id.trim();
    if !normalized_requested_agent_id.is_empty()
        && department
            .agent_ids
            .iter()
            .any(|item| item.trim() == normalized_requested_agent_id)
    {
        return normalized_requested_agent_id.to_string();
    }
    department
        .agent_ids
        .iter()
        .find(|item| !item.trim().is_empty())
        .map(|item| item.trim().to_string())
        .unwrap_or_else(|| data.assistant_department_agent_id.trim().to_string())
}

fn build_derived_conversation_record_from_selection(
    data_path: &PathBuf,
    data: &AppData,
    source: &Conversation,
    department: &DepartmentConfig,
    title: &str,
    latest_compaction_message: Option<&ChatMessage>,
    selected_messages: &[ChatMessage],
) -> Conversation {
    let agent_id = derive_conversation_settings_agent_id(data, department, &source.agent_id);
    let mut conversation = build_conversation_record(
        &department_primary_api_config_id(department),
        &agent_id,
        &department.id,
        title,
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    conversation.parent_conversation_id = Some(source.id.clone());
    conversation.plan_mode_enabled = source.plan_mode_enabled;
    conversation.shell_workspace_path = source.shell_workspace_path.clone();
    conversation.shell_workspaces = source.shell_workspaces.clone();
    conversation.current_todos = source.current_todos.clone();
    let user_profile_snapshot = data
        .agents
        .iter()
        .find(|item| item.id == agent_id)
        .and_then(|agent| match build_user_profile_snapshot_block(data_path, agent, 12) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                runtime_log_warn(format!(
                    "[派生会话] 跳过，任务=构建用户画像快照，agent_id={}，error={}",
                    agent.id, err
                ));
                None
            }
        })
        .or_else(|| {
            let snapshot = source.user_profile_snapshot.trim();
            if snapshot.is_empty() {
                None
            } else {
                Some(snapshot.to_string())
            }
        });
    if let Some(snapshot) = user_profile_snapshot.clone() {
        conversation.user_profile_snapshot = snapshot;
    }
    if let Some(message) = latest_compaction_message {
        conversation
            .messages
            .push(clone_chat_message_for_copied_conversation(message));
    } else {
        conversation.messages.push(build_initial_summary_context_message(
            None,
            user_profile_snapshot.as_deref(),
            Some(&conversation.current_todos),
        ));
    }
    conversation.messages.extend(
        selected_messages
            .iter()
            .map(clone_chat_message_for_copied_conversation),
    );
    if let Some(last_message) = conversation.messages.last() {
        conversation.last_read_message_id = last_message.id.clone();
        conversation.updated_at = last_message.created_at.clone();
        conversation.last_user_at = Some(last_message.created_at.clone());
    }
    conversation
}

fn latest_compaction_message_for_derive(source: &Conversation) -> Option<ChatMessage> {
    source
        .messages
        .iter()
        .rev()
        .find(|message| archive_pipeline_is_context_compaction_message(message))
        .cloned()
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
    let copy_source_conversation_id = trimmed_option(input.copy_source_conversation_id.as_deref());

    for conversation in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        conversation.status = "active".to_string();
    }
    let conversation = if let Some(source_conversation_id) = copy_source_conversation_id.as_deref() {
        let source_conversation = data
            .conversations
            .iter()
            .find(|conversation| {
                conversation.id == source_conversation_id
                    && conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .cloned()
            .ok_or_else(|| "要复制的当前会话不存在或已归档".to_string())?;
        clone_foreground_conversation_for_copy(
            &source_conversation,
            &agent_id,
            &department.id,
            conversation_title,
        )
    } else {
        build_foreground_chat_conversation_record(
            &state.data_path,
            &data,
            &api_config_id,
            &agent_id,
            &department.id,
            conversation_title,
        )
    };
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
    persist_selected_conversations_and_runtime(
        state.inner(),
        &data,
        std::slice::from_ref(&conversation_id),
        "create_unarchived_conversation",
    )?;
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);

    Ok(CreateUnarchivedConversationOutput { conversation_id })
}

#[tauri::command]
async fn derive_unarchived_conversation_from_selection(
    input: DeriveUnarchivedConversationFromSelectionInput,
    state: State<'_, AppState>,
) -> Result<DeriveUnarchivedConversationFromSelectionOutput, String> {
    let source_conversation_id = input.source_conversation_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    let normalized_selected_message_ids = input
        .selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if normalized_selected_message_ids.is_empty() {
        return Err("selectedMessageIds 不能为空".to_string());
    }

    let (
        app_config,
        source_conversation,
        selected_messages,
        derived_title,
        department,
        latest_compaction_message,
    ) = {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let app_config = state_read_config_cached(&state)?;
        let data = state_read_app_data_cached(&state)?;
        let source_conversation = data
            .conversations
            .iter()
            .find(|conversation| {
                conversation.id == source_conversation_id
                    && conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .cloned()
            .ok_or_else(|| "源会话不存在或已归档".to_string())?;
        let (selected_messages, first_selected_ordinal) =
            collect_selected_messages_for_derive(&source_conversation, &normalized_selected_message_ids);
        if selected_messages.is_empty() {
            drop(guard);
            return Err("未找到可派生的已选消息".to_string());
        }
        let department = department_by_id(&app_config, source_conversation.department_id.trim())
            .cloned()
            .ok_or_else(|| "源会话所属部门不存在".to_string())?;
        let derived_title = build_derived_conversation_title(
            &source_conversation.title,
            first_selected_ordinal.max(1),
            data.main_conversation_id.as_deref().map(str::trim)
                == Some(source_conversation.id.as_str()),
        );
        let latest_compaction_message = latest_compaction_message_for_derive(&source_conversation);
        drop(guard);
        (
            app_config,
            source_conversation,
            selected_messages,
            derived_title,
            department,
            latest_compaction_message,
        )
    };

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    if !data.conversations.iter().any(|conversation| {
        conversation.id == source_conversation.id
            && conversation.summary.trim().is_empty()
            && conversation_visible_in_foreground_lists(conversation)
    }) {
        drop(guard);
        return Err("源会话已变化，请重新选择消息后再试".to_string());
    }
    let conversation = build_derived_conversation_record_from_selection(
        &state.data_path,
        &data,
        &source_conversation,
        &department,
        &derived_title,
        latest_compaction_message.as_ref(),
        &selected_messages,
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
    persist_selected_conversations_and_runtime(
        state.inner(),
        &data,
        std::slice::from_ref(&conversation_id),
        "derive_unarchived_conversation_from_selection",
    )?;
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);
    runtime_log_info(format!(
        "[派生会话] 完成，任务=按已选消息派生新会话，source_conversation_id={}，conversation_id={}，selected_count={}，has_compaction_seed={}",
        source_conversation.id,
        conversation_id,
        selected_messages.len(),
        latest_compaction_message.is_some()
    ));

    Ok(DeriveUnarchivedConversationFromSelectionOutput {
        conversation_id,
        title: derived_title,
        warning: None,
    })
}

#[tauri::command]
fn deliver_unarchived_conversation_selection(
    input: DeliverUnarchivedConversationSelectionInput,
    state: State<'_, AppState>,
) -> Result<DeliverUnarchivedConversationSelectionOutput, String> {
    let source_conversation_id = input.source_conversation_id.trim();
    let target_conversation_id = input.target_conversation_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    if target_conversation_id.is_empty() {
        return Err("targetConversationId 不能为空".to_string());
    }
    if source_conversation_id == target_conversation_id {
        return Err("目标会话不能是当前会话".to_string());
    }
    let normalized_selected_message_ids = input
        .selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if normalized_selected_message_ids.is_empty() {
        return Err("selectedMessageIds 不能为空".to_string());
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let target_runtime_state = {
        let runtime_slots = lock_conversation_runtime_slots(state.inner())?;
        runtime_slots
            .get(target_conversation_id)
            .map(|slot| slot.state.clone())
            .unwrap_or(MainSessionState::Idle)
    };
    if target_runtime_state == MainSessionState::AssistantStreaming {
        drop(guard);
        return Err("目标会话正在流式输出中，暂时无法投送".to_string());
    }
    if target_runtime_state == MainSessionState::OrganizingContext {
        drop(guard);
        return Err("目标会话正在整理上下文，暂时无法投送".to_string());
    }
    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;

    let source_conversation = data
        .conversations
        .iter()
        .find(|conversation| {
            conversation.id == source_conversation_id
                && conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
        })
        .cloned()
        .ok_or_else(|| "源会话不存在或已归档".to_string())?;
    let (selected_messages, _) =
        collect_selected_messages_for_derive(&source_conversation, &normalized_selected_message_ids);
    if selected_messages.is_empty() {
        drop(guard);
        return Err("未找到可投送的已选消息".to_string());
    }

    let target_idx = data
        .conversations
        .iter()
        .position(|conversation| {
            conversation.id == target_conversation_id
                && conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
        })
        .ok_or_else(|| "目标会话不存在或已归档".to_string())?;

    let now = now_iso();
    {
        let target_conversation = data
            .conversations
            .get_mut(target_idx)
            .ok_or_else(|| "目标会话索引无效".to_string())?;
        target_conversation.messages.extend(
            selected_messages
                .iter()
                .map(clone_chat_message_for_copied_conversation),
        );
        target_conversation.updated_at = now.clone();
        target_conversation.status = "active".to_string();
        if let Some(last_message) = target_conversation.messages.last() {
            target_conversation.last_read_message_id = last_message.id.clone();
            if last_message.role.trim().eq_ignore_ascii_case("assistant") {
                target_conversation.last_assistant_at = Some(now.clone());
            } else if last_message.role.trim().eq_ignore_ascii_case("user") {
                target_conversation.last_user_at = Some(now.clone());
            }
        }
    }

    let overview_payload =
        build_unarchived_conversation_overview_payload(state.inner(), &app_config, &data);
    persist_single_conversation_runtime_fast(state.inner(), &data, target_conversation_id)?;
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);
    runtime_log_info(format!(
        "[投送消息] 完成，任务=投送已选消息到目标会话，source_conversation_id={}，target_conversation_id={}，message_count={}",
        source_conversation_id,
        target_conversation_id,
        selected_messages.len()
    ));

    Ok(DeliverUnarchivedConversationSelectionOutput {
        target_conversation_id: target_conversation_id.to_string(),
        delivered_count: selected_messages.len(),
    })
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
    let mut target_conversation_ids = Vec::<String>::new();
    if !active_conversation_id.trim().is_empty() {
        target_conversation_ids.push(active_conversation_id.clone());
    }
    persist_removed_and_selected_conversations_and_runtime(
        state.inner(),
        &data,
        std::slice::from_ref(&conversation_id.to_string()),
        &target_conversation_ids,
        "delete_unarchived_conversation",
    )?;
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
) -> Result<(usize, usize, Option<String>, Vec<ConversationTodoItem>), String> {
    let removed_count = conversation.messages.len().saturating_sub(remove_from);
    conversation.messages.truncate(remove_from);
    restore_conversation_todos_after_rewind(conversation)?;
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
    Ok((
        removed_count,
        remove_from,
        conversation_current_todo_text(conversation),
        conversation.current_todos.clone(),
    ))
}

fn latest_todos_from_message_tool_history(
    message: &ChatMessage,
) -> Result<Option<Vec<ConversationTodoItem>>, String> {
    for event in normalize_message_tool_history_events(message, MessageToolHistoryView::Display)
        .into_iter()
        .rev()
    {
        if event.role != "assistant" {
            continue;
        }
        for call in event.tool_calls.into_iter().rev() {
            if call.tool_name.as_deref().map(str::trim) != Some("todo") {
                continue;
            }
            let raw_arguments = match &call.raw_arguments {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let request = serde_json::from_str::<TodoWriteRequest>(&raw_arguments)
                .map_err(|err| format!("todo 参数不是合法 JSON：{err}"))?;
            let normalized = todo_items_normalized(&request.todos)?;
            let stored = if !normalized.is_empty()
                && normalized.iter().all(|item| item.status == "completed")
            {
                Vec::new()
            } else {
                normalized
            };
            return Ok(Some(stored));
        }
    }
    Ok(None)
}

fn restore_conversation_todos_after_rewind(conversation: &mut Conversation) -> Result<(), String> {
    let mut restored = None::<Vec<ConversationTodoItem>>;
    for message in conversation.messages.iter().rev() {
        if let Some(todos) = latest_todos_from_message_tool_history(message)? {
            restored = Some(todos);
            break;
        }
    }
    conversation.current_todos = restored.unwrap_or_default();
    Ok(())
}

#[tauri::command]
async fn rewind_conversation_from_message(
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
    let (
        conversation_id,
        removed_count,
        remaining_count,
        current_todo,
        current_todos,
        mut recalled_user_message,
        git_snapshot,
    ) = {
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
        let git_snapshot = recalled_user_message
            .as_ref()
            .and_then(|message| read_git_snapshot_record_from_provider_meta(message.provider_meta.as_ref()));
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
        let (removed_count, remaining_count, current_todo, current_todos) =
            persist_rewind_conversation_state(conversation, remove_from)?;
        (
            conversation_id,
            removed_count,
            remaining_count,
            current_todo,
            current_todos,
            recalled_user_message,
            git_snapshot,
        )
    };
    if removed_count > 0 {
        persist_single_conversation_runtime_fast(&state, &data, &conversation_id)?;
    }
    drop(guard);

    if removed_count > 0 {
        emit_conversation_todos_updated_payload(
            state.inner(),
            &ConversationTodosUpdatedPayload {
                conversation_id: conversation_id.clone(),
                current_todo,
                current_todos,
            },
        );
        let _ = emit_unarchived_conversation_overview_updated_from_state(state.inner());
    }

    if let Some(snapshot) = git_snapshot.as_ref() {
        if snapshot.status.trim() == "created"
            && snapshot
                .ghost_commit_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
        {
            runtime_log_info(format!(
                "[会话撤回] 开始 Git 幽灵快照恢复，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，workspace={}",
                conversation_id,
                message_id,
                snapshot.main_workspace_path
            ));
            match restore_main_workspace_from_git_ghost_snapshot(snapshot).await {
                Ok(()) => runtime_log_info(format!(
                    "[会话撤回] Git 幽灵快照恢复完成，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，commit_id={}",
                    conversation_id,
                    message_id,
                    snapshot.ghost_commit_id.as_deref().unwrap_or_default()
                )),
                Err(err) => runtime_log_error(format!(
                    "[会话撤回] Git 幽灵快照恢复失败，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，error={}",
                    conversation_id, message_id, err
                )),
            }
        } else {
            runtime_log_info(format!(
                "[会话撤回] 跳过 Git 幽灵快照恢复，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，status={}",
                conversation_id, message_id, snapshot.status
            ));
        }
    }

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

#[cfg(test)]
mod unarchived_conversations_tests {
    use super::*;

    fn build_test_message(id: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
            created_at: "2026-04-18T10:00:00Z".to_string(),
            speaker_agent_id: Some("agent-a".to_string()),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn build_test_conversation() -> Conversation {
        Conversation {
            id: "source-conversation".to_string(),
            title: "原会话".to_string(),
            agent_id: "agent-a".to_string(),
            department_id: "dept-a".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:01:00Z".to_string(),
            last_user_at: None,
            last_assistant_at: Some("2026-04-18T10:01:00Z".to_string()),
            last_context_usage_ratio: 0.4,
            last_effective_prompt_tokens: 2048,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![build_test_message("m1", "hello"), build_test_message("m2", "world")],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: true,
        }
    }

    fn build_test_compaction_message(id: &str) -> ChatMessage {
        let mut message = build_test_message(id, "[上下文整理]");
        message.role = "user".to_string();
        message.speaker_agent_id = Some(SYSTEM_PERSONA_ID.to_string());
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "scene": "compaction",
            }
        }));
        message
    }

    fn build_test_todo_tool_message(
        id: &str,
        todos: serde_json::Value,
        tool_result: &str,
    ) -> ChatMessage {
        let mut message = build_test_message(id, "");
        message.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": format!("call_{id}"),
                    "type": "function",
                    "function": {
                        "name": "todo",
                        "arguments": serde_json::json!({ "todos": todos }),
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": format!("call_{id}"),
                "content": tool_result,
            }),
        ]);
        message
    }

    #[test]
    fn persist_rewind_conversation_state_should_restore_previous_todos_from_remaining_messages() {
        let mut conversation = build_test_conversation();
        conversation.messages = vec![
            ChatMessage {
                id: "user-1".to_string(),
                role: "user".to_string(),
                created_at: "2026-04-18T10:00:00Z".to_string(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "先做任务".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            build_test_todo_tool_message(
                "assistant-1",
                serde_json::json!([
                    { "content": "第一步", "status": "completed" },
                    { "content": "第二步", "status": "in_progress" }
                ]),
                "## Current Todo List\n\n✓ 第一步\n→ 第二步",
            ),
            ChatMessage {
                id: "user-2".to_string(),
                role: "user".to_string(),
                created_at: "2026-04-18T10:02:00Z".to_string(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "再做一轮".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            build_test_todo_tool_message(
                "assistant-2",
                serde_json::json!([{ "content": "新步骤", "status": "in_progress" }]),
                "## Current Todo List\n\n→ 新步骤",
            ),
        ];
        conversation.current_todos = vec![ConversationTodoItem {
            content: "新步骤".to_string(),
            status: "in_progress".to_string(),
        }];

        let (removed_count, remaining_count, current_todo, current_todos) =
            persist_rewind_conversation_state(&mut conversation, 2).expect("rewind state");

        assert_eq!(removed_count, 2);
        assert_eq!(remaining_count, 2);
        assert_eq!(current_todo.as_deref(), Some("第二步"));
        assert_eq!(current_todos.len(), 2);
        assert_eq!(conversation.current_todos.len(), 2);
        assert_eq!(conversation.current_todos[0].content, "第一步");
        assert_eq!(conversation.current_todos[0].status, "completed");
        assert_eq!(conversation.current_todos[1].content, "第二步");
        assert_eq!(conversation.current_todos[1].status, "in_progress");
    }

    #[test]
    fn persist_rewind_conversation_state_should_clear_todos_when_no_history_found() {
        let mut conversation = build_test_conversation();
        conversation.current_todos = vec![ConversationTodoItem {
            content: "残留步骤".to_string(),
            status: "in_progress".to_string(),
        }];

        let (removed_count, remaining_count, current_todo, current_todos) =
            persist_rewind_conversation_state(&mut conversation, 1).expect("rewind state");

        assert_eq!(removed_count, 1);
        assert_eq!(remaining_count, 1);
        assert_eq!(current_todo, None);
        assert!(current_todos.is_empty());
        assert!(conversation.current_todos.is_empty());
    }

    #[test]
    fn clone_foreground_conversation_for_copy_should_record_parent_and_fork_cursor() {
        let source = build_test_conversation();
        let cloned = clone_foreground_conversation_for_copy(&source, "agent-b", "dept-b", "");

        assert_ne!(cloned.id, source.id);
        assert_eq!(cloned.title, source.title);
        assert_eq!(cloned.parent_conversation_id.as_deref(), Some(source.id.as_str()));
        assert!(cloned.bound_conversation_id.is_none());
        assert_eq!(cloned.agent_id, "agent-b");
        assert_eq!(cloned.department_id, "dept-b");
        assert_eq!(cloned.messages.len(), source.messages.len());
        assert_ne!(cloned.messages[0].id, source.messages[0].id);
        assert_eq!(
            cloned.fork_message_cursor.as_deref(),
            cloned.messages.last().map(|message| message.id.as_str())
        );
    }

    #[test]
    fn collect_selected_messages_for_derive_should_keep_source_order_and_visible_ordinal() {
        let mut source = build_test_conversation();
        source.messages.insert(
            0,
            build_initial_summary_context_message(Some("历史摘要"), None, None),
        );
        let (selected, first_selected_ordinal) = collect_selected_messages_for_derive(
            &source,
            &["m2".to_string(), "m1".to_string()],
        );

        assert_eq!(first_selected_ordinal, 1);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].id, "m1");
        assert_eq!(selected[1].id, "m2");
    }

    #[test]
    fn build_derived_conversation_title_should_include_source_title_and_ordinal() {
        assert_eq!(
            build_derived_conversation_title("原会话", 7, false),
            "原会话[派生自第7条对话]"
        );
        assert_eq!(
            build_derived_conversation_title("Chat 2026-04-18T10:00", 3, true),
            "主会话[派生自第3条对话]"
        );
    }

    #[test]
    fn build_derived_conversation_record_should_copy_latest_compaction_and_selected_messages() {
        let mut data = AppData::default();
        data.assistant_department_agent_id = "agent-a".to_string();
        data.user_alias = "用户".to_string();
        data.agents.push(AgentProfile {
            id: "agent-a".to_string(),
            name: "助手".to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            source: "manual".to_string(),
            scope: "global".to_string(),
        });
        let source = Conversation {
            messages: vec![
                build_test_compaction_message("seed-1"),
                build_test_message("m1", "hello"),
                build_test_compaction_message("seed-2"),
                build_test_message("m2", "world"),
            ],
            ..build_test_conversation()
        };
        let department = DepartmentConfig {
            id: "dept-a".to_string(),
            name: "部门".to_string(),
            summary: String::new(),
            guide: String::new(),
            agent_ids: vec!["agent-a".to_string()],
            api_config_id: "api-a".to_string(),
            api_config_ids: vec!["api-a".to_string()],
            order_index: 0,
            is_built_in_assistant: false,
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:00:00Z".to_string(),
            source: "main_config".to_string(),
            scope: "global".to_string(),
            permission_control: DepartmentPermissionControl::default(),
        };

        let derived = build_derived_conversation_record_from_selection(
            &PathBuf::from("."),
            &data,
            &source,
            &department,
            "派生标题",
            latest_compaction_message_for_derive(&source).as_ref(),
            &[source.messages[1].clone(), source.messages[3].clone()],
        );

        assert_eq!(derived.messages.len(), 3);
        assert_eq!(
            render_prompt_message_text(&derived.messages[0]),
            render_prompt_message_text(&source.messages[2])
        );
        assert_eq!(
            render_prompt_message_text(&derived.messages[1]),
            render_prompt_message_text(&source.messages[1])
        );
        assert_eq!(
            render_prompt_message_text(&derived.messages[2]),
            render_prompt_message_text(&source.messages[3])
        );
        assert_ne!(derived.messages[0].id, source.messages[2].id);
    }
}
