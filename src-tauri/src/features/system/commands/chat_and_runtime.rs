fn inflight_chat_key(
    agent_id: &str,
    conversation_id: Option<&str>,
) -> String {
    match conversation_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(conversation_id) => format!("{}::{}", agent_id.trim(), conversation_id),
        None => agent_id.trim().to_string(),
    }
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

fn delegate_thread_chat_key(thread: &DelegateRuntimeThread) -> String {
    inflight_chat_key(
        &thread.target_agent_id,
        Some(&thread.conversation.id),
    )
}

fn abort_delegate_runtime_descendants_by_parent_session(
    state: &AppState,
    parent_chat_key: &str,
) -> Result<usize, String> {
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| thread.parent_chat_session_key.as_deref() == Some(parent_chat_key))
        .collect::<Vec<_>>();
    let mut aborted_count = 0usize;
    for thread in children {
        let child_chat_key = delegate_thread_chat_key(&thread);
        let aborted_chat = {
            let mut inflight = state
                .inflight_chat_abort_handles
                .lock()
                .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
            if let Some(handle) = inflight.remove(&child_chat_key) {
                handle.abort();
                true
            } else {
                false
            }
        };
        let aborted_tool = abort_inflight_tool_abort_handle(state, &child_chat_key)?;
        if aborted_chat || aborted_tool {
            aborted_count += 1;
            eprintln!(
                "[INFO][CHAT] aborted sync delegate child session: parent_session={}, child_session={}, delegate_id={}",
                parent_chat_key,
                child_chat_key,
                thread.delegate_id
            );
        }
        aborted_count += abort_delegate_runtime_descendants_by_parent_session(state, &child_chat_key)?;
    }
    Ok(aborted_count)
}

fn model_reply_has_visible_content(reply: &ModelReply) -> bool {
    !reply.assistant_text.trim().is_empty()
        || !reply.reasoning_standard.trim().is_empty()
        || !reply.reasoning_inline.trim().is_empty()
        || reply.suppress_assistant_message
}

fn effective_prompt_tokens_from_provider(
    estimated_prompt_tokens: u64,
    trusted_input_tokens: Option<u64>,
) -> (u64, &'static str) {
    let estimated = estimated_prompt_tokens.max(1);
    let Some(provider) = trusted_input_tokens.filter(|value| *value > 0) else {
        return (estimated_prompt_tokens, "estimate_no_provider");
    };
    let gap = provider.abs_diff(estimated) as f64 / estimated as f64;
    if gap > 0.5 {
        return (provider.max(estimated_prompt_tokens), "max_large_gap");
    }
    (provider, "provider")
}

