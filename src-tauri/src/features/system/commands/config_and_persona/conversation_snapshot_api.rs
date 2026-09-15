#[tauri::command]
async fn get_chat_snapshot(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_chat_snapshot(&app_state, &input)
    })
    .await
    .map_err(|err| format!("读取聊天快照任务异常：{err}"))?
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
    #[serde(skip_serializing_if = "Option::is_none")]
    summary_title: Option<String>,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_message_at: Option<String>,
    message_count: usize,
    body_message_count: usize,
    body_text_length: usize,
    has_assistant_reply: bool,
    unread_count: usize,
    agent_id: String,
    #[serde(default)]
    conversation_kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    child_conversation_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    child_conversations: Vec<ChildConversationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fork_message_cursor: Option<String>,
    workspace_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_root_path: Option<String>,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_system_notification_conversation: bool,
    #[serde(default)]
    is_pinned: bool,
    #[serde(default)]
    is_draft: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_index: Option<usize>,    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_state: Option<MainSessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default)]
    plan_mode_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_push_remote_contact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    #[serde(default)]
    detached_window_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detached_window_label: Option<String>,
    state: ConversationListItemState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    preview_messages: Vec<ConversationPreviewMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChildConversationSummary {
    conversation_id: String,
    title: String,
    status: String,
    conversation_kind: String,
    parent_conversation_id: Option<String>,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationListItemState {
    activity: String,
    runtime_state: MainSessionState,
    unread_count: usize,
    open_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_viewer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_viewer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opened_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<String>,
}

const DESKTOP_CHAT_VIEWER_ID: &str = "desktop:chat";
const VSCODE_SIDEBAR_WINDOW_LABEL_PREFIX: &str = "vscode-sidebar:";
const LEGACY_IDE_CHAT_SIDEBAR_WINDOW_LABEL_PREFIX: &str = "ide-chat-sidebar-";
const CONVERSATION_PREVIEW_TEXT_CHAR_LIMIT: usize = 20;

fn chat_viewer_id_for_window_label(label: &str) -> Option<String> {
    let window_label = label.trim();
    if window_label.is_empty() {
        return None;
    }
    if window_label == "chat" || window_label == "main" {
        return Some(DESKTOP_CHAT_VIEWER_ID.to_string());
    }
    if let Some(client_id) = window_label.strip_prefix(VSCODE_SIDEBAR_WINDOW_LABEL_PREFIX) {
        let client_id = client_id.trim();
        if !client_id.is_empty() {
            return Some(format!("web:{client_id}"));
        }
    }
    if let Some(client_id) = window_label.strip_prefix(LEGACY_IDE_CHAT_SIDEBAR_WINDOW_LABEL_PREFIX) {
        let client_id = client_id.trim();
        if !client_id.is_empty() {
            return Some(format!("web:{client_id}"));
        }
    }
    Some(format!("desktop:window:{window_label}"))
}

fn opened_by_for_window_label(label: &str) -> String {
    let window_label = label.trim();
    if window_label == "chat" || window_label == "main" {
        "main".to_string()
    } else if window_label.starts_with(VSCODE_SIDEBAR_WINDOW_LABEL_PREFIX)
        || window_label.starts_with(LEGACY_IDE_CHAT_SIDEBAR_WINDOW_LABEL_PREFIX)
    {
        "vscode".to_string()
    } else {
        "main".to_string()
    }
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

fn conversation_current_todo_text_from_items(
    todos: &[ConversationTodoItem],
) -> Option<String> {
    todos.iter()
        .find(|item| item.status.trim().eq_ignore_ascii_case("in_progress"))
        .or_else(|| {
            todos.iter().find(|item| {
                !item.status.trim().eq_ignore_ascii_case("completed")
                    && !item.content.trim().is_empty()
            })
        })
        .map(|item| item.content.trim().to_string())
        .filter(|value| !value.is_empty())
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
                    MessagePart::Text { text, .. } => Some(text.trim()),
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
            MessagePart::Text { text, .. } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    truncate_conversation_preview_text(&clean_text(text.trim()))
}

fn truncate_conversation_preview_text(text: &str) -> String {
    text.chars()
        .take(CONVERSATION_PREVIEW_TEXT_CHAR_LIMIT)
        .collect()
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
    build_preview_messages_from_chat_messages(&conversation.messages, limit)
}

fn conversation_workspace_name_fallback(path_text: &str) -> String {
    let normalized = path_text.trim().replace('\\', "/");
    normalized
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path_text.trim().to_string())
}

fn conversation_default_workspace_summary_from_meta_view(
    state: &AppState,
    conversation_meta: &ConversationMetaView,
) -> (String, Option<String>) {
    let fallback = conversation_meta
        .shell_workspaces
        .iter()
        .find(|workspace| normalize_shell_workspace_level_text(&workspace.level) == SHELL_WORKSPACE_LEVEL_MAIN)
        .or_else(|| conversation_meta.shell_workspaces.first());
    if let Some(workspace) = fallback {
        let path = workspace.path.trim().to_string();
        let mut label = workspace.name.trim().to_string();
        if label.is_empty() {
            label = conversation_workspace_name_fallback(&path);
        }
        return (label, Some(path).filter(|value| !value.trim().is_empty()));
    }

    if let Some(path) = conversation_meta
        .shell_workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized_path = normalize_terminal_path_for_compare(&PathBuf::from(path));
        if let Ok(workspaces) = terminal_allowed_workspaces_canonical(state) {
            if let Some(workspace) = workspaces.into_iter().find(|workspace| {
                normalize_terminal_path_for_compare(&workspace.path) == normalized_path
            }) {
                let label = workspace.name.trim();
                if !label.is_empty() {
                    return (label.to_string(), Some(path.to_string()));
                }
            }
        }
        return (
            conversation_workspace_name_fallback(path),
            Some(path.to_string()),
        );
    }

    if let Ok(workspace) = terminal_default_workspace_for_conversation_resolved(state, None) {
        let path = workspace.path.to_string_lossy().to_string();
        let mut label = workspace.name.trim().to_string();
        if label.is_empty() {
            label = conversation_workspace_name_fallback(&path);
        }
        return (label, Some(path));
    }

    (String::new(), None)
}

