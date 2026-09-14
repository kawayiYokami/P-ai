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
                runtime_log_error(format!(
                    "[models.dev缓存] 刷新失败，回退旧缓存: error={:?}, updated_at={}, fetched_at_ms={}",
                    err, cache.updated_at, cache.fetched_at_ms
                ));
                Ok(cache)
            }
        },
        None => {
            let root = fetch_models_dev_root(state).await?;
            write_models_dev_cache_file(state, &root)
        }
    }
}

fn read_models_dev_cache_only(state: &AppState) -> Result<Option<ModelsDevCacheFile>, String> {
    read_models_dev_cache_file(state)
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
    GenaiAdapter(genai::adapter::AdapterKind),
}

fn codex_builtin_models() -> Vec<String> {
    vec![
        "gpt-6-astra".to_string(),
        "gpt-5.6-sol".to_string(),
        "gpt-5.6-terra".to_string(),
        "gpt-5.6-luna".to_string(),
        "gpt-5.5".to_string(),
        "gpt-5.4".to_string(),
        "gpt-5.4-mini".to_string(),
        "gpt-5.3-codex".to_string(),
    ]
}

#[tauri::command]
fn codex_get_builtin_models() -> Vec<String> {
    codex_builtin_models()
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
    let adapter_kind = resolve_adapter_kind_from_base_url(base_url)?;
    Some(match adapter_kind {
        genai::adapter::AdapterKind::OpenAIResp => ModelRefreshStrategy::CodexBuiltin,
        genai::adapter::AdapterKind::Gemini => ModelRefreshStrategy::GeminiNative,
        genai::adapter::AdapterKind::Anthropic => ModelRefreshStrategy::AnthropicNative,
        genai::adapter::AdapterKind::OpenAI | genai::adapter::AdapterKind::DeepSeek => {
            ModelRefreshStrategy::OpenAi
        }
        adapter_kind => ModelRefreshStrategy::GenaiAdapter(adapter_kind),
    })
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
        RequestFormat::MimoAsr => {
            push_unique_refresh_strategy(
                &mut strategies,
                ModelRefreshStrategy::GenaiAdapter(genai::adapter::AdapterKind::Mimo),
            );
            push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::OpenAi);
        }
        RequestFormat::Baidu | RequestFormat::BedrockApi | RequestFormat::OpenCodeGo => {
            if let Some(adapter_kind) = input.request_format.genai_adapter_kind() {
                push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::GenaiAdapter(adapter_kind));
            }
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
    if let Some(adapter_kind) = input.request_format.genai_adapter_kind() {
        push_unique_refresh_strategy(&mut strategies, ModelRefreshStrategy::GenaiAdapter(adapter_kind));
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
        ModelRefreshStrategy::GenaiAdapter(adapter_kind) => fetch_models_genai(input, adapter_kind).await,
    }
}

async fn fetch_models_genai(
    input: &RefreshModelsInput,
    adapter_kind: genai::adapter::AdapterKind,
) -> Result<Vec<String>, String> {
    let api_key = input.api_key.trim().to_string();
    if api_key.contains('\r') || api_key.contains('\n') {
        return Err("API key contains newline characters. Please paste a single-line token.".to_string());
    }
    if matches!(api_key.as_str(), "..." | "***" | "•••" | "···") {
        return Err("API key is still a placeholder ('...' / '***'). Please paste the real token.".to_string());
    }
    let mut provider_config = genai::resolver::ProviderConfig::from_auth(
        genai::resolver::AuthData::from_single(api_key),
    );
    let endpoint = normalize_provider_genai_base_url(adapter_kind, &input.base_url);
    if !endpoint.trim().is_empty() {
        provider_config = provider_config.with_endpoint(genai::resolver::Endpoint::from_owned(endpoint));
    }
    let client = genai::Client::builder()
        .with_adapter_kind(adapter_kind)
        .build()
        .map_err(|err| format!("构建 genai 客户端失败: {err}"))?;
    let mut models = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        client.all_model_names(adapter_kind, provider_config),
    )
    .await
    .map_err(|_| format!("Fetch genai model list timed out: {adapter_kind}"))?
    .map_err(|err| format!("Fetch genai model list failed ({adapter_kind}): {err}"))?;
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