fn estimate_prepared_prompt_tokens(
    prepared: &PreparedPrompt,
    selected_api: &ApiConfig,
    current_agent: &AgentProfile,
) -> u64 {
    let mut total = 0.0f64;
    total += estimated_tokens_for_text(&prepared.preamble);
    for hm in &prepared.history_messages {
        total += 10.0;
        total += estimated_tokens_for_text(&hm.text);
        if let Some(v) = hm.user_time_text.as_deref() {
            total += estimated_tokens_for_text(v);
        }
        if let Some(v) = hm.reasoning_content.as_deref() {
            total += estimated_tokens_for_text(v);
        }
        if let Some(calls) = hm.tool_calls.as_ref() {
            let text = serde_json::to_string(calls).unwrap_or_default();
            total += estimated_tokens_for_text(&text);
        }
    }
    for text_block in prepared_prompt_latest_user_text_blocks(prepared) {
        total += estimated_tokens_for_text(&text_block);
    }
    total += prepared.latest_images.len() as f64 * 280.0;
    total += prepared.latest_audios.len() as f64 * 320.0;

    if selected_api.enable_tools {
        let mut seen = std::collections::HashSet::<String>::new();
        for tool in selected_api
            .tools
            .iter()
            .chain(current_agent.tools.iter())
            .filter(|tool| tool.enabled)
        {
            let key = tool.id.trim().to_ascii_lowercase();
            if key.is_empty() || !seen.insert(key) {
                continue;
            }
            let text = serde_json::to_string(tool).unwrap_or_default();
            total += estimated_tokens_for_text(&text);
        }
    }

    total.ceil().max(0.0).min(u64::MAX as f64) as u64
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
    let trace_id = input
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("chat-{}", Uuid::new_v4()));
    let oldest_queue_created_at = input
        .oldest_queue_created_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let queue_wait_ms = oldest_queue_created_at
        .as_deref()
        .and_then(parse_iso)
        .map(|created_at| (now_utc() - created_at).whole_milliseconds())
        .filter(|ms| *ms > 0)
        .map(|ms| ms.min(i128::from(u64::MAX)) as u64);
    let session_for_log = input.session.clone();

    let chat_started_at = std::time::Instant::now();
    let stage_timeline = std::sync::Arc::new(std::sync::Mutex::new(Vec::<LlmRoundLogStage>::new()));
    let stage_timeline_for_chat = stage_timeline.clone();
    let log_chat_stage = |stage: &str| {
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut timeline) = stage_timeline_for_chat.lock() {
            let since_prev_ms = timeline
                .last()
                .map(|last| elapsed_ms.saturating_sub(last.elapsed_ms))
                .unwrap_or(elapsed_ms);
            timeline.push(LlmRoundLogStage {
                stage: stage.to_string(),
                elapsed_ms,
                since_prev_ms,
            });
        }
        eprintln!(
            "[聊天耗时] stage={}, elapsed_ms={}",
            stage,
            elapsed_ms
        );
    };
    log_chat_stage("send_chat_message_inner.start");

    let trigger_only = input.trigger_only;
    let requested_department_id = input
        .session
        .as_ref()
        .and_then(|s| s.department_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let requested_agent_id = input
        .session
        .as_ref()
        .map(|s| s.agent_id.trim().to_string())
        .filter(|v| !v.is_empty());
    let requested_conversation_id = input
        .session
        .as_ref()
        .and_then(|s| s.conversation_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);

    let (app_config, selected_api, resolved_api, effective_department_id, effective_agent_id, candidate_api_ids) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let mut app_config = state_read_config_cached(state)?;
        let mut data = state_read_app_data_cached(state)?;
        let changed = ensure_default_agent(&mut data);
        if changed {
            state_write_app_data_cached(state, &data)?;
        }
        let mut runtime_data = data.clone();
        merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
        let effective_agent_id = if let Some(agent_id) = requested_agent_id.as_deref() {
            if runtime_data
                .agents
                .iter()
                .any(|a| a.id == agent_id && !a.is_built_in_user)
            {
                agent_id.to_string()
            } else {
                return Err(format!("Selected agent '{agent_id}' not found."));
            }
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
                .ok_or_else(|| "No assistant agent configured.".to_string())?
        };
        let effective_department = if let Some(department_id) = requested_department_id.as_deref() {
            department_by_id(&app_config, department_id)
                .ok_or_else(|| format!("Department '{department_id}' not found."))?
        } else {
            department_for_agent_id(&app_config, &effective_agent_id)
                .or_else(|| assistant_department(&app_config))
                .ok_or_else(|| "No assistant department configured.".to_string())?
        };
        if !effective_department
            .agent_ids
            .iter()
            .any(|id| id.trim() == effective_agent_id)
        {
            return Err(format!(
                "Agent '{effective_agent_id}' is not assigned to department '{}'.",
                effective_department.id
            ));
        }
        let effective_department_id = effective_department.id.clone();
        let mut candidate_api_ids = department_api_config_ids(effective_department)
            .into_iter()
            .filter(|api_id| {
                app_config
                    .api_configs
                    .iter()
                    .any(|api| api.id == *api_id && api.request_format.is_chat_text())
            })
            .collect::<Vec<_>>();
        if candidate_api_ids.is_empty() {
            let fallback = resolve_selected_api_config(&app_config, None)
                .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
            candidate_api_ids.push(fallback.id.clone());
        }
        let selected_api_id = candidate_api_ids
            .first()
            .cloned()
            .ok_or_else(|| format!("Department '{}' has no available chat model.", effective_department_id))?;
        let selected_api = app_config
            .api_configs
            .iter()
            .find(|a| a.id == selected_api_id)
            .cloned()
            .ok_or_else(|| format!("Selected API config '{}' not found.", selected_api_id))?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        drop(guard);
        (
            app_config,
            selected_api,
            resolved_api,
            effective_department_id,
            effective_agent_id,
            candidate_api_ids,
        )
    };
    log_chat_stage("runtime_and_session_ready");

    let chat_key = inflight_chat_key(
        &effective_agent_id,
        requested_conversation_id.as_deref(),
    );
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
    let chat_session_key_for_log = chat_session_key.clone();
    let selected_api_for_log = selected_api.clone();
    let resolved_api_for_log = resolved_api.clone();
    let state_for_run = state.clone();
    let stage_timeline_for_run = stage_timeline.clone();
    let run = async move {
    let state = state_for_run;
    let log_run_stage = |stage: &str| {
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut timeline) = stage_timeline_for_run.lock() {
            let since_prev_ms = timeline
                .last()
                .map(|last| elapsed_ms.saturating_sub(last.elapsed_ms))
                .unwrap_or(elapsed_ms);
            timeline.push(LlmRoundLogStage {
                stage: stage.to_string(),
                elapsed_ms,
                since_prev_ms,
            });
        }
        eprintln!(
            "[聊天耗时] stage={}, elapsed_ms={}",
            stage,
            elapsed_ms
        );
    };
    log_run_stage("run.begin");
    if !resolved_api.request_format.is_chat_text() {
        return Err(format!(
            "Request format '{}' is not implemented in chat router yet.",
            resolved_api.request_format
        ));
    }

    let mut effective_payload = input.payload.clone();
    let extra_block_count = effective_payload
        .extra_text_blocks
        .as_ref()
        .map(|v| v.len())
        .unwrap_or(0);
    let attachment_count = effective_payload
        .attachments
        .as_ref()
        .map(|v| v.len())
        .unwrap_or(0);
    if extra_block_count > 0 {
        eprintln!(
            "[CHAT][ATTACHMENT] payload carries extra_text_blocks: count={}",
            extra_block_count
        );
    }
    if attachment_count > 0 {
        eprintln!(
            "[CHAT][ATTACHMENT] payload carries attachments json: count={}",
            attachment_count
        );
    }
    let audios = effective_payload.audios.clone().unwrap_or_default();
    if !audios.is_empty() {
        return Err("当前版本仅支持本地语音识别，发送消息不支持语音附件。".to_string());
    }
    if !trigger_only {
        let images = effective_payload.images.clone().unwrap_or_default();
        if !images.is_empty() {
            let notices = persist_payload_images_to_workspace_downloads(&state, &images);
            if !notices.is_empty() {
                let notice_text = notices.join("\n\n");
                let merged_text = effective_payload
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(|text| format!("{text}\n\n{notice_text}"))
                    .unwrap_or(notice_text);
                effective_payload.text = Some(merged_text);
            }
        }
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
                        let data = state_read_app_data_cached(&state)?;
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
                    let mut data = state_read_app_data_cached(&state)?;
                    let mapped = if let Some(existing) =
                        find_image_text_cache(&data, &hash, &vision_api.id)
                    {
                        format!("[图片{}]\n{}", idx + 1, existing)
                    } else {
                        upsert_image_text_cache(&mut data, &hash, &vision_api.id, &converted);
                        state_write_app_data_cached(&state, &data)?;
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
                let filtered_notice = match app_config.ui_language.trim() {
                    "en-US" => "[SYSTEM NOTICE] Image attachment was filtered out: current model has image input disabled and no vision fallback model is configured.",
                    "zh-TW" => "[系統提示] 已過濾圖片附件：當前模型未啟用圖片輸入，且未配置視覺回退模型。",
                    _ => "[系统提示] 已过滤图片附件：当前模型未启用图片输入，且未配置视觉回退模型。",
                };
                let merged_text = effective_payload
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(|text| format!("{text}\n\n{filtered_notice}"))
                    .unwrap_or_else(|| filtered_notice.to_string());
                effective_payload.text = Some(merged_text);
                effective_payload.images = None;
            }
        }
    }
    log_run_stage("attachments_processed");

    let effective_user_parts = if trigger_only {
        Vec::new()
    } else {
        build_user_parts(&effective_payload, &selected_api)?
    };
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

    let specific_conversation_requested = requested_conversation_id.is_some();
    let mut archived_before_send = false;
    let mut pending_archive_source: Option<Conversation> = None;
    let mut pending_archive_reason = String::new();
    let mut pending_archive_forced = false;

    if !trigger_only && !specific_conversation_requested {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let mut data = state_read_app_data_cached(&state)?;
        let changed = ensure_default_agent(&mut data);
        let mut config = state_read_config_cached(&state)?;
        let mut runtime_agents = data.agents.clone();
        merge_private_organization_into_runtime(&state.data_path, &mut config, &mut runtime_agents)?;
        let _agent = runtime_agents
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
        let _ = changed;
        state_write_app_data_cached(&state, &data)?;
        drop(guard);
    }

    if let Some(source) = pending_archive_source {
        if pending_archive_forced {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                tool_name: Some("archive".to_string()),
                tool_status: Some("running".to_string()),
                tool_args: None,
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
                        tool_args: None,
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
                        tool_args: None,
                        message: Some(format!("归档失败：{err}")),
                    });
                }
                return Err(format!("归档失败：{err}"));
            }
        }
    }
    log_run_stage("pre_send_archive_checked");

    let (
        _primary_model_name,
        prepared_prompt,
        conversation_id,
        latest_user_text,
        current_agent,
        estimated_prompt_tokens,
    ) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;

        let mut data = state_read_app_data_cached(&state)?;
        ensure_default_agent(&mut data);
        let agent = data
            .agents
            .iter()
            .find(|a| a.id == effective_agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;

        let runtime_conversation_id = requested_conversation_id
            .as_deref()
            .filter(|conversation_id| {
                delegate_runtime_thread_conversation_get(&state, conversation_id)
                    .ok()
                    .flatten()
                    .is_some()
            })
            .map(ToOwned::to_owned);
        let mut runtime_conversation = if let Some(conversation_id) = runtime_conversation_id.as_deref() {
            delegate_runtime_thread_conversation_get(&state, conversation_id)?
                .ok_or_else(|| format!("指定临时会话不存在：{conversation_id}"))?
        } else {
            Conversation {
                id: String::new(),
                title: String::new(),
                agent_id: String::new(),
                conversation_kind: String::new(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_user_at: None,
                last_assistant_at: None,
                last_context_usage_ratio: 0.0,
                last_effective_prompt_tokens: 0,
                status: String::new(),
                summary: String::new(),
                archived_at: None,
                messages: Vec::new(),
                memory_recall_table: Vec::new(),
            }
        };
        let is_runtime_conversation = runtime_conversation_id.is_some();
        let idx = if is_runtime_conversation {
            None
        } else if let Some(conversation_id) = requested_conversation_id.as_deref() {
            Some(
                data.conversations
                    .iter()
                    .position(|item| {
                        item.id == conversation_id
                            && item.summary.trim().is_empty()
                            && !conversation_is_delegate(item)
                    })
                    .ok_or_else(|| format!("指定会话不存在或不可用：{conversation_id}"))?,
            )
        } else {
            Some(ensure_active_conversation_index(&mut data, &selected_api.id, &effective_agent_id))
        };
        let conversation_before = if let Some(idx) = idx {
            data.conversations[idx].clone()
        } else {
            runtime_conversation.clone()
        };
        let is_delegate_conversation =
            conversation_before.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE;
        let last_archive_summary = if is_runtime_conversation || conversation_is_delegate(&conversation_before) {
            None
        } else {
            data.conversations
                .iter()
                .rev()
                .find(|c| c.agent_id == effective_agent_id && !c.summary.trim().is_empty())
                .map(|c| c.summary.clone())
        };
        let conversation = if trigger_only {
            let latest_message = conversation_before
                .messages
                .last()
                .ok_or_else(|| "当前对话没有可供继续处理的消息。".to_string())?;
            if latest_message
                .speaker_agent_id
                .as_deref()
                .map(str::trim)
                == Some(effective_agent_id.as_str())
            {
                return Err("当前最后一条消息来自助理自身，无需重复激活。".to_string());
            }
            conversation_before.clone()
        } else {
            let mut storage_api = selected_api.clone();
            storage_api.enable_image = true;
            storage_api.enable_audio = true;
            let mut storage_payload = input.payload.clone();
            if let Some(display_text) = input.payload.display_text.as_deref() {
                storage_payload.text = Some(display_text.trim().to_string());
            }
            let mut user_parts = build_user_parts(&storage_payload, &storage_api)?;
            externalize_message_parts_to_media_refs(&mut user_parts, &state.data_path)?;
            let (recall_hit_ids, memory_board_xml) = if is_delegate_conversation {
                (Vec::<String>::new(), None)
            } else {
                let recall_query_text =
                    memory_recall_query_text(&conversation_before, &effective_user_text);
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
                let latest_recall_ids = memory_board_ids_from_current_hits(&recall_hit_ids, 7);
                let memory_board_xml =
                    build_memory_board_xml_from_recall_ids(&store_memories, &latest_recall_ids);
                (recall_hit_ids, memory_board_xml)
            };
            let mut extra_text_blocks = input.payload.extra_text_blocks.clone().unwrap_or_default();
            if let Some(xml) = &memory_board_xml {
                extra_text_blocks.push(xml.clone());
            }
            let attachment_meta = normalize_payload_attachments(input.payload.attachments.as_ref());
            let user_provider_meta =
                merge_provider_meta_with_attachments(input.payload.provider_meta.clone(), &attachment_meta);
            let now = now_iso();
            let user_message = ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(input.speaker_agent_id.clone().unwrap_or_else(|| USER_PERSONA_ID.to_string())),
                parts: user_parts,
                extra_text_blocks,
                provider_meta: user_provider_meta,
                tool_call: None,
                mcp_call: None,
            };
            if let Some(idx) = idx {
                for memory_id in &recall_hit_ids {
                    data.conversations[idx]
                        .memory_recall_table
                        .push(memory_id.clone());
                }
                data.conversations[idx].messages.push(user_message);
                data.conversations[idx].updated_at = now.clone();
                data.conversations[idx].last_user_at = Some(now_iso());
                data.conversations[idx].last_context_usage_ratio =
                    compute_context_usage_ratio(&data.conversations[idx], selected_api.context_window_tokens);
                data.conversations[idx].clone()
            } else {
                for memory_id in &recall_hit_ids {
                    runtime_conversation.memory_recall_table.push(memory_id.clone());
                }
                runtime_conversation.messages.push(user_message);
                runtime_conversation.updated_at = now.clone();
                runtime_conversation.last_user_at = Some(now_iso());
                runtime_conversation.last_context_usage_ratio =
                    compute_context_usage_ratio(&runtime_conversation, selected_api.context_window_tokens);
                delegate_runtime_thread_conversation_update(
                    &state,
                    runtime_conversation_id.as_deref().unwrap_or_default(),
                    runtime_conversation.clone(),
                )?;
                runtime_conversation.clone()
            }
        };
        let user_name = user_persona_name(&data);
        let user_intro = user_persona_intro(&data);
        let latest_user_text = if trigger_only {
            conversation
                .messages
                .iter()
                .rev()
                .find(|message| prompt_role_for_message(message, &effective_agent_id).as_deref() == Some("user"))
                .map(render_message_content_for_model)
                .unwrap_or_default()
        } else {
            effective_user_text.clone()
        };
        let is_delegate_conversation =
            conversation.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE;
        let latest_user_meta_text = conversation
            .messages
            .last()
            .map(|message| {
                let speaker_block = build_prompt_speaker_block(
                    message,
                    &data.agents,
                    &user_name,
                    &app_config.ui_language,
                );
                let time_text = if is_delegate_conversation {
                    message.created_at.trim().to_string()
                } else {
                    format_message_time_rfc3339_local(&message.created_at)
                };
                match (speaker_block.trim().is_empty(), time_text.trim().is_empty()) {
                    (false, false) => format!("{speaker_block} {time_text}"),
                    (false, true) => speaker_block,
                    (true, false) => time_text,
                    (true, true) => String::new(),
                }
            })
            .unwrap_or_default();
        let mut chat_overrides = ChatPromptOverrides::default();
        chat_overrides
            .system_preamble_blocks
            .push(build_hidden_skill_snapshot_block(&state));
        chat_overrides
            .system_preamble_blocks
            .push(build_hidden_skill_usage_block());
        if !trigger_only {
            chat_overrides.latest_user_text = Some(latest_user_text.clone());
            chat_overrides.latest_user_meta_text = Some(latest_user_meta_text);
            if !is_delegate_conversation {
                if let Some(task_board) = build_hidden_task_board_block(&state) {
                    chat_overrides.latest_user_extra_blocks.push(task_board);
                }
            }
            let attachment_meta = normalize_payload_attachments(input.payload.attachments.as_ref());
            for item in attachment_meta {
                let file_name = item
                    .get("fileName")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                let relative_path = item
                    .get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if file_name.is_empty() || relative_path.is_empty() {
                    continue;
                }
                chat_overrides.latest_user_extra_blocks.push(format!(
                    "用户本次上传了一个附件：{}\n保存到了你工作区的downloads文件夹内（路径：{}）\n如果需要，请使用 shell 工具读取该文件内容。",
                    file_name, relative_path
                ));
            }
            chat_overrides.latest_images = Some(effective_images.clone());
            chat_overrides.latest_audios = Some(effective_audios.clone());
        }
        let chat_overrides = Some(chat_overrides);
        let prepared = build_prepared_prompt_for_mode(
            if is_delegate_conversation {
                PromptBuildMode::Delegate
            } else {
                PromptBuildMode::Chat
            },
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
            terminal_prompt_trusted_roots_block(&state, &selected_api),
            chat_overrides,
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
        let estimated_prompt_tokens =
            estimate_prepared_prompt_tokens(&prepared, &selected_api, &agent);

        if !trigger_only && !is_runtime_conversation {
            state_write_app_data_cached(&state, &data)?;
        }
        drop(guard);

        (
            model_name,
            prepared,
            conversation_id,
            latest_user_text,
            agent,
            estimated_prompt_tokens,
        )
    };
    log_run_stage("prompt_ready");

    let mut model_reply: Option<ModelReply> = None;
    let mut active_selected_api = selected_api.clone();
    let mut active_resolved_api = resolved_api.clone();
    let mut fallback_errors = Vec::<String>::new();
    for (candidate_index, candidate_api_id) in candidate_api_ids.iter().enumerate() {
        eprintln!(
            "[聊天耗时] stage=model_candidate.start, elapsed_ms={}, candidate_index={}, candidate_api_id={}",
            chat_started_at.elapsed().as_millis(),
            candidate_index,
            candidate_api_id
        );
        log_run_stage("model_candidate.start");
        let candidate_selected_api = if candidate_api_id == &selected_api.id {
            selected_api.clone()
        } else {
            match resolve_selected_api_config(&app_config, Some(candidate_api_id.as_str())) {
                Some(api) => api,
                None => {
                    fallback_errors.push(format!("{candidate_api_id}: 候选模型不存在"));
                    continue;
                }
            }
        };
        let candidate_resolved_api =
            match resolve_api_config(&app_config, Some(candidate_selected_api.id.as_str())) {
                Ok(api) => api,
                Err(error) => {
                    fallback_errors.push(format!("{}: {}", candidate_selected_api.name, error));
                    continue;
                }
            };
        let candidate_model_name = if candidate_selected_api.model.trim().is_empty() {
            candidate_resolved_api.model.clone()
        } else {
            candidate_selected_api.model.trim().to_string()
        };
        let max_failure_retries = candidate_selected_api.failure_retry_count as usize;
        let mut candidate_final_error: Option<String> = None;
        for attempt in 0..=max_failure_retries {
            eprintln!(
                "[聊天耗时] stage=model_request.start, elapsed_ms={}, candidate_api_id={}, attempt={}",
                chat_started_at.elapsed().as_millis(),
                candidate_selected_api.id,
                attempt + 1
            );
            log_run_stage("model_request.start");
            let reply_result = call_model_openai_style(
                &candidate_resolved_api,
                &app_config,
                &candidate_selected_api,
                &current_agent,
                &candidate_model_name,
                prepared_prompt.clone(),
                Some(&state),
                on_delta,
                app_config.tool_max_iterations as usize,
                &chat_session_key,
            )
            .await;
            eprintln!(
                "[聊天耗时] stage=model_request.finish, elapsed_ms={}, candidate_api_id={}, attempt={}",
                chat_started_at.elapsed().as_millis(),
                candidate_selected_api.id,
                attempt + 1
            );
            log_run_stage("model_request.finish");

            let (reason_text, final_error_text) = match reply_result {
                Ok(reply) => {
                    if model_reply_has_visible_content(&reply) {
                        active_selected_api = candidate_selected_api.clone();
                        active_resolved_api = candidate_resolved_api.clone();
                        model_reply = Some(reply);
                        candidate_final_error = None;
                        break;
                    }
                    (
                        "模型返回空响应".to_string(),
                        "模型持续返回空响应，已停止重试，请稍后再试或切换模型。"
                            .to_string(),
                    )
                }
                Err(error) => {
                    if !is_retryable_model_error(&error) {
                        candidate_final_error = Some(error);
                        break;
                    }
                    (
                        "模型请求被限流 (429)".to_string(),
                        format!("模型持续被限流 (429)，重试后仍失败: {error}"),
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
                    tool_args: None,
                    message: Some(format!(
                        "{reason_text}，正在重试 ({retry_index}/{max_failure_retries})，等待 {wait_seconds} 秒..."
                    )),
                });
                tokio::time::sleep(std::time::Duration::from_secs(wait_seconds)).await;
                continue;
            }
            let total_attempts = max_failure_retries + 1;
            candidate_final_error = Some(format!(
                "{final_error_text} (attempted {total_attempts} times)"
            ));
        }
        if model_reply.is_some() {
            break;
        }
        if let Some(error) = candidate_final_error {
            fallback_errors.push(format!("{}: {}", candidate_selected_api.name, error));
        }
        if candidate_index + 1 < candidate_api_ids.len() {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                tool_name: None,
                tool_status: Some("running".to_string()),
                tool_args: None,
                message: Some(format!(
                    "当前模型失败，正在切换到下一个候选模型（{}/{}）...",
                    candidate_index + 2,
                    candidate_api_ids.len()
                )),
            });
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    let model_reply =
        model_reply.ok_or_else(|| {
            if fallback_errors.is_empty() {
                "模型回复无效：未收到可用内容。".to_string()
            } else {
                format!("所有候选模型均失败：{}", fallback_errors.join(" | "))
            }
        })?;
    let assistant_text = model_reply.assistant_text;
    let reasoning_standard = model_reply.reasoning_standard;
    let reasoning_inline = model_reply.reasoning_inline;
    let tool_history_events = model_reply.tool_history_events;
    let suppress_assistant_message = model_reply.suppress_assistant_message;
    let trusted_input_tokens = model_reply.trusted_input_tokens;
    let (effective_prompt_tokens, effective_prompt_source) =
        effective_prompt_tokens_from_provider(estimated_prompt_tokens, trusted_input_tokens);
    let context_usage_ratio =
        effective_prompt_tokens as f64 / f64::from(active_selected_api.context_window_tokens.max(1));
    let context_usage_percent = context_usage_ratio.mul_add(100.0, 0.0).round().clamp(0.0, 100.0) as u32;

    let assistant_text_for_storage = assistant_text.clone();
    let provider_meta = {
        let standard = reasoning_standard.trim();
        let inline = reasoning_inline.trim();
        if standard.is_empty()
            && inline.is_empty()
            && trusted_input_tokens.is_none()
            && estimated_prompt_tokens == 0
        {
            None
        } else {
            Some(serde_json::json!({
                "reasoningStandard": standard,
                "reasoningInline": inline,
                "providerPromptTokens": trusted_input_tokens,
                "estimatedPromptTokens": estimated_prompt_tokens,
                "effectivePromptTokens": effective_prompt_tokens,
                "effectivePromptSource": effective_prompt_source,
                "contextUsagePercent": context_usage_percent,
                "contextUsageRatio": context_usage_ratio
            }))
        }
    };
    log_run_stage("model_reply_ready");

    let mut post_reply_forced_archive_source: Option<Conversation> = None;
    let mut persisted_assistant_message: Option<ChatMessage> = None;
    {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;

        let mut data = state_read_app_data_cached(&state)?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id && c.summary.trim().is_empty())
        {
            let now = now_iso();
            if !suppress_assistant_message {
                let assistant_message = ChatMessage {
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
                };
                conversation.messages.push(assistant_message.clone());
                persisted_assistant_message = Some(assistant_message);
                conversation.updated_at = now.clone();
                conversation.last_assistant_at = Some(now);
            }
            conversation.last_effective_prompt_tokens = effective_prompt_tokens;
            conversation.last_context_usage_ratio = context_usage_ratio;
            if !suppress_assistant_message && conversation.last_context_usage_ratio >= 0.82 {
                post_reply_forced_archive_source = Some(conversation.clone());
            }
            state_write_app_data_cached(&state, &data)?;
        } else if let Some(mut conversation) =
            delegate_runtime_thread_conversation_get(&state, &conversation_id)?
        {
            let now = now_iso();
            if !suppress_assistant_message {
                let assistant_message = ChatMessage {
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
                };
                conversation.messages.push(assistant_message.clone());
                persisted_assistant_message = Some(assistant_message);
                conversation.updated_at = now.clone();
                conversation.last_assistant_at = Some(now);
            }
            conversation.last_effective_prompt_tokens = effective_prompt_tokens;
            conversation.last_context_usage_ratio = context_usage_ratio;
            delegate_runtime_thread_conversation_update(&state, &conversation_id, conversation)?;
        }
        drop(guard);
    }
    log_run_stage("assistant_message_persisted");

    if let Some(source) = post_reply_forced_archive_source {
        let _ = on_delta.send(AssistantDeltaEvent {
            delta: "".to_string(),
            kind: Some("tool_status".to_string()),
            tool_name: Some("archive".to_string()),
            tool_status: Some("running".to_string()),
            tool_args: None,
            message: Some("回复后上下文已达到 82%，正在自动归档...".to_string()),
        });
        let archive_res = run_archive_pipeline(
            &state,
            &active_selected_api,
            &active_resolved_api,
            &source,
            &effective_agent_id,
            "force_context_usage_82_after_reply",
            "ARCHIVE-AFTER-REPLY",
        )
        .await;
        match archive_res {
            Ok(result) => {
                let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                    "自动归档完成，已开启新对话。".to_string()
                } else {
                    format!("自动归档完成（降级摘要）：{}", result.warning.unwrap_or_default())
                };
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: "".to_string(),
                    kind: Some("tool_status".to_string()),
                    tool_name: Some("archive".to_string()),
                    tool_status: Some("done".to_string()),
                    tool_args: None,
                    message: Some(done_message),
                });
            }
            Err(err) => {
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: "".to_string(),
                    kind: Some("tool_status".to_string()),
                    tool_name: Some("archive".to_string()),
                    tool_status: Some("failed".to_string()),
                    tool_args: None,
                    message: Some(format!("自动归档失败：{err}")),
                });
                return Err(format!("自动归档失败：{err}"));
            }
        }
    }
    log_run_stage("post_reply_archive_checked");

    Ok(SendChatResult {
        conversation_id,
        latest_user_text,
        assistant_text,
        reasoning_standard,
        reasoning_inline,
        archived_before_send,
        assistant_message: persisted_assistant_message,
        provider_prompt_tokens: trusted_input_tokens,
        estimated_prompt_tokens: Some(estimated_prompt_tokens),
        effective_prompt_tokens: Some(effective_prompt_tokens),
        effective_prompt_source: Some(effective_prompt_source.to_string()),
        context_window_tokens: Some(active_selected_api.context_window_tokens),
        max_output_tokens: Some(active_selected_api.max_output_tokens),
        context_usage_percent: Some(context_usage_percent),
    })
    };

    let result = futures_util::future::Abortable::new(run, abort_registration).await;
    log_chat_stage("send_chat_message_inner.finish");
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
    let final_result = match result {
        Ok(inner) => inner,
        Err(_) => {
            eprintln!(
                "[INFO][CHAT] chat request aborted by user (session={})",
                chat_key
            );
            Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
        }
    };
    let timeline = stage_timeline.lock().ok().map(|items| items.clone());
    let (mut pipeline_headers, pipeline_tools) = latest_chat_round_headers_and_tools(
        state,
        resolved_api_for_log.request_format,
        &selected_api_for_log.name,
        &selected_api_for_log.model,
        &resolved_api_for_log.base_url,
    );
    if pipeline_headers.is_empty() {
        pipeline_headers = masked_auth_headers(&selected_api_for_log.api_key);
    }
    push_llm_round_log(
        Some(state),
        Some(trace_id),
        "chat_pipeline",
        resolved_api_for_log.request_format,
        &selected_api_for_log.name,
        &selected_api_for_log.model,
        &resolved_api_for_log.base_url,
        pipeline_headers,
        pipeline_tools,
        serde_json::json!({
            "triggerOnly": trigger_only,
            "queueWaitMs": queue_wait_ms,
            "oldestQueueCreatedAt": oldest_queue_created_at,
            "chatSessionKey": chat_session_key_for_log,
            "effectiveDepartmentId": effective_department_id,
            "session": session_for_log,
        }),
        final_result
            .as_ref()
            .ok()
            .map(|value| serde_json::json!({
                "conversationId": value.conversation_id,
                "assistantTextLength": value.assistant_text.chars().count(),
                "reasoningStandardLength": value.reasoning_standard.chars().count(),
                "reasoningInlineLength": value.reasoning_inline.chars().count(),
                "usage": {
                    "rigPromptTokens": value.provider_prompt_tokens,
                    "estimatedPromptTokens": value.estimated_prompt_tokens,
                    "effectivePromptTokens": value.effective_prompt_tokens,
                    "effectivePromptSource": value.effective_prompt_source,
                    "contextWindowTokens": value.context_window_tokens,
                    "maxOutputTokens": value.max_output_tokens,
                    "contextUsagePercent": value.context_usage_percent
                }
            })),
        final_result.as_ref().err().cloned(),
        chat_started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        timeline,
    );
    final_result
}

