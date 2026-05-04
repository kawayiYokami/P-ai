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
    #[serde(default = "default_remote_im_contact_patience_seconds")]
    patience_seconds: u64,
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
struct RemoteImContactWorkspaceUpdateInput {
    contact_id: String,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_name: Option<String>,
    platform: RemoteImPlatform,
    contact_display_name: String,
    bound_department_id: Option<String>,
    processing_mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    preview_messages: Vec<ConversationPreviewMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationMessagesInput {
    contact_id: String,
}

pub(crate) fn remote_im_channel_by_id<'a>(
    config: &'a AppConfig,
    channel_id: &str,
) -> Option<&'a RemoteImChannelConfig> {
    config
        .remote_im_channels
        .iter()
        .find(|channel| channel.id == channel_id)
}

fn remote_im_upsert_contact_for_inbound(
    runtime: &mut RuntimeStateFile,
    channel: &RemoteImChannelConfig,
    input: &RemoteImEnqueueInput,
    now: &str,
) -> String {
    let default_allow_receive = remote_im_resolve_inbound_activate(channel, input.activate_assistant);
    if let Some(contact) = runtime.remote_im_contacts.iter_mut().find(|item| {
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
    runtime.remote_im_contacts.push(RemoteImContact {
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
        patience_seconds: default_remote_im_contact_patience_seconds(),
        activation_cooldown_seconds: 0,
        route_mode: "dedicated_contact_conversation".to_string(),
        bound_department_id: Some(REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string()),
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
        shell_workspaces: Vec::new(),
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
        _ => "dedicated_contact_conversation".to_string(),
    }
}

fn normalize_contact_processing_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "qa" => "qa".to_string(),
        _ => "continuous".to_string(),
    }
}

fn lock_remote_im_contact_runtime_states(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, std::collections::HashMap<String, RemoteImContactRuntimeState>>, String>
{
    state
        .remote_im_contact_runtime_states
        .lock()
        .map_err(|_| "无法获取远程 IM 联系人运行时状态的锁".to_string())
}

fn remote_im_contact_runtime_state_mut<'a>(
    states: &'a mut std::collections::HashMap<String, RemoteImContactRuntimeState>,
    contact_id: &str,
) -> &'a mut RemoteImContactRuntimeState {
    states
        .entry(contact_id.to_string())
        .or_insert_with(RemoteImContactRuntimeState::default)
}

#[cfg(test)]
fn remote_im_contact_checkpoint_mut<'a>(
    data: &'a mut AppData,
    contact_id: &str,
) -> &'a mut RemoteImContactCheckpoint {
    remote_im_contact_checkpoint_mut_in_list(&mut data.remote_im_contact_checkpoints, contact_id)
}

fn remote_im_contact_checkpoint_mut_in_list<'a>(
    checkpoints: &'a mut Vec<RemoteImContactCheckpoint>,
    contact_id: &str,
) -> &'a mut RemoteImContactCheckpoint {
    if let Some(index) = checkpoints
        .iter()
        .position(|item| item.contact_id == contact_id)
    {
        return &mut checkpoints[index];
    }
    checkpoints.push(RemoteImContactCheckpoint {
        contact_id: contact_id.to_string(),
        ..RemoteImContactCheckpoint::default()
    });
    let last_index = checkpoints.len().saturating_sub(1);
    &mut checkpoints[last_index]
}

fn remote_im_contact_by_source_in_runtime<'a>(
    contacts: &'a [RemoteImContact],
    source: &RemoteImMessageSource,
) -> Option<&'a RemoteImContact> {
    contacts.iter().find(|item| {
        item.channel_id == source.channel_id
            && item.remote_contact_type == source.remote_contact_type
            && item.remote_contact_id == source.remote_contact_id
    })
}

#[cfg(test)]
fn remote_im_contact_by_source<'a>(
    data: &'a AppData,
    source: &RemoteImMessageSource,
) -> Option<&'a RemoteImContact> {
    remote_im_contact_by_source_in_runtime(&data.remote_im_contacts, source)
}

fn remote_im_contact_by_activation_source_in_runtime<'a>(
    contacts: &'a [RemoteImContact],
    source: &RemoteImActivationSource,
) -> Option<&'a RemoteImContact> {
    contacts.iter().find(|item| {
        item.channel_id == source.channel_id
            && item.remote_contact_type == source.remote_contact_type
            && item.remote_contact_id == source.remote_contact_id
    })
}

fn remote_im_contact_matches_reply_target(
    source: &RemoteImActivationSource,
    target: &RemoteImReplyTarget,
) -> bool {
    source.channel_id.trim() == target.channel_id.trim()
        && source.remote_contact_id.trim() == target.contact_id.trim()
}

fn remote_im_text_contains_keyword(text: &str, keyword: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&keyword.to_ascii_lowercase())
}

