const DINGTALK_STREAM_TOPIC: &str = "/v1.0/im/bot/messages/get";
const DINGTALK_DOWNLOAD_API: &str = "https://api.dingtalk.com/v1.0/robot/messageFiles/download";
const DINGTALK_RECONNECT_INTERVAL_SECS: u64 = 30;
const DINGTALK_MAX_DOWNLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
struct DingtalkRuntimeState {
    connected: bool,
    connected_at: Option<chrono::DateTime<chrono::Utc>>,
    endpoint: Option<String>,
    last_error: Option<String>,
}

impl Default for DingtalkRuntimeState {
    fn default() -> Self {
        Self {
            connected: false,
            connected_at: None,
            endpoint: None,
            last_error: None,
        }
    }
}

pub struct DingtalkStreamManager {
    states: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, DingtalkRuntimeState>>>,
    stop_senders: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
    >,
    tasks: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
    >,
}

impl DingtalkStreamManager {
    pub fn new() -> Self {
        Self {
            states: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            stop_senders: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    async fn set_state(
        &self,
        channel_id: &str,
        connected: bool,
        endpoint: Option<String>,
        last_error: Option<String>,
    ) {
        let mut states = self.states.write().await;
        let state = states
            .entry(channel_id.to_string())
            .or_insert_with(DingtalkRuntimeState::default);
        let was_connected = state.connected;
        state.connected = connected;
        state.endpoint = endpoint;
        state.last_error = last_error;
        state.connected_at = if connected {
            if was_connected {
                state.connected_at
            } else {
                Some(chrono::Utc::now())
            }
        } else {
            None
        };
    }

    async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        let manager = napcat_ws_manager();
        manager.add_log(channel_id, level, message).await;
    }

    pub(crate) async fn stop_channel(&self, channel_id: &str) {
        if let Some(tx) = self.stop_senders.write().await.remove(channel_id) {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.tasks.write().await.remove(channel_id) {
            if let Err(err) = handle.await {
                self.add_log(
                    channel_id,
                    "warn",
                    &format!("[钉钉生命周期] task={} status=失败 trigger=stop_channel error={err}", channel_id),
                )
                .await;
            }
        }
        self.set_state(channel_id, false, None, None).await;
    }

    pub(crate) async fn reconcile_channel_runtime(&self, channel: &RemoteImChannelConfig, state: AppState) -> Result<(), String> {
        self.stop_channel(&channel.id).await;
        if channel.enabled && channel.platform == RemoteImPlatform::Dingtalk {
            self.start_channel(channel.clone(), state).await?;
        }
        Ok(())
    }

    pub(crate) async fn start_channel(&self, channel: RemoteImChannelConfig, state: AppState) -> Result<(), String> {
        let channel_id = channel.id.clone();
        self.stop_channel(&channel_id).await;
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        self.stop_senders
            .write()
            .await
            .insert(channel_id.clone(), stop_tx);

        let task_channel_id = channel_id.clone();
        let manager = dingtalk_stream_manager();
        let handle = tauri::async_runtime::spawn(async move {
            manager
                .add_log(
                    &task_channel_id,
                    "info",
                    &format!(
                        "[钉钉生命周期] task={} status=开始 trigger=start_channel key_counts=0 duration_ms=0",
                        task_channel_id
                    ),
                )
                .await;
            let mut stop_rx = stop_rx;
            loop {
                if *stop_rx.borrow() {
                    break;
                }
                let result = tokio::select! {
                    changed = stop_rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if *stop_rx.borrow() {
                                    break;
                                }
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    ret = run_single_dingtalk_stream_session(&channel, &state) => ret,
                };
                match result {
                    Ok(()) => break,
                    Err(err) => {
                        manager
                            .set_state(&task_channel_id, false, None, Some(err.clone()))
                            .await;
                        manager
                            .add_log(
                                &task_channel_id,
                                "warn",
                                &format!(
                                    "[钉钉生命周期] task={} status=失败 trigger=run_session backoff_secs={} error={}",
                                    task_channel_id,
                                    DINGTALK_RECONNECT_INTERVAL_SECS,
                                    err
                                ),
                            )
                            .await;
                    }
                }
                tokio::select! {
                    changed = stop_rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if *stop_rx.borrow() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(DINGTALK_RECONNECT_INTERVAL_SECS)) => {}
                }
            }
            manager
                .set_state(&task_channel_id, false, None, None)
                .await;
            manager
                .add_log(
                    &task_channel_id,
                    "info",
                    &format!(
                        "[钉钉生命周期] task={} status=完成 trigger=stop_channel key_counts=0 duration_ms=0",
                        task_channel_id
                    ),
                )
                .await;
        });
        self.tasks.write().await.insert(channel_id, handle);
        Ok(())
    }

    pub(crate) async fn get_channel_status(&self, channel_id: &str) -> ChannelConnectionStatus {
        let state = self
            .states
            .read()
            .await
            .get(channel_id)
            .cloned()
            .unwrap_or_default();
        ChannelConnectionStatus {
            channel_id: channel_id.to_string(),
            connected: state.connected,
            peer_addr: state.endpoint.clone(),
            connected_at: state.connected_at,
            listen_addr: String::new(),
        }
    }
}

impl Default for DingtalkStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

static DINGTALK_STREAM_MANAGER: once_cell::sync::Lazy<std::sync::Arc<DingtalkStreamManager>> =
    once_cell::sync::Lazy::new(|| std::sync::Arc::new(DingtalkStreamManager::new()));

pub fn dingtalk_stream_manager() -> std::sync::Arc<DingtalkStreamManager> {
    DINGTALK_STREAM_MANAGER.clone()
}

fn credential_text(credentials: &Value, key: &str) -> String {
    credentials
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn dingtalk_robot_code(channel: &RemoteImChannelConfig, payload: &Value) -> String {
    let from_payload = payload
        .get("robotCode")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if !from_payload.is_empty() {
        return from_payload;
    }
    let from_config = credential_text(&channel.credentials, "robotCode");
    if !from_config.is_empty() {
        return from_config;
    }
    credential_text(&channel.credentials, "clientId")
}

fn dingtalk_access_token_request_body(channel: &RemoteImChannelConfig) -> Result<Value, String> {
    let client_id = credential_text(&channel.credentials, "clientId");
    let client_secret = credential_text(&channel.credentials, "clientSecret");
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(format!(
            "dingtalk channel '{}' missing clientId/clientSecret",
            channel.id
        ));
    }
    Ok(serde_json::json!({
        "appKey": client_id,
        "appSecret": client_secret
    }))
}

async fn dingtalk_access_token(channel: &RemoteImChannelConfig) -> Result<String, String> {
    let body = dingtalk_access_token_request_body(channel)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("build dingtalk token client failed: {err}"))?;
    let response = client
        .post("https://api.dingtalk.com/v1.0/oauth2/accessToken")
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("dingtalk token request failed: {err}"))?;
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("parse dingtalk token response failed: {err}"))?;
    let token = body
        .get("accessToken")
        .and_then(Value::as_str)
        .or_else(|| body.get("data").and_then(|v| v.get("accessToken")).and_then(Value::as_str))
        .map(str::trim)
        .unwrap_or("");
    if token.is_empty() {
        return Err(format!("dingtalk token missing: {body}"));
    }
    Ok(token.to_string())
}

