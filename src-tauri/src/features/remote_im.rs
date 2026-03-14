#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImEnqueueInput {
    channel_id: String,
    platform: RemoteImPlatform,
    im_name: String,
    remote_contact_type: String,
    remote_contact_id: String,
    #[serde(default)]
    remote_contact_name: Option<String>,
    sender_id: String,
    sender_name: String,
    #[serde(default)]
    sender_avatar_url: Option<String>,
    #[serde(default)]
    platform_message_id: Option<String>,
    #[serde(default)]
    activate_assistant: Option<bool>,
    session: SessionSelector,
    payload: ChatInputPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImEnqueueResult {
    event_id: String,
    conversation_id: String,
    activate_assistant: bool,
    contact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactReplyModeUpdateInput {
    contact_id: String,
    reply_mode: RemoteImReplyMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactRemarkUpdateInput {
    contact_id: String,
    #[serde(default)]
    remark_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDeleteInput {
    contact_id: String,
}

#[derive(Debug, Clone)]
struct RemoteImDirectSendTarget {
    channel: RemoteImChannelConfig,
    contact: RemoteImContact,
    payload: Value,
}

fn remote_im_structured_log(value: Value) {
    eprintln!("{}", value);
}

fn remote_im_channel_by_id<'a>(
    config: &'a AppConfig,
    channel_id: &str,
) -> Option<&'a RemoteImChannelConfig> {
    config
        .remote_im_channels
        .iter()
        .find(|channel| channel.id == channel_id)
}

fn remote_im_text_parts(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn remote_im_should_forward(
    reply_mode: &RemoteImReplyMode,
    has_new_message: bool,
    forwarded_once_since_last_inbound: bool,
) -> bool {
    match reply_mode {
        RemoteImReplyMode::None => false,
        RemoteImReplyMode::Always => true,
        RemoteImReplyMode::ReplyOnce => has_new_message && !forwarded_once_since_last_inbound,
    }
}

fn remote_im_upsert_contact_for_inbound(
    data: &mut AppData,
    _channel: &RemoteImChannelConfig,
    input: &RemoteImEnqueueInput,
    now: &str,
) -> String {
    if let Some(contact) = data.remote_im_contacts.iter_mut().find(|item| {
        item.channel_id == input.channel_id
            && item.remote_contact_type == input.remote_contact_type.trim()
            && item.remote_contact_id == input.remote_contact_id
    }) {
        if let Some(name) = input
            .remote_contact_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            contact.remote_contact_name = name.to_string();
        }
        contact.has_new_message = true;
        contact.forwarded_once_since_last_inbound = false;
        contact.last_message_at = Some(now.to_string());
        return contact.id.clone();
    }

    let contact_id = Uuid::new_v4().to_string();
    data.remote_im_contacts.push(RemoteImContact {
        id: contact_id.clone(),
        channel_id: input.channel_id.clone(),
        platform: input.platform.clone(),
        remote_contact_type: input.remote_contact_type.trim().to_string(),
        remote_contact_id: input.remote_contact_id.trim().to_string(),
        remote_contact_name: input
            .remote_contact_name
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        remark_name: String::new(),
        reply_mode: RemoteImReplyMode::None,
        has_new_message: true,
        forwarded_once_since_last_inbound: false,
        last_message_at: Some(now.to_string()),
        last_forwarded_at: None,
    });
    contact_id
}

fn remote_im_build_outbound_payload(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    message: &ChatMessage,
) -> Result<Value, String> {
    let mut content = Vec::<Value>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => {
                if !text.trim().is_empty() {
                    content.push(serde_json::json!({
                        "type": "text",
                        "text": text
                    }));
                }
            }
            MessagePart::Image {
                mime,
                bytes_base64,
                name,
                ..
            } => {
                if channel.allow_send_files {
                    content.push(serde_json::json!({
                        "type": "image",
                        "mime": mime,
                        "name": name,
                        "data": bytes_base64
                    }));
                }
            }
            MessagePart::Audio {
                mime,
                bytes_base64,
                name,
                ..
            } => {
                if channel.allow_send_files {
                    content.push(serde_json::json!({
                        "type": "audio",
                        "mime": mime,
                        "name": name,
                        "data": bytes_base64
                    }));
                }
            }
        }
    }
    if content.is_empty() {
        let fallback_text = remote_im_text_parts(message);
        if fallback_text.trim().is_empty() {
            return Err(format!(
                "remote_im outbound payload is empty (allow_send_files={})",
                channel.allow_send_files
            ));
        }
        content.push(serde_json::json!({
            "type": "text",
            "text": fallback_text
        }));
    }
    Ok(serde_json::json!({
        "channelId": contact.channel_id,
        "contactId": contact.id,
        "platform": channel.platform,
        "remoteContactType": contact.remote_contact_type,
        "remoteContactId": contact.remote_contact_id,
        "content": content,
    }))
}

