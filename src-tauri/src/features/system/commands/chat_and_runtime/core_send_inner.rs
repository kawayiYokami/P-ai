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
                        describe_image_with_vision_api(&state, &vision_resolved, &vision_api, image)
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
                message: Some("正在整理上下文...".to_string()),
            });
        }

        let archive_res = run_context_compaction_pipeline(
            &state,
            &selected_api,
            &resolved_api,
            &source,
            &effective_agent_id,
            &pending_archive_reason,
            "COMPACTION-AUTO",
        )
        .await;

        match archive_res {
            Ok(result) => {
                archived_before_send = result.archived;
                if pending_archive_forced {
                    let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                        "整理完成，将继续当前会话。".to_string()
                    } else {
                        format!(
                            "整理完成（降级摘要）：{}",
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
                        message: Some(format!("整理失败：{err}")),
                    });
                }
                return Err(format!("整理失败：{err}"));
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
        if let Some(selected_idx) = idx {
            for (i, conversation) in data.conversations.iter_mut().enumerate() {
                if conversation_is_delegate(conversation) || !conversation.summary.trim().is_empty() {
                    continue;
                }
                conversation.status = if i == selected_idx {
                    "active".to_string()
                } else {
                    "inactive".to_string()
                };
            }
        }

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
                .find(|c| !conversation_is_delegate(c) && !c.summary.trim().is_empty())
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
                let relative_path = item
                    .get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if relative_path.is_empty() {
                    continue;
                }
                chat_overrides.latest_user_extra_blocks.push(format!(
                    "用户上传了附件，文件位于你工作区的 downloads 目录（路径：{}）。\n你可以先用 shell 工具定位或查看基础文件信息；具体解析方式应按文件类型选择合适 skill 或在线检索正确方法。\n仅当用户明确要求处理该附件时再处理；若用户未明确要求，请先询问用户想如何处理。",
                    relative_path
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
            Some(&state),
            Some(&resolved_api),
            Some(data.pdf_read_mode == "image" && selected_api.enable_image),
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
            message: Some("回复后上下文已达到 82%，正在自动整理...".to_string()),
        });
        let archive_res = run_context_compaction_pipeline(
            &state,
            &active_selected_api,
            &active_resolved_api,
            &source,
            &effective_agent_id,
            "force_context_usage_82_after_reply",
            "COMPACTION-AFTER-REPLY",
        )
        .await;
        match archive_res {
            Ok(result) => {
                let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                    "自动整理完成，将继续当前会话。".to_string()
                } else {
                    format!("自动整理完成（降级摘要）：{}", result.warning.unwrap_or_default())
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
                    message: Some(format!("自动整理失败：{err}")),
                });
                return Err(format!("自动整理失败：{err}"));
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
            "[聊天] 清理进行中工具中断句柄失败 (session={}): {}",
            chat_key, err
        );
    }
    let final_result = match result {
        Ok(inner) => inner,
        Err(_) => {
            eprintln!(
                "[聊天] 用户中止聊天请求 (session={})",
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
    // 兜底催处理：归档阶段可能短暂阻塞出队，这里在当前轮次结束后补一次调度触发。
    trigger_chat_queue_processing(state);
    final_result
}
