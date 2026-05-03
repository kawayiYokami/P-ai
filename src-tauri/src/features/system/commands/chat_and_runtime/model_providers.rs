const MODELS_DEV_CACHE_FILE_NAME: &str = "models_dev_api_cache.json";
const MODELS_DEV_CACHE_MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;
const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelsDevCacheFile {
    updated_at: String,
    fetched_at_ms: i64,
    root: Value,
}

fn models_dev_cache_path(state: &AppState) -> std::path::PathBuf {
    state
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(MODELS_DEV_CACHE_FILE_NAME)
}

fn read_models_dev_cache_file(state: &AppState) -> Result<Option<ModelsDevCacheFile>, String> {
    let path = models_dev_cache_path(state);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(&path)
        .map_err(|err| format!("Read models.dev cache failed ({}): {err}", path.display()))?;
    let cache = serde_json::from_slice::<ModelsDevCacheFile>(&raw)
        .map_err(|err| format!("Parse models.dev cache failed ({}): {err}", path.display()))?;
    Ok(Some(cache))
}

fn write_models_dev_cache_file(
    state: &AppState,
    root: &Value,
) -> Result<ModelsDevCacheFile, String> {
    let path = models_dev_cache_path(state);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Create models.dev cache directory failed ({}): {err}",
                parent.display()
            )
        })?;
    }
    let cache = ModelsDevCacheFile {
        updated_at: now_iso(),
        fetched_at_ms: chrono::Utc::now().timestamp_millis(),
        root: root.clone(),
    };
    let raw = serde_json::to_vec_pretty(&cache)
        .map_err(|err| format!("Serialize models.dev cache failed: {err}"))?;
    std::fs::write(&path, raw)
        .map_err(|err| format!("Write models.dev cache failed ({}): {err}", path.display()))?;
    Ok(cache)
}

fn models_dev_cache_is_stale(cache: &ModelsDevCacheFile) -> bool {
    let age_ms = chrono::Utc::now().timestamp_millis() - cache.fetched_at_ms;
    age_ms > MODELS_DEV_CACHE_MAX_AGE_MS
}

async fn fetch_models_dev_root(state: &AppState) -> Result<Value, String> {
    let resp = state
        .shared_http_client
        .get(MODELS_DEV_API_URL)
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
    resp.json::<Value>()
        .await
        .map_err(|err| format!("Parse models.dev metadata failed: {err}"))
}

async fn ensure_models_dev_cache_current(state: &AppState) -> Result<ModelsDevCacheFile, String> {
    let cached = read_models_dev_cache_file(state)?;
    match cached {
        Some(cache) if !models_dev_cache_is_stale(&cache) => Ok(cache),
        Some(cache) => match fetch_models_dev_root(state).await {
            Ok(root) => write_models_dev_cache_file(state, &root),
            Err(err) => {
                eprintln!(
                    "[models.dev缓存] 刷新失败，回退旧缓存: error={:?}, updated_at={}, fetched_at_ms={}",
                    err, cache.updated_at, cache.fetched_at_ms
                );
                Ok(cache)
            }
        },
        None => {
            let root = fetch_models_dev_root(state).await?;
            write_models_dev_cache_file(state, &root)
        }
    }
}

fn read_models_dev_cache_only(state: &AppState) -> Result<ModelsDevCacheFile, String> {
    read_models_dev_cache_file(state)?
        .ok_or_else(|| "暂无模型元数据缓存，请先手动刷新模型列表。".to_string())
}

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
    let base_with_version = if base.ends_with("/v1") {
        base.to_string()
    } else {
        format!("{base}/v1")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelRefreshStrategy {
    OpenAi,
    GeminiNative,
    AnthropicNative,
    CodexBuiltin,
}

fn codex_builtin_models() -> Vec<String> {
    vec![
        "gpt-5.5".to_string(),
        "gpt-5.4".to_string(),
        "gpt-5.4-mini".to_string(),
        "gpt-5.3-codex".to_string(),
        "gpt-5.2".to_string(),
    ]
}

fn push_unique_refresh_strategy(
    strategies: &mut Vec<ModelRefreshStrategy>,
    strategy: ModelRefreshStrategy,
) {
    if !strategies.contains(&strategy) {
        strategies.push(strategy);
    }
}

fn inferred_model_refresh_strategy_from_base_url(base_url: &str) -> Option<ModelRefreshStrategy> {
    let normalized = base_url.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized.contains("chatgpt.com/backend-api/codex") {
        return Some(ModelRefreshStrategy::CodexBuiltin);
    }
    if normalized.contains("generativelanguage.googleapis.com")
        || normalized.contains("aistudio.google.com")
        || normalized.contains("gemini")
    {
        return Some(ModelRefreshStrategy::GeminiNative);
    }
    if normalized.contains("api.anthropic.com") || normalized.contains("anthropic") {
        return Some(ModelRefreshStrategy::AnthropicNative);
    }
    Some(ModelRefreshStrategy::OpenAi)
}

fn model_refresh_strategies(input: &RefreshModelsInput) -> Vec<ModelRefreshStrategy> {
    let mut strategies = Vec::<ModelRefreshStrategy>::new();
    let inferred = inferred_model_refresh_strategy_from_base_url(&input.base_url);
    match input.request_format {
        RequestFormat::Gemini => {
            push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::GeminiNative);
        }
        RequestFormat::Anthropic => {
            push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::AnthropicNative);
        }
        RequestFormat::Codex => {
            push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::CodexBuiltin);
        }
        RequestFormat::Auto => {
            if let Some(strategy) = inferred {
                push_unique_refresh_strategy(&mut strategies, strategy);
            }
        }
        _ => {
            push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::OpenAi);
        }
    }
    for strategy in [
        ModelRefreshStrategy::OpenAi,
        ModelRefreshStrategy::GeminiNative,
        ModelRefreshStrategy::AnthropicNative,
    ] {
        push_unique_refresh_strategy(&mut strategies, strategy);
    }
    if matches!(input.request_format, RequestFormat::Codex)
        || matches!(inferred, Some(ModelRefreshStrategy::CodexBuiltin))
    {
        push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::CodexBuiltin);
    }
    strategies
}

