#[tauri::command]
fn get_chat_snapshot(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;

    let mut data = state_read_app_data_cached(&state)?;
    let data_before = data.clone();
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

    let idx = if let Some(existing_idx) =
        latest_active_conversation_index(&data, "", &effective_agent_id)
    {
        existing_idx
    } else {
        let api_config = resolve_selected_api_config(&app_config, None)
            .ok_or_else(|| "No API config available".to_string())?;
        ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id)
    };
    let conversation = data
        .conversations
        .get(idx)
        .ok_or_else(|| "Selected conversation index out of bounds.".to_string())?;

    let mut latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();
    let mut latest_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && !is_tool_review_report_message(m))
        .cloned();

    persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    drop(guard);

    if let Some(message) = latest_user.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }
    if let Some(message) = latest_assistant.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }

    Ok(ChatSnapshot {
        conversation_id: conversation.id.clone(),
        latest_user,
        latest_assistant,
        active_message_count: conversation
            .messages
            .iter()
            .filter(|message| !is_tool_review_report_message(message))
            .count(),
    })
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
        .find(|m| m.role == "user")
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
    if compact.is_empty() {
        "无内容".to_string()
    } else {
        compact.chars().take(12).collect::<String>()
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

fn workspace_label_for_unarchived_conversation(
    state: &AppState,
    conversation: &Conversation,
) -> String {
    if let Some(path) = terminal_workspace_path_from_conversation(state, conversation) {
        return resolve_workspace_display_name_for_conversation(state, Some(conversation), &path);
    }
    if let Ok(workspace) = terminal_default_workspace_for_conversation_resolved(state, Some(conversation)) {
        return workspace.name;
    }
    "默认工作空间".to_string()
}

fn build_unarchived_conversation_summary(
    state: &AppState,
    app_config: &AppConfig,
    main_conversation_id: &str,
    conversation: &Conversation,
) -> UnarchivedConversationSummary {
    let last_message_at = conversation.messages.last().map(|m| m.created_at.clone());
    let department_id = resolved_foreground_department_id_for_conversation(
        app_config,
        conversation,
        conversation.id.trim() == main_conversation_id,
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
        workspace_label: workspace_label_for_unarchived_conversation(state, conversation),
        is_active: conversation.status.trim() == "active",
        is_main_conversation: conversation.id.trim() == main_conversation_id,
        runtime_state: unarchived_conversation_runtime_state(state, &conversation.id),
        current_todo: conversation_current_todo_text(conversation),
        plan_mode_enabled: get_conversation_plan_mode_enabled(state, &conversation.id)
            .unwrap_or(conversation.plan_mode_enabled),
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

fn sort_unarchived_conversation_summaries_with_parent_first(
    summaries: Vec<UnarchivedConversationSummary>,
) -> Vec<UnarchivedConversationSummary> {
    let mut ordered = summaries;
    ordered.sort_by(|a, b| {
        if a.is_main_conversation != b.is_main_conversation {
            return b.is_main_conversation.cmp(&a.is_main_conversation);
        }
        let (a_primary, a_secondary) = unarchived_conversation_sort_key(a);
        let (b_primary, b_secondary) = unarchived_conversation_sort_key(b);
        b_primary
            .cmp(a_primary)
            .then_with(|| b_secondary.cmp(a_secondary))
            .then_with(|| a.conversation_id.cmp(&b.conversation_id))
    });

    let ordered_ids = ordered
        .iter()
        .map(|item| item.conversation_id.clone())
        .collect::<Vec<_>>();
    let children_by_parent = ordered.iter().enumerate().fold(
        std::collections::HashMap::<String, Vec<String>>::new(),
        |mut acc, (_index, item)| {
            if let Some(parent_id) = item
                .parent_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                acc.entry(parent_id.to_string())
                    .or_default()
                    .push(item.conversation_id.clone());
            }
            acc
        },
    );
    let mut by_id = ordered
        .into_iter()
        .map(|item| (item.conversation_id.clone(), item))
        .collect::<std::collections::HashMap<_, _>>();
    let mut visited = std::collections::HashSet::<String>::new();
    let mut output = Vec::<UnarchivedConversationSummary>::new();

    fn append_with_children(
        conversation_id: &str,
        by_id: &mut std::collections::HashMap<String, UnarchivedConversationSummary>,
        children_by_parent: &std::collections::HashMap<String, Vec<String>>,
        visited: &mut std::collections::HashSet<String>,
        output: &mut Vec<UnarchivedConversationSummary>,
    ) {
        let normalized_id = conversation_id.trim();
        if normalized_id.is_empty() || !visited.insert(normalized_id.to_string()) {
            return;
        }
        let Some(item) = by_id.remove(normalized_id) else {
            return;
        };
        output.push(item);
        if let Some(child_ids) = children_by_parent.get(normalized_id) {
            for child_id in child_ids {
                append_with_children(child_id, by_id, children_by_parent, visited, output);
            }
        }
    }

    for conversation_id in &ordered_ids {
        let Some(parent_id) = by_id
            .get(conversation_id)
            .and_then(|item| item.parent_conversation_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            append_with_children(
                conversation_id,
                &mut by_id,
                &children_by_parent,
                &mut visited,
                &mut output,
            );
            continue;
        };
        if !by_id.contains_key(parent_id) {
            append_with_children(
                conversation_id,
                &mut by_id,
                &children_by_parent,
                &mut visited,
                &mut output,
            );
        }
    }
    for conversation_id in ordered_ids {
        append_with_children(
            &conversation_id,
            &mut by_id,
            &children_by_parent,
            &mut visited,
            &mut output,
        );
    }
    output
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
                conversation,
            )
        })
        .collect::<Vec<_>>();
    sort_unarchived_conversation_summaries_with_parent_first(summaries)
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
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let data_before = data.clone();
    let app_config = state_read_config_cached(&state)?;
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let summaries = collect_unarchived_conversation_summaries(state.inner(), &app_config, &data);
    if normalized_changed || department_changed {
        persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    }
    drop(guard);
    Ok(summaries)
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
struct SwitchActiveConversationSnapshotOutput {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    has_more_history: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_todo: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    current_todos: Vec<ConversationTodoItem>,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

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

fn build_unarchived_conversation_overview_payload(
    state: &AppState,
    app_config: &AppConfig,
    data: &AppData,
) -> UnarchivedConversationOverviewUpdatedPayload {
    let unarchived_conversations = collect_unarchived_conversation_summaries(state, app_config, data);
    let preferred_conversation_id = unarchived_conversations
        .first()
        .map(|item| item.conversation_id.clone());
    UnarchivedConversationOverviewUpdatedPayload {
        unarchived_conversations,
        preferred_conversation_id,
    }
}

fn persist_conversation_set_delta(
    state: &AppState,
    before: &[Conversation],
    after: &[Conversation],
) -> Result<(), String> {
    let total_started_at = std::time::Instant::now();
    let before_map_started_at = std::time::Instant::now();
    let before_map = before
        .iter()
        .map(|conversation| (conversation.id.clone(), conversation))
        .collect::<std::collections::HashMap<_, _>>();
    let before_map_elapsed_ms = before_map_started_at.elapsed().as_millis();
    let after_ids_started_at = std::time::Instant::now();
    let after_ids = after
        .iter()
        .map(|conversation| conversation.id.clone())
        .collect::<std::collections::HashSet<_>>();
    let after_ids_elapsed_ms = after_ids_started_at.elapsed().as_millis();

    let compare_and_write_started_at = std::time::Instant::now();
    let mut changed_count = 0usize;
    for conversation in after {
        let changed = before_map
            .get(&conversation.id)
            .map(|previous| serde_json::to_vec(previous).ok() != serde_json::to_vec(conversation).ok())
            .unwrap_or(true);
        if changed {
            changed_count += 1;
            state_write_conversation_cached(state, conversation)?;
        }
    }
    let compare_and_write_elapsed_ms = compare_and_write_started_at.elapsed().as_millis();

    let delete_started_at = std::time::Instant::now();
    let mut deleted_count = 0usize;
    for conversation in before {
        if !after_ids.contains(&conversation.id) {
            deleted_count += 1;
            state_delete_conversation_cached(state, &conversation.id)?;
        }
    }
    let delete_elapsed_ms = delete_started_at.elapsed().as_millis();
    eprintln!(
        "[会话持久化] 会话集合增量耗时：总计={}ms，构建旧映射={}ms，构建新ID集合={}ms，比较并写会话={}ms，删除会话={}ms，before_count={}，after_count={}，changed_count={}，deleted_count={}",
        total_started_at.elapsed().as_millis(),
        before_map_elapsed_ms,
        after_ids_elapsed_ms,
        compare_and_write_elapsed_ms,
        delete_elapsed_ms,
        before.len(),
        after.len(),
        changed_count,
        deleted_count
    );

    Ok(())
}

fn persist_app_data_conversation_runtime_delta(
    state: &AppState,
    before: &AppData,
    after: &AppData,
) -> Result<(), String> {
    let total_started_at = std::time::Instant::now();
    let conversations_compare_started_at = std::time::Instant::now();
    let conversations_changed =
        serde_json::to_vec(&before.conversations).ok() != serde_json::to_vec(&after.conversations).ok();
    let conversations_compare_elapsed_ms = conversations_compare_started_at.elapsed().as_millis();
    let runtime_before_started_at = std::time::Instant::now();
    let runtime_before = build_runtime_state_file(before);
    let runtime_before_elapsed_ms = runtime_before_started_at.elapsed().as_millis();
    let runtime_after_started_at = std::time::Instant::now();
    let runtime_after = build_runtime_state_file(after);
    let runtime_after_elapsed_ms = runtime_after_started_at.elapsed().as_millis();
    let runtime_compare_started_at = std::time::Instant::now();
    let runtime_changed =
        serde_json::to_vec(&runtime_before).ok() != serde_json::to_vec(&runtime_after).ok();
    let runtime_compare_elapsed_ms = runtime_compare_started_at.elapsed().as_millis();

    let conversation_delta_started_at = std::time::Instant::now();
    if conversations_changed {
        persist_conversation_set_delta(state, &before.conversations, &after.conversations)?;
        let chat_index = build_chat_index_file(&after.conversations);
        state_write_chat_index_cached(state, &chat_index)?;
    }
    let conversation_delta_elapsed_ms = conversation_delta_started_at.elapsed().as_millis();
    let runtime_write_started_at = std::time::Instant::now();
    if runtime_changed {
        state_write_runtime_state_cached(state, &runtime_after)?;
    }
    let runtime_write_elapsed_ms = runtime_write_started_at.elapsed().as_millis();
    eprintln!(
        "[会话持久化] 运行态增量耗时：总计={}ms，会话列表比较={}ms，构建运行态(before)={}ms，构建运行态(after)={}ms，运行态比较={}ms，会话增量落盘+索引={}ms，运行态落盘={}ms，conversation_count_before={}，conversation_count_after={}，conversations_changed={}，runtime_changed={}",
        total_started_at.elapsed().as_millis(),
        conversations_compare_elapsed_ms,
        runtime_before_elapsed_ms,
        runtime_after_elapsed_ms,
        runtime_compare_elapsed_ms,
        conversation_delta_elapsed_ms,
        runtime_write_elapsed_ms,
        before.conversations.len(),
        after.conversations.len(),
        conversations_changed,
        runtime_changed
    );

    Ok(())
}

fn persist_single_conversation_runtime_fast(
    state: &AppState,
    data: &AppData,
    conversation_id: &str,
) -> Result<(), String> {
    let total_started_at = std::time::Instant::now();
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return Ok(());
    }
    let conversation = data
        .conversations
        .iter()
        .find(|item| item.id == cid)
        .ok_or_else(|| format!("会话不存在，无法快速持久化: {cid}"))?;

