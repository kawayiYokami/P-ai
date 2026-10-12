// ==================== QQ 官方机器人：出站发送 ====================

/// 富媒体业务类型（官方 `file_type`）。
const QQ_OFFICIAL_UPLOAD_TYPE_IMAGE: i64 = 1;
const QQ_OFFICIAL_UPLOAD_TYPE_VIDEO: i64 = 2;
const QQ_OFFICIAL_UPLOAD_TYPE_VOICE: i64 = 3;
const QQ_OFFICIAL_UPLOAD_TYPE_FILE: i64 = 4;
/// `md5_10m` 取样长度（官方约定：文件前 10002432 字节）。
const QQ_OFFICIAL_MD5_10M_BYTES: usize = 10_002_432;

/// 发送失败的分类：仅客户端错误（4xx）可安全降级为主动消息重试，传输层错误可能是「已送达但响应失败」，重复发送会变成重复投递。
#[derive(Debug, Clone)]
struct QqOfficialPostError {
    message: String,
    client_error: bool,
}

/// 单条消息的发送结果：`passive_used` 标明本次是否真正走了被动回复。
#[derive(Debug, Clone)]
struct QqOfficialSendOutcome {
    message_id: String,
    passive_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QqOfficialSendScope {
    C2c,
    Group,
}

impl QqOfficialSendScope {
    fn path_segment(&self) -> &'static str {
        match self {
            QqOfficialSendScope::C2c => "users",
            QqOfficialSendScope::Group => "groups",
        }
    }

    fn passive_window_secs(&self) -> i64 {
        match self {
            QqOfficialSendScope::C2c => QQ_OFFICIAL_C2C_PASSIVE_WINDOW_SECS,
            QqOfficialSendScope::Group => QQ_OFFICIAL_GROUP_PASSIVE_WINDOW_SECS,
        }
    }

    fn passive_max_replies(&self) -> u32 {
        match self {
            QqOfficialSendScope::C2c => QQ_OFFICIAL_C2C_PASSIVE_MAX_REPLIES,
            QqOfficialSendScope::Group => QQ_OFFICIAL_GROUP_PASSIVE_MAX_REPLIES,
        }
    }
}

fn qq_official_scope_for_contact_type(contact_type: &str) -> Option<QqOfficialSendScope> {
    match contact_type.trim().to_ascii_lowercase().as_str() {
        "group" => Some(QqOfficialSendScope::Group),
        "private" => Some(QqOfficialSendScope::C2c),
        _ => None,
    }
}

fn qq_official_contact_scope(contact: &RemoteImContact) -> Option<QqOfficialSendScope> {
    qq_official_scope_for_contact_type(&contact.remote_contact_type)
}

fn qq_official_openid_path(
    scope: QqOfficialSendScope,
    openid: &str,
    suffix: &str,
) -> String {
    format!("/v2/{}/{}/{}", scope.path_segment(), openid, suffix)
}

fn qq_official_sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest as _;
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn qq_official_md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", md5::compute(bytes))
}

/// 依据内容项推断官方的富媒体业务类型；不支持的图片格式（如 gif）降级为文件发送。
fn qq_official_upload_type_for_item(name: &str, mime: &str) -> i64 {
    let mime_lower = mime.trim().to_ascii_lowercase();
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let is_png = mime_lower == "image/png" || ext == "png";
    let is_jpeg = mime_lower == "image/jpeg" || mime_lower == "image/jpg" || ext == "jpg" || ext == "jpeg";
    if is_png || is_jpeg {
        return QQ_OFFICIAL_UPLOAD_TYPE_IMAGE;
    }
    if mime_lower == "video/mp4" || ext == "mp4" {
        return QQ_OFFICIAL_UPLOAD_TYPE_VIDEO;
    }
    if mime_lower == "audio/x-silk" || mime_lower == "audio/silk" || ext == "silk" {
        return QQ_OFFICIAL_UPLOAD_TYPE_VOICE;
    }
    QQ_OFFICIAL_UPLOAD_TYPE_FILE
}

/// 被动回复配额：窗口与次数都在限制内时才可用。
#[derive(Debug, Clone)]
struct QqOfficialPassiveSlot {
    msg_id: String,
    next_seq: i64,
    remaining: u32,
}

