const ALIYUN_MULTIMODAL_CACHE_META_KEY: &str = "aliyunMultimodalCache";
const ALIYUN_MULTIMODAL_CACHE_ITEM_KIND_IMAGE: &str = "image";
const ALIYUN_MULTIMODAL_CACHE_ITEM_KIND_AUDIO: &str = "audio";
const ALIYUN_OSS_RESOLVE_HEADER_KEY: &str = "X-DashScope-OssResourceResolve";
const ALIYUN_OSS_RESOLVE_HEADER_VALUE: &str = "enable";
const ALIYUN_TEMP_URL_TTL_MS: i64 = 48 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AliyunMultimodalCacheEnvelope {
    #[serde(default)]
    items: Vec<AliyunMultimodalCacheItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AliyunMultimodalCacheItem {
    kind: String,
    mime: String,
    content_hash: String,
    model: String,
    url: String,
    expires_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AliyunUploadPolicyResponse {
    data: AliyunUploadPolicy,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AliyunUploadPolicy {
    upload_host: String,
    upload_dir: String,
    oss_access_key_id: String,
    signature: String,
    policy: String,
    x_oss_object_acl: String,
    x_oss_forbid_overwrite: String,
}

fn is_aliyun_dashscope_base_url(base_url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(base_url.trim()) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    host.to_ascii_lowercase().contains("aliyuncs")
}

fn aliyun_multimodal_uploads_url(base_url: &str) -> Result<String, String> {
    let mut parsed =
        reqwest::Url::parse(base_url.trim()).map_err(|err| format!("解析百炼 base_url 失败: {err}"))?;
    parsed.set_path("/api/v1/uploads");
    parsed.set_query(None);
    Ok(parsed.to_string())
}

fn is_remote_binary_url(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("oss://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("data:")
}

fn aliyun_multimodal_kind_for_mime(mime: &str) -> Option<&'static str> {
    let lower = mime.trim().to_ascii_lowercase();
    if lower.starts_with("image/") {
        Some(ALIYUN_MULTIMODAL_CACHE_ITEM_KIND_IMAGE)
    } else if lower.starts_with("audio/") {
        Some(ALIYUN_MULTIMODAL_CACHE_ITEM_KIND_AUDIO)
    } else {
        None
    }
}

fn aliyun_multimodal_supports_urlization(request_format: RequestFormat, mime: &str) -> bool {
    match aliyun_multimodal_kind_for_mime(mime) {
        Some(ALIYUN_MULTIMODAL_CACHE_ITEM_KIND_IMAGE) => true,
        Some(ALIYUN_MULTIMODAL_CACHE_ITEM_KIND_AUDIO) => {
            request_format.is_openai_responses_family()
        }
        _ => false,
    }
}

fn media_hash_from_raw(raw: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest as _;
    hasher.update(&raw);
    format!("sha256:{:x}", hasher.finalize())
}

fn media_hash_from_base64(base64_value: &str) -> Result<String, String> {
    let raw = B64
        .decode(base64_value.trim())
        .map_err(|err| format!("解析媒体 base64 失败: {err}"))?;
    Ok(media_hash_from_raw(&raw))
}

fn aliyun_multimodal_cache_lookup_key(kind: &str, model: &str, content_hash: &str) -> String {
    format!(
        "{}|{}|{}",
        kind.trim().to_ascii_lowercase(),
        model.trim(),
        content_hash.trim()
    )
}

fn aliyun_multimodal_cache_item_is_fresh(item: &AliyunMultimodalCacheItem, now_ms: i64) -> bool {
    !item.url.trim().is_empty()
        && item.expires_at_ms > now_ms
        && !item.model.trim().is_empty()
        && !item.content_hash.trim().is_empty()
}

fn aliyun_multimodal_cache_items_from_meta(
    provider_meta: Option<&Value>,
) -> Vec<AliyunMultimodalCacheItem> {
    provider_meta
        .and_then(|meta| meta.get(ALIYUN_MULTIMODAL_CACHE_META_KEY))
        .cloned()
        .and_then(|value| serde_json::from_value::<AliyunMultimodalCacheEnvelope>(value).ok())
        .map(|envelope| envelope.items)
        .unwrap_or_default()
}

fn upsert_aliyun_multimodal_cache_item(
    provider_meta: &mut Option<Value>,
    next_item: AliyunMultimodalCacheItem,
) {
    let mut meta = provider_meta
        .take()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    let mut envelope = aliyun_multimodal_cache_items_from_meta(Some(&meta));
    envelope.retain(|item| {
        !item.kind.eq_ignore_ascii_case(&next_item.kind)
            || item.model.trim() != next_item.model.trim()
            || item.content_hash.trim() != next_item.content_hash.trim()
    });
    envelope.push(next_item);
    if let Some(obj) = meta.as_object_mut() {
        obj.insert(
            ALIYUN_MULTIMODAL_CACHE_META_KEY.to_string(),
            serde_json::to_value(AliyunMultimodalCacheEnvelope { items: envelope })
                .unwrap_or_else(|_| serde_json::json!({ "items": [] })),
        );
    }
    *provider_meta = Some(meta);
}

fn upsert_api_extra_header(api_config: &mut ResolvedApiConfig, key: &str, value: &str) {
    api_config
        .extra_headers
        .retain(|(existing_key, _)| !existing_key.eq_ignore_ascii_case(key));
    api_config
        .extra_headers
        .push((key.to_string(), value.to_string()));
}

fn aliyun_media_extension_from_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/heic" => "heic",
        "image/heif" => "heif",
        "image/svg+xml" => "svg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "audio/flac" => "flac",
        _ => "bin",
    }
}

fn resolve_message_part_binary_base64_for_hash(
    data_path: &PathBuf,
    bytes_base64: &str,
) -> Result<String, String> {
    if media_id_from_marker(bytes_base64).is_some() {
        resolve_stored_binary_base64(data_path, bytes_base64)
    } else {
        Ok(bytes_base64.trim().to_string())
    }
}

fn collect_aliyun_multimodal_cache_lookup(
    conversation: &Conversation,
) -> std::collections::HashMap<String, AliyunMultimodalCacheItem> {
    let mut lookup = std::collections::HashMap::<String, AliyunMultimodalCacheItem>::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    for message in &conversation.messages {
        for item in aliyun_multimodal_cache_items_from_meta(message.provider_meta.as_ref()) {
            if !aliyun_multimodal_cache_item_is_fresh(&item, now_ms) {
                continue;
            }
            lookup.insert(
                aliyun_multimodal_cache_lookup_key(
                    &item.kind,
                    &item.model,
                    &item.content_hash,
                ),
                item,
            );
        }
    }
    lookup
}

async fn get_aliyun_upload_policy(
    state: &AppState,
    api_key: &str,
    base_url: &str,
    model_name: &str,
) -> Result<AliyunUploadPolicy, String> {
    let url = aliyun_multimodal_uploads_url(base_url)?;
    let response = state
        .shared_http_client
        .get(url)
        .bearer_auth(api_key.trim())
        .query(&[("action", "getPolicy"), ("model", model_name.trim())])
        .send()
        .await
        .map_err(|err| format!("获取百炼上传凭证失败: {err}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("读取百炼上传凭证响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "获取百炼上传凭证失败: {} {}",
            status,
            body.trim()
        ));
    }
    serde_json::from_str::<AliyunUploadPolicyResponse>(&body)
        .map(|value| value.data)
        .map_err(|err| format!("解析百炼上传凭证失败: {err}"))
}

