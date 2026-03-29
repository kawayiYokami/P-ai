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
    dingtalk_session_webhook: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook_expired_time: Option<i64>,
    #[serde(default)]
    activate_assistant: Option<bool>,
    session: SessionSelector,
    payload: ChatInputPayload,
}

fn provider_meta_string(meta: &Option<Value>, key: &str) -> Option<String> {
    meta.as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn provider_meta_i64(meta: &Option<Value>, key: &str) -> Option<i64> {
    let value = meta.as_ref().and_then(|item| item.get(key))?;
    if let Some(v) = value.as_i64() {
        return Some(v);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| raw.parse::<i64>().ok())
}

fn resolve_dingtalk_session_webhook(input: &RemoteImEnqueueInput) -> Option<String> {
    let direct = input
        .dingtalk_session_webhook
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if direct.is_some() {
        return direct;
    }

    provider_meta_string(&input.payload.provider_meta, "sessionWebhook")
        .or_else(|| provider_meta_string(&input.payload.provider_meta, "dingtalkSessionWebhook"))
}

fn resolve_dingtalk_session_webhook_expired_time(input: &RemoteImEnqueueInput) -> Option<i64> {
    input.dingtalk_session_webhook_expired_time
        .or_else(|| provider_meta_i64(&input.payload.provider_meta, "sessionWebhookExpiredTime"))
        .or_else(|| {
            provider_meta_i64(
                &input.payload.provider_meta,
                "dingtalkSessionWebhookExpiredTime",
            )
        })
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
struct RemoteImContactAllowSendUpdateInput {
    contact_id: String,
    allow_send: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactAllowSendFilesUpdateInput {
    contact_id: String,
    allow_send_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactAllowReceiveUpdateInput {
    contact_id: String,
    allow_receive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactActivationUpdateInput {
    contact_id: String,
    activation_mode: String,
    #[serde(default)]
    activation_keywords: Vec<String>,
    #[serde(default)]
    activation_cooldown_seconds: u64,
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
struct RemoteImContactRouteModeUpdateInput {
    contact_id: String,
    route_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDepartmentBindingUpdateInput {
    contact_id: String,
    #[serde(default)]
    department_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactProcessingModeUpdateInput {
    contact_id: String,
    processing_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDeleteInput {
    contact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationSummary {
    contact_id: String,
    conversation_id: String,
    title: String,
    updated_at: String,
    last_message_at: Option<String>,
    message_count: usize,
    channel_id: String,
    platform: RemoteImPlatform,
    contact_display_name: String,
    bound_department_id: Option<String>,
    processing_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationMessagesInput {
    contact_id: String,
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

fn remote_im_upsert_contact_for_inbound(
    data: &mut AppData,
    channel: &RemoteImChannelConfig,
    input: &RemoteImEnqueueInput,
    now: &str,
) -> String {
    let default_allow_receive = remote_im_resolve_inbound_activate(channel, input.activate_assistant);
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
        if matches!(input.platform, RemoteImPlatform::Dingtalk) {
            let session_webhook = resolve_dingtalk_session_webhook(input);
            if session_webhook.is_some() {
                contact.dingtalk_session_webhook = session_webhook;
            }
            let expired_time = resolve_dingtalk_session_webhook_expired_time(input);
            if expired_time.is_some() {
                contact.dingtalk_session_webhook_expired_time = expired_time;
            }
        }
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
        allow_send: false,
        allow_send_files: false,
        allow_receive: default_allow_receive,
        activation_mode: "never".to_string(),
        activation_keywords: Vec::new(),
        activation_cooldown_seconds: 0,
        route_mode: "main_session".to_string(),
        bound_department_id: Some(FRONT_DESK_DEPARTMENT_ID.to_string()),
        bound_conversation_id: None,
        processing_mode: "continuous".to_string(),
        last_activated_at: None,
        last_message_at: Some(now.to_string()),
        dingtalk_session_webhook: if matches!(input.platform, RemoteImPlatform::Dingtalk) {
            resolve_dingtalk_session_webhook(input)
        } else {
            None
        },
        dingtalk_session_webhook_expired_time: if matches!(input.platform, RemoteImPlatform::Dingtalk)
        {
            resolve_dingtalk_session_webhook_expired_time(input)
        } else {
            None
        },
    });
    contact_id
}

fn normalize_contact_activation_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "always" | "keyword" => value.trim().to_ascii_lowercase(),
        "never" => "never".to_string(),
        _ => "never".to_string(),
    }
}

fn normalize_contact_activation_keywords(values: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if out.iter().any(|item| item == trimmed) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

fn normalize_contact_route_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "dedicated_contact_conversation" => "dedicated_contact_conversation".to_string(),
        _ => "main_session".to_string(),
    }
}

fn normalize_contact_processing_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "qa" => "qa".to_string(),
        _ => "continuous".to_string(),
    }
}

fn remote_im_contact_display_name(contact: &RemoteImContact) -> String {
    let remark = contact.remark_name.trim();
    if !remark.is_empty() {
        return remark.to_string();
    }
    let remote_name = contact.remote_contact_name.trim();
    if !remote_name.is_empty() {
        return remote_name.to_string();
    }
    contact.remote_contact_id.trim().to_string()
}

fn remote_im_contact_is_main_department(config: &AppConfig, contact: &RemoteImContact) -> bool {
    let Some(bound_department_id) = contact
        .bound_department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return true;
    };
    bound_department_id == ASSISTANT_DEPARTMENT_ID
        || assistant_department(config)
            .map(|dept| dept.id.trim() == bound_department_id)
            .unwrap_or(false)
}

fn remote_im_resolve_effective_route_mode(
    config: &AppConfig,
    contact: &RemoteImContact,
) -> String {
    if remote_im_contact_is_main_department(config, contact) {
        "main_session".to_string()
    } else {
        "dedicated_contact_conversation".to_string()
    }
}

fn remote_im_contact_conversation_title(contact: &RemoteImContact) -> String {
    format!("联系人 · {}", remote_im_contact_display_name(contact))
}

fn remote_im_contact_conversation_key_parts(
    channel_id: &str,
    remote_contact_type: &str,
    remote_contact_id: &str,
) -> String {
    format!(
        "remote_im_contact:{}:{}:{}",
        channel_id.trim(),
        remote_contact_type.trim().to_ascii_lowercase(),
        remote_contact_id.trim()
    )
}

fn remote_im_contact_conversation_key(contact: &RemoteImContact) -> String {
    remote_im_contact_conversation_key_parts(
        &contact.channel_id,
        &contact.remote_contact_type,
        &contact.remote_contact_id,
    )
}

fn find_remote_im_contact_conversation_id(
    data: &AppData,
    contact: &RemoteImContact,
) -> Option<String> {
    let key = remote_im_contact_conversation_key(contact);
    data.conversations
        .iter()
        .find(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_is_remote_im_contact(conversation)
                && conversation
                    .root_conversation_id
                    .as_deref()
                    .map(str::trim)
                    == Some(key.as_str())
        })
        .map(|conversation| conversation.id.clone())
}

fn ensure_remote_im_contact_conversation_id(
    data: &mut AppData,
    contact: &mut RemoteImContact,
) -> Result<String, String> {
    if let Some(found_id) = find_remote_im_contact_conversation_id(data, contact) {
        contact.bound_conversation_id = Some(found_id.clone());
        return Ok(found_id);
    }

    let mut conversation = build_conversation_record(
        "",
        "",
        &remote_im_contact_conversation_title(contact),
        CONVERSATION_KIND_REMOTE_IM_CONTACT,
        Some(remote_im_contact_conversation_key(contact)),
        None,
    );
    conversation.status = "inactive".to_string();
    let conversation_id = conversation.id.clone();
    data.conversations.push(conversation);
    contact.bound_conversation_id = Some(conversation_id.clone());
    Ok(conversation_id)
}

fn remote_im_set_sender_origin_meta(
    input: &RemoteImEnqueueInput,
    conversation_id: &str,
    contact_record_id: &str,
) -> Value {
    serde_json::json!({
        "origin": {
            "kind": "remote_im",
            "channel_id": input.channel_id,
            "platform": input.platform,
            "im_name": input.im_name,
            "contact_type": input.remote_contact_type,
            "contact_id": input.remote_contact_id,
            "contact_name": input.remote_contact_name,
            "contact_record_id": contact_record_id,
            "sender_id": input.sender_id,
            "sender_name": input.sender_name,
            "sender_avatar_url": input.sender_avatar_url,
            "platform_message_id": input.platform_message_id,
            "conversation_id": conversation_id
        }
    })
}

fn remote_im_resolve_inbound_activate(
    channel: &RemoteImChannelConfig,
    message_flag: Option<bool>,
) -> bool {
    message_flag.unwrap_or(channel.activate_assistant)
}

fn origin_value_string<'a>(origin: &'a Value, key: &str) -> Option<&'a str> {
    origin
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn message_origin_string<'a>(message: &'a ChatMessage, key: &str) -> Option<&'a str> {
    let origin = message.provider_meta.as_ref()?.get("origin")?;
    origin_value_string(origin, key)
}