fn qq_official_passive_slot(
    scope: QqOfficialSendScope,
    context: Option<&QqOfficialPassiveContext>,
) -> Option<QqOfficialPassiveSlot> {
    let context = context?;
    let age = chrono::Utc::now() - context.received_at;
    if age >= chrono::Duration::seconds(scope.passive_window_secs()) {
        return None;
    }
    let remaining = scope
        .passive_max_replies()
        .saturating_sub(context.reply_count);
    if remaining == 0 {
        return None;
    }
    Some(QqOfficialPassiveSlot {
        msg_id: context.msg_id.clone(),
        next_seq: i64::from(context.reply_count) + 1,
        remaining,
    })
}

/// 发送一条消息体；被动回复失败时（窗口超时 / 次数超限 / 越权）自动降级为主动消息重试。
async fn qq_official_post_message_body(
    credentials: &QqOfficialCredentials,
    access_token: &str,
    scope: QqOfficialSendScope,
    openid: &str,
    body: &Value,
    passive: Option<(&str, i64)>,
) -> Result<String, QqOfficialPostError> {
    let client = qq_official_build_http_client(credentials).map_err(|err| QqOfficialPostError {
        message: err,
        client_error: false,
    })?;
    let url = format!(
        "{}{}",
        credentials.api_base_url(),
        qq_official_openid_path(scope, openid, "messages")
    );
    let mut request_body = body.clone();
    if let Some((msg_id, msg_seq)) = passive {
        if let Some(object) = request_body.as_object_mut() {
            object.insert("msg_id".to_string(), Value::String(msg_id.to_string()));
            object.insert("msg_seq".to_string(), Value::Number(msg_seq.into()));
        }
    }
    let response = client
        .post(url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("QQBot {access_token}"),
        )
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )
        .json(&request_body)
        .send()
        .await
        .map_err(|err| QqOfficialPostError {
            message: format!("发送消息请求失败: {err}"),
            client_error: false,
        })?;
    let status = response.status();
    let response_body = response.json::<Value>().await.map_err(|err| QqOfficialPostError {
        message: format!("解析发送响应失败: {err}"),
        client_error: false,
    })?;
    if !status.is_success() {
        return Err(QqOfficialPostError {
            message: format!("发送消息失败: HTTP {status}, body={response_body}"),
            client_error: status.is_client_error(),
        });
    }
    Ok(response_body
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

async fn qq_official_send_text(
    credentials: &QqOfficialCredentials,
    access_token: &str,
    scope: QqOfficialSendScope,
    openid: &str,
    text: &str,
    passive: Option<(&str, i64)>,
) -> Result<QqOfficialSendOutcome, String> {
    let body = serde_json::json!({
        "msg_type": QQ_OFFICIAL_MSG_TYPE_TEXT,
        "content": text,
    });
    match qq_official_post_message_body(credentials, access_token, scope, openid, &body, passive)
        .await
    {
        Ok(message_id) => Ok(QqOfficialSendOutcome {
            message_id,
            passive_used: passive.is_some(),
        }),
        Err(err) if passive.is_some() && err.client_error => {
            runtime_log_warn(format!(
                "[远程IM][QQ官方机器人] 被动回复被拒绝，降级为主动消息重试: {}",
                err.message
            ));
            qq_official_post_message_body(credentials, access_token, scope, openid, &body, None)
                .await
                .map(|message_id| QqOfficialSendOutcome {
                    message_id,
                    passive_used: false,
                })
                .map_err(|fallback| format!("{}; 降级主动消息失败: {}", err.message, fallback.message))
        }
        Err(err) => Err(err.message),
    }
}

/// 分片上传本地字节并返回 `file_info`（图片 / 视频 / 语音 / 文件共用一条链路）。
async fn qq_official_upload_media(
    credentials: &QqOfficialCredentials,
    access_token: &str,
    scope: QqOfficialSendScope,
    openid: &str,
    file_type: i64,
    file_name: &str,
    bytes: &[u8],
) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("上传内容为空".to_string());
    }
    let client = qq_official_build_http_client(credentials)?;
    let authorization = format!("QQBot {access_token}");
    let md5_10m_len = bytes.len().min(QQ_OFFICIAL_MD5_10M_BYTES);
    let prepare_body = serde_json::json!({
        "file_type": file_type,
        "file_size": bytes.len().to_string(),
        "file_name": file_name,
        "md5": qq_official_md5_hex(bytes),
        "sha1": qq_official_sha1_hex(bytes),
        "md5_10m": qq_official_md5_hex(&bytes[..md5_10m_len]),
    });
    let prepare_url = format!(
        "{}{}",
        credentials.api_base_url(),
        qq_official_openid_path(scope, openid, "upload_prepare")
    );
    let prepared = client
        .post(prepare_url)
        .header(reqwest::header::AUTHORIZATION, authorization.clone())
        .json(&prepare_body)
        .send()
        .await
        .map_err(|err| format!("富媒体预上传请求失败: {err}"))?;
    let status = prepared.status();
    let prepared = prepared
        .json::<Value>()
        .await
        .map_err(|err| format!("解析富媒体预上传响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!("富媒体预上传失败: HTTP {status}, body={prepared}"));
    }
    let upload_id = prepared
        .get("upload_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("富媒体预上传响应缺少 upload_id: {prepared}"))?
        .to_string();
    let default_block_size = prepared
        .get("block_size")
        .and_then(qq_official_json_i64)
        .filter(|value| *value > 0)
        .unwrap_or(5 * 1024 * 1024) as usize;
    let parts = prepared
        .get("parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut offset = 0usize;
    for part in &parts {
        if offset >= bytes.len() {
            break;
        }
        let index = part.get("index").and_then(qq_official_json_i64).unwrap_or(0);
        let presigned_url = part
            .get("presigned_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if presigned_url.is_empty() {
            return Err(format!("分片 {index} 缺少预签名上传地址"));
        }
        let part_size = part
            .get("block_size")
            .and_then(qq_official_json_i64)
            .filter(|value| *value > 0)
            .map(|value| value as usize)
            .unwrap_or(default_block_size)
            .max(1);
        let end = offset.saturating_add(part_size).min(bytes.len());
        let chunk = &bytes[offset..end];
        let upload = client
            .put(presigned_url)
            .body(chunk.to_vec())
            .send()
            .await
            .map_err(|err| format!("上传分片 {index} 失败: {err}"))?;
        if !upload.status().is_success() {
            return Err(format!("上传分片 {index} 失败: HTTP {}", upload.status()));
        }
        let finish_body = serde_json::json!({
            "upload_id": upload_id,
            "part_index": index,
            "block_size": chunk.len().to_string(),
            "md5": qq_official_md5_hex(chunk),
        });
        let finish_url = format!(
            "{}{}",
            credentials.api_base_url(),
            qq_official_openid_path(scope, openid, "upload_part_finish")
        );
        let finish = client
            .post(finish_url)
            .header(reqwest::header::AUTHORIZATION, authorization.clone())
            .json(&finish_body)
            .send()
            .await
            .map_err(|err| format!("通知分片 {index} 完成失败: {err}"))?;
        if !finish.status().is_success() {
            let status = finish.status();
            let body = finish.text().await.unwrap_or_default();
            return Err(format!("通知分片 {index} 完成失败: HTTP {status}, body={body}"));
        }
        offset = end;
    }

    let merge_body = serde_json::json!({
        "file_type": file_type,
        "srv_send_msg": false,
        "file_name": file_name,
        "upload_id": upload_id,
    });
    let merge_url = format!(
        "{}{}",
        credentials.api_base_url(),
        qq_official_openid_path(scope, openid, "files")
    );
    let merged = client
        .post(merge_url)
        .header(reqwest::header::AUTHORIZATION, authorization)
        .json(&merge_body)
        .send()
        .await
        .map_err(|err| format!("合并富媒体上传请求失败: {err}"))?;
    let status = merged.status();
    let merged = merged
        .json::<Value>()
        .await
        .map_err(|err| format!("解析富媒体上传响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!("富媒体上传失败: HTTP {status}, body={merged}"));
    }
    merged
        .get("file_info")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("富媒体上传响应缺少 file_info: {merged}"))
}

async fn qq_official_send_media(
    credentials: &QqOfficialCredentials,
    access_token: &str,
    scope: QqOfficialSendScope,
    openid: &str,
    file_type: i64,
    file_name: &str,
    bytes: &[u8],
    passive: Option<(&str, i64)>,
) -> Result<QqOfficialSendOutcome, String> {
    let file_info = qq_official_upload_media(
        credentials,
        access_token,
        scope,
        openid,
        file_type,
        file_name,
        bytes,
    )
    .await?;
    let body = serde_json::json!({
        "msg_type": QQ_OFFICIAL_MSG_TYPE_MEDIA,
        "media": { "file_info": file_info },
    });
    match qq_official_post_message_body(credentials, access_token, scope, openid, &body, passive)
        .await
    {
        Ok(message_id) => Ok(QqOfficialSendOutcome {
            message_id,
            passive_used: passive.is_some(),
        }),
        Err(err) if passive.is_some() && err.client_error => {
            runtime_log_warn(format!(
                "[远程IM][QQ官方机器人] 富媒体被动回复被拒绝，降级为主动消息重试: {}",
                err.message
            ));
            qq_official_post_message_body(credentials, access_token, scope, openid, &body, None)
                .await
                .map(|message_id| QqOfficialSendOutcome {
                    message_id,
                    passive_used: false,
                })
                .map_err(|fallback| format!("{}; 降级主动消息失败: {}", err.message, fallback.message))
        }
        Err(err) => Err(err.message),
    }
}

/// 渠道出站入口：文本与富媒体分段发送，每条消息尽量使用被动回复配额。
async fn qq_official_send_outbound(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    payload: &Value,
) -> Result<String, RemoteImSdkSendError> {
    let started = std::time::Instant::now();
    let channel_id = channel.id.trim().to_string();
    let content_count = remote_im_payload_content_items(payload).len();
    remote_im_log(
        "INFO",
        "qq_official.send_outbound",
        serde_json::json!({
            "task_name": "qq_official.send_outbound",
            "trigger": "remote_im_outbound",
            "channel_id": channel_id,
            "remote_contact_id": contact.remote_contact_id,
            "status": "开始",
            "content_count": content_count,
            "payload_summary": remote_im_payload_media_summary(payload),
        }),
    );
    let result = qq_official_send_outbound_inner(channel, contact, payload).await;
    match &result {
        Ok(message_id) => remote_im_log(
            "INFO",
            "qq_official.send_outbound",
            serde_json::json!({
                "task_name": "qq_official.send_outbound",
                "trigger": "remote_im_outbound",
                "channel_id": channel_id,
                "remote_contact_id": contact.remote_contact_id,
                "status": "完成",
                "message_id": message_id,
                "duration_ms": started.elapsed().as_millis(),
            }),
        ),
        Err(err) => remote_im_log(
            "ERROR",
            "qq_official.send_outbound",
            serde_json::json!({
                "task_name": "qq_official.send_outbound",
                "trigger": "remote_im_outbound",
                "channel_id": channel_id,
                "remote_contact_id": contact.remote_contact_id,
                "status": "失败",
                "duration_ms": started.elapsed().as_millis(),
                "error": &err.message,
            }),
        ),
    }
    result
}

async fn qq_official_send_outbound_inner(
    channel: &RemoteImChannelConfig,
    contact: &RemoteImContact,
    payload: &Value,
) -> Result<String, RemoteImSdkSendError> {
    let credentials = QqOfficialCredentials::from_value(&channel.credentials);
    let scope = qq_official_contact_scope(contact)
        .ok_or_else(|| RemoteImSdkSendError::definitely_not_sent("QQ 官方机器人渠道不支持的联系人类型"))?;
    let openid = contact.remote_contact_id.trim().to_string();
    if openid.is_empty() {
        return Err(RemoteImSdkSendError::definitely_not_sent(
            "QQ 官方机器人渠道联系人标识为空",
        ));
    }
    let items = remote_im_payload_content_items(payload);
    if items.is_empty() {
        return Err(RemoteImSdkSendError::definitely_not_sent(
            "QQ 官方机器人渠道发送内容为空",
        ));
    }

    let manager = qq_official_manager();
    let access_token = manager
        .access_token(channel)
        .await
        .map_err(RemoteImSdkSendError::definitely_not_sent)?;
    let passive_context = manager
        .passive_context(&channel.id, &contact.remote_contact_id)
        .await;
    let mut passive_slot = qq_official_passive_slot(scope, passive_context.as_ref());

    let mut last_message_id = String::new();
    let mut delivered_any = false;
    let mut pending_text = String::new();

    for item in &items {
        match item.get("type").and_then(Value::as_str).unwrap_or("") {
            "text" => {
                let Some(text) = item.get("text").and_then(Value::as_str) else {
                    continue;
                };
                if text.trim().is_empty() {
                    continue;
                }
                pending_text.push_str(text);
            }
            "image" | "file" => {
                if !pending_text.trim().is_empty() {
                    let text = std::mem::take(&mut pending_text);
                    qq_official_flush_text(
                        &credentials,
                        &access_token,
                        scope,
                        &openid,
                        &text,
                        &mut passive_slot,
                        &manager,
                        &channel.id,
                        &contact.remote_contact_id,
                        delivered_any,
                    )
                    .await?;
                    delivered_any = true;
                }
                let is_image = item.get("type").and_then(Value::as_str).unwrap_or("") == "image";
                let name = remote_im_content_item_name(
                    item,
                    if is_image { "image.png" } else { "file" },
                );
                let mime = remote_im_content_item_mime(
                    item,
                    if is_image { "image/png" } else { "application/octet-stream" },
                );
                let bytes = remote_im_content_item_bytes(item)
                    .await
                    .map_err(RemoteImSdkSendError::definitely_not_sent)
                    .map_err(|err| err.after_confirmed_partial_delivery(delivered_any))?;
                let file_type = qq_official_upload_type_for_item(&name, &mime);
                let passive = qq_official_peek_passive(&passive_slot);
                let outcome = qq_official_send_media(
                    &credentials,
                    &access_token,
                    scope,
                    &openid,
                    file_type,
                    &name,
                    &bytes,
                    passive.as_ref().map(|(msg_id, seq)| (msg_id.as_str(), *seq)),
                )
                .await
                .map_err(RemoteImSdkSendError::definitely_not_sent)
                .map_err(|err| err.after_confirmed_partial_delivery(delivered_any))?;
                if outcome.passive_used {
                    qq_official_commit_passive(&mut passive_slot);
                    manager
                        .mark_passive_reply_sent(&channel.id, &contact.remote_contact_id)
                        .await;
                }
                last_message_id = outcome.message_id;
                delivered_any = true;
            }
            _ => {}
        }
    }

    if !pending_text.trim().is_empty() {
        let text = std::mem::take(&mut pending_text);
        last_message_id = qq_official_flush_text(
            &credentials,
            &access_token,
            scope,
            &openid,
            &text,
            &mut passive_slot,
            &manager,
            &channel.id,
            &contact.remote_contact_id,
            delivered_any,
        )
        .await?;
        delivered_any = true;
    }

    if !delivered_any {
        return Err(RemoteImSdkSendError::definitely_not_sent(
            "QQ 官方机器人渠道发送内容为空",
        ));
    }
    Ok(last_message_id)
}

/// 预览一次可用的被动回复配额（不消耗，发送成功后再提交）。
fn qq_official_peek_passive(slot: &Option<QqOfficialPassiveSlot>) -> Option<(String, i64)> {
    let slot = slot.as_ref()?;
    if slot.remaining == 0 {
        return None;
    }
    Some((slot.msg_id.clone(), slot.next_seq))
}

/// 被动回复确实发送成功后消耗一次配额。
fn qq_official_commit_passive(slot: &mut Option<QqOfficialPassiveSlot>) {
    if let Some(slot) = slot.as_mut() {
        if slot.remaining > 0 {
            slot.next_seq += 1;
            slot.remaining -= 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn qq_official_flush_text(
    credentials: &QqOfficialCredentials,
    access_token: &str,
    scope: QqOfficialSendScope,
    openid: &str,
    text: &str,
    passive_slot: &mut Option<QqOfficialPassiveSlot>,
    manager: &std::sync::Arc<QqOfficialManager>,
    channel_id: &str,
    contact_id: &str,
    delivered_any: bool,
) -> Result<String, RemoteImSdkSendError> {
    let passive = qq_official_peek_passive(passive_slot);
    let outcome = qq_official_send_text(
        credentials,
        access_token,
        scope,
        openid,
        text,
        passive.as_ref().map(|(msg_id, seq)| (msg_id.as_str(), *seq)),
    )
    .await
    .map_err(RemoteImSdkSendError::definitely_not_sent)
    .map_err(|err| err.after_confirmed_partial_delivery(delivered_any))?;
    if outcome.passive_used {
        qq_official_commit_passive(passive_slot);
        manager
            .mark_passive_reply_sent(channel_id, contact_id)
            .await;
    }
    Ok(outcome.message_id)
}

/// 发送「输入中」状态；走主动消息路径，避免占用被动回复配额。
async fn qq_official_send_input_notify(
    channel: &RemoteImChannelConfig,
    contact_type: &str,
    contact_id: &str,
) -> Result<(), String> {
    let credentials = QqOfficialCredentials::from_value(&channel.credentials);
    let scope = qq_official_scope_for_contact_type(contact_type)
        .ok_or_else(|| "不支持的 QQ 官方机器人联系人类型".to_string())?;
    let openid = contact_id.trim();
    if openid.is_empty() {
        return Err("联系人标识为空".to_string());
    }
    let access_token = qq_official_manager().access_token(channel).await?;
    let body = serde_json::json!({
        "msg_type": QQ_OFFICIAL_MSG_TYPE_INPUT_NOTIFY,
        "input_notify": {
            "input_type": 1,
            "input_second": QQ_OFFICIAL_INPUT_NOTIFY_SECONDS,
        },
    });
    qq_official_post_message_body(&credentials, &access_token, scope, openid, &body, None)
        .await
        .map_err(|err| err.message)?;
    Ok(())
}