#[derive(Clone)]
struct EasyCallDingtalkAsyncHandler {
    channel: RemoteImChannelConfig,
    state: AppState,
    manager: std::sync::Arc<DingtalkStreamManager>,
}

impl dingtalk_stream::AsyncChatbotHandler for EasyCallDingtalkAsyncHandler {
    fn pre_start(&self) {
        let manager = self.manager.clone();
        let channel_id = self.channel.id.clone();
        tauri::async_runtime::spawn(async move {
            manager
                .add_log(
                    &channel_id,
                    "info",
                    &format!(
                        "[钉钉生命周期] task={} status=完成 trigger=callback_handler_ready key_counts=0 duration_ms=0",
                        channel_id
                    ),
                )
                .await;
        });
    }

    fn process(&self, callback_message: &dingtalk_stream::MessageBody) {
        let manager = self.manager.clone();
        let channel = self.channel.clone();
        let state = self.state.clone();
        let topic = callback_message
            .headers
            .topic
            .clone()
            .unwrap_or_else(|| DINGTALK_STREAM_TOPIC.to_string());
        let message_id = callback_message
            .headers
            .message_id
            .clone()
            .unwrap_or_default();
        let raw = callback_message.data.clone();

        tauri::async_runtime::spawn(async move {
            manager
                .add_log(
                    &channel.id,
                    "debug",
                    &format!("收到钉钉 CALLBACK: topic={topic}, messageId={message_id}"),
                )
                .await;
            let payload = match serde_json::from_str::<Value>(&raw) {
                Ok(value) => value,
                Err(err) => {
                    let fallback_topic = if topic.trim().is_empty() { "-" } else { topic.trim() };
                    let fallback_message_id = if message_id.trim().is_empty() { "-" } else { message_id.trim() };
                    manager
                        .add_log(
                            &channel.id,
                            "warn",
                            &format!(
                                "钉钉 CALLBACK data 解析失败: err={}, messageId={}, topic={}, payload_len={}",
                                err,
                                fallback_message_id,
                                fallback_topic,
                                raw.len()
                            ),
                        )
                        .await;
                    return;
                }
            };
            let msg_type = string_field(&payload, "msgtype");
            manager
                .add_log(
                    &channel.id,
                    "debug",
                    &format!("钉钉 CALLBACK 解析完成: msgtype={msg_type}, messageId={message_id}"),
                )
                .await;
            match parse_and_enqueue_dingtalk_callback(&channel, &payload, &state).await {
                Ok(result) => {
                    manager
                        .add_log(
                            &channel.id,
                            "debug",
                            &format!("钉钉消息入队成功: event_id={}", result.event_id),
                        )
                        .await;
                }
                Err(err) if err.contains("联系人未开启收信") => {
                    manager
                        .add_log(
                            &channel.id,
                            "warn",
                            &format!("钉钉消息被联系人策略拦截: {err}"),
                        )
                        .await;
                }
                Err(err) if err.contains("跳过") => {
                    manager
                        .add_log(&channel.id, "debug", &format!("钉钉消息跳过: {err}"))
                        .await;
                }
                Err(err) => {
                    manager
                        .add_log(&channel.id, "warn", &format!("钉钉消息入队失败: {err}"))
                        .await;
                }
            }
        });
    }
}

