async fn parse_and_enqueue_dingtalk_callback(
    channel: &RemoteImChannelConfig,
    callback_payload: &Value,
    state: &AppState,
) -> Result<RemoteImEnqueueResult, String> {
    if !callback_payload.is_object() {
        return Err("跳过: callback payload 不是对象".to_string());
    }
    let conversation_type = string_field(callback_payload, "conversationType");
    let remote_contact_type = if conversation_type == "2" {
        "group".to_string()
    } else {
        "private".to_string()
    };
    let sender_id = string_field(callback_payload, "senderId");
    let sender_staff_id = string_field(callback_payload, "senderStaffId");
    let remote_contact_id = if remote_contact_type == "group" {
        string_field(callback_payload, "conversationId")
    } else if !sender_staff_id.is_empty() {
        sender_staff_id.clone()
    } else {
        sender_id.clone()
    };
    if remote_contact_id.trim().is_empty() {
        return Err("钉钉消息缺少 contact id".to_string());
    }

    let sender_name = string_field(callback_payload, "senderNick");
    let remote_contact_name = if remote_contact_type == "group" {
        Some(string_field(callback_payload, "conversationTitle"))
    } else if !sender_name.is_empty() {
        Some(sender_name.clone())
    } else {
        None
    };
    let msg_type = string_field(callback_payload, "msgtype");
    if msg_type.is_empty() {
        return Err("跳过: 缺少 msgtype".to_string());
    }
    let robot_code = dingtalk_robot_code(channel, callback_payload);
    let mut text_chunks = Vec::<String>::new();
    let mut images = Vec::<BinaryPart>::new();
    let mut audios = Vec::<BinaryPart>::new();
    let mut attachments = Vec::<AttachmentMetaInput>::new();

    if msg_type == "text" {
        let text = callback_payload
            .get("text")
            .and_then(|v| v.get("content"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if !text.is_empty() {
            text_chunks.push(text.to_string());
        } else {
            return Err("跳过: 文本消息内容为空".to_string());
        }
    } else if msg_type == "picture" {
        let content = callback_payload
            .get("content")
            .ok_or_else(|| "跳过: picture 缺少 content".to_string())?;
        let download_code = callback_payload
            .get("content")
            .and_then(|v| v.get("downloadCode"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let _ = content;
        if download_code.is_empty() {
            return Err("跳过: picture 缺少 downloadCode".to_string());
        }
        if robot_code.is_empty() {
            return Err("跳过: picture 缺少 robotCode".to_string());
        }
        let (raw, mime) = dingtalk_download_file_by_code(channel, download_code, &robot_code).await?;
        let mime = normalize_dingtalk_image_mime(&raw, &mime);
        images.push(BinaryPart {
            mime,
            bytes_base64: B64.encode(raw),
            saved_path: None,
        });
    } else if msg_type == "richText" {
        let items = callback_payload
            .get("content")
            .and_then(|v| v.get("richText"))
            .and_then(Value::as_array)
            .ok_or_else(|| "跳过: richText 缺少 content.richText".to_string())?;
        for item in items {
            if let Some(text) = item.get("text").and_then(Value::as_str).map(str::trim) {
                if !text.is_empty() {
                    text_chunks.push(text.to_string());
                }
                continue;
            }
            if item.get("type").and_then(Value::as_str) == Some("picture") {
                let download_code = item
                    .get("downloadCode")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if download_code.is_empty() {
                    return Err("跳过: richText.picture 缺少 downloadCode".to_string());
                }
                if robot_code.is_empty() {
                    return Err("跳过: richText.picture 缺少 robotCode".to_string());
                }
                let (raw, mime) =
                    dingtalk_download_file_by_code(channel, download_code, &robot_code).await?;
                let mime = normalize_dingtalk_image_mime(&raw, &mime);
                images.push(BinaryPart {
                    mime,
                    bytes_base64: B64.encode(raw),
                    saved_path: None,
                });
            }
        }
    } else if msg_type == "audio" || msg_type == "voice" {
        let content = callback_payload
            .get("content")
            .cloned()
            .ok_or_else(|| "跳过: audio 缺少 content".to_string())?;
        let download_code = string_field(&content, "downloadCode");
        if download_code.is_empty() {
            return Err("跳过: audio 缺少 downloadCode".to_string());
        }
        if robot_code.is_empty() {
            return Err("跳过: audio 缺少 robotCode".to_string());
        }
        let (raw, mut mime) =
            dingtalk_download_file_by_code(channel, &download_code, &robot_code).await?;
        if mime == "application/octet-stream" {
            let ext = string_field(&content, "fileExtension");
            if !ext.is_empty() {
                mime = mime_from_name_fallback(&format!("voice.{ext}"));
            }
        }
        audios.push(BinaryPart {
            mime,
            bytes_base64: B64.encode(raw),
            saved_path: None,
        });
    } else if msg_type == "file" || msg_type == "video" {
        let content = callback_payload
            .get("content")
            .cloned()
            .ok_or_else(|| "跳过: file/video 缺少 content".to_string())?;
        let download_code = string_field(&content, "downloadCode");
        if download_code.is_empty() {
            return Err("跳过: file/video 缺少 downloadCode".to_string());
        }
        if robot_code.is_empty() {
            return Err("跳过: file/video 缺少 robotCode".to_string());
        }
        let (raw, mut mime) =
            dingtalk_download_file_by_code(channel, &download_code, &robot_code).await?;
        let mut file_name = string_field(&content, "fileName");
        if file_name.is_empty() {
            let ext = string_field(&content, "fileExtension");
            file_name = if ext.is_empty() {
                format!("dingtalk-{msg_type}-{}", Uuid::new_v4())
            } else {
                format!("dingtalk-{msg_type}-{}.{}", Uuid::new_v4(), ext.trim_matches('.'))
            };
        }
        if mime == "application/octet-stream" {
            mime = mime_from_name_fallback(&file_name);
        }
        let saved = persist_raw_attachment_to_downloads(state, &file_name, &mime, &raw)?;
        let relative_path = workspace_relative_path(state, &saved);
        attachments.push(AttachmentMetaInput {
            file_name,
            relative_path,
            mime,
        });
    }

    if text_chunks.is_empty() && images.is_empty() && audios.is_empty() && attachments.is_empty() {
        return Err("跳过: 钉钉消息无可入队内容".to_string());
    }

    let text = text_chunks.join("").trim().to_string();
    let mut provider_meta = serde_json::json!({
        "dingtalkRaw": callback_payload,
        "sessionWebhook": string_field(callback_payload, "sessionWebhook"),
        "sessionWebhookExpiredTime": int_field(callback_payload, "sessionWebhookExpiredTime"),
        "conversationId": string_field(callback_payload, "conversationId"),
        "conversationType": conversation_type,
        "senderStaffId": sender_staff_id,
        "msgtype": msg_type
    });
    if provider_meta.get("sessionWebhookExpiredTime").and_then(Value::as_i64).is_none() {
        if let Some(obj) = provider_meta.as_object_mut() {
            obj.remove("sessionWebhookExpiredTime");
        }
    }

    let input = RemoteImEnqueueInput {
        channel_id: channel.id.clone(),
        platform: RemoteImPlatform::Dingtalk,
        im_name: channel.name.clone(),
        remote_contact_type,
        remote_contact_id,
        remote_contact_name,
        sender_id: sender_id.clone(),
        sender_name: sender_name.clone(),
        sender_avatar_url: None,
        platform_message_id: {
            let mid = string_field(callback_payload, "msgId");
            if mid.is_empty() { None } else { Some(mid) }
        },
        dingtalk_session_webhook: {
            let value = string_field(callback_payload, "sessionWebhook");
            if value.is_empty() { None } else { Some(value) }
        },
        dingtalk_session_webhook_expired_time: int_field(callback_payload, "sessionWebhookExpiredTime"),
        activate_assistant: Some(channel.activate_assistant),
        session: SessionSelector {
            api_config_id: None,
            department_id: None,
            agent_id: String::new(),
            conversation_id: None,
        },
        payload: ChatInputPayload {
            text: if text.is_empty() { None } else { Some(text) },
            display_text: None,
            images: if images.is_empty() { None } else { Some(images) },
            audios: if audios.is_empty() { None } else { Some(audios) },
            attachments: if attachments.is_empty() {
                None
            } else {
                Some(attachments)
            },
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: Some(provider_meta),
        },
    };
    remote_im_enqueue_message_internal(input, state)
}

async fn run_single_dingtalk_stream_session(
    channel: &RemoteImChannelConfig,
    state: &AppState,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let client_id = credential_text(&channel.credentials, "clientId");
    let client_secret = credential_text(&channel.credentials, "clientSecret");
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(format!(
            "钉钉渠道 '{}' 缺少 clientId/clientSecret",
            channel.id
        ));
    }
    let manager = dingtalk_stream_manager();
    manager
        .add_log(
            &channel.id,
            "info",
            &format!(
                "[钉钉生命周期] 开始 task={} trigger=run_single_dingtalk_stream_session key_counts=0 duration_ms=0",
                channel.id
            ),
        )
        .await;
    manager
        .add_log(&channel.id, "info", "提示：群聊消息通常需要 @机器人 才会触发回调")
        .await;
    manager
        .set_state(
            &channel.id,
            false,
            Some("sdk-connecting".to_string()),
            None,
        )
        .await;

    let credential = dingtalk_stream::Credential::new(&client_id, &client_secret);
    let callback_handler = EasyCallDingtalkAsyncHandler {
        channel: channel.clone(),
        state: state.clone(),
        manager: manager.clone(),
    };
    let mut client = dingtalk_stream::DingTalkStreamClient::builder(credential)
        .register_async_chatbot_handler(DINGTALK_STREAM_TOPIC, callback_handler)
        .build();

    manager
        .add_log(&channel.id, "debug", "钉钉 Stream SDK start() 已启动")
        .await;
    manager
        .set_state(
            &channel.id,
            true,
            Some("sdk-running".to_string()),
            None,
        )
        .await;
    client
        .start()
        .await
        .map_err(|err| format!("钉钉 Stream SDK 运行失败: {err}"))?;
    manager
        .add_log(
            &channel.id,
            "info",
            &format!(
                "[钉钉生命周期] 完成 task={} trigger=run_single_dingtalk_stream_session key_counts=0 duration_ms={}",
                channel.id,
                started_at.elapsed().as_millis()
            ),
        )
        .await;
    Ok(())
}