fn build_preview_messages_from_chat_messages(
    messages: &[ChatMessage],
    limit: usize,
) -> Vec<ConversationPreviewMessage> {
    let mut selected = messages
        .iter()
        .filter(|message| {
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
                    MessagePart::Attachment { mime, .. } => match message_attachment_kind(mime) {
                        "image" => has_image = true,
                        "audio" => has_audio = true,
                        "pdf" => has_pdf = true,
                        _ => {}
                    },
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

fn conversation_list_open_state(
    state: &AppState,
    conversation_id: &str,
) -> (String, Option<String>, Option<String>, Option<String>) {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return ("closed".to_string(), None, None, None);
    }
    if cid == SYSTEM_NOTIFICATION_CONVERSATION_ID {
        return ("closed".to_string(), None, None, None);
    }
    if let Some(label) = detached_chat_window_for_conversation(cid) {
        let opened_by = opened_by_for_window_label(&label);
        let open_viewer_id = chat_viewer_id_for_window_label(&label);
        return ("open".to_string(), Some(opened_by), Some(label), open_viewer_id);
    }
    let opened = state
        .active_chat_view_bindings
        .lock()
        .ok()
        .and_then(|bindings| {
            bindings.values().find_map(|binding| {
                if binding.conversation_id.trim() != cid {
                    return None;
                }
                Some((
                    opened_by_for_window_label(&binding.window_label),
                    chat_viewer_id_for_window_label(&binding.window_label),
                ))
            })
        });
    if let Some((opened_by, open_viewer_id)) = opened {
        ("open".to_string(), Some(opened_by), None, open_viewer_id)
    } else {
        ("closed".to_string(), None, None, None)
    }
}

fn conversation_list_activity_mark(
    state: &AppState,
    conversation_id: &str,
) -> Option<ConversationListActivityMark> {
    state
        .conversation_list_activity_marks
        .lock()
        .ok()
        .and_then(|marks| marks.get(conversation_id.trim()).cloned())
}

fn set_conversation_list_activity_mark(
    state: &AppState,
    conversation_id: &str,
    mark: ConversationListActivityMark,
) {
    if let Ok(mut marks) = state.conversation_list_activity_marks.lock() {
        marks.insert(conversation_id.trim().to_string(), mark);
    }
}

fn clear_conversation_list_activity_mark(state: &AppState, conversation_id: &str) {
    if let Ok(mut marks) = state.conversation_list_activity_marks.lock() {
        marks.remove(conversation_id.trim());
    }
}

fn build_conversation_list_item_state(
    state: &AppState,
    conversation_id: &str,
    unread_count: usize,
    _is_system_notification_conversation: bool,
    current_viewer_id: Option<&str>,
) -> ConversationListItemState {
    let runtime_state = get_conversation_runtime_state(state, conversation_id)
        .unwrap_or(MainSessionState::Idle);
    let (open_state, opened_by, _open_label, open_viewer_id) =
        conversation_list_open_state(state, conversation_id);
    let mark = conversation_list_activity_mark(state, conversation_id);
    let activity = if runtime_state != MainSessionState::Idle {
        "busy".to_string()
    } else {
        mark.as_ref()
            .map(|item| item.activity.trim().to_string())
            .filter(|value| matches!(value.as_str(), "completed" | "failed"))
            .unwrap_or_else(|| "idle".to_string())
    };
    let disabled_reason = if runtime_state == MainSessionState::OrganizingContext {
        Some("organizing_context".to_string())
    } else {
        None
    };
    ConversationListItemState {
        activity,
        runtime_state,
        unread_count,
        open_state,
        open_viewer_id,
        current_viewer_id: current_viewer_id.map(ToOwned::to_owned),
        opened_by,
        disabled_reason,
        failed_message: mark.as_ref().and_then(|item| item.failed_message.clone()),
        completed_at: mark.and_then(|item| item.completed_at),
    }
}

fn build_unarchived_conversation_summary_from_meta_view(
    state: &AppState,
    main_conversation_id: &str,
    pinned_conversation_ids: &[String],
    conversation_meta: &ConversationMetaView,
    current_viewer_id: Option<&str>,
) -> UnarchivedConversationSummary {
    let conversation_id = conversation_meta.id.trim();
    let is_system_notification_conversation = conversation_id == main_conversation_id
        || conversation_meta.id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID
        || conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_SYSTEM_NOTIFICATION;
    let pin_index = pinned_conversation_ids
        .iter()
        .position(|item| item.trim() == conversation_id);
    let detached_window_label = if is_system_notification_conversation {
        None
    } else {
        detached_chat_window_for_conversation(conversation_id)
    };
    let unread_count = if conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT
    {
        0
    } else {
        conversation_meta.unread_count
    };
    let item_state = build_conversation_list_item_state(
        state,
        conversation_id,
        unread_count,
        is_system_notification_conversation,
        current_viewer_id,
    );
    let (workspace_label, workspace_root_path) =
        conversation_default_workspace_summary_from_meta_view(state, conversation_meta);
    let child_conversations = conversation_meta
        .child_conversation_ids
        .iter()
        .filter_map(|child_id| conversation_service_v2().get_conversation_meta(state, child_id).ok())
        .filter(|child| {
            child.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT
                && child.status.trim() != "archived"
        })
        .map(|child| ChildConversationSummary {
            conversation_id: child.id,
            title: child.title,
            status: child.status,
            conversation_kind: child.conversation_kind,
            parent_conversation_id: child.parent_conversation_id,
            updated_at: child.updated_at,
        })
        .collect::<Vec<_>>();
    UnarchivedConversationSummary {
        conversation_id: conversation_meta.id.clone(),
        title: conversation_meta.title.clone(),
        summary_title: conversation_meta.latest_summary_title.clone(),
        updated_at: conversation_meta.updated_at.clone(),
        last_message_at: conversation_meta.last_message_at.clone(),
        message_count: conversation_meta.message_count,
        body_message_count: conversation_meta.body_message_count,
        body_text_length: conversation_meta.body_text_length,
        has_assistant_reply: conversation_meta.has_assistant_reply,
        unread_count,
        agent_id: conversation_meta.agent_id.clone(),
        conversation_kind: conversation_meta.conversation_kind.clone(),
        child_conversation_ids: conversation_meta.child_conversation_ids.clone(),
        child_conversations,
        parent_conversation_id: conversation_meta
            .parent_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        fork_message_cursor: conversation_meta
            .fork_message_cursor
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        workspace_label,
        workspace_root_path,
        is_active: conversation_meta.status.trim() == "active",
        is_system_notification_conversation,
        is_pinned: is_system_notification_conversation || pin_index.is_some(),
        is_draft: conversation_meta.is_draft,
        pin_index,
        runtime_state: unarchived_conversation_runtime_state(state, &conversation_meta.id),
        current_todo: conversation_current_todo_text_from_items(&conversation_meta.current_todos),
        plan_mode_enabled: get_conversation_plan_mode_enabled(state, conversation_id).unwrap_or(false),
        auto_push_remote_contact_id: conversation_meta.auto_push_remote_contact_id.clone(),
        last_error: conversation_meta.last_error.clone(),
        detached_window_open: detached_window_label.is_some(),
        detached_window_label,
        state: item_state,
        preview_messages: conversation_meta
            .preview_messages
            .iter()
            .map(|message| ConversationPreviewMessage {
                message_id: message.message_id.clone(),
                role: message.role.clone(),
                speaker_agent_id: message.speaker_agent_id.clone(),
                created_at: message.created_at.clone(),
                text_preview: message.text_preview.clone(),
                has_image: message.has_image,
                has_pdf: message.has_pdf,
                has_audio: message.has_audio,
                has_attachment: message.has_attachment,
            })
            .collect(),
    }
}

fn conversation_body_text_length(conversation: &Conversation) -> usize {
    conversation
        .messages
        .iter()
        .filter(|message| {
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant"
            )
        })
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(text.trim()),
            _ => None,
        })
        .map(|text| text.chars().count())
        .sum()
}