async fn upload_media_to_aliyun_temp_url(
    state: &AppState,
    api_key: &str,
    base_url: &str,
    model_name: &str,
    mime: &str,
    content_hash: &str,
    raw: Vec<u8>,
) -> Result<String, String> {
    let policy = get_aliyun_upload_policy(state, api_key, base_url, model_name).await?;
    let ext = aliyun_media_extension_from_mime(mime);
    let safe_hash = content_hash
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(content_hash.trim());
    let file_name = format!("{safe_hash}.{ext}");
    let object_key = format!("{}/{}", policy.upload_dir.trim_end_matches('/'), file_name);
    let file_part = reqwest::multipart::Part::bytes(raw)
        .file_name(file_name)
        .mime_str(mime.trim())
        .map_err(|err| format!("构造百炼上传文件分片失败: {err}"))?;
    let form = reqwest::multipart::Form::new()
        .text("OSSAccessKeyId", policy.oss_access_key_id)
        .text("Signature", policy.signature)
        .text("policy", policy.policy)
        .text("x-oss-object-acl", policy.x_oss_object_acl)
        .text("x-oss-forbid-overwrite", policy.x_oss_forbid_overwrite)
        .text("key", object_key.clone())
        .text("success_action_status", "200")
        .part("file", file_part);
    let response = state
        .shared_http_client
        .post(policy.upload_host)
        .multipart(form)
        .send()
        .await
        .map_err(|err| format!("上传媒体到百炼临时存储失败: {err}"))?;
    let status = response.status();
    let body = match response.text().await {
        Ok(text) => text,
        Err(err) => {
            if !status.is_success() {
                return Err(format!(
                    "上传媒体到百炼临时存储失败: {} 读取响应体失败: {}",
                    status, err
                ));
            }
            return Err(format!("读取百炼上传响应失败: {err}"));
        }
    };
    if !status.is_success() {
        return Err(format!(
            "上传媒体到百炼临时存储失败: {} {}",
            status,
            body.trim()
        ));
    }
    Ok(format!("oss://{object_key}"))
}

