#[tauri::command]
fn get_prompt_preview(
    _input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<PromptPreview, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    let _ = ensure_default_agent(&mut data);
    let effective_agent_id = if data
        .agents
        .iter()
        .any(|a| a.id == data.selected_agent_id && !a.is_built_in_user)
    {
        data.selected_agent_id.clone()
    } else {
        data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let agent = data
        .agents
        .iter()
        .find(|a| a.id == effective_agent_id)
        .cloned()
        .ok_or_else(|| "Selected agent not found.".to_string())?;

    let conversation = latest_active_conversation_index(&data, "", &effective_agent_id)
        .and_then(|idx| data.conversations.get(idx).cloned())
        .unwrap_or_else(|| Conversation {
            id: "preview".to_string(),
            title: "Preview".to_string(),
            api_config_id: api_config.id.clone(),
            agent_id: effective_agent_id.clone(),
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            status: "active".to_string(),
            summary: String::new(),
            archived_at: None,
            messages: Vec::new(),
            memory_recall_table: Vec::new(),
        });

    let user_name = user_persona_name(&data);
    let user_intro = user_persona_intro(&data);
    let mut prepared = build_prompt(
        &conversation,
        &agent,
        &user_name,
        &user_intro,
        &data.response_style_id,
        &app_config.ui_language,
        Some(&state.data_path),
    );
    let last_archive_summary = data
        .conversations
        .iter()
        .rev()
        .find(|c| c.agent_id == effective_agent_id && !c.summary.trim().is_empty())
        .map(|c| c.summary.clone());
    if let Some(summary) = last_archive_summary {
        prepared.preamble.push_str(
            "\n[HIDDEN ARCHIVE RECAP]\nUSER: 上次我们聊到哪里？\nASSISTANT: ",
        );
        prepared.preamble.push_str(summary.trim());
        prepared.preamble.push('\n');
    }
    if let Some(terminal_block) = terminal_prompt_trusted_roots_block(&state, &api_config) {
        prepared.preamble.push('\n');
        prepared.preamble.push_str(&terminal_block);
        prepared.preamble.push('\n');
    }
    let mut user_content = vec![serde_json::json!({
        "type": "text",
        "text": prepared.latest_user_text,
    })];
    if !prepared.latest_user_time_text.trim().is_empty() {
        user_content.push(serde_json::json!({
            "type": "text",
            "text": prepared.latest_user_time_text,
        }));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        user_content.push(serde_json::json!({
            "type": "text",
            "text": prepared.latest_user_system_text,
        }));
    }
    for (mime, bytes_base64) in &prepared.latest_images {
        user_content.push(serde_json::json!({
            "type": "image",
            "mime": mime,
            "bytesBase64Length": bytes_base64.len(),
        }));
    }
    for (mime, bytes_base64) in &prepared.latest_audios {
        user_content.push(serde_json::json!({
            "type": "audio",
            "mime": mime,
            "bytesBase64Length": bytes_base64.len(),
        }));
    }
    let request_preview = build_request_preview_value(&api_config, &prepared, user_content);
    let request_body_json = serde_json::to_string_pretty(&request_preview)
        .map_err(|err| format!("Serialize request preview failed: {err}"))?;
    drop(guard);

    Ok(PromptPreview {
        preamble: prepared.preamble,
        latest_user_text: prepared.latest_user_text,
        latest_images: prepared.latest_images.len(),
        latest_audios: prepared.latest_audios.len(),
        request_body_json,
    })
}

fn build_request_preview_value(
    api_config: &ApiConfig,
    prepared: &PreparedPrompt,
    user_content: Vec<Value>,
) -> Value {
    let mut preview_messages = Vec::<Value>::new();
    preview_messages.push(serde_json::json!({
        "role": "system",
        "content": prepared.preamble.clone()
    }));
    for hm in &prepared.history_messages {
        if hm.role == "assistant" && hm.tool_calls.is_some() {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("assistant".to_string()));
            if hm.text.trim().is_empty() {
                msg.insert("content".to_string(), Value::Null);
            } else {
                msg.insert("content".to_string(), Value::String(hm.text.clone()));
            }
            if let Some(reasoning) = &hm.reasoning_content {
                if !reasoning.trim().is_empty() {
                    msg.insert("reasoning_content".to_string(), Value::String(reasoning.clone()));
                }
            }
            if let Some(calls) = &hm.tool_calls {
                msg.insert("tool_calls".to_string(), Value::Array(calls.clone()));
            }
            preview_messages.push(Value::Object(msg));
        } else if hm.role == "tool" {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("tool".to_string()));
            msg.insert("content".to_string(), Value::String(hm.text.clone()));
            if let Some(call_id) = &hm.tool_call_id {
                msg.insert("tool_call_id".to_string(), Value::String(call_id.clone()));
            }
            preview_messages.push(Value::Object(msg));
        } else {
            if hm.role == "user" {
                let mut content = vec![serde_json::json!(hm.text)];
                if let Some(time_text) = &hm.user_time_text {
                    if !time_text.trim().is_empty() {
                        content.push(serde_json::json!(time_text));
                    }
                }
                preview_messages.push(serde_json::json!({
                    "role": "user",
                    "content": content,
                }));
            } else {
                preview_messages.push(serde_json::json!({
                    "role": hm.role,
                    "content": hm.text,
                }));
            }
        }
    }
    preview_messages.push(serde_json::json!({
        "role": "user",
        "content": user_content
    }));
    serde_json::json!({
        "requestFormat": api_config.request_format,
        "baseUrl": api_config.base_url,
        "model": api_config.model,
        "temperature": api_config.temperature,
        "enableTools": api_config.enable_tools,
        "toolIds": api_config.tools.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
        "messages": preview_messages
    })
}

#[tauri::command]
fn get_system_prompt_preview(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<SystemPromptPreview, String> {
    let preview = get_prompt_preview(input, state)?;
    Ok(SystemPromptPreview {
        system_prompt: preview.preamble,
    })
}

fn archive_time_label(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return "unknown-time".to_string();
    }
    let mut normalized = s.replace('T', " ");
    if normalized.ends_with('Z') {
        normalized.pop();
    }
    if normalized.chars().count() >= 16 {
        normalized.chars().take(16).collect::<String>()
    } else {
        normalized
    }
}

fn archive_first_user_preview(conversation: &Conversation) -> String {
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
        compact.chars().take(10).collect::<String>()
    }
}

fn conversation_to_archive(conversation: &Conversation) -> ConversationArchive {
    ConversationArchive {
        archive_id: conversation.id.clone(),
        archived_at: conversation
            .archived_at
            .clone()
            .unwrap_or_else(|| conversation.updated_at.clone()),
        reason: "conversation_summary".to_string(),
        summary: conversation.summary.clone(),
        source_conversation: conversation.clone(),
    }
}

fn archived_conversations_from_data(data: &AppData) -> Vec<ConversationArchive> {
    let mut out = data
        .conversations
        .iter()
        .filter(|c| !c.summary.trim().is_empty())
        .map(conversation_to_archive)
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    out
}

fn archive_to_conversation(archive: ConversationArchive) -> Conversation {
    let mut conversation = archive.source_conversation;
    if conversation.id.trim().is_empty() {
        conversation.id = archive.archive_id;
    }
    if conversation.id.trim().is_empty() {
        conversation.id = Uuid::new_v4().to_string();
    }
    if conversation.summary.trim().is_empty() {
        conversation.summary = archive.summary;
    }
    if conversation.archived_at.as_deref().unwrap_or("").trim().is_empty() {
        conversation.archived_at = Some(archive.archived_at);
    }
    conversation.status = "archived".to_string();
    conversation
}

