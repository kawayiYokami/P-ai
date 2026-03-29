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
    let (conversation_id, department_id, model_config_id) = {
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
                    && conv.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conv)
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
                        if !conv.summary.trim().is_empty() {
                            "summary_present"
                        } else if !conversation_visible_in_foreground_lists(conv) {
                            "background_conversation"
                        } else {
                            "unknown"
                        }
                    })
                    .unwrap_or("not_found");
                eprintln!(
                    "[聊天] 会话 conversation_id 被拒绝，已选择回退会话: requested_cid={}, reject_reason={}, fallback_cid={}, department_id={}, agent_id={}",
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
        (conversation_id, department.id.clone(), api_config_id)
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
    let request_id = runtime_context_request_id_or_new(None, input.trace_id.as_deref(), "chat");
    let mut runtime_context = runtime_context_new("user_message", "user_send");
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.clone());
    runtime_context.target_conversation_id = Some(conversation_id.clone());
    runtime_context.root_conversation_id = Some(conversation_id.clone());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.executor_department_id = Some(department_id.clone());
    runtime_context.model_config_id = Some(model_config_id.clone());
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
        runtime_context: Some(runtime_context.clone()),
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

    // 入队前先做阻塞判定：空闲且无排队则直写历史；否则入队。
    let ingress = match ingress_chat_event(state.inner(), event) {
        Ok(value) => value,
        Err(err) => {
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
    };

    let queue_len = total_queue_len(state.inner()).unwrap_or_default();
    let ingress_mode = match &ingress {
        ChatEventIngress::Direct(_) => "direct",
        ChatEventIngress::Queued { .. } => "queued",
    };
    eprintln!(
        "[聊天调度] 用户消息已接入调度: mode={}, event_id={}, request_id={}, conversation_id={}, department_id={}, agent_id={}, queue_len={}, main_session_state={}",
        ingress_mode,
        event_id,
        request_id,
        conversation_id,
        department_id,
        agent_id,
        queue_len,
        main_session_state_text,
    );

    // 根据 ingress 结果执行：直写或排队；排队仅在事件仍滞留时才通知前端。
    process_chat_event_after_ingress(state.inner(), ingress).await;

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
    runtime_log_info(format!(
        "[聊天推送] 收到前端绑定请求: window={}, conversation_id={}",
        window_label,
        conversation_id.unwrap_or("*")
    ));
    if let Some(conversation_id) = conversation_id {
        set_active_chat_view_stream_binding(
            state.inner(),
            &window_label,
            Some(conversation_id),
            on_delta,
        )?;
        runtime_log_debug(format!(
            "[聊天] 已绑定活动聊天流: window={}, conversation_id={}",
            window_label, conversation_id
        ));
    } else {
        // 空会话视图仍保留绑定，作为单窗口通配接收端，避免远程消息落地后前端无推送。
        set_active_chat_view_stream_binding(
            state.inner(),
            &window_label,
            Some("*"),
            on_delta,
        )?;
        runtime_log_info(format!(
            "[聊天调度] 活动聊天流已通配绑定：window={}",
            window_label,
        ));
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
            "[聊天] 停止请求已级联到同步委托子会话: session={}, child_count={}",
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
