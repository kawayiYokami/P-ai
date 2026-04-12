use std::{future::Future, pin::Pin};

trait RemoteImSdk: Send + Sync {
    #[allow(dead_code)] // 在测试中使用
    fn platform(&self) -> RemoteImPlatform;
    fn validate_channel(&self, channel: &RemoteImChannelConfig) -> Result<(), String>;
    fn send_outbound<'a>(
        &'a self,
        channel: &'a RemoteImChannelConfig,
        contact: &'a RemoteImContact,
        payload: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;
}

fn remote_im_payload_text(payload: &Value) -> String {
    payload
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn remote_im_onebot_message_segments(payload: &Value) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    let Some(items) = payload.get("content").and_then(Value::as_array) else {
        return out;
    };
    for item in items {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "text" => {
                let text = item.get("text").and_then(Value::as_str).unwrap_or("").trim();
                if text.is_empty() {
                    continue;
                }
                out.push(serde_json::json!({
                    "type": "text",
                    "data": {"text": text}
                }));
            }
            "image" => {
                let bytes_base64 = item
                    .get("bytesBase64")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty());
                let file_value = if let Some(b64) = bytes_base64 {
                    format!("base64://{b64}")
                } else {
                    item.get("path")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_string)
                        .unwrap_or_default()
                };
                if file_value.is_empty() {
                    continue;
                }
                out.push(serde_json::json!({
                    "type": "image",
                    "data": {"file": file_value}
                }));
            }
            "file" => {
                let path = item
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .unwrap_or("");
                if path.is_empty() {
                    continue;
                }
                out.push(serde_json::json!({
                    "type": "file",
                    "data": {"file": path}
                }));
            }
            _ => {}
        }
    }
    out
}

fn remote_im_payload_content_items(payload: &Value) -> Vec<Value> {
    payload
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn remote_im_payload_has_non_text_items(payload: &Value) -> bool {
    remote_im_payload_content_items(payload)
        .iter()
        .any(|item| item.get("type").and_then(Value::as_str).unwrap_or("") != "text")
}

fn remote_im_payload_media_summary(payload: &Value) -> Value {
    let items = remote_im_payload_content_items(payload);
    let mut text_count = 0usize;
    let mut image_count = 0usize;
    let mut file_count = 0usize;
    let mut unknown_count = 0usize;
    let mut image_mimes = Vec::<String>::new();
    let mut file_names = Vec::<String>::new();
    for item in items {
        match item.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => text_count += 1,
            "image" => {
                image_count += 1;
                if let Some(mime) = item.get("mime").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
                    image_mimes.push(mime.to_string());
                }
            }
            "file" => {
                file_count += 1;
                if let Some(name) = item.get("name").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
                    file_names.push(name.to_string());
                }
            }
            _ => unknown_count += 1,
        }
    }
    serde_json::json!({
        "textCount": text_count,
        "imageCount": image_count,
        "fileCount": file_count,
        "unknownCount": unknown_count,
        "imageMimes": image_mimes,
        "fileNames": file_names,
    })
}

fn remote_im_content_item_name(item: &Value, default_name: &str) -> String {
    item.get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(default_name)
        .to_string()
}

fn remote_im_content_item_mime(item: &Value, default_mime: &str) -> String {
    item.get("mime")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(default_mime)
        .to_string()
}

async fn remote_im_content_item_bytes(item: &Value) -> Result<Vec<u8>, String> {
    if let Some(b64) = item
        .get("bytesBase64")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return B64
            .decode(b64)
            .map_err(|err| format!("解析内容项 bytesBase64 失败: {err}"));
    }
    let path = item
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "内容项缺少 bytesBase64 或 path".to_string())?;
    tokio::fs::read(path)
        .await
        .map_err(|err| format!("读取内容项文件失败: path={path}, err={err}"))
}

fn remote_im_file_ext_from_name(file_name: &str) -> String {
    std::path::Path::new(file_name)
        .extension()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("bin")
        .to_ascii_lowercase()
}

fn remote_im_credential_text(credentials: &Value, key: &str) -> String {
    credentials
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn remote_im_is_group_contact(contact: &RemoteImContact) -> bool {
    contact.remote_contact_type.trim().eq_ignore_ascii_case("group")
}

fn remote_im_json_or_text(response_text: &str) -> Value {
    serde_json::from_str::<Value>(response_text).unwrap_or_else(|_| {
        serde_json::json!({
            "raw": response_text
        })
    })
}

fn remote_im_is_dingtalk_private_target_likely_conversation_id(remote_contact_id: &str) -> bool {
    remote_contact_id.trim().starts_with("cid")
}

fn remote_im_log(level: &str, event: &str, fields: Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "level": level,
            "event": event,
            "fields": fields
        })
    );
}