fn persist_aliyun_multimodal_cache_conversation_update(
    state: &AppState,
    conversation: &Conversation,
    is_runtime_conversation: bool,
) -> Result<(), String> {
    if is_runtime_conversation {
        return delegate_runtime_thread_conversation_update(
            state,
            &conversation.id,
            conversation.clone(),
        );
    }

    let _guard = lock_conversation_with_metrics(state, "aliyun_multimodal_cache_persist")?;
    let mut data = state_read_app_data_cached(state)?;
    let Some(index) = data
        .conversations
        .iter()
        .position(|item| item.id == conversation.id)
    else {
        return Err(format!(
            "写回百炼多模态缓存失败：会话不存在，conversation_id={}",
            conversation.id
        ));
    };
    data.conversations[index] = conversation.clone();
    let _ = state_schedule_app_data_persist(state, &data)?;
    Ok(())
}

fn workspace_absolute_path_from_relative(state: &AppState, relative_or_absolute: &str) -> PathBuf {
    let trimmed = relative_or_absolute.trim();
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return candidate;
    }
    let workspace_root =
        configured_workspace_root_path(state).unwrap_or_else(|_| state.llm_workspace_path.clone());
    workspace_root.join(trimmed.replace('/', "\\"))
}

async fn prepared_binary_raw_bytes_for_aliyun(
    state: &AppState,
    payload: &PreparedBinaryPayload,
) -> Result<Vec<u8>, String> {
    if let Some(saved_path) = payload
        .saved_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let absolute_path = workspace_absolute_path_from_relative(state, saved_path);
        if absolute_path.exists() {
            return tokio::fs::read(&absolute_path).await.map_err(|err| {
                format!(
                    "读取百炼多模态原图失败: path={}, err={}",
                    absolute_path.display(),
                    err
                )
            });
        }
    }
    B64.decode(payload.content.trim())
        .map_err(|err| format!("解析百炼多模态 base64 失败: {err}"))
}

fn apply_aliyun_multimodal_cache_items_to_conversation(
    conversation: &mut Conversation,
    data_path: &PathBuf,
    items: &[AliyunMultimodalCacheItem],
) {
    if items.is_empty() {
        return;
    }
    let mut item_map =
        std::collections::HashMap::<String, AliyunMultimodalCacheItem>::new();
    for item in items {
        item_map.insert(
            aliyun_multimodal_cache_lookup_key(
                &item.kind,
                &item.model,
                &item.content_hash,
            ),
            item.clone(),
        );
    }

    for message in &mut conversation.messages {
        let mut matched_items = Vec::<AliyunMultimodalCacheItem>::new();
        let mut seen_keys = std::collections::HashSet::<String>::new();
        for part in &message.parts {
            let (mime, bytes_base64) = match part {
                MessagePart::Image {
                    mime, bytes_base64, ..
                } => (mime, bytes_base64),
                MessagePart::Audio {
                    mime, bytes_base64, ..
                } => (mime, bytes_base64),
                MessagePart::Text { .. } => continue,
            };
            let Some(kind) = aliyun_multimodal_kind_for_mime(mime) else {
                continue;
            };
            let Ok(resolved_base64) =
                resolve_message_part_binary_base64_for_hash(data_path, bytes_base64)
            else {
                continue;
            };
            let Ok(content_hash) = media_hash_from_base64(&resolved_base64) else {
                continue;
            };
            for item in item_map.values() {
                let key =
                    aliyun_multimodal_cache_lookup_key(kind, &item.model, &content_hash);
                if item.kind.eq_ignore_ascii_case(kind)
                    && item.content_hash.trim() == content_hash.trim()
                    && seen_keys.insert(key)
                {
                    matched_items.push(item.clone());
                }
            }
        }
        if matched_items.is_empty() {
            continue;
        }
        let mut provider_meta = message.provider_meta.clone();
        for item in matched_items {
            upsert_aliyun_multimodal_cache_item(&mut provider_meta, item);
        }
        message.provider_meta = provider_meta;
    }
}

