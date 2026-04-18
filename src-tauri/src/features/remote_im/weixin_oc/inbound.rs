fn weixin_oc_message_text(item_list: &[WeixinOcMessageItem]) -> String {
    let mut parts = Vec::<String>::new();
    for item in item_list {
        match item.item_type.unwrap_or(0) {
            1 => {
                let text = item
                    .text_item
                    .as_ref()
                    .and_then(|value| value.text.as_deref())
                    .map(str::trim)
                    .unwrap_or("");
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    parts.join("\n").trim().to_string()
}

fn weixin_oc_contact_display_name(
    data: &AppData,
    channel: &RemoteImChannelConfig,
    user_id: &str,
) -> String {
    let user_alias = data.user_alias.trim();
    if !user_alias.is_empty() {
        return user_alias.to_string();
    }
    let persona_name = user_persona_name(data);
    if !persona_name.trim().is_empty() {
        return persona_name.trim().to_string();
    }
    let channel_name = channel.name.trim();
    if !channel_name.is_empty() {
        return channel_name.to_string();
    }
    let normalized_user_id = user_id.trim();
    if !normalized_user_id.is_empty() {
        return normalized_user_id.to_string();
    }
    "个人微信".to_string()
}

async fn handle_weixin_oc_inbound_message(
    channel: &RemoteImChannelConfig,
    state: &AppState,
    msg: WeixinOcInboundMessage,
) -> Result<(), String> {
    let from_user_id = msg
        .from_user_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if from_user_id.is_empty() {
        return Ok(());
    }
    if let Some(token) = msg
        .context_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        weixin_oc_manager()
            .set_context_token(&channel.id, from_user_id, token)
            .await;
    }
    let item_list = msg.item_list.unwrap_or_default();
    let text = weixin_oc_message_text(&item_list);
    let creds = WeixinOcCredentials::from_value(&channel.credentials);
    let client = build_weixin_oc_http_client(creds.normalized_api_timeout_ms())?;
    let media = weixin_oc_collect_media(state, &client, &item_list).await?;
    let data = state_read_app_data_cached(state)?;
    let display_name = weixin_oc_contact_display_name(&data, channel, from_user_id);
    let message_id = msg
        .message_id
        .or(msg.msg_id)
        .map(|value| value.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    remote_im_enqueue_message_internal(
        RemoteImEnqueueInput {
            channel_id: channel.id.clone(),
            platform: RemoteImPlatform::WeixinOc,
            im_name: "weixin".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: from_user_id.to_string(),
            remote_contact_name: Some(display_name.clone()),
            sender_id: from_user_id.to_string(),
            sender_name: display_name,
            sender_avatar_url: None,
            platform_message_id: Some(message_id),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            activate_assistant: Some(channel.activate_assistant),
            session: SessionSelector {
                api_config_id: None,
                conversation_id: None,
                department_id: None,
                agent_id: String::new(),
            },
            payload: ChatInputPayload {
                text: if text.is_empty() { None } else { Some(text.clone()) },
                display_text: if text.is_empty() { None } else { Some(text) },
                images: if media.images.is_empty() {
                    None
                } else {
                    Some(media.images)
                },
                audios: if media.audios.is_empty() {
                    None
                } else {
                    Some(media.audios)
                },
                attachments: if media.attachments.is_empty() {
                    None
                } else {
                    Some(media.attachments)
                },
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: msg.context_token.map(|token| {
                    serde_json::json!({
                        "contextToken": token,
                    })
                }),
            },
        },
        state,
    )?;
    Ok(())
}

async fn run_single_weixin_oc_poll_cycle(
    channel_id: &str,
    state: &AppState,
) -> Result<(), String> {
    let channel = {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let config = state_read_config_cached(state)?;
        let channel = config
            .remote_im_channels
            .iter()
            .find(|item| item.id == channel_id)
            .cloned()
            .ok_or_else(|| format!("个人微信渠道不存在: {channel_id}"))?;
        drop(guard);
        channel
    };
    let creds = WeixinOcCredentials::from_value(&channel.credentials);
    let token = creds.token.trim().to_string();
    if token.is_empty() {
        return Err("缺少 token，请先扫码登录".to_string());
    }
    let body = serde_json::json!({
        "base_info": {
            "channel_version": "easy_call_ai"
        },
        "get_updates_buf": creds.sync_buf,
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 getupdates 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(&token))?;
    let client = build_weixin_oc_http_client(creds.normalized_long_poll_timeout_ms())?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/getupdates",
            creds.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 getupdates 失败: {err}"))?;
    let status_code = resp.status();
    if !status_code.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("请求 getupdates 失败: status={} body={}", status_code, text));
    }
    let data = resp
        .json::<WeixinOcGetUpdatesResp>()
        .await
        .map_err(|err| format!("解析 getupdates 响应失败: {err}"))?;
    if data.ret.unwrap_or(0) != 0 || data.errcode.unwrap_or(0) != 0 {
        return Err(format!(
            "getupdates 返回错误: ret={} errcode={} errmsg={}",
            data.ret.unwrap_or(0),
            data.errcode.unwrap_or(0),
            data.errmsg.unwrap_or_default()
        ));
    }
    if let Some(next_sync_buf) = data
        .get_updates_buf
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let mut writable = state_read_config_cached(state)?;
        if let Some(writable_channel) = writable
            .remote_im_channels
            .iter_mut()
            .find(|item| item.id == channel.id)
        {
            let mut next_creds = WeixinOcCredentials::from_value(&writable_channel.credentials);
            if next_creds.sync_buf.trim() != next_sync_buf {
                next_creds.sync_buf = next_sync_buf.to_string();
                writable_channel.credentials = serde_json::to_value(&next_creds)
                    .map_err(|err| format!("序列化个人微信凭证失败: {err}"))?;
                state_write_config_cached(state, &writable)?;
            }
        }
        drop(guard);
    }
    for msg in data.msgs.unwrap_or_default() {
        handle_weixin_oc_inbound_message(&channel, state, msg).await?;
    }
    Ok(())
}

fn upsert_weixin_oc_contact(
    data: &mut AppData,
    channel: &RemoteImChannelConfig,
    user_id: &str,
) -> (String, bool) {
    let normalized_user_id = user_id.trim();
    let display_name = weixin_oc_contact_display_name(data, channel, normalized_user_id);
    if let Some(contact) = data.remote_im_contacts.iter_mut().find(|item| {
        item.channel_id == channel.id
            && item.remote_contact_type == "private"
            && item.remote_contact_id == normalized_user_id
    }) {
        let current_name = contact.remote_contact_name.trim();
        if current_name.is_empty() || current_name == normalized_user_id {
            contact.remote_contact_name = display_name;
        }
        return (contact.id.clone(), false);
    }

    let contact_id = Uuid::new_v4().to_string();
    data.remote_im_contacts.push(RemoteImContact {
        id: contact_id.clone(),
        channel_id: channel.id.clone(),
        platform: RemoteImPlatform::WeixinOc,
        remote_contact_type: "private".to_string(),
        remote_contact_id: normalized_user_id.to_string(),
        remote_contact_name: display_name,
        remark_name: String::new(),
        allow_send: true,
        allow_send_files: false,
        allow_receive: channel.activate_assistant,
        activation_mode: "never".to_string(),
        activation_keywords: Vec::new(),
        activation_cooldown_seconds: 0,
        route_mode: "main_session".to_string(),
        bound_department_id: None,
        bound_conversation_id: None,
        processing_mode: "continuous".to_string(),
        last_activated_at: None,
        last_message_at: None,
        dingtalk_session_webhook: None,
        dingtalk_session_webhook_expired_time: None,
    });
    (contact_id, true)
}

#[cfg(test)]
mod weixin_oc_inbound_tests {
    use super::*;

    #[test]
    fn weixin_oc_contact_display_name_prefers_user_alias() {
        let mut data = AppData::default();
        data.user_alias = "派蒙".to_string();
        let channel = RemoteImChannelConfig {
            id: "channel-1".to_string(),
            name: "我的微信".to_string(),
            platform: RemoteImPlatform::WeixinOc,
            enabled: true,
            credentials: serde_json::json!({}),
            activate_assistant: true,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_send_files: false,
        };

        let display_name = weixin_oc_contact_display_name(&data, &channel, "wxid_123");

        assert_eq!(display_name, "派蒙".to_string());
    }
}

fn sync_weixin_oc_contact_from_user_id(
    state: &AppState,
    channel: &RemoteImChannelConfig,
    user_id: &str,
) -> Result<(String, bool), String> {
    let normalized_user_id = user_id.trim();
    if normalized_user_id.is_empty() {
        return Err("当前登录状态没有返回联系人 user_id，暂时无法补录联系人".to_string());
    }
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(state)?;
    let data_before = data.clone();
    let result = upsert_weixin_oc_contact(&mut data, channel, normalized_user_id);
    persist_app_data_conversation_runtime_delta(state, &data_before, &data)?;
    drop(guard);
    Ok(result)
}

pub(crate) async fn weixin_oc_send_text_message(
    credentials: WeixinOcCredentials,
    to_user_id: &str,
    text: &str,
    context_token: Option<&str>,
) -> Result<String, String> {
    let client = build_weixin_oc_http_client(credentials.normalized_api_timeout_ms())?;
    let client_id = Uuid::new_v4().to_string();
    let body = serde_json::json!({
        "base_info": {
            "channel_version": "easy_call_ai"
        },
        "msg": {
            "from_user_id": "",
            "to_user_id": to_user_id,
            "client_id": client_id,
            "message_type": 2,
            "message_state": 2,
            "context_token": context_token.map(str::trim).filter(|value| !value.is_empty()),
            "item_list": [
                {
                    "type": 1,
                    "text_item": {
                        "text": text
                    }
                }
            ]
        }
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 sendmessage 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(credentials.token.as_str()))?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/sendmessage",
            credentials.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 sendmessage 失败: {err}"))?;
    let status_code = resp.status();
    if !status_code.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("请求 sendmessage 失败: status={} body={}", status_code, body));
    }
    Ok(client_id)
}