#[derive(Debug, Clone)]
struct ModelMetadataCandidate {
    provider_name: String,
    provider_api: String,
    model_id: String,
    context_window_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
    enable_image: bool,
    enable_tools: bool,
    enable_audio: bool,
    enable_video: bool,
    reasoning: Option<bool>,
    reasoning_effort_options: Vec<String>,
    documentation_url: Option<String>,
}

fn normalize_model_metadata_base_url(value: &str) -> String {
    let raw = value.trim();
    let Ok(mut parsed) = reqwest::Url::parse(raw) else {
        return raw
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string();
    };
    let scheme = parsed.scheme().to_ascii_lowercase();
    let _ = parsed.set_scheme(&scheme);
    if let Some(host) = parsed.host_str() {
        let _ = parsed.set_host(Some(&host.to_ascii_lowercase()));
    }
    let normalized_path = parsed.path().trim_end_matches('/').to_string();
    if normalized_path.eq_ignore_ascii_case("/v1") || normalized_path == "/" {
        parsed.set_path("");
    } else {
        parsed.set_path(&normalized_path);
    }
    parsed.to_string().trim_end_matches('/').to_string()
}

fn select_model_metadata_candidates<'a>(
    candidates: &'a [ModelMetadataCandidate],
    requested_base_url: &str,
) -> (Vec<&'a ModelMetadataCandidate>, Vec<&'a ModelMetadataCandidate>, bool) {
    let url_matched = candidates
        .iter()
        .filter(|candidate| {
            !requested_base_url.is_empty()
                && normalize_model_metadata_base_url(&candidate.provider_api) == requested_base_url
        })
        .collect::<Vec<_>>();
    if url_matched.is_empty() {
        (candidates.iter().collect(), Vec::new(), false)
    } else {
        (url_matched.clone(), url_matched, true)
    }
}