fn conversation_body_message_count(conversation: &Conversation) -> usize {
    conversation
        .messages
        .iter()
        .filter(|message| {
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant"
            )
        })
        .count()
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
        if a.is_system_notification_conversation != b.is_system_notification_conversation {
            return b
                .is_system_notification_conversation
                .cmp(&a.is_system_notification_conversation);
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
            runtime_log_error(format!(
                "[会话] 读取运行态失败，任务=unarchived_runtime_state，conversation_id={}，error={}",
                conversation_id, err
            ));
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
async fn list_unarchived_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<UnarchivedConversationSummary>, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || list_unarchived_conversations_blocking(&app_state))
        .await
        .map_err(|err| format!("读取未归档会话列表任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListUnarchivedConversationsChangedSinceInput {
    #[serde(default)]
    since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListUnarchivedConversationsChangedSinceOutput {
    changed: Vec<UnarchivedConversationSummary>,
    #[serde(default)]
    deleted_ids: Vec<String>,
    server_time: String,
}

#[derive(Debug, Default)]
struct OverviewBroadcastWatermarkState {
    conversation_times: std::collections::HashMap<String, String>,
    removed_times: std::collections::HashMap<String, String>,
    known_ids: std::collections::HashSet<String>,
    known_ids_initialized: bool,
    last_server_time: String,
}

static OVERVIEW_BROADCAST_WATERMARK_STATE: OnceLock<Mutex<OverviewBroadcastWatermarkState>> =
    OnceLock::new();

fn overview_broadcast_watermark_state() -> &'static Mutex<OverviewBroadcastWatermarkState> {
    OVERVIEW_BROADCAST_WATERMARK_STATE
        .get_or_init(|| Mutex::new(OverviewBroadcastWatermarkState::default()))
}

fn overview_next_server_time_locked(state: &mut OverviewBroadcastWatermarkState) -> String {
    let now = now_iso();
    let next = if state.last_server_time.trim().is_empty() || now > state.last_server_time {
        now
    } else {
        parse_iso(&state.last_server_time)
            .and_then(|value| value.checked_add(time::Duration::seconds(1)))
            .and_then(|value| value.format(&Rfc3339).ok())
            .unwrap_or(now)
    };
    state.last_server_time = next.clone();
    next
}

fn overview_reserve_server_time() -> String {
    match overview_broadcast_watermark_state().lock() {
        Ok(mut guard) => overview_next_server_time_locked(&mut guard),
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览水位] 失败，任务=生成服务端水位，error={:?}",
                err
            ));
            now_iso()
        }
    }
}

