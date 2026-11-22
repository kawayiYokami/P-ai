fn mime_from_name_or_hint(file_name: &str, mime_hint: Option<&str>) -> String {
    if let Some(hint) = mime_hint
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
    {
        return hint;
    }
    media_mime_from_path(std::path::Path::new(file_name))
        .unwrap_or("application/octet-stream")
        .to_string()
}

fn onebot_guess_name_from_ref(file_ref: &str, fallback: &str) -> String {
    if let Some(pos) = file_ref.rfind('/') {
        let candidate = file_ref[(pos + 1)..].trim();
        if !candidate.is_empty() && !candidate.contains('?') {
            return candidate.to_string();
        }
    }
    if let Some(pos) = file_ref.rfind('\\') {
        let candidate = file_ref[(pos + 1)..].trim();
        if !candidate.is_empty() {
            return candidate.to_string();
        }
    }
    fallback.to_string()
}

async fn onebot_read_media_bytes(file_ref: &str) -> Result<(Vec<u8>, Option<String>), String> {
    let raw = file_ref.trim();
    if raw.is_empty() {
        return Err("empty file ref".to_string());
    }
    if let Some(b64) = raw.strip_prefix("base64://") {
        let bytes = B64
            .decode(b64.trim())
            .map_err(|err| format!("decode onebot base64 media failed: {err}"))?;
        return Ok((bytes, None));
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return onebot_download_media_by_url(raw, None).await;
    }

    let local_path = if let Some(without_scheme) = raw.strip_prefix("file://") {
        if let Some(win) = without_scheme.strip_prefix('/') {
            std::path::Path::new(win).to_path_buf()
        } else {
            std::path::Path::new(without_scheme).to_path_buf()
        }
    } else {
        std::path::Path::new(raw).to_path_buf()
    };
    if !local_path.is_absolute() {
        return Err(format!(
            "onebot local media ref is relative, unsupported: {}",
            local_path.to_string_lossy()
        ));
    }
    let bytes = tokio::fs::read(&local_path)
        .await
        .map_err(|err| format!("read onebot local media failed: path={}, err={err}", local_path.to_string_lossy()))?;
    Ok((bytes, None))
}

