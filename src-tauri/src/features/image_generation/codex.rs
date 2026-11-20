struct CodexImageAuth {
    access_token: String,
    account_id: Option<String>,
    base_url: String,
    is_custom_url: bool,
}

const CODEX_IMAGE_TOOL_OUTPUT_FORMAT: &str = "png";

/// 按供应商 id 找 Codex API 账号并解出可用凭证。生图与模型清单共用。
async fn resolve_codex_api_auth_by_id(
    state: &AppState,
    requested_id: Option<&str>,
) -> Result<CodexImageAuth, String> {
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    let requested_id = requested_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let api_provider = requested_id
        .and_then(|id| config.api_providers.iter().find(|item| item.id == id))
        .or_else(|| {
            config
                .api_providers
                .iter()
                .find(|item| item.request_format.is_codex() && !item.deprecated)
        })
        .ok_or_else(|| "未找到可复用的 Codex API 供应商，请先在 API 设置中配置并登录 Codex。".to_string())?;
    if api_provider.deprecated || !api_provider.request_format.is_codex() {
        return Err(format!(
            "关联的 API 供应商不是可用的 Codex 供应商：{}",
            api_provider.name
        ));
    }

    let auth_mode = normalize_codex_auth_mode(&api_provider.codex_auth_mode);
    let (access_token, account_id) = if auth_mode == CODEX_AUTH_MODE_CUSTOM_URL {
        (
            api_provider
                .codex_custom_api_key
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string(),
            None,
        )
    } else {
        let snapshot = read_codex_runtime_auth_snapshot(
            &api_provider.id,
            &auth_mode,
            &api_provider.codex_local_auth_path,
        )?;
        let fresh = ensure_codex_runtime_auth_fresh(&snapshot).await?;
        (fresh.access_token.clone(), fresh.account_id.clone())
    };
    if access_token.trim().is_empty() {
        return Err(format!(
            "Codex 供应商“{}”尚未登录或凭证已失效，请先在 API 设置中完成登录。",
            api_provider.name
        ));
    }
    let base_url = if auth_mode == CODEX_AUTH_MODE_CUSTOM_URL {
        api_provider
            .codex_custom_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&api_provider.base_url)
            .trim()
            .to_string()
    } else {
        api_provider.base_url.trim().to_string()
    };
    let base_url = if base_url.is_empty() {
        "https://chatgpt.com/backend-api/codex".to_string()
    } else {
        base_url
    };
    Ok(CodexImageAuth {
        access_token,
        account_id,
        base_url,
        is_custom_url: auth_mode == CODEX_AUTH_MODE_CUSTOM_URL,
    })
}

async fn resolve_codex_image_auth(
    state: &AppState,
    provider: &ImageGenerationProviderConfig,
) -> Result<CodexImageAuth, String> {
    resolve_codex_api_auth_by_id(state, provider.codex_api_provider_id.as_deref()).await
}

/// 兜底路径（custom_url 中转）：沿用 /responses + image_generation 工具，中转站只代理该形态。
fn codex_legacy_responses_payload(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
    edit_inputs: &ImageEditInputs,
) -> Value {
    let mut tool = serde_json::json!({
        "type": "image_generation",
        "model": CODEX_IMAGE_MODEL,
        "output_format": CODEX_IMAGE_TOOL_OUTPUT_FORMAT,
    });
    if let Some(size) = effective_image_generation_size(request, model) {
        tool["size"] = Value::String(size);
    }
    if let Some(quality) = effective_image_generation_quality(request, model) {
        tool["quality"] = Value::String(quality);
    }
    // Responses API 的 mask 通过 image_generation 工具配置传入，而非消息内容。
    if let Some(mask) = &edit_inputs.mask {
        tool["input_image_mask"] = serde_json::json!({ "image_url": image_edit_data_url(mask) });
    }
    let mut content = vec![serde_json::json!({
        "type": "input_text",
        "text": effective_image_generation_prompt(request)
    })];
    for input in &edit_inputs.images {
        content.push(serde_json::json!({
            "type": "input_image",
            "image_url": image_edit_data_url(input)
        }));
    }
    serde_json::json!({
        "instructions": "",
        "stream": true,
        "reasoning": {"effort": "medium", "summary": "auto"},
        "store": false,
        "parallel_tool_calls": true,
        "include": ["reasoning.encrypted_content"],
        "model": CODEX_IMAGE_MODEL,
        "input": [{
            "type": "message",
            "role": "user",
            "content": content
        }],
        "tools": [tool],
        "tool_choice": { "type": "image_generation" }
    })
}