fn overview_remember_full_list_at(summaries: &[UnarchivedConversationSummary], server_time: &str) {
    let ids = summaries
        .iter()
        .map(|item| item.conversation_id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    match overview_broadcast_watermark_state().lock() {
        Ok(mut guard) => {
            guard.known_ids = ids;
            guard.known_ids_initialized = true;
            guard
                .conversation_times
                .retain(|_, changed_at| changed_at.as_str() > server_time);
            guard
                .removed_times
                .retain(|_, removed_at| removed_at.as_str() > server_time);
        }
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览水位] 失败，任务=记录全量基线，error={:?}",
                err
            ));
        }
    }
}

fn overview_register_item_watermark(conversation_id: &str) -> String {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return overview_reserve_server_time();
    }
    match overview_broadcast_watermark_state().lock() {
        Ok(mut guard) => {
            let server_time = overview_next_server_time_locked(&mut guard);
            guard
                .conversation_times
                .insert(cid.to_string(), server_time.clone());
            guard.removed_times.remove(cid);
            guard.known_ids.insert(cid.to_string());
            guard.known_ids_initialized = true;
            server_time
        }
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览水位] 失败，任务=记录单项广播，conversation_id={}，error={:?}",
                cid, err
            ));
            now_iso()
        }
    }
}

