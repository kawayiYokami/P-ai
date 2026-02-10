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

    archive_if_idle(&mut data, &api_config.id, &input.agent_id);
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

    archive_if_idle(&mut data, &api_config.id, &input.agent_id);
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
            let vision_api = resolve_vision_api_config(&app_config)?;
            let vision_resolved = resolve_api_config(&app_config, Some(vision_api.id.as_str()))?;
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

                let converted = describe_image_with_vision_api(&vision_resolved, &vision_api, image)
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

    let (model_name, prepared_prompt, conversation_id, latest_user_text, archived_before_send) = {
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

        let archived_before_send = archive_if_idle(&mut data, &selected_api.id, &input.agent_id);
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

        let conversation = data.conversations[idx].clone();
        let mut prepared = build_prompt(&conversation, &agent, &data.user_alias, &now_iso());
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
            archived_before_send,
        )
    };

    let assistant_text = call_model_openai_style(
        &resolved_api,
        &selected_api,
        &model_name,
        prepared_prompt,
        Some(&state),
        &on_delta,
        app_config.tool_max_iterations as usize,
    )
    .await?;

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
                    text: assistant_text.clone(),
                }],
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            });
            conversation.updated_at = now.clone();
            conversation.last_assistant_at = Some(now);
            write_app_data(&state.data_path, &data)?;
        }
        drop(guard);
    }

    Ok(SendChatResult {
        conversation_id,
        latest_user_text,
        assistant_text,
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

    call_model_openai_rig_style(&api_config, &api_config.model, prepared).await
}