async fn ensure_aliyun_multimodal_urls_for_entries(
    state: &AppState,
    request_format: RequestFormat,
    base_url: &str,
    api_key: &str,
    model_name: &str,
    entries: &mut Vec<PreparedBinaryPayload>,
    cache_lookup: &mut std::collections::HashMap<String, AliyunMultimodalCacheItem>,
    new_items: &mut Vec<AliyunMultimodalCacheItem>,
) -> Result<(usize, usize, usize), String> {
    let mut hit_count = 0usize;
    let mut upload_count = 0usize;
    let mut used_url_count = 0usize;
    let now_ms = chrono::Utc::now().timestamp_millis();

    for entry in entries.iter_mut() {
        if is_remote_binary_url(&entry.content) {
            used_url_count += 1;
            continue;
        }
        if !aliyun_multimodal_supports_urlization(request_format, &entry.mime) {
            continue;
        }
        let Some(kind) = aliyun_multimodal_kind_for_mime(&entry.mime) else {
            continue;
        };
        let raw = prepared_binary_raw_bytes_for_aliyun(state, entry).await?;
        let content_hash = media_hash_from_raw(&raw);
        let cache_key = aliyun_multimodal_cache_lookup_key(kind, model_name, &content_hash);
        if let Some(item) = cache_lookup.get(&cache_key) {
            if aliyun_multimodal_cache_item_is_fresh(item, now_ms) {
                entry.content = item.url.clone();
                hit_count += 1;
                used_url_count += 1;
                continue;
            }
        }

        let url = upload_media_to_aliyun_temp_url(
            state,
            api_key,
            base_url,
            model_name,
            &entry.mime,
            &content_hash,
            raw,
        )
        .await?;
        let item = AliyunMultimodalCacheItem {
            kind: kind.to_string(),
            mime: entry.mime.clone(),
            content_hash,
            model: model_name.to_string(),
            url: url.clone(),
            expires_at_ms: now_ms.saturating_add(ALIYUN_TEMP_URL_TTL_MS),
        };
        cache_lookup.insert(cache_key, item.clone());
        new_items.push(item);
        entry.content = url;
        upload_count += 1;
        used_url_count += 1;
    }

    Ok((hit_count, upload_count, used_url_count))
}

