#[derive(Debug, Clone)]
struct BuiltinRemoteImSendTool {
    app_state: AppState,
}

impl Tool for BuiltinRemoteImSendTool {
    const NAME: &'static str = "remote_im_send";
    type Error = ToolInvokeError;
    type Args = RemoteImSendToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remote_im_send".to_string(),
            description: "远程联系人通讯工具。action=list 可列出当前可用联系人；action=send 可向指定联系人发送消息。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["list", "send"], "description": "动作。list=列出可用联系人；send=发送消息", "default": "send" },
                "channel_id": { "type": "string", "description": "action=send 时必填；action=list 时可选（用于按渠道过滤）" },
                "contact_id": { "type": "string", "description": "action=send 时必填；远程联系人 ID（contactId，即QQ号或群号）" },
                "text": { "type": "string", "description": "action=send 时必填；要发送的文本内容" },
                "status": { "type": "string", "enum": ["continue", "done"], "description": "发送后状态。continue=还需继续下一步；done=本轮已完成并停止后续工具链。大小写不敏感，内部统一转小写。", "default": "done" },
                "file_paths": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "（预留）附件文件路径列表，当前版本暂不支持"
                }
              },
              "required": ["action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=remote_im_send args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_remote_im_send(&self.app_state, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=remote_im_send result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=remote_im_send err={err}"),
        }
        result
    }
}

async fn builtin_remote_im_send(
    state: &AppState,
    args: RemoteImSendToolArgs,
) -> Result<Value, String> {
    let action = args.action.trim().to_ascii_lowercase();
    match action.as_str() {
        "list" => {
            let channel_filter = args
                .channel_id
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            let config = state_read_config_cached(state)?;
            let data = state_read_app_data_cached(state)?;
            let mut contacts = Vec::<Value>::new();
            for contact in &data.remote_im_contacts {
                if let Some(filter) = channel_filter.as_deref() {
                    if contact.channel_id != filter {
                        continue;
                    }
                }
                let Some(channel) = config
                    .remote_im_channels
                    .iter()
                    .find(|item| item.id == contact.channel_id)
                else {
                    continue;
                };
                if !channel.enabled {
                    continue;
                }
                contacts.push(serde_json::json!({
                    "channelId": contact.channel_id,
                    "channelName": channel.name,
                    "platform": contact.platform,
                    "contactId": contact.remote_contact_id,
                    "contactName": contact.remote_contact_name,
                    "remarkName": contact.remark_name,
                    "contactType": contact.remote_contact_type,
                    "allowSend": contact.allow_send,
                    "allowReceive": contact.allow_receive,
                    "activationMode": contact.activation_mode,
                }));
            }
            return Ok(serde_json::json!({
                "ok": true,
                "action": "list",
                "count": contacts.len(),
                "contacts": contacts
            }));
        }
        "send" => {}
        other => {
            return Err(format!(
                "remote_im_send.action 非法：`{other}`。请返回正确动作：list 或 send"
            ));
        }
    }

    let channel_id = args
        .channel_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "action=send 时 channel_id 不能为空".to_string())?
        .to_string();
    let contact_id_input = args
        .contact_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "action=send 时 contact_id 不能为空".to_string())?
        .to_string();
    let text = args
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "action=send 时 text 不能为空".to_string())?
        .to_string();

    let config = state_read_config_cached(state)?;
    let channel = remote_im_channel_by_id(&config, &channel_id)
        .ok_or_else(|| format!("远程IM渠道不存在: {channel_id}"))?
        .clone();
    if !channel.enabled {
        return Err(format!("远程IM渠道未启用: {channel_id}"));
    }

    let data = state_read_app_data_cached(state)?;
    let contact = data
        .remote_im_contacts
        .iter()
        .find(|c| c.channel_id == channel_id && c.remote_contact_id == contact_id_input)
        .ok_or_else(|| format!("未找到匹配的远程联系人: channel_id={channel_id}, contact_id={contact_id_input}"))?
        .clone();

    if !contact.allow_send {
        return Err(format!(
            "用户已禁止向该联系人发送消息: channel_id={}, contact_id={}",
            channel_id, contact_id_input
        ));
    }

    let payload = serde_json::json!({
        "channelId": contact.channel_id,
        "contactId": contact.id,
        "platform": channel.platform,
        "remoteContactType": contact.remote_contact_type,
        "remoteContactId": contact.remote_contact_id,
        "content": [{
            "type": "text",
            "text": text
        }],
    });

    let status = args.status.trim().to_ascii_lowercase();
    let stop_tool_loop = match status.as_str() {
        "done" => true,
        "continue" => false,
        other => {
            return Err(format!(
                "remote_im_send.status 非法：`{other}`。请返回正确状态：continue 或 done"
            ))
        }
    };

    let platform_message_id = remote_im_send_via_sdk(&channel, &contact, &payload).await?;

    Ok(serde_json::json!({
        "ok": true,
        "action": "send",
        "status": status,
        "done": stop_tool_loop,
        "continue": !stop_tool_loop,
        "stopToolLoop": stop_tool_loop,
        "channelId": channel_id,
        "contactId": contact_id_input,
        "contactName": contact.remote_contact_name,
        "contactType": contact.remote_contact_type,
        "platformMessageId": platform_message_id
    }))
}