fn conversation_has_remote_im_platform_message(
    conversation: &Conversation,
    channel_id: &str,
    remote_contact_type: &str,
    remote_contact_id: &str,
    platform_message_id: &str,
) -> bool {
    conversation.messages.iter().any(|message| {
        message_origin_string(message, "kind") == Some("remote_im")
            && message_origin_string(message, "channel_id") == Some(channel_id)
            && message_origin_string(message, "contact_type") == Some(remote_contact_type)
            && message_origin_string(message, "contact_id") == Some(remote_contact_id)
            && message_origin_string(message, "platform_message_id") == Some(platform_message_id)
    })
}

fn pending_event_has_remote_im_platform_message(
    event: &ChatPendingEvent,
    channel_id: &str,
    remote_contact_type: &str,
    remote_contact_id: &str,
    platform_message_id: &str,
) -> bool {
    event.sender_info.as_ref().is_some_and(|sender| {
        sender.channel_id.trim() == channel_id
            && sender.remote_contact_type.trim() == remote_contact_type
            && sender.remote_contact_id.trim() == remote_contact_id
            && sender.platform_message_id.as_deref().map(str::trim) == Some(platform_message_id)
    })
}

fn remote_im_is_duplicate_platform_message(
    state: &AppState,
    data: &AppData,
    conversation_id: &str,
    channel_id: &str,
    remote_contact_type: &str,
    remote_contact_id: &str,
    platform_message_id: &str,
) -> Result<bool, String> {
    if data.conversations.iter().any(|conversation| {
        conversation.id == conversation_id
            && conversation_has_remote_im_platform_message(
                conversation,
                channel_id,
                remote_contact_type,
                remote_contact_id,
                platform_message_id,
            )
    }) {
        return Ok(true);
    }

    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots.values().any(|slot| {
        slot.pending_queue.iter().any(|event| {
            event.conversation_id == conversation_id
                && pending_event_has_remote_im_platform_message(
                    event,
                    channel_id,
                    remote_contact_type,
                    remote_contact_id,
                    platform_message_id,
                )
        })
    }))
}

