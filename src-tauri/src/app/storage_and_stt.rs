fn ensure_parent_dir(path: &PathBuf) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Config path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("Create config directory failed: {err}"))
}

fn read_config(path: &PathBuf) -> Result<AppConfig, String> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(path).map_err(|err| format!("Read config failed: {err}"))?;
    let mut parsed = toml::from_str::<AppConfig>(&content).unwrap_or_default();
    normalize_app_config(&mut parsed);
    Ok(parsed)
}

fn write_config(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let toml_str =
        toml::to_string_pretty(config).map_err(|err| format!("Serialize config failed: {err}"))?;
    fs::write(path, toml_str).map_err(|err| format!("Write config failed: {err}"))
}

fn normalize_api_tools(config: &mut AppConfig) {
    for api in &mut config.api_configs {
        api.enable_audio = false;
        api.temperature = api.temperature.clamp(0.0, 2.0);
        api.context_window_tokens = api.context_window_tokens.clamp(16_000, 200_000);
        if api.enable_tools {
            if api.tools.is_empty() {
                api.tools = default_api_tools();
            } else {
                let defaults = default_api_tools();
                for d in defaults {
                    if !api.tools.iter().any(|t| t.id == d.id) {
                        api.tools.push(d);
                    }
                }
            }
        }
    }
}

fn normalize_app_config(config: &mut AppConfig) {
    if config.api_configs.is_empty() {
        *config = AppConfig::default();
        return;
    }
    ensure_hotkey_config_normalized(config);

    normalize_api_tools(config);

    if !config
        .api_configs
        .iter()
        .any(|a| a.id == config.selected_api_config_id)
    {
        config.selected_api_config_id = config.api_configs[0].id.clone();
    }

    let chat_valid = config
        .api_configs
        .iter()
        .any(|a| a.id == config.chat_api_config_id && a.enable_text);
    if !chat_valid {
        if let Some(api) = config.api_configs.iter().find(|a| a.enable_text) {
            config.chat_api_config_id = api.id.clone();
        } else {
            config.chat_api_config_id = config.api_configs[0].id.clone();
        }
    }

    if config.record_hotkey.trim().is_empty() {
        config.record_hotkey = default_record_hotkey();
    }
    if config.min_record_seconds == 0 {
        config.min_record_seconds = default_min_record_seconds();
    }
    if config.max_record_seconds < config.min_record_seconds {
        config.max_record_seconds = default_max_record_seconds().max(config.min_record_seconds);
    }
    config.tool_max_iterations = config.tool_max_iterations.clamp(1, 100);

    config.vision_api_config_id = config
        .vision_api_config_id
        .as_deref()
        .filter(|id| {
            config
                .api_configs
                .iter()
                .any(|a| a.id == *id && a.enable_image)
        })
        .map(ToOwned::to_owned);
}

fn read_app_data(path: &PathBuf) -> Result<AppData, String> {
    if !path.exists() {
        return Ok(AppData::default());
    }

    let content = fs::read_to_string(path).map_err(|err| format!("Read app_data failed: {err}"))?;
    let mut parsed = serde_json::from_str::<AppData>(&content).unwrap_or_default();
    parsed.version = APP_DATA_SCHEMA_VERSION;
    ensure_default_agent(&mut parsed);
    Ok(parsed)
}

fn write_app_data(path: &PathBuf, data: &AppData) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let body = serde_json::to_string_pretty(data)
        .map_err(|err| format!("Serialize app_data failed: {err}"))?;
    fs::write(path, body).map_err(|err| format!("Write app_data failed: {err}"))
}

fn candidate_debug_config_paths() -> Vec<PathBuf> {
    vec![PathBuf::from(".debug").join("api-key.json")]
}

fn read_debug_api_config() -> Result<Option<DebugApiConfig>, String> {
    for path in candidate_debug_config_paths() {
        if !path.exists() {
            continue;
        }

        let content = fs::read_to_string(&path)
            .map_err(|err| format!("Read debug config failed ({}): {err}", path.display()))?;
        let parsed = serde_json::from_str::<DebugApiConfig>(&content)
            .map_err(|err| format!("Parse debug config failed ({}): {err}", path.display()))?;
        return Ok(Some(parsed));
    }
    Ok(None)
}

fn resolve_selected_api_config(
    app_config: &AppConfig,
    requested_id: Option<&str>,
) -> Option<ApiConfig> {
    if app_config.api_configs.is_empty() {
        return None;
    }

    let target_id = requested_id
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(app_config.chat_api_config_id.as_str());

    if let Some(found) = app_config.api_configs.iter().find(|p| p.id == target_id) {
        return Some(found.clone());
    }

    app_config.api_configs.first().cloned()
}