fn parse_documentation_url(model_obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in [
        "documentation_url",
        "documentationUrl",
        "docs_url",
        "docsUrl",
        "doc_url",
        "docUrl",
        "docs",
        "doc",
        "website",
        "reference",
        "url",
    ] {
        if let Some(value) = model_obj.get(key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn parse_reasoning_effort_options(model_obj: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut values = Vec::<String>::new();
    let mut has_toggle = false;
    let Some(items) = model_obj.get("reasoning_options").and_then(Value::as_array) else {
        return values;
    };
    for item in items {
        let Some(entry) = item.as_object() else {
            continue;
        };
        let option_type = entry
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        // toggle 表示思考可开关（如 DeepSeek 的 non-thinking mode），映射为可关闭档位 none。
        if option_type == "toggle" {
            has_toggle = true;
            continue;
        }
        if option_type != "effort" {
            continue;
        }
        let Some(raw_values) = entry.get("values") else {
            continue;
        };
        let Some(raw_values) = raw_values.as_array() else {
            continue;
        };
        for raw_value in raw_values {
            let normalized = raw_value
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            if normalized.is_empty() || values.iter().any(|item| item == &normalized) {
                continue;
            }
            values.push(normalized);
        }
    }
    // 思考可开关的模型在无 none 档位时补充 none，让前端可以显式关闭思考。
    if has_toggle && !values.iter().any(|item| item == "none") {
        values.insert(0, "none".to_string());
    }
    values
}

fn merge_reasoning_flag(selected_candidates: &[&ModelMetadataCandidate]) -> Option<bool> {
    if selected_candidates.iter().any(|candidate| candidate.reasoning == Some(true)) {
        Some(true)
    } else if selected_candidates
        .iter()
        .any(|candidate| candidate.reasoning == Some(false))
    {
        Some(false)
    } else {
        None
    }
}

fn merge_reasoning_effort_options(selected_candidates: &[&ModelMetadataCandidate]) -> Vec<String> {
    let mut merged = Vec::<String>::new();
    for candidate in selected_candidates {
        for value in &candidate.reasoning_effort_options {
            if merged.iter().any(|item| item == value) {
                continue;
            }
            merged.push(value.clone());
        }
    }
    merged
}

fn merge_documentation_url(selected_candidates: &[&ModelMetadataCandidate]) -> Option<String> {
    selected_candidates
        .iter()
        .filter_map(|candidate| candidate.documentation_url.as_ref())
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn merge_model_metadata_candidates(
    selected_candidates: &[&ModelMetadataCandidate],
    documentation_candidates: &[&ModelMetadataCandidate],
    exact_match: bool,
) -> FetchModelMetadataOutput {
    let provider = selected_candidates
        .first()
        .map(|candidate| candidate.provider_name.clone())
        .filter(|value| !value.is_empty());
    let api = selected_candidates
        .first()
        .map(|candidate| candidate.provider_api.clone())
        .filter(|value| !value.is_empty());
    FetchModelMetadataOutput {
        found: true,
        fuzzy_match: !exact_match,
        provider_name: if exact_match { provider } else { None },
        provider_api: if exact_match { api } else { None },
        matched_model_id: selected_candidates
            .first()
            .map(|candidate| candidate.model_id.clone()),
        context_window_tokens: selected_candidates
            .iter()
            .filter_map(|candidate| candidate.context_window_tokens)
            .max(),
        max_output_tokens: selected_candidates
            .iter()
            .filter_map(|candidate| candidate.max_output_tokens)
            .max(),
        enable_image: Some(selected_candidates.iter().any(|candidate| candidate.enable_image)),
        enable_tools: Some(selected_candidates.iter().any(|candidate| candidate.enable_tools)),
        enable_audio: Some(selected_candidates.iter().any(|candidate| candidate.enable_audio)),
        enable_video: Some(selected_candidates.iter().any(|candidate| candidate.enable_video)),
        reasoning: merge_reasoning_flag(selected_candidates),
        reasoning_effort_options: merge_reasoning_effort_options(selected_candidates),
        documentation_url: merge_documentation_url(documentation_candidates),
    }
}

#[tauri::command]
async fn fetch_model_metadata(
    state: State<'_, AppState>,
    input: FetchModelMetadataInput,
) -> Result<FetchModelMetadataOutput, String> {
    fetch_model_metadata_inner(&state, input).await
}

async fn fetch_model_metadata_inner(
    state: &AppState,
    input: FetchModelMetadataInput,
) -> Result<FetchModelMetadataOutput, String> {
    let requested_model = input.model.trim();
    if requested_model.is_empty() {
        return Err("Model is empty.".to_string());
    }
    let Some(cache) = read_models_dev_cache_only(&state)? else {
        return Ok(FetchModelMetadataOutput {
            found: false,
            fuzzy_match: false,
            provider_name: None,
            provider_api: None,
            matched_model_id: None,
            context_window_tokens: None,
            max_output_tokens: None,
            enable_image: None,
            enable_tools: None,
            enable_audio: None,
            enable_video: None,
            reasoning: None,
            reasoning_effort_options: Vec::new(),
            documentation_url: None,
        });
    };
    let root = cache.root;
    let providers = root
        .as_object()
        .ok_or_else(|| "Invalid models.dev payload: expected root object.".to_string())?;
    let requested_base_url = normalize_model_metadata_base_url(&input.base_url);
    let mut candidates = Vec::<ModelMetadataCandidate>::new();
    for (_, provider_value) in providers {
        let Some(provider_obj) = provider_value.as_object() else {
            continue;
        };
        let Some(models_obj) = provider_obj.get("models").and_then(Value::as_object) else {
            continue;
        };
        let provider_api = provider_obj
            .get("api")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let provider_name = provider_obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
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
            let enable_video = input_modalities.iter().any(|item| item.contains("video"));
            let enable_tools = model_obj
                .get("tool_call")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let reasoning = model_obj.get("reasoning").and_then(Value::as_bool);
            let reasoning_effort_options = parse_reasoning_effort_options(model_obj);
            let documentation_url = parse_documentation_url(provider_obj)
                .or_else(|| parse_documentation_url(model_obj));
            candidates.push(ModelMetadataCandidate {
                provider_name: provider_name.clone(),
                provider_api: provider_api.clone(),
                model_id: model_id.to_string(),
                context_window_tokens,
                max_output_tokens,
                enable_image,
                enable_tools,
                enable_audio,
                enable_video,
                reasoning,
                reasoning_effort_options,
                documentation_url,
            });
        }
    }
    if candidates.is_empty() {
        return Ok(FetchModelMetadataOutput {
            found: false,
            fuzzy_match: false,
            provider_name: None,
            provider_api: None,
            matched_model_id: None,
            context_window_tokens: None,
            max_output_tokens: None,
            enable_image: None,
            enable_tools: None,
            enable_audio: None,
            enable_video: None,
            reasoning: None,
            reasoning_effort_options: Vec::new(),
            documentation_url: None,
        });
    }
    let (selected_candidates, documentation_candidates, exact_match) =
        select_model_metadata_candidates(&candidates, &requested_base_url);
    let merged = merge_model_metadata_candidates(&selected_candidates, &documentation_candidates, exact_match);
    Ok(merged)
}

#[tauri::command]
async fn refresh_models(
    state: State<'_, AppState>,
    input: RefreshModelsInput,
) -> Result<Vec<String>, String> {
    refresh_models_inner(&state, input).await
}

async fn refresh_models_inner(
    state: &AppState,
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
        runtime_log_error(format!(
            "[models.dev缓存] 刷新模型时更新元数据缓存失败: error={:?}",
            err
        ));
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
                    Err(err) => {
                        errors.push(format!("{strategy:?}: {err}"));
                    }
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
    quick_genai_chat_inner(&state, input).await
}

async fn quick_genai_chat_inner(
    state: &AppState,
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
        max_concurrent_requests: None,
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        model: model.to_string(),
        reasoning_effort: None,
        temperature: None,
        max_output_tokens: Some(16),
        prompt_cache_key: None,
        extra_headers: Vec::new(),
        codex_auth: None,
        codex_custom_api_key: None,
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
        call_model_genai_non_stream(&resolved_api, model, prepared, Some(&state), None),
    )
    .await
    .map_err(|_| "Quick setup connectivity test timed out.".to_string())??;
    // 已切换至调度事件：旧 quick setup 日志不再写入
    let _ = (&reply, &started_at);
    let final_text = reply.final_response_text.trim();
    let text = if final_text.is_empty() {
        reply.assistant_text.trim().to_string()
    } else {
        final_text.to_string()
    };
    Ok(text)
}

fn resolve_model_adapter_kind_label(
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> String {
    resolve_model_protocol(
        request_format,
        base_url,
        model_name,
        genai::adapter::AdapterKind::OpenAI,
    )
    .adapter_kind
    .to_string()
}

#[tauri::command]
async fn resolve_model_adapter_kind(
    model_name: String,
    base_url: Option<String>,
    request_format: Option<RequestFormat>,
) -> Result<String, String> {
    Ok(resolve_model_adapter_kind_label(
        request_format.unwrap_or(RequestFormat::Auto),
        base_url.as_deref().unwrap_or_default(),
        &model_name,
    ))
}

#[cfg(test)]
mod model_adapter_kind_tests {
    use super::*;

    #[test]
    fn resolve_model_adapter_kind_label_should_follow_auto_url_then_model() {
        assert_eq!(
            resolve_model_adapter_kind_label(
                RequestFormat::Auto,
                "https://opencode.ai/zen/go/v1",
                "qwen3.7-plus",
            ),
            "OpenCodeGo"
        );
        assert_eq!(
            resolve_model_adapter_kind_label(
                RequestFormat::Auto,
                "https://example.com/v1",
                "minimax-m3:free",
            ),
            "MiniMax"
        );
    }
}

#[tauri::command]
async fn test_embedding_connection(
    _state: State<'_, AppState>,
    input: TestEmbeddingConnectionInput,
) -> Result<TestEmbeddingConnectionResult, String> {
    test_embedding_connection_inner(input).await
}

async fn test_embedding_connection_inner(
    input: TestEmbeddingConnectionInput,
) -> Result<TestEmbeddingConnectionResult, String> {
    let base_url = input.base_url.trim();
    let api_key = input.api_key.trim();
    let model = input.model.trim();
    if base_url.is_empty() {
        return Err("Base URL is empty.".to_string());
    }
    if api_key.is_empty() {
        return Err("API key is empty.".to_string());
    }
    if model.is_empty() {
        return Err("Model name is empty.".to_string());
    }
    let kind = match input.request_format {
        RequestFormat::GeminiEmbedding => MemoryProviderKind::GeminiEmbedding,
        RequestFormat::OpenAIRerank => {
            return Err("Rerank format cannot be tested as embedding.".to_string());
        }
        _ => MemoryProviderKind::OpenAIEmbedding,
    };
    let cfg = MemoryProviderApiConfig {
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        model: model.to_string(),
    };
    let provider = memory_create_embedding_provider(kind, &cfg, Some(model))?;
    let started = std::time::Instant::now();
    let text = input
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("embedding connectivity test")
        .to_string();
    let vectors = tokio::task::spawn_blocking(move || provider.embed_batch(&[text]))
        .await
        .map_err(|err| format!("Embedding test failed: {err}"))?
        .map_err(|err| format!("Embedding test failed: {err}"))?;
    let dim = vectors.first().map(|v| v.len()).unwrap_or(0);
    if dim == 0 {
        return Err("Embedding returned zero-dim vector.".to_string());
    }
    let elapsed_ms = started.elapsed().as_millis();
    runtime_log_info(format!(
        "[连通性测试] 类型=嵌入 模型={} 向量维度={} 耗时={}ms",
        model, dim, elapsed_ms
    ));
    Ok(TestEmbeddingConnectionResult {
        vector_dim: dim,
        elapsed_ms,
    })
}

#[tauri::command]
async fn test_rerank_connection(
    _state: State<'_, AppState>,
    input: TestRerankConnectionInput,
) -> Result<TestRerankConnectionResult, String> {
    test_rerank_connection_inner(input).await
}

async fn test_rerank_connection_inner(
    input: TestRerankConnectionInput,
) -> Result<TestRerankConnectionResult, String> {
    let base_url = input.base_url.trim();
    let api_key = input.api_key.trim();
    let model = input.model.trim();
    if base_url.is_empty() {
        return Err("Base URL is empty.".to_string());
    }
    if model.is_empty() {
        return Err("Model name is empty.".to_string());
    }
    if !matches!(input.request_format, RequestFormat::OpenAIRerank) {
        return Err("Request format is not rerank.".to_string());
    }
    let cfg = MemoryProviderApiConfig {
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        model: model.to_string(),
    };
    let provider = memory_create_rerank_provider(MemoryProviderKind::VllmRerank, &cfg, Some(model))?;
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("rerank connectivity test")
        .to_string();
    let documents = input.documents.unwrap_or_else(|| {
        vec![
            "The user prefers concise answers with direct conclusions.".to_string(),
            "The weather forecast is unrelated to the current test.".to_string(),
            "Memory retrieval and rerank settings are being configured.".to_string(),
        ]
    });
    if documents.is_empty() {
        return Err("Rerank documents are empty.".to_string());
    }
    let started = std::time::Instant::now();
    let results = tokio::task::spawn_blocking(move || provider.rerank(&query, &documents, Some(3)))
        .await
        .map_err(|err| format!("Rerank test failed: {err}"))?
        .map_err(|err| format!("Rerank test failed: {err}"))?;
    if results.is_empty() {
        return Err("Rerank returned empty results.".to_string());
    }
    let elapsed_ms = started.elapsed().as_millis();
    runtime_log_info(format!(
        "[连通性测试] 类型=重排 模型={} 结果数={} 耗时={}ms",
        model,
        results.len(),
        elapsed_ms
    ));
    Ok(TestRerankConnectionResult {
        result_count: results.len(),
        elapsed_ms,
    })
}

#[tauri::command]
async fn test_voice_connection(input: TestVoiceConnectionInput) -> Result<TestVoiceConnectionResult, String> {
    test_voice_connection_inner(input).await
}

async fn test_voice_connection_inner(input: TestVoiceConnectionInput) -> Result<TestVoiceConnectionResult, String> {
    let base_url = input.base_url.trim();
    let api_key = input.api_key.trim();
    if base_url.is_empty() {
        return Err("Base URL is empty.".to_string());
    }
    if api_key.is_empty() {
        return Err("API key is empty.".to_string());
    }
    let is_tts = matches!(input.request_format, RequestFormat::OpenAITts);
    let models_url = {
        let base = base_url.trim_end_matches('/');
        let lower = base.to_ascii_lowercase();
        if lower.ends_with("/v1") {
            format!("{base}/models")
        } else if lower.ends_with("/audio/transcriptions") || lower.ends_with("/audio/speech") {
            let prefix = lower
                .rfind("/v1/")
                .map(|idx| &base[..idx])
                .unwrap_or_else(|| {
                    let suffix = if lower.ends_with("/audio/transcriptions") {
                        "/audio/transcriptions"
                    } else {
                        "/audio/speech"
                    };
                    &base[..base.len().saturating_sub(suffix.len())]
                })
                .trim_end_matches('/');
            format!("{prefix}/v1/models")
        } else {
            format!("{base}/v1/models")
        }
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let started = std::time::Instant::now();
    let resp = client
        .get(&models_url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("Voice endpoint unreachable: {err}"))?;
    let elapsed_ms = started.elapsed().as_millis();
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let snippet = body.chars().take(300).collect::<String>();
        return Err(format!("{status}: {snippet}"));
    }
    let kind = if is_tts { "TTS" } else { "STT" };
    runtime_log_info(format!(
        "[连通性测试] 类型={} 耗时={}ms",
        kind, elapsed_ms
    ));
    Ok(TestVoiceConnectionResult { elapsed_ms })
}

#[cfg(test)]
mod model_metadata_selection_tests {
    use super::*;

    fn candidate(
        provider_api: &str,
        context_window_tokens: u32,
        max_output_tokens: u32,
        enable_audio: bool,
        enable_video: bool,
    ) -> ModelMetadataCandidate {
        ModelMetadataCandidate {
            provider_name: "mimo".to_string(),
            provider_api: provider_api.to_string(),
            model_id: "mimo-v2.5".to_string(),
            context_window_tokens: Some(context_window_tokens),
            max_output_tokens: Some(max_output_tokens),
            enable_image: true,
            enable_tools: true,
            enable_audio,
            enable_video,
            reasoning: None,
            reasoning_effort_options: Vec::new(),
            documentation_url: None,
        }
    }

    #[test]
    fn codex_builtin_models_should_keep_gpt_55_and_gpt_56_models() {
        let models = codex_builtin_models();

        for model in [
            "gpt-5.6-sol",
            "gpt-5.6-terra",
            "gpt-5.6-luna",
            "gpt-5.5",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.3-codex",
        ] {
            assert!(models.iter().any(|item| item == model), "missing model: {model}");
        }
    }

    #[test]
    fn model_metadata_should_prefer_candidates_with_exact_provider_api_url() {
        let candidates = vec![
            candidate("", 1_050_000, 131_100, false, false),
            candidate(
                "https://token-plan-cn.xiaomimimo.com/v1",
                1_048_576,
                131_072,
                true,
                true,
            ),
        ];
        let requested_base_url =
            normalize_model_metadata_base_url("https://token-plan-cn.xiaomimimo.com/v1/");

        let (selected, documentation_candidates, strategy) =
            select_model_metadata_candidates(&candidates, &requested_base_url);
        let merged = merge_model_metadata_candidates(&selected, &documentation_candidates, strategy);

        assert!(strategy, "URL 精准匹配时应为 exact_match");
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0].provider_api,
            "https://token-plan-cn.xiaomimimo.com/v1"
        );
        assert_eq!(merged.context_window_tokens, Some(1_048_576));
        assert_eq!(merged.enable_audio, Some(true));
        assert_eq!(merged.enable_video, Some(true));
    }

    #[test]
    fn model_metadata_should_treat_root_and_v1_as_same_provider_api() {
        let candidates = vec![candidate(
            "https://api.deepseek.com",
            256_000,
            8_192,
            true,
            false,
        )];
        let requested_base_url =
            normalize_model_metadata_base_url("https://api.deepseek.com/v1");

        let (selected, _documentation_candidates, strategy) =
            select_model_metadata_candidates(&candidates, &requested_base_url);

        assert!(strategy, "根路径与 v1 视为同一 provider 时应为 exact_match");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].provider_api, "https://api.deepseek.com");
    }

    #[test]
    fn model_metadata_should_keep_documentation_url_from_selected_candidate() {
        let mut exact = candidate(
            "https://api.deepseek.com",
            256_000,
            8_192,
            true,
            false,
        );
        exact.documentation_url = Some("https://api-docs.deepseek.com".to_string());
        let mut fallback = candidate("", 512_000, 16_384, false, false);
        fallback.documentation_url = Some("https://fallback.invalid".to_string());
        let requested_base_url =
            normalize_model_metadata_base_url("https://api.deepseek.com/v1");

        let binding = [fallback, exact];
        let (selected, documentation_candidates, strategy) =
            select_model_metadata_candidates(&binding, &requested_base_url);
        let merged = merge_model_metadata_candidates(&selected, &documentation_candidates, strategy);

        assert_eq!(
            merged.documentation_url.as_deref(),
            Some("https://api-docs.deepseek.com")
        );
    }

    #[test]
    fn model_metadata_should_merge_all_candidates_when_provider_api_url_is_unknown() {
        let mut first = candidate("", 1_050_000, 131_100, false, false);
        first.documentation_url = Some("https://wrong.example.com/docs".to_string());
        let mut second = candidate(
            "https://api.xiaomimimo.com/v1",
            1_048_576,
            131_072,
            true,
            true,
        );
        second.documentation_url = Some("https://api.xiaomimimo.com/docs".to_string());
        let candidates = vec![first, second];
        let requested_base_url = normalize_model_metadata_base_url("https://proxy.example.com/v1");

        let (selected, documentation_candidates, strategy) =
            select_model_metadata_candidates(&candidates, &requested_base_url);
        let merged = merge_model_metadata_candidates(&selected, &documentation_candidates, strategy);

        assert!(!strategy, "URL 未匹配时应为模糊合并");
        assert_eq!(selected.len(), 2);
        assert_eq!(merged.context_window_tokens, Some(1_050_000));
        assert_eq!(merged.max_output_tokens, Some(131_100));
        assert_eq!(merged.enable_audio, Some(true));
        assert_eq!(merged.enable_video, Some(true));
        assert_eq!(merged.documentation_url, None);
    }

    #[test]
    fn model_metadata_base_url_should_treat_v1_and_root_as_same_provider_api() {
        assert_eq!(
            normalize_model_metadata_base_url("https://api.example.com/V1"),
            normalize_model_metadata_base_url("https://api.example.com/v1"),
        );
    }

    #[test]
    fn parse_reasoning_effort_options_should_add_none_when_toggle_present() {
        // DeepSeek 声明：toggle(可关闭思考) + effort(low/high/max)
        let obj = serde_json::json!({
            "reasoning_options": [
                { "type": "toggle" },
                { "type": "effort", "values": ["low", "high", "max"] }
            ]
        });
        let map = obj.as_object().unwrap().clone();
        let options = parse_reasoning_effort_options(&map);
        assert_eq!(options, vec!["none", "low", "high", "max"]);
    }

    #[test]
    fn parse_reasoning_effort_options_should_not_duplicate_none() {
        let obj = serde_json::json!({
            "reasoning_options": [
                { "type": "toggle" },
                { "type": "effort", "values": ["none", "high"] }
            ]
        });
        let map = obj.as_object().unwrap().clone();
        let options = parse_reasoning_effort_options(&map);
        assert_eq!(options, vec!["none", "high"]);
    }

    #[test]
    fn parse_reasoning_effort_options_without_toggle_should_keep_effort_values() {
        let obj = serde_json::json!({
            "reasoning_options": [
                { "type": "effort", "values": ["low", "high"] }
            ]
        });
        let map = obj.as_object().unwrap().clone();
        let options = parse_reasoning_effort_options(&map);
        assert_eq!(options, vec!["low", "high"]);
    }
}