fn remote_im_collect_outbound_after_assistant_message(
    config: &AppConfig,
    data: &mut AppData,
    assistant_message: &ChatMessage,
) -> Vec<RemoteImDirectSendTarget> {
    let now = now_iso();
    let mut targets = Vec::<RemoteImDirectSendTarget>::new();
    for contact in &mut data.remote_im_contacts {
        let Some(channel) = remote_im_channel_by_id(config, &contact.channel_id) else {
            continue;
        };
        if !channel.enabled {
            continue;
        }
        if !remote_im_should_forward(
            &contact.reply_mode,
            contact.has_new_message,
            contact.forwarded_once_since_last_inbound,
        ) {
            continue;
        }
        let payload = match remote_im_build_outbound_payload(channel, contact, assistant_message) {
            Ok(value) => value,
            Err(err) => {
                remote_im_structured_log(serde_json::json!({
                        "task": "远程IM直发",
                        "trigger": "direct",
                        "status": "失败",
                        "stage": "build_payload",
                        "channel_id": channel.id,
                        "contact_id": contact.id,
                        "platform": channel.platform,
                        "error": err
                    }));
                continue;
            }
        };
        targets.push(RemoteImDirectSendTarget {
            channel: channel.clone(),
            contact: contact.clone(),
            payload,
        });
        contact.last_forwarded_at = Some(now.clone());
        contact.has_new_message = false;
        contact.forwarded_once_since_last_inbound = true;
    }
    targets
}

fn remote_im_set_sender_origin_meta(
    input: &RemoteImEnqueueInput,
    conversation_id: &str,
    contact_id: &str,
) -> Value {
    serde_json::json!({
        "origin": {
            "kind": "remote_im",
            "channelId": input.channel_id,
            "platform": input.platform,
            "imName": input.im_name,
            "remoteContactType": input.remote_contact_type,
            "remoteContactId": input.remote_contact_id,
            "remoteContactName": input.remote_contact_name,
            "senderId": input.sender_id,
            "senderName": input.sender_name,
            "senderAvatarUrl": input.sender_avatar_url,
            "platformMessageId": input.platform_message_id,
            "contactId": contact_id,
            "conversationId": conversation_id
        }
    })
}

fn remote_im_resolve_inbound_activate(
    channel: &RemoteImChannelConfig,
    message_flag: Option<bool>,
) -> bool {
    message_flag.unwrap_or(channel.activate_assistant)
}

fn validate_enqueue_input(
    input: &RemoteImEnqueueInput,
    state: &State<'_, AppState>,
    config: &AppConfig,
    data: &mut AppData,
) -> Result<
    (
        String,
        Vec<BinaryPart>,
        Vec<BinaryPart>,
        Vec<AttachmentMetaInput>,
        RemoteImChannelConfig,
        String,
        String,
        String,
    ),
    String,