fn remote_im_keyword_matched(contact: &RemoteImContact, message_text: &str) -> bool {
    contact
        .activation_keywords
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .any(|keyword| remote_im_text_contains_keyword(message_text, keyword))
}

fn remote_im_patience_exhausted(
    contact: &RemoteImContact,
    runtime: &RemoteImContactRuntimeState,
) -> bool {
    let Some(last_success_at) = runtime.last_success_reply_at.as_deref() else {
        return false;
    };
    let elapsed_seconds = parse_iso(last_success_at)
        .map(|last| (now_utc() - last).whole_seconds().max(0) as u64)
        .unwrap_or_default();
    elapsed_seconds > contact.patience_seconds
}

fn remote_im_should_activate_while_away(
    contact: &RemoteImContact,
    message_text: &str,
) -> (bool, String) {
    match contact.activation_mode.trim().to_ascii_lowercase().as_str() {
        "always" => (true, "away 命中 always，切换为在场".to_string()),
        "keyword" => {
            let matched = remote_im_keyword_matched(contact, message_text);
            if matched {
                (true, "away 命中 keyword，切换为在场".to_string())
            } else {
                (false, "away 未命中 keyword，仅记录消息".to_string())
            }
        }
        _ => (false, "away 命中 never，仅记录消息".to_string()),
    }
}

fn remote_im_prepare_enqueue_runtime_state(
    state: &AppState,
    contact: &RemoteImContact,
    message_text: &str,
) -> Result<(bool, String), String> {
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
    let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
    let (activate_assistant, reason) = match runtime.presence_state {
        RemoteImPresenceState::Away => {
            let (activate, reason) = remote_im_should_activate_while_away(contact, message_text);
            if activate {
                runtime.presence_state = RemoteImPresenceState::Present;
                runtime.needs_boundary = true;
                runtime.consecutive_no_reply_count = 0;
            }
            (activate, reason)
        }
        RemoteImPresenceState::Present => {
            let keyword_mode = contact.activation_mode.trim().eq_ignore_ascii_case("keyword");
            let keyword_matched = !keyword_mode || remote_im_keyword_matched(contact, message_text);
            if runtime.work_state == RemoteImWorkState::Idle
                && keyword_mode
                && !keyword_matched
                && remote_im_patience_exhausted(contact, runtime)
            {
                runtime.presence_state = RemoteImPresenceState::Away;
                runtime.has_pending = false;
                (
                    false,
                    "present + idle 未命中 keyword 且耐心耗尽，切换为离场".to_string(),
                )
            } else if runtime.work_state == RemoteImWorkState::Busy {
                runtime.has_pending = true;
                (false, "present + busy，新消息标记待办，等待当前轮次收尾续跑".to_string())
            } else {
                (true, "present + idle，等待本轮调度".to_string())
            }
        }
    };
    eprintln!(
        "[远程联系人状态机] 入站判定 完成: contact_id={}, presence={:?}, work={:?}, pending={}, activate_assistant={}, reason={}",
        contact.id,
        runtime.presence_state,
        runtime.work_state,
        runtime.has_pending,
        activate_assistant,
        reason
    );
    Ok((activate_assistant, reason))
}

fn remote_im_message_text_len(message: &ChatMessage) -> usize {
    let body_len = message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.chars().count()),
            _ => None,
        })
        .sum::<usize>();
    let extra_len = message
        .extra_text_blocks
        .iter()
        .map(|item| item.chars().count())
        .sum::<usize>();
    body_len + extra_len
}

fn remote_im_message_is_readable(message: &ChatMessage) -> bool {
    if matches!(
        message.role.trim(),
        "assistant" | "user"
    ) && !is_context_compaction_message(message, message.role.trim())
    {
        return remote_im_message_text_len(message) > 0;
    }
    false
}

fn remote_im_collect_recent_readable_messages(
    messages: &[ChatMessage],
    budget_chars: usize,
) -> Vec<ChatMessage> {
    let mut selected = Vec::<ChatMessage>::new();
    let mut used_chars = 0usize;
    for message in messages.iter().rev() {
        if is_context_compaction_message(message, message.role.trim()) {
            break;
        }
        if !remote_im_message_is_readable(message) {
            continue;
        }
        let message_chars = remote_im_message_text_len(message);
        if !selected.is_empty() && used_chars + message_chars > budget_chars {
            break;
        }
        used_chars += message_chars;
        selected.push(message.clone());
    }
    selected.reverse();
    selected
}

fn remote_im_build_presence_boundary_message(
    contact: &RemoteImContact,
    now: &str,
) -> ChatMessage {
    let text = if normalize_contact_processing_mode(&contact.processing_mode) == "qa" {
        "当前是问答模式，尽可能快地回答问题".to_string()
    } else {
        "上下文窗口已经滑动".to_string()
    };
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now.to_string(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text { text }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "scene": "remote_im_presence_boundary",
                "contactId": contact.id,
                "processingMode": normalize_contact_processing_mode(&contact.processing_mode),
            }
        })),
        tool_call: None,
        mcp_call: None,
    }
}