#[tauri::command]
async fn send_chat_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    // 如果是 trigger_only 模式（由调度器调用），直接执行
    if input.trigger_only {
        return send_chat_message_inner(input, state.inner(), &on_delta).await;
    }

    // 用户发言：构造消息并入队
    let text = input.payload.text.as_deref().unwrap_or("").trim();
    let images = input.payload.images.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let attachments = input
        .payload
        .attachments
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    if text.is_empty() && images.is_empty() && attachments.is_empty() {
        return Err("消息内容为空".to_string());
    }

    // 获取会话信息
    let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
    let requested_department_id = session
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let agent_id = session.agent_id.trim().to_string();

    if agent_id.is_empty() {
        return Err("会话信息不完整".to_string());
    }

    // 获取或创建会话ID
    let (conversation_id, department_id) = {
        let guard = state.state_lock.lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let app_config = state_read_config_cached(&state)?;
        let mut data = state_read_app_data_cached(&state)?;
        let department = if let Some(department_id) = requested_department_id.as_deref() {
            department_by_id(&app_config, department_id)
                .ok_or_else(|| format!("部门不存在: {department_id}"))?
        } else {
            department_for_agent_id(&app_config, &agent_id)
                .or_else(|| assistant_department(&app_config))
                .ok_or_else(|| "未找到可用部门".to_string())?
        };
        let api_config_id = department_primary_api_config_id(department);
        if api_config_id.trim().is_empty() {
            return Err(format!("部门模型未配置: {}", department.id));
        }

        let conversation_id = if let Some(cid) = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if data.conversations.iter().any(|conv| {
                conv.id == cid
                    && conv.status == "active"
                    && conv.summary.trim().is_empty()
                    && !conversation_is_delegate(conv)
                    && conv.agent_id == agent_id
            }) {
                cid.to_string()
            } else {
                let idx = ensure_active_conversation_index(&mut data, &api_config_id, &agent_id);
                let fallback_id = data.conversations
                    .get(idx)
                    .map(|item| item.id.clone())
                    .ok_or_else(|| "活动会话索引超出范围".to_string())?;
                let reject_reason = data
                    .conversations
                    .iter()
                    .find(|conv| conv.id == cid)
                    .map(|conv| {
                        if conv.agent_id != agent_id {
                            "mismatched_agent"
                        } else if conv.status != "active" {
                            "inactive"
                        } else if !conv.summary.trim().is_empty() {
                            "summary_present"
                        } else if conversation_is_delegate(conv) {
                            "delegate_conversation"
                        } else {
                            "unknown"
                        }
                    })
                    .unwrap_or("not_found");
                eprintln!(
                    "[INFO][CHAT] session conversation id rejected and fallback selected: requested_cid={}, reject_reason={}, fallback_cid={}, department_id={}, agent_id={}",
                    cid,
                    reject_reason,
                    fallback_id,
                    department.id,
                    agent_id
                );
                fallback_id
            }
        } else {
            let idx = ensure_active_conversation_index(&mut data, &api_config_id, &agent_id);
            data.conversations
                .get(idx)
                .map(|item| item.id.clone())
                .ok_or_else(|| "活动会话索引超出范围".to_string())?
        };
        state_write_app_data_cached(&state, &data)?;

        drop(guard);
        (conversation_id, department.id.clone())
    };

    // 构造用户消息
    let mut message_parts = Vec::new();
    if !text.is_empty() {
        message_parts.push(MessagePart::Text { text: text.to_string() });
    }
    for img in images {
        message_parts.push(MessagePart::Image {
            mime: img.mime.clone(),
            bytes_base64: img.bytes_base64.clone(),
            name: None,
            compressed: false,
        });
    }

    let user_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: message_parts,
        extra_text_blocks: Vec::new(),
        provider_meta: merge_provider_meta_with_attachments(
            input.payload.provider_meta.clone(),
            &normalize_payload_attachments(input.payload.attachments.as_ref()),
        ),
        tool_call: None,
        mcp_call: None,
    };

    // 构造队列事件
    let event_id = Uuid::new_v4().to_string();
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        messages: vec![user_message],
        activate_assistant: true,
        session_info: ChatSessionInfo {
            department_id: department_id.clone(),
            agent_id: agent_id.clone(),
        },
        sender_info: None,
    };

    let main_session_state_text = get_main_session_state(state.inner())
        .map(|value| match value {
            MainSessionState::Idle => "idle".to_string(),
            MainSessionState::AssistantStreaming => "assistant_streaming".to_string(),
            MainSessionState::OrganizingContext => "organizing_context".to_string(),
        })
        .unwrap_or_else(|err| format!("unknown({err})"));

    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    register_chat_event_runtime(state.inner(), &event_id, on_delta.clone(), result_tx)?;

    // 入队
    if let Err(err) = enqueue_chat_event(state.inner(), event) {
        let _ = state
            .pending_chat_delta_channels
            .lock()
            .map(|mut map| map.remove(&event_id));
        let _ = state
            .pending_chat_result_senders
            .lock()
            .map(|mut map| map.remove(&event_id));
        return Err(err);
    }

    let queue_len = state
        .chat_pending_queue
        .lock()
        .map(|queue| queue.len())
        .unwrap_or_default();
    eprintln!(
        "[聊天调度] 用户消息已入队: event_id={}, conversation_id={}, department_id={}, agent_id={}, queue_len={}, main_session_state={}",
        event_id,
        conversation_id,
        department_id,
        agent_id,
        queue_len,
        main_session_state_text,
    );

    // 触发出队处理
    trigger_chat_queue_processing(state.inner());

    result_rx
        .await
        .map_err(|_| "聊天请求已取消或调度结果丢失".to_string())?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BindActiveChatViewStreamInput {
    #[serde(default)]
    conversation_id: Option<String>,
}

