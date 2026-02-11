#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut result = read_config(&state.config_path)?;
    normalize_app_config(&mut result);
    drop(guard);
    Ok(result)
}

#[tauri::command]
fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<AppConfig, String> {
    if config.api_configs.is_empty() {
        return Err("At least one API config must be configured.".to_string());
    }
    let mut config = config;
    normalize_app_config(&mut config);

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    write_config(&state.config_path, &config)?;
    drop(guard);
    Ok(config)
}

#[tauri::command]
fn load_agents(state: State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(data.agents)
}

#[tauri::command]
fn save_agents(
    input: SaveAgentsInput,
    state: State<'_, AppState>,
) -> Result<Vec<AgentProfile>, String> {
    if input.agents.is_empty() {
        return Err("At least one agent is required.".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    data.agents = input.agents;
    ensure_default_agent(&mut data);
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(data.agents)
}

#[tauri::command]
fn load_chat_settings(state: State<'_, AppState>) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ChatSettings {
        selected_agent_id: data.selected_agent_id,
        user_alias: data.user_alias,
    })
}

#[tauri::command]
fn save_chat_settings(
    input: ChatSettings,
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data.agents.iter().any(|a| a.id == input.selected_agent_id) {
        return Err("Selected agent not found.".to_string());
    }
    data.selected_agent_id = input.selected_agent_id.clone();
    data.user_alias = if input.user_alias.trim().is_empty() {
        default_user_alias()
    } else {
        input.user_alias.trim().to_string()
    };
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(input)
}

#[tauri::command]
fn save_conversation_api_settings(
    input: ConversationApiSettings,
    state: State<'_, AppState>,
) -> Result<ConversationApiSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut config = read_config(&state.config_path)?;
    config.chat_api_config_id = input.chat_api_config_id.clone();
    config.vision_api_config_id = input.vision_api_config_id.clone();
    normalize_app_config(&mut config);
    write_config(&state.config_path, &config)?;
    drop(guard);

    Ok(ConversationApiSettings {
        chat_api_config_id: config.chat_api_config_id,
        vision_api_config_id: config.vision_api_config_id,
    })
}

#[tauri::command]
fn get_chat_snapshot(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data.agents.iter().any(|a| a.id == input.agent_id) {
        return Err("Selected agent not found.".to_string());
    }

    let idx = ensure_active_conversation_index(&mut data, &api_config.id, &input.agent_id);
    let conversation = &data.conversations[idx];

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();
    let latest_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .cloned();

    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ChatSnapshot {
        conversation_id: conversation.id.clone(),
        latest_user,
        latest_assistant,
        active_message_count: conversation.messages.len(),
    })
}

#[tauri::command]
fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    let idx = ensure_active_conversation_index(&mut data, &api_config.id, &input.agent_id);
    let messages = data.conversations[idx].messages.clone();

    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(messages)
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
            title: archive.source_conversation.title.clone(),
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

fn conversation_search_text(conversation: &Conversation) -> String {
    let mut lines = Vec::<String>::new();
    for msg in &conversation.messages {
        for part in &msg.parts {
            if let MessagePart::Text { text } = part {
                if !text.trim().is_empty() {
                    lines.push(text.to_lowercase());
                }
            }
        }
    }
    lines.join("\n")
}