include!("remote_im/feishu_sdk.rs");

struct DingtalkSdk;

impl DingtalkSdk {
    async fn access_token(&self, channel: &RemoteImChannelConfig) -> Result<String, String> {
        let started = std::time::Instant::now();
        remote_im_log(
            "INFO",
            "dingtalk.access_token",
            serde_json::json!({
                "task_name": "dingtalk.access_token",
                "trigger": "remote_im_send",
                "channel_id": channel.id,
                "status": "开始"
            }),
        );
        let client_id = remote_im_credential_text(&channel.credentials, "clientId");
        let client_secret = remote_im_credential_text(&channel.credentials, "clientSecret");
        if client_id.is_empty() || client_secret.is_empty() {
            let err = format!("dingtalk channel '{}' missing clientId/clientSecret", channel.id);
            remote_im_log(
                "ERROR",
                "dingtalk.access_token",
                serde_json::json!({
                    "task_name": "dingtalk.access_token",
                    "trigger": "remote_im_send",
                    "channel_id": channel.id,
                    "status": "失败",
                    "error": err,
                    "duration_ms": started.elapsed().as_millis()
                }),
            );
            return Err(err);
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|err| {
                let msg = format!("build dingtalk client failed: {err}");
                remote_im_log(
                    "ERROR",
                    "dingtalk.access_token",
                    serde_json::json!({
                        "task_name": "dingtalk.access_token",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "status": "失败",
                        "error": msg,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                msg
            })?;
        let response = client
            .post("https://api.dingtalk.com/v1.0/oauth2/accessToken")
            .json(&serde_json::json!({
                "appKey": client_id,
                "appSecret": client_secret
            }))
            .send()
            .await
            .map_err(|err| {
                let msg = format!("dingtalk token request failed: {err}");
                remote_im_log(
                    "ERROR",
                    "dingtalk.access_token",
                    serde_json::json!({
                        "task_name": "dingtalk.access_token",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "status": "失败",
                        "error": msg,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                msg
            })?;
        let body = response
            .json::<Value>()
            .await
            .map_err(|err| {
                let msg = format!("parse dingtalk token response failed: {err}");
                remote_im_log(
                    "ERROR",
                    "dingtalk.access_token",
                    serde_json::json!({
                        "task_name": "dingtalk.access_token",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "status": "失败",
                        "error": msg,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                msg
            })?;
        let token = body
            .get("accessToken")
            .and_then(Value::as_str)
            .or_else(|| {
                body.get("data")
                    .and_then(|v| v.get("accessToken"))
                    .and_then(Value::as_str)
            })
            .map(str::trim)
            .unwrap_or("");
        if token.is_empty() {
            let err = format!("dingtalk token missing: {}", body);
            remote_im_log(
                "ERROR",
                "dingtalk.access_token",
                serde_json::json!({
                    "task_name": "dingtalk.access_token",
                    "trigger": "remote_im_send",
                    "channel_id": channel.id,
                    "status": "失败",
                    "token_present": false,
                    "error": err,
                    "duration_ms": started.elapsed().as_millis()
                }),
            );
            return Err(err);
        }
        remote_im_log(
            "INFO",
            "dingtalk.access_token",
            serde_json::json!({
                "task_name": "dingtalk.access_token",
                "trigger": "remote_im_send",
                "channel_id": channel.id,
                "status": "完成",
                "token_present": true,
                "duration_ms": started.elapsed().as_millis()
            }),
        );
        Ok(token.to_string())
    }

    // ========== input validation ==========
    fn validate_and_get_text(&self, payload: &Value) -> Result<String, String> {
        let text = remote_im_payload_text(payload);
        if text.trim().is_empty() {
            return Err("dingtalk outbound text is empty".to_string());
        }
        Ok(text)
    }

    // ========== auth ==========
    async fn access_token_for_channel(
        &self,
        channel: &RemoteImChannelConfig,
    ) -> Result<String, String> {
        self.access_token(channel).await
    }

    fn get_robot_code(&self, channel: &RemoteImChannelConfig) -> Option<String> {
        let robot_code = remote_im_credential_text(&channel.credentials, "robotCode");
        if !robot_code.is_empty() {
            return Some(robot_code);
        }
        let client_id = remote_im_credential_text(&channel.credentials, "clientId");
        if client_id.is_empty() {
            return None;
        }
        Some(client_id)
    }

    fn session_webhook_from_contact(&self, contact: &RemoteImContact) -> Option<String> {
        contact
            .dingtalk_session_webhook
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn session_webhook_expired(&self, contact: &RemoteImContact) -> bool {
        let Some(expired_ms) = contact.dingtalk_session_webhook_expired_time else {
            return false;
        };
        let now_ms = chrono::Utc::now().timestamp_millis();
        now_ms >= expired_ms
    }

    fn validate_target_is_private(&self, contact: &RemoteImContact) -> Result<(), String> {
        let is_group = contact.remote_contact_type.trim().eq_ignore_ascii_case("group");
        if !is_group
            && remote_im_is_dingtalk_private_target_likely_conversation_id(
                &contact.remote_contact_id,
            )
        {
            return Err("dingtalk private outbound expects userId/staffId, got conversation id".to_string());
        }
        Ok(())
    }

    // ========== request build ==========
    fn build_request_body(
        &self,
        is_group: bool,
        contact: &RemoteImContact,
        robot_code: &str,
        msg_key: &str,
        msg_param: Value,
    ) -> (String, Value) {
        if is_group {
            (
                "https://api.dingtalk.com/v1.0/robot/groupMessages/send".to_string(),
                serde_json::json!({
                    "msgKey": msg_key,
                    "msgParam": msg_param.to_string(),
                    "openConversationId": contact.remote_contact_id,
                    "robotCode": robot_code
                }),
            )
        } else {
            (
                "https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend".to_string(),
                serde_json::json!({
                    "robotCode": robot_code,
                    "userIds": [contact.remote_contact_id],
                    "msgKey": msg_key,
                    "msgParam": msg_param.to_string()
                }),
            )
        }
    }

    async fn upload_media_id(
        &self,
        client: &reqwest::Client,
        token: &str,
        media_type: &str,
        file_name: &str,
        mime: &str,
        raw: Vec<u8>,
    ) -> Result<String, String> {
        let part = reqwest::multipart::Part::bytes(raw)
            .file_name(file_name.to_string())
            .mime_str(mime)
            .map_err(|err| format!("build dingtalk media part mime failed: {err}"))?;
        let form = reqwest::multipart::Form::new().part("media", part);
        let response = client
            .post(format!(
                "https://oapi.dingtalk.com/media/upload?access_token={token}&type={media_type}"
            ))
            .multipart(form)
            .send()
            .await
            .map_err(|err| format!("dingtalk media upload failed: {err}"))?;
        let body = response
            .json::<Value>()
            .await
            .map_err(|err| format!("parse dingtalk media upload response failed: {err}"))?;
        if body.get("errcode").and_then(Value::as_i64).unwrap_or(0) != 0 {
            return Err(format!("dingtalk media upload rejected: {}", body));
        }
        let media_id = body
            .get("media_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if media_id.is_empty() {
            return Err(format!("dingtalk media upload missing media_id: {}", body));
        }
        Ok(media_id)
    }

    // ========== http send/parse ==========
    async fn send_and_parse(
        &self,
        client: &reqwest::Client,
        url: String,
        token: &str,
        body: &Value,
    ) -> Result<Value, String> {
        let response = client
            .post(url)
            .header("x-acs-dingtalk-access-token", token)
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(|err| format!("dingtalk send failed: {err}"))?;
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|err| format!("read dingtalk send response failed: {err}"))?;
        let parsed = remote_im_json_or_text(&response_text);
        if !status.is_success() {
            return Err(format!(
                "dingtalk send rejected http {}: {}",
                status.as_u16(),
                parsed
            ));
        }
        if parsed.get("errcode").and_then(Value::as_i64).unwrap_or(0) != 0
            || parsed.get("code").and_then(Value::as_i64).unwrap_or(0) != 0
        {
            return Err(format!("dingtalk send rejected: {}", parsed));
        }
        Ok(parsed)
    }

    async fn send_via_session_webhook(
        &self,
        client: &reqwest::Client,
        webhook: &str,
        text: &str,
    ) -> Result<Value, String> {
        let response = client
            .post(webhook)
            .header(CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({
                "msgtype": "text",
                "text": {
                    "content": text
                }
            }))
            .send()
            .await
            .map_err(|err| format!("dingtalk sessionWebhook send failed: {err}"))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|err| format!("read dingtalk sessionWebhook response failed: {err}"))?;
        let parsed = remote_im_json_or_text(&response_text);
        if !status.is_success() {
            return Err(format!(
                "dingtalk sessionWebhook rejected http {}: {}",
                status.as_u16(),
                parsed
            ));
        }
        if parsed.get("errcode").and_then(Value::as_i64).unwrap_or(0) != 0
            || parsed.get("code").and_then(Value::as_i64).unwrap_or(0) != 0
        {
            return Err(format!("dingtalk sessionWebhook rejected: {}", parsed));
        }
        Ok(parsed)
    }

    fn process_query_key_from_response(&self, body: &Value) -> String {
        body.get("processQueryKey")
            .and_then(Value::as_str)
            .or_else(|| {
                body.get("data")
                    .and_then(|v| v.get("processQueryKey"))
                    .and_then(Value::as_str)
            })
            .unwrap_or_default()
            .to_string()
    }

    fn log_outcome(
        &self,
        level: &str,
        channel: &RemoteImChannelConfig,
        contact: &RemoteImContact,
        is_group: bool,
        content_count: usize,
        text_len: usize,
        started: std::time::Instant,
        status: &str,
        process_query_key: Option<&str>,
        error: Option<&str>,
        send_mode: Option<&str>,
    ) {
        let mut fields = serde_json::json!({
            "task_name": "dingtalk.send_outbound",
            "trigger": "remote_im_send",
            "channel_id": channel.id,
            "contact_id": contact.id,
            "remote_contact_id": contact.remote_contact_id,
            "is_group": is_group,
            "content_item_count": content_count,
            "message_length": text_len,
            "status": status,
            "duration_ms": started.elapsed().as_millis()
        });
        if let Some(value) = process_query_key {
            if let Some(obj) = fields.as_object_mut() {
                obj.insert("process_query_key".to_string(), serde_json::json!(value));
            }
        }
        if let Some(value) = error {
            if let Some(obj) = fields.as_object_mut() {
                obj.insert("error".to_string(), serde_json::json!(value));
            }
        }
        if let Some(value) = send_mode {
            if let Some(obj) = fields.as_object_mut() {
                obj.insert("send_mode".to_string(), serde_json::json!(value));
            }
        }
        remote_im_log(level, "dingtalk.send_outbound", fields);
    }
}

impl RemoteImSdk for DingtalkSdk {
    fn platform(&self) -> RemoteImPlatform {
        RemoteImPlatform::Dingtalk
    }

    fn validate_channel(&self, channel: &RemoteImChannelConfig) -> Result<(), String> {
        if remote_im_credential_text(&channel.credentials, "clientId").is_empty()
            || remote_im_credential_text(&channel.credentials, "clientSecret").is_empty()
        {
            return Err(format!("dingtalk channel '{}' credentials invalid", channel.id));
        }
        Ok(())
    }

    fn send_outbound<'a>(
        &'a self,
        channel: &'a RemoteImChannelConfig,
        contact: &'a RemoteImContact,
        payload: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let started = std::time::Instant::now();
            let items = remote_im_payload_content_items(payload);
            if items.is_empty() {
                return Err("dingtalk outbound content is empty".to_string());
            }
            let content_count = items.len();
            let is_group = contact.remote_contact_type.trim().eq_ignore_ascii_case("group");
            let text_preview = remote_im_payload_text(payload);
            let has_non_text = remote_im_payload_has_non_text_items(payload);
            let has_session_webhook = self.session_webhook_from_contact(contact).is_some() && !has_non_text;
            let has_robot_code = self.get_robot_code(channel).is_some();
            let default_mode = if has_non_text {
                "openapi_media"
            } else if has_session_webhook {
                "stream_session_webhook"
            } else if has_robot_code {
                "openapi_robot"
            } else {
                "none"
            };
            self.log_outcome(
                "INFO",
                channel,
                contact,
                is_group,
                content_count,
                text_preview.len(),
                started,
                "开始",
                None,
                None,
                Some(default_mode),
            );
            remote_im_log(
                "INFO",
                "dingtalk.send_outbound_payload",
                serde_json::json!({
                    "task_name": "dingtalk.send_outbound_payload",
                    "trigger": "remote_im_send",
                    "channel_id": channel.id,
                    "contact_id": contact.id,
                    "remote_contact_id": contact.remote_contact_id,
                    "send_mode": default_mode,
                    "payload_summary": remote_im_payload_media_summary(payload),
                    "status": "开始"
                }),
            );

            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .build()
            {
                Ok(client) => client,
                Err(err) => {
                    let message = format!("build dingtalk client failed: {err}");
                    self.log_outcome(
                        "ERROR",
                        channel,
                        contact,
                        is_group,
                        content_count,
                        text_preview.len(),
                        started,
                        "失败",
                        None,
                        Some(&message),
                        None,
                    );
                    return Err(message);
                }
            };

            // ========== stream session webhook ==========
            if let Some(webhook) = self.session_webhook_from_contact(contact) {
                if !has_non_text {
                    if self.session_webhook_expired(contact) {
                        let err =
                            "dingtalk sessionWebhook 已过期，请等待联系人发送新消息后再回复".to_string();
                        self.log_outcome(
                            "ERROR",
                            channel,
                            contact,
                            is_group,
                            content_count,
                            text_preview.len(),
                            started,
                            "失败",
                            None,
                            Some(&err),
                            Some("stream_session_webhook"),
                        );
                        return Err(err);
                    }
                    let text = match self.validate_and_get_text(payload) {
                        Ok(text) => text,
                        Err(err) => {
                            self.log_outcome(
                                "ERROR",
                                channel,
                                contact,
                                is_group,
                                content_count,
                                0,
                                started,
                                "失败",
                                None,
                                Some(&err),
                                Some("stream_session_webhook"),
                            );
                            return Err(err);
                        }
                    };
                    let parsed = match self.send_via_session_webhook(&client, &webhook, &text).await {
                        Ok(parsed) => parsed,
                        Err(err) => {
                            self.log_outcome(
                                "ERROR",
                                channel,
                                contact,
                                is_group,
                                content_count,
                                text.len(),
                                started,
                                "失败",
                                None,
                                Some(&err),
                                Some("stream_session_webhook"),
                            );
                            return Err(err);
                        }
                    };
                    let message_id = parsed
                        .get("processQueryKey")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    self.log_outcome(
                        "INFO",
                        channel,
                        contact,
                        is_group,
                        content_count,
                        text.len(),
                        started,
                        "完成",
                        Some(&message_id),
                        None,
                        Some("stream_session_webhook"),
                    );
                    return Ok(message_id);
                }
            }

            // ========== openapi fallback ==========
            let Some(robot_code) = self.get_robot_code(channel) else {
                let err =
                    "dingtalk 非文本发送缺少可用发送身份：未找到 robotCode，且 clientId 也不可用"
                        .to_string();
                self.log_outcome(
                    "ERROR",
                    channel,
                    contact,
                    is_group,
                    content_count,
                    text_preview.len(),
                    started,
                    "失败",
                    None,
                    Some(&err),
                    Some("none"),
                );
                return Err(err);
            };
            if let Err(err) = self.validate_target_is_private(contact) {
                self.log_outcome(
                    "ERROR",
                    channel,
                    contact,
                    is_group,
                    content_count,
                    text_preview.len(),
                    started,
                    "失败",
                    None,
                    Some(&err),
                    Some("openapi_robot"),
                );
                return Err(err);
            }
            let token = match self.access_token_for_channel(channel).await {
                Ok(token) => token,
                Err(err) => {
                    self.log_outcome(
                        "ERROR",
                        channel,
                        contact,
                        is_group,
                        content_count,
                        text_preview.len(),
                        started,
                        "失败",
                        None,
                        Some(&err),
                        Some("openapi_robot"),
                    );
                    return Err(err);
                }
            };
            let mut process_query_key = String::new();
            for item in &items {
                let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
                let (msg_key, msg_param) = match item_type {
                    "text" => {
                        let text = item.get("text").and_then(Value::as_str).unwrap_or("").trim();
                        if text.is_empty() {
                            continue;
                        }
                        ("sampleText".to_string(), serde_json::json!({ "content": text }))
                    }
                    "image" => {
                        let image_name = remote_im_content_item_name(item, "image.png");
                        let image_mime = remote_im_content_item_mime(item, "image/png");
                        let image_raw = remote_im_content_item_bytes(item).await?;
                        let media_id = self
                            .upload_media_id(&client, &token, "image", &image_name, &image_mime, image_raw)
                            .await?;
                        (
                            "sampleImageMsg".to_string(),
                            serde_json::json!({ "photoURL": media_id }),
                        )
                    }
                    "file" => {
                        let file_name = remote_im_content_item_name(item, "attachment.bin");
                        let file_mime = remote_im_content_item_mime(item, "application/octet-stream");
                        let file_raw = remote_im_content_item_bytes(item).await?;
                        let media_id = self
                            .upload_media_id(&client, &token, "file", &file_name, &file_mime, file_raw)
                            .await?;
                        (
                            "sampleFile".to_string(),
                            serde_json::json!({
                                "mediaId": media_id,
                                "fileName": file_name,
                                "fileType": remote_im_file_ext_from_name(&file_name),
                            }),
                        )
                    }
                    _ => continue,
                };
                let (url, body) =
                    self.build_request_body(is_group, contact, &robot_code, &msg_key, msg_param);
                let parsed = match self.send_and_parse(&client, url, &token, &body).await {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        self.log_outcome(
                            "ERROR",
                            channel,
                            contact,
                            is_group,
                            content_count,
                            text_preview.len(),
                            started,
                            "失败",
                            None,
                            Some(&err),
                            Some("openapi_robot"),
                        );
                        return Err(err);
                    }
                };
                let current = self.process_query_key_from_response(&parsed);
                if !current.trim().is_empty() {
                    process_query_key = current;
                }
            }
            if process_query_key.trim().is_empty() {
                let err = "钉钉发送被跳过：未生成任何可发送消息".to_string();
                self.log_outcome(
                    "WARN",
                    channel,
                    contact,
                    is_group,
                    content_count,
                    text_preview.len(),
                    started,
                    "跳过",
                    None,
                    Some(&err),
                    Some("openapi_robot"),
                );
                return Err(err);
            }

            // ========== final logging ==========
            self.log_outcome(
                "INFO",
                channel,
                contact,
                is_group,
                content_count,
                text_preview.len(),
                started,
                "完成",
                Some(&process_query_key),
                None,
                Some("openapi_robot"),
            );
            Ok(process_query_key)
        })
    }
}

struct OnebotV11Sdk;

impl RemoteImSdk for OnebotV11Sdk {
    fn platform(&self) -> RemoteImPlatform {
        RemoteImPlatform::OnebotV11
    }

    fn validate_channel(&self, channel: &RemoteImChannelConfig) -> Result<(), String> {
        let ws_port = channel
            .credentials
            .get("wsPort")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if ws_port == 0 {
            return Err(format!("onebot v11 channel '{}' missing wsPort", channel.id));
        }
        Ok(())
    }

    fn send_outbound<'a>(
        &'a self,
        channel: &'a RemoteImChannelConfig,
        contact: &'a RemoteImContact,
        payload: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let segments = remote_im_onebot_message_segments(payload);
            if segments.is_empty() {
                return Err("onebot v11 outbound content is empty".to_string());
            }

            let manager = onebot_v11_ws_manager();
            if !manager.is_connected(&channel.id).await {
                return Err(format!("onebot v11 channel '{}' not connected", channel.id));
            }

            let action = if remote_im_is_group_contact(contact) {
                "send_group_msg"
            } else {
                "send_private_msg"
            };

            let params = if action == "send_group_msg" {
                serde_json::json!({
                    "group_id": contact.remote_contact_id,
                    "message": segments
                })
            } else {
                serde_json::json!({
                    "user_id": contact.remote_contact_id,
                    "message": segments
                })
            };

            let started = std::time::Instant::now();
            remote_im_log(
                "INFO",
                "onebot.send_outbound",
                serde_json::json!({
                    "channel_id": channel.id,
                    "action": action,
                    "remote_contact_id": contact.remote_contact_id,
                    "status": "开始",
                    "payload_summary": remote_im_payload_media_summary(payload)
                }),
            );
            let result = match manager.call_api(&channel.id, action, params, 10000).await {
                Ok(value) => value,
                Err(err) => {
                    remote_im_log(
                        "ERROR",
                        "onebot.send_outbound",
                        serde_json::json!({
                            "channel_id": channel.id,
                            "action": action,
                            "remote_contact_id": contact.remote_contact_id,
                            "status": "失败",
                            "error": err,
                            "duration_ms": started.elapsed().as_millis()
                        }),
                    );
                    return Err(err);
                }
            };
            
            // 提取 message_id
            let message_id = result
                .get("message_id")
                .map(|v| {
                    if let Some(id) = v.as_i64() {
                        id.to_string()
                    } else {
                        v.as_str().unwrap_or_default().to_string()
                    }
                })
                .unwrap_or_default();
            remote_im_log(
                "INFO",
                "onebot.send_outbound",
                serde_json::json!({
                    "channel_id": channel.id,
                    "action": action,
                    "remote_contact_id": contact.remote_contact_id,
                    "status": "完成",
                    "message_id": message_id,
                    "duration_ms": started.elapsed().as_millis()
                }),
            );

            Ok(message_id)
        })
    }
}

struct WeixinOcSdk;

impl RemoteImSdk for WeixinOcSdk {
    fn platform(&self) -> RemoteImPlatform {
        RemoteImPlatform::WeixinOc
    }