fn string_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn int_field(v: &Value, key: &str) -> Option<i64> {
    if let Some(num) = v.get(key).and_then(Value::as_i64) {
        return Some(num);
    }
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| raw.parse::<i64>().ok())
}

async fn dingtalk_download_file_by_code(
    channel: &RemoteImChannelConfig,
    download_code: &str,
    robot_code: &str,
) -> Result<(Vec<u8>, String), String> {
    let access_token = dingtalk_access_token(channel).await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("build dingtalk download client failed: {err}"))?;
    let response = client
        .post(DINGTALK_DOWNLOAD_API)
        .header("x-acs-dingtalk-access-token", access_token)
        .json(&serde_json::json!({
            "downloadCode": download_code,
            "robotCode": robot_code
        }))
        .send()
        .await
        .map_err(|err| format!("dingtalk download-url request failed: {err}"))?;
    let status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("parse dingtalk download-url response failed: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "dingtalk download-url rejected http {}: {}",
            status.as_u16(),
            body
        ));
    }
    let download_url = body
        .get("downloadUrl")
        .and_then(Value::as_str)
        .or_else(|| body.get("data").and_then(|v| v.get("downloadUrl")).and_then(Value::as_str))
        .map(str::trim)
        .unwrap_or("");
    if download_url.is_empty() {
        return Err(format!("dingtalk download-url missing: {body}"));
    }
    let file_resp = client
        .get(download_url)
        .send()
        .await
        .map_err(|err| format!("dingtalk download file failed: {err}"))?;
    let file_status = file_resp.status();
    if !file_status.is_success() {
        return Err(format!(
            "dingtalk download file rejected http {}",
            file_status.as_u16()
        ));
    }
    if let Some(content_len) = file_resp.content_length() {
        if content_len > DINGTALK_MAX_DOWNLOAD_SIZE_BYTES {
            return Err(format!(
                "dingtalk download file too large: {} bytes > {} bytes",
                content_len,
                DINGTALK_MAX_DOWNLOAD_SIZE_BYTES
            ));
        }
    }
    let mime = file_resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .unwrap_or("application/octet-stream")
        .to_string();
    let mut stream = file_resp.bytes_stream();
    let mut total = 0u64;
    let mut bytes = Vec::<u8>::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| format!("read dingtalk downloaded file failed: {err}"))?;
        total = total.saturating_add(chunk.len() as u64);
        if total > DINGTALK_MAX_DOWNLOAD_SIZE_BYTES {
            return Err(format!(
                "dingtalk download file too large while streaming: {} bytes > {} bytes",
                total,
                DINGTALK_MAX_DOWNLOAD_SIZE_BYTES
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((bytes, mime))
}

fn mime_from_name_fallback(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png".to_string()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.ends_with(".gif") {
        "image/gif".to_string()
    } else if lower.ends_with(".webp") {
        "image/webp".to_string()
    } else if lower.ends_with(".mp3") {
        "audio/mpeg".to_string()
    } else if lower.ends_with(".wav") {
        "audio/wav".to_string()
    } else if lower.ends_with(".ogg") {
        "audio/ogg".to_string()
    } else if lower.ends_with(".amr") {
        "audio/amr".to_string()
    } else if lower.ends_with(".mp4") {
        "video/mp4".to_string()
    } else if lower.ends_with(".pdf") {
        "application/pdf".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

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
            "dingtalk channel '{}' missing clientId/clientSecret",
            channel.id
        ));
    }
    let manager = dingtalk_stream_manager();
    manager
        .add_log(
            &channel.id,
            "info",
            &format!(
                "[钉钉生命周期] task={} status=开始 trigger=run_single_dingtalk_stream_session key_counts=0 duration_ms=0",
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
                "[钉钉生命周期] task={} status=完成 trigger=run_single_dingtalk_stream_session key_counts=0 duration_ms={}",
                channel.id,
                started_at.elapsed().as_millis()
            ),
        )
        .await;
    Ok(())
}
