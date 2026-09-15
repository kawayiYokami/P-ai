async fn remote_im_get_channel_status_inner(
    state: &AppState,
    channel_id: String,
) -> Result<ChannelConnectionStatus, String> {
    let config = state_read_config_cached(state).map_err(|e| format!("{e:?}"))?;
    if let Some(channel) = config
        .remote_im_channels
        .iter()
        .find(|ch| ch.id == channel_id)
    {
        return match channel.platform {
            RemoteImPlatform::OnebotV11 => get_channel_connection_status(channel_id).await,
            RemoteImPlatform::Dingtalk => Ok(dingtalk_stream_manager()
                .get_channel_status(&channel.id)
                .await),
            RemoteImPlatform::Feishu => Ok(ChannelConnectionStatus {
                channel_id: channel.id.clone(),
                connected: false,
                peer_addr: None,
                connected_at: None,
                listen_addr: String::new(),
                status_text: None,
                last_error: None,
                account_id: None,
                base_url: None,
                login_session_key: None,
                qrcode_url: None,
            }),
            RemoteImPlatform::WeixinOc => Ok(weixin_oc_manager().build_status(&channel.id).await),
        };
    }
    get_channel_connection_status(channel_id).await
}

async fn ide_chat_remote_im_get_channel_status_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let channel_id = match params {
        Value::Object(mut map) => map
            .remove("channelId")
            .or_else(|| map.remove("channel_id"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default(),
        _ => String::new(),
    };
    ide_chat_serialize(remote_im_get_channel_status_inner(state, channel_id).await?)
}

async fn remote_im_restart_channel_inner(
    channel_id: String,
    state: &AppState,
) -> Result<ChannelConnectionStatus, String> {
    let channel_id = channel_id.trim().to_string();
    if channel_id.is_empty() {
        return Err("channelId 为必填项。".to_string());
    }
    runtime_log_info(format!("[远程IM] 重启渠道: {}", channel_id));
    onebot_v11_ws_manager()
        .add_log(&channel_id, "info", "[远程IM] 收到渠道重启请求")
        .await;
    let config = state_read_config_cached(state)?;
    let channel = config
        .remote_im_channels
        .iter()
        .find(|ch| ch.id == channel_id)
        .ok_or_else(|| format!("渠道 {} 未找到", channel_id))?
        .clone();
    onebot_v11_ws_manager()
        .add_log(
            &channel_id,
            "info",
            &format!(
                "[远程IM] 当前渠道配置: enabled={}, platform={:?}",
                channel.enabled, channel.platform
            ),
        )
        .await;

    let effective_channel = remote_im_channel_with_effective_credentials(state, &channel)?;
    let manager = onebot_v11_ws_manager();
    manager
        .reconcile_channel_runtime(&effective_channel)
        .await
        .map_err(|err| format!("重启渠道失败: {}", err))?;
    runtime_log_info(format!(
        "[远程IM] 渠道 {} 已按配置收敛: enabled={}, platform={:?}",
        channel_id, channel.enabled, channel.platform
    ));

    if channel.enabled && channel.platform == RemoteImPlatform::OnebotV11 {
        manager
            .start_event_consumer(channel_id.clone(), state.clone())
            .await
            .map_err(|err| format!("重启事件消费器失败: {}", err))?;
    } else if channel.enabled && channel.platform == RemoteImPlatform::Dingtalk {
        let state_clone = state.clone();
        let manager = dingtalk_stream_manager();
        let channel_clone = remote_im_channel_with_effective_credentials(&state_clone, &channel)?;
        tauri::async_runtime::spawn(async move {
            if let Err(err) = manager
                .reconcile_channel_runtime(&channel_clone, state_clone)
                .await
            {
                runtime_log_error(format!(
                    "[远程IM] 钉钉渠道收敛失败: channel_id={}, platform={:?}, error={}",
                    channel_clone.id, channel_clone.platform, err
                ));
            }
        });
    } else if channel.platform == RemoteImPlatform::WeixinOc {
        weixin_oc_manager()
            .reconcile_channel_runtime(&effective_channel, state.clone())
            .await?;
    }

    if channel.platform == RemoteImPlatform::Dingtalk {
        Ok(dingtalk_stream_manager()
            .get_channel_status(&channel_id)
            .await)
    } else if channel.platform == RemoteImPlatform::WeixinOc {
        Ok(weixin_oc_manager().build_status(&channel_id).await)
    } else {
        Ok(manager.get_connection_status(&channel_id).await)
    }
}

async fn ide_chat_remote_im_restart_channel_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let channel_id = match params {
        Value::Object(mut map) => map
            .remove("channelId")
            .or_else(|| map.remove("channel_id"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default(),
        _ => String::new(),
    };
    ide_chat_serialize(remote_im_restart_channel_inner(channel_id, state).await?)
}

async fn ide_chat_remote_im_get_channel_logs_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let channel_id = match params {
        Value::Object(mut map) => map
            .remove("channelId")
            .or_else(|| map.remove("channel_id"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default(),
        _ => String::new(),
    };
    ide_chat_serialize(get_remote_im_channel_logs(state, channel_id).await?)
}

async fn ide_chat_remote_im_get_contact_logs_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactLogsInput>(params, "input")?;
    ide_chat_serialize(remote_im_get_contact_logs_inner(state, input).await?)
}

async fn remote_im_get_contact_logs_inner(
    state: &AppState,
    input: RemoteImContactLogsInput,
) -> Result<Vec<ChannelLogEntry>, String> {
    let (channel_id, contact_marker) =
        remote_im_resolve_contact_log_query(state, &input.contact_id)?;
    let logs = get_remote_im_channel_logs(state, channel_id).await?;
    Ok(remote_im_filter_channel_logs_for_contact(logs, &contact_marker))
}

async fn get_remote_im_channel_logs(
    state: &AppState,
    channel_id: String,
) -> Result<Vec<ChannelLogEntry>, String> {
    let config = state_read_config_cached(state)?;
    let channel = remote_im_channel_by_id(&config, &channel_id)
        .ok_or_else(|| format!("未找到远程 IM 渠道: {}", channel_id))?;
    match channel.platform {
        RemoteImPlatform::Dingtalk => Ok(dingtalk_stream_manager().get_logs(&channel_id).await),
        RemoteImPlatform::WeixinOc => Ok(weixin_oc_manager().get_logs(&channel_id).await),
        _ => get_channel_logs(channel_id).await,
    }
}

fn ide_chat_remote_im_list_channels_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(remote_im_list_channels_inner(state)?)
}

fn ide_chat_remote_im_list_contacts_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(remote_im_list_contacts_inner(state)?)
}