    fn validate_channel(&self, channel: &RemoteImChannelConfig) -> Result<(), String> {
        let credentials = WeixinOcCredentials::from_value(&channel.credentials);
        if credentials.normalized_base_url().trim().is_empty() {
            return Err(format!("weixin_oc channel '{}' missing baseUrl", channel.id));
        }
        if credentials.token.trim().is_empty() {
            return Err(format!("weixin_oc channel '{}' missing token, please login first", channel.id));
        }
        Ok(())
    }

    fn send_outbound<'a>(
        &'a self,
        channel: &'a RemoteImChannelConfig,
        contact: &'a RemoteImContact,
        payload: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            if remote_im_payload_has_non_text_items(payload) {
                return Err("个人微信渠道首版仅支持文本发送".to_string());
            }
            let text = remote_im_payload_text(payload);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err("个人微信渠道发送内容为空".to_string());
            }
            let credentials = WeixinOcCredentials::from_value(&channel.credentials);
            let context_token = weixin_oc_manager()
                .get_context_token(&channel.id, &contact.remote_contact_id)
                .await;
            let message_id = weixin_oc_send_text_message(
                credentials,
                &contact.remote_contact_id,
                trimmed,
                context_token.as_deref(),
            )
            .await?;
            // 回复发送成功后停止 typing
            weixin_oc_manager()
                .stop_typing(&channel.id, &contact.remote_contact_id)
                .await;
            Ok(message_id)
        })
    }
}

