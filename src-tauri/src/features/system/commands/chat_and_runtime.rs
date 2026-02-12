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

    if !matches!(
        resolved_api.request_format.trim(),
        "openai" | "deepseek/kimi" | "gemini"
    ) {
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
                if !matches!(
                    vision_resolved.request_format.trim(),
                    "openai" | "deepseek/kimi" | "gemini"
                ) {
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
                    if memory_merged > 0 {
                        invalidate_memory_matcher_cache();
                    }
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
                    let fallback_messages = keep_recent_turns(&source.messages, 3);
                    conv.messages = fallback_messages.clone();
                    let mut tmp = conv.clone();
                    tmp.messages = fallback_messages.clone();
                    let usage_after = compute_context_usage_ratio(&tmp, selected_api.context_window_tokens);
                    if usage_after >= 0.82 {
                        let now = now_iso();
                        conv.id = Uuid::new_v4().to_string();
                        conv.title = format!("Chat {}", &now.chars().take(16).collect::<String>());
                        conv.created_at = now.clone();
                        conv.updated_at = now;
                        conv.messages.clear();
                        conv.last_user_at = None;
                        conv.last_assistant_at = None;
                        conv.last_context_usage_ratio = 0.0;
                        write_app_data(&state.data_path, &data)?;
                        drop(guard);
                        if pending_archive_forced {
                            let _ = on_delta.send(AssistantDeltaEvent {
                                delta: "".to_string(),
                                kind: Some("tool_status".to_string()),
                                tool_name: Some("archive".to_string()),
                                tool_status: Some("failed".to_string()),
                                message: Some("归档失败且回退仍超限，已开启新对话。".to_string()),
                            });
                        }
                        return Err("归档失败且上下文仍超限，已自动开启新对话，请重新发送消息。".to_string());
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

        let mut extra_text_blocks = Vec::<String>::new();
        if let Some(xml) = &memory_board_xml {
            extra_text_blocks.push(xml.clone());
        }
        let latest_user_text = effective_user_text.clone();

        let now = now_iso();
        let time_context_block = build_time_context_block();

        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            parts: user_parts,
            extra_text_blocks,
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
        if let Some(summary) = last_archive_summary {
            prepared.preamble.push_str(
                "\n[HIDDEN ARCHIVE RECAP]\nUSER: 上次我们聊到哪里？\nASSISTANT: ",
            );
            prepared.preamble.push_str(summary.trim());
            prepared.preamble.push('\n');
        }
        let mut block2_parts = Vec::<String>::new();
        if let Some(xml) = &memory_board_xml {
            block2_parts.push(xml.clone());
        }
        block2_parts.push(time_context_block.clone());
        prepared.latest_user_text = latest_user_text.clone();
        prepared.latest_user_system_text = block2_parts.join("\n\n");
        prepared.latest_images = effective_images.clone();
        prepared.latest_audios = effective_audios.clone();

        // Use persisted API config as the source of truth to avoid stale
        // frontend model overrides after editing/saving config.
        let model_name = selected_api.model.trim().to_string();
        let model_name = if model_name.trim().is_empty() {
            resolved_api.model.clone()
        } else {
            model_name
        };
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
    let tool_history_events = model_reply.tool_history_events;

    let assistant_text_for_storage = assistant_text.clone();
    let provider_meta = {
        let standard = reasoning_standard.trim();
        let inline = reasoning_inline.trim();
        if standard.is_empty() && inline.is_empty() {
            None
        } else {
            Some(serde_json::json!({
                "reasoningStandard": standard,
                "reasoningInline": inline
            }))
        }
    };

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
                extra_text_blocks: Vec::new(),
                provider_meta: provider_meta.clone(),
                tool_call: if tool_history_events.is_empty() {
                    None
                } else {
                    Some(tool_history_events.clone())
                },
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
    let url = format!("{base}/models");
    let api_key = input.api_key.trim();

    if api_key.contains('\r') || api_key.contains('\n') {
        return Err("API key contains newline characters. Please paste a single-line token.".to_string());
    }
    if matches!(api_key, "..." | "***" | "•••" | "···") {
        return Err("API key is still a placeholder ('...' / '***'). Please paste the real token.".to_string());
    }
    let auth = format!("Bearer {api_key}");
    let auth_value = HeaderValue::from_str(&auth)
        .map_err(|err| {
            format!(
                "Build authorization header failed: {err}. The API key may contain invalid characters."
            )
        })?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    // 按用户约定：仅使用同一个 URL 规则（base_url + /models），不做额外路径推断。
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, auth_value)
        .send()
        .await
        .map_err(|err| format!("Fetch model list failed ({url}): {err}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        let snippet = raw.chars().take(600).collect::<String>();
        return Err(format!(
            "Fetch model list failed: {url} -> {status} | {snippet}"
        ));
    }

    let body = resp
        .json::<OpenAIModelListResponse>()
        .await
        .map_err(|err| format!("Parse model list failed ({url}): {err}"))?;

    let mut models = body.data.into_iter().map(|item| item.id).collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

async fn fetch_models_gemini_native(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');
    let has_version_path = base.contains("/v1beta") || base.contains("/v1/");
    let base_with_version = if has_version_path {
        base.to_string()
    } else {
        format!("{base}/v1beta")
    };
    let url = format!("{}/models", base_with_version.trim_end_matches('/'));
    let api_key = input.api_key.trim();

    if api_key.contains('\r') || api_key.contains('\n') {
        return Err("API key contains newline characters. Please paste a single-line token.".to_string());
    }
    if matches!(api_key, "..." | "***" | "•••" | "···") {
        return Err("API key is still a placeholder ('...' / '***'). Please paste the real token.".to_string());
    }

    let api_key_header = HeaderValue::from_str(api_key)
        .map_err(|err| format!("Build x-goog-api-key header failed: {err}"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    let resp = client
        .get(&url)
        .header("x-goog-api-key", api_key_header)
        .send()
        .await
        .map_err(|err| format!("Fetch Gemini model list failed ({url}): {err}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        let snippet = raw.chars().take(600).collect::<String>();
        return Err(format!(
            "Fetch Gemini model list failed: {url} -> {status} | {snippet}"
        ));
    }

    let body = resp
        .json::<GeminiNativeModelListResponse>()
        .await
        .map_err(|err| format!("Parse Gemini model list failed ({url}): {err}"))?;

    let mut models = body
        .models
        .into_iter()
        .map(|item| item.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .map(|name| name.trim_start_matches("models/").to_string())
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
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
        "gemini" => fetch_models_gemini_native(&input).await,
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
        if !tool.enabled {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "disabled".to_string(),
                detail: "该工具开关已关闭。".to_string(),
            });
            continue;
        }
        let (status, detail) = match tool.id.as_str() {
            "fetch" => ("loaded".to_string(), "内置网页抓取工具可用".to_string()),
            "bing-search" => ("loaded".to_string(), "内置 Bing 爬虫搜索可用".to_string()),
            "memory-save" => ("loaded".to_string(), "内置记忆工具可用".to_string()),
            "desktop-screenshot" => ("loaded".to_string(), "桌面截图工具可用".to_string()),
            "desktop-wait" => ("loaded".to_string(), "桌面等待工具可用".to_string()),
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
        history_messages: Vec::new(),
        latest_user_text: api_config.fixed_test_prompt.clone(),
        latest_user_system_text: String::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };

    let reply = call_model_openai_rig_style(&api_config, &api_config.model, prepared).await?;
    Ok(reply.assistant_text)
}

