#[derive(Debug, Clone)]
struct BuiltinContactSendFilesTool {
    app_state: AppState,
    session_id: String,
}

const REMOTE_IM_URL_ATTACHMENT_MAX_BYTES: u64 = 100 * 1024 * 1024;

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

impl RuntimeValueTool for BuiltinContactSendFilesTool {
    const NAME: &'static str = "contact_send_files";
    type Args = ContactSendFilesToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=contact_send_files args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_contact_send_files(&self.app_state, &self.session_id, args)
                .await
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=contact_send_files result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行失败 name=contact_send_files err={err}"
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

fn remote_im_is_http_url(raw: &str) -> bool {
    reqwest::Url::parse(raw.trim())
        .ok()
        .map(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or(false)
}

fn remote_im_content_disposition_file_name(raw: &str) -> Option<String> {
    raw.split(';').find_map(|part| {
        let trimmed = part.trim();
        let value = trimmed
            .strip_prefix("filename*=")
            .or_else(|| trimmed.strip_prefix("filename="))?
            .trim()
            .trim_matches('"');
        let value = value
            .strip_prefix("UTF-8''")
            .or_else(|| value.strip_prefix("utf-8''"))
            .unwrap_or(value);
        urlencoding::decode(value)
            .ok()
            .map(|decoded| sanitize_download_file_name(&decoded))
            .filter(|name| !name.trim().is_empty())
    })
}

fn remote_im_file_name_from_url(url: &reqwest::Url, content_disposition: Option<&str>) -> String {
    if let Some(name) = content_disposition.and_then(remote_im_content_disposition_file_name) {
        return name;
    }
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| urlencoding::decode(value).ok().map(|decoded| decoded.to_string()))
        .map(|value| sanitize_download_file_name(&value))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "attachment.bin".to_string())
}

fn remote_im_mime_from_name_or_bytes(file_name: &str, raw: &[u8], header_mime: Option<&str>) -> String {
    if let Some(mime) = image_mime_from_bytes(raw) {
        return mime.to_string();
    }
    header_mime
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| media_mime_from_path(std::path::Path::new(file_name)).map(str::to_string))
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

fn remote_im_image_content_item_from_bytes(
    file_name: &str,
    mime: &str,
    raw: &[u8],
) -> Result<Value, String> {
    let normalized = normalize_image_bytes_for_llm_request(raw, Some(mime))
        .map_err(|err| format!("规范化网络图片失败: file_name={file_name}, err={err}"))?;
    let send_name = remote_im_local_image_send_name(file_name, &normalized.mime);
    Ok(serde_json::json!({
        "type": "image",
        "mime": normalized.mime,
        "name": send_name,
        "bytesBase64": B64.encode(&normalized.bytes)
    }))
}

async fn remote_im_download_url_content_item(state: &AppState, raw: &str) -> Result<Value, String> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|err| format!("附件 URL 无效: {err}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("附件 URL 协议不支持: {}", url.scheme()));
    }
    let response = state
        .shared_http_client
        .get(url.clone())
        .send()
        .await
        .map_err(|err| format!("下载附件失败: url={url}, err={err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("下载附件失败: url={url}, status={status}"));
    }
    if let Some(length) = response.content_length() {
        if length > REMOTE_IM_URL_ATTACHMENT_MAX_BYTES {
            return Err(format!(
                "网络附件过大: url={}, bytes={}, max_bytes={}",
                url, length, REMOTE_IM_URL_ATTACHMENT_MAX_BYTES
            ));
        }
    }
    let headers = response.headers().clone();
    let header_mime = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let content_disposition = headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("读取网络附件失败: url={url}, err={err}"))?;
    if bytes.len() as u64 > REMOTE_IM_URL_ATTACHMENT_MAX_BYTES {
        return Err(format!(
            "网络附件过大: url={}, bytes={}, max_bytes={}",
            url,
            bytes.len(),
            REMOTE_IM_URL_ATTACHMENT_MAX_BYTES
        ));
    }
    let file_name = remote_im_file_name_from_url(&url, content_disposition.as_deref());
    let mime = remote_im_mime_from_name_or_bytes(&file_name, &bytes, header_mime.as_deref());
    if mime.starts_with("image/") {
        return remote_im_image_content_item_from_bytes(&file_name, &mime, &bytes);
    }
    Ok(serde_json::json!({
        "type": "file",
        "name": file_name,
        "mime": mime,
        "bytesBase64": B64.encode(&bytes)
    }))
}

