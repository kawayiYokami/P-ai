// ==================== QQ 官方机器人：附件下载 ====================

/// 单个附件下载大小上限（与钉钉渠道保持一致）。
const QQ_OFFICIAL_MAX_DOWNLOAD_BYTES: u64 = 20 * 1024 * 1024;

fn qq_official_attachment_field<'a>(attachment: &'a Value, key: &str) -> &'a str {
    attachment
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
}

fn qq_official_attachment_mime(attachment: &Value, bytes: &[u8], fallback_name: &str) -> String {
    let raw = qq_official_attachment_field(attachment, "content_type");
    if raw.contains('/') {
        return raw.to_string();
    }
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type().to_string();
    }
    mime_from_name_fallback(fallback_name)
}

fn qq_official_attachment_name(attachment: &Value, fallback: &str) -> String {
    let raw = qq_official_attachment_field(attachment, "filename");
    if raw.is_empty() {
        fallback.to_string()
    } else {
        raw.to_string()
    }
}

/// 下载单个入站附件；失败时返回可读错误，由调用方降级为文本提示。
async fn qq_official_download_attachment(
    credentials: &QqOfficialCredentials,
    attachment: &Value,
) -> Result<ChatIngressPart, String> {
    let url = qq_official_attachment_field(attachment, "url");
    if url.is_empty() {
        return Err("附件缺少下载地址".to_string());
    }
    let client = qq_official_build_http_client(credentials)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("下载失败: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("下载失败: HTTP {status}"));
    }
    if let Some(length) = response.content_length() {
        if length > QQ_OFFICIAL_MAX_DOWNLOAD_BYTES {
            return Err(format!("附件超过大小上限 {} 字节", length));
        }
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("读取下载内容失败: {err}"))?;
    if bytes.len() as u64 > QQ_OFFICIAL_MAX_DOWNLOAD_BYTES {
        return Err(format!("附件超过大小上限 {} 字节", bytes.len()));
    }
    let fallback_name = format!("qq-attachment-{}", Uuid::new_v4());
    let name = qq_official_attachment_name(attachment, &fallback_name);
    let mime = qq_official_attachment_mime(attachment, &bytes, &name);
    Ok(ChatIngressPart::Attachment {
        path: None,
        bytes_base64: Some(B64.encode(&bytes)),
        mime,
        name,
    })
}
