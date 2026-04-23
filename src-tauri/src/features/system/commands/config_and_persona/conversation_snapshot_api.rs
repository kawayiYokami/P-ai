#[tauri::command]
fn get_chat_snapshot(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    conversation_service().read_chat_snapshot(state.inner(), &input)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationPreviewMessage {
    message_id: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text_preview: String,
    #[serde(default)]
    has_image: bool,
    #[serde(default)]
    has_pdf: bool,
    #[serde(default)]
    has_audio: bool,
    #[serde(default)]
    has_attachment: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnarchivedConversationSummary {
    conversation_id: String,
    title: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_message_at: Option<String>,
    message_count: usize,
    unread_count: usize,
    agent_id: String,
    department_id: String,
    department_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fork_message_cursor: Option<String>,
    workspace_label: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_main_conversation: bool,
    #[serde(default)]
    is_pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_state: Option<MainSessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default)]
    plan_mode_enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    preview_messages: Vec<ConversationPreviewMessage>,
}

fn conversation_current_todo_text(conversation: &Conversation) -> Option<String> {
    conversation
        .current_todos
        .iter()
        .find(|item| item.status.trim().eq_ignore_ascii_case("in_progress"))
        .or_else(|| {
            conversation.current_todos.iter().find(|item| {
                !item.status.trim().eq_ignore_ascii_case("completed") && !item.content.trim().is_empty()
            })
        })
        .map(|item| item.content.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn conversation_unread_count(conversation: &Conversation) -> usize {
    let anchor = conversation.last_read_message_id.trim();
    if anchor.is_empty() {
        return conversation.messages.len();
    }
    let Some(anchor_index) = conversation
        .messages
        .iter()
        .position(|message| message.id.trim() == anchor)
    else {
        return conversation.messages.len();
    };
    conversation.messages.len().saturating_sub(anchor_index + 1)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DelegateConversationSummary {
    conversation_id: String,
    title: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_message_at: Option<String>,
    message_count: usize,
    agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
}

fn conversation_preview_title(conversation: &Conversation) -> String {
    let text = conversation
        .messages
        .iter()
        .find(|m| {
            m.role == "user"
                && m
                    .speaker_agent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    != Some(SYSTEM_PERSONA_ID)
        })
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text } => Some(text.trim()),
                    _ => None,
                })
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let compact = clean_text(text.trim());
    let sentence = compact
        .split(['。', '！', '？', '!', '?', ';', '；', '\n', '\r'])
        .map(str::trim)
        .find(|segment| !segment.is_empty())
        .unwrap_or("");
    let preview = if sentence.is_empty() { compact.as_str() } else { sentence };
    if preview.is_empty() {
        "无内容".to_string()
    } else {
        preview.chars().take(12).collect::<String>()
    }
}

fn build_conversation_preview_text(message: &ChatMessage) -> String {
    let text = message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    clean_text(text.trim())
}

fn conversation_message_has_attachment(message: &ChatMessage) -> bool {
    message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("attachments"))
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

fn build_conversation_preview_messages(
    conversation: &Conversation,
    limit: usize,
) -> Vec<ConversationPreviewMessage> {
    let mut selected = conversation
        .messages
        .iter()
        .filter(|message| {
            if is_tool_review_report_message(message) {
                return false;
            }
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant" | "tool"
            )
        })
        .rev()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    selected.reverse();
    selected
        .into_iter()
        .map(|message| {
            let mut has_image = false;
            let mut has_pdf = false;
            let mut has_audio = false;
            for part in &message.parts {
                match part {
                    MessagePart::Image { mime, .. } => {
                        if mime.trim().eq_ignore_ascii_case("application/pdf") {
                            has_pdf = true;
                        } else {
                            has_image = true;
                        }
                    }
                    MessagePart::Audio { .. } => {
                        has_audio = true;
                    }
                    MessagePart::Text { .. } => {}
                }
            }
            ConversationPreviewMessage {
                message_id: message.id.clone(),
                role: message.role.clone(),
                speaker_agent_id: message
                    .speaker_agent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                created_at: Some(message.created_at.clone())
                    .filter(|value| !value.trim().is_empty()),
                text_preview: build_conversation_preview_text(&message),
                has_image,
                has_pdf,
                has_audio,
                has_attachment: conversation_message_has_attachment(&message),
            }
        })
        .collect()
}

fn normalized_pinned_conversation_ids(data: &AppData) -> Vec<String> {
    let main_conversation_id = data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let visible_ids = data
        .conversations
        .iter()
        .filter(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
        })
        .map(|conversation| conversation.id.trim().to_string())
        .filter(|conversation_id| !conversation_id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut seen = std::collections::HashSet::<String>::new();
    data.pinned_conversation_ids
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| *item != main_conversation_id)
        .filter(|item| visible_ids.contains(item))
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