fn resolve_api_config(
    app_config: &AppConfig,
    requested_id: Option<&str>,
) -> Result<ResolvedApiConfig, String> {
    if let Some(debug_cfg) = read_debug_api_config()? {
        let enabled = debug_cfg.enabled.unwrap_or(true);
        let request_format_ok = debug_cfg
            .request_format
            .as_deref()
            .map(str::trim)
            .unwrap_or("openai")
            .eq_ignore_ascii_case("openai");

        if enabled && request_format_ok {
            if debug_cfg.api_key.trim().is_empty() {
                return Err(".debug/api-key.json exists but apiKey is empty.".to_string());
            }
            return Ok(ResolvedApiConfig {
                request_format: "openai".to_string(),
                base_url: debug_cfg.base_url.trim().to_string(),
                api_key: debug_cfg.api_key.trim().to_string(),
                model: debug_cfg.model.trim().to_string(),
                temperature: debug_cfg
                    .temperature
                    .unwrap_or(default_api_temperature())
                    .clamp(0.0, 2.0),
                fixed_test_prompt: debug_cfg
                    .fixed_test_prompt
                    .unwrap_or_else(|| "EASY_CALL_AI_CACHE_TEST_V1".to_string()),
            });
        }
    }

    let selected = resolve_selected_api_config(app_config, requested_id).ok_or_else(|| {
        "No API config configured. Please add at least one API config.".to_string()
    })?;

    if selected.api_key.trim().is_empty() {
        return Err(
            "Selected API config API key is empty. Please fill it in settings.".to_string(),
        );
    }

    Ok(ResolvedApiConfig {
        request_format: selected.request_format.trim().to_string(),
        base_url: selected.base_url.trim().to_string(),
        api_key: selected.api_key.trim().to_string(),
        model: selected.model.trim().to_string(),
        temperature: selected.temperature.clamp(0.0, 2.0),
        fixed_test_prompt: "EASY_CALL_AI_CACHE_TEST_V1".to_string(),
    })
}

fn resolve_vision_api_config(app_config: &AppConfig) -> Result<ApiConfig, String> {
    let vision_id = app_config.vision_api_config_id.as_deref().ok_or_else(|| {
        "Current chat API does not support image and no 图转文AI is configured.".to_string()
    })?;

    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == vision_id)
        .cloned()
        .ok_or_else(|| "Configured 图转文AI not found.".to_string())?;

    if !api.enable_image {
        return Err("Configured 图转文AI has image disabled.".to_string());
    }
    if api.base_url.trim().is_empty() {
        return Err("图转文AI Base URL is empty.".to_string());
    }
    if api.api_key.trim().is_empty() {
        return Err("图转文AI API key is empty.".to_string());
    }
    if api.model.trim().is_empty() {
        return Err("图转文AI model is empty.".to_string());
    }

    Ok(api)
}

fn decode_image_bytes(image: &BinaryPart) -> Result<Vec<u8>, String> {
    B64.decode(image.bytes_base64.trim())
        .map_err(|err| format!("Decode image base64 failed: {err}"))
}

fn compute_image_hash_hex(image: &BinaryPart) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let raw = decode_image_bytes(image)?;
    let mut hasher = Sha256::new();
    hasher.update(raw);
    Ok(format!("{:x}", hasher.finalize()))
}

fn find_image_text_cache(
    data: &AppData,
    hash: &str,
    vision_api_id: &str,
) -> Option<String> {
    data.image_text_cache
        .iter()
        .find(|entry| entry.hash == hash && entry.vision_api_id == vision_api_id)
        .map(|entry| entry.text.clone())
}

fn upsert_image_text_cache(data: &mut AppData, hash: &str, vision_api_id: &str, text: &str) {
    if let Some(entry) = data
        .image_text_cache
        .iter_mut()
        .find(|entry| entry.hash == hash && entry.vision_api_id == vision_api_id)
    {
        entry.text = text.to_string();
        entry.updated_at = now_iso();
        return;
    }

    data.image_text_cache.push(ImageTextCacheEntry {
        hash: hash.to_string(),
        vision_api_id: vision_api_id.to_string(),
        text: text.to_string(),
        updated_at: now_iso(),
    });
}

fn is_openai_style_request_format(request_format: &str) -> bool {
    matches!(request_format.trim(), "openai" | "deepseek/kimi")
}
