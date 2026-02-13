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

    let conversation = data
        .conversations
        .iter()
        .rfind(|c| c.status == "active" && c.agent_id == effective_agent_id)
        .cloned()
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
            messages: Vec::new(),
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
    );
    let last_archive_summary = data
        .archived_conversations
        .iter()
        .rev()
        .find(|a| {
            a.source_conversation.agent_id == effective_agent_id && !a.summary.trim().is_empty()
        })
        .map(|a| a.summary.clone());
    if let Some(summary) = last_archive_summary {
        prepared.preamble.push_str(
            "\n[HIDDEN ARCHIVE RECAP]\nUSER: 上次我们聊到哪里？\nASSISTANT: ",
        );
        prepared.preamble.push_str(summary.trim());
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

#[tauri::command]
fn list_archives(state: State<'_, AppState>) -> Result<Vec<ArchiveSummary>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let mut summaries = data
        .archived_conversations
        .iter()
        .map(|archive| ArchiveSummary {
            archive_id: archive.archive_id.clone(),
            archived_at: archive.archived_at.clone(),
            title: format!(
                "{} - {}",
                archive_time_label(&archive.archived_at),
                archive_first_user_preview(&archive.source_conversation)
            ),
            message_count: archive.source_conversation.messages.len(),
            api_config_id: archive.source_conversation.api_config_id.clone(),
            agent_id: archive.source_conversation.agent_id.clone(),
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
        .archived_conversations
        .iter()
        .find(|a| a.archive_id == archive_id)
        .ok_or_else(|| "Archive not found".to_string())?;

    Ok(archive.source_conversation.messages.clone())
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
    let before = data.archived_conversations.len();
    data.archived_conversations
        .retain(|a| a.archive_id != archive_id);

    if data.archived_conversations.len() == before {
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
        .archived_conversations
        .iter()
        .find(|a| a.archive_id == input.archive_id)
        .cloned()
        .ok_or_else(|| "Archive not found".to_string())?;

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
fn list_memories(state: State<'_, AppState>) -> Result<Vec<MemoryEntry>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let mut memories = data.memories.clone();
    memories.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(memories)
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
struct ExportMemoriesFileResult {
    path: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMemoriesToPathInput {
    path: String,
}

fn memory_content_key(content: &str) -> String {
    clean_text(content.trim()).to_lowercase()
}

#[tauri::command]
fn export_memories(state: State<'_, AppState>) -> Result<MemoryExportPayload, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let mut memories = data.memories;
    memories.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

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
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let data = read_app_data(&state.data_path)?;
    drop(guard);
    let mut memories = data.memories;
    memories.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
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

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let data = read_app_data(&state.data_path)?;
    drop(guard);
    let mut memories = data.memories;
    memories.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
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
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&state.data_path)?;

    let now = now_iso();
    let mut created_count = 0usize;
    let mut merged_count = 0usize;
    let mut imported_count = 0usize;

    let mut key_index = std::collections::HashMap::<String, usize>::new();
    for (idx, m) in data.memories.iter().enumerate() {
        let key = memory_content_key(&m.content);
        if !key.is_empty() {
            key_index.insert(key, idx);
        }
    }

    for incoming in input.memories {
        let content = clean_text(incoming.content.trim());
        if content.is_empty() {
            continue;
        }
        let keywords = normalize_memory_keywords(&incoming.keywords);
        if keywords.is_empty() {
            continue;
        }

        imported_count += 1;
        let key = memory_content_key(&content);
        if let Some(idx) = key_index.get(&key).copied() {
            let existing = &mut data.memories[idx];
            for kw in keywords {
                if !existing.keywords.iter().any(|x| x == &kw) {
                    existing.keywords.push(kw);
                }
            }
            existing.updated_at = now.clone();
            merged_count += 1;
            continue;
        }

        let id = if incoming.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            incoming.id
        };
        data.memories.push(MemoryEntry {
            id,
            content: content.clone(),
            keywords,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        key_index.insert(key, data.memories.len() - 1);
        created_count += 1;
    }

    write_app_data(&state.data_path, &data)?;
    invalidate_memory_matcher_cache();
    drop(guard);

    Ok(ImportMemoriesResult {
        imported_count,
        created_count,
        merged_count,
        total_count: data.memories.len(),
    })
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
    content: String,
    keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchiveSummaryDraft {
    summary: String,
    #[serde(default)]
    memories: Vec<ArchiveMemoryDraft>,
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

fn merge_memories_into_app_data(data: &mut AppData, drafts: &[ArchiveMemoryDraft]) -> usize {
    let now = now_iso();
    let mut merged = 0usize;
    for d in drafts {
        let content = clean_text(d.content.trim());
        if content.is_empty() {
            continue;
        }
        let keywords = normalize_memory_keywords(&d.keywords);
        if keywords.is_empty() {
            continue;
        }
        if memory_contains_sensitive(&content, &keywords) {
            continue;
        }
        let mut merged_existing = false;
        for existing in &mut data.memories {
            if memory_content_key(&existing.content) == memory_content_key(&content) {
                for kw in &keywords {
                    if !existing.keywords.iter().any(|x| x == kw) {
                        existing.keywords.push(kw.clone());
                    }
                }
                existing.updated_at = now.clone();
                merged_existing = true;
                merged += 1;
                break;
            }
        }
        if merged_existing {
            continue;
        }
        data.memories.push(MemoryEntry {
            id: Uuid::new_v4().to_string(),
            content,
            keywords,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        merged += 1;
    }
    merged
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceArchiveResult {
    archived: bool,
    archive_id: Option<String>,
    summary: String,
    merged_memories: usize,
}

async fn summarize_archived_conversation_with_model(
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    user_alias: &str,
    source_conversation: &Conversation,
    memories: &[MemoryEntry],
) -> Result<(String, Vec<ArchiveMemoryDraft>), String> {
    let mut transcript = String::new();
    for msg in &source_conversation.messages {
        transcript.push_str(&render_message_for_context(msg));
        transcript.push('\n');
    }
    let search_text = conversation_search_text(source_conversation);
    let memory_board_xml = build_memory_board_xml(memories, &search_text, "");
    let extra_memory = memory_board_xml
        .map(|xml| format!("\n\n[MEMORY BOARD]\n{xml}"))
        .unwrap_or_default();

    let summary_tool_rules = if selected_api.enable_tools && tool_enabled(selected_api, "memory-save")
    {
        "工具规则：仅允许 memory-save，最多 3 次；达到上限后必须立刻输出 summary，不得继续工具调用。"
    } else {
        "工具规则：当前模型或配置不支持工具调用，禁止调用任何工具。"
    };

    let instruction = format!(
        "你要做归档总结。输出严格 JSON，不要 markdown，不要代码块。\n\
         JSON schema: {{\"summary\":\"string\",\"memories\":[{{\"content\":\"string\",\"keywords\":[\"string\"]}}]}}\n\
         规则:\n\
         1) summary 必填，简洁说明这轮对话的目标、结论、待办。\n\
         2) memories 最多 7 条；非必要不生成；仅保留对用户长期有价值的信息。\n\
         3) 不要记录高风险敏感信息（密码、密钥、身份证、银行卡等）。\n\
         4) 你是 {assistant_name}，用户称谓是 {user_name}。\n\
         5) {tool_rules}",
        assistant_name = agent.name,
        user_name = user_alias,
        tool_rules = summary_tool_rules
    );

    let prepared = PreparedPrompt {
        preamble: format!("[ARCHIVE TASK]\n{instruction}"),
        history_messages: Vec::new(),
        latest_user_text: format!(
            "[CONVERSATION]\n{}\n{}",
            transcript.trim(),
            extra_memory.trim()
        ),
        latest_user_time_text: String::new(),
        latest_user_system_text: String::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };

    let reply = call_model_openai_rig_style(resolved_api, &selected_api.model, prepared).await?;
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
    let memories = parsed.memories.into_iter().take(7).collect::<Vec<_>>();
    Ok((summary, memories))
}

#[tauri::command]
async fn force_archive_current(
    _input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceArchiveResult, String> {
    let (selected_api, resolved_api, source, agent, user_alias, memories) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let app_config = read_config(&state.config_path)?;
        let selected_api = resolve_selected_api_config(&app_config, None)
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
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
        let user_alias = data.user_alias.clone();
        let memories = data.memories.clone();
        let source_idx = latest_active_conversation_index(&data, &effective_agent_id)
            .ok_or_else(|| "当前没有可归档的活动对话。".to_string())?;
        let source = data
            .conversations
            .get(source_idx)
            .cloned()
            .ok_or_else(|| "当前没有可归档的活动对话。".to_string())?;
        drop(guard);
        (selected_api, resolved_api, source, agent, user_alias, memories)
    };

    if source.messages.is_empty() {
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            summary: "当前对话为空，无需归档。".to_string(),
            merged_memories: 0,
        });
    }

    let (summary, summary_memories) = summarize_archived_conversation_with_model(
        &resolved_api,
        &selected_api,
        &agent,
        &user_alias,
        &source,
        &memories,
    )
    .await?;

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    let archive_id = archive_conversation_now(&mut data, &source.id, "manual_force_archive", &summary);
    if archive_id.is_none() {
        drop(guard);
        return Err("活动对话已变化，请重试强制归档。".to_string());
    }
    let _ = ensure_active_conversation_index(
        &mut data,
        &selected_api.id,
        &source.agent_id,
    );
    let merged_memories = merge_memories_into_app_data(&mut data, &summary_memories);
    if merged_memories > 0 {
        invalidate_memory_matcher_cache();
    }
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ForceArchiveResult {
        archived: true,
        archive_id,
        summary,
        merged_memories,
    })
}