    let conversation_write_started_at = std::time::Instant::now();
    state_write_conversation_cached(state, conversation)?;
    let conversation_write_elapsed_ms = conversation_write_started_at.elapsed().as_millis();

    let chat_index_started_at = std::time::Instant::now();
    let chat_index = build_chat_index_file(&data.conversations);
    state_write_chat_index_cached(state, &chat_index)?;
    let chat_index_elapsed_ms = chat_index_started_at.elapsed().as_millis();

    let runtime_state_started_at = std::time::Instant::now();
    let runtime_state = build_runtime_state_file(data);
    state_write_runtime_state_cached(state, &runtime_state)?;
    let runtime_state_elapsed_ms = runtime_state_started_at.elapsed().as_millis();

    eprintln!(
        "[会话持久化] 单会话快速写入耗时：总计={}ms，会话写入={}ms，聊天索引写入={}ms，运行态写入={}ms，conversation_id={}，conversation_count={}",
        total_started_at.elapsed().as_millis(),
        conversation_write_elapsed_ms,
        chat_index_elapsed_ms,
        runtime_state_elapsed_ms,
        cid,
        data.conversations.len()
    );

    Ok(())
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
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let app_config = state_read_config_cached(state)?;
    let mut conversation = match state_read_conversation_cached(state, cid) {
        Ok(conversation) if conversation.summary.trim().is_empty() => conversation,
        Ok(_) => {
            drop(guard);
            return Ok(());
        }
        Err(err) => {
            runtime_log_debug(format!(
                "[Todo] 读取会话失败，函数=update_conversation_todos_and_emit，conversation_id={}，error={}",
                cid, err
            ));
            drop(guard);
            return Ok(());
        }
    };
    if conversation.current_todos == stored_todos {
        drop(guard);
        return Ok(());
    }
    conversation.current_todos = stored_todos.clone();
    let current_todo = conversation_current_todo_text(&conversation);
    state_write_conversation_cached(state, &conversation)?;
    let data = state_read_app_data_cached(state)?;
    let todo_payload = ConversationTodosUpdatedPayload {
        conversation_id: cid.to_string(),
        current_todo,
        current_todos: stored_todos,
    };
    let overview_payload = build_unarchived_conversation_overview_payload(state, &app_config, &data);
    drop(guard);
    emit_conversation_todos_updated_payload(state, &todo_payload);
    emit_unarchived_conversation_overview_updated_payload(state, &overview_payload);
    Ok(())
}