async fn fetch_models_with_strategy(
    input: &RefreshModelsInput,
    strategy: ModelRefreshStrategy,
) -> Result<Vec<String>, String> {
    match strategy {
        ModelRefreshStrategy::OpenAi => fetch_models_openai(input).await,
        ModelRefreshStrategy::GeminiNative => fetch_models_gemini_native(input).await,
        ModelRefreshStrategy::AnthropicNative => fetch_models_anthropic(input).await,
        ModelRefreshStrategy::CodexBuiltin => Ok(codex_builtin_models()),
    }
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
    state: State<'_, AppState>,
    input: FetchModelMetadataInput,
) -> Result<FetchModelMetadataOutput, String> {
    let requested_model = input.model.trim();
    if requested_model.is_empty() {
        return Err("Model is empty.".to_string());
    }
    let cache = read_models_dev_cache_only(&state)?;
    let root = cache.root;
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
async fn refresh_models(
    state: State<'_, AppState>,
    input: RefreshModelsInput,
) -> Result<Vec<String>, String> {
    let inferred_strategy = inferred_model_refresh_strategy_from_base_url(&input.base_url);
    let can_refresh_without_api_key = input.request_format.is_codex()
        || matches!(inferred_strategy, Some(ModelRefreshStrategy::CodexBuiltin));
    if !can_refresh_without_api_key && input.api_key.trim().is_empty() {
        return Err("API key is empty.".to_string());
    }
    if input.base_url.trim().is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    if let Err(err) = ensure_models_dev_cache_current(&state).await {
        eprintln!(
            "[models.dev缓存] 刷新模型时更新元数据缓存失败: error={:?}",
            err
        );
    }

    match input.request_format {
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
        RequestFormat::OpenAIRerank | RequestFormat::GeminiEmbedding => Err(
            "Request format is for embedding/rerank and does not support model list refresh."
                .to_string(),
        ),
        _ => {
            let mut errors = Vec::<String>::new();
            for strategy in model_refresh_strategies(&input) {
                match fetch_models_with_strategy(&input, strategy).await {
                    Ok(models) => return Ok(models),
                    Err(err) => errors.push(format!("{strategy:?}: {err}")),
                }
            }
            Err(format!("Refresh model list failed: {}", errors.join(" | ")))
        }
    }
}

#[tauri::command]
async fn quick_genai_chat(
    state: State<'_, AppState>,
    input: QuickGenaiChatInput,
) -> Result<String, String> {
    let base_url = input.base_url.trim();
    let api_key = input.api_key.trim();
    let model = input.model.trim();
    let prompt = input.prompt.trim();
    if base_url.is_empty() {
        return Err("Base URL is empty.".to_string());
    }
    if api_key.is_empty() {
        return Err("API key is empty.".to_string());
    }
    if model.is_empty() {
        return Err("Model is empty.".to_string());
    }
    if prompt.is_empty() {
        return Err("Prompt is empty.".to_string());
    }
    if !input.request_format.is_chat_text() {
        return Err(format!(
            "Request format '{}' is not a chat text format.",
            input.request_format
        ));
    }

    let resolved_api = ResolvedApiConfig {
        provider_id: input.provider_id,
        provider_api_keys: vec![api_key.to_string()],
        provider_key_cursor: 0,
        request_format: input.request_format,
        allow_concurrent_requests: true,
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        reasoning_effort: None,
        temperature: Some(0.0),
        max_output_tokens: Some(16),
        prompt_cache_key: None,
        extra_headers: Vec::new(),
        codex_auth: None,
    };
    let prepared = PreparedPrompt {
        preamble: String::new(),
        history_messages: Vec::new(),
        latest_user_text: prompt.to_string(),
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };
    let started_at = std::time::Instant::now();
    let reply = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        call_model_openai_non_stream(&resolved_api, model, prepared, Some(&state)),
    )
    .await
    .map_err(|_| "Quick setup connectivity test timed out.".to_string())??;
    push_llm_round_log(
        Some(&state),
        None,
        None,
        "Quick setup connectivity test",
        resolved_api.request_format,
        resolved_api
            .provider_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("quick-setup"),
        model,
        &resolved_api.base_url,
        masked_auth_headers(&resolved_api.api_key),
        None,
        Some(model_reply_to_log_value(&reply)),
        None,
        started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        None,
    );
    let final_text = reply.final_response_text.trim();
    let text = if final_text.is_empty() {
        reply.assistant_text.trim().to_string()
    } else {
        final_text.to_string()
    };
    Ok(text)
}

#[tauri::command]
async fn resolve_model_adapter_kind(model_name: String) -> Result<String, String> {
    let stripped = model_name
        .split(['/', ':'])
        .last()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&model_name);
    match genai::adapter::AdapterKind::from_model(stripped) {
        // Ollama 在本应用里使用 OpenAI-compatible 接口刷新/调用模型。
        Ok(genai::adapter::AdapterKind::Ollama) => Ok(genai::adapter::AdapterKind::OpenAI.to_string()),
        Ok(kind) => Ok(kind.to_string()),
        Err(err) => {
            eprintln!(
                "[模型适配器] 状态=回退 模型={} 适配器={} 原因={:?}",
                stripped,
                genai::adapter::AdapterKind::OpenAI,
                err
            );
            Ok(genai::adapter::AdapterKind::OpenAI.to_string())
        }
    }
}