struct ValidatedEnqueueInput {
    text: String,
    images: Vec<BinaryPart>,
    audios: Vec<BinaryPart>,
    attachments: Vec<AttachmentMetaInput>,
    channel: RemoteImChannelConfig,
}

fn validate_images(channel: &RemoteImChannelConfig, input: &RemoteImEnqueueInput) -> Vec<BinaryPart> {
    if channel.receive_files {
        input.payload.images.clone().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn validate_audios(channel: &RemoteImChannelConfig, input: &RemoteImEnqueueInput) -> Vec<BinaryPart> {
    if channel.receive_files {
        input.payload.audios.clone().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn validate_attachments(
    channel: &RemoteImChannelConfig,
    input: &RemoteImEnqueueInput,
) -> Vec<AttachmentMetaInput> {
    if channel.receive_files {
        input.payload.attachments.clone().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn resolve_channel_config(
    input: &RemoteImEnqueueInput,
    config: &AppConfig,
) -> Result<(String, RemoteImChannelConfig), String> {
    let channel_id = input.channel_id.trim().to_string();
    if channel_id.is_empty() {
        return Err("channel_id 不能为空".to_string());
    }
    let channel = remote_im_channel_by_id(config, &channel_id)
        .ok_or_else(|| format!("远程IM渠道不存在: {channel_id}"))?
        .clone();
    if !channel.enabled {
        return Err(format!("远程IM渠道未启用: {channel_id}"));
    }
    Ok((channel_id, channel))
}

fn resolve_department_agent_pair(
    requested_department_id: Option<&str>,
    requested_agent_id: Option<&str>,
    config: &AppConfig,
) -> Result<(String, String), String> {
    let requested_department_id = requested_department_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let requested_agent_id = requested_agent_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_default();
    let agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id
    } else {
        assistant_department_agent_id(config)
            .ok_or_else(|| "路由信息不完整（缺少 agentId）".to_string())?
    };
    let department = if let Some(department_id) = requested_department_id.as_deref() {
        department_by_id(config, department_id)
            .ok_or_else(|| format!("路由部门不存在: {department_id}"))?
    } else {
        department_for_agent_id(config, &agent_id)
            .or_else(|| assistant_department(config))
            .ok_or_else(|| "路由部门不存在".to_string())?
    };
    if !department
        .agent_ids
        .iter()
        .any(|id| id.trim() == agent_id)
    {
        return Err(format!(
            "agentId 与部门不匹配: agentId={}, departmentId={}",
            agent_id, department.id
        ));
    }
    let department_model_api_id = department_primary_api_config_id(department);
    if department_model_api_id.trim().is_empty() {
        return Err(format!("部门模型未配置: {}", department.id));
    }
    Ok((department.id.clone(), agent_id))
}

fn validate_enqueue_input(
    input: &RemoteImEnqueueInput,
    config: &AppConfig,
) -> Result<ValidatedEnqueueInput, String> {
    let text = input.payload.text.as_deref().unwrap_or("").trim().to_string();
    let (_channel_id, channel) = resolve_channel_config(input, config)?;
    let images = validate_images(&channel, input);
    let audios = validate_audios(&channel, input);
    let attachments = validate_attachments(&channel, input);
    if text.is_empty() && images.is_empty() && audios.is_empty() && attachments.is_empty() {
        return Err("远程IM消息内容为空".to_string());
    }

    Ok(ValidatedEnqueueInput {
        text,
        images,
        audios,
        attachments,
        channel,
    })
}

fn resolve_contact_session_target(
    config: &AppConfig,
    data: &mut AppData,
    contact: &mut RemoteImContact,
) -> Result<(String, String, String), String> {
    let effective_route_mode = remote_im_resolve_effective_route_mode(config, contact);
    contact.route_mode = effective_route_mode.clone();
    if effective_route_mode == "main_session" {
        let (department_id, agent_id) = resolve_department_agent_pair(
            Some(ASSISTANT_DEPARTMENT_ID),
            assistant_department_agent_id(config).as_deref(),
            config,
        )?;
        let main_idx = ensure_main_conversation_index(data, "", &agent_id);
        let conversation_id = data
            .conversations
            .get(main_idx)
            .map(|item| item.id.clone())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "主会话索引超出范围".to_string())?;
        return Ok((department_id, agent_id, conversation_id));
    }

    let (department_id, agent_id) = resolve_department_agent_pair(
        contact.bound_department_id.as_deref(),
        None,
        config,
    )?;
    let conversation_id = ensure_remote_im_contact_conversation_id(data, contact)?;
    Ok((department_id, agent_id, conversation_id))
}

fn build_chat_message_from_input(
    input: &RemoteImEnqueueInput,
    conversation_id: &str,
    contact_id: &str,
    now: &str,
    text: &str,
    images: &[BinaryPart],
    audios: &[BinaryPart],
    attachments: &[AttachmentMetaInput],
) -> ChatMessage {
    let mut parts = Vec::<MessagePart>::new();
    if !text.is_empty() {
        parts.push(MessagePart::Text {
            text: text.to_string(),
        });
    }
    for img in images {
        parts.push(MessagePart::Image {
            mime: img.mime.clone(),
            bytes_base64: img.bytes_base64.clone(),
            name: None,
            compressed: false,
        });
    }
    for audio in audios {
        parts.push(MessagePart::Audio {
            mime: audio.mime.clone(),
            bytes_base64: audio.bytes_base64.clone(),
            name: None,
            compressed: false,
        });
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
    let attachment_meta = normalize_payload_attachments(Some(&attachments.to_vec()));
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
        runtime_context: None,
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
fn remote_im_update_contact_allow_send(
    input: RemoteImContactAllowSendUpdateInput,
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
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.allow_send = input.allow_send;
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_allow_send_files(
    input: RemoteImContactAllowSendFilesUpdateInput,
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
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.allow_send_files = input.allow_send_files;
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_allow_receive(
    input: RemoteImContactAllowReceiveUpdateInput,
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
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.allow_receive = input.allow_receive;
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_activation(
    input: RemoteImContactActivationUpdateInput,
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
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.activation_mode = normalize_contact_activation_mode(&input.activation_mode);
    contact.activation_keywords = normalize_contact_activation_keywords(&input.activation_keywords);
    contact.activation_cooldown_seconds = input.activation_cooldown_seconds;
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
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.remark_name = input.remark_name.trim().to_string();
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_route_mode(
    input: RemoteImContactRouteModeUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let contact = data
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    let requested_mode = normalize_contact_route_mode(&input.route_mode);
    let final_mode = if remote_im_contact_is_main_department(&config, contact) {
        "main_session".to_string()
    } else {
        "dedicated_contact_conversation".to_string()
    };
    if requested_mode != final_mode {
        eprintln!(
            "[远程IM] 联系人路由模式已被约束修正: contact_id={}, requested={}, final={}",
            contact.id, requested_mode, final_mode
        );
    }
    contact.route_mode = final_mode;
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_department_binding(
    input: RemoteImContactDepartmentBindingUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let contact = data
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    let next_department_id = input
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(department_id) = next_department_id.as_deref() {
        let department = department_by_id(&config, department_id)
            .ok_or_else(|| format!("部门不存在: {department_id}"))?;
        let api_id = department_primary_api_config_id(department);
        if api_id.trim().is_empty() {
            return Err(format!("部门模型未配置: {}", department.id));
        }
    }
    contact.bound_department_id = next_department_id;
    contact.route_mode = remote_im_resolve_effective_route_mode(&config, contact);
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_processing_mode(
    input: RemoteImContactProcessingModeUpdateInput,
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
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.processing_mode = normalize_contact_processing_mode(&input.processing_mode);
    let output = contact.clone();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(output)
}

#[tauri::command]
fn remote_im_list_contact_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<RemoteImContactConversationSummary>, String> {
    let started_at = std::time::Instant::now();
    eprintln!("[远程IM][联系人会话][列表] 开始");
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let data = state_read_app_data_cached(&state)?;
    let mut items = data
        .remote_im_contacts
        .iter()
        .filter_map(|contact| {
            let conversation_id = contact
                .bound_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| find_remote_im_contact_conversation_id(&data, contact))?;
            let conversation = data.conversations.iter().find(|conversation| {
                conversation.id == conversation_id
                    && conversation.summary.trim().is_empty()
                    && conversation_is_remote_im_contact(conversation)
            })?;
            Some(RemoteImContactConversationSummary {
                contact_id: contact.id.clone(),
                conversation_id: conversation.id.clone(),
                title: remote_im_contact_conversation_title(contact),
                updated_at: conversation.updated_at.clone(),
                last_message_at: conversation.messages.last().map(|message| message.created_at.clone()),
                message_count: conversation.messages.len(),
                channel_id: contact.channel_id.clone(),
                platform: contact.platform.clone(),
                contact_display_name: remote_im_contact_display_name(contact),
                bound_department_id: contact.bound_department_id.clone(),
                processing_mode: normalize_contact_processing_mode(&contact.processing_mode),
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        let bk = b.last_message_at.as_deref().unwrap_or(b.updated_at.as_str());
        let ak = a.last_message_at.as_deref().unwrap_or(a.updated_at.as_str());
        bk.cmp(ak).then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    drop(guard);
    eprintln!(
        "[远程IM][联系人会话][列表] 完成: contact_count={}, elapsed_ms={}",
        items.len(),
        started_at.elapsed().as_millis()
    );
    Ok(items)
}

#[tauri::command]
fn remote_im_get_contact_conversation_messages(
    input: RemoteImContactConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let started_at = std::time::Instant::now();
    eprintln!(
        "[远程IM][联系人会话][读取] 开始: contact_id={}",
        contact_id
    );
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let data = state_read_app_data_cached(&state)?;
    let contact = data
        .remote_im_contacts
        .iter()
        .find(|item| item.id == contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{contact_id}"))?;
    let conversation_id = contact
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| find_remote_im_contact_conversation_id(&data, contact))
        .ok_or_else(|| format!("联系人未绑定联系人会话：{contact_id}"))?;
    let mut messages = data
        .conversations
        .iter()
        .find(|conversation| {
            conversation.id == conversation_id
                && conversation.summary.trim().is_empty()
                && conversation_is_remote_im_contact(conversation)
        })
        .map(|conversation| conversation.messages.clone())
        .ok_or_else(|| format!("联系人会话不存在：{conversation_id}"))?;
    drop(guard);
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    eprintln!(
        "[远程IM][联系人会话][读取] 完成: contact_id={}, message_count={}, elapsed_ms={}",
        contact_id,
        messages.len(),
        started_at.elapsed().as_millis()
    );
    Ok(messages)
}

#[tauri::command]
fn remote_im_delete_contact(
    input: RemoteImContactDeleteInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(&state)?;
    let before_contacts = data.remote_im_contacts.len();
    data.remote_im_contacts
        .retain(|item| item.id != contact_id);
    let removed = data.remote_im_contacts.len() != before_contacts;
    if removed {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);
    Ok(removed)
}

#[tauri::command]
fn remote_im_clear_contact_conversation(
    input: RemoteImContactDeleteInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let started_at = std::time::Instant::now();
    eprintln!(
        "[远程IM][联系人会话][清空] 开始: contact_id={}",
        contact_id
    );
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(&state)?;
    let Some(contact_idx) = data.remote_im_contacts.iter().position(|item| item.id == contact_id) else {
        return Err(format!("未找到远程联系人：{contact_id}"));
    };
    let conversation_id = {
        let contact = &data.remote_im_contacts[contact_idx];
        contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| find_remote_im_contact_conversation_id(&data, contact))
    };
    let Some(conversation_id) = conversation_id else {
        return Ok(false);
    };
    let Some(conversation) = data.conversations.iter_mut().find(|conversation| {
        conversation.id == conversation_id
            && conversation.summary.trim().is_empty()
            && conversation_is_remote_im_contact(conversation)
    }) else {
        return Ok(false);
    };

    conversation.messages.clear();
    conversation.memory_recall_table.clear();
    conversation.last_user_at = None;
    conversation.last_assistant_at = None;
    conversation.last_context_usage_ratio = 0.0;
    conversation.last_effective_prompt_tokens = 0;
    conversation.status = "inactive".to_string();
    conversation.updated_at = now_iso();

    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    eprintln!(
        "[远程IM][联系人会话][清空] 完成: contact_id={}, elapsed_ms={}",
        contact_id,
        started_at.elapsed().as_millis()
    );
    Ok(true)
}

#[tauri::command]
fn remote_im_enqueue_message(
    input: RemoteImEnqueueInput,
    state: State<'_, AppState>,
) -> Result<RemoteImEnqueueResult, String> {
    remote_im_enqueue_message_internal(input, state.inner())
}

/// 内部入队函数，供事件消费循环调用
pub(crate) fn remote_im_enqueue_message_internal(
    input: RemoteImEnqueueInput,
    state: &AppState,
) -> Result<RemoteImEnqueueResult, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let config = state_read_config_cached(state)?;
    let mut data = state_read_app_data_cached(state)?;
    let validated = validate_enqueue_input(&input, &config)?;
    let channel = validated.channel;
    let text = validated.text;
    let images = validated.images;
    let audios = validated.audios;
    let attachments = validated.attachments;

    let now = now_iso();
    let contact_id = remote_im_upsert_contact_for_inbound(&mut data, &channel, &input, &now);
    let contact_idx = data
        .remote_im_contacts
        .iter()
        .position(|item| item.id == contact_id)
        .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
    let contact = data
        .remote_im_contacts
        .get_mut(contact_idx)
        .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
    let mut allow_receive = contact.allow_receive;
    if !allow_receive
        && matches!(channel.platform, RemoteImPlatform::Dingtalk)
        && channel.activate_assistant
    {
        let looks_like_default_contact = !contact.allow_send
            && !contact.allow_receive
            && contact.activation_mode == "never"
            && contact.activation_keywords.is_empty()
            && contact.activation_cooldown_seconds == 0;
        if looks_like_default_contact {
            contact.allow_receive = true;
            eprintln!(
                "[远程IM] 自动开启收信: contact_id={}, contact_name={}, channel_id={}, platform={:?}, reason=matched_default_contact",
                contact.id,
                contact.remote_contact_name,
                channel.id,
                channel.platform
            );
            allow_receive = true;
        }
    }
    if !allow_receive {
        state_write_app_data_cached(state, &data)?;
        drop(guard);
        return Err(format!("联系人未开启收信，跳过: contact_id={contact_id}"));
    }
    let (department_id, agent_id, conversation_id) = {
        let mut detached_contact = data
            .remote_im_contacts
            .get(contact_idx)
            .cloned()
            .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
        let resolved = resolve_contact_session_target(&config, &mut data, &mut detached_contact)?;
        data.remote_im_contacts[contact_idx] = detached_contact;
        resolved
    };
    eprintln!(
        "[远程IM] 入站消息路由完成: contact_id={}, channel_id={}, department_id={}, agent_id={}, conversation_id={}, route_mode={}, processing_mode={}",
        contact_id,
        input.channel_id.trim(),
        department_id,
        agent_id,
        conversation_id,
        data.remote_im_contacts[contact_idx].route_mode,
        data.remote_im_contacts[contact_idx].processing_mode
    );
    eprintln!(
        "[远程IM] 入站媒体摘要: contact_id={}, channel_id={}, text_len={}, image_count={}, image_mimes={:?}, audio_count={}, attachment_count={}, attachment_names={:?}",
        contact_id,
        input.channel_id.trim(),
        text.chars().count(),
        images.len(),
        images.iter().map(|item| item.mime.clone()).collect::<Vec<_>>(),
        audios.len(),
        attachments.len(),
        attachments.iter().map(|item| item.file_name.clone()).collect::<Vec<_>>()
    );
    if let Some(platform_message_id) = input
        .platform_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if remote_im_is_duplicate_platform_message(
            state,
            &data,
            &conversation_id,
            input.channel_id.trim(),
            input.remote_contact_type.trim(),
            input.remote_contact_id.trim(),
            platform_message_id,
        )? {
            eprintln!(
                "[远程IM] 入站消息去重: channel_id={}, contact_id={}, conversation_id={}, platform_message_id={}",
                input.channel_id.trim(),
                input.remote_contact_id.trim(),
                conversation_id,
                platform_message_id
            );
            state_write_app_data_cached(state, &data)?;
            drop(guard);
            return Ok(RemoteImEnqueueResult {
                event_id: String::new(),
                conversation_id,
                activate_assistant: false,
                contact_id,
            });
        }
    }
    let message = build_chat_message_from_input(
        &input,
        &conversation_id,
        &contact_id,
        &now,
        &text,
        &images,
        &audios,
        &attachments,
    );

    let event_id = Uuid::new_v4().to_string();
    let activate_assistant = remote_im_resolve_inbound_activate(&channel, input.activate_assistant);
    let event = create_pending_event(
        event_id.clone(),
        conversation_id.clone(),
        vec![message],
        activate_assistant,
        ChatSessionInfo {
            department_id,
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
    let ingress = ingress_chat_event(state, event)?;
    state_write_app_data_cached(state, &data)?;
    drop(guard);
    trigger_chat_event_after_ingress(state, ingress);
    Ok(RemoteImEnqueueResult {
        event_id,
        conversation_id,
        activate_assistant,
        contact_id,
    })
}
