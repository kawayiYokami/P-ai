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

struct FeishuSdk;

impl FeishuSdk {
    async fn tenant_access_token(&self, channel: &RemoteImChannelConfig) -> Result<String, String> {
        let started = std::time::Instant::now();
        remote_im_log(
            "INFO",
            "feishu.tenant_access_token",
            serde_json::json!({
                "channel_id": channel.id,
                "status": "开始"
            }),
        );
        let app_id = remote_im_credential_text(&channel.credentials, "appId");
        let app_secret = remote_im_credential_text(&channel.credentials, "appSecret");
        if app_id.is_empty() || app_secret.is_empty() {
            let err = format!("feishu channel '{}' missing appId/appSecret", channel.id);
            remote_im_log(
                "ERROR",
                "feishu.tenant_access_token",
                serde_json::json!({
                    "channel_id": channel.id,
                    "status": "失败",
                    "error": err
                }),
            );
            return Err(err);
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|err| {
                let msg = format!("build feishu client failed: {err}");
                remote_im_log(
                    "ERROR",
                    "feishu.tenant_access_token",
                    serde_json::json!({
                        "channel_id": channel.id,
                        "status": "失败",
                        "error": msg
                    }),
                );
                msg
            })?;
        let response = client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": app_id,
                "app_secret": app_secret
            }))
            .send()
            .await
            .map_err(|err| {
                let msg = format!("feishu token request failed: {err}");
                remote_im_log(
                    "ERROR",
                    "feishu.tenant_access_token",
                    serde_json::json!({
                        "channel_id": channel.id,
                        "status": "失败",
                        "error": msg
                    }),
                );
                msg
            })?;
        let body = response
            .json::<Value>()
            .await
            .map_err(|err| {
                let msg = format!("parse feishu token response failed: {err}");
                remote_im_log(
                    "ERROR",
                    "feishu.tenant_access_token",
                    serde_json::json!({
                        "channel_id": channel.id,
                        "status": "失败",
                        "error": msg
                    }),
                );
                msg
            })?;
        let token = body
            .get("tenant_access_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if token.is_empty() {
            let err = format!("feishu token missing: {}", body);
            remote_im_log(
                "ERROR",
                "feishu.tenant_access_token",
                serde_json::json!({
                    "channel_id": channel.id,
                    "status": "失败",
                    "error": err
                }),
            );
            return Err(err);
        }
        remote_im_log(
            "INFO",
            "feishu.tenant_access_token",
            serde_json::json!({
                "channel_id": channel.id,
                "status": "完成",
                "duration_ms": started.elapsed().as_millis(),
                "token_masked": format!("***len:{}***", token.len())
            }),
        );
        Ok(token.to_string())
    }
}

impl RemoteImSdk for FeishuSdk {
    fn platform(&self) -> RemoteImPlatform {
        RemoteImPlatform::Feishu
    }