fn overview_register_item_broadcast(
    state: &AppState,
    payload: &UnarchivedConversationOverviewItemUpdatedPayload,
) {
    let conversation_id = payload.conversation.conversation_id.trim();
    let server_time = overview_register_item_watermark(conversation_id);
    let emitted_payload = UnarchivedConversationOverviewItemUpdatedPayload {
        conversation: payload.conversation.clone(),
        server_time,
    };
    ide_chat_broadcast_notification(
        "conversation.overviewItemUpdated",
        serde_json::json!(&emitted_payload),
    );
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览] 失败，任务=推送单会话概览，阶段=获取app_handle，conversation_id={}，error={:?}",
                conversation_id, err
            ));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!(
            "[会话概览] 跳过，任务=推送单会话概览，conversation_id={}，原因=app_handle_missing",
            conversation_id
        ));
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-overview-item-updated", &emitted_payload) {
        runtime_log_error(format!(
            "[会话概览] 失败，任务=推送单会话概览，conversation_id={}，error={}",
            conversation_id, err
        ));
    }
}

fn overview_register_missing_item(conversation_id: &str) {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return;
    }
    match overview_broadcast_watermark_state().lock() {
        Ok(mut guard) => {
            if !guard.known_ids.contains(cid) && !guard.conversation_times.contains_key(cid) {
                return;
            }
            let server_time = overview_next_server_time_locked(&mut guard);
            guard.conversation_times.remove(cid);
            guard.removed_times.insert(cid.to_string(), server_time);
            guard.known_ids.remove(cid);
            guard.known_ids_initialized = true;
        }
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览水位] 失败，任务=记录单项移除，conversation_id={}，error={:?}",
                cid, err
            ));
        }
    }
}

fn overview_register_full_broadcast(
    summaries: &[UnarchivedConversationSummary],
) -> String {
    let current_ids = summaries
        .iter()
        .map(|item| item.conversation_id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    match overview_broadcast_watermark_state().lock() {
        Ok(mut guard) => {
            let server_time = overview_next_server_time_locked(&mut guard);
            let removed_ids = if guard.known_ids_initialized {
                guard
                    .known_ids
                    .difference(&current_ids)
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            for id in &current_ids {
                guard
                    .conversation_times
                    .insert(id.clone(), server_time.clone());
                guard.removed_times.remove(id);
            }
            for id in removed_ids {
                guard.conversation_times.remove(&id);
                guard.removed_times.insert(id, server_time.clone());
            }
            guard.known_ids = current_ids;
            guard.known_ids_initialized = true;
            server_time
        }
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览水位] 失败，任务=记录全量广播，error={:?}",
                err
            ));
            now_iso()
        }
    }
}

fn overview_watermark_changes_since(
    since: &str,
) -> (Vec<String>, Vec<String>, String) {
    match overview_broadcast_watermark_state().lock() {
        Ok(guard) => {
            let changed_ids = guard
                .conversation_times
                .iter()
                .filter(|(_, changed_at)| changed_at.trim() > since)
                .map(|(conversation_id, _)| conversation_id.clone())
                .collect::<Vec<_>>();
            let deleted_ids = guard
                .removed_times
                .iter()
                .filter(|(_, removed_at)| removed_at.trim() > since)
                .map(|(conversation_id, _)| conversation_id.clone())
                .collect::<Vec<_>>();
            let server_time = guard.last_server_time.trim().to_string();
            (changed_ids, deleted_ids, server_time)
        }
        Err(err) => {
            runtime_log_error(format!(
                "[会话概览水位] 失败，任务=读取差量水位，since={}，error={:?}",
                since, err
            ));
            (Vec::new(), Vec::new(), now_iso())
        }
    }
}

fn overview_updated_payload_with_server_time(
    payload: &UnarchivedConversationOverviewUpdatedPayload,
    server_time: &str,
) -> serde_json::Value {
    let mut value = serde_json::json!(payload);
    if let serde_json::Value::Object(ref mut object) = value {
        object.insert(
            "serverTime".to_string(),
            serde_json::Value::String(server_time.to_string()),
        );
    }
    value
}

#[tauri::command]
async fn list_unarchived_conversations_changed_since(
    input: ListUnarchivedConversationsChangedSinceInput,
    state: State<'_, AppState>,
) -> Result<ListUnarchivedConversationsChangedSinceOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        list_unarchived_conversations_changed_since_blocking(&app_state, &input)
    })
    .await
    .map_err(|err| format!("读取未归档会话列表差量任务异常：{err}"))?
}