fn build_unarchived_conversation_summary(
    _state: &AppState,
    app_config: &AppConfig,
    main_conversation_id: &str,
    pinned_conversation_ids: &[String],
    conversation: &Conversation,
) -> UnarchivedConversationSummary {
    let last_message_at = conversation.messages.last().map(|m| m.created_at.clone());
    let conversation_id = conversation.id.trim();
    let is_main_conversation = conversation_id == main_conversation_id;
    let pin_index = pinned_conversation_ids
        .iter()
        .position(|item| item.trim() == conversation_id);
    let department_id = resolved_foreground_department_id_for_conversation(
        app_config,
        conversation,
        is_main_conversation,
    );
    let department_name = department_by_id(app_config, &department_id)
        .map(|department| department.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| department_id.clone());
    UnarchivedConversationSummary {
        conversation_id: conversation.id.clone(),
        title: if conversation.title.trim().is_empty() {
            conversation_preview_title(conversation)
        } else {
            conversation.title.clone()
        },
        updated_at: conversation.updated_at.clone(),
        last_message_at,
        message_count: conversation
            .messages
            .iter()
            .filter(|message| !is_tool_review_report_message(message))
            .count(),
        unread_count: conversation_unread_count(conversation),
        agent_id: conversation.agent_id.clone(),
        department_id,
        department_name,
        parent_conversation_id: conversation
            .parent_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        fork_message_cursor: conversation
            .fork_message_cursor
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        // 会话列表预览保持最轻量，只返回标题/时间/基础标识与最近预览消息。
        // 工作区、运行态、计划模式等重字段走其他链路，不在这里同步重算。
        workspace_label: String::new(),
        is_active: conversation.status.trim() == "active",
        is_main_conversation,
        is_pinned: is_main_conversation || pin_index.is_some(),
        pin_index,
        runtime_state: None,
        current_todo: None,
        plan_mode_enabled: false,
        preview_messages: build_conversation_preview_messages(conversation, 2),
    }
}

fn unarchived_conversation_sort_key(summary: &UnarchivedConversationSummary) -> (&str, &str) {
    (
        summary
            .last_message_at
            .as_deref()
            .unwrap_or(summary.updated_at.as_str()),
        summary.updated_at.as_str(),
    )
}

fn sort_unarchived_conversation_summaries(
    summaries: Vec<UnarchivedConversationSummary>,
) -> Vec<UnarchivedConversationSummary> {
    let mut ordered = summaries;
    ordered.sort_by(|a, b| {
        if a.is_main_conversation != b.is_main_conversation {
            return b.is_main_conversation.cmp(&a.is_main_conversation);
        }
        if a.is_pinned != b.is_pinned {
            return b.is_pinned.cmp(&a.is_pinned);
        }
        if a.is_pinned && b.is_pinned {
            let a_index = a.pin_index.unwrap_or(usize::MAX);
            let b_index = b.pin_index.unwrap_or(usize::MAX);
            return a_index
                .cmp(&b_index)
                .then_with(|| a.conversation_id.cmp(&b.conversation_id));
        }
        let (a_primary, a_secondary) = unarchived_conversation_sort_key(a);
        let (b_primary, b_secondary) = unarchived_conversation_sort_key(b);
        b_primary
            .cmp(a_primary)
            .then_with(|| b_secondary.cmp(a_secondary))
            .then_with(|| a.conversation_id.cmp(&b.conversation_id))
    });
    ordered
}

fn collect_unarchived_conversation_summaries(
    state: &AppState,
    app_config: &AppConfig,
    data: &AppData,
) -> Vec<UnarchivedConversationSummary> {
    let main_conversation_id = data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let pinned_conversation_ids = normalized_pinned_conversation_ids(data);
    let summaries = data
        .conversations
        .iter()
        .filter(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
        })
        .map(|conversation| {
            build_unarchived_conversation_summary(
                state,
                app_config,
                &main_conversation_id,
                &pinned_conversation_ids,
                conversation,
            )
        })
        .collect::<Vec<_>>();
    sort_unarchived_conversation_summaries(summaries)
}

fn delegate_conversation_summary_from_runtime_thread(
    thread: &DelegateRuntimeThread,
) -> DelegateConversationSummary {
    let last_message_at = thread
        .conversation
        .messages
        .last()
        .map(|m| m.created_at.clone());
    DelegateConversationSummary {
        conversation_id: thread.delegate_id.clone(),
        title: if thread.title.trim().is_empty() {
            conversation_preview_title(&thread.conversation)
        } else {
            thread.title.clone()
        },
        updated_at: thread.conversation.updated_at.clone(),
        last_message_at,
        message_count: thread.conversation.messages.len(),
        agent_id: thread.target_agent_id.clone(),
        delegate_id: Some(thread.delegate_id.clone()),
        root_conversation_id: Some(thread.root_conversation_id.clone()),
        archived_at: thread.archived_at.clone(),
    }
}