#[tauri::command]
fn list_archives(state: State<'_, AppState>) -> Result<Vec<ArchiveSummary>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let mut summaries = data
        .conversations
        .iter()
        .filter(|c| !c.summary.trim().is_empty())
        .map(|archive| ArchiveSummary {
            archive_id: archive.id.clone(),
            archived_at: archive
                .archived_at
                .clone()
                .unwrap_or_else(|| archive.updated_at.clone()),
            title: archive_first_user_preview(archive),
            message_count: archive.messages.len(),
            api_config_id: archive.api_config_id.clone(),
            agent_id: archive.agent_id.clone(),
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    Ok(summaries)
}

#[tauri::command]
fn get_archive_messages(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let archive = data
        .conversations
        .iter()
        .find(|c| c.id == archive_id && !c.summary.trim().is_empty())
        .ok_or_else(|| "Archive not found".to_string())?;

    let mut messages = archive.messages.clone();
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[tauri::command]
fn get_archive_summary(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let archive = data
        .conversations
        .iter()
        .find(|c| c.id == archive_id && !c.summary.trim().is_empty())
        .ok_or_else(|| "Archive not found".to_string())?;

    Ok(archive.summary.clone())
}

#[tauri::command]
fn delete_archive(archive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if archive_id.trim().is_empty() {
        return Err("archiveId is required".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    let before = data.conversations.len();
    data.conversations
        .retain(|c| !(c.id == archive_id && !c.summary.trim().is_empty()));

    if data.conversations.len() == before {
        drop(guard);
        return Err("Archive not found".to_string());
    }

    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArchiveToFileInput {
    archive_id: String,
    format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArchiveFileResult {
    path: String,
    archive_id: String,
    format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveExportPayload {
    version: u32,
    exported_at: String,
    archive: ConversationArchive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportArchivesFromJsonInput {
    payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportArchivesResult {
    imported_count: usize,
    replaced_count: usize,
    skipped_count: usize,
    total_count: usize,
    selected_archive_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportBatchPayload {
    archives: Vec<ConversationArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportAppDataPayload {
    archived_conversations: Vec<ConversationArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportConversationsPayload {
    conversations: Vec<Conversation>,
}

fn parse_archives_for_import(raw: &str) -> Result<Vec<ConversationArchive>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Archive payload is empty".to_string());
    }
    if let Ok(payload) = serde_json::from_str::<ArchiveExportPayload>(trimmed) {
        return Ok(vec![payload.archive]);
    }
    if let Ok(archive) = serde_json::from_str::<ConversationArchive>(trimmed) {
        return Ok(vec![archive]);
    }
    if let Ok(batch) = serde_json::from_str::<ArchiveImportBatchPayload>(trimmed) {
        if !batch.archives.is_empty() {
            return Ok(batch.archives);
        }
    }
    if let Ok(batch) = serde_json::from_str::<ArchiveImportAppDataPayload>(trimmed) {
        if !batch.archived_conversations.is_empty() {
            return Ok(batch.archived_conversations);
        }
    }
    if let Ok(batch) = serde_json::from_str::<ArchiveImportConversationsPayload>(trimmed) {
        let out = batch
            .conversations
            .into_iter()
            .filter(|c| !c.summary.trim().is_empty())
            .map(|c| conversation_to_archive(&c))
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return Ok(out);
        }
    }
    if let Ok(list) = serde_json::from_str::<Vec<ConversationArchive>>(trimmed) {
        if !list.is_empty() {
            return Ok(list);
        }
    }
    Err("Invalid archive payload. Expected exported archive JSON.".to_string())
}

fn normalize_media_for_import(data_path: &PathBuf, mime: &str, stored: &str) -> String {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if media_id_from_marker(trimmed).is_some() {
        let Ok(decoded) = resolve_stored_binary_base64(data_path, trimmed) else {
            return String::new();
        };
        return externalize_stored_binary_base64(data_path, mime, &decoded).unwrap_or_default();
    }
    externalize_stored_binary_base64(data_path, mime, trimmed).unwrap_or_default()
}

fn normalize_archive_for_import(archive: &mut ConversationArchive, data_path: &PathBuf) {
    if archive.archive_id.trim().is_empty() {
        archive.archive_id = Uuid::new_v4().to_string();
    }
    if archive.archived_at.trim().is_empty() {
        archive.archived_at = now_iso();
    }
    archive.reason = clean_text(archive.reason.trim());
    if archive.reason.is_empty() {
        archive.reason = "import_archive".to_string();
    }
    archive.summary = clean_text(archive.summary.trim());

    let conversation = &mut archive.source_conversation;
    if conversation.id.trim().is_empty() {
        conversation.id = Uuid::new_v4().to_string();
    }
    conversation.title = clean_text(conversation.title.trim());
    if conversation.title.is_empty() {
        conversation.title = format!("Imported {}", archive_time_label(&archive.archived_at));
    }
    if conversation.created_at.trim().is_empty() {
        conversation.created_at = archive.archived_at.clone();
    }
    if conversation.updated_at.trim().is_empty() {
        conversation.updated_at = conversation.created_at.clone();
    }
    conversation.status = "archived".to_string();
    if conversation.last_user_at.as_ref().map(|v| v.trim().is_empty()).unwrap_or(false) {
        conversation.last_user_at = None;
    }
    if conversation
        .last_assistant_at
        .as_ref()
        .map(|v| v.trim().is_empty())
        .unwrap_or(false)
    {
        conversation.last_assistant_at = None;
    }
    if !conversation.last_context_usage_ratio.is_finite() {
        conversation.last_context_usage_ratio = 0.0;
    }

    for message in &mut conversation.messages {
        if message.id.trim().is_empty() {
            message.id = Uuid::new_v4().to_string();
        }
        if message.created_at.trim().is_empty() {
            message.created_at = conversation.updated_at.clone();
        }
        message.role = clean_text(message.role.trim());
        if message.role.is_empty() {
            message.role = "user".to_string();
        }
        for part in &mut message.parts {
            match part {
                MessagePart::Text { text } => {
                    *text = clean_text(text.trim());
                }
                MessagePart::Image {
                    mime,
                    bytes_base64,
                    name,
                    ..
                } => {
                    *mime = clean_text(mime.trim());
                    if mime.is_empty() {
                        *mime = "image/webp".to_string();
                    }
                    *bytes_base64 = normalize_media_for_import(data_path, mime, bytes_base64);
                    *name = name
                        .as_ref()
                        .map(|v| clean_text(v.trim()))
                        .filter(|v| !v.is_empty());
                }
                MessagePart::Audio {
                    mime,
                    bytes_base64,
                    name,
                    ..
                } => {
                    *mime = clean_text(mime.trim());
                    if mime.is_empty() {
                        *mime = "audio/webm".to_string();
                    }
                    *bytes_base64 = normalize_media_for_import(data_path, mime, bytes_base64);
                    *name = name
                        .as_ref()
                        .map(|v| clean_text(v.trim()))
                        .filter(|v| !v.is_empty());
                }
            }
        }
        message
            .extra_text_blocks
            .iter_mut()
            .for_each(|text| *text = clean_text(text.trim()));
        message.extra_text_blocks.retain(|text| !text.is_empty());
    }
}

fn archive_message_plain_text(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.trim().to_string()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn archive_message_image_count(message: &ChatMessage) -> usize {
    message
        .parts
        .iter()
        .filter(|part| matches!(part, MessagePart::Image { .. }))
        .count()
}

fn archive_message_audio_count(message: &ChatMessage) -> usize {
    message
        .parts
        .iter()
        .filter(|part| matches!(part, MessagePart::Audio { .. }))
        .count()
}

fn tool_call_markdown_lines(message: &ChatMessage) -> Vec<String> {
    let mut out = Vec::new();
    let Some(events) = message.tool_call.as_ref() else {
        return out;
    };

    for event in events {
        let Some(role) = event.get("role").and_then(Value::as_str) else {
            continue;
        };
        if role == "assistant" {
            let calls = event
                .get("tool_calls")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for call in calls {
                let name = call
                    .get("function")
                    .and_then(Value::as_object)
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let args = call
                    .get("function")
                    .and_then(Value::as_object)
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if args.is_empty() {
                    out.push(format!("- 工具调用: {name}"));
                } else {
                    out.push(format!("- 工具调用: {name} | 参数: {args}"));
                }
            }
        } else if role == "tool" {
            let content = event
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if !content.is_empty() {
                let snippet = if content.chars().count() > 300 {
                    format!("{}...", content.chars().take(300).collect::<String>())
                } else {
                    content.to_string()
                };
                out.push(format!("- 工具结果: {snippet}"));
            }
        }
    }
    out
}

fn archive_message_markdown_block(message: &ChatMessage) -> String {
    let role = match message.role.as_str() {
        "user" => "用户",
        "assistant" => "助手",
        "tool" => "工具",
        other => other,
    };
    let mut lines = Vec::<String>::new();
    lines.push(format!("### {}  {}", role, message.created_at));

    let text = archive_message_plain_text(message);
    if !text.is_empty() {
        lines.push(text);
    }

    let image_count = archive_message_image_count(message);
    if image_count > 0 {
        lines.push(format!("- 图片 x{image_count}"));
    }
    let audio_count = archive_message_audio_count(message);
    if audio_count > 0 {
        lines.push(format!("- 音频 x{audio_count}"));
    }

    for line in tool_call_markdown_lines(message) {
        lines.push(line);
    }

    if lines.len() == 1 {
        lines.push("- (空消息)".to_string());
    }
    lines.join("\n")
}

fn build_archive_markdown(archive: &ConversationArchive) -> String {
    let mut blocks = Vec::<String>::new();
    blocks.push("# 对话归档".to_string());
    blocks.push(format!("- 标题: {}", archive.source_conversation.title));
    blocks.push(format!("- 归档时间: {}", archive.archived_at));
    if !archive.summary.trim().is_empty() {
        blocks.push(String::new());
        blocks.push("## 摘要".to_string());
        blocks.push(archive.summary.trim().to_string());
    }
    blocks.push(String::new());
    blocks.push("## 消息时间线".to_string());
    for message in &archive.source_conversation.messages {
        let role = message.role.as_str();
        if role != "user" && role != "assistant" && role != "tool" {
            continue;
        }
        blocks.push(String::new());
        blocks.push(archive_message_markdown_block(message));
    }
    blocks.join("\n")
}

#[tauri::command]
fn export_archive_to_file(
    input: ExportArchiveToFileInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExportArchiveFileResult, String> {
    if input.archive_id.trim().is_empty() {
        return Err("archiveId is required".to_string());
    }
    let export_format = match input.format.trim().to_ascii_lowercase().as_str() {
        "json" => "json",
        "markdown" | "md" => "markdown",
        _ => return Err("Unsupported export format. Use 'json' or 'markdown'.".to_string()),
    };

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let archive = data
        .conversations
        .iter()
        .find(|c| c.id == input.archive_id && !c.summary.trim().is_empty())
        .cloned()
        .ok_or_else(|| "Archive not found".to_string())?;
    let mut archive = conversation_to_archive(&archive);
    if export_format == "json" {
        materialize_chat_message_parts_from_media_refs(
            &mut archive.source_conversation.messages,
            &state.data_path,
        );
    }

    let selected = if export_format == "json" {
        app.dialog()
            .file()
            .add_filter("JSON", &["json"])
            .blocking_save_file()
    } else {
        app.dialog()
            .file()
            .add_filter("Markdown", &["md", "markdown"])
            .blocking_save_file()
    };

    let file_path = selected
        .and_then(|fp| fp.as_path().map(ToOwned::to_owned))
        .ok_or_else(|| "Export cancelled".to_string())?;

    let body = if export_format == "json" {
        let payload = ArchiveExportPayload {
            version: 1,
            exported_at: now_iso(),
            archive: archive.clone(),
        };
        serde_json::to_string_pretty(&payload)
            .map_err(|err| format!("Serialize archive export failed: {err}"))?
    } else {
        build_archive_markdown(&archive)
    };

    fs::write(&file_path, body).map_err(|err| format!("Write export file failed: {err}"))?;

    Ok(ExportArchiveFileResult {
        path: file_path.to_string_lossy().to_string(),
        archive_id: archive.archive_id,
        format: export_format.to_string(),
    })
}

#[tauri::command]
fn import_archives_from_json(
    input: ImportArchivesFromJsonInput,
    state: State<'_, AppState>,
) -> Result<ImportArchivesResult, String> {
    let mut incoming_archives = parse_archives_for_import(&input.payload_json)?;
    if incoming_archives.is_empty() {
        return Err("No archives found in payload.".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&state.data_path)?;

    let mut index_by_conversation_id = std::collections::HashMap::<String, usize>::new();
    for (idx, conv) in data.conversations.iter().enumerate() {
        index_by_conversation_id.insert(conv.id.clone(), idx);
    }

    let mut imported_count = 0usize;
    let mut replaced_count = 0usize;
    let skipped_count = 0usize;
    let mut selected_archive_id: Option<String> = None;

    for archive in &mut incoming_archives {
        normalize_archive_for_import(archive, &state.data_path);
    }

    for archive in incoming_archives {
        let archive_id = archive.archive_id.clone();
        let conversation = archive_to_conversation(archive);
        let conversation_id = conversation.id.clone();
        if let Some(idx) = index_by_conversation_id.get(&conversation_id).copied() {
            data.conversations[idx] = conversation;
            replaced_count += 1;
        } else {
            data.conversations.push(conversation);
            index_by_conversation_id.insert(conversation_id, data.conversations.len() - 1);
            imported_count += 1;
        }
        if selected_archive_id.is_none() {
            selected_archive_id = Some(archive_id);
        }
    }

    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ImportArchivesResult {
        imported_count,
        replaced_count,
        skipped_count,
        total_count: archived_conversations_from_data(&data).len(),
        selected_archive_id,
    })
}

#[tauri::command]
fn list_memories(state: State<'_, AppState>) -> Result<Vec<MemoryEntry>, String> {
    memory_store_list_memories(&state.data_path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteMemoryInput {
    memory_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteMemoryResult {
    status: String,
}

#[tauri::command]
fn delete_memory(
    input: DeleteMemoryInput,
    state: State<'_, AppState>,
) -> Result<DeleteMemoryResult, String> {
    memory_store_delete_memory(&state.data_path, &input.memory_id)?;
    Ok(DeleteMemoryResult {
        status: "deleted".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryExportPayload {
    version: u32,
    exported_at: String,
    memories: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportMemoriesInput {
    memories: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportMemoriesResult {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMemoriesMixedInput {
    query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMemoriesMixedResult {
    memories: Vec<SearchMemoriesMixedHit>,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMemoriesMixedHit {
    memory: MemoryEntry,
    bm25_score: f64,
    bm25_raw_score: f64,
    vector_score: f64,
    final_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMemoriesFileResult {
    path: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMemoriesToPathInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncMemoryEmbeddingProviderInput {
    provider_id: String,
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryEmbeddingProviderInput {
    provider_id: Option<String>,
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryRerankProviderInput {
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    documents: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryEmbeddingProviderResult {
    provider_kind: String,
    model_name: String,
    vector_dim: usize,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryRerankProviderResult {
    provider_kind: String,
    model_name: String,
    elapsed_ms: u128,
    result_count: usize,
    top_index: Option<usize>,
    top_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveMemoryEmbeddingBindingInput {
    api_config_id: String,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveMemoryRerankBindingInput {
    api_config_id: String,
    #[serde(default)]
    model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryProviderBindings {
    #[serde(default)]
    embedding_api_config_id: Option<String>,
    #[serde(default)]
    rerank_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveMemoryRerankBindingResult {
    status: String,
    rerank_api_config_id: Option<String>,
    model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryEmbeddingSyncProgress {
    status: String,
    done_batches: usize,
    total_batches: usize,
    trace_id: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryHealthCheckInput {
    #[serde(default)]
    auto_repair: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryBackupInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryRestoreInput {
    path: String,
}

#[tauri::command]
fn export_memories(state: State<'_, AppState>) -> Result<MemoryExportPayload, String> {
    let memories = memory_store_list_memories(&state.data_path)?;

    Ok(MemoryExportPayload {
        version: 1,
        exported_at: now_iso(),
        memories,
    })
}

#[tauri::command]
fn export_memories_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExportMemoriesFileResult, String> {
    let memories = memory_store_list_memories(&state.data_path)?;
    let payload = MemoryExportPayload {
        version: 1,
        exported_at: now_iso(),
        memories,
    };
    let selected = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_save_file();
    let file_path = selected
        .and_then(|fp| fp.as_path().map(ToOwned::to_owned))
        .ok_or_else(|| "Export cancelled".to_string())?;
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Serialize export payload failed: {err}"))?;
    fs::write(&file_path, body).map_err(|err| format!("Write export file failed: {err}"))?;

    Ok(ExportMemoriesFileResult {
        path: file_path.to_string_lossy().to_string(),
        count: payload.memories.len(),
    })
}

#[tauri::command]
fn export_memories_to_path(
    input: ExportMemoriesToPathInput,
    state: State<'_, AppState>,
) -> Result<ExportMemoriesFileResult, String> {
    let target = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("Export path is empty".to_string());
    }
    let parent = target
        .parent()
        .ok_or_else(|| "Export path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("Create export dir failed: {err}"))?;

    let memories = memory_store_list_memories(&state.data_path)?;
    let payload = MemoryExportPayload {
        version: 1,
        exported_at: now_iso(),
        memories,
    };
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Serialize export payload failed: {err}"))?;
    fs::write(&target, body).map_err(|err| format!("Write export file failed: {err}"))?;

    Ok(ExportMemoriesFileResult {
        path: target.to_string_lossy().to_string(),
        count: payload.memories.len(),
    })
}

#[tauri::command]
fn import_memories(
    input: ImportMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ImportMemoriesResult, String> {
    let stats = memory_store_import_memories(&state.data_path, &input.memories)?;
    Ok(ImportMemoriesResult {
        imported_count: stats.imported_count,
        created_count: stats.created_count,
        merged_count: stats.merged_count,
        total_count: stats.total_count,
    })
}

#[tauri::command]
fn search_memories_mixed(
    input: SearchMemoriesMixedInput,
    state: State<'_, AppState>,
) -> Result<SearchMemoriesMixedResult, String> {
    let started = std::time::Instant::now();
    let query = input.query.trim();
    if query.is_empty() {
        // Empty query is intentionally used by the frontend as "browse all memories" mode.
        // Real mixed retrieval always provides non-empty query text.
        return Ok(SearchMemoriesMixedResult {
            memories: memory_store_list_memories(&state.data_path)?
                .into_iter()
                .map(|memory| SearchMemoriesMixedHit {
                    memory,
                    bm25_score: 0.0,
                    bm25_raw_score: 0.0,
                    vector_score: 0.0,
                    final_score: 0.0,
                })
                .collect::<Vec<_>>(),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }

    let memories = memory_store_list_memories(&state.data_path)?;
    let ranked = memory_mixed_ranked_items(
        &state.data_path,
        &memories,
        query,
        MEMORY_MATCH_MAX_ITEMS * MEMORY_CANDIDATE_MULTIPLIER,
    );
    if ranked.is_empty() {
        return Ok(SearchMemoriesMixedResult {
            memories: Vec::new(),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }

    let memory_map = memories
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect::<std::collections::HashMap<_, _>>();
    let mut out = Vec::<SearchMemoriesMixedHit>::new();
    for item in ranked {
        if let Some(memory) = memory_map.get(&item.memory_id) {
            out.push(SearchMemoriesMixedHit {
                memory: memory.clone(),
                bm25_score: item.bm25_score,
                bm25_raw_score: item.bm25_raw_score,
                vector_score: item.vector_score,
                final_score: item.final_score,
            });
        }
    }
    Ok(SearchMemoriesMixedResult {
        memories: out,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[tauri::command]
fn sync_memory_embedding_provider(
    input: SyncMemoryEmbeddingProviderInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreProviderSyncReport, String> {
    let provider_id = input.provider_id.trim();
    if provider_id.is_empty() {
        return Err("providerId is required".to_string());
    }
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("");
    let batch_size = input.batch_size.unwrap_or(64).max(1);
    let provider_kind = memory_provider_kind_from_id(provider_id);

    if matches!(provider_kind, MemoryProviderKind::DeterministicLocal) {
        let deterministic_model = if model_name.is_empty() {
            "deterministic-local-embedder"
        } else {
            model_name
        };
        return memory_store_sync_provider_index(
            &state.data_path,
            provider_id,
            deterministic_model,
            batch_size,
            |texts| {
                let mut out = Vec::<Vec<f32>>::new();
                for text in texts {
                    let mut hasher = Sha256::new();
                    hasher.update(provider_id.as_bytes());
                    hasher.update(b"|");
                    hasher.update(text.as_bytes());
                    let digest = hasher.finalize();
                    let mut vec = Vec::<f32>::new();
                    for chunk in digest.chunks(4) {
                        let mut bytes = [0u8; 4];
                        for (idx, b) in chunk.iter().enumerate() {
                            bytes[idx] = *b;
                        }
                        let value = u32::from_le_bytes(bytes) as f32 / u32::MAX as f32;
                        vec.push(value);
                    }
                    out.push(vec);
                }
                Ok(out)
            },
        );
    }

    let app_config = read_config(&state.config_path)?;
    let provider_cfg = memory_resolve_provider_api_config(
        &app_config,
        provider_kind,
        input.api_config_id.as_deref(),
        provider_id,
    )
    .ok_or_else(|| {
        format!(
            "No API config matches provider kind '{provider_kind:?}'. Please set apiConfigId."
        )
    })?;
    let embedding_provider = memory_create_embedding_provider(
        provider_kind,
        &provider_cfg,
        if model_name.is_empty() {
            None
        } else {
            Some(model_name)
        },
    )?;
    let model_for_report = if model_name.is_empty() {
        provider_cfg.model.as_str()
    } else {
        model_name
    };

    memory_store_sync_provider_index(
        &state.data_path,
        provider_id,
        model_for_report,
        batch_size,
        |texts| embedding_provider.embed_batch(texts),
    )
}

#[tauri::command]
fn test_memory_embedding_provider(
    input: TestMemoryEmbeddingProviderInput,
    state: State<'_, AppState>,
) -> Result<TestMemoryEmbeddingProviderResult, String> {
    let started = std::time::Instant::now();
    let provider_id = input.provider_id.as_deref().unwrap_or("openai_embedding");
    let provider_kind = memory_provider_kind_from_id(provider_id);
    if matches!(provider_kind, MemoryProviderKind::VllmRerank) {
        return Err("rerank provider cannot be used as embedding provider.".to_string());
    }
    let app_config = read_config(&state.config_path)?;
    let provider_cfg = memory_resolve_provider_api_config(
        &app_config,
        provider_kind,
        input.api_config_id.as_deref(),
        provider_id,
    )
    .ok_or_else(|| "No matching API config for embedding test.".to_string())?;
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let provider = memory_create_embedding_provider(provider_kind, &provider_cfg, model_name)?;
    let text = input
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("memory embedding connectivity test")
        .to_string();
    let vectors = provider.embed_batch(&vec![text])?;
    let first = vectors
        .first()
        .ok_or_else(|| "embedding test returned empty vectors".to_string())?;
    let dim = first.len();
    if dim == 0 {
        return Err("embedding test returned zero-dim vector".to_string());
    }
    Ok(TestMemoryEmbeddingProviderResult {
        provider_kind: format!("{provider_kind:?}"),
        model_name: model_name.unwrap_or(provider_cfg.model.trim()).to_string(),
        vector_dim: dim,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[tauri::command]
fn test_memory_rerank_provider(
    input: TestMemoryRerankProviderInput,
    state: State<'_, AppState>,
) -> Result<TestMemoryRerankProviderResult, String> {
    let started = std::time::Instant::now();
    let app_config = read_config(&state.config_path)?;
    let provider_kind = MemoryProviderKind::VllmRerank;
    let provider_cfg = memory_resolve_provider_api_config(
        &app_config,
        provider_kind,
        input.api_config_id.as_deref(),
        "vllm_rerank",
    )
    .ok_or_else(|| "No matching API config for rerank test.".to_string())?;
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let provider = memory_create_rerank_provider(provider_kind, &provider_cfg, model_name)?;
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("用户偏好什么风格？")
        .to_string();
    let documents = input.documents.unwrap_or_else(|| {
        vec![
            "用户偏好简洁回答，尽量直接结论。".to_string(),
            "用户喜欢复杂铺垫和长篇解释。".to_string(),
            "今天主要讨论了记忆系统检索。".to_string(),
        ]
    });
    let results = provider.rerank(&query, &documents, Some(3))?;
    let top = results
        .iter()
        .max_by(|a, b| a.relevance_score.partial_cmp(&b.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(TestMemoryRerankProviderResult {
        provider_kind: format!("{provider_kind:?}"),
        model_name: model_name.unwrap_or(provider_cfg.model.trim()).to_string(),
        elapsed_ms: started.elapsed().as_millis(),
        result_count: results.len(),
        top_index: top.map(|t| t.index),
        top_score: top.map(|t| t.relevance_score),
    })
}

fn memory_binding_provider_id(api_id: &str, request_format: &str, model: &str) -> String {
    let norm = |raw: &str| -> String {
        raw.trim()
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' { c } else { '_' })
            .collect::<String>()
            .replace("__", "_")
            .trim_matches('_')
            .to_string()
    };
    let id = norm(api_id);
    let fmt = norm(request_format);
    let mdl = norm(model);
    format!("{id}_{fmt}_{mdl}")
}

#[tauri::command]
fn get_memory_provider_bindings(state: State<'_, AppState>) -> Result<MemoryProviderBindings, String> {
    let conn = memory_store_open(&state.data_path)?;
    Ok(MemoryProviderBindings {
        embedding_api_config_id: memory_store_get_runtime_state(&conn, "embedding_api_config_id")?,
        rerank_api_config_id: memory_store_get_runtime_state(&conn, "rerank_api_config_id")?,
    })
}

#[tauri::command]
fn get_memory_embedding_sync_progress(state: State<'_, AppState>) -> Result<MemoryEmbeddingSyncProgress, String> {
    let conn = memory_store_open(&state.data_path)?;
    let status = memory_store_get_runtime_state(&conn, "rebuild_status")?
        .unwrap_or_else(|| "idle".to_string());
    let done_batches = memory_store_get_runtime_state(&conn, "rebuild_done_batches")?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let total_batches = memory_store_get_runtime_state(&conn, "rebuild_total_batches")?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let trace_id = memory_store_get_runtime_state(&conn, "rebuild_trace_id")?;
    let error = memory_store_get_runtime_state(&conn, "rebuild_error")?;
    Ok(MemoryEmbeddingSyncProgress {
        status,
        done_batches,
        total_batches,
        trace_id,
        error,
    })
}

#[tauri::command]
fn save_memory_embedding_binding(
    input: SaveMemoryEmbeddingBindingInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreProviderSyncReport, String> {
    let api_id = input.api_config_id.trim();
    if api_id.is_empty() {
        let conn = memory_store_open(&state.data_path)?;
        let old_provider_id = memory_store_get_runtime_state(&conn, "active_index_provider_id")?;
        memory_store_set_runtime_state(&conn, "embedding_api_config_id", "")?;
        memory_store_set_runtime_state(&conn, "active_index_provider_id", "")?;
        return Ok(MemoryStoreProviderSyncReport {
            status: "disabled".to_string(),
            old_provider_id,
            new_provider_id: String::new(),
            deleted: 0,
            added: 0,
            batch_count: 0,
        });
    }
    let app_config = read_config(&state.config_path)?;
    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == api_id)
        .cloned()
        .ok_or_else(|| "Selected embedding API config not found.".to_string())?;

    let provider_kind = match api.request_format {
        RequestFormat::OpenAIEmbedding => MemoryProviderKind::OpenAIEmbedding,
        RequestFormat::GeminiEmbedding => MemoryProviderKind::GeminiEmbedding,
        _ => {
            return Err(format!(
                "request_format '{}' is not embedding protocol.",
                api.request_format
            ))
        }
    };
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(api.model.trim());
    if model_name.is_empty() {
        return Err("Embedding model is empty.".to_string());
    }
    let provider_cfg = MemoryProviderApiConfig {
        base_url: api.base_url.clone(),
        api_key: api.api_key.clone(),
        model: api.model.clone(),
    };
    let provider = memory_create_embedding_provider(provider_kind, &provider_cfg, Some(model_name))?;

    let provider_id = memory_binding_provider_id(&api.id, api.request_format.as_str(), model_name);
    let batch_size = input.batch_size.unwrap_or(64).max(1);
    let report = memory_store_sync_provider_index(
        &state.data_path,
        &provider_id,
        model_name,
        batch_size,
        |texts| provider.embed_batch(texts),
    )?;

    let conn = memory_store_open(&state.data_path)?;
    memory_store_set_runtime_state(&conn, "embedding_api_config_id", &api.id)?;
    Ok(report)
}

#[tauri::command]
fn save_memory_rerank_binding(
    input: SaveMemoryRerankBindingInput,
    state: State<'_, AppState>,
) -> Result<SaveMemoryRerankBindingResult, String> {
    let api_id = input.api_config_id.trim();
    if api_id.is_empty() {
        let conn = memory_store_open(&state.data_path)?;
        memory_store_set_runtime_state(&conn, "rerank_api_config_id", "")?;
        return Ok(SaveMemoryRerankBindingResult {
            status: "disabled".to_string(),
            rerank_api_config_id: None,
            model_name: String::new(),
        });
    }
    let app_config = read_config(&state.config_path)?;
    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == api_id)
        .cloned()
        .ok_or_else(|| "Selected rerank API config not found.".to_string())?;
    if !matches!(api.request_format, RequestFormat::OpenAIRerank) {
        return Err(format!(
            "request_format '{}' is not rerank protocol.",
            api.request_format
        ));
    }
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(api.model.trim());
    if model_name.is_empty() {
        return Err("Rerank model is empty.".to_string());
    }

    let conn = memory_store_open(&state.data_path)?;
    memory_store_set_runtime_state(&conn, "rerank_api_config_id", &api.id)?;
    Ok(SaveMemoryRerankBindingResult {
        status: "saved".to_string(),
        rerank_api_config_id: Some(api.id),
        model_name: model_name.to_string(),
    })
}

#[tauri::command]
fn memory_rebuild_indexes(state: State<'_, AppState>) -> Result<MemoryStoreRebuildReport, String> {
    memory_store_rebuild_indexes(&state.data_path)
}

#[tauri::command]
fn memory_health_check(
    input: MemoryHealthCheckInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreHealthReport, String> {
    memory_store_health_check(&state.data_path, input.auto_repair)
}

#[tauri::command]
fn memory_backup_db(
    input: MemoryBackupInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreBackupResult, String> {
    let path = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("backup path is empty".to_string());
    }
    memory_store_backup_db(&state.data_path, &path)
}

#[tauri::command]
fn memory_restore_db(
    input: MemoryRestoreInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreBackupResult, String> {
    let path = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("restore path is empty".to_string());
    }
    memory_store_restore_db(&state.data_path, &path)
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("Only http/https URLs are allowed.".to_string());
    }
    webbrowser::open(trimmed).map_err(|err| format!("Open browser failed: {err}"))?;
    Ok(())
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Debug, Clone, Deserialize)]
struct ArchiveMemoryDraft {
    #[serde(default, alias = "memoryType")]
    memory_type: String,
    #[serde(default, alias = "content")]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default, alias = "keywords")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveMergeGroupDraft {
    #[serde(default)]
    source_ids: Vec<String>,
    target: ArchiveMemoryDraft,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveSummaryDraft {
    summary: String,
    #[serde(default)]
    useful_memory_ids: Vec<String>,
    #[serde(default, alias = "memories")]
    new_memories: Vec<ArchiveMemoryDraft>,
    #[serde(default)]
    merge_groups: Vec<ArchiveMergeGroupDraft>,
}

fn parse_archive_summary_draft(raw: &str) -> Option<ArchiveSummaryDraft> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<ArchiveSummaryDraft>(trimmed) {
        return Some(parsed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<ArchiveSummaryDraft>(&trimmed[start..=end]).ok()
}

fn upsert_memories_into_store_with_ids(
    data_path: &PathBuf,
    drafts: &[ArchiveMemoryDraft],
    owner_agent_id: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut inputs = Vec::<MemoryDraftInput>::new();
    for d in drafts {
        let judgment = clean_text(d.judgment.trim());
        if judgment.is_empty() {
            continue;
        }
        let tags = normalize_memory_keywords(&d.tags);
        if tags.is_empty() {
            continue;
        }
        inputs.push(MemoryDraftInput {
            memory_type: d.memory_type.clone(),
            judgment,
            reasoning: clean_text(d.reasoning.trim()),
            tags,
            owner_agent_id: owner_agent_id
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned),
        });
    }
    let (results, _) = memory_store_upsert_drafts(data_path, &inputs)?;
    Ok(results.into_iter().filter_map(|r| r.id).collect::<Vec<_>>())
}

fn merge_memories_into_store(
    data_path: &PathBuf,
    drafts: &[ArchiveMemoryDraft],
    owner_agent_id: Option<&str>,
) -> Result<usize, String> {
    Ok(upsert_memories_into_store_with_ids(data_path, drafts, owner_agent_id)?.len())
}

fn merge_memory_groups_into_store(
    data_path: &PathBuf,
    groups: &[ArchiveMergeGroupDraft],
    owner_agent_id: Option<&str>,
) -> Result<usize, String> {
    let mut merged_groups = 0usize;
    for group in groups {
        let source_ids = group
            .source_ids
            .iter()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if source_ids.len() < 2 {
            continue;
        }
        let upserted_ids = upsert_memories_into_store_with_ids(
            data_path,
            &[group.target.clone()],
            owner_agent_id,
        )?;
        let retained = upserted_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<HashSet<_>>();
        let mut deleted_any = false;
        for source_id in source_ids {
            if retained.contains(source_id.as_str()) {
                continue;
            }
            if memory_store_delete_memory(data_path, &source_id).is_ok() {
                deleted_any = true;
            }
        }
        if deleted_any {
            merged_groups += 1;
        }
    }
    Ok(merged_groups)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceArchiveResult {
    archived: bool,
    archive_id: Option<String>,
    summary: String,
    merged_memories: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_feedback: Option<MemoryArchiveFeedbackReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_groups: Option<usize>,
}

const ARCHIVE_DEBUG_LOG_ENABLED: bool = false;

fn archive_endpoint_hint(format: RequestFormat) -> &'static str {
    match format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => "/chat/completions",
        RequestFormat::Gemini => "/models/*:generateContent",
        RequestFormat::Anthropic => "/v1/messages",
        RequestFormat::OpenAITts => "/audio/speech",
        RequestFormat::OpenAIStt => "/audio/transcriptions",
        RequestFormat::GeminiEmbedding => "/models/*:embedContent",
        RequestFormat::OpenAIEmbedding => "/embeddings",
        RequestFormat::OpenAIRerank => "/rerank",
    }
}

fn archive_log_request(
    trace_tag: &str,
    trace_id: &str,
    stage: &str,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    prepared: &PreparedPrompt,
    timeout_secs: u64,
) {
    if !ARCHIVE_DEBUG_LOG_ENABLED {
        return;
    }
    let mut user_content = vec![serde_json::json!({
        "type": "text",
        "text": prepared.latest_user_text,
    })];
    if !prepared.latest_user_time_text.trim().is_empty() {
        user_content.push(serde_json::json!({
            "type": "text",
            "text": prepared.latest_user_time_text,
        }));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        user_content.push(serde_json::json!({
            "type": "text",
            "text": prepared.latest_user_system_text,
        }));
    }
    for (mime, bytes_base64) in &prepared.latest_images {
        user_content.push(serde_json::json!({
            "type": "image",
            "mime": mime,
            "bytesBase64Length": bytes_base64.len(),
        }));
    }
    for (mime, bytes_base64) in &prepared.latest_audios {
        user_content.push(serde_json::json!({
            "type": "audio",
            "mime": mime,
            "bytesBase64Length": bytes_base64.len(),
        }));
    }
    let payload = build_request_preview_value(selected_api, prepared, user_content);
    let payload_pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
    let history_count = prepared.history_messages.len();
    let preamble_chars = prepared.preamble.chars().count();
    let latest_user_chars = prepared.latest_user_text.chars().count();
    let latest_system_chars = prepared.latest_user_system_text.chars().count();
    eprintln!(
        "[{}] trace={} stage={} request format={} base_url={} endpoint_hint={} model={} timeout_s={} stats={{history:{}, system_chars:{}, user_chars:{}, latest_system_chars:{}}}",
        trace_tag,
        trace_id,
        stage,
        resolved_api.request_format,
        resolved_api.base_url,
        archive_endpoint_hint(resolved_api.request_format),
        selected_api.model,
        timeout_secs,
        history_count,
        preamble_chars,
        latest_user_chars,
        latest_system_chars,
    );
    eprintln!(
        "[{}] trace={} stage={} request_system_prompt={}",
        trace_tag, trace_id, stage, prepared.preamble
    );
    eprintln!(
        "[{}] trace={} stage={} request_latest_user_prompt={}",
        trace_tag, trace_id, stage, prepared.latest_user_text
    );
    eprintln!(
        "[{}] trace={} stage={} request_body_json={}",
        trace_tag, trace_id, stage, payload_pretty
    );
}

fn archive_log_response(
    trace_tag: &str,
    trace_id: &str,
    stage: &str,
    elapsed_ms: u128,
    assistant_text: &str,
) {
    if !ARCHIVE_DEBUG_LOG_ENABLED {
        return;
    }
    eprintln!(
        "[{}] trace={} stage={} response elapsed_ms={} chars={} body={}",
        trace_tag,
        trace_id,
        stage,
        elapsed_ms,
        assistant_text.chars().count(),
        assistant_text
    );
}

fn build_archive_history_messages(source_conversation: &Conversation) -> Vec<PreparedHistoryMessage> {
    source_conversation
        .messages
        .iter()
        .map(|msg| {
            let mut text = render_message_for_context(msg);
            if text.trim().is_empty() {
                text = archive_message_plain_text(msg);
            }
            let reasoning_content = msg
                .provider_meta
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("reasoningStandard").and_then(Value::as_str))
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            PreparedHistoryMessage {
                role: msg.role.clone(),
                text,
                user_time_text: if msg.role == "user" {
                    Some(format_message_time_text(&msg.created_at))
                } else {
                    None
                },
                tool_calls: msg.tool_call.clone(),
                tool_call_id: None,
                reasoning_content,
            }
        })
        .collect::<Vec<_>>()
}

fn archive_used_memories_block(memories: &[MemoryEntry], recall_table: &[String]) -> String {
    if recall_table.is_empty() {
        return "（无）".to_string();
    }
    let mut seen = HashSet::<String>::new();
    let memory_map = memories
        .iter()
        .map(|m| (m.id.clone(), m))
        .collect::<HashMap<String, &MemoryEntry>>();
    let mut lines = Vec::<String>::new();
    for memory_id in recall_table
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
    {
        if !seen.insert(memory_id.to_string()) {
            continue;
        }
        if let Some(memory) = memory_map.get(memory_id) {
            lines.push(format!(
                "<{}>\n判断：{}\n理由：{}\n</{}>",
                memory_id,
                clean_text(memory.judgment.trim()),
                clean_text(memory.reasoning.trim()),
                memory_id
            ));
        } else {
            lines.push(format!("<{}>\n判断：\n理由：\n</{}>", memory_id, memory_id));
        }
    }
    if lines.is_empty() {
        "（无）".to_string()
    } else {
        lines.join("\n")
    }
}

fn archive_example_output_block() -> &'static str {
    r#"{
  "summary": "string",
  "usefulMemoryIds": ["string"],
  "newMemories": [
    {
      "memoryType": "knowledge|skill|emotion|event",
      "judgment": "string",
      "reasoning": "string",
      "tags": ["string"]
    }
  ],
  "mergeGroups": [
    {
      "sourceIds": ["string", "string"],
      "target": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    }
  ]
}"#
}

async fn summarize_archived_conversation_with_model_v2(
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    user_alias: &str,
    source_conversation: &Conversation,
    memories: &[MemoryEntry],
    recall_table: &[String],
    trace_tag: &str,
    trace_id: &str,
) -> Result<ArchiveSummaryDraft, String> {
    let history_messages = build_archive_history_messages(source_conversation);
    let used_memories = archive_used_memories_block(memories, recall_table);

    let instruction = format!(
        "你要做归档总结。输出严格 JSON，不要 markdown，不要代码块。\n\
         ## 强制要求（MUST）\n\
         A) reasoning 必须写“支撑该 judgment 的论据/证据”，不得写流程话术。\n\
         B) reasoning 只允许描述对话中可追溯的依据，不得写“为了归档/为了生成记忆”。\n\
         C) reasoning 应尽量简洁具体；若没有可靠理由或证据不足，可留空字符串。\n\
         D) judgment 必须能被 reasoning 支撑；若无法支撑，宁可不生成该条记忆。\n\
         E) tags/judgment/reasoning 必须使用当前用户本轮语言（专有名词除外）。\n\
         \n\
         规则:\n\
         1) summary 必填，必须按时间顺序写，语言自然、具体，不要模板化空话。\n\
         2) summary 必须覆盖并按此顺序组织：论题（讨论了什么）-> 经过（关键分歧/变化）-> 结论（已决定事项）。\n\
         3) summary 必须明确写出：最新的话题、用户最后的意图、接下来应该怎么做（可执行下一步）。\n\
         4) summary 必须单独明确两部分：悬而未定的论题；接下来建议决策（给出可执行的下一步）。\n\
         5) 如有多个论题，必须逐个输出（按时间先后分别写清每个论题的经过与结论），禁止合并成笼统描述。\n\
         6) summary 必须保留可追溯锚点：关键对象、关键时间点、关键数字或约束条件；不确定就写“待确认”，禁止猜测。\n\
         7) newMemories 最多 7 条；非必要不生成；memoryType 只能是 knowledge/skill/emotion/event（禁止 task）。\n\
         8) usefulMemoryIds 只能从“本次会话使用过的记忆”中选择。\n\
         9) mergeGroups 不是必须，默认输出 []；仅当语义等价或高度重复且合并后不丢信息时才允许填写。\n\
         10) mergeGroups.sourceIds 只能从“本次会话使用过的记忆”中选择，且每组至少 2 个；不确定时必须保持 []。\n\
         11) newMemories 中的 judgment/reasoning/tags 必须使用当前用户本轮使用的语言，禁止夹杂其他语言。\n\
         12) reasoning 定义：给出“支撑该 judgment 的论据/证据”；若没有可靠理由可以留空。\n\
         13) 不要记录高风险敏感信息（密码、密钥、身份证、银行卡等）。\n\
         14) 你是 {assistant_name}，用户称谓是 {user_name}。",
        assistant_name = agent.name,
        user_name = user_alias
    );

    let prepared = PreparedPrompt {
        // Keep system/preamble stable; put archive constraints in the last user message.
        preamble: "你是一个严格遵循用户指令的助手。".to_string(),
        history_messages,
        latest_user_text: format!(
            "<压缩上下文的提示>\n{}\n</压缩上下文的提示>\n\n<本次会话使用过的记忆>\n{}\n</本次会话使用过的记忆>\n\n<示例输出>\n{}\n</示例输出>",
            instruction,
            used_memories,
            archive_example_output_block()
        ),
        latest_user_time_text: String::new(),
        latest_user_system_text: String::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };
    let timeout_secs = 360u64;
    archive_log_request(
        trace_tag,
        trace_id,
        "structured",
        resolved_api,
        selected_api,
        &prepared,
        timeout_secs,
    );

    let call_started = std::time::Instant::now();
    let reply = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        async {
            match resolved_api.request_format {
                RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
                    call_model_openai_rig_style(resolved_api, &selected_api.model, prepared).await
                }
                RequestFormat::Gemini => {
                    call_model_gemini_rig_style(resolved_api, &selected_api.model, prepared).await
                }
                RequestFormat::Anthropic => {
                    call_model_anthropic_rig_style(resolved_api, &selected_api.model, prepared).await
                }
                RequestFormat::OpenAITts
                | RequestFormat::OpenAIStt
                | RequestFormat::GeminiEmbedding
                | RequestFormat::OpenAIEmbedding
                | RequestFormat::OpenAIRerank => Err(format!(
                    "Request format '{}' is not supported for archive summary.",
                    resolved_api.request_format
                )),
            }
        },
    )
    .await
    .map_err(|_| {
        format!(
            "Archive summary request timed out (elapsed={}ms, timeout={}s)",
            call_started.elapsed().as_millis(),
            timeout_secs
        )
    })??;
    archive_log_response(
        trace_tag,
        trace_id,
        "structured",
        call_started.elapsed().as_millis(),
        &reply.assistant_text,
    );
    let parsed = parse_archive_summary_draft(&reply.assistant_text).ok_or_else(|| {
        format!(
            "Parse archive summary JSON failed. raw={}",
            reply.assistant_text.chars().take(240).collect::<String>()
        )
    })?;
    let summary = clean_text(parsed.summary.trim());
    if summary.is_empty() {
        return Err("Archive summary is empty".to_string());
    }
    Ok(ArchiveSummaryDraft {
        summary,
        useful_memory_ids: parsed
            .useful_memory_ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>(),
        new_memories: parsed.new_memories.into_iter().take(7).collect::<Vec<_>>(),
        merge_groups: parsed.merge_groups.into_iter().take(7).collect::<Vec<_>>(),
    })
}

fn archive_message_agent_hint(message: &ChatMessage) -> Option<String> {
    let meta = message.provider_meta.as_ref()?;
    let obj = meta.as_object()?;
    for key in ["agentId", "agent_id", "speakerAgentId", "speaker_agent_id"] {
        let value = obj.get(key).and_then(Value::as_str).unwrap_or("").trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn choose_archive_host_agent_id(data: &AppData, source: &Conversation, fallback_agent_id: &str) -> String {
    let fallback = fallback_agent_id.trim();
    if !fallback.is_empty()
        && data
            .agents
            .iter()
            .any(|a| !a.is_built_in_user && a.id == fallback)
    {
        return fallback.to_string();
    }

    let mut count_map = HashMap::<String, usize>::new();
    let mut last_idx_map = HashMap::<String, usize>::new();
    for (idx, message) in source.messages.iter().enumerate() {
        let hint = archive_message_agent_hint(message)
            .or_else(|| {
                if message.role == "assistant" {
                    Some(source.agent_id.clone())
                } else {
                    None
                }
            });
        let Some(agent_id) = hint else {
            continue;
        };
        *count_map.entry(agent_id.clone()).or_insert(0) += 1;
        last_idx_map.insert(agent_id, idx);
    }

    let public_agents = data
        .agents
        .iter()
        .filter(|a| !a.is_built_in_user && !a.private_memory_enabled)
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    if !public_agents.is_empty() {
        return public_agents
            .into_iter()
            .max_by(|a, b| {
                let ac = count_map.get(a).copied().unwrap_or(0);
                let bc = count_map.get(b).copied().unwrap_or(0);
                ac.cmp(&bc)
                    .then_with(|| {
                        let ai = last_idx_map.get(a).copied().unwrap_or(0);
                        let bi = last_idx_map.get(b).copied().unwrap_or(0);
                        ai.cmp(&bi)
                    })
                    .then_with(|| b.cmp(a))
            })
            .unwrap_or_else(|| source.agent_id.clone());
    }

    source.agent_id.clone()
}

#[cfg(test)]
mod archive_host_selection_tests {
    use super::*;

    fn mk_agent(id: &str, private_memory_enabled: bool) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: String::new(),
            created_at: now_iso(),
            updated_at: now_iso(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            private_memory_enabled,
        }
    }

    fn mk_msg(role: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some("test-agent".to_string()),
            parts: vec![MessagePart::Text {
                text: "x".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    #[test]
    fn host_should_prefer_fallback_agent_when_valid() {
        let data = AppData {
            version: APP_DATA_SCHEMA_VERSION,
            agents: vec![mk_agent("pub-a", false), mk_agent("pub-b", false)],
            selected_agent_id: "pub-b".to_string(),
            user_alias: "u".to_string(),
            response_style_id: "concise".to_string(),
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
            image_text_cache: Vec::new(),
        };
        let source = Conversation {
            id: "c1".to_string(),
            title: "t".to_string(),
            api_config_id: "api".to_string(),
            agent_id: "pub-a".to_string(),
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            status: "active".to_string(),
            summary: String::new(),
            archived_at: None,
            messages: vec![mk_msg("assistant"), mk_msg("assistant"), mk_msg("assistant")],
            memory_recall_table: Vec::new(),
        };
        let host = choose_archive_host_agent_id(&data, &source, "pub-b");
        assert_eq!(host, "pub-b");
    }

    #[test]
    fn host_should_fallback_to_selected_when_all_private() {
        let data = AppData {
            version: APP_DATA_SCHEMA_VERSION,
            agents: vec![mk_agent("p1", true), mk_agent("p2", true)],
            selected_agent_id: "p2".to_string(),
            user_alias: "u".to_string(),
            response_style_id: "concise".to_string(),
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
            image_text_cache: Vec::new(),
        };
        let source = Conversation {
            id: "c1".to_string(),
            title: "t".to_string(),
            api_config_id: "api".to_string(),
            agent_id: "p1".to_string(),
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            status: "active".to_string(),
            summary: String::new(),
            archived_at: None,
            messages: vec![mk_msg("assistant")],
            memory_recall_table: Vec::new(),
        };
        let host = choose_archive_host_agent_id(&data, &source, "p2");
        assert_eq!(host, "p2");
    }
}

#[tauri::command]
async fn force_archive_current(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceArchiveResult, String> {
    let (selected_api, resolved_api, source, effective_agent_id) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let app_config = read_config(&state.config_path)?;
        let selected_api = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let requested_agent_id = input.agent_id.trim();
        let effective_agent_id = if data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
        {
            requested_agent_id.to_string()
        } else if data
            .agents
            .iter()
            .any(|a| a.id == data.selected_agent_id && !a.is_built_in_user)
        {
            data.selected_agent_id.clone()
        } else {
            data.agents
                .iter()
                .find(|a| !a.is_built_in_user)
                .map(|a| a.id.clone())
                .ok_or_else(|| "Selected agent not found.".to_string())?
        };
        let source_idx = latest_active_conversation_index(&data, &selected_api.id, &effective_agent_id)
            .ok_or_else(|| "当前没有可归档的活动对话。".to_string())?;
        let source = data
            .conversations
            .get(source_idx)
            .cloned()
            .ok_or_else(|| "当前没有可归档的活动对话。".to_string())?;
        drop(guard);
        (selected_api, resolved_api, source, effective_agent_id)
    };

    run_archive_pipeline(
        &state,
        &selected_api,
        &resolved_api,
        &source,
        &effective_agent_id,
        "manual_force_archive",
        "ARCHIVE-FORCE",
    )
    .await
}

pub(crate) async fn run_archive_pipeline(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    archive_reason: &str,
    trace_tag: &str,
) -> Result<ForceArchiveResult, String> {
    let started_at = std::time::Instant::now();
    let trace_id = Uuid::new_v4().to_string();

    if source.messages.is_empty() {
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            summary: "当前对话为空，无需归档。".to_string(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("empty_conversation".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let (host_agent, host_agent_id, user_alias, memories) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let user_alias = data.user_alias.clone();
        let host_agent_id = choose_archive_host_agent_id(&data, source, effective_agent_id);
        let host_agent = data
            .agents
            .iter()
            .find(|a| a.id == host_agent_id)
            .cloned()
            .ok_or_else(|| "Host agent not found.".to_string())?;
        let host_private_memory_enabled = host_agent.private_memory_enabled;
        drop(guard);
        let memories = memory_store_list_memories_visible_for_agent(
            &state.data_path,
            &host_agent_id,
            host_private_memory_enabled,
        )?;
        (host_agent, host_agent_id, user_alias, memories)
    };

    eprintln!(
        "[{}] trace={} begin api={} model={} format={} conversation={} hostAgent={}",
        trace_tag,
        trace_id,
        selected_api.id,
        selected_api.model,
        resolved_api.request_format,
        source.id,
        host_agent_id
    );

    let parsed = summarize_archived_conversation_with_model_v2(
        resolved_api,
        selected_api,
        &host_agent,
        &user_alias,
        source,
        &memories,
        &source.memory_recall_table,
        trace_tag,
        &trace_id,
    )
    .await?;
    let summary = parsed.summary;
    let useful_memory_ids = parsed.useful_memory_ids;
    let summary_memories = parsed.new_memories;
    let merge_groups = parsed.merge_groups;

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    let archive_id = archive_conversation_now(&mut data, &source.id, archive_reason, &summary)
        .ok_or_else(|| "活动对话已变化，请重试归档。".to_string())?;
    let _ = ensure_active_conversation_index(&mut data, &selected_api.id, &source.agent_id);
    let owner_agent_id = data
        .agents
        .iter()
        .find(|a| a.id == host_agent_id && !a.is_built_in_user && a.private_memory_enabled)
        .map(|a| a.id.as_str());
    let memory_feedback = memory_store_apply_archive_feedback(
        &state.data_path,
        &source.memory_recall_table,
        &useful_memory_ids,
    )?;
    let merged_memories = merge_memories_into_store(&state.data_path, &summary_memories, owner_agent_id)?;
    let merged_groups = merge_memory_groups_into_store(&state.data_path, &merge_groups, owner_agent_id)?;
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    eprintln!(
        "[{}] trace={} done archived=true merged_memories={} merged_groups={} useful_accept={} penalized={} natural_decay={}",
        trace_tag,
        trace_id,
        merged_memories,
        merged_groups,
        memory_feedback.useful_accepted_count,
        memory_feedback.penalized_count,
        memory_feedback.natural_decay_count,
    );

    Ok(ForceArchiveResult {
        archived: true,
        archive_id: Some(archive_id),
        summary,
        merged_memories,
        warning: None,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback: Some(memory_feedback),
        merge_groups: Some(merged_groups),
    })
}