fn list_unarchived_conversations_changed_since_blocking(
    state: &AppState,
    input: &ListUnarchivedConversationsChangedSinceInput,
) -> Result<ListUnarchivedConversationsChangedSinceOutput, String> {
    let since = input
        .since
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if since.is_empty() {
        let server_time = overview_reserve_server_time();
        let changed = list_unarchived_conversations_blocking(state)?;
        overview_remember_full_list_at(&changed, &server_time);
        return Ok(ListUnarchivedConversationsChangedSinceOutput {
            changed,
            deleted_ids: Vec::new(),
            server_time,
        });
    }

    let (changed_ids, mut deleted_ids, server_time) = overview_watermark_changes_since(since);
    // 差量查询不再全量枚举全部会话：按 changed_ids 直读单条 summary（O(k)，k=实际变更数）。
    // 直读不到说明该会话已消失（可能未走 register_missing_item 的路径），追加进 deleted_ids 兜底。
    let mut changed = Vec::new();
    for conversation_id in changed_ids {
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            continue;
        }
        match conversation_service_v2().read_unarchived_conversation_summary(state, conversation_id) {
            Ok(Some(summary)) => changed.push(summary),
            Ok(None) => {
                if !deleted_ids.iter().any(|deleted_id| deleted_id == conversation_id) {
                    deleted_ids.push(conversation_id.to_string());
                }
            }
            Err(err) => {
                // 读取失败不等于删除：保留 watermark 中的该 id，下次差量查询自然重试；
                // 误判为删除会污染前端列表，把仍存在的会话从客户端移除。
                runtime_log_warn(format!(
                    "[会话概览] 跳过，任务=按 id 直读差量概览，conversation_id={}，error={}",
                    conversation_id, err
                ));
            }
        }
    }
    deleted_ids.sort();
    deleted_ids.dedup();
    Ok(ListUnarchivedConversationsChangedSinceOutput {
        changed,
        deleted_ids,
        server_time,
    })
}

fn list_unarchived_conversations_blocking(
    state: &AppState,
) -> Result<Vec<UnarchivedConversationSummary>, String> {
    let summaries = conversation_service_v2()
        .list_unarchived_conversation_summaries(state)?
        .summaries;
    if !summaries.is_empty() {
        return Ok(summaries);
    }

    runtime_log_info("[会话] 开始，任务=确保默认未归档会话，触发条件=未归档会话列表为空".to_string());
    let create_input = CreateUnarchivedConversationInput {
        api_config_id: None,
        agent_id: Some(state_service_get_assistant_agent_id(state)?),
        title: None,
        copy_source_conversation_id: None,
        shell_workspaces: None,
        shell_work_mode: None,        shell_work_branch: None,
        shell_autonomous_mode: None,
        is_draft: None,
    };
    let result = conversation_service_v2().create_conversation(state, &create_input)?;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &result.conversation_id,
    );
    runtime_log_debug(format!(
        "[会话] 完成，任务=确保默认未归档会话，conversation_id={}，overview_count={}",
        result.conversation_id,
        result.overview_payload.unarchived_conversations.len()
    ));
    Ok(result.overview_payload.unarchived_conversations)
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
    #[serde(default)]
    resume_projection: bool,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preferred_api_config_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_goal: Option<ConversationGoalState>,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForegroundConversationLightSnapshotOutput {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_message_id: Option<String>,
    has_more_history: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_state: Option<MainSessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preferred_api_config_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_goal: Option<ConversationGoalState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conversation: Option<UnarchivedConversationSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stream_cache: Option<ConversationStreamRuntimeCacheSnapshot>,
    #[serde(default)]
    should_bind_stream: bool,
    #[serde(default)]
    resume_projection_authoritative: bool,
}

