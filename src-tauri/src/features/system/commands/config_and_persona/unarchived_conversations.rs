mod git_ghost_snapshot;

use git_ghost_snapshot::read_git_snapshot_record_from_provider_meta;
use git_ghost_snapshot::restore_main_workspace_from_git_ghost_snapshot;

#[tauri::command]
fn switch_active_conversation_snapshot(
    input: SwitchActiveConversationSnapshotInput,
    state: State<'_, AppState>,
) -> Result<SwitchActiveConversationSnapshotOutput, String> {
    let started_at = std::time::Instant::now();
    let result =
        conversation_service().switch_active_conversation_snapshot(state.inner(), &input)?;
    let snapshot = result.snapshot;
    let unarchived_conversations = result.unarchived_conversations;
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

#[tauri::command]
fn get_foreground_conversation_light_snapshot(
    input: ForegroundConversationLightSnapshotInput,
    state: State<'_, AppState>,
) -> Result<ForegroundConversationLightSnapshotOutput, String> {
    let started_at = std::time::Instant::now();
    let recent_limit = input
        .limit
        .unwrap_or(DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT)
        .clamp(1, 50);
    let snapshot = conversation_service().read_foreground_snapshot(
        state.inner(),
        input.conversation_id.as_deref(),
        input.agent_id.as_deref(),
        recent_limit,
    )?;
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
struct BranchUnarchivedConversationFromSelectionInput {
    source_conversation_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchUnarchivedConversationFromSelectionOutput {
    conversation_id: String,
    title: String,
    #[serde(default)]
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardUnarchivedConversationSelectionInput {
    source_conversation_id: String,
    target_conversation_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardUnarchivedConversationSelectionOutput {
    target_conversation_id: String,
    forwarded_count: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToggleUnarchivedConversationPinInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToggleUnarchivedConversationPinOutput {
    conversation_id: String,
    is_pinned: bool,
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

fn build_branch_conversation_title(
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
    format!("{prefix}[会话分支自第{first_selected_ordinal}条对话]")
}

fn collect_selected_messages_for_branch(
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

fn branch_conversation_settings_agent_id(
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

fn build_branch_conversation_record_from_selection(
    data_path: &PathBuf,
    data: &AppData,
    source: &Conversation,
    department: &DepartmentConfig,
    title: &str,
    latest_compaction_message: Option<&ChatMessage>,
    selected_messages: &[ChatMessage],
) -> Conversation {
    let agent_id = branch_conversation_settings_agent_id(data, department, &source.agent_id);
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
                    "[会话分支] 跳过，任务=构建用户画像快照，agent_id={}，error={}",
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

fn latest_compaction_message_for_branch(source: &Conversation) -> Option<ChatMessage> {
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
    let result = conversation_service().create_unarchived_conversation(state.inner(), &input)?;
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &result.overview_payload);
    Ok(CreateUnarchivedConversationOutput {
        conversation_id: result.conversation_id,
    })
}

#[tauri::command]
async fn branch_unarchived_conversation_from_selection(
    input: BranchUnarchivedConversationFromSelectionInput,
    state: State<'_, AppState>,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
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

    let result = conversation_service().branch_unarchived_conversation_from_selection(
        state.inner(),
        source_conversation_id,
        &normalized_selected_message_ids,
    )?;
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &result.overview_payload);
    runtime_log_info(format!(
        "[会话分支] 完成，任务=按已选消息创建会话分支，source_conversation_id={}，conversation_id={}，selected_count={}，has_compaction_seed={}",
        source_conversation_id,
        result.conversation_id,
        result.selected_count,
        result.has_compaction_seed
    ));

    Ok(BranchUnarchivedConversationFromSelectionOutput {
        conversation_id: result.conversation_id,
        title: result.title,
        warning: None,
    })
}

#[tauri::command]
fn forward_unarchived_conversation_selection(
    input: ForwardUnarchivedConversationSelectionInput,
    state: State<'_, AppState>,
) -> Result<ForwardUnarchivedConversationSelectionOutput, String> {
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

    let result = conversation_service().forward_unarchived_conversation_selection(
        state.inner(),
        source_conversation_id,
        target_conversation_id,
        &normalized_selected_message_ids,
    )?;
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &result.overview_payload);
    runtime_log_info(format!(
        "[转发到会话] 完成，任务=转发已选消息到目标会话，source_conversation_id={}，target_conversation_id={}，message_count={}",
        source_conversation_id,
        result.target_conversation_id,
        result.forwarded_count
    ));

    Ok(ForwardUnarchivedConversationSelectionOutput {
        target_conversation_id: result.target_conversation_id,
        forwarded_count: result.forwarded_count,
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
    let next_title = conversation_service().rename_unarchived_conversation(
        state.inner(),
        conversation_id,
        &next_title,
    )?;

    let overview_payload =
        conversation_service().refresh_unarchived_conversation_overview_payload(state.inner())?;

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
fn toggle_unarchived_conversation_pin(
    input: ToggleUnarchivedConversationPinInput,
    state: State<'_, AppState>,
) -> Result<ToggleUnarchivedConversationPinOutput, String> {
    let result = conversation_service().toggle_unarchived_conversation_pin(
        state.inner(),
        &input.conversation_id,
    )?;
    runtime_log_info(format!(
        "[会话] 完成，任务=切换会话置顶，conversation_id={}，is_pinned={}",
        result.conversation_id, result.is_pinned
    ));
    emit_conversation_pin_updated_payload(
        state.inner(),
        &ConversationPinUpdatedPayload {
            conversation_id: result.conversation_id.clone(),
            is_pinned: result.is_pinned,
            pin_index: result.pin_index,
        },
    );

    Ok(ToggleUnarchivedConversationPinOutput {
        conversation_id: result.conversation_id,
        is_pinned: result.is_pinned,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationMessageByIdInput {
    conversation_id: String,
    message_id: String,
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
    conversation_service().read_unarchived_messages(state.inner(), conversation_id)
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
    conversation_service().read_recent_unarchived_messages(
        state.inner(),
        conversation_id,
        input.limit,
    )
}

#[tauri::command]
fn get_unarchived_conversation_message_by_id(
    input: GetUnarchivedConversationMessageByIdInput,
    state: State<'_, AppState>,
) -> Result<ChatMessage, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let message_id = input.message_id.trim();
    if message_id.is_empty() {
        return Err("messageId is required.".to_string());
    }
    conversation_service().read_message_by_id(state.inner(), conversation_id, message_id)
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
    let result = match conversation_service().delete_unarchived_conversation(
        state.inner(),
        conversation_id,
    ) {
        Ok(result) => result,
        Err(err) => {
            let reason = if err.contains("主会话暂不支持删除") {
                "main_conversation_locked"
            } else if err.contains("Unarchived conversation not found") {
                "not_found"
            } else if err.contains("删除后未找到可用会话") {
                "no_active_conversation_after_delete"
            } else {
                "delete_failed"
            };
            runtime_log_info(format!(
                "[会话] 失败，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}，reason={}，duration_ms={}",
                conversation_id,
                reason,
                started_at.elapsed().as_millis()
            ));
            return Err(err);
        }
    };
    runtime_log_info(format!(
        "[会话] 完成，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}，duration_ms={}",
        conversation_id,
        started_at.elapsed().as_millis()
    ));
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &result.overview_payload);
    cleanup_pdf_session_memory_cache_for_conversation(conversation_id);
    Ok(DeleteUnarchivedConversationOutput {
        deleted_conversation_id: result.deleted_conversation_id,
        active_conversation_id: result.active_conversation_id,
    })
}

#[tauri::command]
fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    conversation_service().read_active_conversation_messages(state.inner(), &input)
}

#[tauri::command]
fn mark_conversation_read(
    input: MarkConversationReadInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    conversation_service().mark_conversation_read(state.inner(), &input.conversation_id)
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

fn clone_messages_after_page(
    messages: &[ChatMessage],
    after_message_id: &str,
    limit: usize,
) -> Result<Vec<ChatMessage>, String> {
    let after_idx = messages
        .iter()
        .position(|item| item.id == after_message_id)
        .ok_or_else(|| format!("afterMessageId not found: {after_message_id}"))?;
    let end = (after_idx + 1 + limit).min(messages.len());
    Ok(messages[(after_idx + 1)..end].to_vec())
}

fn clone_messages_before_page(
    messages: &[ChatMessage],
    before_message_id: &str,
    limit: usize,
) -> Result<(Vec<ChatMessage>, bool), String> {
    let before_idx = messages
        .iter()
        .position(|item| item.id == before_message_id)
        .ok_or_else(|| format!("beforeMessageId not found: {before_message_id}"))?;
    let start = before_idx.saturating_sub(limit);
    let has_more = start > 0;
    Ok((messages[start..before_idx].to_vec(), has_more))
}

fn resolve_unarchived_conversation_messages_after(
    state: &AppState,
    conversation_id: &str,
    after_message_id: Option<&str>,
    fallback_limit: usize,
) -> Result<(Vec<ChatMessage>, Option<String>), String> {
    conversation_service().read_messages_after_with_fallback(
        state,
        conversation_id,
        after_message_id,
        fallback_limit,
    )
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
    let (page, has_more) = conversation_service().read_messages_before(
        state.inner(),
        &input.session,
        before_message_id,
        limit,
    )?;
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
    let page = conversation_service().read_messages_after(
        state.inner(),
        &input.session,
        after_message_id,
        limit,
    )?;
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

    let has_requested_agent = {
        let mut app_config = read_config(&state.config_path)?;
        let data = state_read_app_data_cached(&state)?;
        let mut runtime_data = data.clone();
        merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
        runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    };
    if !has_requested_agent {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=rewind_conversation_from_message，reason=agent_not_found，agent_id={}，duration_ms={}",
            requested_agent_id, elapsed_ms
        ));
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }

    let result = conversation_service().rewind_conversation_from_message(
        state.inner(),
        &input,
        &requested_agent_id,
        &message_id,
        &started_at,
    )?;
    let conversation_id = result.conversation_id;
    let removed_count = result.removed_count;
    let remaining_count = result.remaining_count;
    let current_todo = result.current_todo;
    let current_todos = result.current_todos;
    let mut recalled_user_message = result.recalled_user_message;
    let git_snapshot = result.git_snapshot;

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
                .map(|value: &str| value.trim())
                .filter(|value: &&str| !value.is_empty())
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
    fn collect_selected_messages_for_branch_should_keep_source_order_and_visible_ordinal() {
        let mut source = build_test_conversation();
        source.messages.insert(
            0,
            build_initial_summary_context_message(Some("历史摘要"), None, None),
        );
        let (selected, first_selected_ordinal) = collect_selected_messages_for_branch(
            &source,
            &["m2".to_string(), "m1".to_string()],
        );

        assert_eq!(first_selected_ordinal, 1);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].id, "m1");
        assert_eq!(selected[1].id, "m2");
    }

    #[test]
    fn build_branch_conversation_title_should_include_source_title_and_ordinal() {
        assert_eq!(
            build_branch_conversation_title("原会话", 7, false),
            "原会话[会话分支自第7条对话]"
        );
        assert_eq!(
            build_branch_conversation_title("Chat 2026-04-18T10:00", 3, true),
            "主会话[会话分支自第3条对话]"
        );
    }

    #[test]
    fn build_branch_conversation_record_should_copy_latest_compaction_and_selected_messages() {
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

        let branched = build_branch_conversation_record_from_selection(
            &PathBuf::from("."),
            &data,
            &source,
            &department,
            "会话分支标题",
            latest_compaction_message_for_branch(&source).as_ref(),
            &[source.messages[1].clone(), source.messages[3].clone()],
        );

        assert_eq!(branched.messages.len(), 3);
        assert_eq!(
            render_prompt_message_text(&branched.messages[0]),
            render_prompt_message_text(&source.messages[2])
        );
        assert_eq!(
            render_prompt_message_text(&branched.messages[1]),
            render_prompt_message_text(&source.messages[1])
        );
        assert_eq!(
            render_prompt_message_text(&branched.messages[2]),
            render_prompt_message_text(&source.messages[3])
        );
        assert_ne!(branched.messages[0].id, source.messages[2].id);
    }
}