> {
    let text = input.payload.text.as_deref().unwrap_or("").trim().to_string();
    let channel_id = input.channel_id.trim().to_string();
    if channel_id.is_empty() {
        return Err("channelId 不能为空".to_string());
    }
    let api_config_id = input
        .session
        .api_config_id
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let agent_id = input.session.agent_id.trim().to_string();
    if api_config_id.is_empty() || agent_id.is_empty() {
        return Err("路由信息不完整（apiConfigId/agentId）".to_string());
    }
    let channel = remote_im_channel_by_id(config, &channel_id)
        .ok_or_else(|| format!("远程IM渠道不存在: {channel_id}"))?
        .clone();
    if !channel.enabled {
        return Err(format!("远程IM渠道未启用: {channel_id}"));
    }

    let images = if channel.receive_files {
        input.payload.images.clone().unwrap_or_default()
    } else {
        Vec::new()
    };
    let audios = if channel.receive_files {
        input.payload.audios.clone().unwrap_or_default()
    } else {
        Vec::new()
    };
    let attachments = if channel.receive_files {
        input.payload.attachments.clone().unwrap_or_default()
    } else {
        Vec::new()
    };
    if text.is_empty() && images.is_empty() && audios.is_empty() && attachments.is_empty() {
        return Err("远程IM消息内容为空".to_string());
    }

    let conversation_id = if let Some(requested) = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if data.conversations.iter().any(|conv| {
            conv.id == requested
                && conv.status == "active"
                && conv.summary.trim().is_empty()
                && !conversation_is_delegate(conv)
                && conv.api_config_id == api_config_id
                && conv.agent_id == agent_id
        }) {
            requested.to_string()
        } else {
            let idx = ensure_active_conversation_index(data, &api_config_id, &agent_id);
            data.conversations
                .get(idx)
                .map(|item| item.id.clone())
                .ok_or_else(|| "活动会话索引超出范围".to_string())?
        }
    } else {
        let idx = ensure_active_conversation_index(data, &api_config_id, &agent_id);
        data.conversations
            .get(idx)
            .map(|item| item.id.clone())
            .ok_or_else(|| "活动会话索引超出范围".to_string())?
    };

    let _ = state;
    Ok((
        text,
        images,
        audios,
        attachments,
        channel,
        api_config_id,
        agent_id,
        conversation_id,
    ))
}

fn build_chat_message_from_input(
    input: &RemoteImEnqueueInput,
    channel: &RemoteImChannelConfig,
    conversation_id: &str,
    contact_id: &str,
    now: &str,
) -> ChatMessage {
    let text = input.payload.text.as_deref().unwrap_or("").trim();
    let mut parts = Vec::<MessagePart>::new();
    if !text.is_empty() {
        parts.push(MessagePart::Text {
            text: text.to_string(),
        });
    }
    if channel.receive_files {
        for img in input.payload.images.as_deref().unwrap_or(&[]) {
            parts.push(MessagePart::Image {
                mime: img.mime.clone(),
                bytes_base64: img.bytes_base64.clone(),
                name: None,
                compressed: false,
            });
        }
        for audio in input.payload.audios.as_deref().unwrap_or(&[]) {
            parts.push(MessagePart::Audio {
                mime: audio.mime.clone(),
                bytes_base64: audio.bytes_base64.clone(),
                name: None,
                compressed: false,
            });
        }
    }

    let origin_meta = remote_im_set_sender_origin_meta(input, conversation_id, contact_id);
    let mut base_meta = input
        .payload
        .provider_meta
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(base_obj) = base_meta.as_object_mut() {
        base_obj.insert("origin".to_string(), origin_meta["origin"].clone());
    } else {
        base_meta = origin_meta;
    }
    let attachment_meta = if channel.receive_files {
        normalize_payload_attachments(input.payload.attachments.as_ref())
    } else {
        Vec::new()
    };
    let merged_meta = merge_provider_meta_with_attachments(Some(base_meta), &attachment_meta);

    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now.to_string(),
        speaker_agent_id: None,
        parts,
        extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
        provider_meta: merged_meta,
        tool_call: None,
        mcp_call: None,
    }
}

fn create_pending_event(
    event_id: String,
    conversation_id: String,
    messages: Vec<ChatMessage>,
    activate_assistant: bool,
    session_info: ChatSessionInfo,
    sender_info: RemoteImMessageSource,
) -> ChatPendingEvent {
    ChatPendingEvent {
        id: event_id,
        conversation_id,
        created_at: now_iso(),
        source: ChatEventSource::RemoteIm,
        messages,
        activate_assistant,
        session_info,
        sender_info: Some(sender_info),
    }
}

#[tauri::command]
fn remote_im_list_channels(state: State<'_, AppState>) -> Result<Vec<RemoteImChannelConfig>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let config = state_read_config_cached(&state)?;
    drop(guard);
    Ok(config.remote_im_channels)
}

