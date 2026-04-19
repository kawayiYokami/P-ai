#[derive(Debug, Clone)]
struct BuiltinContactReplyTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinContactSendFilesTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinContactNoReplyTool;

impl RuntimeToolMetadata for BuiltinContactReplyTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "contact_reply",
            CONTACT_REPLY_TOOL_DESCRIPTION,
            serde_json::json!({
              "type": "object",
              "properties": {
                "text": { "type": "string", "description": CONTACT_REPLY_TOOL_TEXT_DESCRIPTION }
              },
              "required": ["text"]
            }),
        )
    }
}

impl RuntimeToolMetadata for BuiltinContactSendFilesTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "contact_send_files",
            CONTACT_SEND_FILES_TOOL_DESCRIPTION,
            serde_json::json!({
              "type": "object",
              "properties": {
                "file_paths": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": CONTACT_SEND_FILES_TOOL_FILE_PATHS_DESCRIPTION
                }
              },
              "required": ["file_paths"]
            }),
        )
    }
}

impl RuntimeToolMetadata for BuiltinContactNoReplyTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "contact_no_reply",
            CONTACT_NO_REPLY_TOOL_DESCRIPTION,
            serde_json::json!({
              "type": "object",
              "properties": {
                "reason": { "type": "string", "description": CONTACT_NO_REPLY_TOOL_REASON_DESCRIPTION }
              }
            }),
        )
    }
}

impl RuntimeJsonTool for BuiltinContactReplyTool {
    const NAME: &'static str = "contact_reply";
    type Args = ContactReplyToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeJsonValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[TOOL-DEBUG] execute_builtin_tool.start name=contact_reply args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_contact_reply(&self.app_state, &self.session_id, args)
                .await
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=contact_reply result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_debug(format!(
                    "[TOOL-DEBUG] execute_builtin_tool.err name=contact_reply err={err}"
                )),
            }
            result
        })
    }
}

impl RuntimeJsonTool for BuiltinContactSendFilesTool {
    const NAME: &'static str = "contact_send_files";
    type Args = ContactSendFilesToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeJsonValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[TOOL-DEBUG] execute_builtin_tool.start name=contact_send_files args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_contact_send_files(&self.app_state, &self.session_id, args)
                .await
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=contact_send_files result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_debug(format!(
                    "[TOOL-DEBUG] execute_builtin_tool.err name=contact_send_files err={err}"
                )),
            }
            result
        })
    }
}

impl RuntimeJsonTool for BuiltinContactNoReplyTool {
    const NAME: &'static str = "contact_no_reply";
    type Args = ContactNoReplyToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeJsonValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[TOOL-DEBUG] execute_builtin_tool.start name=contact_no_reply args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_contact_no_reply(args).map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=contact_no_reply result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_debug(format!(
                    "[TOOL-DEBUG] execute_builtin_tool.err name=contact_no_reply err={err}"
                )),
            }
            result
        })
    }
}