fn remote_im_refresh_conversation_activity(conversation: &mut Conversation) {
    conversation.last_user_at = conversation
        .messages
        .iter()
        .rev()
        .find(|message| message.role.trim() == "user")
        .map(|message| message.created_at.clone());
    conversation.last_assistant_at = conversation
        .messages
        .iter()
        .rev()
        .find(|message| message.role.trim() == "assistant")
        .map(|message| message.created_at.clone());
}

fn remote_im_apply_presence_boundary_to_conversation(
    conversation: &mut Conversation,
    checkpoints: &mut Vec<RemoteImContactCheckpoint>,
    contact: &RemoteImContact,
    now: &str,
) -> Result<(), String> {
    let retained = remote_im_collect_recent_readable_messages(&conversation.messages, 20_000);
    let boundary = remote_im_build_presence_boundary_message(contact, now);
    let boundary_id = boundary.id.clone();
    let mut next_messages = Vec::<ChatMessage>::with_capacity(retained.len() + 1);
    next_messages.push(boundary);
    next_messages.extend(retained);
    conversation.messages = next_messages;
    conversation.updated_at = now.to_string();
    remote_im_refresh_conversation_activity(conversation);

    let checkpoint = remote_im_contact_checkpoint_mut_in_list(checkpoints, &contact.id);
    let previous_cursor = checkpoint
        .latest_seen_message_id
        .clone()
        .unwrap_or_default();
    checkpoint.last_boundary_message_id = Some(boundary_id.clone());
    checkpoint.last_boundary_covers_message_id = checkpoint
        .latest_seen_message_id
        .clone()
        .or(checkpoint.last_boundary_covers_message_id.clone())
        .or(checkpoint.last_boundary_message_id.clone())
        .or(checkpoint.updated_at.clone());
    checkpoint.updated_at = Some(now.to_string());

    eprintln!(
        "[远程联系人状态机] 压缩边界 完成: contact_id={}, conversation_id={}, retained_messages={}, boundary_message_id={}, previous_cursor={}",
        contact.id,
        conversation.id,
        conversation.messages.len().saturating_sub(1),
        boundary_id,
        previous_cursor
    );
    Ok(())
}

#[cfg(test)]
fn remote_im_apply_presence_boundary_if_needed(
    data: &mut AppData,
    conversation_id: &str,
    contact: &RemoteImContact,
    now: &str,
) -> Result<(), String> {
    let (retained_count, boundary_id) = {
        let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|item| item.id == conversation_id)
        else {
            return Err(format!("目标会话不存在，conversation_id={conversation_id}"));
        };
        if !conversation_is_remote_im_contact(conversation) {
            eprintln!(
                "[远程联系人状态机] 压缩边界 跳过: contact_id={}, conversation_id={}, reason=not_dedicated_contact_conversation",
                contact.id, conversation_id
            );
            return Ok(());
        }

        remote_im_apply_presence_boundary_to_conversation(
            conversation,
            &mut data.remote_im_contact_checkpoints,
            contact,
            now,
        )?;
        (
            conversation.messages.len().saturating_sub(1),
            data.remote_im_contact_checkpoints
                .iter()
                .find(|item| item.contact_id == contact.id)
                .and_then(|item| item.last_boundary_message_id.clone())
                .unwrap_or_default(),
        )
    };

    eprintln!(
        "[远程联系人状态机] 压缩边界 完成: contact_id={}, conversation_id={}, retained_messages={}, boundary_message_id={}, previous_cursor={}",
        contact.id,
        conversation_id,
        retained_count,
        boundary_id,
        data.remote_im_contact_checkpoints
            .iter()
            .find(|item| item.contact_id == contact.id)
            .and_then(|item| item.latest_seen_message_id.clone())
            .unwrap_or_default()
    );
    Ok(())
}

fn remote_im_event_latest_message_id(event: &ChatPendingEvent) -> Option<String> {
    event.messages.last().map(|message| message.id.clone())
}

#[cfg(test)]
fn remote_im_update_checkpoint_latest_seen(
    data: &mut AppData,
    contact_id: &str,
    message_id: Option<&str>,
    now: &str,
) {
    let checkpoint = remote_im_contact_checkpoint_mut(data, contact_id);
    remote_im_update_checkpoint_latest_seen_in_checkpoint(checkpoint, message_id, now);
}

fn remote_im_update_checkpoint_latest_seen_in_list(
    checkpoints: &mut Vec<RemoteImContactCheckpoint>,
    contact_id: &str,
    message_id: Option<&str>,
    now: &str,
) {
    let checkpoint = remote_im_contact_checkpoint_mut_in_list(checkpoints, contact_id);
    remote_im_update_checkpoint_latest_seen_in_checkpoint(checkpoint, message_id, now);
}

