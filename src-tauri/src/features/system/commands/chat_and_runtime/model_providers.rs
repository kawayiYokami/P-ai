async fn fetch_models_gemini_native(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');
    let has_version_path = base.contains("/v1beta") || base.contains("/v1/");
    let base_with_version = if has_version_path {
        base.to_string()
    } else {
        format!("{base}/v1beta")
    };
    let url = format!("{}/models", base_with_version.trim_end_matches('/'));
    let api_key = input.api_key.trim();

    if api_key.contains('\r') || api_key.contains('\n') {
        return Err("API key contains newline characters. Please paste a single-line token.".to_string());
    }
    if matches!(api_key, "..." | "***" | "•••" | "···") {
        return Err("API key is still a placeholder ('...' / '***'). Please paste the real token.".to_string());
    }

    let api_key_header = HeaderValue::from_str(api_key)
        .map_err(|err| format!("Build x-goog-api-key header failed: {err}"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    let resp = client
        .get(&url)
        .header("x-goog-api-key", api_key_header)
        .send()
        .await
        .map_err(|err| format!("Fetch Gemini model list failed ({url}): {err}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        let snippet = raw.chars().take(600).collect::<String>();
        return Err(format!(
            "Fetch Gemini model list failed: {url} -> {status} | {snippet}"
        ));
    }

    let body = resp
        .json::<GeminiNativeModelListResponse>()
        .await
        .map_err(|err| format!("Parse Gemini model list failed ({url}): {err}"))?;

    let mut models = body
        .models
        .into_iter()
        .map(|item| item.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .map(|name| name.trim_start_matches("models/").to_string())
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

async fn fetch_models_anthropic(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');
    let url = format!("{base}/v1/models");
    let api_key = input.api_key.trim();

    if api_key.contains('\r') || api_key.contains('\n') {
        return Err("API key contains newline characters. Please paste a single-line token.".to_string());
    }
    if matches!(api_key, "..." | "***" | "•••" | "···") {
        return Err("API key is still a placeholder ('...' / '***'). Please paste the real token.".to_string());
    }

    let api_key_header = HeaderValue::from_str(api_key)
        .map_err(|err| format!("Build x-api-key header failed: {err}"))?;
    let anthropic_version = HeaderValue::from_static("2023-06-01");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    let resp = client
        .get(&url)
        .header("x-api-key", api_key_header)
        .header("anthropic-version", anthropic_version)
        .send()
        .await
        .map_err(|err| format!("Fetch Anthropic model list failed ({url}): {err}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        let snippet = raw.chars().take(600).collect::<String>();
        return Err(format!(
            "Fetch Anthropic model list failed: {url} -> {status} | {snippet}"
        ));
    }

    let body = resp
        .json::<AnthropicModelListResponse>()
        .await
        .map_err(|err| format!("Parse Anthropic model list failed ({url}): {err}"))?;

    let mut models = body
        .data
        .into_iter()
        .map(|item| item.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

fn model_id_exact_match(requested_model: &str, candidate_model: &str) -> bool {
    let requested = requested_model.trim();
    let candidate = candidate_model.trim();
    if requested.is_empty() || candidate.is_empty() {
        return false;
    }
    if candidate == requested || candidate.eq_ignore_ascii_case(requested) {
        return true;
    }
    for sep in ['/', ':'] {
        if let Some((_, suffix)) = candidate.split_once(sep) {
            let suffix = suffix.trim();
            if suffix == requested || suffix.eq_ignore_ascii_case(requested) {
                return true;
            }
        }
    }
    let requested_norm = normalize_model_id(requested);
    let candidate_norm = normalize_model_id(candidate);
    if requested_norm.is_empty() || candidate_norm.is_empty() {
        return false;
    }
    if candidate_norm == requested_norm {
        return true;
    }
    for sep in ['/', ':'] {
        if let Some((_, suffix)) = candidate.split_once(sep) {
            let suffix_norm = normalize_model_id(suffix.trim());
            if suffix_norm == requested_norm {
                return true;
            }
        }
    }
    false
}

fn normalize_model_id(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>()
}

#[tauri::command]
async fn fetch_model_metadata(
    input: FetchModelMetadataInput,
) -> Result<FetchModelMetadataOutput, String> {
    let requested_model = input.model.trim();
    if requested_model.is_empty() {
        return Err("Model is empty.".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let resp = client
        .get("https://models.dev/api.json")
        .send()
        .await
        .map_err(|err| format!("Fetch models.dev metadata failed: {err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let snippet = body.chars().take(400).collect::<String>();
        return Err(format!(
            "Fetch models.dev metadata failed: {status} | {snippet}"
        ));
    }
    let root = resp
        .json::<Value>()
        .await
        .map_err(|err| format!("Parse models.dev metadata failed: {err}"))?;
    let providers = root
        .as_object()
        .ok_or_else(|| "Invalid models.dev payload: expected root object.".to_string())?;
    let mut best_match: Option<FetchModelMetadataOutput> = None;
    let mut best_sort_key: Option<(u8, u8, u64, u32, u32, u8)> = None;
    for provider_obj in providers.values().filter_map(Value::as_object) {
        let Some(models_obj) = provider_obj.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_id, model_value) in models_obj
            .iter()
            .filter(|(model_id, _)| model_id_exact_match(requested_model, model_id))
        {
            let Some(model_obj) = model_value.as_object() else {
                continue;
            };
            let limit_obj = model_obj.get("limit").and_then(Value::as_object);
            let context_window_tokens = limit_obj
                .and_then(|limit| limit.get("context"))
                .and_then(Value::as_u64)
                .map(|v| v.min(u64::from(u32::MAX)) as u32);
            let max_output_tokens = limit_obj
                .and_then(|limit| limit.get("output"))
                .and_then(Value::as_u64)
                .map(|v| v.min(u64::from(u32::MAX)) as u32);
            let input_modalities = model_obj
                .get("modalities")
                .and_then(Value::as_object)
                .and_then(|modalities| modalities.get("input"))
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|s| s.to_ascii_lowercase())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let enable_image = input_modalities.iter().any(|item| item.contains("image"));
            let enable_audio = input_modalities.iter().any(|item| item.contains("audio"));
            let enable_tools = model_obj
                .get("tool_call")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            // 冲突时优先“可用信息更完整”，其次“上限更大”。
            let numeric_count = context_window_tokens.is_some() as u8 + max_output_tokens.is_some() as u8;
            let capability_count = enable_image as u8 + enable_tools as u8 + enable_audio as u8;
            let has_any_usable = (numeric_count > 0 || capability_count > 0) as u8;
            let context_value = context_window_tokens.unwrap_or(0);
            let output_value = max_output_tokens.unwrap_or(0);
            let size_sum = u64::from(context_value) + u64::from(output_value);
            let candidate_sort_key = (
                has_any_usable,
                numeric_count,
                size_sum,
                context_value,
                output_value,
                capability_count,
            );

            let should_take = match best_sort_key {
                None => true,
                Some(current) => candidate_sort_key > current,
            };
            if should_take {
                best_sort_key = Some(candidate_sort_key);
                best_match = Some(FetchModelMetadataOutput {
                    found: true,
                    matched_model_id: Some(model_id.to_string()),
                    context_window_tokens,
                    max_output_tokens,
                    enable_image: Some(enable_image),
                    enable_tools: Some(enable_tools),
                    enable_audio: Some(enable_audio),
                });
            }
        }
    }
    let Some(best_match) = best_match else {
        return Ok(FetchModelMetadataOutput {
            found: false,
            matched_model_id: None,
            context_window_tokens: None,
            max_output_tokens: None,
            enable_image: None,
            enable_tools: None,
            enable_audio: None,
        });
    };
    Ok(best_match)
}

#[tauri::command]
async fn refresh_models(input: RefreshModelsInput) -> Result<Vec<String>, String> {
    if input.api_key.trim().is_empty() {
        return Err("API key is empty.".to_string());
    }
    if input.base_url.trim().is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    match input.request_format {
        RequestFormat::OpenAI | RequestFormat::OpenAIResponses => {
            fetch_models_openai(&input).await
        }
        RequestFormat::Gemini => fetch_models_gemini_native(&input).await,
        RequestFormat::GeminiEmbedding => Err(
            "Request format 'gemini_embedding' is for embedding and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::Anthropic => fetch_models_anthropic(&input).await,
        RequestFormat::OpenAITts => Err(
            "Request format 'openai_tts' is for TTS and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::OpenAIStt => Err(
            "Request format 'openai_stt' is for STT and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::OpenAIEmbedding => Err(
            "Request format 'openai_embedding' is for embedding and does not support model list refresh."
                .to_string(),
        ),
        RequestFormat::OpenAIRerank => Err(
            "Request format 'openai_rerank' is for rerank and does not support model list refresh."
                .to_string(),
        ),
    }
}

