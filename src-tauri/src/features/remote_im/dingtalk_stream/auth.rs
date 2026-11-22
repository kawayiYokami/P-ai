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
                        "[远程IM] 钉钉生命周期 task={} status=完成 trigger=callback_handler_ready key_counts=0 duration_ms=0",
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