async fn onebot_download_media_by_url(
    url: &str,
    headers: Option<reqwest::header::HeaderMap>,
) -> Result<(Vec<u8>, Option<String>), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("build onebot media download client failed: {err}"))?;
    let mut request = client.get(url);
    if let Some(h) = headers {
        request = request.headers(h);
    }
    let response = request
        .send()
        .await
        .map_err(|err| format!("download onebot media failed: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("download onebot media rejected http {}", status.as_u16()));
    }
    if let Some(content_len) = response.content_length() {
        if content_len > NAPCAT_MAX_MEDIA_DOWNLOAD_SIZE_BYTES {
            return Err(format!(
                "onebot media too large: {} > {}",
                content_len, NAPCAT_MAX_MEDIA_DOWNLOAD_SIZE_BYTES
            ));
        }
    }
    let header_mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let mut stream = response.bytes_stream();
    let mut total = 0u64;
    let mut bytes = Vec::<u8>::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| format!("read onebot media stream failed: {err}"))?;
        total = total.saturating_add(chunk.len() as u64);
        if total > NAPCAT_MAX_MEDIA_DOWNLOAD_SIZE_BYTES {
            return Err(format!(
                "onebot media too large while streaming: {} > {}",
                total, NAPCAT_MAX_MEDIA_DOWNLOAD_SIZE_BYTES
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((bytes, header_mime))
}

fn onebot_extract_file_ref_from_get_file_response(value: &Value) -> Option<String> {
    let direct = value
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| value.get("download_url").and_then(Value::as_str))
        .or_else(|| value.get("path").and_then(Value::as_str))
        .or_else(|| value.get("file").and_then(Value::as_str))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    if direct.is_some() {
        return direct;
    }
    let nested = value.get("data").and_then(Value::as_object)?;
    nested
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| nested.get("download_url").and_then(Value::as_str))
        .or_else(|| nested.get("path").and_then(Value::as_str))
        .or_else(|| nested.get("file").and_then(Value::as_str))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

fn onebot_extract_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .or_else(|| value.get("data").and_then(|v| v.get(key)).and_then(Value::as_str))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

fn onebot_extract_headers_field(value: &Value) -> Option<reqwest::header::HeaderMap> {
    let raw_headers = value
        .get("headers")
        .or_else(|| value.get("data").and_then(|v| v.get("headers")))?;
    let mut out = reqwest::header::HeaderMap::new();
    if let Some(obj) = raw_headers.as_object() {
        for (k, v) in obj {
            let key = k.trim();
            let val = v.as_str().map(str::trim).unwrap_or("");
            if key.is_empty() || val.is_empty() {
                continue;
            }
            let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) else {
                continue;
            };
            let Ok(value) = reqwest::header::HeaderValue::from_str(val) else {
                continue;
            };
            out.insert(name, value);
        }
    } else if let Some(arr) = raw_headers.as_array() {
        for item in arr {
            let Some(pair) = item.as_array() else {
                continue;
            };
            if pair.len() != 2 {
                continue;
            }
            let key = pair[0].as_str().map(str::trim).unwrap_or("");
            let val = pair[1].as_str().map(str::trim).unwrap_or("");
            if key.is_empty() || val.is_empty() {
                continue;
            }
            let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) else {
                continue;
            };
            let Ok(value) = reqwest::header::HeaderValue::from_str(val) else {
                continue;
            };
            out.insert(name, value);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

async fn onebot_get_file_with_type(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    file_id: &str,
    group_id: Option<u64>,
    user_id: Option<u64>,
) -> Result<(Vec<u8>, Option<String>), String> {
    let file_id = file_id.trim();
    if file_id.is_empty() {
        return Err("onebot file_id is empty".to_string());
    }

    let try_url = onebot_call_action_try_params(
        manager,
        channel_id,
        "get_file",
        &[
            serde_json::json!({ "file_id": file_id, "type": "url" }),
            serde_json::json!({ "file": file_id, "type": "url" }),
            serde_json::json!({ "id": file_id, "type": "url" }),
            serde_json::json!({ "file_id": file_id }),
            serde_json::json!({ "file": file_id }),
        ],
    )
    .await;
    if let Ok(value) = try_url {
        if let Some(url) = onebot_extract_string_field(&value, "url")
            .or_else(|| onebot_extract_string_field(&value, "download_url"))
            .or_else(|| onebot_extract_file_ref_from_get_file_response(&value))
        {
            let headers = onebot_extract_headers_field(&value);
            if let Ok(ok) = onebot_download_media_by_url(&url, headers).await {
                return Ok(ok);
            }
        }
    }

    let try_path = onebot_call_action_try_params(
        manager,
        channel_id,
        "get_file",
        &[
            serde_json::json!({ "file_id": file_id, "type": "path" }),
            serde_json::json!({ "file": file_id, "type": "path" }),
            serde_json::json!({ "id": file_id, "type": "path" }),
        ],
    )
    .await;
    if let Ok(value) = try_path {
        if let Some(path) = onebot_extract_string_field(&value, "path")
            .or_else(|| onebot_extract_string_field(&value, "file"))
            .or_else(|| onebot_extract_file_ref_from_get_file_response(&value))
        {
            if let Ok(ok) = onebot_read_media_bytes(&path).await {
                return Ok(ok);
            }
        }
    }

    let try_data = onebot_call_action_try_params(
        manager,
        channel_id,
        "get_file",
        &[
            serde_json::json!({ "file_id": file_id, "type": "data" }),
            serde_json::json!({ "file": file_id, "type": "data" }),
            serde_json::json!({ "id": file_id, "type": "data" }),
        ],
    )
    .await;
    if let Ok(value) = try_data {
        if let Some(data_raw) = onebot_extract_string_field(&value, "data")
            .or_else(|| onebot_extract_string_field(&value, "base64"))
        {
            let data_raw = data_raw.trim();
            let data_raw = data_raw.strip_prefix("base64://").unwrap_or(data_raw);
            let bytes = B64
                .decode(data_raw)
                .map_err(|err| format!("decode onebot get_file(data) base64 failed: {err}"))?;
            let mime = onebot_extract_string_field(&value, "mime");
            return Ok((bytes, mime));
        }
    }

    if let Some(gid) = group_id {
        let try_group_url = onebot_call_action_try_params(
            manager,
            channel_id,
            "get_group_file_url",
            &[serde_json::json!({ "group_id": gid, "file_id": file_id })],
        )
        .await;
        if let Ok(value) = try_group_url {
            if let Some(url) = onebot_extract_string_field(&value, "url")
                .or_else(|| onebot_extract_string_field(&value, "download_url"))
                .or_else(|| onebot_extract_file_ref_from_get_file_response(&value))
            {
                let headers = onebot_extract_headers_field(&value);
                if let Ok(ok) = onebot_download_media_by_url(&url, headers).await {
                    return Ok(ok);
                }
            }
        }
    }

    if let Some(uid) = user_id {
        let try_private_url = onebot_call_action_try_params(
            manager,
            channel_id,
            "get_private_file_url",
            &[
                serde_json::json!({ "user_id": uid, "file_id": file_id }),
                serde_json::json!({ "file_id": file_id }),
            ],
        )
        .await;
        if let Ok(value) = try_private_url {
            if let Some(url) = onebot_extract_string_field(&value, "url")
                .or_else(|| onebot_extract_string_field(&value, "download_url"))
                .or_else(|| onebot_extract_file_ref_from_get_file_response(&value))
            {
                let headers = onebot_extract_headers_field(&value);
                if let Ok(ok) = onebot_download_media_by_url(&url, headers).await {
                    return Ok(ok);
                }
            }
        }
    }

    Err(format!(
        "onebot get_file unresolved for file_id={}",
        file_id
    ))
}

async fn onebot_resolve_inbound_media(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    group_id: Option<u64>,
    user_id: Option<u64>,
    media_refs: &[OnebotInboundMediaRef],
) -> (Vec<ChatIngressPart>, Vec<String>) {
    let mut parts = Vec::<ChatIngressPart>::new();
    let mut notices = Vec::<String>::new();
    for (idx, item) in media_refs.iter().enumerate() {
        let fallback_name = match item.kind {
            OnebotInboundMediaKind::Image => format!("onebot-image-{}.png", idx + 1),
            OnebotInboundMediaKind::File => format!("onebot-file-{}", idx + 1),
        };
        let file_name = item
            .file_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| onebot_guess_name_from_ref(&item.file_ref, &fallback_name));
        let mut refs_to_try = Vec::<String>::new();
        refs_to_try.push(item.file_ref.clone());
        let mut resolved_raw: Option<Vec<u8>> = None;
        let mut resolved_mime: Option<String> = None;
        let mut last_err = String::new();
        for file_ref in refs_to_try {
            match onebot_read_media_bytes(&file_ref).await {
                Ok((raw, mime)) => {
                    resolved_raw = Some(raw);
                    resolved_mime = mime;
                    last_err.clear();
                    break;
                }
                Err(err) => {
                    last_err = err;
                }
            }
        }
        if resolved_raw.is_none() {
            if let Some(file_id) = item.file_id.as_deref() {
                match onebot_get_file_with_type(manager, channel_id, file_id, group_id, user_id).await {
                    Ok((raw, mime)) => {
                        resolved_raw = Some(raw);
                        resolved_mime = mime;
                        last_err.clear();
                    }
                    Err(err) => {
                        last_err = err;
                    }
                }
            }
        }
        let Some(raw) = resolved_raw else {
            runtime_log_warn(format!(
                "[远程IM] 媒体下载失败，已跳过 类型={} 引用={} 文件={} 原因={}",
                item.kind.as_str(),
                item.file_ref,
                item.file_id.clone().unwrap_or_default(),
                last_err
            ));
            notices.push(format!(
                "[附件不可用：{} 下载失败，已跳过该附件并继续处理消息]",
                file_name
            ));
            continue;
        };
        let hint = resolved_mime.as_deref().or(item.mime_hint.as_deref());
        let mime = if matches!(item.kind, OnebotInboundMediaKind::Image) {
            image_mime_from_bytes(&raw)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| mime_from_name_or_hint(&file_name, hint))
        } else {
            mime_from_name_or_hint(&file_name, hint)
        };

        parts.push(ChatIngressPart::Attachment {
            path: None,
            bytes_base64: Some(B64.encode(raw)),
            mime,
            name: file_name,
        });
    }
    (parts, notices)
}