    fn validate_channel(&self, channel: &RemoteImChannelConfig) -> Result<(), String> {
        if remote_im_credential_text(&channel.credentials, "appId").is_empty()
            || remote_im_credential_text(&channel.credentials, "appSecret").is_empty()
        {
            return Err(format!("feishu channel '{}' credentials invalid", channel.id));
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
            let text = remote_im_payload_text(payload);
            if text.trim().is_empty() {
                remote_im_log(
                    "ERROR",
                    "feishu.send_outbound",
                    serde_json::json!({
                        "channel_id": channel.id,
                        "contact_id": contact.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "status": "失败",
                        "error": "feishu outbound text is empty"
                    }),
                );
                return Err("feishu outbound text is empty".to_string());
            }
            let token = self.tenant_access_token(channel).await?;
            let receive_id_type = remote_im_credential_text(&channel.credentials, "receiveIdType");
            let receive_id_type = if receive_id_type.is_empty() {
                if remote_im_is_group_contact(contact) {
                    "chat_id".to_string()
                } else {
                    "open_id".to_string()
                }
            } else {
                receive_id_type
            };
            remote_im_log(
                "INFO",
                "feishu.send_outbound",
                serde_json::json!({
                    "channel_id": channel.id,
                    "contact_id": contact.id,
                    "remote_contact_id": contact.remote_contact_id,
                    "receive_id_type": receive_id_type,
                    "status": "开始"
                }),
            );
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .build()
                .map_err(|err| {
                    let msg = format!("build feishu client failed: {err}");
                    remote_im_log(
                        "ERROR",
                        "feishu.send_outbound",
                        serde_json::json!({
                            "channel_id": channel.id,
                            "contact_id": contact.id,
                            "status": "失败",
                            "error": msg
                        }),
                    );
                    msg
                })?;
            let response = client
                .post(format!(
                    "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type={receive_id_type}"
                ))
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header(CONTENT_TYPE, "application/json")
                .json(&serde_json::json!({
                    "receive_id": contact.remote_contact_id,
                    "msg_type": "text",
                    "content": serde_json::json!({"text": text}).to_string()
                }))
                .send()
                .await
                .map_err(|err| {
                    let msg = format!("feishu send failed: {err}");
                    remote_im_log(
                        "ERROR",
                        "feishu.send_outbound",
                        serde_json::json!({
                            "channel_id": channel.id,
                            "contact_id": contact.id,
                            "status": "失败",
                            "error": msg
                        }),
                    );
                    msg
                })?;
            let body = response
                .json::<Value>()
                .await
                .map_err(|err| {
                    let msg = format!("parse feishu send response failed: {err}");
                    remote_im_log(
                        "ERROR",
                        "feishu.send_outbound",
                        serde_json::json!({
                            "channel_id": channel.id,
                            "contact_id": contact.id,
                            "status": "失败",
                            "error": msg
                        }),
                    );
                    msg
                })?;
            if body.get("code").and_then(Value::as_i64).unwrap_or(-1) != 0 {
                let err = format!("feishu send rejected: {}", body);
                remote_im_log(
                    "ERROR",
                    "feishu.send_outbound",
                    serde_json::json!({
                        "channel_id": channel.id,
                        "contact_id": contact.id,
                        "status": "失败",
                        "error": err
                    }),
                );
                return Err(err);
            }
            let message_id = body
                .get("data")
                .and_then(|v| v.get("message_id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            remote_im_log(
                "INFO",
                "feishu.send_outbound",
                serde_json::json!({
                    "channel_id": channel.id,
                    "contact_id": contact.id,
                    "remote_contact_id": contact.remote_contact_id,
                    "receive_id_type": receive_id_type,
                    "status": "完成",
                    "message_id": message_id,
                    "duration_ms": started.elapsed().as_millis()
                }),
            );
            Ok(message_id)
        })
    }
}

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
            let text = remote_im_payload_text(payload);
            let content_count = payload
                .get("content")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0);
            let is_group = contact.remote_contact_type.trim().eq_ignore_ascii_case("group");
            remote_im_log(
                "INFO",
                "dingtalk.send_outbound",
                serde_json::json!({
                    "task_name": "dingtalk.send_outbound",
                    "trigger": "remote_im_send",
                    "channel_id": channel.id,
                    "contact_id": contact.id,
                    "remote_contact_id": contact.remote_contact_id,
                    "is_group": is_group,
                    "content_item_count": content_count,
                    "message_length": text.len(),
                    "status": "开始"
                }),
            );
            if text.trim().is_empty() {
                let err = "dingtalk outbound text is empty".to_string();
                remote_im_log(
                    "ERROR",
                    "dingtalk.send_outbound",
                    serde_json::json!({
                        "task_name": "dingtalk.send_outbound",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "is_group": is_group,
                        "content_item_count": content_count,
                        "message_length": text.len(),
                        "status": "失败",
                        "error": err,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                return Err(err);
            }
            let token = match self.access_token(channel).await {
                Ok(token) => token,
                Err(err) => {
                    remote_im_log(
                        "ERROR",
                        "dingtalk.send_outbound",
                        serde_json::json!({
                            "task_name": "dingtalk.send_outbound",
                            "trigger": "remote_im_send",
                            "channel_id": channel.id,
                            "remote_contact_id": contact.remote_contact_id,
                            "is_group": is_group,
                            "status": "失败",
                            "error": err,
                            "duration_ms": started.elapsed().as_millis()
                        }),
                    );
                    return Err(err);
                }
            };
            let robot_code = remote_im_credential_text(&channel.credentials, "robotCode");
            if robot_code.is_empty() {
                let err = format!("dingtalk channel '{}' missing robotCode", channel.id);
                remote_im_log(
                    "ERROR",
                    "dingtalk.send_outbound",
                    serde_json::json!({
                        "task_name": "dingtalk.send_outbound",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "is_group": is_group,
                        "status": "失败",
                        "error": err,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                return Err(err);
            }
            if !is_group
                && remote_im_is_dingtalk_private_target_likely_conversation_id(
                    &contact.remote_contact_id,
                )
            {
                let err =
                    "dingtalk private outbound expects userId/staffId, got conversation id".to_string();
                remote_im_log(
                    "ERROR",
                    "dingtalk.send_outbound",
                    serde_json::json!({
                        "task_name": "dingtalk.send_outbound",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "is_group": is_group,
                        "status": "失败",
                        "error": err,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                return Err(err);
            }
            let (url, body) = if is_group {
                (
                    "https://api.dingtalk.com/v1.0/robot/groupMessages/send".to_string(),
                    serde_json::json!({
                        "msgKey": "sampleText",
                        "msgParam": serde_json::json!({"content": text}).to_string(),
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
                        "msgKey": "sampleText",
                        "msgParam": serde_json::json!({"content": text}).to_string()
                    }),
                )
            };
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .build()
                .map_err(|err| {
                    let msg = format!("build dingtalk client failed: {err}");
                    remote_im_log(
                        "ERROR",
                        "dingtalk.send_outbound",
                        serde_json::json!({
                            "task_name": "dingtalk.send_outbound",
                            "trigger": "remote_im_send",
                            "channel_id": channel.id,
                            "remote_contact_id": contact.remote_contact_id,
                            "is_group": is_group,
                            "status": "失败",
                            "error": msg,
                            "duration_ms": started.elapsed().as_millis()
                        }),
                    );
                    msg
                })?;
            let response = client
                .post(url)
                .header("x-acs-dingtalk-access-token", token)
                .header(CONTENT_TYPE, "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|err| {
                    let msg = format!("dingtalk send failed: {err}");
                    remote_im_log(
                        "ERROR",
                        "dingtalk.send_outbound",
                        serde_json::json!({
                            "task_name": "dingtalk.send_outbound",
                            "trigger": "remote_im_send",
                            "channel_id": channel.id,
                            "remote_contact_id": contact.remote_contact_id,
                            "is_group": is_group,
                            "status": "失败",
                            "error": msg,
                            "duration_ms": started.elapsed().as_millis()
                        }),
                    );
                    msg
                })?;
            let status = response.status();
            let response_text = response
                .text()
                .await
                .map_err(|err| {
                    let msg = format!("read dingtalk send response failed: {err}");
                    remote_im_log(
                        "ERROR",
                        "dingtalk.send_outbound",
                        serde_json::json!({
                            "task_name": "dingtalk.send_outbound",
                            "trigger": "remote_im_send",
                            "channel_id": channel.id,
                            "remote_contact_id": contact.remote_contact_id,
                            "is_group": is_group,
                            "status": "失败",
                            "error": msg,
                            "duration_ms": started.elapsed().as_millis()
                        }),
                    );
                    msg
                })?;
            let body = remote_im_json_or_text(&response_text);
            if !status.is_success() {
                let err = format!(
                    "dingtalk send rejected http {}: {}",
                    status.as_u16(),
                    body
                );
                remote_im_log(
                    "ERROR",
                    "dingtalk.send_outbound",
                    serde_json::json!({
                        "task_name": "dingtalk.send_outbound",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "is_group": is_group,
                        "status": "失败",
                        "error": err,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                return Err(err);
            }
            if body.get("errcode").and_then(Value::as_i64).unwrap_or(0) != 0 {
                let err = format!("dingtalk send rejected: {}", body);
                remote_im_log(
                    "ERROR",
                    "dingtalk.send_outbound",
                    serde_json::json!({
                        "task_name": "dingtalk.send_outbound",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "is_group": is_group,
                        "status": "失败",
                        "error": err,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                return Err(err);
            }
            if body.get("code").and_then(Value::as_i64).unwrap_or(0) != 0 {
                let err = format!("dingtalk send rejected: {}", body);
                remote_im_log(
                    "ERROR",
                    "dingtalk.send_outbound",
                    serde_json::json!({
                        "task_name": "dingtalk.send_outbound",
                        "trigger": "remote_im_send",
                        "channel_id": channel.id,
                        "remote_contact_id": contact.remote_contact_id,
                        "is_group": is_group,
                        "status": "失败",
                        "error": err,
                        "duration_ms": started.elapsed().as_millis()
                    }),
                );
                return Err(err);
            }
            let process_query_key = body
                .get("processQueryKey")
                .and_then(Value::as_str)
                .or_else(|| {
                    body.get("data")
                        .and_then(|v| v.get("processQueryKey"))
                        .and_then(Value::as_str)
                })
                .unwrap_or_default()
                .to_string();
            remote_im_log(
                "INFO",
                "dingtalk.send_outbound",
                serde_json::json!({
                    "task_name": "dingtalk.send_outbound",
                    "trigger": "remote_im_send",
                    "channel_id": channel.id,
                    "contact_id": contact.id,
                    "remote_contact_id": contact.remote_contact_id,
                    "is_group": is_group,
                    "content_item_count": content_count,
                    "message_length": text.len(),
                    "status": "完成",
                    "process_query_key": process_query_key,
                    "duration_ms": started.elapsed().as_millis()
                }),
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
            let text = remote_im_payload_text(payload);
            if text.trim().is_empty() {
                return Err("onebot v11 outbound text is empty".to_string());
            }

            let manager = napcat_ws_manager();
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
                    "message": text
                })
            } else {
                serde_json::json!({
                    "user_id": contact.remote_contact_id,
                    "message": text
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
                    "status": "开始"
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

fn remote_im_sdk_for_platform(platform: &RemoteImPlatform) -> Box<dyn RemoteImSdk> {
    match platform {
        RemoteImPlatform::Feishu => Box::new(FeishuSdk),
        RemoteImPlatform::Dingtalk => Box::new(DingtalkSdk),
        RemoteImPlatform::OnebotV11 => Box::new(OnebotV11Sdk),
    }
}

async fn remote_im_send_via_sdk(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    payload: &Value,
) -> Result<String, String> {
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
            allow_receive: false,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            activation_cooldown_seconds: 0,
            last_activated_at: None,
            last_message_at: None,
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
