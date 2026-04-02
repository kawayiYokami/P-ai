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
            description: "远程联系人回复决策工具。来自联系人的消息，必须且只能通过本工具完成回复决策：回复时使用 action=send；决定不回复时也必须使用 action=no_reply；不要直接输出给联系人的回复正文来代替工具调用。action=list 仅用于获取可用联系人。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["list", "send", "no_reply"], "description": "动作。list=列出可用联系人；send=向联系人发送消息；no_reply=本轮决定不回复。对于联系人消息，最终必须使用 send 或 no_reply 做出决策。", "default": "send" },
                "channel_id": { "type": "string", "description": "action=send 时必填；action=list 时可选（用于按渠道过滤）" },
                "contact_id": { "type": "string", "description": "action=send 时必填；必须使用 action=list 返回的 contact_id。不要使用联系人记录主键（UUID）" },
                "text": { "type": "string", "description": "action=send 时可选；要发送的文本内容（当传入 file_paths 时可为空）" },
                "status": { "type": "string", "enum": ["continue", "done"], "description": "发送后状态。continue=还需继续下一步；done=本轮已完成并停止后续工具链。大小写不敏感，内部统一转小写。", "default": "done" },
                "file_paths": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "可选附件路径列表：图片按图片发送，其他文件按附件发送"
                }
              },
              "required": ["action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        runtime_log_debug(format!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=remote_im_send args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        let result = builtin_remote_im_send(&self.app_state, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=remote_im_send result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_debug(format!("[TOOL-DEBUG] execute_builtin_tool.err name=remote_im_send err={err}")),
        }
        result
    }
}

async fn remote_im_resolve_file_path(state: &AppState, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("file_paths 包含空路径".to_string());
    }
    let direct = PathBuf::from(trimmed);
    let candidate = if direct.is_absolute() {
        direct
    } else {
        state.llm_workspace_path.join(direct)
    };
    let metadata = tokio::fs::metadata(candidate.clone())
        .await
        .map_err(|_| format!("附件路径不存在: {}", candidate.to_string_lossy()))?;
    if !metadata.is_file() {
        return Err(format!("附件路径不是文件: {}", candidate.to_string_lossy()));
    }
    Ok(candidate)
}

async fn remote_im_build_file_content_items(
    state: &AppState,
    file_paths: &[String],
) -> Result<Vec<Value>, String> {
    let mut out = Vec::<Value>::new();
    for raw in file_paths {
        let path = remote_im_resolve_file_path(state, raw).await?;
        let file_name = path
            .file_name()
            .and_then(|v| v.to_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("attachment")
            .to_string();
        let mime = media_mime_from_path(path.as_path())
            .unwrap_or("application/octet-stream")
            .to_string();
        if mime.starts_with("image/") {
            let raw = tokio::fs::read(&path).await.map_err(|err| {
                format!("读取图片失败: path={}, err={err}", path.to_string_lossy())
            })?;
            out.push(serde_json::json!({
                "type": "image",
                "mime": mime,
                "name": file_name,
                "bytesBase64": B64.encode(raw)
            }));
        } else {
            out.push(serde_json::json!({
                "type": "file",
                "name": file_name,
                "path": path.to_string_lossy().replace('\\', "/")
            }));
        }
    }
    Ok(out)
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
                    "channel_id": contact.channel_id,
                    "channel_name": channel.name,
                    "platform": contact.platform,
                    "contact_id": contact.remote_contact_id,
                    "contact_record_id": contact.id,
                    "contact_name": contact.remote_contact_name,
                    "remark_name": contact.remark_name,
                    "contact_type": contact.remote_contact_type,
                    "allow_send": contact.allow_send,
                    "allow_receive": contact.allow_receive,
                    "activation_mode": contact.activation_mode,
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
        "no_reply" => {
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
            return Ok(serde_json::json!({
                "ok": true,
                "action": "no_reply",
                "status": status,
                "done": stop_tool_loop,
                "continue": !stop_tool_loop,
                "stop_tool_loop": stop_tool_loop,
                "no_reply": true
            }));
        }
        other => {
            return Err(format!(
                "remote_im_send.action 非法：`{other}`。请返回正确动作：list、send 或 no_reply"
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
        .unwrap_or("")
        .to_string();
    let file_paths = args
        .file_paths
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();

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
        .ok_or_else(|| format!("未找到匹配的远程联系人: channel_id={channel_id}, contact_id={contact_id_input}。请先 action=list，并使用返回的 contact_id"))?
        .clone();

    if !contact.allow_send {
        return Err(format!(
            "用户已禁止向该联系人发送消息: channel_id={}, contact_id={}",
            channel_id, contact_id_input
        ));
    }
    if !file_paths.is_empty() && !contact.allow_send_files {
        return Err(format!(
            "用户已禁止向该联系人发送文件: channel_id={}, contact_id={}",
            channel_id, contact_id_input
        ));
    }

    let mut content = Vec::<Value>::new();
    if !text.trim().is_empty() {
        content.push(serde_json::json!({
            "type": "text",
            "text": text
        }));
    }
    if !file_paths.is_empty() {
        let mut file_items = remote_im_build_file_content_items(state, &file_paths).await?;
        content.append(&mut file_items);
    }
    if content.is_empty() {
        return Err("action=send 时 text 与 file_paths 不能同时为空".to_string());
    }

    let payload = serde_json::json!({
        "channel_id": contact.channel_id,
        "contact_record_id": contact.id,
        "platform": channel.platform,
        "contact_type": contact.remote_contact_type,
        "contact_id": contact.remote_contact_id,
        "content": content,
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
        "stop_tool_loop": stop_tool_loop,
        "channel_id": channel_id,
        "contact_id": contact_id_input,
        "contact_name": contact.remote_contact_name,
        "contact_type": contact.remote_contact_type,
        "platform_message_id": platform_message_id
    }))
}