fn emit_unarchived_conversation_overview_updated_from_state(state: &AppState) -> Result<(), String> {
    let total_started_at = std::time::Instant::now();
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let app_data_started_at = std::time::Instant::now();
    let mut data = state_read_app_data_cached(state)?;
    let app_data_elapsed_ms = app_data_started_at.elapsed().as_millis();
    let clone_started_at = std::time::Instant::now();
    let data_before = data.clone();
    let clone_elapsed_ms = clone_started_at.elapsed().as_millis();
    let config_started_at = std::time::Instant::now();
    let app_config = state_read_config_cached(state)?;
    let config_elapsed_ms = config_started_at.elapsed().as_millis();
    let normalize_main_started_at = std::time::Instant::now();
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let normalize_main_elapsed_ms = normalize_main_started_at.elapsed().as_millis();
    let normalize_department_started_at = std::time::Instant::now();
    let department_changed = normalize_foreground_conversation_departments(&app_config, &mut data);
    let normalize_department_elapsed_ms = normalize_department_started_at.elapsed().as_millis();
    let persist_started_at = std::time::Instant::now();
    if normalized_changed || department_changed {
        persist_app_data_conversation_runtime_delta(state, &data_before, &data)?;
    }
    let persist_elapsed_ms = persist_started_at.elapsed().as_millis();
    let payload_started_at = std::time::Instant::now();
    let payload = build_unarchived_conversation_overview_payload(state, &app_config, &data);
    let payload_elapsed_ms = payload_started_at.elapsed().as_millis();
    let lock_held_elapsed_ms = total_started_at.elapsed().as_millis();
    drop(guard);
    let emit_started_at = std::time::Instant::now();
    emit_unarchived_conversation_overview_updated_payload(state, &payload);
    let emit_elapsed_ms = emit_started_at.elapsed().as_millis();
    eprintln!(
        "[会话概览] 状态刷新耗时：总计={}ms，锁内={}ms，读取应用数据={}ms，克隆数据={}ms，读取配置={}ms，主会话归一化={}ms，部门归一化={}ms，持久化={}ms，构建概览={}ms，事件推送={}ms，normalized_changed={}，department_changed={}",
        total_started_at.elapsed().as_millis(),
        lock_held_elapsed_ms,
        app_data_elapsed_ms,
        clone_elapsed_ms,
        config_elapsed_ms,
        normalize_main_elapsed_ms,
        normalize_department_elapsed_ms,
        persist_elapsed_ms,
        payload_elapsed_ms,
        emit_elapsed_ms,
        normalized_changed,
        department_changed
    );
    Ok(())
}

