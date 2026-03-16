#[tauri::command]
fn get_prompt_preview(
    _input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<PromptPreview, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    let _ = ensure_default_agent(&mut data);
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut data)?;
    let effective_agent_id = if data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
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
            agent_id: effective_agent_id.clone(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: "active".to_string(),
            summary: String::new(),
            archived_at: None,
            messages: Vec::new(),
            memory_recall_table: Vec::new(),
        });

    let user_name = user_persona_name(&data);
    let user_intro = user_persona_intro(&data);
    let last_archive_summary = data
        .conversations
        .iter()
        .rev()
        .find(|c| c.agent_id == effective_agent_id && !c.summary.trim().is_empty())
        .map(|c| c.summary.clone());
    let prepared = build_prepared_prompt_for_mode(
        PromptBuildMode::Chat,
        &conversation,
        &agent,
        &data.agents,
        &app_config.departments,
        &user_name,
        &user_intro,
        &data.response_style_id,
        &app_config.ui_language,
        Some(&state.data_path),
        last_archive_summary.as_deref(),
        terminal_prompt_trusted_roots_block(&state, &api_config),
        Some(ChatPromptOverrides {
            system_preamble_blocks: vec![
                build_hidden_skill_snapshot_block(&state),
                build_hidden_skill_usage_block(),
            ],
            ..Default::default()
        }),
    );
    let mut user_content = Vec::<Value>::new();
    for text_block in prepared_prompt_latest_user_text_blocks(&prepared) {
        user_content.push(serde_json::json!({
            "type": "text",
            "text": text_block,
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

fn archive_no_content_label(ui_language: &str) -> String {
    match ui_language.trim() {
        "en-US" => "No content".to_string(),
        "zh-TW" => "無內容".to_string(),
        _ => "无内容".to_string(),
    }
}

fn archive_first_user_preview(conversation: &Conversation, ui_language: &str) -> String {
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
        archive_no_content_label(ui_language)
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
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let data = state_read_app_data_cached(&state)?;
    let app_config = read_config(&state.config_path)?;
    drop(guard);

    let mut summaries = data
        .conversations
        .iter()
        .filter(|c| !c.summary.trim().is_empty())
        .map(|archive| {
            let api_config_id = department_for_agent_id(&app_config, &archive.agent_id)
                .map(department_primary_api_config_id)
                .unwrap_or_default();
            ArchiveSummary {
                archive_id: archive.id.clone(),
                archived_at: archive
                    .archived_at
                    .clone()
                    .unwrap_or_else(|| archive.updated_at.clone()),
                title: archive_first_user_preview(archive, &app_config.ui_language),
                message_count: archive.messages.len(),
                api_config_id,
                agent_id: archive.agent_id.clone(),
            }
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
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let data = state_read_app_data_cached(&state)?;
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
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let data = state_read_app_data_cached(&state)?;
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
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = state_read_app_data_cached(&state)?;
    let before = data.conversations.len();
    data.conversations
        .retain(|c| !(c.id == archive_id && !c.summary.trim().is_empty()));

    if data.conversations.len() == before {
        drop(guard);
        return Err("Archive not found".to_string());
    }

    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(())
}