fn remote_im_update_checkpoint_latest_seen_in_checkpoint(
    checkpoint: &mut RemoteImContactCheckpoint,
    message_id: Option<&str>,
    now: &str,
) {
    checkpoint.latest_seen_message_id = message_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(checkpoint.latest_seen_message_id.clone());
    checkpoint.updated_at = Some(now.to_string());
}

fn remote_im_handle_persisted_event_after_history_flush_runtime(
    state: &AppState,
    contacts: &[RemoteImContact],
    checkpoints: &mut Vec<RemoteImContactCheckpoint>,
    conversation: &mut Conversation,
    event: &ChatPendingEvent,
    now: &str,
    activated_contacts_in_batch: &mut std::collections::HashSet<String>,
) -> Result<bool, String> {
    let Some(sender) = event.sender_info.as_ref() else {
        return Ok(false);
    };
    let Some(contact) = remote_im_contact_by_source_in_runtime(contacts, sender).cloned() else {
        return Ok(false);
    };
    let latest_message_id = remote_im_event_latest_message_id(event);
    remote_im_update_checkpoint_latest_seen_in_list(
        checkpoints,
        &contact.id,
        latest_message_id.as_deref(),
        now,
    );
    if activated_contacts_in_batch.contains(&contact.id) {
        return Ok(false);
    }

    let mut should_activate = false;
    let mut should_apply_boundary = false;
    {
        let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
        let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
        match runtime.presence_state {
            RemoteImPresenceState::Away => {
                eprintln!(
                    "[远程联系人状态机] 历史落地后跳过: contact_id={}, reason=still_away",
                    contact.id
                );
            }
            RemoteImPresenceState::Present => {
                if runtime.work_state == RemoteImWorkState::Busy {
                    runtime.has_pending = true;
                    eprintln!(
                        "[远程联系人状态机] 历史落地后待办: contact_id={}, reason=busy_mark_pending",
                        contact.id
                    );
                } else {
                    should_apply_boundary = runtime.needs_boundary;
                    runtime.needs_boundary = false;
                    runtime.work_state = RemoteImWorkState::Busy;
                    runtime.has_pending = false;
                    should_activate = true;
                }
            }
        }
    }

    if should_apply_boundary {
        if !conversation_is_remote_im_contact(conversation) {
            eprintln!(
                "[远程联系人状态机] 压缩边界 跳过: contact_id={}, conversation_id={}, reason=not_dedicated_contact_conversation",
                contact.id,
                conversation.id
            );
        } else {
            remote_im_apply_presence_boundary_to_conversation(
                conversation,
                checkpoints,
                &contact,
                now,
            )?;
        }
    }
    if should_activate {
        activated_contacts_in_batch.insert(contact.id.clone());
        eprintln!(
            "[远程联系人状态机] 激活调度 开始: contact_id={}, conversation_id={}, boundary_applied={}",
            contact.id,
            conversation.id,
            should_apply_boundary
        );
    }
    Ok(should_activate)
}

#[cfg(test)]
fn remote_im_handle_persisted_event_after_history_flush(
    state: &AppState,
    data: &mut AppData,
    conversation_id: &str,
    event: &ChatPendingEvent,
    now: &str,
    activated_contacts_in_batch: &mut std::collections::HashSet<String>,
) -> Result<bool, String> {
    let Some(sender) = event.sender_info.as_ref() else {
        return Ok(false);
    };
    let Some(contact) = remote_im_contact_by_source(data, sender).cloned() else {
        return Ok(false);
    };
    let latest_message_id = remote_im_event_latest_message_id(event);
    remote_im_update_checkpoint_latest_seen(data, &contact.id, latest_message_id.as_deref(), now);
    if activated_contacts_in_batch.contains(&contact.id) {
        return Ok(false);
    }

    let mut should_activate = false;
    let mut should_apply_boundary = false;
    {
        let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
        let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
        match runtime.presence_state {
            RemoteImPresenceState::Away => {
                eprintln!(
                    "[远程联系人状态机] 历史落地后跳过: contact_id={}, reason=still_away",
                    contact.id
                );
            }
            RemoteImPresenceState::Present => {
                if runtime.work_state == RemoteImWorkState::Busy {
                    runtime.has_pending = true;
                    eprintln!(
                        "[远程联系人状态机] 历史落地后待办: contact_id={}, reason=busy_mark_pending",
                        contact.id
                    );
                } else {
                    should_apply_boundary = runtime.needs_boundary;
                    runtime.needs_boundary = false;
                    runtime.work_state = RemoteImWorkState::Busy;
                    runtime.has_pending = false;
                    should_activate = true;
                }
            }
        }
    }

    if should_apply_boundary {
        remote_im_apply_presence_boundary_if_needed(data, conversation_id, &contact, now)?;
    }
    if should_activate {
        activated_contacts_in_batch.insert(contact.id.clone());
        eprintln!(
            "[远程联系人状态机] 激活调度 开始: contact_id={}, conversation_id={}, boundary_applied={}",
            contact.id, conversation_id, should_apply_boundary
        );
    }
    Ok(should_activate)
}

