fn inflight_chat_key(api_config_id: &str, agent_id: &str) -> String {
    format!("{}::{}", api_config_id.trim(), agent_id.trim())
}

fn register_inflight_tool_abort_handle(
    state: &AppState,
    chat_key: &str,
    handle: AbortHandle,
) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(previous) = inflight.insert(chat_key.to_string(), handle) {
        previous.abort();
    }
    Ok(())
}

fn clear_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn abort_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(handle) = inflight.remove(chat_key) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn model_reply_has_visible_content(reply: &ModelReply) -> bool {
    !reply.assistant_text.trim().is_empty()
        || !reply.reasoning_standard.trim().is_empty()
        || !reply.reasoning_inline.trim().is_empty()
}

fn is_retryable_model_error(error: &str) -> bool {
    let normalized = error.trim().to_ascii_lowercase();
    normalized.contains("429") || normalized.contains("too many requests")
}

async fn send_chat_message_inner(
    input: SendChatRequest,
    state: &AppState,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    let requested_api_id = input
        .session
        .as_ref()
        .and_then(|s| s.api_config_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let requested_agent_id = input
        .session
        .as_ref()
        .map(|s| s.agent_id.trim().to_string())
        .filter(|v| !v.is_empty());

    let (app_config, selected_api, resolved_api, effective_agent_id) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let app_config = read_config(&state.config_path)?;
        let selected_api = if let Some(api_id) = requested_api_id.as_deref() {
            app_config
                .api_configs
                .iter()
                .find(|a| a.id == api_id)
                .cloned()
                .ok_or_else(|| format!("Selected API config '{api_id}' not found."))?
        } else {
            resolve_selected_api_config(&app_config, None)
                .ok_or_else(|| "No API config configured. Please add one.".to_string())?
        };
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let mut data = read_app_data(&state.data_path)?;
        let changed = ensure_default_agent(&mut data);
        if changed {
            write_app_data(&state.data_path, &data)?;
        }
        let effective_agent_id = if let Some(agent_id) = requested_agent_id.as_deref() {
            if data
                .agents
                .iter()
                .any(|a| a.id == agent_id && !a.is_built_in_user)
            {
                agent_id.to_string()
            } else {
                return Err(format!("Selected agent '{agent_id}' not found."));
            }
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
                .ok_or_else(|| "No assistant agent configured.".to_string())?
        };
        drop(guard);
        (app_config, selected_api, resolved_api, effective_agent_id)
    };

    let chat_key = inflight_chat_key(&selected_api.id, &effective_agent_id);
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(previous) = inflight.insert(chat_key.clone(), abort_handle) {
            previous.abort();
        }
    }
    let _ = abort_inflight_tool_abort_handle(state, &chat_key);

    let chat_session_key = chat_key.clone();
    let state_for_run = state.clone();
    let run = async move {
    let state = state_for_run;
    if !resolved_api.request_format.is_chat_text() {
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
                if !vision_resolved.request_format.is_chat_text() {
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
                            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
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

                    let guard = state
                        .state_lock
                        .lock()
                        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
                    let mut data = read_app_data(&state.data_path)?;
                    let mapped = if let Some(existing) =
                        find_image_text_cache(&data, &hash, &vision_api.id)
                    {
                        format!("[图片{}]\n{}", idx + 1, existing)
                    } else {
                        upsert_image_text_cache(&mut data, &hash, &vision_api.id, &converted);
                        write_app_data(&state.data_path, &data)?;
                        format!("[图片{}]\n{}", idx + 1, converted)
                    };
                    drop(guard);
                    converted_texts.push(mapped);
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
            MessagePart::Image { mime, .. } => {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    "[pdf]".to_string()
                } else {
                    "[image]".to_string()
                }
            }
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
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let _agent = data
            .agents
            .iter()
            .find(|a| a.id == effective_agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;

        if let Some(idx) =
            latest_active_conversation_index(&data, &selected_api.id, &effective_agent_id)
        {
            let conversation = data
                .conversations
                .get_mut(idx)
                .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?;
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

        let archive_res = run_archive_pipeline(
            &state,
            &selected_api,
            &resolved_api,
            &source,
            &effective_agent_id,
            &pending_archive_reason,
            "ARCHIVE-AUTO",
        )
        .await;

        match archive_res {
            Ok(result) => {
                archived_before_send = result.archived;
                if pending_archive_forced {
                    let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                        "归档完成，已开启新对话。".to_string()
                    } else {
                        format!(
                            "归档完成（降级摘要）：{}",
                            result.warning.unwrap_or_default()
                        )
                    };
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: "".to_string(),
                        kind: Some("tool_status".to_string()),
                        tool_name: Some("archive".to_string()),
                        tool_status: Some("done".to_string()),
                        message: Some(done_message),
                    });
                }
            }
            Err(err) => {
                if pending_archive_forced {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: "".to_string(),
                        kind: Some("tool_status".to_string()),
                        tool_name: Some("archive".to_string()),
                        tool_status: Some("failed".to_string()),
                        message: Some(format!("归档失败：{err}")),
                    });
                }
                return Err(format!("归档失败：{err}"));
            }
        }
    }

    let (model_name, prepared_prompt, conversation_id, latest_user_text) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;

        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let agent = data
            .agents
            .iter()
            .find(|a| a.id == effective_agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;

        let idx = ensure_active_conversation_index(&mut data, &selected_api.id, &effective_agent_id);

        // 聊天记录保留用户可见内容；模型请求使用 effective_payload（可能已做图转文）。
        let mut storage_api = selected_api.clone();
        storage_api.enable_image = true;
        storage_api.enable_audio = true;
        let mut storage_payload = input.payload.clone();
        if let Some(display_text) = input
            .payload
            .display_text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            storage_payload.text = Some(display_text.to_string());
        }
        let mut user_parts = build_user_parts(&storage_payload, &storage_api)?;
        externalize_message_parts_to_media_refs(&mut user_parts, &state.data_path)?;
        let conversation_before = data.conversations[idx].clone();
        let recall_query_text = memory_recall_query_text(&conversation_before, &effective_user_text);
        let private_memory_enabled = data
            .agents
            .iter()
            .find(|a| a.id == effective_agent_id)
            .map(|a| a.private_memory_enabled)
            .unwrap_or(false);
        let store_memories = memory_store_list_memories_visible_for_agent(
            &state.data_path,
            &effective_agent_id,
            private_memory_enabled,
        )?;
        let recall_hit_ids =
            memory_recall_hit_ids(&state.data_path, &store_memories, &recall_query_text);

        for memory_id in &recall_hit_ids {
            data.conversations[idx]
                .memory_recall_table
                .push(memory_id.clone());
        }

        let latest_recall_ids = memory_board_ids_from_current_hits(&recall_hit_ids, 7);
        let memory_board_xml =
            build_memory_board_xml_from_recall_ids(&store_memories, &latest_recall_ids);
        let last_archive_summary = data
            .conversations
            .iter()
            .rev()
            .find(|c| c.agent_id == effective_agent_id && !c.summary.trim().is_empty())
            .map(|c| c.summary.clone());

        let mut extra_text_blocks = input.payload.extra_text_blocks.clone().unwrap_or_default();
        if let Some(xml) = &memory_board_xml {
            extra_text_blocks.push(xml.clone());
        }
        let latest_user_text = effective_user_text.clone();

        let now = now_iso();
        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(input.speaker_agent_id.clone().unwrap_or_else(|| USER_PERSONA_ID.to_string())),
            parts: user_parts,
            extra_text_blocks,
            provider_meta: input.payload.provider_meta.clone(),
            tool_call: None,
            mcp_call: None,
        };

        data.conversations[idx].messages.push(user_message);
        data.conversations[idx].updated_at = now.clone();
        data.conversations[idx].last_user_at = Some(now_iso());
        data.conversations[idx].last_context_usage_ratio =
            compute_context_usage_ratio(&data.conversations[idx], selected_api.context_window_tokens);

        let conversation = data.conversations[idx].clone();
        let user_name = user_persona_name(&data);
        let user_intro = user_persona_intro(&data);
        let latest_user_prompt_text = conversation
            .messages
            .last()
            .map(|message| {
                let speaker_block = build_prompt_speaker_block(
                    message,
                    &data.agents,
                    &user_name,
                    &app_config.ui_language,
                );
                if speaker_block.trim().is_empty() {
                    latest_user_text.clone()
                } else if latest_user_text.trim().is_empty() {
                    speaker_block
                } else {
                    format!("{}\n{}", speaker_block, latest_user_text)
                }
            })
            .unwrap_or_else(|| latest_user_text.clone());
        let mut chat_overrides = ChatPromptOverrides::default();
        chat_overrides.latest_user_text = Some(latest_user_prompt_text);
        chat_overrides.latest_user_time_iso = Some(now.clone());
        chat_overrides
            .latest_user_system_blocks
            .push(build_hidden_skill_snapshot_block(&state));
        if let Some(xml) = &memory_board_xml {
            chat_overrides.latest_user_system_blocks.push(xml.clone());
        }
        if let Some(task_board) = build_hidden_task_board_block(&state) {
            chat_overrides.latest_user_system_blocks.push(task_board);
        }
        chat_overrides.latest_images = effective_images.clone();
        chat_overrides.latest_audios = effective_audios.clone();
        let prepared = build_prepared_prompt_for_mode(
            PromptBuildMode::Chat,
            &conversation,
            &agent,
            &data.agents,
            &user_name,
            &user_intro,
            &data.response_style_id,
            &app_config.ui_language,
            Some(&state.data_path),
            last_archive_summary.as_deref(),
            terminal_prompt_trusted_roots_block(&state, &selected_api),
            Some(chat_overrides),
        );

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

    let max_failure_retries = selected_api.failure_retry_count as usize;
    let mut model_reply: Option<ModelReply> = None;
    for attempt in 0..=max_failure_retries {
        let reply_result = call_model_openai_style(
            &resolved_api,
            &selected_api,
            &model_name,
            prepared_prompt.clone(),
            Some(&state),
            on_delta,
            app_config.tool_max_iterations as usize,
            &chat_session_key,
        )
        .await;

        let (reason_text, final_error_text) = match reply_result {
            Ok(reply) => {
                if model_reply_has_visible_content(&reply) {
                    model_reply = Some(reply);
                    break;
                }
                (
                    "Model returned an empty reply".to_string(),
                    "Model kept returning empty replies. Stopped retrying. Please try again later or switch model."
                        .to_string(),
                )
            }
            Err(error) => {
                if !is_retryable_model_error(&error) {
                    return Err(error);
                }
                (
                    "Model request was rate-limited (429)".to_string(),
                    format!("Model remained rate-limited (429) after retries: {error}"),
                )
            }
        };

        if attempt < max_failure_retries {
            let retry_index = attempt + 1;
            let wait_seconds = (retry_index as u64) * 5;
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                tool_name: None,
                tool_status: Some("running".to_string()),
                message: Some(format!(
                    "{reason_text}. Retrying ({retry_index}/{max_failure_retries}) in {wait_seconds}s..."
                )),
            });
            tokio::time::sleep(std::time::Duration::from_secs(wait_seconds)).await;
            continue;
        }
        let total_attempts = max_failure_retries + 1;
        let final_error = format!("{final_error_text} (attempted {total_attempts} times)");
        let _ = on_delta.send(AssistantDeltaEvent {
            delta: "".to_string(),
            kind: Some("tool_status".to_string()),
            tool_name: None,
            tool_status: Some("failed".to_string()),
            message: Some(final_error.clone()),
        });
        return Err(final_error);
    }
    let model_reply =
        model_reply.ok_or_else(|| "Model reply was invalid: no usable content received.".to_string())?;
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
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;

        let mut data = read_app_data(&state.data_path)?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id && c.summary.trim().is_empty())
        {
            let now = now_iso();
            conversation.messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(effective_agent_id.clone()),
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
    };

    let result = futures_util::future::Abortable::new(run, abort_registration).await;
    {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        inflight.remove(&chat_key);
    }
    if let Err(err) = clear_inflight_tool_abort_handle(state, &chat_key) {
        eprintln!(
            "[WARN][CHAT] clear inflight tool abort handle failed (session={}): {}",
            chat_key, err
        );
    }
    match result {
        Ok(inner) => inner,
        Err(_) => {
            eprintln!(
                "[INFO][CHAT] chat request aborted by user (session={})",
                chat_key
            );
            Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
        }
    }
}