#[tauri::command]
fn set_active_unarchived_conversation(
    input: SetActiveUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<SetActiveUnarchivedConversationOutput, String> {
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

    let mut target_idx = requested_conversation_id.and_then(|conversation_id| {
        data.conversations.iter().position(|item| {
            item.id == conversation_id
                && item.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(item)
        })
    });
    if target_idx.is_none() {
        target_idx = latest_active_conversation_index(&data, "", "")
            .or_else(|| {
                data.conversations
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        item.summary.trim().is_empty()
                            && conversation_visible_in_foreground_lists(item)
                    })
                    .max_by(|(idx_a, a), (idx_b, b)| {
                        let a_updated = a.updated_at.trim();
                        let b_updated = b.updated_at.trim();
                        let a_created = a.created_at.trim();
                        let b_created = b.created_at.trim();
                        a_updated
                            .cmp(b_updated)
                            .then_with(|| a_created.cmp(b_created))
                            .then_with(|| idx_a.cmp(idx_b))
                    })
                    .map(|(idx, _)| idx)
            });
    }
    let target_idx = match target_idx {
        Some(value) => value,
        None => {
            let api_config = resolve_selected_api_config(&app_config, None)
                .ok_or_else(|| "No API config available".to_string())?;
            // resolve_selected_api_config 保证有可选 API 配置后，ensure_active_conversation_index 会创建或复用可见会话并返回有效索引
            ensure_active_conversation_index(&mut data, &api_config.id, "")
        }
    };
    let conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    ensure_unarchived_conversation_not_organizing(state.inner(), &conversation_id)?;

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
    if changed {
        persist_app_data_conversation_runtime_delta(&state, &data_before, &data)?;
    }
    drop(guard);
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
            runtime_state: None,
            current_todo: None,
            plan_mode_enabled: false,
            preview_messages: Vec::new(),
        }
    }

    #[test]
    fn sort_unarchived_conversation_summaries_should_keep_parent_before_child() {
        let ordered = sort_unarchived_conversation_summaries_with_parent_first(vec![
            test_summary("child", "2026-04-18T10:02:00Z", Some("parent")),
            test_summary("independent", "2026-04-18T10:03:00Z", None),
            test_summary("parent", "2026-04-18T10:01:00Z", None),
        ]);
        let ids = ordered
            .iter()
            .map(|item| item.conversation_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["independent", "parent", "child"]);
    }
}
