// ==================== 外部激活：把一句话送进会话 ====================
//
// Webhook / 外部脚本凭激活令牌打进来，落点只有一处：以系统身份构造一条引导事件
// 入队。这条路径与会话间通知共用同一套语义：
//   source = System、queue_mode = Guided、activate_assistant = true
// 它是一条正式会话消息，由主助理在新一轮看到。
//
// 收到就投。会话在不在、归没归档、是什么种类，都不在这里判断。

/// 激活消息的正文：显式标明来源，让主助理一眼分辨这是外部事件而不是用户说话。
fn activation_build_message(text: &str) -> ChatMessage {
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
            "messageKind": "activation_event",
            "activationEvent": true,
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// 向目标会话投递一条系统引导事件并唤醒主助理。返回事件 ID。
fn activation_deliver_event(
    state: &AppState,
    conversation_id: &str,
    text: &str,
) -> Result<String, String> {
    let conversation = conversation_service_v2()
        .get_conversation_metadata_record(state, conversation_id)?;
    let mut runtime_context = runtime_context_new("activation_event", "activation_event");
    runtime_context.request_id = Some(format!("activation-request-{}", Uuid::new_v4()));
    runtime_context.dispatch_id = Some(format!("activation-dispatch-{}", Uuid::new_v4()));
    runtime_context.target_conversation_id = Some(conversation_id.to_string());
    runtime_context.root_conversation_id = conversation
        .root_conversation_id
        .clone()
        .or_else(|| Some(conversation_id.to_string()));
    runtime_context.executor_agent_id = Some(conversation.agent_id.clone());
    let event = ChatPendingEvent {
        id: format!("activation-{}", Uuid::new_v4()),
        conversation_id: conversation_id.to_string(),
        created_at: now_iso(),
        created_at_ms: now_unix_ms(),
        source: ChatEventSource::System,
        queue_mode: ChatQueueMode::Guided,
        messages: vec![activation_build_message(text)],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: conversation.agent_id.clone(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    let event_id = event.id.clone();
    let ingress = ingress_chat_event(state, event)?;
    trigger_chat_event_after_ingress_with_delay(state, ingress, std::time::Duration::ZERO);
    Ok(event_id)
}

/// 会话短号：优先读内存态元数据（新建会话还没刷到 SQLite 时也能拿到），
/// 读不到再回落 SQLite 索引列。对外回执一律用短号。
fn conversation_short_id_for(state: &AppState, conversation_id: &str) -> Option<u64> {
    if let Ok(meta) = conversation_service_v2().get_conversation_meta(state, conversation_id) {
        if let Some(short_id) = meta.short_id {
            return Some(short_id);
        }
    }
    chat_metadata_conversation_short_id(&state.data_path, conversation_id)
        .ok()
        .flatten()
}