#[tauri::command]
async fn bind_active_chat_view_stream(
    input: BindActiveChatViewStreamInput,
    state: State<'_, AppState>,
    window: tauri::Window,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(), String> {
    let window_label = window.label().to_string();
    let conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(conversation_id) = conversation_id {
        set_active_chat_view_stream_binding(
            state.inner(),
            &window_label,
            Some(conversation_id),
            on_delta,
        )?;
        eprintln!(
            "[INFO][CHAT] active chat stream bound: window={}, conversation_id={}",
            window_label,
            conversation_id
        );
    } else {
        clear_active_chat_view_stream_binding(state.inner(), &window_label)?;
        eprintln!(
            "[INFO][CHAT] active chat stream unbound: window={}",
            window_label
        );
    }
    Ok(())
}

#[tauri::command]
async fn stop_chat_message(
    input: StopChatRequest,
    state: State<'_, AppState>,
) -> Result<StopChatResult, String> {
    let agent_id = input.session.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err("Missing session.agentId".to_string());
    }
    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let requested_department_id = input
        .session
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let chat_key = inflight_chat_key(
        &agent_id,
        requested_conversation_id.as_deref(),
    );
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
    let aborted_delegate_children =
        abort_delegate_runtime_descendants_by_parent_session(state.inner(), &chat_key)?;
    let aborted = aborted_chat || aborted_tool || aborted_delegate_children > 0;
    if aborted_delegate_children > 0 {
        eprintln!(
            "[INFO][CHAT] stop request cascaded to sync delegate children: session={}, child_count={}",
            chat_key,
            aborted_delegate_children
        );
    }

    let partial_assistant_text = input.partial_assistant_text.trim().to_string();
    let partial_reasoning_standard = input.partial_reasoning_standard.trim().to_string();
    let partial_reasoning_inline = input.partial_reasoning_inline.trim().to_string();
    let build_stop_result =
        |persisted: bool,
         conversation_id: Option<String>,
         assistant_message: Option<ChatMessage>|
         -> StopChatResult {
            StopChatResult {
                aborted,
                persisted,
                conversation_id,
                assistant_text: partial_assistant_text.clone(),
                reasoning_standard: partial_reasoning_standard.clone(),
                reasoning_inline: partial_reasoning_inline.clone(),
                assistant_message,
            }
        };
    let should_persist = !partial_assistant_text.is_empty()
        || !partial_reasoning_standard.is_empty()
        || !partial_reasoning_inline.is_empty();
    if !should_persist {
        return Ok(build_stop_result(false, None, None));
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    let api_config_id = if let Some(_conversation_id) = requested_conversation_id.as_deref() {
        requested_department_id
            .as_deref()
            .and_then(|id| department_by_id(&app_config, id))
            .map(department_primary_api_config_id)
            .or_else(|| {
                department_for_agent_id(&app_config, &agent_id).map(department_primary_api_config_id)
            })
            .or_else(|| resolve_selected_api_config(&app_config, None).map(|api| api.id.clone()))
            .ok_or_else(|| "Missing available API config for stop request".to_string())?
    } else {
        requested_department_id
            .as_deref()
            .and_then(|id| department_by_id(&app_config, id))
            .map(department_primary_api_config_id)
            .or_else(|| {
                department_for_agent_id(&app_config, &agent_id).map(department_primary_api_config_id)
            })
            .or_else(|| resolve_selected_api_config(&app_config, None).map(|api| api.id.clone()))
            .ok_or_else(|| "Missing available API config for stop request".to_string())?
    };
    let selected_api = app_config
        .api_configs
        .iter()
        .find(|api| api.id == api_config_id)
        .cloned()
        .ok_or_else(|| format!("Selected API config '{api_config_id}' not found."))?;
    let runtime_requested = requested_conversation_id
        .as_deref()
        .filter(|conversation_id| {
            delegate_runtime_thread_conversation_get(state.inner(), conversation_id)
                .ok()
                .flatten()
                .is_some()
        })
        .map(ToOwned::to_owned);
    let mut runtime_conversation = if let Some(conversation_id) = runtime_requested.as_deref() {
        delegate_runtime_thread_conversation_get(state.inner(), conversation_id)?
    } else {
        None
    };
    let idx = if runtime_conversation.is_some() {
        None
    } else {
        latest_active_conversation_index(&data, "", &agent_id)
    };
    let conversation = if let Some(conversation) = runtime_conversation.as_mut() {
        conversation
    } else {
        let Some(idx) = idx else {
            drop(guard);
            return Ok(build_stop_result(false, None, None));
        };
        data.conversations
            .get_mut(idx)
            .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?
    };

    // If the latest message is already an assistant message, do not append duplicate partial output.
    if conversation
        .messages
        .last()
        .map(|m| m.role == "assistant")
        .unwrap_or(false)
    {
        let conversation_id = conversation.id.clone();
        let assistant_message = conversation.messages.last().cloned();
        drop(guard);
        return Ok(build_stop_result(false, Some(conversation_id), assistant_message));
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
    let assistant_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now.clone(),
        speaker_agent_id: Some(agent_id.clone()),
        parts: vec![MessagePart::Text {
            text: partial_assistant_text.clone(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta,
        tool_call: None,
        mcp_call: None,
    };
    conversation.messages.push(assistant_message.clone());
    conversation.updated_at = now.clone();
    conversation.last_assistant_at = Some(now);
    conversation.last_context_usage_ratio =
        compute_context_usage_ratio(conversation, selected_api.context_window_tokens);
    let conversation_id = conversation.id.clone();

    if let Some(conversation) = runtime_conversation {
        delegate_runtime_thread_conversation_update(state.inner(), &conversation_id, conversation)?;
    } else {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);

    Ok(build_stop_result(true, Some(conversation_id), Some(assistant_message)))
}

#[tauri::command]
async fn get_chat_queue_snapshot(
    state: State<'_, AppState>,
) -> Result<Vec<ChatQueueEventSummary>, String> {
    get_queue_snapshot(state.inner())
}

#[tauri::command]
async fn remove_chat_queue_event(
    event_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let removed = remove_from_queue(state.inner(), &event_id)?;
    Ok(removed.is_some())
}

#[tauri::command]
async fn get_main_session_state_snapshot(
    state: State<'_, AppState>,
) -> Result<MainSessionState, String> {
    get_main_session_state(state.inner())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueLocalFileAttachmentInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueInlineFileAttachmentInput {
    file_name: String,
    #[serde(default)]
    mime: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueLocalFileAttachmentOutput {
    mime: String,
    file_name: String,
    saved_path: String,
    attach_as_media: bool,
    #[serde(default)]
    bytes_base64: Option<String>,
    text_notice: String,
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

fn workspace_downloads_dir(state: &AppState) -> PathBuf {
    // downloads 是用户与 LLM 共用的附件落地区；允许 LLM 后续自行清理和管理空间占用。
    state.llm_workspace_path.join("downloads")
}

fn media_extension_from_mime_for_download(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/heic" => "heic",
        "image/heif" => "heif",
        "image/svg+xml" => "svg",
        "audio/wav" => "wav",
        "audio/x-wav" => "wav",
        "audio/mpeg" => "mp3",
        "audio/mp3" => "mp3",
        "audio/mp4" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "audio/flac" => "flac",
        _ => "bin",
    }
}

fn is_dangerous_executable_extension(ext: &str) -> bool {
    matches!(
        ext.trim().to_ascii_lowercase().as_str(),
        "bat"
            | "cmd"
            | "ps1"
            | "psm1"
            | "psd1"
            | "vbs"
            | "js"
            | "jse"
            | "wsf"
            | "wsh"
            | "hta"
            | "msi"
            | "com"
            | "exe"
            | "scr"
            | "pif"
    )
}

fn should_force_bin_by_file_name(file_name: &str) -> bool {
    std::path::Path::new(file_name.trim())
        .extension()
        .and_then(|v| v.to_str())
        .map(is_dangerous_executable_extension)
        .unwrap_or(false)
}

fn apply_download_extension_policy(file_name: &str, mime: &str) -> String {
    let normalized = sanitize_download_file_name(file_name);
    if should_force_bin_by_file_name(&normalized) {
        let stem = std::path::Path::new(&normalized)
            .file_stem()
            .and_then(|v| v.to_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("attachment");
        return format!("{stem}.bin");
    }
    let ext = media_extension_from_mime_for_download(mime);
    if should_append_download_extension(&normalized, ext) {
        format!("{normalized}.{ext}")
    } else {
        normalized
    }
}

fn should_append_download_extension(file_name: &str, ext: &str) -> bool {
    let file_name = file_name.trim();
    if file_name.is_empty() || ext.trim().is_empty() {
        return false;
    }
    if ext.eq_ignore_ascii_case("bin") {
        let has_existing_ext = std::path::Path::new(file_name)
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if has_existing_ext {
            return false;
        }
    }
    !file_name
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", ext.to_ascii_lowercase()))
}

fn sanitize_download_file_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return format!("attachment-{}", Uuid::new_v4());
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        let blocked = matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
        if blocked || ch.is_control() {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let normalized = out.trim().trim_matches('.').trim().to_string();
    if normalized.is_empty() {
        format!("attachment-{}", Uuid::new_v4())
    } else {
        normalized
    }
}

fn persist_raw_attachment_to_downloads(
    state: &AppState,
    suggested_name: &str,
    mime: &str,
    raw: &[u8],
) -> Result<PathBuf, String> {
    let dir = workspace_downloads_dir(state);
    fs::create_dir_all(&dir).map_err(|err| format!("Create downloads dir failed: {err}"))?;

    let file_name = apply_download_extension_policy(suggested_name, mime);
    let target = dir.join(file_name);
    let final_target = if target.exists() {
        let stem = target
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("attachment");
        let ext = target.extension().and_then(|v| v.to_str()).unwrap_or("bin");
        dir.join(format!("{stem}-{}.{}", Uuid::new_v4(), ext))
    } else {
        target
    };
    fs::write(&final_target, raw).map_err(|err| format!("Write attachment failed: {err}"))?;
    Ok(final_target)
}

fn workspace_relative_path(state: &AppState, absolute: &std::path::Path) -> String {
    absolute
        .strip_prefix(&state.llm_workspace_path)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| absolute.to_string_lossy().replace('\\', "/"))
}

fn build_attachment_notice_text(file_name: &str, relative_path: &str) -> String {
    format!(
        "用户本次上传了一个附件：{file_name}\n保存到了你工作区的downloads文件夹内（路径：{relative_path}）\n如果需要，请使用 shell 工具读取该文件内容。"
    )
}

fn queue_attachment_from_raw(
    state: &AppState,
    file_name_input: &str,
    mime_input: &str,
    raw: &[u8],
) -> Result<QueueLocalFileAttachmentOutput, String> {
    let file_name = file_name_input
        .trim()
        .trim_matches(['\\', '/'])
        .trim()
        .to_string();
    let file_name = if file_name.is_empty() {
        "attachment".to_string()
    } else {
        file_name
    };
    let mime = if mime_input.trim().is_empty() {
        media_mime_from_path(std::path::Path::new(&file_name))
            .unwrap_or("application/octet-stream")
            .to_string()
    } else {
        mime_input.trim().to_ascii_lowercase()
    };
    let attach_as_media = matches!(
        mime.as_str(),
        "application/pdf"
            | "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
    ) && raw.len() <= MAX_MULTIMODAL_BYTES;

    // 入队即落盘：附件进入队列后立刻可在 downloads 查看与复查。
    let saved_path = persist_raw_attachment_to_downloads(state, &file_name, &mime, raw)?;
    let final_saved_path = workspace_relative_path(state, &saved_path);
    let final_file_name = saved_path
        .file_name()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(file_name.as_str())
        .to_string();

    let bytes_base64 = if attach_as_media {
        Some(B64.encode(raw))
    } else {
        None
    };
    let text_notice = build_attachment_notice_text(&final_file_name, &final_saved_path);
    Ok(QueueLocalFileAttachmentOutput {
        mime,
        file_name: final_file_name,
        saved_path: final_saved_path,
        attach_as_media,
        bytes_base64,
        text_notice,
    })
}

fn normalize_payload_attachments(
    raw: Option<&Vec<AttachmentMetaInput>>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::<serde_json::Value>::new();
    let Some(items) = raw else {
        return out;
    };
    for item in items {
        let file_name = String::from(item.file_name.trim());
        let relative_path = String::from(item.relative_path.trim()).replace('\\', "/");
        let mime = String::from(item.mime.trim());
        if file_name.is_empty() || relative_path.is_empty() {
            continue;
        }
        out.push(serde_json::json!({
            "fileName": file_name,
            "relativePath": relative_path,
            "mime": mime,
        }));
    }
    out
}

fn merge_provider_meta_with_attachments(
    provider_meta: Option<Value>,
    attachments: &[Value],
) -> Option<Value> {
    let mut merged = provider_meta.unwrap_or_else(|| serde_json::json!({}));
    if !merged.is_object() {
        merged = serde_json::json!({});
    }
    if attachments.is_empty() {
        return if merged.as_object().map(|v| v.is_empty()).unwrap_or(true) {
            None
        } else {
            Some(merged)
        };
    }
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("attachments".to_string(), Value::Array(attachments.to_vec()));
    }
    Some(merged)
}

fn persist_payload_images_to_workspace_downloads(
    state: &AppState,
    images: &[BinaryPart],
) -> Vec<String> {
    let mut notices = Vec::<String>::new();
    for (idx, image) in images.iter().enumerate() {
        if let Some(saved_path) = image
            .saved_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let file_name = std::path::Path::new(saved_path)
                .file_name()
                .and_then(|v| v.to_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("attachment")
                .to_string();
            notices.push(build_attachment_notice_text(&file_name, saved_path));
            continue;
        }
        let mime = image.mime.trim();
        if mime.is_empty() {
            continue;
        }
        let Ok(raw) = B64.decode(image.bytes_base64.trim()) else {
            eprintln!("[CHAT] skip persist image to downloads: invalid base64, index={idx}");
            continue;
        };
        let suggested = format!("queued-image-{}", idx + 1);
        match persist_raw_attachment_to_downloads(state, &suggested, mime, &raw) {
            Ok(saved_path) => {
                let relative = workspace_relative_path(state, &saved_path);
                let file_name = saved_path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or(&suggested)
                    .to_string();
                notices.push(build_attachment_notice_text(&file_name, &relative));
            }
            Err(err) => {
                eprintln!("[CHAT] persist queued image to downloads failed: index={}, err={}", idx, err);
            }
        }
    }
    notices
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

#[tauri::command]
fn queue_local_file_attachment(
    input: QueueLocalFileAttachmentInput,
    state: State<'_, AppState>,
) -> Result<QueueLocalFileAttachmentOutput, String> {
    let path_text = input.path.trim();
    if path_text.is_empty() {
        return Err("File path is empty.".to_string());
    }
    let path = std::path::PathBuf::from(path_text);
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("attachment")
        .to_string();
    let raw = fs::read(&path).map_err(|err| format!("Read file failed: {err}"))?;
    let mime = media_mime_from_path(&path)
        .unwrap_or("application/octet-stream")
        .to_string();
    queue_attachment_from_raw(state.inner(), &file_name, &mime, &raw)
}

#[tauri::command]
fn queue_inline_file_attachment(
    input: QueueInlineFileAttachmentInput,
    state: State<'_, AppState>,
) -> Result<QueueLocalFileAttachmentOutput, String> {
    if input.bytes_base64.trim().is_empty() {
        return Err("Attachment payload is empty.".to_string());
    }
    let raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode attachment base64 failed: {err}"))?;
    queue_attachment_from_raw(
        state.inner(),
        input.file_name.trim(),
        input.mime.trim(),
        &raw,
    )
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

fn model_id_exact_match(requested_model: &str, candidate_model: &str) -> bool {
    let requested = requested_model.trim();
    let candidate = candidate_model.trim();
    if requested.is_empty() || candidate.is_empty() {
        return false;
    }
    if candidate == requested || candidate.eq_ignore_ascii_case(requested) {
        return true;
    }
    for sep in ['/', ':'] {
        if let Some((_, suffix)) = candidate.split_once(sep) {
            let suffix = suffix.trim();
            if suffix == requested || suffix.eq_ignore_ascii_case(requested) {
                return true;
            }
        }
    }
    let requested_norm = normalize_model_id(requested);
    let candidate_norm = normalize_model_id(candidate);
    if requested_norm.is_empty() || candidate_norm.is_empty() {
        return false;
    }
    if candidate_norm == requested_norm {
        return true;
    }
    for sep in ['/', ':'] {
        if let Some((_, suffix)) = candidate.split_once(sep) {
            let suffix_norm = normalize_model_id(suffix.trim());
            if suffix_norm == requested_norm {
                return true;
            }
        }
    }
    false
}

fn normalize_model_id(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>()
}

#[tauri::command]
async fn fetch_model_metadata(
    input: FetchModelMetadataInput,
) -> Result<FetchModelMetadataOutput, String> {
    let requested_model = input.model.trim();
    if requested_model.is_empty() {
        return Err("Model is empty.".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let resp = client
        .get("https://models.dev/api.json")
        .send()
        .await
        .map_err(|err| format!("Fetch models.dev metadata failed: {err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let snippet = body.chars().take(400).collect::<String>();
        return Err(format!(
            "Fetch models.dev metadata failed: {status} | {snippet}"
        ));
    }
    let root = resp
        .json::<Value>()
        .await
        .map_err(|err| format!("Parse models.dev metadata failed: {err}"))?;
    let providers = root
        .as_object()
        .ok_or_else(|| "Invalid models.dev payload: expected root object.".to_string())?;
    let mut best_match: Option<FetchModelMetadataOutput> = None;
    let mut best_sort_key: Option<(u8, u8, u64, u32, u32, u8)> = None;
    for provider_obj in providers.values().filter_map(Value::as_object) {
        let Some(models_obj) = provider_obj.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_id, model_value) in models_obj
            .iter()
            .filter(|(model_id, _)| model_id_exact_match(requested_model, model_id))
        {
            let Some(model_obj) = model_value.as_object() else {
                continue;
            };
            let limit_obj = model_obj.get("limit").and_then(Value::as_object);
            let context_window_tokens = limit_obj
                .and_then(|limit| limit.get("context"))
                .and_then(Value::as_u64)
                .map(|v| v.min(u64::from(u32::MAX)) as u32);
            let max_output_tokens = limit_obj
                .and_then(|limit| limit.get("output"))
                .and_then(Value::as_u64)
                .map(|v| v.min(u64::from(u32::MAX)) as u32);
            let input_modalities = model_obj
                .get("modalities")
                .and_then(Value::as_object)
                .and_then(|modalities| modalities.get("input"))
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|s| s.to_ascii_lowercase())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let enable_image = input_modalities.iter().any(|item| item.contains("image"));
            let enable_audio = input_modalities.iter().any(|item| item.contains("audio"));
            let enable_tools = model_obj
                .get("tool_call")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            // 冲突时优先“可用信息更完整”，其次“上限更大”。
            let numeric_count = context_window_tokens.is_some() as u8 + max_output_tokens.is_some() as u8;
            let capability_count = enable_image as u8 + enable_tools as u8 + enable_audio as u8;
            let has_any_usable = (numeric_count > 0 || capability_count > 0) as u8;
            let context_value = context_window_tokens.unwrap_or(0);
            let output_value = max_output_tokens.unwrap_or(0);
            let size_sum = u64::from(context_value) + u64::from(output_value);
            let candidate_sort_key = (
                has_any_usable,
                numeric_count,
                size_sum,
                context_value,
                output_value,
                capability_count,
            );

            let should_take = match best_sort_key {
                None => true,
                Some(current) => candidate_sort_key > current,
            };
            if should_take {
                best_sort_key = Some(candidate_sort_key);
                best_match = Some(FetchModelMetadataOutput {
                    found: true,
                    matched_model_id: Some(model_id.to_string()),
                    context_window_tokens,
                    max_output_tokens,
                    enable_image: Some(enable_image),
                    enable_tools: Some(enable_tools),
                    enable_audio: Some(enable_audio),
                });
            }
        }
    }
    let Some(best_match) = best_match else {
        return Ok(FetchModelMetadataOutput {
            found: false,
            matched_model_id: None,
            context_window_tokens: None,
            max_output_tokens: None,
            enable_image: None,
            enable_tools: None,
            enable_audio: None,
        });
    };
    Ok(best_match)
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
    let mut config = state_read_config_cached(&state)?;
    normalize_api_tools(&mut config);
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    merge_private_organization_into_runtime_data(&state.data_path, &mut config, &mut data)?;
    drop(guard);

    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "缺少人格 ID".to_string())?;
    let selected_agent = data
        .agents
        .iter()
        .find(|item| item.id == target_agent_id)
        .cloned()
        .ok_or_else(|| format!("未找到人格：{target_agent_id}"))?;
    let current_department = department_for_agent_id(&config, &target_agent_id);

    let effective_api_id = department_for_agent_id(&config, &target_agent_id)
        .map(|item| item.api_config_id.clone())
        .or_else(|| input.api_config_id.clone());
    let selected = effective_api_id
        .as_deref()
        .and_then(|api_id| resolve_selected_api_config(&config, Some(api_id)));

    if selected.is_none() {
        return Ok(selected_agent
            .tools
            .iter()
            .map(|tool| {
                let restricted_reason = tool_restricted_by_department(current_department, &tool.id);
                let detail = if let Some(reason) = restricted_reason.clone() {
                    reason
                } else if matches!(tool.id.as_str(), "screenshot" | "wait") {
                    "当前人格尚未委任部门，需绑定支持图像的部门模型后才能运行。".to_string()
                } else if tool.enabled {
                    "当前人格已启用该工具，但尚未委任部门，暂不校验运行模型。".to_string()
                } else {
                    "当前人格未启用该工具。".to_string()
                };
                ToolLoadStatus {
                    id: tool.id.clone(),
                    status: if restricted_reason.is_some() {
                        "unavailable".to_string()
                    } else if tool.enabled {
                        if matches!(tool.id.as_str(), "screenshot" | "wait") {
                            "unavailable".to_string()
                        } else {
                            "loaded".to_string()
                        }
                    } else {
                        "disabled".to_string()
                    },
                    detail,
                }
            })
            .collect());
    }
    let selected = selected.ok_or_else(|| "No API config configured. Please add one.".to_string())?;

    if !selected.enable_tools {
        return Ok(selected_agent
            .tools
            .iter()
            .map(|tool| ToolLoadStatus {
                id: tool.id.clone(),
                status: "disabled".to_string(),
                detail: "当前模型未启用工具调用。".to_string(),
            })
            .collect());
    }

    let runtime_shell = terminal_shell_for_state(&state);
    let mut statuses = Vec::new();
    for tool in selected_agent.tools {
        if let Some(reason) = tool_restricted_by_department(current_department, &tool.id) {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "unavailable".to_string(),
                detail: reason,
            });
            continue;
        }
        if !tool.enabled {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "disabled".to_string(),
                detail: "当前人格未启用该工具。".to_string(),
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
            "organize_context" => (
                "loaded".to_string(),
                "整理上下文工具可用".to_string(),
            ),
            "task" => (
                "loaded".to_string(),
                "任务工具可用".to_string(),
            ),
            "delegate" => (
                "loaded".to_string(),
                "委托工具可用".to_string(),
            ),
            "remote_im_send" => (
                "loaded".to_string(),
                "远程IM发送工具可用".to_string(),
            ),
            "exec" => {
                #[cfg(target_os = "windows")]
                {
                    if runtime_shell.kind == "missing-terminal-shell" {
                        (
                            "unavailable".to_string(),
                            "未检测到可用终端。请安装 PowerShell 7（推荐）/ Windows PowerShell 5.1 / Git Bash。"
                                .to_string(),
                        )
                    } else {
                        (
                            "loaded".to_string(),
                            format!("执行工具可用（{}）", terminal_shell_runtime_label(&runtime_shell)),
                        )
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    (
                        "loaded".to_string(),
                        format!("执行工具可用（{}）", terminal_shell_runtime_label(&runtime_shell)),
                    )
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
    let data = state_read_app_data_cached(&state)?;
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
    let mut data = state_read_app_data_cached(&state)?;
    data.image_text_cache.clear();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);

    Ok(ImageTextCacheStats {
        entries: 0,
        total_chars: 0,
        latest_updated_at: None,
    })
}