fn remote_im_finalize_round_completion(
    state: &AppState,
    activated_sources: &[RemoteImActivationSource],
    reply_decision: Option<&str>,
    reply_target: Option<&RemoteImReplyTarget>,
    failed_error: Option<&str>,
    finished_at: &str,
) -> Result<Vec<RemoteImActivationSource>, String> {
    if activated_sources.is_empty() {
        return Ok(Vec::new());
    }
    let runtime = state_read_runtime_state_cached(state)?;
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
    let mut follow_up_sources = Vec::<RemoteImActivationSource>::new();
    for source in activated_sources {
        let Some(contact) =
            remote_im_contact_by_activation_source_in_runtime(&runtime.remote_im_contacts, source)
        else {
            continue;
        };
        let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
        runtime.work_state = RemoteImWorkState::Idle;
        let previous_presence = runtime.presence_state;
        let previous_pending = runtime.has_pending;
        let previous_no_reply_count = runtime.consecutive_no_reply_count;
        if let Some(error) = failed_error {
            eprintln!(
                "[远程联系人状态机] 轮次结束 失败: contact_id={}, presence={:?}->{:?}, pending={}, error={}",
                contact.id,
                previous_presence,
                runtime.presence_state,
                previous_pending,
                error
            );
            continue;
        }
        let should_follow_up_after_round = previous_pending;
        match reply_decision.unwrap_or("") {
            "reply" | "send_files" | "send" | "reply_async" => {
                let target_matched = reply_target
                    .map(|target| remote_im_contact_matches_reply_target(source, target))
                    .unwrap_or(activated_sources.len() == 1);
                runtime.presence_state = RemoteImPresenceState::Present;
                runtime.consecutive_no_reply_count = 0;
                if target_matched {
                    runtime.last_success_reply_at = Some(finished_at.to_string());
                }
            }
            "no_reply" => {
                runtime.consecutive_no_reply_count =
                    runtime.consecutive_no_reply_count.saturating_add(1);
                if runtime.has_pending {
                    runtime.presence_state = RemoteImPresenceState::Present;
                } else if runtime.consecutive_no_reply_count >= 2 {
                    runtime.presence_state = RemoteImPresenceState::Away;
                } else if let Some(last_success_at) = runtime.last_success_reply_at.as_deref() {
                    let elapsed_seconds = parse_iso(last_success_at)
                        .map(|last| (now_utc() - last).whole_seconds().max(0) as u64)
                        .unwrap_or_default();
                    if elapsed_seconds > contact.patience_seconds {
                        runtime.presence_state = RemoteImPresenceState::Away;
                    } else {
                        runtime.presence_state = RemoteImPresenceState::Present;
                    }
                } else {
                    runtime.presence_state = RemoteImPresenceState::Present;
                }
            }
            "send_async" | "" => {
                runtime.presence_state = RemoteImPresenceState::Present;
                runtime.consecutive_no_reply_count = 0;
            }
            _ => {}
        }
        if should_follow_up_after_round {
            runtime.has_pending = false;
            runtime.presence_state = RemoteImPresenceState::Present;
            follow_up_sources.push(source.clone());
        }
        eprintln!(
            "[远程联系人状态机] 轮次结束 完成: contact_id={}, decision={}, presence={:?}->{:?}, pending={}->{}, no_reply_count={}->{}, follow_up={}, last_success_reply_at={}",
            contact.id,
            reply_decision.unwrap_or(""),
            previous_presence,
            runtime.presence_state,
            previous_pending,
            runtime.has_pending,
            previous_no_reply_count,
            runtime.consecutive_no_reply_count,
            should_follow_up_after_round,
            runtime.last_success_reply_at.as_deref().unwrap_or("")
        );
    }
    Ok(follow_up_sources)
}