fn build_memory_board_xml(
    memories: &[MemoryEntry],
    search_text: &str,
    latest_user_text: &str,
) -> Option<String> {
    let mut hits = Vec::<(&MemoryEntry, Vec<String>)>::new();
    let mut corpus = String::new();
    corpus.push_str(search_text);
    if !latest_user_text.trim().is_empty() {
        corpus.push('\n');
        corpus.push_str(&latest_user_text.to_lowercase());
    }

    for memory in memories {
        let mut matched = Vec::<String>::new();
        for kw in &memory.keywords {
            let k = kw.trim().to_lowercase();
            if k.len() < 2 {
                continue;
            }
            if corpus.contains(&k) {
                matched.push(k);
            }
        }
        if !matched.is_empty() {
            hits.push((memory, matched));
        }
        if hits.len() >= 4 {
            break;
        }
    }

    if hits.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str("<memory_board>\n");
    out.push_str("  <note>这是记忆提示板，请按需参考，不要编造未命中的记忆。</note>\n");
    out.push_str("  <memories>\n");
    for (memory, matched) in hits {
        out.push_str(&format!("    <memory id=\"{}\">\n", xml_escape(&memory.id)));
        out.push_str(&format!(
            "      <keywords>{}</keywords>\n",
            xml_escape(&memory.keywords.join(","))
        ));
        out.push_str(&format!(
            "      <content>{}</content>\n",
            xml_escape(&memory.content)
        ));
        out.push_str(&format!(
            "      <reason>命中关键词: {}</reason>\n",
            xml_escape(&matched.join(","))
        ));
        out.push_str("    </memory>\n");
    }
    out.push_str("  </memories>\n");
    out.push_str("</memory_board>");
    Some(out)
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

    let instruction = format!(
        "你要做归档总结。输出严格 JSON，不要 markdown，不要代码块。\n\
         JSON schema: {{\"summary\":\"string\",\"memories\":[{{\"content\":\"string\",\"keywords\":[\"string\"]}}]}}\n\
         规则:\n\
         1) summary 必填，简洁说明这轮对话的目标、结论、待办。\n\
         2) memories 最多 7 条；非必要不生成；仅保留对用户长期有价值的信息。\n\
         3) 不要记录高风险敏感信息（密码、密钥、身份证、银行卡等）。\n\
         4) 你是 {assistant_name}，用户称谓是 {user_name}。",
        assistant_name = agent.name,
        user_name = user_alias
    );

    let prepared = PreparedPrompt {
        preamble: format!("[ARCHIVE TASK]\n{instruction}"),
        latest_user_text: format!(
            "[CONVERSATION]\n{}\n{}",
            transcript.trim(),
            extra_memory.trim()
        ),
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
async fn send_chat_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    let (app_config, selected_api, resolved_api) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let app_config = read_config(&state.config_path)?;
        let selected_api = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        drop(guard);
        (app_config, selected_api, resolved_api)
    };

    if !is_openai_style_request_format(&resolved_api.request_format) {
        return Err(format!(
            "Request format '{}' is not implemented in chat router yet.",
            resolved_api.request_format
        ));
    }

    let mut effective_payload = input.payload.clone();
    let audios = effective_payload.audios.clone().unwrap_or_default();
    if !audios.is_empty() {
        return Err("当前版本仅支持本地语音识别，发送消息不支持语音附件。".to_string());
    }

    if !selected_api.enable_image {
        let images = effective_payload.images.clone().unwrap_or_default();
        if !images.is_empty() {
            let vision_api = resolve_vision_api_config(&app_config).ok();
            if let Some(vision_api) = vision_api {
                let vision_resolved =
                    resolve_api_config(&app_config, Some(vision_api.id.as_str()))?;
                if !is_openai_style_request_format(&vision_resolved.request_format) {
                    return Err(format!(
                        "Vision request format '{}' is not implemented in image conversion router yet.",
                        vision_resolved.request_format
                    ));
                }

                let mut converted_texts = Vec::<String>::new();
                for (idx, image) in images.iter().enumerate() {
                    let hash = compute_image_hash_hex(image)?;
                    let cached = {
                        let guard = state
                            .state_lock
                            .lock()
                            .map_err(|_| "Failed to lock state mutex".to_string())?;
                        let data = read_app_data(&state.data_path)?;
                        drop(guard);
                        find_image_text_cache(&data, &hash, &vision_api.id)
                    };

                    if let Some(text) = cached {
                        let mapped = format!("[图片{}]\n{}", idx + 1, text);
                        converted_texts.push(mapped);
                        continue;
                    }

                    let converted =
                        describe_image_with_vision_api(&vision_resolved, &vision_api, image)
                            .await?;
                    let converted = converted.trim().to_string();
                    if converted.is_empty() {
                        continue;
                    }
                    let mapped = format!("[图片{}]\n{}", idx + 1, converted);
                    converted_texts.push(mapped);

                    let guard = state
                        .state_lock
                        .lock()
                        .map_err(|_| "Failed to lock state mutex".to_string())?;
                    let mut data = read_app_data(&state.data_path)?;
                    upsert_image_text_cache(&mut data, &hash, &vision_api.id, &converted);
                    write_app_data(&state.data_path, &data)?;
                    drop(guard);
                }

                if !converted_texts.is_empty() {
                    let converted_all = converted_texts.join("\n\n");
                    let merged_text = effective_payload
                        .text
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(|text| format!("{text}\n\n{converted_all}"))
                        .unwrap_or(converted_all);
                    effective_payload.text = Some(merged_text);
                }
                effective_payload.images = None;
            } else {
                eprintln!(
                    "[CHAT] Image input filtered out because current chat API does not support image and no vision fallback is configured."
                );
                effective_payload.images = None;
            }
        }
    }

    let effective_user_parts = build_user_parts(&effective_payload, &selected_api)?;
    let effective_user_text = effective_user_parts
        .iter()
        .map(|part| match part {
            MessagePart::Text { text } => text.clone(),
            MessagePart::Image { .. } => "[image]".to_string(),
            MessagePart::Audio { .. } => "[audio]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let effective_images = effective_user_parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Image {
                mime, bytes_base64, ..
            } => Some((mime.clone(), bytes_base64.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let effective_audios = effective_user_parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Audio {
                mime, bytes_base64, ..
            } => Some((mime.clone(), bytes_base64.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut archived_before_send = false;
    let mut pending_archive_source: Option<Conversation> = None;
    let mut pending_archive_reason = String::new();
    let mut pending_archive_forced = false;

    {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let _agent = data
            .agents
            .iter()
            .find(|a| a.id == input.agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;

        if let Some(conversation) = data.conversations.iter_mut().find(|c| {
            c.status == "active" && c.api_config_id == selected_api.id && c.agent_id == input.agent_id
        }) {
            let decision =
                decide_archive_before_user_message(conversation, selected_api.context_window_tokens);
            conversation.last_context_usage_ratio = decision.usage_ratio;
            eprintln!(
                "[ARCHIVE] check before user message: should_archive={}, forced={}, reason={}, usage_ratio={:.4}",
                decision.should_archive, decision.forced, decision.reason, decision.usage_ratio
            );
            if decision.should_archive {
                pending_archive_source = Some(conversation.clone());
                pending_archive_reason = decision.reason.clone();
                pending_archive_forced = decision.forced;
            }
        }
        write_app_data(&state.data_path, &data)?;
        drop(guard);
    }

    if let Some(source) = pending_archive_source {
        if pending_archive_forced {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                tool_name: Some("archive".to_string()),
                tool_status: Some("running".to_string()),
                message: Some("正在归档优化上下文...".to_string()),
            });
        }

        let (summary_result, summary_memories) = {
            let guard = state
                .state_lock
                .lock()
                .map_err(|_| "Failed to lock state mutex".to_string())?;
            let mut data = read_app_data(&state.data_path)?;
            ensure_default_agent(&mut data);
            let agent = data
                .agents
                .iter()
                .find(|a| a.id == input.agent_id)
                .cloned()
                .ok_or_else(|| "Selected agent not found.".to_string())?;
            let user_alias = data.user_alias.clone();
            let memories = data.memories.clone();
            drop(guard);

            match summarize_archived_conversation_with_model(
                &resolved_api,
                &selected_api,
                &agent,
                &user_alias,
                &source,
                &memories,
            )
            .await
            {
                Ok((summary, drafts)) => (Ok(summary), drafts),
                Err(err) => (Err(err), Vec::new()),
            }
        };

        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);

        match summary_result {
            Ok(summary) => {
                if archive_conversation_now(
                    &mut data,
                    &source.id,
                    &pending_archive_reason,
                    &summary,
                )
                .is_some()
                {
                    let memory_merged = merge_memories_into_app_data(&mut data, &summary_memories);
                    eprintln!(
                        "[ARCHIVE] archived ok. conversation_id={}, reason={}, summary_len={}, merged_memories={}",
                        source.id,
                        pending_archive_reason,
                        summary.chars().count(),
                        memory_merged
                    );
                    archived_before_send = true;
                }
            }
            Err(err) => {
                eprintln!(
                    "[ARCHIVE] summary failed, fallback to recent turns. conversation_id={}, err={}",
                    source.id, err
                );
                if let Some(conv) = data
                    .conversations
                    .iter_mut()
                    .find(|c| c.id == source.id && c.status == "active")
                {
                    let mut fallback_messages = keep_recent_turns(&source.messages, 3);
                    conv.messages = fallback_messages.clone();
                    let mut tmp = conv.clone();
                    tmp.messages = fallback_messages.clone();
                    let usage_after = compute_context_usage_ratio(&tmp, selected_api.context_window_tokens);
                    if usage_after >= 0.82 {
                        fallback_messages.clear();
                        conv.messages.clear();
                    }
                    conv.last_user_at = conv
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.role == "user")
                        .map(|m| m.created_at.clone());
                    conv.last_assistant_at = conv
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.role == "assistant")
                        .map(|m| m.created_at.clone());
                    conv.updated_at = now_iso();
                    conv.last_context_usage_ratio = if conv.messages.is_empty() {
                        0.0
                    } else {
                        compute_context_usage_ratio(conv, selected_api.context_window_tokens)
                    };
                }
            }
        }

        write_app_data(&state.data_path, &data)?;
        drop(guard);

        if pending_archive_forced {
            let status = if archived_before_send { "done" } else { "failed" };
            let message = if archived_before_send {
                "归档完成，已优化上下文。"
            } else {
                "归档失败，已自动回退为最近三轮或新对话。"
            };
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                tool_name: Some("archive".to_string()),
                tool_status: Some(status.to_string()),
                message: Some(message.to_string()),
            });
        }
    }

    let (model_name, prepared_prompt, conversation_id, latest_user_text) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;

        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let agent = data
            .agents
            .iter()
            .find(|a| a.id == input.agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;

        let idx = ensure_active_conversation_index(&mut data, &selected_api.id, &input.agent_id);

        // 聊天记录保留用户原始多模态内容；模型请求使用 effective_payload（可能已做图转文）。
        let mut storage_api = selected_api.clone();
        storage_api.enable_image = true;
        storage_api.enable_audio = true;
        let user_parts = build_user_parts(&input.payload, &storage_api)?;
        let conversation_before = data.conversations[idx].clone();
        let search_text = conversation_search_text(&conversation_before);
        let memory_board_xml =
            build_memory_board_xml(&data.memories, &search_text, &effective_user_text);
        let last_archive_summary = data
            .archived_conversations
            .iter()
            .rev()
            .find(|a| {
                a.source_conversation.api_config_id == selected_api.id
                    && a.source_conversation.agent_id == input.agent_id
                    && !a.summary.trim().is_empty()
            })
            .map(|a| a.summary.clone());

        let latest_user_text = if let Some(xml) = &memory_board_xml {
            if effective_user_text.trim().is_empty() {
                xml.clone()
            } else {
                format!("{effective_user_text}\n\n{xml}")
            }
        } else {
            effective_user_text.clone()
        };

        let now = now_iso();
        let mut user_parts_for_storage = user_parts;
        if let Some(xml) = &memory_board_xml {
            user_parts_for_storage.push(MessagePart::Text { text: xml.clone() });
        }

        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            parts: user_parts_for_storage,
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        };

        data.conversations[idx].messages.push(user_message);
        data.conversations[idx].updated_at = now;
        data.conversations[idx].last_user_at = Some(now_iso());
        data.conversations[idx].last_context_usage_ratio =
            compute_context_usage_ratio(&data.conversations[idx], selected_api.context_window_tokens);

        let conversation = data.conversations[idx].clone();
        let mut prepared = build_prompt(&conversation, &agent, &data.user_alias, &now_iso());
        if let Some(summary) = last_archive_summary {
            prepared.preamble.push_str(
                "\n[HIDDEN ARCHIVE RECAP]\nUSER: 上次我们聊到哪里？\nASSISTANT: ",
            );
            prepared.preamble.push_str(summary.trim());
            prepared.preamble.push('\n');
        }
        prepared.latest_user_text = latest_user_text.clone();
        prepared.latest_images = effective_images.clone();
        prepared.latest_audios = effective_audios.clone();

        let model_name = input
            .payload
            .model
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| resolved_api.model.clone());
        let conversation_id = conversation.id.clone();

        write_app_data(&state.data_path, &data)?;
        drop(guard);

        (
            model_name,
            prepared,
            conversation_id,
            latest_user_text,
        )
    };

    let model_reply = call_model_openai_style(
        &resolved_api,
        &selected_api,
        &model_name,
        prepared_prompt,
        Some(&state),
        &on_delta,
        app_config.tool_max_iterations as usize,
    )
    .await?;
    let assistant_text = model_reply.assistant_text;
    let reasoning_standard = model_reply.reasoning_standard;
    let reasoning_inline = model_reply.reasoning_inline;

    let mut assistant_text_for_storage = assistant_text.clone();
    if !reasoning_standard.trim().is_empty() {
        assistant_text_for_storage.push_str("\n\n[标准思考]\n");
        assistant_text_for_storage.push_str(reasoning_standard.trim());
    }

    {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;

        let mut data = read_app_data(&state.data_path)?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id && c.status == "active")
        {
            let now = now_iso();
            conversation.messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                parts: vec![MessagePart::Text {
                    text: assistant_text_for_storage.clone(),
                }],
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            });
            conversation.updated_at = now.clone();
            conversation.last_assistant_at = Some(now);
            conversation.last_context_usage_ratio =
                compute_context_usage_ratio(conversation, selected_api.context_window_tokens);
            write_app_data(&state.data_path, &data)?;
        }
        drop(guard);
    }

    Ok(SendChatResult {
        conversation_id,
        latest_user_text,
        assistant_text,
        reasoning_standard,
        reasoning_inline,
        archived_before_send,
    })
}