#[tauri::command]
fn remote_im_list_contacts(state: State<'_, AppState>) -> Result<Vec<RemoteImContact>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let data = state_read_app_data_cached(&state)?;
    let mut contacts = data.remote_im_contacts;
    contacts.sort_by(|a, b| {
        a.channel_id
            .cmp(&b.channel_id)
            .then_with(|| b.last_message_at.cmp(&a.last_message_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    drop(guard);
    Ok(contacts)
}

#[tauri::command]
fn remote_im_update_contact_reply_mode(
    input: RemoteImContactReplyModeUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(&state)?;
    let contact = data
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("remote contact not found: {}", input.contact_id))?;
    contact.reply_mode = input.reply_mode;
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_remark(
    input: RemoteImContactRemarkUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(&state)?;
    let contact = data
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("remote contact not found: {}", input.contact_id))?;
    contact.remark_name = input.remark_name.trim().to_string();
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_delete_contact(
    input: RemoteImContactDeleteInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(&state)?;
    let before = data.remote_im_contacts.len();
    data.remote_im_contacts
        .retain(|item| item.id != input.contact_id.trim());
    let removed = data.remote_im_contacts.len() != before;
    if removed {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);
    Ok(removed)
}

#[tauri::command]
fn remote_im_enqueue_message(
    input: RemoteImEnqueueInput,
    state: State<'_, AppState>,
) -> Result<RemoteImEnqueueResult, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let (_text, _images, _audios, _attachments, channel, api_config_id, agent_id, conversation_id) =
        validate_enqueue_input(&input, &state, &config, &mut data)?;

    let now = now_iso();
    let contact_id = remote_im_upsert_contact_for_inbound(&mut data, &channel, &input, &now);
    let message =
        build_chat_message_from_input(&input, &channel, &conversation_id, &contact_id, &now);

    let event_id = Uuid::new_v4().to_string();
    let activate_assistant = remote_im_resolve_inbound_activate(&channel, input.activate_assistant);
    let event = create_pending_event(
        event_id.clone(),
        conversation_id.clone(),
        vec![message],
        activate_assistant,
        ChatSessionInfo {
            api_config_id,
            agent_id,
        },
        RemoteImMessageSource {
            channel_id: input.channel_id.trim().to_string(),
            platform: input.platform,
            im_name: input.im_name,
            remote_contact_type: input.remote_contact_type,
            remote_contact_id: input.remote_contact_id,
            remote_contact_name: input.remote_contact_name.unwrap_or_default(),
            sender_id: input.sender_id,
            sender_name: input.sender_name,
            sender_avatar_url: input.sender_avatar_url,
            platform_message_id: input.platform_message_id,
        },
    );
    enqueue_chat_event(state.inner(), event)?;
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    trigger_chat_queue_processing(state.inner());
    Ok(RemoteImEnqueueResult {
        event_id,
        conversation_id,
        activate_assistant,
        contact_id,
    })
}

async fn remote_im_on_assistant_round_completed(
    state: &AppState,
    result: &SendChatResult,
) -> Result<usize, String> {
    let Some(assistant_message) = result.assistant_message.as_ref() else {
        return Ok(0);
    };
    if assistant_message.role.trim() != "assistant" {
        return Ok(0);
    }
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let config = state_read_config_cached(state)?;
    let mut data = state_read_app_data_cached(state)?;
    let targets = remote_im_collect_outbound_after_assistant_message(&config, &mut data, assistant_message);
    if !targets.is_empty() {
        state_write_app_data_cached(state, &data)?;
    }
    drop(guard);

    fn payload_size_of(value: &Value) -> usize {
        match value {
            Value::Array(v) => v.len(),
            Value::Object(v) => v.len(),
            _ => value.to_string().len(),
        }
    }

    for target in &targets {
        let start = std::time::Instant::now();
        match remote_im_send_via_sdk(&target.channel, &target.contact, &target.payload).await {
            Ok(platform_message_id) => {
                let duration_ms = start.elapsed().as_millis();
                remote_im_structured_log(serde_json::json!({
                        "task": "远程IM直发",
                        "trigger": "direct",
                        "total_targets": targets.len(),
                        "payload_size": payload_size_of(&target.payload),
                        "platform": target.channel.platform,
                        "contact_id": target.contact.id,
                        "status": "完成",
                        "platform_message_id": platform_message_id,
                        "duration_ms": duration_ms
                    }));
            }
            Err(err) => {
                let duration_ms = start.elapsed().as_millis();
                remote_im_structured_log(serde_json::json!({
                        "task": "远程IM直发",
                        "trigger": "direct",
                        "total_targets": targets.len(),
                        "payload_size": payload_size_of(&target.payload),
                        "platform": target.channel.platform,
                        "contact_id": target.contact.id,
                        "status": "失败",
                        "error": err,
                        "duration_ms": duration_ms
                    }));
            }
        }
    }
    Ok(targets.len())
}