#[derive(Debug, Clone)]
struct ForegroundConversationSnapshotCore {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    last_message_id: Option<String>,
    has_more_history: bool,
    runtime_state: Option<MainSessionState>,
    current_todo: Option<String>,
    current_todos: Vec<ConversationTodoItem>,
    preferred_api_config_id: Option<String>,
    active_goal: Option<ConversationGoalState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForegroundConversationFreshnessInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForegroundConversationFreshnessOutput {
    conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

const DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT: usize = 4;

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
struct SetConversationPreferredModelInput {
    conversation_id: String,
    #[serde(default)]
    preferred_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConversationPreferredModelOutput {
    conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConversationAutoPushRemoteContactInput {
    conversation_id: String,
    #[serde(default)]
    remote_contact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConversationAutoPushRemoteContactOutput {
    conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_contact_id: Option<String>,
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
struct UnarchivedConversationOverviewItemUpdatedPayload {
    conversation: UnarchivedConversationSummary,
    server_time: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationRuntimeStateUpdatedPayload {
    conversation_id: String,
    runtime_state: MainSessionState,
}

fn emit_unarchived_conversation_overview_updated_payload(
    state: &AppState,
    payload: &UnarchivedConversationOverviewUpdatedPayload,
) {
    let server_time = overview_register_full_broadcast(&payload.unarchived_conversations);
    let event_payload = overview_updated_payload_with_server_time(payload, &server_time);
    ide_chat_broadcast_notification("conversation.overviewUpdated", event_payload.clone());
    let started_at = std::time::Instant::now();
    runtime_log_debug(format!(
        "[会话概览] 开始，任务=推送未归档会话概览，preferred_conversation_id={}，conversation_count={}，server_time={}",
        payload.preferred_conversation_id.as_deref().unwrap_or(""),
        payload.unarchived_conversations.len(),
        server_time
    ));
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!("[会话概览] 失败，任务=推送未归档会话概览，阶段=获取app_handle，error={:?}", err));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn("[会话概览] 跳过，任务=推送未归档会话概览，原因=app_handle_missing".to_string());
        return;
    };
    if let Err(err) = app_handle.emit(CHAT_CONVERSATION_OVERVIEW_UPDATED_EVENT, &event_payload) {
        runtime_log_error(format!(
            "[会话概览] 失败，任务=推送未归档会话概览，event={}，error={}，duration_ms={}",
            CHAT_CONVERSATION_OVERVIEW_UPDATED_EVENT,
            err,
            started_at.elapsed().as_millis()
        ));
        return;
    }
    runtime_log_debug(format!(
        "[会话概览] 完成，任务=推送未归档会话概览，event={}，preferred_conversation_id={}，conversation_count={}，server_time={}，duration_ms={}",
        CHAT_CONVERSATION_OVERVIEW_UPDATED_EVENT,
        payload.preferred_conversation_id.as_deref().unwrap_or(""),
        payload.unarchived_conversations.len(),
        server_time,
        started_at.elapsed().as_millis()
    ));
}

fn emit_unarchived_conversation_overview_item_updated_payload(
    state: &AppState,
    payload: &UnarchivedConversationOverviewItemUpdatedPayload,
) {
    overview_register_item_broadcast(state, payload);
}

fn emit_unarchived_conversation_overview_item_updated_from_state(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let Some(conversation) = conversation_service_v2()
        .read_unarchived_conversation_summary(state, conversation_id)?
    else {
        overview_register_missing_item(conversation_id);
        return Ok(false);
    };
    // 追问会话（side_chat）只出现在侧边追问视图，不参与前台会话列表：
    // 不广播列表项更新事件，避免其被前端 applyConversationOverviewItemUpdated
    // 无条件 push 进「最近会话」列表。追问视图运行时依赖的是
    // runtimeStateUpdated / messageAppended 等事件，与本事件无关。
    if conversation.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT {
        return Ok(true);
    }
    emit_unarchived_conversation_overview_item_updated_payload(
        state,
        &UnarchivedConversationOverviewItemUpdatedPayload {
            conversation,
            server_time: String::new(),
        },
    );
    Ok(true)
}

fn emit_conversation_todos_updated_payload(
    state: &AppState,
    payload: &ConversationTodosUpdatedPayload,
) {
    ide_chat_broadcast_notification("conversation.todosUpdated", serde_json::json!(payload));
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!("[Todo] 获取 app_handle 失败：锁已损坏，error={:?}", err));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!("[Todo] 推送跳过：无法获取 app_handle"));
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-todos-updated", payload) {
        runtime_log_error(format!("[Todo] 推送失败：错误={}", err));
    }
}

fn emit_conversation_pin_updated_payload(
    state: &AppState,
    payload: &ConversationPinUpdatedPayload,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!("[会话置顶] 获取 app_handle 失败：锁已损坏，error={:?}", err));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!("[会话置顶] 推送跳过：无法获取 app_handle"));
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-pin-updated", payload) {
        runtime_log_error(format!("[会话置顶] 推送失败：错误={}", err));
    }
}