/// 原生端点合法的 size 取值（OpenAI Images 集合）。不在集合里的显式值直接丢弃，
/// 由后端决定默认，不把非法值发出去。注意 512x512 不在集合里：Codex 分支
/// 即使模型默认值是 512x512 也不得原样发出。
fn codex_native_size_allowed(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "1024x1024" | "1536x1024" | "1024x1536" | "auto"
    ) {
        Some(normalized)
    } else {
        None
    }
}

/// 原生端点 size 解析：只产出合法集合。显式 size 命中集合才用；宽高比走
/// openai_size_from_aspect_ratio（gpt-image-2 支持任意比例映射）；模型默认值
/// 里的 512x512 会被集合过滤掉，对应「不发 size 字段」。
fn codex_native_image_size(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Option<String> {
    if let Some(size) = trimmed_image_generation_option(&request.size) {
        if let Some(allowed) = codex_native_size_allowed(&size) {
            return Some(allowed);
        }
        return None;
    }
    if let Some(aspect) = trimmed_image_generation_option(&request.aspect_ratio) {
        if let Some(mapped) = openai_size_from_aspect_ratio(&aspect, false) {
            return Some(mapped);
        }
    }
    if let Some(aspect) = trimmed_image_generation_option(&model.default_aspect_ratio) {
        if let Some(mapped) = openai_size_from_aspect_ratio(&aspect, false) {
            return Some(mapped);
        }
    }
    trimmed_image_generation_option(&model.default_size)
        .as_deref()
        .and_then(codex_native_size_allowed)
}

/// 生成 payload（无参考图）：照抄 magpie draw.go:1178-1191，非空才加。
fn codex_native_generations_payload(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Value {
    let mut payload = serde_json::json!({
        "model": CODEX_IMAGE_MODEL,
        "prompt": effective_image_generation_prompt(request),
        "n": 1,
        "output_format": "png",
    });
    if let Some(object) = payload.as_object_mut() {
        if let Some(size) = codex_native_image_size(request, model) {
            object.insert("size".to_string(), Value::String(size));
        }
        if let Some(quality) = effective_image_generation_quality(request, model) {
            object.insert("quality".to_string(), Value::String(quality));
        }
    }
    payload
}

/// 编辑 payload（有参考图）：照抄 magpie draw.go:1193-1208，只收 JSON data URL，不用 multipart。
fn codex_native_edits_payload(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
    edit_inputs: &ImageEditInputs,
) -> Value {
    let mut payload = codex_native_generations_payload(request, model);
    if let Some(object) = payload.as_object_mut() {
        let images: Vec<Value> = edit_inputs
            .images
            .iter()
            .map(|input| serde_json::json!({ "image_url": image_edit_data_url(input) }))
            .collect();
        object.insert("images".to_string(), Value::Array(images));
        if let Some(mask) = &edit_inputs.mask {
            object.insert(
                "mask".to_string(),
                serde_json::json!({ "image_url": image_edit_data_url(mask) }),
            );
        }
    }
    payload
}

/// Codex 后端身份头（codexSign 六件套里除 Authorization 的五个 + residency）。
/// images 与 models 共用：models 传 for_images=false，不带 Accept 与 imagegen id。
fn codex_backend_identity_headers(auth: &CodexImageAuth) -> Result<Vec<(String, String)>, String> {
    let mut headers = vec![
        ("OpenAI-Beta".to_string(), "responses=experimental".to_string()),
        ("originator".to_string(), "codex_cli_rs".to_string()),
        ("version".to_string(), CODEX_PRESET_VERSION.to_string()),
        ("User-Agent".to_string(), CODEX_PRESET_USER_AGENT.to_string()),
    ];
    if let Some(account_id) = auth
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.push(("ChatGPT-Account-Id".to_string(), account_id.to_string()));
    }
    // 对齐聊天路径：从 access_token JWT 提取 compute residency，无值不发。
    if let Some(residency) = codex_extract_residency_from_token(&auth.access_token) {
        headers.push(("x-openai-internal-codex-residency".to_string(), residency));
    }
    for (name, value) in &headers {
        HeaderValue::from_str(value)
            .map_err(|err| format!("Codex 生图请求头构造失败 ({name})：{err}"))?;
    }
    Ok(headers)
}

/// 原生端点请求头：身份头 + images 增补（draw.go:1009-1016：Accept 与两个 imagegen id）。
fn codex_native_image_headers(auth: &CodexImageAuth) -> Result<reqwest::header::HeaderMap, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    for (name, value) in codex_backend_identity_headers(auth)? {
        headers.insert(
            name.parse::<reqwest::header::HeaderName>()
                .map_err(|err| format!("Codex 生图请求头构造失败：{err}"))?,
            HeaderValue::from_str(&value)
                .map_err(|err| format!("Codex 生图请求头构造失败 ({value})：{err}"))?,
        );
    }
    for name in ["x-codex-imagegen-request-id", "x-codex-image-turn-id"] {
        headers.insert(
            name,
            HeaderValue::from_str(&Uuid::new_v4().to_string())
                .map_err(|err| format!("Codex 生图请求头构造失败：{err}"))?,
        );
    }
    Ok(headers)
}