async fn maybe_prepare_aliyun_multimodal_urls_for_candidate(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &mut ResolvedApiConfig,
    model_name: &str,
    prepared_prompt: &mut PreparedPrompt,
    conversation: &mut Conversation,
    is_runtime_conversation: bool,
    persist_cache: bool,
) -> Result<(), String> {
    if !is_aliyun_dashscope_base_url(&resolved_api.base_url) {
        return Ok(());
    }
    let has_multimodal = !prepared_prompt.latest_images.is_empty()
        || !prepared_prompt.latest_audios.is_empty()
        || prepared_prompt
            .history_messages
            .iter()
            .any(|message| !message.images.is_empty() || !message.audios.is_empty());
    if !has_multimodal {
        return Ok(());
    }
    if resolved_api.api_key.trim().is_empty() {
        return Err("百炼多模态 URL 缓存需要可用的 API Key。".to_string());
    }

    let started_at = std::time::Instant::now();
    let mut cache_lookup = collect_aliyun_multimodal_cache_lookup(conversation);
    let mut new_items = Vec::<AliyunMultimodalCacheItem>::new();
    let mut hit_count = 0usize;
    let mut upload_count = 0usize;
    let mut used_url_count = 0usize;

    for message in &mut prepared_prompt.history_messages {
        let (hits, uploads, used_urls) = ensure_aliyun_multimodal_urls_for_entries(
            state,
            selected_api.request_format,
            &resolved_api.base_url,
            &resolved_api.api_key,
            model_name,
            &mut message.images,
            &mut cache_lookup,
            &mut new_items,
        )
        .await?;
        hit_count += hits;
        upload_count += uploads;
        used_url_count += used_urls;

        let (hits, uploads, used_urls) = ensure_aliyun_multimodal_urls_for_entries(
            state,
            selected_api.request_format,
            &resolved_api.base_url,
            &resolved_api.api_key,
            model_name,
            &mut message.audios,
            &mut cache_lookup,
            &mut new_items,
        )
        .await?;
        hit_count += hits;
        upload_count += uploads;
        used_url_count += used_urls;
    }

    let (hits, uploads, used_urls) = ensure_aliyun_multimodal_urls_for_entries(
        state,
        selected_api.request_format,
        &resolved_api.base_url,
        &resolved_api.api_key,
        model_name,
        &mut prepared_prompt.latest_images,
        &mut cache_lookup,
        &mut new_items,
    )
    .await?;
    hit_count += hits;
    upload_count += uploads;
    used_url_count += used_urls;

    let (hits, uploads, used_urls) = ensure_aliyun_multimodal_urls_for_entries(
        state,
        selected_api.request_format,
        &resolved_api.base_url,
        &resolved_api.api_key,
        model_name,
        &mut prepared_prompt.latest_audios,
        &mut cache_lookup,
        &mut new_items,
    )
    .await?;
    hit_count += hits;
    upload_count += uploads;
    used_url_count += used_urls;

    if !new_items.is_empty() {
        apply_aliyun_multimodal_cache_items_to_conversation(
            conversation,
            &state.data_path,
            &new_items,
        );
        if persist_cache {
            persist_aliyun_multimodal_cache_conversation_update(
                state,
                conversation,
                is_runtime_conversation,
            )?;
        }
    }

    if used_url_count > 0 {
        upsert_api_extra_header(
            resolved_api,
            ALIYUN_OSS_RESOLVE_HEADER_KEY,
            ALIYUN_OSS_RESOLVE_HEADER_VALUE,
        );
    }

    runtime_log_info(format!(
        "[百炼多模态URL缓存] 完成，任务=prepare_multimodal_url_cache，触发条件=base_url命中dashscope.aliyuncs.com，命中条数={}，上传条数={}，URL条数={}，会话ID={}，模型={}，耗时毫秒={}",
        hit_count,
        upload_count,
        used_url_count,
        conversation.id,
        model_name,
        started_at.elapsed().as_millis()
    ));
    Ok(())
}

#[cfg(test)]
mod aliyun_multimodal_cache_tests {
    use super::*;

    #[test]
    fn aliyun_multimodal_cache_meta_should_upsert_by_kind_model_and_hash() {
        let mut meta = None;
        upsert_aliyun_multimodal_cache_item(
            &mut meta,
            AliyunMultimodalCacheItem {
                kind: "image".to_string(),
                mime: "image/webp".to_string(),
                content_hash: "sha256:a".to_string(),
                model: "qwen-vl-plus".to_string(),
                url: "oss://a".to_string(),
                expires_at_ms: 1,
            },
        );
        upsert_aliyun_multimodal_cache_item(
            &mut meta,
            AliyunMultimodalCacheItem {
                kind: "image".to_string(),
                mime: "image/webp".to_string(),
                content_hash: "sha256:a".to_string(),
                model: "qwen-vl-plus".to_string(),
                url: "oss://b".to_string(),
                expires_at_ms: 2,
            },
        );
        let items = aliyun_multimodal_cache_items_from_meta(meta.as_ref());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "oss://b");
        assert_eq!(items[0].expires_at_ms, 2);
    }

    #[test]
    fn is_aliyun_dashscope_base_url_should_match_host() {
        assert!(is_aliyun_dashscope_base_url(
            "https://dashscope.aliyuncs.com/compatible-mode/v1"
        ));
        assert!(is_aliyun_dashscope_base_url(
            "https://coding.dashscope.aliyuncs.com/v1"
        ));
        assert!(!is_aliyun_dashscope_base_url("https://api.openai.com/v1"));
    }
}
