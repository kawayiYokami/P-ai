fn normalized_session_search_keyword(keyword: Option<&str>) -> Option<String> {
    keyword
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn session_search_hit(haystacks: &[String], keyword: Option<&str>) -> bool {
    let Some(keyword) = normalized_session_search_keyword(keyword) else {
        return true;
    };
    haystacks.iter().any(|value| {
        let normalized = value.trim().to_lowercase();
        !normalized.is_empty() && normalized.contains(&keyword)
    })
}

fn session_notification_source_label(
    title: &str,
    persona_name: Option<&str>,
) -> String {
    let title = title.trim();
    let persona_name = persona_name.unwrap_or("").trim();
    let left = if title.is_empty() { "未命名会话" } else { title };
    let right = if persona_name.is_empty() { "未绑定人格" } else { persona_name };
    format!("[{}·{}]", left, right)
}

fn delegate_completion_notification_label(persona_name: Option<&str>) -> String {
    let persona_name = persona_name.unwrap_or("").trim();
    if persona_name.is_empty() {
        "未绑定人格".to_string()
    } else {
        persona_name.to_string()
    }
}

fn build_delegate_completion_notification_body(
    state: &AppState,
    target_agent_id: &str,
    delegate_title: &str,
    content: &str,
) -> Result<String, String> {
    let normalized_title = delegate_title.trim();
    if normalized_title.is_empty() {
        return Err("委托标题不能为空".to_string());
    }
    let normalized_content = content.trim();
    if normalized_content.is_empty() {
        return Err("通知正文不能为空".to_string());
    }
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let persona_name = runtime_snapshot
        .agents
        .iter()
        .find(|agent| agent.id.trim() == target_agent_id.trim())
        .map(|agent| agent.name.trim().to_string());
    let label = delegate_completion_notification_label(persona_name.as_deref());
    Ok(format!(
        "{label}的{normalized_title}委托执行成功，以下是汇报内容：\n{normalized_content}"
    ))
}

fn build_session_notification_body(
    state: &AppState,
    source_conversation_id: &str,
    content: &str,
) -> Result<String, String> {
    let normalized_conversation_id = source_conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    let normalized_content = content.trim();
    if normalized_content.is_empty() {
        return Err("通知正文不能为空".to_string());
    }
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let conversation_meta = conversation_service_v2()
        .get_conversation_meta(state, normalized_conversation_id)
        .map_err(|_| "来源会话不存在".to_string())?;
    let persona_name = runtime_snapshot
        .agents
        .iter()
        .find(|agent| agent.id.trim() == conversation_meta.agent_id.trim())
        .map(|agent| agent.name.trim().to_string())
        .filter(|value| !value.is_empty());
    let label = session_notification_source_label(
        &conversation_meta.title,
        persona_name.as_deref(),
    );
    Ok(format!("{label}:{normalized_content}"))
}

fn build_session_notification_message(text: &str) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text {
            text: text.to_string(),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "messageKind": "session_notification",
            "sessionNotification": true,
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

fn selected_messages_notification_content(selected_messages: &[ChatMessage]) -> String {
    selected_messages
        .iter()
        .map(|message| {
            let speaker = match message.role.trim().to_ascii_lowercase().as_str() {
                "user" => "用户",
                "assistant" => "助手",
                "system" => "系统",
                "tool" => "工具",
                _ => "消息",
            };
            let text = message
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.trim()),
                    _ => None,
                })
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() {
                format!("[{speaker}]: [空消息]")
            } else {
                format!("[{speaker}]: {text}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Debug, Clone)]
struct SessionNotificationDispatchRequest {
    state: AppState,
    target_conversation_id: String,
    body: String,
    message: ChatMessage,
    action: String,
}

fn session_notification_body_preview(body: &str) -> String {
    let preview = body.trim().chars().take(60).collect::<String>();
    if preview.is_empty() {
        "[空正文]".to_string()
    } else {
        preview
    }
}

fn session_notification_dispatch_sender(
) -> Result<&'static std::sync::mpsc::Sender<SessionNotificationDispatchRequest>, String> {
    static SENDER: OnceLock<std::sync::mpsc::Sender<SessionNotificationDispatchRequest>> =
        OnceLock::new();
    if let Some(sender) = SENDER.get() {
        return Ok(sender);
    }

    let (tx, rx) = std::sync::mpsc::channel::<SessionNotificationDispatchRequest>();
    std::thread::Builder::new()
        .name("session-notification-worker".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("session-notification-async")
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    runtime_log_error(format!(
                        "[会话通知] 失败，任务=启动后台投递 worker，error={err}"
                    ));
                    return;
                }
            };
            runtime_log_info("[会话通知] 完成，任务=启动后台投递 worker".to_string());
            for request in rx {
                runtime_log_info(format!(
                    "[会话通知] 开始，任务=接收投递请求，action={}，target_conversation_id={}，message_id={}，body_preview={}",
                    request.action,
                    request.target_conversation_id,
                    request.message.id,
                    session_notification_body_preview(&request.body)
                ));
                runtime.spawn(async move {
                    let outcome =
                        futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(
                            process_session_notification_dispatch_request(request),
                        ))
                        .await;
                    match outcome {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => runtime_log_error(format!(
                            "[会话通知] 失败，任务=执行会话投递，error={err}"
                        )),
                        Err(panic_payload) => {
                            let panic_text = panic_payload
                                .downcast_ref::<&str>()
                                .map(|text| (*text).to_string())
                                .or_else(|| {
                                    panic_payload.downcast_ref::<String>().cloned()
                                })
                                .unwrap_or_else(|| "未知 panic".to_string());
                            runtime_log_error(format!(
                                "[会话通知] 失败，任务=执行会话投递，原因=投递协程 panic，panic={panic_text}"
                            ));
                        }
                    }
                });
            }
            runtime_log_warn("[会话通知] 跳过，任务=后台投递 worker，原因=请求通道已关闭".to_string());
        })
        .map_err(|err| format!("启动会话通知 worker 失败: {err}"))?;

    let _ = SENDER.set(tx);
    SENDER
        .get()
        .ok_or_else(|| "会话通知 worker 启动后未注册 sender".to_string())
}