fn codex_native_error_message(status: reqwest::StatusCode, body: &[u8]) -> String {
    let detail = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .or_else(|| value.get("message").and_then(Value::as_str))
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| String::from_utf8_lossy(body).trim().to_string());
    if status.as_u16() == 429 {
        return if detail.is_empty() {
            "Codex 生图额度不足（HTTP 429），请稍后重试或检查套餐额度。".to_string()
        } else {
            format!("Codex 生图额度不足（HTTP 429）：{detail}")
        };
    }
    if detail.is_empty() {
        format!("Codex 生图请求失败（HTTP {}）", status.as_u16())
    } else {
        format!("Codex 生图请求失败（HTTP {}）：{}", status.as_u16(), detail)
    }
}

/// 原生端点 POST：抽出来让测试能用本地假服务器走完整 reqwest 链路。
async fn post_codex_native_image(
    client: &reqwest::Client,
    endpoint: &str,
    headers: reqwest::header::HeaderMap,
    access_token: &str,
    payload: &Value,
    provider_name: &str,
    timeout_seconds: u64,
) -> Result<ProviderImageGenerationOutput, String> {
    let response = client
        .post(endpoint)
        .headers(headers)
        .bearer_auth(access_token)
        .json(payload)
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .send()
        .await
        .map_err(|err| format!("{provider_name} 请求失败：{err}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| format!("读取 Codex 生图响应失败：{err}"))?;
    if !status.is_success() {
        return Err(codex_native_error_message(status, &body));
    }
    let value = serde_json::from_slice::<Value>(&body)
        .map_err(|err| format!("{provider_name} 响应不是有效 JSON：{err}"))?;
    parse_openai_style_image_response(&value)
}

fn codex_image_output_mime(output_format: Option<&str>) -> &'static str {
    match output_format
        .unwrap_or(CODEX_IMAGE_TOOL_OUTPUT_FORMAT)
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn collect_codex_image_value(
    value: &Value,
    final_images: &mut Vec<PendingGeneratedImage>,
    partial_images: &mut Vec<PendingGeneratedImage>,
    final_seen: &mut std::collections::HashSet<String>,
    partial_seen: &mut std::collections::HashSet<String>,
    text_parts: &mut Vec<String>,
) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            let event_type = object.get("type").and_then(Value::as_str).unwrap_or_default();
            if event_type == "output_text" {
                if let Some(text) = object.get("text").and_then(Value::as_str) {
                    let text = text.trim();
                    if !text.is_empty() && !text_parts.iter().any(|item| item == text) {
                        text_parts.push(text.to_string());
                    }
                }
            }
            let encoded = if event_type == "image_generation_call" {
                object.get("result").and_then(Value::as_str)
            } else if event_type == "response.image_generation_call.partial_image" {
                object.get("partial_image_b64").and_then(Value::as_str)
            } else {
                None
            };
            if let Some(encoded) = encoded.map(str::trim).filter(|value| !value.is_empty()) {
                let pending = PendingGeneratedImage {
                    source: PendingImageSource::Bytes(decode_generated_image_base64(encoded)?),
                    mime_hint: Some(
                        codex_image_output_mime(
                            object.get("output_format").and_then(Value::as_str),
                        )
                        .to_string(),
                    ),
                    remote_url: None,
                    revised_prompt: None,
                };
                if event_type == "image_generation_call" {
                    if final_seen.insert(encoded.to_string()) {
                        final_images.push(pending);
                    }
                } else if partial_seen.insert(encoded.to_string()) {
                    partial_images.push(pending);
                }
            }
            for child in object.values() {
                collect_codex_image_value(
                    child,
                    final_images,
                    partial_images,
                    final_seen,
                    partial_seen,
                    text_parts,
                )?;
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_codex_image_value(
                    item,
                    final_images,
                    partial_images,
                    final_seen,
                    partial_seen,
                    text_parts,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_codex_image_generation_response(data: &[u8]) -> Result<ProviderImageGenerationOutput, String> {
    let mut final_images = Vec::new();
    let mut partial_images = Vec::new();
    let mut final_seen = std::collections::HashSet::new();
    let mut partial_seen = std::collections::HashSet::new();
    let mut text_parts = Vec::new();
    let mut parsed_event = false;
    for line in data.split(|byte| *byte == b'\n') {
        let line = line.strip_prefix(b"data:").unwrap_or(line);
        let line = line.strip_prefix(b" ").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let line = line.trim_ascii();
        if line.is_empty() || line == b"[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        parsed_event = true;
        collect_codex_image_value(
            &value,
            &mut final_images,
            &mut partial_images,
            &mut final_seen,
            &mut partial_seen,
            &mut text_parts,
        )?;
    }
    if !parsed_event {
        if let Ok(value) = serde_json::from_slice::<Value>(data) {
            collect_codex_image_value(
                &value,
                &mut final_images,
                &mut partial_images,
                &mut final_seen,
                &mut partial_seen,
                &mut text_parts,
            )?;
        }
    }
    let images = if final_images.is_empty() {
        partial_images.into_iter().last().into_iter().collect()
    } else {
        final_images
    };
    if images.is_empty() {
        let text = text_parts.join("\n");
        return Err(if text.is_empty() {
            "Codex 未返回图片数据".to_string()
        } else {
            format!("Codex 未返回图片数据：{text}")
        });
    }
    Ok(ProviderImageGenerationOutput {
        images,
        text: (!text_parts.is_empty()).then(|| text_parts.join("\n")),
    })
}

async fn generate_codex_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    edit_inputs: &ImageEditInputs,
) -> Result<ProviderImageGenerationOutput, String> {
    if matches!(request.operation, ImageGenerationOperation::Edit) {
        ensure_image_edit_input_limits(
            &resolved.provider.name,
            edit_inputs,
            CODEX_IMAGE_EDIT_MAX_IMAGES,
            true,
        )?;
    }
    let auth = resolve_codex_image_auth(state, &resolved.provider).await?;
    // custom_url 中转只代理 /responses：保留旧路，行为与改动前一致。
    if auth.is_custom_url {
        return generate_codex_legacy_image_once(state, resolved, request, edit_inputs, &auth).await;
    }
    let editing = matches!(request.operation, ImageGenerationOperation::Edit);
    let suffix = if editing { "/images/edits" } else { "/images/generations" };
    let endpoint = append_image_generation_endpoint(&auth.base_url, suffix);
    let headers = codex_native_image_headers(&auth)?;
    let payload = if editing {
        codex_native_edits_payload(request, &resolved.model, edit_inputs)
    } else {
        codex_native_generations_payload(request, &resolved.model)
    };
    post_codex_native_image(
        &state.shared_http_client,
        &endpoint,
        headers,
        &auth.access_token,
        &payload,
        &resolved.provider.name,
        u64::from(resolved.provider.timeout_seconds),
    )
    .await
}

async fn generate_codex_legacy_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    edit_inputs: &ImageEditInputs,
    auth: &CodexImageAuth,
) -> Result<ProviderImageGenerationOutput, String> {
    let endpoint = append_image_generation_endpoint(&auth.base_url, "/responses");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("text/event-stream"),
    );
    if let Some(account_id) = auth
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.insert(
            "ChatGPT-Account-Id",
            HeaderValue::from_str(account_id)
                .map_err(|err| format!("Codex 生图请求头构造失败：{err}"))?,
        );
    }
    let response = state
        .shared_http_client
        .post(endpoint)
        .headers(headers)
        .bearer_auth(&auth.access_token)
        .json(&codex_legacy_responses_payload(request, &resolved.model, edit_inputs))
        .timeout(std::time::Duration::from_secs(u64::from(
            resolved.provider.timeout_seconds,
        )))
        .send()
        .await
        .map_err(|err| format!("{} 请求失败：{err}", resolved.provider.name))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| format!("读取 Codex 生图响应失败：{err}"))?;
    if !status.is_success() {
        let message = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("message").and_then(Value::as_str))
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        return Err(format!("Codex 生图请求失败（HTTP {}）：{}", status.as_u16(), message));
    }
    parse_codex_image_generation_response(&body)
}