fn remote_im_sdk_for_platform(platform: &RemoteImPlatform) -> Box<dyn RemoteImSdk> {
    match platform {
        RemoteImPlatform::Feishu => Box::new(FeishuSdk),
        RemoteImPlatform::Dingtalk => Box::new(DingtalkSdk),
        RemoteImPlatform::OnebotV11 => Box::new(OnebotV11Sdk),
        RemoteImPlatform::WeixinOc => Box::new(WeixinOcSdk),
    }
}

#[cfg(test)]
fn remote_im_mock_send_via_sdk(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    payload: &Value,
) -> Option<Result<String, String>> {
    let mock_error = channel
        .credentials
        .get("mockSendError")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(err) = mock_error {
        return Some(Err(err.to_string()));
    }

    let mock_send_enabled = channel
        .credentials
        .get("mockSend")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !mock_send_enabled {
        return None;
    }

    let text_len = remote_im_payload_text(payload).chars().count();
    Some(Ok(format!(
        "mock:{}:{}:{}",
        channel.id, contact.remote_contact_id, text_len
    )))
}

async fn remote_im_send_via_sdk(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    payload: &Value,
) -> Result<String, String> {
    #[cfg(test)]
    if let Some(mock_result) = remote_im_mock_send_via_sdk(channel, contact, payload) {
        return mock_result;
    }

    let sdk = remote_im_sdk_for_platform(&channel.platform);
    sdk.validate_channel(channel)?;
    sdk.send_outbound(channel, contact, payload).await
}