fn unarchived_conversation_runtime_state(
    state: &AppState,
    conversation_id: &str,
) -> Option<MainSessionState> {
    match get_conversation_runtime_state(state, conversation_id) {
        Ok(MainSessionState::Idle) => None,
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!(
                "[会话] 读取运行态失败，任务=unarchived_runtime_state，conversation_id={}，error={}",
                conversation_id, err
            );
            None
        }
    }
}

fn ensure_unarchived_conversation_not_organizing(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    if get_conversation_runtime_state(state, conversation_id)? == MainSessionState::OrganizingContext {
        return Err("当前会话正在后台归档或整理上下文，暂时不能切换。".to_string());
    }
    Ok(())
}

#[tauri::command]
fn list_unarchived_conversations(state: State<'_, AppState>) -> Result<Vec<UnarchivedConversationSummary>, String> {
    Ok(
        conversation_service()
            .list_unarchived_conversation_summaries(state.inner())?
            .summaries,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetActiveUnarchivedConversationInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetActiveUnarchivedConversationOutput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchActiveConversationSnapshotInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForegroundConversationLightSnapshotInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchActiveConversationSnapshotOutput {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    has_more_history: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_state: Option<MainSessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    current_todos: Vec<ConversationTodoItem>,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForegroundConversationLightSnapshotOutput {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    has_more_history: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_state: Option<MainSessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    current_todos: Vec<ConversationTodoItem>,
}

#[derive(Debug, Clone)]
struct ForegroundConversationSnapshotCore {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    has_more_history: bool,
    runtime_state: Option<MainSessionState>,
    current_todo: Option<String>,
    current_todos: Vec<ConversationTodoItem>,
}

const DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConversationPlanModeInput {
    conversation_id: String,
    plan_mode_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConversationPlanModeOutput {
    conversation_id: String,
    plan_mode_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkConversationReadInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnarchivedConversationOverviewUpdatedPayload {
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationTodosUpdatedPayload {
    conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    current_todos: Vec<ConversationTodoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationPinUpdatedPayload {
    conversation_id: String,
    is_pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_index: Option<usize>,
}

fn emit_unarchived_conversation_overview_updated_payload(
    state: &AppState,
    payload: &UnarchivedConversationOverviewUpdatedPayload,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            eprintln!("[会话概览] 获取 app_handle 失败：锁已损坏，error={:?}", err);
            None
        }
    };
    let Some(app_handle) = app_handle else {
        eprintln!("[会话概览] 推送跳过：无法获取 app_handle");
        return;
    };
    if let Err(err) = app_handle.emit(CHAT_CONVERSATION_OVERVIEW_UPDATED_EVENT, payload) {
        eprintln!("[会话概览] 推送失败：错误={}", err);
    }
}

fn emit_conversation_todos_updated_payload(
    state: &AppState,
    payload: &ConversationTodosUpdatedPayload,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            eprintln!("[Todo] 获取 app_handle 失败：锁已损坏，error={:?}", err);
            None
        }
    };
    let Some(app_handle) = app_handle else {
        eprintln!("[Todo] 推送跳过：无法获取 app_handle");
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-todos-updated", payload) {
        eprintln!("[Todo] 推送失败：错误={}", err);
    }
}

fn emit_conversation_pin_updated_payload(
    state: &AppState,
    payload: &ConversationPinUpdatedPayload,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            eprintln!("[会话置顶] 获取 app_handle 失败：锁已损坏，error={:?}", err);
            None
        }
    };
    let Some(app_handle) = app_handle else {
        eprintln!("[会话置顶] 推送跳过：无法获取 app_handle");
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-pin-updated", payload) {
        eprintln!("[会话置顶] 推送失败：错误={}", err);
    }
}

fn normalize_conversation_todos(
    todos: Vec<ConversationTodoItem>,
) -> Vec<ConversationTodoItem> {
    todos
        .into_iter()
        .filter_map(|item| {
            let content = item.content.trim().to_string();
            if content.is_empty() {
                return None;
            }
            let status = item.status.trim().to_ascii_lowercase();
            if !matches!(status.as_str(), "pending" | "in_progress" | "completed") {
                return None;
            }
            Some(ConversationTodoItem { content, status })
        })
        .collect()
}

fn update_conversation_todos_and_emit(
    state: &AppState,
    conversation_id: &str,
    todos: Vec<ConversationTodoItem>,
) -> Result<(), String> {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return Ok(());
    }
    let next_todos = normalize_conversation_todos(todos);
    let stored_todos = if !next_todos.is_empty()
        && next_todos.iter().all(|item| item.status == "completed")
    {
        Vec::new()
    } else {
        next_todos.clone()
    };
    if let Some(mut conversation) = delegate_runtime_thread_conversation_get(state, cid)? {
        if conversation.current_todos == stored_todos {
            return Ok(());
        }
        conversation.current_todos = stored_todos.clone();
        conversation.updated_at = now_iso();
        let current_todo = conversation_current_todo_text(&conversation);
        delegate_runtime_thread_conversation_update(state, cid, conversation)?;
        let todo_payload = ConversationTodosUpdatedPayload {
            conversation_id: cid.to_string(),
            current_todo,
            current_todos: stored_todos,
        };
        emit_conversation_todos_updated_payload(state, &todo_payload);
        return Ok(());
    }
    let Some(todo_update) = conversation_service().update_conversation_todos(
        state,
        cid,
        &stored_todos,
    )? else {
        return Ok(());
    };
    let todo_payload = ConversationTodosUpdatedPayload {
        conversation_id: cid.to_string(),
        current_todo: todo_update.current_todo,
        current_todos: stored_todos,
    };
    let overview_payload = conversation_service().refresh_unarchived_conversation_overview_payload(state)?;
    emit_conversation_todos_updated_payload(state, &todo_payload);
    emit_unarchived_conversation_overview_updated_payload(state, &overview_payload);
    Ok(())
}