async fn remote_im_build_file_content_items(
    state: &AppState,
    file_paths: &[String],
) -> Result<Vec<Value>, String> {
    let mut out = Vec::<Value>::new();
    for raw in file_paths {
        if remote_im_is_http_url(raw) {
            out.push(remote_im_download_url_content_item(state, raw).await?);
            continue;
        }
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
            let render = local_image_read_for_display(&path, LOCAL_IMAGE_REMOTE_MAX_EDGE)?;
            let send_name = remote_im_local_image_send_name(&file_name, &render.mime);
            out.push(serde_json::json!({
                "type": "image",
                "mime": render.mime,
                "name": send_name,
                "bytesBase64": B64.encode(&render.bytes)
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

fn remote_im_content_contains_file(content: &[Value]) -> bool {
    content.iter().any(|item| {
        item.get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| value == "file")
    })
}

fn remote_im_local_image_send_name(file_name: &str, mime: &str) -> String {
    let trimmed = file_name.trim();
    if mime.trim().eq_ignore_ascii_case("image/webp")
        && !trimmed.to_ascii_lowercase().ends_with(".webp")
    {
        let stem = std::path::Path::new(trimmed)
            .file_stem()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("image");
        return format!("{stem}.webp");
    }
    if trimmed.is_empty() {
        "image".to_string()
    } else {
        trimmed.to_string()
    }
}

fn remote_im_is_generated_image_source_text(text: &str) -> bool {
    let trimmed = text.trim();
    let file_name = ["来源：", "来源:"]
        .iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
        .map(str::trim)
        .unwrap_or_default();
    file_name.len() >= 3 && file_name.starts_with('`') && file_name.ends_with('`')
}

async fn inline_segments_to_remote_im_content_items(
    segments: &[PersistedInlineMessageSegment],
) -> Result<Vec<Value>, String> {
    let mut out = Vec::<Value>::new();
    let mut previous_was_image = false;
    for segment in segments {
        match segment {
            PersistedInlineMessageSegment::Text { text } => {
                if !text.is_empty()
                    && !(previous_was_image && remote_im_is_generated_image_source_text(text))
                {
                    out.push(serde_json::json!({ "type": "text", "text": text }));
                }
                previous_was_image = false;
            }
            PersistedInlineMessageSegment::Meme {
                name,
                mime,
                relative_path: _,
                bytes_base64,
            } => {
                let file_name = name.clone();
                out.push(serde_json::json!({
                    "type": "image",
                    "mime": mime,
                    "name": file_name,
                    "bytesBase64": bytes_base64,
                }));
                previous_was_image = true;
            }
            PersistedInlineMessageSegment::LocalImage {
                path,
                file_name,
                mime: _,
                alt: _,
                width: _,
                height: _,
            } => {
                let resolved = PathBuf::from(path);
                let render = local_image_read_for_display(&resolved, LOCAL_IMAGE_REMOTE_MAX_EDGE)?;
                let send_name = remote_im_local_image_send_name(file_name, &render.mime);
                out.push(serde_json::json!({
                    "type": "image",
                    "mime": render.mime,
                    "name": send_name,
                    "bytesBase64": B64.encode(&render.bytes),
                }));
                previous_was_image = true;
            }
        }
    }
    Ok(out)
}

async fn remote_im_build_text_content_items(
    state: &AppState,
    agent_id: &str,
    text: &str,
) -> Result<Vec<Value>, String> {
    if let Some(segments) = resolve_text_to_persisted_inline_segments(state, agent_id, text)? {
        let items = inline_segments_to_remote_im_content_items(&segments).await?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteImSendContentErrorStage {
    Preflight,
    DeliveryAttempted,
}

#[derive(Debug, Clone)]
struct RemoteImSendContentError {
    stage: RemoteImSendContentErrorStage,
    message: String,
}

async fn remote_im_send_content_payload_with_stage(
    state: &AppState,
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    content: Vec<Value>,
    stop_tool_loop: bool,
    action: &str,
) -> Result<Value, RemoteImSendContentError> {
    if content.is_empty() {
        return Err(RemoteImSendContentError {
            stage: RemoteImSendContentErrorStage::Preflight,
            message: "发送内容不能为空".to_string(),
        });
    }
    let muted = remote_im_contact_is_muted(state, &contact.id).map_err(|message| {
        RemoteImSendContentError {
            stage: RemoteImSendContentErrorStage::Preflight,
            message,
        }
    })?;
    if muted {
        remote_im_append_contact_log_async(
            contact,
            "info",
            format!(
                "[联系人消息] 发出跳过: contact={}, action={}, reason=muted",
                remote_im_contact_log_label(contact),
                action.trim()
            ),
        )
        .await;
        return Err(RemoteImSendContentError {
            stage: RemoteImSendContentErrorStage::Preflight,
            message: format!(
                "联系人“{}”处于闭嘴状态，已拦截外发",
                remote_im_contact_log_label(contact)
            ),
        });
    }
    let content_digest = remote_im_outbound_content_digest(&content);
    let payload = serde_json::json!({
        "channel_id": contact.channel_id,
        "contact_record_id": contact.id,
        "platform": channel.platform,
        "contact_type": contact.remote_contact_type,
        "contact_id": contact.remote_contact_id,
        "content": content,
    });
    let send_channel = remote_im_channel_with_effective_credentials(state, channel).map_err(
        |message| RemoteImSendContentError {
            stage: RemoteImSendContentErrorStage::Preflight,
            message,
        },
    )?;
    let platform_message_id = match remote_im_send_via_sdk(&send_channel, contact, &payload).await {
        Ok(value) => value,
        Err(err) => {
            remote_im_append_contact_log_async(
                contact,
                "warn",
                format!(
                    "[联系人消息] 发出失败: contact={}, action={}, text_count={}, image_count={}, file_count={}, other_count={}, preview={}, error={}",
                    remote_im_contact_log_label(contact),
                    action.trim(),
                    content_digest.text_count,
                    content_digest.image_count,
                    content_digest.file_count,
                    content_digest.other_count,
                    content_digest.text_preview,
                    err.message
                ),
            )
            .await;
            return Err(RemoteImSendContentError {
                stage: match err.kind {
                    RemoteImSdkSendErrorKind::DefinitelyNotSent => {
                        RemoteImSendContentErrorStage::Preflight
                    }
                    RemoteImSdkSendErrorKind::Uncertain => {
                        RemoteImSendContentErrorStage::DeliveryAttempted
                    }
                },
                message: err.message,
            });
        }
    };
    remote_im_append_contact_log_async(
        contact,
        "info",
        format!(
            "[联系人消息] 发出: contact={}, action={}, text_count={}, image_count={}, file_count={}, other_count={}, preview={}",
            remote_im_contact_log_label(contact),
            action.trim(),
            content_digest.text_count,
            content_digest.image_count,
            content_digest.file_count,
            content_digest.other_count,
            content_digest.text_preview
        ),
    )
    .await;
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


async fn remote_im_send_content_payload(
    state: &AppState,
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    content: Vec<Value>,
    stop_tool_loop: bool,
    action: &str,
) -> Result<Value, String> {
    remote_im_send_content_payload_with_stage(
        state,
        channel,
        contact,
        content,
        stop_tool_loop,
        action,
    )
    .await
    .map_err(|err| err.message)
}

fn contact_tool_target_conversation_id(session_id: &str) -> Result<String, String> {
    delegate_session_conversation_id(session_id)
        .ok_or_else(|| "联系人专用工具缺少 conversation_id，无法定位当前联系人".to_string())
}

fn remote_im_bound_contact_context_from_runtime(
    state: &AppState,
    session_id: &str,
) -> Result<(RemoteImChannelConfig, RemoteImContact), String> {
    let conversation_id = contact_tool_target_conversation_id(session_id)?;
    let activation_sources = get_conversation_remote_im_activation_sources(state, &conversation_id)?;
    let bound_source = if let Some(source) = resolve_bound_remote_im_activation_source(&activation_sources) {
        source
    } else {
        let source = remote_im_auto_send_source_for_contact_conversation(state, &conversation_id)?
            .ok_or_else(|| "当前轮次未绑定联系人，无法调用联系人专用工具".to_string())?;
        runtime_log_debug(format!(
            "[联系人文件发送] 使用会话绑定联系人，conversation_id={}, contact_id={}",
            conversation_id, source.remote_contact_id
        ));
        source
    };
    let config = state_read_config_cached(state)?;
    let contact = state_service_find_remote_im_contact_by_identity(
        state,
        &bound_source.channel_id,
        &bound_source.remote_contact_type,
        &bound_source.remote_contact_id,
    )?
    .ok_or_else(|| {
        format!(
            "未找到当前轮次绑定的联系人: channel_id={}, contact_type={}, contact_id={}",
            bound_source.channel_id,
            bound_source.remote_contact_type,
            bound_source.remote_contact_id
        )
    })?;
    let channel = remote_im_channel_by_id(&config, &contact.channel_id)
        .cloned()
        .ok_or_else(|| format!("远程 IM 渠道不存在: {}", contact.channel_id))?;
    if !channel.enabled {
        return Err(format!("远程 IM 渠道未启用: {}", contact.channel_id));
    }
    Ok((channel, contact))
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
    let (channel, contact) = {
        let state = state.clone();
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || {
            remote_im_bound_contact_context_from_runtime(&state, &session_id)
        })
        .await
        .map_err(|err| format!("读取联系人上下文失败：error={err}"))?
    }?;
    if !contact.allow_send {
        return Err("当前联系人不允许发送消息".to_string());
    }
    let content = remote_im_build_file_content_items(state, &file_paths).await?;
    if !contact.allow_send_files && remote_im_content_contains_file(&content) {
        return Err("当前联系人已禁止接收非图片文件".to_string());
    }
    let mut result =
        remote_im_send_content_payload(state, &channel, &contact, content, false, "send_files").await?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("file_count".to_string(), serde_json::json!(file_paths.len()));
    }
    Ok(result)
}

#[cfg(test)]
mod remote_im_local_image_tests {
    use super::*;

    #[test]
    fn remote_im_local_image_send_name_should_match_webp_mime() {
        assert_eq!(
            remote_im_local_image_send_name("result.png", "image/webp"),
            "result.webp"
        );
        assert_eq!(
            remote_im_local_image_send_name("result.webp", "image/webp"),
            "result.webp"
        );
        assert_eq!(
            remote_im_local_image_send_name("result.png", "image/png"),
            "result.png"
        );
    }

    #[test]
    fn remote_im_file_name_from_url_should_decode_path_or_content_disposition() {
        let url = reqwest::Url::parse("https://example.com/files/http%20502.png?token=1")
            .expect("valid url");
        assert_eq!(
            remote_im_file_name_from_url(&url, None),
            "http 502.png"
        );
        assert_eq!(
            remote_im_file_name_from_url(
                &url,
                Some("attachment; filename*=UTF-8''cat%20502.jpg")
            ),
            "cat 502.jpg"
        );
    }

    #[test]
    fn remote_im_image_content_item_from_bytes_should_emit_image_payload() {
        let mut png = Vec::<u8>::new();
        {
            let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([1, 2, 3, 255]),
            ));
            let mut cursor = std::io::Cursor::new(&mut png);
            image
                .write_to(&mut cursor, image::ImageFormat::Png)
                .expect("write png");
        }

        let item = remote_im_image_content_item_from_bytes("cat.png", "image/png", &png)
            .expect("image item");

        assert_eq!(item.get("type").and_then(Value::as_str), Some("image"));
        assert_eq!(item.get("mime").and_then(Value::as_str), Some("image/webp"));
        assert_eq!(item.get("name").and_then(Value::as_str), Some("cat.webp"));
        assert!(item
            .get("bytesBase64")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty()));
    }

    #[test]
    fn remote_im_generated_image_source_text_should_only_match_source_line() {
        assert!(remote_im_is_generated_image_source_text(
            "\n来源：`我觉得这就是一种自信.png`\n"
        ));
        assert!(remote_im_is_generated_image_source_text("来源:`image.webp`"));
        assert!(!remote_im_is_generated_image_source_text("来源：图片来自群友"));
        assert!(!remote_im_is_generated_image_source_text(
            "这里是图片的来源：`image.webp`"
        ));
    }

    #[test]
    fn remote_im_inline_image_should_omit_generated_source_text() {
        let segments = vec![
            PersistedInlineMessageSegment::Meme {
                name: "image.webp".to_string(),
                mime: "image/webp".to_string(),
                relative_path: "memes/image.webp".to_string(),
                bytes_base64: "aGVsbG8=".to_string(),
            },
            PersistedInlineMessageSegment::Text {
                text: "\n来源：`image.webp`\n".to_string(),
            },
        ];

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create test runtime");
        let items = runtime
            .block_on(inline_segments_to_remote_im_content_items(&segments))
            .expect("build remote image payload");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].get("type").and_then(Value::as_str), Some("image"));
    }
}