async fn fetch_models_openai(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let auth = format!("Bearer {}", input.api_key.trim());
    let auth_value = HeaderValue::from_str(&auth)
        .map_err(|err| format!("Build authorization header failed: {err}"))?;
    headers.insert(AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .default_headers(headers)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    let mut candidate_urls = vec![format!("{base}/models")];
    if !base.to_ascii_lowercase().contains("/v1") {
        candidate_urls.push(format!("{base}/v1/models"));
    }
    candidate_urls.sort();
    candidate_urls.dedup();

    let mut errors = Vec::new();
    for url in candidate_urls {
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|err| format!("Fetch model list failed ({url}): {err}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let raw = resp.text().await.unwrap_or_default();
            let snippet = raw.chars().take(300).collect::<String>();
            errors.push(format!("{url} -> {status} | {snippet}"));
            continue;
        }

        let body = resp
            .json::<OpenAIModelListResponse>()
            .await
            .map_err(|err| format!("Parse model list failed ({url}): {err}"))?;

        let mut models = body
            .data
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        return Ok(models);
    }

    if errors.is_empty() {
        Err("Fetch model list failed: no candidate URL attempted".to_string())
    } else {
        Err(format!(
            "Fetch model list failed. Tried: {}",
            errors.join(" || ")
        ))
    }
}

#[tauri::command]
async fn refresh_models(input: RefreshModelsInput) -> Result<Vec<String>, String> {
    if input.api_key.trim().is_empty() {
        return Err("API key is empty.".to_string());
    }
    if input.base_url.trim().is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    match input.request_format.trim() {
        "openai" | "deepseek/kimi" => fetch_models_openai(&input).await,
        "openai_tts" => Err(
            "Request format 'openai_tts' is for audio transcriptions and does not support model list refresh."
                .to_string(),
        ),
        other => Err(format!(
            "Request format '{other}' model refresh is not implemented yet."
        )),
    }
}

#[tauri::command]
fn check_tools_status(
    input: CheckToolsStatusInput,
    state: State<'_, AppState>,
) -> Result<Vec<ToolLoadStatus>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut config = read_config(&state.config_path)?;
    normalize_api_tools(&mut config);
    drop(guard);

    let selected = resolve_selected_api_config(&config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config configured. Please add one.".to_string())?;

    if !selected.enable_tools {
        return Ok(selected
            .tools
            .iter()
            .map(|tool| ToolLoadStatus {
                id: tool.id.clone(),
                status: "disabled".to_string(),
                detail: "此 API 配置未启用工具调用。".to_string(),
            })
            .collect());
    }

    let mut statuses = Vec::new();
    for tool in selected.tools {
        let (status, detail) = match tool.id.as_str() {
            "fetch" => ("loaded".to_string(), "内置网页抓取工具可用".to_string()),
            "bing-search" => ("loaded".to_string(), "内置 Bing 爬虫搜索可用".to_string()),
            "memory-save" => ("loaded".to_string(), "内置记忆工具可用".to_string()),
            other => ("failed".to_string(), format!("未支持的内置工具: {other}")),
        };
        statuses.push(ToolLoadStatus {
            id: tool.id,
            status,
            detail,
        });
    }
    Ok(statuses)
}

#[tauri::command]
fn get_image_text_cache_stats(state: State<'_, AppState>) -> Result<ImageTextCacheStats, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let entries = data.image_text_cache.len();
    let total_chars = data
        .image_text_cache
        .iter()
        .map(|entry| entry.text.chars().count())
        .sum::<usize>();
    let latest_updated_at = data
        .image_text_cache
        .iter()
        .map(|entry| entry.updated_at.clone())
        .max();

    Ok(ImageTextCacheStats {
        entries,
        total_chars,
        latest_updated_at,
    })
}

#[tauri::command]
fn clear_image_text_cache(state: State<'_, AppState>) -> Result<ImageTextCacheStats, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&state.data_path)?;
    data.image_text_cache.clear();
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ImageTextCacheStats {
        entries: 0,
        total_chars: 0,
        latest_updated_at: None,
    })
}

#[tauri::command]
async fn send_debug_probe(state: State<'_, AppState>) -> Result<String, String> {
    let app_config = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let cfg = read_config(&state.config_path)?;
        drop(guard);
        cfg
    };

    let api_config = resolve_api_config(&app_config, None)?;
    if !is_openai_style_request_format(&api_config.request_format) {
        return Err(format!(
            "Request format '{}' is not implemented in probe router yet.",
            api_config.request_format
        ));
    }

    let prepared = PreparedPrompt {
        preamble: format!("[TIME]\nCurrent UTC time: {}", now_iso()),
        latest_user_text: api_config.fixed_test_prompt.clone(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };

    let reply = call_model_openai_rig_style(&api_config, &api_config.model, prepared).await?;
    Ok(reply.assistant_text)
}