fn emit_conversation_runtime_state_updated_payload(
    state: &AppState,
    payload: &ConversationRuntimeStateUpdatedPayload,
) {
    ide_chat_broadcast_notification("conversation.runtimeStateUpdated", serde_json::json!(payload));
    let started_at = std::time::Instant::now();
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!(
                "[会话运行态] 失败，任务=推送会话运行态，阶段=获取app_handle，conversation_id={}，error={:?}",
                payload.conversation_id, err
            ));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!(
            "[会话运行态] 跳过，任务=推送会话运行态，conversation_id={}，原因=app_handle_missing",
            payload.conversation_id
        ));
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-runtime-state-updated", payload) {
        runtime_log_error(format!(
            "[会话运行态] 失败，任务=推送会话运行态，conversation_id={}，state={:?}，error={}，duration_ms={}",
            payload.conversation_id,
            payload.runtime_state,
            err,
            started_at.elapsed().as_millis()
        ));
        return;
    }
    runtime_log_debug(format!(
        "[会话运行态] 完成，任务=推送会话运行态，conversation_id={}，state={:?}，duration_ms={}",
        payload.conversation_id,
        payload.runtime_state,
        started_at.elapsed().as_millis()
    ));
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
    let Some(todo_update) = conversation_service_v2().update_conversation_todos(
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
    emit_conversation_todos_updated_payload(state, &todo_payload);
    emit_unarchived_conversation_overview_item_updated_from_state(state, cid)?;
    Ok(())
}

fn emit_unarchived_conversation_overview_updated_from_state(state: &AppState) -> Result<(), String> {
    let total_started_at = std::time::Instant::now();
    let payload_started_at = std::time::Instant::now();
    let payload = conversation_service_v2().refresh_unarchived_conversation_overview(state)?;
    let payload_elapsed_ms = payload_started_at.elapsed().as_millis();
    let emit_started_at = std::time::Instant::now();
    emit_unarchived_conversation_overview_updated_payload(state, &payload);
    let emit_elapsed_ms = emit_started_at.elapsed().as_millis();
    runtime_log_debug(format!(
        "[会话概览] 状态刷新耗时：总计={}ms，构建概览={}ms，事件推送={}ms",
        total_started_at.elapsed().as_millis(),
        payload_elapsed_ms,
        emit_elapsed_ms
    ));
    Ok(())
}

#[tauri::command]
fn set_active_unarchived_conversation(
    input: SetActiveUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<SetActiveUnarchivedConversationOutput, String> {
    let conversation_id =
        conversation_service_v2().set_active_conversation(state.inner(), &input)?;
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
            summary_title: None,
            updated_at: updated_at.to_string(),
            last_message_at: Some(updated_at.to_string()),
            message_count: 1,
            body_message_count: 1,
            body_text_length: 0,
            has_assistant_reply: true,
            unread_count: 0,
            auto_push_remote_contact_id: None,
            last_error: None,
            agent_id: "agent-a".to_string(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            child_conversation_ids: Vec::new(),
            child_conversations: Vec::new(),
            parent_conversation_id: parent_conversation_id.map(ToOwned::to_owned),
            fork_message_cursor: None,
            workspace_label: "默认会话目录".to_string(),
            workspace_root_path: None,
            is_active: false,
            is_system_notification_conversation: false,
            is_pinned: false,
            pin_index: None,
            runtime_state: None,
            current_todo: None,
            plan_mode_enabled: false,
            detached_window_open: false,
            detached_window_label: None,
            state: ConversationListItemState {
                activity: "idle".to_string(),
                runtime_state: MainSessionState::Idle,
                unread_count: 0,
                open_state: "closed".to_string(),
                open_viewer_id: None,
                current_viewer_id: Some(DESKTOP_CHAT_VIEWER_ID.to_string()),
                opened_by: None,
                disabled_reason: None,
                failed_message: None,
                completed_at: None,
            },
            preview_messages: Vec::new(),
            is_draft: false,
        }
    }

    #[test]
    fn sort_unarchived_conversation_summaries_should_group_main_pinned_and_recent() {
        let mut main = test_summary("main", "2026-04-18T10:00:00Z", None);
        main.is_system_notification_conversation = true;
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

    #[test]
    fn build_conversation_preview_text_should_truncate_to_20_chars() {
        let message = ChatMessage {
            id: "message-1".to_string(),
            role: "assistant".to_string(),
            created_at: "2026-06-15T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "123456789012345678901234567890".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };

        assert_eq!(
            build_conversation_preview_text(&message),
            "12345678901234567890"
        );
    }
}