fn emit_unarchived_conversation_overview_updated_from_state(state: &AppState) -> Result<(), String> {
    let total_started_at = std::time::Instant::now();
    let payload_started_at = std::time::Instant::now();
    let payload = conversation_service().refresh_unarchived_conversation_overview_payload(state)?;
    let payload_elapsed_ms = payload_started_at.elapsed().as_millis();
    let emit_started_at = std::time::Instant::now();
    emit_unarchived_conversation_overview_updated_payload(state, &payload);
    let emit_elapsed_ms = emit_started_at.elapsed().as_millis();
    eprintln!(
        "[会话概览] 状态刷新耗时：总计={}ms，构建概览={}ms，事件推送={}ms",
        total_started_at.elapsed().as_millis(),
        payload_elapsed_ms,
        emit_elapsed_ms
    );
    Ok(())
}

#[tauri::command]
fn set_active_unarchived_conversation(
    input: SetActiveUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<SetActiveUnarchivedConversationOutput, String> {
    let conversation_id =
        conversation_service().set_active_unarchived_conversation(state.inner(), &input)?;
    Ok(SetActiveUnarchivedConversationOutput { conversation_id })
}

#[cfg(test)]
mod conversation_snapshot_api_tests {
    use super::*;

    fn test_summary(
        conversation_id: &str,
        updated_at: &str,
        parent_conversation_id: Option<&str>,
    ) -> UnarchivedConversationSummary {
        UnarchivedConversationSummary {
            conversation_id: conversation_id.to_string(),
            title: conversation_id.to_string(),
            updated_at: updated_at.to_string(),
            last_message_at: Some(updated_at.to_string()),
            message_count: 1,
            unread_count: 0,
            agent_id: "agent-a".to_string(),
            department_id: "dept-a".to_string(),
            department_name: "部门A".to_string(),
            parent_conversation_id: parent_conversation_id.map(ToOwned::to_owned),
            fork_message_cursor: None,
            workspace_label: "默认工作空间".to_string(),
            is_active: false,
            is_main_conversation: false,
            is_pinned: false,
            pin_index: None,
            runtime_state: None,
            current_todo: None,
            plan_mode_enabled: false,
            preview_messages: Vec::new(),
        }
    }

    #[test]
    fn sort_unarchived_conversation_summaries_should_group_main_pinned_and_recent() {
        let mut main = test_summary("main", "2026-04-18T10:00:00Z", None);
        main.is_main_conversation = true;
        main.is_pinned = true;
        let mut pinned = test_summary("pinned", "2026-04-18T10:01:00Z", None);
        pinned.is_pinned = true;
        pinned.pin_index = Some(0);
        let recent = test_summary("recent", "2026-04-18T10:03:00Z", None);
        let older = test_summary("older", "2026-04-18T10:02:00Z", None);
        let ordered = sort_unarchived_conversation_summaries(vec![
            older,
            recent,
            pinned,
            main,
        ]);
        let ids = ordered
            .iter()
            .map(|item| item.conversation_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["main", "pinned", "recent", "older"]);
    }
}