fn enqueue_session_notification_dispatch(
    state: &AppState,
    target_conversation_id: &str,
    body: &str,
    message: &ChatMessage,
    action: &str,
) -> Result<(), String> {
    let normalized_target_conversation_id = target_conversation_id.trim();
    let sender = session_notification_dispatch_sender()?;
    runtime_log_info(format!(
        "[会话通知] 开始，任务=投递请求入队，action={}，target_conversation_id={}，message_id={}，body_preview={}",
        action,
        normalized_target_conversation_id,
        message.id,
        session_notification_body_preview(body)
    ));
    sender
        .send(SessionNotificationDispatchRequest {
            state: state.clone(),
            target_conversation_id: normalized_target_conversation_id.to_string(),
            body: body.to_string(),
            message: message.clone(),
            action: action.to_string(),
        })
        .map_err(|err| format!("投递会话通知入队失败: {err}"))?;
    runtime_log_info(format!(
        "[会话通知] 完成，任务=投递请求入队，action={}，target_conversation_id={}，message_id={}",
        action,
        normalized_target_conversation_id,
        message.id
    ));
    Ok(())
}

async fn process_session_notification_dispatch_request(
    request: SessionNotificationDispatchRequest,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[会话通知] 开始，任务=执行会话投递，action={}，target_conversation_id={}，message_id={}",
        request.action,
        request.target_conversation_id,
        request.message.id
    ));

    let service = conversation_service_v2();
    let target_conversation_id = request.target_conversation_id.trim().to_string();
    let target_conversation_meta = service
        .get_conversation_meta(&request.state, &target_conversation_id)
        .map_err(|err| {
            runtime_log_error(format!(
                "[会话通知] 失败，任务=目标会话元数据读取，action={}，target_conversation_id={}，message_id={}，error={err}",
                request.action,
                target_conversation_id,
                request.message.id
            ));
            format!("目标会话元数据读取失败: {err}")
        })?;
    runtime_log_info(format!(
        "[会话通知] 节点，任务=投递元数据读取完成，action={}，target_conversation_id={}，message_id={}，is_remote_im_contact={}，kind={}，duration_ms={}",
        request.action,
        target_conversation_id,
        request.message.id,
        target_conversation_meta.is_remote_im_contact,
        target_conversation_meta.conversation_kind.trim(),
        started_at.elapsed().as_millis()
    ));

    if target_conversation_meta.is_remote_im_contact {
        // 远程联系人保持原投递：渠道外发 + 历史写入 + 前端事件
        service
            .deliver_session_notification(
                &request.state,
                &target_conversation_id,
                &request.body,
                &request.message,
                &request.action,
            )
            .await?;
        runtime_log_info(format!(
            "[会话通知] 完成，任务=会话间投递，action={}，target_conversation_id={}，message_id={}，duration_ms={}",
            request.action,
            target_conversation_id,
            request.message.id,
            started_at.elapsed().as_millis()
        ));
        return Ok(());
    }

    // 运行时能力判据：未归档的本地 chat / side_chat 会话都能收通知（分支会话是 side_chat）
    if !service
        .conversation_meta_is_local_conversation_runtime_meta_view(&target_conversation_meta)
    {
        return Err(format!(
            "目标会话不支持通知投递，kind={}",
            target_conversation_meta.conversation_kind.trim()
        ));
    }

    // 本地普通会话：系统引导消息入队（Guided），会话空闲时插话并激活主助理开新回合
    let conversation = service
        .get_conversation_metadata_record(&request.state, &target_conversation_id)?;
    let mut runtime_context =
        runtime_context_new("session_notification", "session_notification");
    runtime_context.request_id = Some(format!(
        "session-notification-request-{}",
        Uuid::new_v4()
    ));
    runtime_context.dispatch_id = Some(format!(
        "session-notification-dispatch-{}",
        Uuid::new_v4()
    ));
    runtime_context.target_conversation_id = Some(target_conversation_id.clone());
    runtime_context.root_conversation_id = conversation
        .root_conversation_id
        .clone()
        .or_else(|| Some(target_conversation_id.clone()));
    runtime_context.executor_agent_id = Some(conversation.agent_id.clone());
    let event = ChatPendingEvent {
        id: format!("session-notification-{}", Uuid::new_v4()),
        conversation_id: target_conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::System,
        queue_mode: ChatQueueMode::Guided,
        messages: vec![request.message.clone()],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: conversation.agent_id.clone(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    let ingress = ingress_chat_event(&request.state, event)?;
    let ingress_kind = match &ingress {
        ChatEventIngress::Direct(_) => "direct",
        ChatEventIngress::Queued { .. } => "queued",
    };
    runtime_log_info(format!(
        "[会话通知] 节点，任务=引导消息入队，action={}，target_conversation_id={}，message_id={}，ingress={}，duration_ms={}",
        request.action,
        target_conversation_id,
        request.message.id,
        ingress_kind,
        started_at.elapsed().as_millis()
    ));
    trigger_chat_event_after_ingress_with_delay(
        &request.state,
        ingress,
        std::time::Duration::from_secs(1),
    );
    runtime_log_info(format!(
        "[会话通知] 完成，任务=会话间投递，action={}，target_conversation_id={}，message_id={}，duration_ms={}",
        request.action,
        target_conversation_id,
        request.message.id,
        started_at.elapsed().as_millis()
    ));
    Ok(())
}