#[tauri::command]
async fn remote_im_weixin_oc_start_login(
    input: WeixinOcLoginStartInput,
    state: State<'_, AppState>,
) -> Result<WeixinOcLoginStartResult, String> {
    weixin_oc_manager().start_login(state.inner(), input).await
}

#[tauri::command]
async fn remote_im_weixin_oc_get_login_status(
    input: WeixinOcLoginStatusInput,
    state: State<'_, AppState>,
) -> Result<WeixinOcLoginStatusResult, String> {
    weixin_oc_manager()
        .poll_login_status(state.inner(), input)
        .await
}

#[tauri::command]
async fn remote_im_weixin_oc_logout(
    input: WeixinOcLoginStatusInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    weixin_oc_manager()
        .logout(state.inner(), input.channel_id.as_str())
        .await?;
    Ok(true)
}

#[tauri::command]
async fn remote_im_weixin_oc_sync_contacts(
    input: WeixinOcLoginStatusInput,
    state: State<'_, AppState>,
) -> Result<WeixinOcSyncContactsResult, String> {
    let config = state_read_config_cached(state.inner())?;
    let channel = remote_im_channel_by_id(&config, &input.channel_id)
        .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
    if channel.platform != RemoteImPlatform::WeixinOc {
        return Err("该渠道不是个人微信渠道".to_string());
    }
    let creds = WeixinOcCredentials::from_value(&channel.credentials);
    if creds.account_id.trim().is_empty() || creds.token.trim().is_empty() {
        return Ok(WeixinOcSyncContactsResult {
            channel_id: input.channel_id,
            synced_count: 0,
            message: "当前还没有完成扫码登录，请先登录后再同步联系人。".to_string(),
        });
    }
    let user_id = creds.user_id.trim().to_string();
    let (_, created) = sync_weixin_oc_contact_from_user_id(state.inner(), channel, &user_id)?;
    Ok(WeixinOcSyncContactsResult {
        channel_id: input.channel_id,
        synced_count: 1,
        message: if created {
            format!("已同步个人微信联系人：{}", user_id)
        } else {
            format!("联系人已存在，无需重复同步：{}", user_id)
        },
    })
}