#[cfg(test)]
mod remote_im_adapter_tests {
    use super::*;

    fn mock_channel(platform: RemoteImPlatform, credentials: Value) -> RemoteImChannelConfig {
        RemoteImChannelConfig {
            id: "ch".to_string(),
            name: "test".to_string(),
            platform,
            enabled: true,
            credentials,
            activate_assistant: true,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_send_files: false,
        }
    }

    #[test]
    fn payload_text_should_merge_all_text_blocks() {
        let payload = serde_json::json!({
            "content": [
                {"type":"text","text":"a"},
                {"type":"image","data":"x"},
                {"type":"text","text":"b"}
            ]
        });
        assert_eq!(remote_im_payload_text(&payload), "a\nb".to_string());
    }

    #[test]
    fn onebot_segments_should_keep_text_image_file() {
        let payload = serde_json::json!({
            "content": [
                {"type":"text","text":"hello"},
                {"type":"image","bytesBase64":"YWJj"},
                {"type":"file","path":"C:/tmp/readme.txt"}
            ]
        });
        let segments = remote_im_onebot_message_segments(&payload);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].get("type").and_then(Value::as_str), Some("text"));
        assert_eq!(segments[1].get("type").and_then(Value::as_str), Some("image"));
        assert_eq!(segments[2].get("type").and_then(Value::as_str), Some("file"));
    }

    #[test]
    fn sdk_router_should_return_expected_platform() {
        assert_eq!(
            remote_im_sdk_for_platform(&RemoteImPlatform::Feishu).platform(),
            RemoteImPlatform::Feishu
        );
        assert_eq!(
            remote_im_sdk_for_platform(&RemoteImPlatform::Dingtalk).platform(),
            RemoteImPlatform::Dingtalk
        );
        assert_eq!(
            remote_im_sdk_for_platform(&RemoteImPlatform::OnebotV11).platform(),
            RemoteImPlatform::OnebotV11
        );
    }

    #[test]
    fn feishu_validate_should_require_app_credentials() {
        let sdk = FeishuSdk;
        let ok = mock_channel(
            RemoteImPlatform::Feishu,
            serde_json::json!({"appId":"x","appSecret":"y"}),
        );
        assert!(sdk.validate_channel(&ok).is_ok());
        let bad = mock_channel(RemoteImPlatform::Feishu, serde_json::json!({"appId":"x"}));
        assert!(sdk.validate_channel(&bad).is_err());
    }

    #[test]
    fn dingtalk_validate_should_require_client_credentials() {
        let sdk = DingtalkSdk;
        let ok = mock_channel(
            RemoteImPlatform::Dingtalk,
            serde_json::json!({"clientId":"x","clientSecret":"y"}),
        );
        assert!(sdk.validate_channel(&ok).is_ok());
        let bad = mock_channel(RemoteImPlatform::Dingtalk, serde_json::json!({"clientId":"x"}));
        assert!(sdk.validate_channel(&bad).is_err());
    }

    #[test]
    fn dingtalk_robot_code_should_fallback_to_client_id() {
        let sdk = DingtalkSdk;
        let channel = mock_channel(
            RemoteImPlatform::Dingtalk,
            serde_json::json!({"clientId":"client-x","clientSecret":"secret-y"}),
        );
        assert_eq!(sdk.get_robot_code(&channel).as_deref(), Some("client-x"));

        let explicit = mock_channel(
            RemoteImPlatform::Dingtalk,
            serde_json::json!({"clientId":"client-x","clientSecret":"secret-y","robotCode":"robot-z"}),
        );
        assert_eq!(sdk.get_robot_code(&explicit).as_deref(), Some("robot-z"));
    }

    #[test]
    fn napcat_validate_should_require_ws_port() {
        let sdk = OnebotV11Sdk;
        let ok = mock_channel(
            RemoteImPlatform::OnebotV11,
            serde_json::json!({"wsPort":6199}),
        );
        assert!(sdk.validate_channel(&ok).is_ok());
        let bad = mock_channel(RemoteImPlatform::OnebotV11, serde_json::json!({}));
        assert!(sdk.validate_channel(&bad).is_err());
    }

    #[test]
    fn is_group_contact_should_match_remote_contact_type() {
        let group = RemoteImContact {
            id: "1".to_string(),
            channel_id: "c".to_string(),
            platform: RemoteImPlatform::Feishu,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "gid".to_string(),
            remote_contact_name: "g".to_string(),
            remark_name: String::new(),
            allow_send: false,
            allow_send_files: false,
            allow_receive: false,
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
        };
        let private = RemoteImContact {
            remote_contact_type: "private".to_string(),
            ..group.clone()
        };
        assert!(remote_im_is_group_contact(&group));
        assert!(!remote_im_is_group_contact(&private));
    }

    #[test]
    fn json_or_text_should_fallback_to_raw_text() {
        let value = remote_im_json_or_text("not-json");
        assert_eq!(
            value.get("raw").and_then(Value::as_str),
            Some("not-json")
        );
    }

    #[test]
    fn dingtalk_private_target_guard_should_detect_conversation_id_prefix() {
        assert!(remote_im_is_dingtalk_private_target_likely_conversation_id(
            "cid6KeBBLoveMJOGX"
        ));
        assert!(!remote_im_is_dingtalk_private_target_likely_conversation_id(
            "manager1234"
        ));
    }
}