#[cfg(test)]
mod codex_image_generation_tests {
    use super::*;

    fn test_request() -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "一只戴红围巾的猫".to_string(),
            ..ImageGenerationRequest::default()
        }
    }

    #[test]
    fn legacy_payload_should_force_codex_image_generation_tool() {
        let mut model = ImageGenerationModelConfig::default();
        model.model = "user-configured-model-should-be-ignored".to_string();
        let payload = codex_legacy_responses_payload(
            &test_request(),
            &model,
            &ImageEditInputs::default(),
        );

        assert_eq!(payload["model"], CODEX_IMAGE_MODEL);
        assert_eq!(payload["tools"][0]["type"], "image_generation");
        assert_eq!(payload["tools"][0]["model"], CODEX_IMAGE_MODEL);
        assert_eq!(payload["tools"][0]["output_format"], "png");
        assert_eq!(payload["tool_choice"]["type"], "image_generation");
        assert_eq!(payload["reasoning"]["effort"], "medium");
    }

    #[test]
    fn legacy_payload_should_attach_edit_images_and_mask() {
        let inputs = ImageEditInputs {
            images: vec![ImageEditInputImage {
                bytes: b"img".to_vec(),
                mime: "image/png".to_string(),
            }],
            mask: Some(ImageEditInputImage {
                bytes: b"mask".to_vec(),
                mime: "image/png".to_string(),
            }),
        };
        let payload = codex_legacy_responses_payload(
            &test_request(),
            &ImageGenerationModelConfig::default(),
            &inputs,
        );

        assert_eq!(payload["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(payload["input"][0]["content"][1]["type"], "input_image");
        assert!(payload["input"][0]["content"][1]["image_url"]
            .as_str()
            .unwrap_or_default()
            .starts_with("data:image/png;base64,"));
        assert!(payload["tools"][0]["input_image_mask"]["image_url"]
            .as_str()
            .unwrap_or_default()
            .starts_with("data:image/png;base64,"));
    }

    #[test]
    fn parser_should_prefer_final_result_over_partial_preview() {
        let response = concat!(
            "data: {\"type\":\"response.image_generation_call.partial_image\",\"output_format\":\"png\",\"partial_image_b64\":\"cHJldmlldw==\"}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"image_generation_call\",\"output_format\":\"png\",\"result\":\"ZmluYWw=\"}}\n\n",
            "data: [DONE]\n\n"
        );
        let output = parse_codex_image_generation_response(response.as_bytes())
            .expect("Codex image response should parse");

        assert_eq!(output.images.len(), 1);
        match &output.images[0].source {
            PendingImageSource::Bytes(bytes) => assert_eq!(bytes, b"final"),
            PendingImageSource::RemoteUrl(_) => panic!("expected inline image bytes"),
        }
    }

    #[test]
    fn parser_should_accept_partial_image_when_no_final_result_exists() {
        let response = b"data: {\"type\":\"response.image_generation_call.partial_image\",\"output_format\":\"png\",\"partial_image_b64\":\"cHJldmlldw==\"}\n\n";
        let output = parse_codex_image_generation_response(response)
            .expect("Codex partial image response should parse");

        assert_eq!(output.images.len(), 1);
        match &output.images[0].source {
            PendingImageSource::Bytes(bytes) => assert_eq!(bytes, b"preview"),
            PendingImageSource::RemoteUrl(_) => panic!("expected inline image bytes"),
        }
    }

    // ==================== 原生 /images/* ====================

    fn test_model() -> ImageGenerationModelConfig {
        ImageGenerationModelConfig {
            default_size: Some("512x512".to_string()),
            default_aspect_ratio: None,
            default_quality: Some("medium".to_string()),
            ..ImageGenerationModelConfig::default()
        }
    }

    #[test]
    fn native_generations_payload_should_pin_gpt_image_2() {
        let payload =
            codex_native_generations_payload(&test_request(), &ImageGenerationModelConfig::default());
        assert_eq!(payload["model"], "gpt-image-2");
        assert_eq!(payload["n"], 1);
        assert_eq!(payload["output_format"], "png");
        assert!(payload.get("images").is_none());
    }

    #[test]
    fn native_size_should_never_emit_512() {
        // 模型默认值 512x512 不在合法集合里：不得原样发出，由后端决定默认。
        assert_eq!(codex_native_image_size(&test_request(), &test_model()), None);
        let mut aspect_request = test_request();
        aspect_request.aspect_ratio = Some("16:9".to_string());
        assert_eq!(
            codex_native_image_size(&aspect_request, &test_model()),
            Some("1536x1024".to_string())
        );
        let mut explicit_request = test_request();
        explicit_request.size = Some("1024x1024".to_string());
        assert_eq!(
            codex_native_image_size(&explicit_request, &test_model()),
            Some("1024x1024".to_string())
        );
        let mut bad_request = test_request();
        bad_request.size = Some("512x512".to_string());
        assert_eq!(codex_native_image_size(&bad_request, &test_model()), None);
    }

    #[test]
    fn native_edits_payload_should_carry_images_and_mask_as_data_urls() {
        let inputs = ImageEditInputs {
            images: vec![ImageEditInputImage {
                bytes: b"img".to_vec(),
                mime: "image/png".to_string(),
            }],
            mask: Some(ImageEditInputImage {
                bytes: b"mask".to_vec(),
                mime: "image/png".to_string(),
            }),
        };
        let payload = codex_native_edits_payload(&test_request(), &test_model(), &inputs);
        assert_eq!(payload["model"], "gpt-image-2");
        assert!(payload["images"][0]["image_url"]
            .as_str()
            .unwrap_or_default()
            .starts_with("data:image/png;base64,"));
        assert!(payload["mask"]["image_url"]
            .as_str()
            .unwrap_or_default()
            .starts_with("data:image/png;base64,"));
    }

    #[test]
    fn native_headers_should_carry_codex_sign_set() {
        let auth = CodexImageAuth {
            access_token: "tok".to_string(),
            account_id: Some("acct-1".to_string()),
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            is_custom_url: false,
        };
        let headers = codex_native_image_headers(&auth).expect("headers should build");
        let get = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_string()
        };
        assert_eq!(get("accept"), "application/json");
        assert_eq!(get("openai-beta"), "responses=experimental");
        assert_eq!(get("originator"), "codex_cli_rs");
        assert_eq!(get("version"), CODEX_PRESET_VERSION);
        assert_eq!(get("user-agent"), CODEX_PRESET_USER_AGENT);
        assert_eq!(get("chatgpt-account-id"), "acct-1");
        assert!(!get("x-codex-imagegen-request-id").is_empty());
        assert!(!get("x-codex-image-turn-id").is_empty());
    }

    #[test]
    fn native_error_429_should_read_quota_text() {
        let message = codex_native_error_message(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            br#"{"error":{"message":"quota exceeded"}}"#,
        );
        assert!(message.contains("429"), "unexpected: {message}");
        assert!(message.contains("quota exceeded"), "unexpected: {message}");
    }

    fn native_test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
    }

    #[test]
    fn native_post_should_send_signed_json_and_parse_b64() {
        use httpmock::{Method::POST, MockServer};
        native_test_runtime().block_on(async {
            use base64::Engine as _;
            let server = MockServer::start();
            let body_b64 = base64::engine::general_purpose::STANDARD.encode(b"fake-png-bytes");
            let mock = server.mock(|when, then| {
                when.method(POST)
                    .path("/images/generations")
                    .header("accept", "application/json")
                    .header("originator", "codex_cli_rs")
                    .header("user-agent", CODEX_PRESET_USER_AGENT);
                then.status(200).json_body(serde_json::json!({
                    "created": 1,
                    "output_format": "png",
                    "data": [{ "b64_json": body_b64 }]
                }));
            });
            let auth = CodexImageAuth {
                access_token: "test-token".to_string(),
                account_id: Some("acct-1".to_string()),
                base_url: server.base_url(),
                is_custom_url: false,
            };
            let endpoint =
                append_image_generation_endpoint(&auth.base_url, "/images/generations");
            let payload =
                codex_native_generations_payload(&test_request(), &ImageGenerationModelConfig::default());
            let output = post_codex_native_image(
                &reqwest::Client::new(),
                &endpoint,
                codex_native_image_headers(&auth).expect("headers"),
                &auth.access_token,
                &payload,
                "Codex",
                30,
            )
            .await
            .expect("native post should succeed");
            mock.assert();
            assert_eq!(output.images.len(), 1);
            match &output.images[0].source {
                PendingImageSource::Bytes(bytes) => assert_eq!(bytes, b"fake-png-bytes"),
                PendingImageSource::RemoteUrl(_) => panic!("expected inline image bytes"),
            }
        });
    }

    #[test]
    fn native_post_should_parse_url_answer() {
        use httpmock::{Method::POST, MockServer};
        native_test_runtime().block_on(async {
            let server = MockServer::start();
            let mock = server.mock(|when, then| {
                when.method(POST).path("/images/edits");
                then.status(200).json_body(serde_json::json!({
                    "created": 1,
                    "data": [{ "url": "https://cdn.example/img.png" }]
                }));
            });
            let auth = CodexImageAuth {
                access_token: "test-token".to_string(),
                account_id: None,
                base_url: server.base_url(),
                is_custom_url: false,
            };
            let endpoint = append_image_generation_endpoint(&auth.base_url, "/images/edits");
            let inputs = ImageEditInputs {
                images: vec![ImageEditInputImage {
                    bytes: b"img".to_vec(),
                    mime: "image/png".to_string(),
                }],
                mask: None,
            };
            let payload = codex_native_edits_payload(&test_request(), &test_model(), &inputs);
            let output = post_codex_native_image(
                &reqwest::Client::new(),
                &endpoint,
                codex_native_image_headers(&auth).expect("headers"),
                &auth.access_token,
                &payload,
                "Codex",
                30,
            )
            .await
            .expect("native edit post should succeed");
            mock.assert();
            assert_eq!(output.images.len(), 1);
            match &output.images[0].source {
                PendingImageSource::RemoteUrl(url) => assert_eq!(url, "https://cdn.example/img.png"),
                PendingImageSource::Bytes(_) => panic!("expected remote url"),
            }
        });
    }
}