#[tauri::command]
async fn send_chat_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    send_chat_message_inner(input, state.inner(), &on_delta).await
}

#[tauri::command]
async fn stop_chat_message(
    input: StopChatRequest,
    state: State<'_, AppState>,
) -> Result<StopChatResult, String> {
    let api_config_id = input
        .session
        .api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "Missing session.apiConfigId".to_string())?
        .to_string();
    let agent_id = input.session.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err("Missing session.agentId".to_string());
    }

    let chat_key = inflight_chat_key(&api_config_id, &agent_id);
    let aborted_chat = {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(handle) = inflight.remove(&chat_key) {
            handle.abort();
            true
        } else {
            false
        }
    };
    let aborted_tool = abort_inflight_tool_abort_handle(state.inner(), &chat_key)?;
    let aborted = aborted_chat || aborted_tool;

    let partial_assistant_text = input.partial_assistant_text.trim().to_string();
    let partial_reasoning_standard = input.partial_reasoning_standard.trim().to_string();
    let partial_reasoning_inline = input.partial_reasoning_inline.trim().to_string();
    let should_persist = !partial_assistant_text.is_empty()
        || !partial_reasoning_standard.is_empty()
        || !partial_reasoning_inline.is_empty();
    if !should_persist {
        return Ok(StopChatResult {
            aborted,
            persisted: false,
            conversation_id: None,
        });
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let app_config = read_config(&state.config_path)?;
    let selected_api = app_config
        .api_configs
        .iter()
        .find(|api| api.id == api_config_id)
        .cloned()
        .ok_or_else(|| format!("Selected API config '{api_config_id}' not found."))?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    let idx = latest_active_conversation_index(&data, &api_config_id, &agent_id);
    let Some(idx) = idx else {
        drop(guard);
        return Ok(StopChatResult {
            aborted,
            persisted: false,
            conversation_id: None,
        });
    };
    let conversation = data
        .conversations
        .get_mut(idx)
        .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?;

    // If the latest message is already an assistant message, do not append duplicate partial output.
    if conversation
        .messages
        .last()
        .map(|m| m.role == "assistant")
        .unwrap_or(false)
    {
        let conversation_id = conversation.id.clone();
        drop(guard);
        return Ok(StopChatResult {
            aborted,
            persisted: false,
            conversation_id: Some(conversation_id),
        });
    }

    let provider_meta = if partial_reasoning_standard.is_empty() && partial_reasoning_inline.is_empty()
    {
        None
    } else {
        Some(serde_json::json!({
            "reasoningStandard": partial_reasoning_standard,
            "reasoningInline": partial_reasoning_inline
        }))
    };

    let now = now_iso();
    conversation.messages.push(ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now.clone(),
        speaker_agent_id: Some(agent_id.clone()),
        parts: vec![MessagePart::Text {
            text: partial_assistant_text,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta,
        tool_call: None,
        mcp_call: None,
    });
    conversation.updated_at = now.clone();
    conversation.last_assistant_at = Some(now);
    conversation.last_context_usage_ratio =
        compute_context_usage_ratio(conversation, selected_api.context_window_tokens);
    let conversation_id = conversation.id.clone();

    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(StopChatResult {
        aborted,
        persisted: true,
        conversation_id: Some(conversation_id),
    })
}

async fn fetch_models_openai(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');
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

    let mut urls = vec![format!("{base}/models")];
    if !base.to_ascii_lowercase().ends_with("/v1") {
        urls.push(format!("{base}/v1/models"));
    }
    urls.dedup();

    let mut last_error = String::new();
    for url in urls {
        let resp = client
            .get(&url)
            .header(AUTHORIZATION, auth_value.clone())
            .send()
            .await
            .map_err(|err| format!("Fetch model list failed ({url}): {err}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let raw = resp.text().await.unwrap_or_default();
            let snippet = raw.chars().take(600).collect::<String>();
            last_error = format!("Fetch model list failed: {url} -> {status} | {snippet}");
            if status.as_u16() == 404 {
                continue;
            }
            return Err(last_error);
        }

        let body = resp
            .json::<OpenAIModelListResponse>()
            .await
            .map_err(|err| format!("Parse model list failed ({url}): {err}"))?;

        let mut models = body.data.into_iter().map(|item| item.id).collect::<Vec<_>>();
        models.sort();
        models.dedup();
        return Ok(models);
    }

    Err(last_error)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SttTranscribeInput {
    mime: String,
    bytes_base64: String,
    #[serde(default)]
    stt_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SttTranscribeOutput {
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalBinaryFileInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalBinaryFileOutput {
    mime: String,
    bytes_base64: String,
}

fn media_mime_from_path(path: &std::path::Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => Some("application/pdf"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "heic" => Some("image/heic"),
        "heif" => Some("image/heif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

#[tauri::command]
fn read_local_binary_file(
    input: ReadLocalBinaryFileInput,
) -> Result<ReadLocalBinaryFileOutput, String> {
    let path_text = input.path.trim();
    if path_text.is_empty() {
        return Err("File path is empty.".to_string());
    }
    let path = std::path::PathBuf::from(path_text);
    let mime = media_mime_from_path(&path)
        .ok_or_else(|| format!("Unsupported file type: '{}'.", path_text))?
        .to_string();
    let raw = fs::read(&path).map_err(|err| format!("Read file failed: {err}"))?;
    if raw.len() > MAX_MULTIMODAL_BYTES {
        return Err(format!(
            "File is too large ({} bytes). Max allowed is {} bytes.",
            raw.len(),
            MAX_MULTIMODAL_BYTES
        ));
    }
    Ok(ReadLocalBinaryFileOutput {
        mime,
        bytes_base64: B64.encode(raw),
    })
}

fn candidate_stt_urls(base_url: &str) -> Vec<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Vec::new();
    }
    let lower = base.to_ascii_lowercase();
    let mut urls = Vec::new();
    if lower.ends_with("/audio/transcriptions") {
        urls.push(base.to_string());
    } else if lower.ends_with("/v1") {
        urls.push(format!("{base}/audio/transcriptions"));
    } else {
        urls.push(format!("{base}/audio/transcriptions"));
        urls.push(format!("{base}/v1/audio/transcriptions"));
    }
    urls.sort();
    urls.dedup();
    urls
}

async fn call_openai_stt_transcribe(
    api_config: &ApiConfig,
    mime: &str,
    audio_raw: Vec<u8>,
) -> Result<String, String> {
    let model = api_config.model.trim();
    if model.is_empty() {
        return Err("STT model is empty.".to_string());
    }
    if api_config.api_key.trim().is_empty() {
        return Err("STT API key is empty.".to_string());
    }
    let urls = candidate_stt_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("STT base URL is empty.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Build STT HTTP client failed: {err}"))?;

    let mut errors = Vec::new();
    for url in urls {
        let file_part = reqwest::multipart::Part::bytes(audio_raw.clone())
            .file_name("speech.webm")
            .mime_str(if mime.trim().is_empty() {
                "audio/webm"
            } else {
                mime.trim()
            })
            .map_err(|err| format!("Build STT mime part failed: {err}"))?;
        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", model.to_string());
        let resp = client
            .post(&url)
            .bearer_auth(api_config.api_key.trim())
            .multipart(form)
            .send()
            .await;
        let Ok(resp) = resp else {
            errors.push(format!("{url} -> request failed"));
            continue;
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let raw = resp.text().await.unwrap_or_default();
            errors.push(format!(
                "{url} -> {status}: {}",
                raw.chars().take(220).collect::<String>()
            ));
            continue;
        }
        let body = resp
            .json::<Value>()
            .await
            .map_err(|err| format!("Parse STT response failed: {err}"))?;
        if let Some(text) = body.get("text").and_then(Value::as_str) {
            return Ok(text.trim().to_string());
        }
        if let Some(text) = body.get("transcript").and_then(Value::as_str) {
            return Ok(text.trim().to_string());
        }
        return Err(format!(
            "STT response does not contain text field: {}",
            body.to_string().chars().take(220).collect::<String>()
        ));
    }

    Err(format!(
        "STT request failed for all candidate URLs: {}",
        errors.join(" || ")
    ))
}

#[tauri::command]
async fn stt_transcribe(
    input: SttTranscribeInput,
    state: State<'_, AppState>,
) -> Result<SttTranscribeOutput, String> {
    if input.bytes_base64.trim().is_empty() {
        return Err("Audio payload is empty.".to_string());
    }

    let app_config = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let cfg = read_config(&state.config_path)?;
        drop(guard);
        cfg
    };

    let selected_id = input
        .stt_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or(app_config.stt_api_config_id.as_deref())
        .ok_or_else(|| "No STT API selected. Using local transcription only.".to_string())?;
    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == selected_id)
        .cloned()
        .ok_or_else(|| "Selected STT API config not found.".to_string())?;
    if !api.request_format.is_openai_stt() {
        return Err("Selected STT API must use request_format='openai_stt'.".to_string());
    }

    let audio_raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode audio base64 failed: {err}"))?;
    let text = call_openai_stt_transcribe(&api, &input.mime, audio_raw).await?;
    Ok(SttTranscribeOutput { text })
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

async fn fetch_models_anthropic(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');
    let url = format!("{base}/v1/models");
    let api_key = input.api_key.trim();

    if api_key.contains('\r') || api_key.contains('\n') {
        return Err("API key contains newline characters. Please paste a single-line token.".to_string());
    }
    if matches!(api_key, "..." | "***" | "•••" | "···") {
        return Err("API key is still a placeholder ('...' / '***'). Please paste the real token.".to_string());
    }

    let api_key_header = HeaderValue::from_str(api_key)
        .map_err(|err| format!("Build x-api-key header failed: {err}"))?;
    let anthropic_version = HeaderValue::from_static("2023-06-01");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    let resp = client
        .get(&url)
        .header("x-api-key", api_key_header)
        .header("anthropic-version", anthropic_version)
        .send()
        .await
        .map_err(|err| format!("Fetch Anthropic model list failed ({url}): {err}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        let snippet = raw.chars().take(600).collect::<String>();
        return Err(format!(
            "Fetch Anthropic model list failed: {url} -> {status} | {snippet}"
        ));
    }

    let body = resp
        .json::<AnthropicModelListResponse>()
        .await
        .map_err(|err| format!("Parse Anthropic model list failed ({url}): {err}"))?;

    let mut models = body
        .data
        .into_iter()
        .map(|item| item.id.trim().to_string())
        .filter(|id| !id.is_empty())
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

    match input.request_format {
        RequestFormat::OpenAI | RequestFormat::OpenAIResponses | RequestFormat::DeepSeekKimi => {
            fetch_models_openai(&input).await
        }
        RequestFormat::Gemini => fetch_models_gemini_native(&input).await,
        RequestFormat::GeminiEmbedding => Err(
            "Request format 'gemini_embedding' is for embedding and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::Anthropic => fetch_models_anthropic(&input).await,
        RequestFormat::OpenAITts => Err(
            "Request format 'openai_tts' is for TTS and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::OpenAIStt => Err(
            "Request format 'openai_stt' is for STT and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::OpenAIEmbedding => Err(
            "Request format 'openai_embedding' is for embedding and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::OpenAIRerank => Err(
            "Request format 'openai_rerank' is for rerank and does not support model list refresh."
                .to_string(),
        ),
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
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
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
        if matches!(tool.id.as_str(), "screenshot" | "wait") && !selected.enable_image {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "unavailable".to_string(),
                detail: "已启用，但当前模型不支持图像，运行时将跳过。".to_string(),
            });
            continue;
        }
        let (status, detail) = match tool.id.as_str() {
            "fetch" => ("loaded".to_string(), "内置网页抓取工具可用".to_string()),
            "websearch" => ("loaded".to_string(), "内置网页搜索工具可用".to_string()),
            "remember" => ("loaded".to_string(), "记住工具可用".to_string()),
            "recall" => ("loaded".to_string(), "回忆工具可用".to_string()),
            "screenshot" => ("loaded".to_string(), "截图工具可用".to_string()),
            "wait" => ("loaded".to_string(), "等待工具可用".to_string()),
            "reload" => (
                "loaded".to_string(),
                "MCP/Skill 重载工具可用".to_string(),
            ),
            "task" => (
                "loaded".to_string(),
                "任务工具可用".to_string(),
            ),
            "exec" => {
                #[cfg(target_os = "windows")]
                {
                    if state.terminal_shell.kind == "missing-git-bash" {
                        (
                            "unavailable".to_string(),
                            "未检测到 Git Bash。请安装 Git for Windows 后再启用终端工具。"
                                .to_string(),
                        )
                    } else {
                        ("loaded".to_string(), "执行工具可用".to_string())
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    ("loaded".to_string(), "执行工具可用".to_string())
                }
            }
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
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
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
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
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