async fn remote_im_resolve_file_path(state: &AppState, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("file_paths 包含空路径".to_string());
    }
    let direct = PathBuf::from(trimmed);
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    let candidate = if direct.is_absolute() {
        direct
    } else {
        workspace_root.join(direct)
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

async fn remote_im_outbound_contains_non_image_file(
    state: &AppState,
    file_paths: &[String],
) -> Result<bool, String> {
    for raw in file_paths {
        let path = remote_im_resolve_file_path(state, raw).await?;
        let mime = media_mime_from_path(path.as_path())
            .unwrap_or("application/octet-stream")
            .to_string();
        if !mime.starts_with("image/") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn remote_im_text_content_items_from_segments(segments: &[PersistedMemeSegment]) -> Vec<Value> {
    meme_segments_to_remote_im_content_items(segments)
}

async fn remote_im_build_text_content_items(
    state: &AppState,
    text: &str,
    seed_source: &str,
) -> Result<Vec<Value>, String> {
    if let Some(segments) = meme_segments_from_remote_im_text(state, text, seed_source)? {
        let items = remote_im_text_content_items_from_segments(&segments);
        if !items.is_empty() {
            return Ok(items);
        }
    }
    if text.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![serde_json::json!({
        "type": "text",
        "text": text,
    })])
}

async fn remote_im_send_content_payload(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    content: Vec<Value>,
    stop_tool_loop: bool,
    action: &str,
) -> Result<Value, String> {
    if content.is_empty() {
        return Err("发送内容不能为空".to_string());
    }
    let payload = serde_json::json!({
        "channel_id": contact.channel_id,
        "contact_record_id": contact.id,
        "platform": channel.platform,
        "contact_type": contact.remote_contact_type,
        "contact_id": contact.remote_contact_id,
        "content": content,
    });
    let platform_message_id = remote_im_send_via_sdk(channel, contact, &payload).await?;
    Ok(serde_json::json!({
        "ok": true,
        "action": action.trim(),
        "done": stop_tool_loop,
        "continue": !stop_tool_loop,
        "stop_tool_loop": stop_tool_loop,
        "channel_id": contact.channel_id,
        "contact_id": contact.remote_contact_id,
        "contact_name": contact.remote_contact_name,
        "contact_type": contact.remote_contact_type,
        "platform_message_id": platform_message_id
    }))
}

fn contact_tool_target_conversation_id(session_id: &str) -> Result<String, String> {
    let (_, _, conversation_id) = delegate_parse_session_parts(session_id);
    conversation_id.ok_or_else(|| "联系人专用工具缺少 conversation_id，无法定位当前联系人".to_string())
}

fn remote_im_current_contact_context_from_session(
    state: &AppState,
    session_id: &str,
) -> Result<(Conversation, RemoteImChannelConfig, RemoteImContact), String> {
    let conversation_id = contact_tool_target_conversation_id(session_id)?;
    let config = state_read_config_cached(state)?;
    let data = state_read_app_data_cached(state)?;
    let conversation = data
        .conversations
        .iter()
        .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("当前会话不存在: conversation_id={conversation_id}"))?;
    if !conversation_is_remote_im_contact(&conversation) {
        return Err("联系人专用工具仅可用于联系人会话".to_string());
    }
    let contact = remote_im_find_contact_by_conversation(&data, &conversation_id)
        .cloned()
        .ok_or_else(|| format!("未找到当前会话绑定的联系人: conversation_id={conversation_id}"))?;
    let channel = remote_im_channel_by_id(&config, &contact.channel_id)
        .cloned()
        .ok_or_else(|| format!("远程 IM 渠道不存在: {}", contact.channel_id))?;
    if !channel.enabled {
        return Err(format!("远程 IM 渠道未启用: {}", contact.channel_id));
    }
    Ok((conversation, channel, contact))
}

async fn builtin_contact_reply(
    state: &AppState,
    session_id: &str,
    args: ContactReplyToolArgs,
) -> Result<Value, String> {
    let text = args.text.trim().to_string();
    if text.is_empty() {
        return Err("contact_reply.text 不能为空".to_string());
    }
    let (_conversation, channel, contact) =
        remote_im_current_contact_context_from_session(state, session_id)?;
    if !contact.allow_send {
        return Err("当前联系人不允许发送消息".to_string());
    }
    let seed_source = format!(
        "contact_reply::{}::{}::{}",
        contact.channel_id, contact.remote_contact_id, text
    );
    let content = remote_im_build_text_content_items(state, &text, &seed_source).await?;
    remote_im_send_content_payload(&channel, &contact, content, false, "reply").await
}

async fn builtin_contact_send_files(
    state: &AppState,
    session_id: &str,
    args: ContactSendFilesToolArgs,
) -> Result<Value, String> {
    let file_paths = args
        .file_paths
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if file_paths.is_empty() {
        return Err("contact_send_files.file_paths 不能为空".to_string());
    }
    let (_conversation, channel, contact) =
        remote_im_current_contact_context_from_session(state, session_id)?;
    if !contact.allow_send {
        return Err("当前联系人不允许发送消息".to_string());
    }
    if !contact.allow_send_files
        && remote_im_outbound_contains_non_image_file(state, &file_paths).await?
    {
        return Err("当前联系人已禁止接收非图片文件".to_string());
    }
    let content = remote_im_build_file_content_items(state, &file_paths).await?;
    let mut result =
        remote_im_send_content_payload(&channel, &contact, content, false, "send_files").await?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("file_count".to_string(), serde_json::json!(file_paths.len()));
    }
    Ok(result)
}

fn builtin_contact_no_reply(args: ContactNoReplyToolArgs) -> Result<Value, String> {
    let reason = args
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    Ok(serde_json::json!({
        "ok": true,
        "action": "no_reply",
        "no_reply": true,
        "done": true,
        "continue": false,
        "stop_tool_loop": true,
        "reason": reason.unwrap_or_default()
    }))
}