fn ide_chat_remote_im_update_contact_allow_send_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactAllowSendUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_allow_send_inner(state, input)?)
}

fn ide_chat_remote_im_update_contact_allow_send_files_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactAllowSendFilesUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_allow_send_files_inner(state, input)?)
}

fn ide_chat_remote_im_update_contact_blocked_message_prefixes_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactBlockedMessagePrefixesUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_blocked_message_prefixes_inner(state, input)?)
}

fn ide_chat_remote_im_update_contact_activation_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactActivationUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_activation_inner(state, input)?)
}

fn ide_chat_remote_im_update_contact_agent_binding_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactAgentBindingUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_agent_binding_inner(state, input)?)
}

fn ide_chat_remote_im_update_contact_processing_mode_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactProcessingModeUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_processing_mode_inner(state, input)?)
}

fn ide_chat_remote_im_update_contact_workspace_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactWorkspaceUpdateInput>(params, "input")?;
    ide_chat_serialize(remote_im_update_contact_workspace_inner(state, input)?)
}

fn ide_chat_remote_im_delete_contact_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactDeleteInput>(params, "input")?;
    ide_chat_serialize(remote_im_delete_contact_inner(state, input)?)
}

async fn ide_chat_remote_im_weixin_oc_start_login_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<WeixinOcLoginStartInput>(params, "input")?;
    ide_chat_serialize(weixin_oc_manager().start_login(state, input).await?)
}

async fn ide_chat_remote_im_weixin_oc_get_login_status_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<WeixinOcLoginStatusInput>(params, "input")?;
    ide_chat_serialize(weixin_oc_manager().poll_login_status(state, input).await?)
}

async fn ide_chat_remote_im_weixin_oc_logout_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<WeixinOcLoginStatusInput>(params, "input")?;
    weixin_oc_manager()
        .logout(state, input.channel_id.as_str())
        .await?;
    ide_chat_serialize(true)
}

async fn ide_chat_remote_im_weixin_oc_sync_contacts_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<WeixinOcLoginStatusInput>(params, "input")?;
    ide_chat_serialize(remote_im_weixin_oc_sync_contacts_inner(state, input)?)
}