fn remote_im_finalize_async_send_result(
    state: &AppState,
    source: &RemoteImActivationSource,
    send_ok: bool,
    now: &str,
    error: Option<&str>,
) -> Result<(), String> {
    let runtime = state_read_runtime_state_cached(state)?;
    let Some(contact) =
        remote_im_contact_by_activation_source_in_runtime(&runtime.remote_im_contacts, source)
    else {
        return Ok(());
    };
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
    let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
    runtime.presence_state = RemoteImPresenceState::Present;
    runtime.consecutive_no_reply_count = 0;
    if send_ok {
        runtime.last_success_reply_at = Some(now.to_string());
    }
    eprintln!(
        "[远程联系人状态机] 异步发送{}: contact_id={}, last_success_reply_at={}, error={}",
        if send_ok { "完成" } else { "失败" },
        contact.id,
        runtime.last_success_reply_at.as_deref().unwrap_or(""),
        error.unwrap_or("")
    );
    Ok(())
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

fn remote_im_resolve_effective_route_mode(
    _config: &AppConfig,
    _contact: &RemoteImContact,
) -> String {
    "dedicated_contact_conversation".to_string()
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
    conversation_id: &str,
    channel_id: &str,
    remote_contact_type: &str,
    remote_contact_id: &str,
    platform_message_id: &str,
) -> Result<bool, String> {
    if state_read_conversation_cached(state, conversation_id)
        .map(|conversation| {
            conversation_has_remote_im_platform_message(
                &conversation,
                channel_id,
                remote_contact_type,
                remote_contact_id,
                platform_message_id,
            )
        })
        .unwrap_or(false)
    {
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
    let department = if let Some(department_id) = requested_department_id.as_deref() {
        department_by_id(config, department_id)
            .ok_or_else(|| format!("路由部门不存在: {department_id}"))?
    } else {
        let agent_id = if !requested_agent_id.is_empty() {
            requested_agent_id.clone()
        } else {
            assistant_department_agent_id(config)
                .ok_or_else(|| "路由信息不完整（缺少 agentId）".to_string())?
        };
        department_for_agent_id(config, &agent_id)
            .or_else(|| assistant_department(config))
            .ok_or_else(|| "路由部门不存在".to_string())?
    };
    let agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id
    } else if requested_department_id.is_some() {
        department
            .agent_ids
            .iter()
            .map(|id| id.trim())
            .find(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("部门没有可用人格：{}", department.id))?
    } else {
        assistant_department_agent_id(config)
            .ok_or_else(|| "路由信息不完整（缺少 agentId）".to_string())?
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

fn ensure_remote_im_contact_conversation_id(
    state: &AppState,
    contact: &mut RemoteImContact,
) -> Result<String, String> {
    if let Some(bound_conversation_id) = contact
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|conversation_id| {
            state_read_conversation_cached(state, conversation_id)
                .ok()
                .filter(|conversation| {
                    conversation.summary.trim().is_empty()
                        && conversation_is_remote_im_contact(conversation)
                })
                .map(|conversation| conversation.id)
        })
    {
        contact.bound_conversation_id = Some(bound_conversation_id.clone());
        return Ok(bound_conversation_id);
    }

    let target_key = remote_im_contact_conversation_key(contact);
    if let Some(found_id) = state_read_chat_index_cached(state)?
        .conversations
        .iter()
        .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
        .find(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_is_remote_im_contact(conversation)
                && conversation.root_conversation_id.as_deref().map(str::trim)
                    == Some(target_key.as_str())
        })
        .map(|conversation| conversation.id)
    {
        contact.bound_conversation_id = Some(found_id.clone());
        return Ok(found_id);
    }

    let mut conversation = build_conversation_record(
        "",
        "",
        "",
        &remote_im_contact_conversation_title(contact),
        CONVERSATION_KIND_REMOTE_IM_CONTACT,
        Some(target_key),
        None,
    );
    conversation.status = "inactive".to_string();
    let conversation_id = conversation.id.clone();
    state_schedule_conversation_persist(state, &conversation, true)?;
    contact.bound_conversation_id = Some(conversation_id.clone());
    Ok(conversation_id)
}

fn resolve_contact_session_target(
    state: &AppState,
    config: &AppConfig,
    _runtime: &mut RuntimeStateFile,
    contact: &mut RemoteImContact,
) -> Result<(String, String, String), String> {
    let effective_route_mode = remote_im_resolve_effective_route_mode(config, contact);
    contact.route_mode = effective_route_mode.clone();

    let (department_id, agent_id) = resolve_department_agent_pair(
        contact.bound_department_id.as_deref(),
        None,
        config,
    )?;
    let conversation_id = ensure_remote_im_contact_conversation_id(state, contact)?;
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
    data_path: &PathBuf,
) -> ChatMessage {
    let mut parts = Vec::<MessagePart>::new();
    let downloads_subdir = remote_im_conversation_downloads_subdir(conversation_id);
    if !text.is_empty() {
        parts.push(MessagePart::Text {
            text: text.to_string(),
        });
    }
    for img in images {
        let bytes_base64 =
            externalize_stored_binary_base64_in_downloads_subdir(
                data_path,
                &downloads_subdir,
                &img.mime,
                &img.bytes_base64,
            )
                .unwrap_or_else(|err| {
                    eprintln!(
                        "[远程IM] 入站图片外置化失败，保留原始内容: conversation_id={}，contact_id={}，mime={}，bytes_len={}，error={}",
                        conversation_id,
                        contact_id,
                        img.mime,
                        img.bytes_base64.len(),
                        err
                    );
                    img.bytes_base64.clone()
                });
        parts.push(MessagePart::Image {
            mime: img.mime.clone(),
            bytes_base64,
            name: None,
            compressed: false,
        });
    }
    for audio in audios {
        let bytes_base64 =
            externalize_stored_binary_base64_in_downloads_subdir(
                data_path,
                &downloads_subdir,
                &audio.mime,
                &audio.bytes_base64,
            )
                .unwrap_or_else(|err| {
                    eprintln!(
                        "[远程IM] 入站音频外置化失败，保留原始内容: conversation_id={}，contact_id={}，mime={}，bytes_len={}，error={}",
                        conversation_id,
                        contact_id,
                        audio.mime,
                        audio.bytes_base64.len(),
                        err
                    );
                    audio.bytes_base64.clone()
                });
        parts.push(MessagePart::Audio {
            mime: audio.mime.clone(),
            bytes_base64,
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

fn remote_im_conversation_downloads_subdir(conversation_id: &str) -> String {
    conversation_id.trim().to_string()
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
        queue_mode: ChatQueueMode::Normal,
        messages,
        activate_assistant,
        session_info,
        runtime_context: None,
        sender_info: Some(sender_info),
    }
}

#[tauri::command]
fn remote_im_list_channels(state: State<'_, AppState>) -> Result<Vec<RemoteImChannelConfig>, String> {
    let config = state_read_config_cached(&state)?;
    Ok(config.remote_im_channels)
}

#[tauri::command]
fn remote_im_list_contacts(state: State<'_, AppState>) -> Result<Vec<RemoteImContact>, String> {
    let runtime = state_read_runtime_state_cached(&state)?;
    let mut contacts = runtime.remote_im_contacts;
    contacts.sort_by(|a, b| {
        a.channel_id
            .cmp(&b.channel_id)
            .then_with(|| b.last_message_at.cmp(&a.last_message_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(contacts)
}

#[tauri::command]
fn remote_im_update_contact_allow_send(
    input: RemoteImContactAllowSendUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.allow_send = input.allow_send;
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_allow_send_files(
    input: RemoteImContactAllowSendFilesUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.allow_send_files = input.allow_send_files;
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_allow_receive(
    input: RemoteImContactAllowReceiveUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.allow_receive = input.allow_receive;
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_activation(
    input: RemoteImContactActivationUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.activation_mode = normalize_contact_activation_mode(&input.activation_mode);
    contact.activation_keywords = normalize_contact_activation_keywords(&input.activation_keywords);
    contact.patience_seconds = input.patience_seconds;
    contact.activation_cooldown_seconds = input.activation_cooldown_seconds;
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_remark(
    input: RemoteImContactRemarkUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.remark_name = input.remark_name.trim().to_string();
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_route_mode(
    input: RemoteImContactRouteModeUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let config = state_read_config_cached(&state)?;
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    let requested_mode = normalize_contact_route_mode(&input.route_mode);
    let final_mode = remote_im_resolve_effective_route_mode(&config, contact);
    if requested_mode != final_mode {
        eprintln!(
            "[远程IM] 联系人路由模式已被约束修正: contact_id={}, requested={}, final={}",
            contact.id, requested_mode, final_mode
        );
    }
    contact.route_mode = final_mode;
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_department_binding(
    input: RemoteImContactDepartmentBindingUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let config = state_read_config_cached(&state)?;
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
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
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_processing_mode(
    input: RemoteImContactProcessingModeUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.processing_mode = normalize_contact_processing_mode(&input.processing_mode);
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_workspace(
    input: RemoteImContactWorkspaceUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let contact = runtime
        .remote_im_contacts
        .iter_mut()
        .find(|item| item.id == input.contact_id)
        .ok_or_else(|| format!("未找到远程联系人：{}", input.contact_id))?;
    contact.shell_workspaces = input.shell_workspaces;
    let output = contact.clone();
    state_write_runtime_state_cached(&state, &runtime)?;
    Ok(output)
}

#[tauri::command]
fn remote_im_list_contact_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<RemoteImContactConversationSummary>, String> {
    let started_at = std::time::Instant::now();
    runtime_log_debug("[远程IM][联系人会话][列表] 开始".to_string());
    let items = conversation_service().list_remote_im_contact_conversations(state.inner())?;
    runtime_log_debug(format!(
        "[远程IM][联系人会话][列表] 完成: contact_count={}, elapsed_ms={}",
        items.len(),
        started_at.elapsed().as_millis()
    ));
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
    runtime_log_debug(format!(
        "[远程IM][联系人会话][读取] 开始: contact_id={}",
        contact_id
    ));
    let messages =
        conversation_service().read_remote_im_contact_conversation_messages(state.inner(), contact_id)?;
    runtime_log_debug(format!(
        "[远程IM][联系人会话][读取] 完成: contact_id={}, message_count={}, elapsed_ms={}",
        contact_id,
        messages.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationBlockPageInput {
    contact_id: String,
    #[serde(default)]
    block_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationBlockSummaryOutput {
    block_id: u32,
    message_count: usize,
    first_message_id: String,
    last_message_id: String,
    first_created_at: Option<String>,
    last_created_at: Option<String>,
    is_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationBlockPageOutput {
    blocks: Vec<RemoteImContactConversationBlockSummaryOutput>,
    selected_block_id: u32,
    messages: Vec<ChatMessage>,
    has_prev_block: bool,
    has_next_block: bool,
}

#[tauri::command]
fn remote_im_get_contact_conversation_block_page(
    input: RemoteImContactConversationBlockPageInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContactConversationBlockPageOutput, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let started_at = std::time::Instant::now();
    runtime_log_debug(format!(
        "[远程IM][联系人会话][块分页] 开始: contact_id={}, requested_block_id={}",
        contact_id,
        input.block_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "latest".to_string())
    ));
    let page = conversation_service().read_remote_im_contact_conversation_block_page(
        state.inner(),
        contact_id,
        input.block_id,
    )?;
    runtime_log_debug(format!(
        "[远程IM][联系人会话][块分页] 完成: contact_id={}, selected_block_id={}, message_count={}, elapsed_ms={}",
        contact_id,
        page.selected_block_id,
        page.messages.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(RemoteImContactConversationBlockPageOutput {
        blocks: page
            .blocks
            .into_iter()
            .map(|item| RemoteImContactConversationBlockSummaryOutput {
                block_id: item.block_id,
                message_count: item.message_count,
                first_message_id: item.first_message_id,
                last_message_id: item.last_message_id,
                first_created_at: item.first_created_at,
                last_created_at: item.last_created_at,
                is_latest: item.is_latest,
            })
            .collect(),
        selected_block_id: page.selected_block_id,
        messages: page.messages,
        has_prev_block: page.has_prev_block,
        has_next_block: page.has_next_block,
    })
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
    let mut runtime = state_read_runtime_state_cached(&state)?;
    let before_contacts = runtime.remote_im_contacts.len();
    runtime.remote_im_contacts
        .retain(|item| item.id != contact_id);
    let removed = runtime.remote_im_contacts.len() != before_contacts;
    if removed {
        state_write_runtime_state_cached(&state, &runtime)?;
    }
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
    let cleared =
        conversation_service().clear_remote_im_contact_conversation(state.inner(), contact_id)?;
    eprintln!(
        "[远程IM][联系人会话][清空] 完成: contact_id={}, elapsed_ms={}",
        contact_id,
        started_at.elapsed().as_millis()
    );
    Ok(cleared)
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
    let config = state_read_config_cached(state)?;
    let mut runtime = state_read_runtime_state_cached(state)?;
    let validated = validate_enqueue_input(&input, &config)?;
    let channel = validated.channel;
    let text = validated.text;
    let images = validated.images;
    let audios = validated.audios;
    let attachments = validated.attachments;

    let now = now_iso();
    let contact_id = remote_im_upsert_contact_for_inbound(&mut runtime, &channel, &input, &now);
    let contact_idx = runtime
        .remote_im_contacts
        .iter()
        .position(|item| item.id == contact_id)
        .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
    let contact = runtime
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
        state_write_runtime_state_cached(state, &runtime)?;
        return Err(format!("联系人未开启收信，跳过: contact_id={contact_id}"));
    }
    let (department_id, agent_id, conversation_id) = {
        let mut detached_contact = runtime
            .remote_im_contacts
            .get(contact_idx)
            .cloned()
            .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
        let resolved =
            resolve_contact_session_target(state, &config, &mut runtime, &mut detached_contact)?;
        runtime.remote_im_contacts[contact_idx] = detached_contact;
        resolved
    };
    eprintln!(
        "[远程IM] 入站消息路由完成: contact_id={}, channel_id={}, department_id={}, agent_id={}, conversation_id={}, route_mode={}, processing_mode={}",
        contact_id,
        input.channel_id.trim(),
        department_id,
        agent_id,
        conversation_id,
        runtime.remote_im_contacts[contact_idx].route_mode,
        runtime.remote_im_contacts[contact_idx].processing_mode
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
            state_write_runtime_state_cached(state, &runtime)?;
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
        &state.data_path,
    );

    let (activate_assistant, state_reason) = remote_im_prepare_enqueue_runtime_state(
        state,
        &runtime.remote_im_contacts[contact_idx],
        &text,
    )?;
    eprintln!(
        "[远程联系人状态机] 入站消息 接入: contact_id={}, conversation_id={}, activate_assistant={}, reason={}",
        contact_id, conversation_id, activate_assistant, state_reason
    );

    let event_id = Uuid::new_v4().to_string();
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
    state_write_runtime_state_cached(state, &runtime)?;
    trigger_chat_event_after_ingress(state, ingress);
    Ok(RemoteImEnqueueResult {
        event_id,
        conversation_id,
        activate_assistant,
        contact_id,
    })
}
