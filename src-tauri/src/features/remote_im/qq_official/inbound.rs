// ==================== QQ 官方机器人：入站事件 ====================

/// 入站事件解析出的身份与内容信息（不含附件）。
#[derive(Debug, Clone)]
struct QqOfficialInboundIdentity {
    message_id: String,
    message_type: i64,
    content: String,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: Option<String>,
    sender_id: String,
    sender_name: String,
}

/// 网关 Dispatch 事件分发入口。
async fn qq_official_dispatch_event(
    channel: &RemoteImChannelConfig,
    state: &AppState,
    event_name: &str,
    data: &Value,
) {
    let result = match event_name {
        QQ_OFFICIAL_EVENT_C2C_MESSAGE_CREATE => {
            qq_official_parse_and_enqueue_message(channel, state, data, false).await
        }
        QQ_OFFICIAL_EVENT_GROUP_AT_MESSAGE_CREATE | QQ_OFFICIAL_EVENT_GROUP_MESSAGE_CREATE => {
            qq_official_parse_and_enqueue_message(channel, state, data, true).await
        }
        _ => return,
    };
    match result {
        Ok(result) => {
            qq_official_manager()
                .add_log(
                    &channel.id,
                    "info",
                    &format!(
                        "[QQ官方机器人] 消息已入队: event={}, contact={}",
                        event_name, result.contact_id
                    ),
                )
                .await;
        }
        Err(err) => {
            qq_official_manager()
                .add_log(
                    &channel.id,
                    "warn",
                    &format!(
                        "[QQ官方机器人] 消息未入队: event={}, reason={}",
                        event_name, err
                    ),
                )
                .await;
        }
    }
}

/// 解析事件的身份、文本与消息类型（纯函数，便于单测）。
fn qq_official_parse_inbound_identity(
    data: &Value,
    is_group: bool,
) -> Result<QqOfficialInboundIdentity, String> {
    if !data.is_object() {
        return Err("事件数据不是对象".to_string());
    }
    let message_id = data
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let author = data.get("author").cloned().unwrap_or(Value::Null);
    let username = author
        .get("username")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();

    let (remote_contact_type, remote_contact_id, remote_contact_name, sender_id) = if is_group {
        let group_openid = data
            .get("group_openid")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if group_openid.is_empty() {
            return Err("群聊事件缺少 group_openid".to_string());
        }
        let member_openid = author
            .get("member_openid")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        // 群名称需要「获取群基本信息」接口，该接口对机器人有白名单限制，一期不做查询。
        ("group".to_string(), group_openid, None, member_openid)
    } else {
        let user_openid = author
            .get("user_openid")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if user_openid.is_empty() {
            return Err("单聊事件缺少 user_openid".to_string());
        }
        let contact_name = if username.is_empty() {
            None
        } else {
            Some(username.clone())
        };
        (
            "private".to_string(),
            user_openid.clone(),
            contact_name,
            user_openid,
        )
    };
    if sender_id.is_empty() {
        return Err("事件缺少发送者标识".to_string());
    }

    let content = data
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let message_type = data
        .get("message_type")
        .and_then(qq_official_json_i64)
        .unwrap_or(QQ_OFFICIAL_MESSAGE_TYPE_PLAIN);
    let sender_name = if username.is_empty() {
        sender_id.clone()
    } else {
        username
    };

    Ok(QqOfficialInboundIdentity {
        message_id,
        message_type,
        content,
        remote_contact_type,
        remote_contact_id,
        remote_contact_name,
        sender_id,
        sender_name,
    })
}

/// 依据身份信息与入站部件构造入队输入（纯函数，便于单测）。
fn qq_official_build_enqueue_input(
    channel: &RemoteImChannelConfig,
    identity: &QqOfficialInboundIdentity,
    data: &Value,
    parts: Vec<ChatIngressPart>,
    is_group: bool,
) -> RemoteImEnqueueInput {
    let text = parts
        .iter()
        .filter_map(|part| match part {
            ChatIngressPart::Text { text } => Some(text.trim()),
            ChatIngressPart::Attachment { .. } => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let provider_meta = serde_json::json!({
        "qqOfficialRaw": data,
        "messageId": identity.message_id,
        "messageType": identity.message_type,
        "messageScene": data.get("message_scene").cloned().unwrap_or(Value::Null),
        "groupId": if is_group { Value::String(identity.remote_contact_id.clone()) } else { Value::Null },
        "userOpenId": if is_group { Value::Null } else { Value::String(identity.remote_contact_id.clone()) },
        "memberOpenId": if is_group { Value::String(identity.sender_id.clone()) } else { Value::Null },
    });

    RemoteImEnqueueInput {
        channel_id: channel.id.clone(),
        platform: RemoteImPlatform::QqOfficial,
        im_name: channel.name.clone(),
        remote_contact_type: identity.remote_contact_type.clone(),
        remote_contact_id: identity.remote_contact_id.clone(),
        remote_contact_name: identity.remote_contact_name.clone(),
        sender_id: identity.sender_id.clone(),
        sender_name: identity.sender_name.clone(),
        sender_avatar_url: None,
        platform_message_id: if identity.message_id.is_empty() {
            None
        } else {
            Some(identity.message_id.clone())
        },
        dingtalk_session_webhook: None,
        dingtalk_session_webhook_expired_time: None,
        session: SessionSelector {
            api_config_id: None,
            agent_id: String::new(),
            conversation_id: None,
        },
        payload: ChatInputPayload {
            text: if text.is_empty() { None } else { Some(text) },
            display_text: None,
            parts: Some(parts),
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: Some(provider_meta),
        },
    }
}

/// 解析消息事件并入队；返回入队结果（含联系人记录 ID）。
async fn qq_official_parse_and_enqueue_message(
    channel: &RemoteImChannelConfig,
    state: &AppState,
    data: &Value,
    is_group: bool,
) -> Result<RemoteImEnqueueResult, String> {
    let identity = qq_official_parse_inbound_identity(data, is_group)?;

    let mut parts = Vec::<ChatIngressPart>::new();
    if !identity.content.is_empty() {
        parts.push(ChatIngressPart::Text {
            text: identity.content.clone(),
        });
    } else if identity.message_type == QQ_OFFICIAL_MESSAGE_TYPE_QUOTE {
        parts.push(ChatIngressPart::Text {
            text: "[引用消息]".to_string(),
        });
    }

    let credentials = QqOfficialCredentials::from_value(&channel.credentials);
    let attachments = data
        .get("attachments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for attachment in &attachments {
        match qq_official_download_attachment(&credentials, attachment).await {
            Ok(part) => parts.push(part),
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程IM][QQ官方机器人] 附件下载失败，已跳过并继续，error={err}"
                ));
                parts.push(ChatIngressPart::Text {
                    text: format!("[附件不可用：{err}，已跳过并继续]"),
                });
            }
        }
    }
    if parts.is_empty() {
        return Err("事件无可入队内容".to_string());
    }

    let input = qq_official_build_enqueue_input(channel, &identity, data, parts, is_group);
    let result = remote_im_enqueue_message_internal(input, state).await?;
    // 被动回复上下文按平台会话标识（openid）记录，必须与出站查询键保持一致。
    if !identity.message_id.is_empty() && !identity.remote_contact_id.is_empty() {
        qq_official_manager()
            .record_passive_context(&channel.id, &identity.remote_contact_id, &identity.message_id)
            .await;
    }
    Ok(result)
}
