fn ensure_parent_dir(path: &PathBuf) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Config path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("Create config directory failed: {err}"))
}

fn read_config(path: &PathBuf) -> Result<AppConfig, String> {
    let resolved_path = if path.exists() {
        path.clone()
    } else {
        let legacy = path.with_file_name("config.toml");
        if legacy.exists() {
            legacy
        } else {
            return Ok(AppConfig::default());
        }
    };

    let content =
        fs::read_to_string(&resolved_path).map_err(|err| format!("Read config failed: {err}"))?;
    let mut parsed = toml::from_str::<AppConfig>(&content).map_err(|err| {
        eprintln!(
            "[CONFIG] Parse config failed ({}): {err}",
            resolved_path.display()
        );
        format!("Parse config failed ({}): {err}", resolved_path.display())
    })?;
    normalize_app_config(&mut parsed);
    if resolved_path != *path {
        let _ = write_config(path, &parsed);
    }
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

fn trim_wrapping_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &trimmed[1..trimmed.len() - 1];
        }
    }
    trimmed
}

fn resolve_user_home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn expand_home_prefix(value: &str) -> String {
    if value == "~" {
        return resolve_user_home_dir()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| value.to_string());
    }
    let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    else {
        return value.to_string();
    };
    let Some(home) = resolve_user_home_dir() else {
        return value.to_string();
    };
    if rest.trim().is_empty() {
        return home.to_string_lossy().to_string();
    }
    home.join(rest).to_string_lossy().to_string()
}

#[cfg(target_os = "windows")]
fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

#[cfg(target_os = "windows")]
fn try_convert_git_bash_drive_path(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return None;
    }
    if bytes.len() > 2 && bytes[2] != b'/' && bytes[2] != b'\\' {
        return None;
    }
    let drive = (bytes[1] as char).to_ascii_uppercase();
    let rest = value[2..].trim_start_matches(['/', '\\']);
    if rest.is_empty() {
        return Some(format!(r"{drive}:\"));
    }
    Some(format!(r"{drive}:\{}", rest.replace('/', "\\")))
}

#[cfg(target_os = "windows")]
fn normalize_windows_path_input(value: &str) -> String {
    let mut text = value.trim().to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", rest.replace('/', "\\"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        text = rest.to_string();
    }
    if let Some(converted) = try_convert_git_bash_drive_path(&text) {
        return converted;
    }
    if text.starts_with("//") {
        return text.replace('/', "\\");
    }
    if has_windows_drive_prefix(&text) {
        return text.replace('/', "\\");
    }
    text
}

fn normalize_terminal_path_input_for_current_platform(raw: &str) -> String {
    let unquoted = trim_wrapping_quotes(raw);
    if unquoted.is_empty() {
        return String::new();
    }
    let expanded = expand_home_prefix(unquoted);
    #[cfg(target_os = "windows")]
    {
        normalize_windows_path_input(&expanded)
    }
    #[cfg(not(target_os = "windows"))]
    {
        expanded
    }
}

fn normalize_shell_workspaces(config: &mut AppConfig) {
    let mut normalized = Vec::<ShellWorkspaceConfig>::new();
    let mut seen_names = std::collections::HashSet::<String>::new();
    for raw in &config.shell_workspaces {
        let name = raw.name.trim().to_string();
        let mut normalized_path = normalize_terminal_path_input_for_current_platform(&raw.path);
        if name.is_empty() || normalized_path.is_empty() {
            continue;
        }
        let name_key = name.to_ascii_lowercase();
        if !seen_names.insert(name_key) {
            continue;
        }
        if normalized_path.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(&normalized_path);
        if candidate.is_absolute() {
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.is_dir() {
                    normalized_path = canonical.to_string_lossy().to_string();
                }
            }
        }
        normalized.push(ShellWorkspaceConfig {
            name,
            path: normalized_path,
            built_in: raw.built_in,
        });
    }
    config.shell_workspaces = normalized;
}

fn normalize_mcp_servers(config: &mut AppConfig) {
    let mut out = Vec::<McpServerConfig>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for raw in &config.mcp_servers {
        let id = raw.id.trim().to_string();
        let mut name = raw.name.trim().to_string();
        let definition_json = raw.definition_json.trim().to_string();
        if id.is_empty() || definition_json.is_empty() {
            continue;
        }
        let key = id.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        if name.is_empty() {
            name = id.clone();
        }
        let mut tool_policies = Vec::<McpToolPolicy>::new();
        let mut seen_tools = std::collections::HashSet::<String>::new();
        for policy in &raw.tool_policies {
            let tool_name = policy.tool_name.trim().to_string();
            if tool_name.is_empty() {
                continue;
            }
            let tool_key = tool_name.to_ascii_lowercase();
            if !seen_tools.insert(tool_key) {
                continue;
            }
            tool_policies.push(McpToolPolicy {
                tool_name,
                enabled: policy.enabled,
            });
        }
        let mut cached_tools = Vec::<McpCachedTool>::new();
        let mut seen_cached_tool_names = std::collections::HashSet::<String>::new();
        for cached in &raw.cached_tools {
            let tool_name = cached.tool_name.trim().to_string();
            if tool_name.is_empty() {
                continue;
            }
            let key = tool_name.to_ascii_lowercase();
            if !seen_cached_tool_names.insert(key) {
                continue;
            }
            cached_tools.push(McpCachedTool {
                tool_name,
                description: cached.description.trim().to_string(),
            });
        }
        out.push(McpServerConfig {
            id,
            name,
            enabled: raw.enabled,
            definition_json,
            tool_policies,
            cached_tools,
            last_status: raw.last_status.trim().to_string(),
            last_error: raw.last_error.trim().to_string(),
            updated_at: raw.updated_at.trim().to_string(),
        });
    }
    config.mcp_servers = out;
}

fn normalize_app_config(config: &mut AppConfig) {
    if config.api_configs.is_empty() {
        *config = AppConfig::default();
        return;
    }
    ensure_hotkey_config_normalized(config);
    let lang = config.ui_language.trim();
    config.ui_language = match lang {
        "zh-CN" | "en-US" | "zh-TW" => lang.to_string(),
        _ => default_ui_language(),
    };
    let ui_font = config.ui_font.trim();
    config.ui_font = if ui_font.is_empty() {
        default_ui_font()
    } else {
        ui_font.chars().take(128).collect::<String>()
    };

    normalize_api_tools(config);

    if let Some(stt_id) = config.stt_api_config_id.clone() {
        if let Some(api) = config.api_configs.iter_mut().find(|a| a.id == stt_id) {
            if matches!(api.request_format, RequestFormat::OpenAITts) {
                api.request_format = RequestFormat::OpenAIStt;
            }
        }
    }
    for api in &mut config.api_configs {
        if matches!(api.request_format, RequestFormat::Gemini) && !api.enable_text {
            api.request_format = RequestFormat::GeminiEmbedding;
        }
    }

    if !config
        .api_configs
        .iter()
        .any(|a| a.id == config.selected_api_config_id)
    {
        config.selected_api_config_id = config.api_configs[0].id.clone();
    }

    let chat_valid = config.api_configs.iter().any(|a| {
        a.id == config.chat_api_config_id
            && a.enable_text
            && a.request_format.is_chat_text()
    });
    if !chat_valid {
        if let Some(api) = config
            .api_configs
            .iter()
            .find(|a| a.enable_text && a.request_format.is_chat_text())
        {
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

    config.stt_api_config_id = config
        .stt_api_config_id
        .as_deref()
        .filter(|id| {
            config
                .api_configs
                .iter()
                .any(|a| a.id == *id && a.request_format.is_openai_stt())
        })
        .map(ToOwned::to_owned);
    if config.stt_api_config_id.is_none() {
        config.stt_auto_send = false;
    }
    normalize_shell_workspaces(config);
    normalize_mcp_servers(config);
}

const MEDIA_REF_PREFIX: &str = "@media:";
const MEDIA_BASE64_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_TEXT_CACHE_ENTRIES: usize = 1000;

#[derive(Default)]
struct MediaBase64Cache {
    entries: std::collections::HashMap<String, MediaBase64CacheEntry>,
    total_bytes: usize,
    seq: u64,
}

#[derive(Debug, Clone)]
struct MediaBase64CacheEntry {
    value: String,
    bytes: usize,
    seq: u64,
}

fn media_base64_cache() -> &'static Mutex<MediaBase64Cache> {
    static CACHE: OnceLock<Mutex<MediaBase64Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MediaBase64Cache::default()))
}

fn media_base64_cache_get(key: &str) -> Option<String> {
    let mut guard = media_base64_cache().lock().ok()?;
    guard.seq = guard.seq.saturating_add(1);
    let current_seq = guard.seq;
    let entry = guard.entries.get_mut(key)?;
    entry.seq = current_seq;
    Some(entry.value.clone())
}

fn media_base64_cache_put(key: String, value: String) {
    let bytes = value.len();
    if bytes > MEDIA_BASE64_CACHE_MAX_BYTES {
        return;
    }
    let Ok(mut guard) = media_base64_cache().lock() else {
        return;
    };
    if let Some(old) = guard.entries.remove(&key) {
        guard.total_bytes = guard.total_bytes.saturating_sub(old.bytes);
    }
    guard.seq = guard.seq.saturating_add(1);
    let seq = guard.seq;
    guard.entries.insert(
        key.clone(),
        MediaBase64CacheEntry {
            value,
            bytes,
            seq,
        },
    );
    guard.total_bytes = guard.total_bytes.saturating_add(bytes);

    while guard.total_bytes > MEDIA_BASE64_CACHE_MAX_BYTES {
        let Some(evict_key) = guard
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.seq)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        if let Some(removed) = guard.entries.remove(&evict_key) {
            guard.total_bytes = guard.total_bytes.saturating_sub(removed.bytes);
        } else {
            break;
        }
    }
}

fn media_storage_dir_from_data_path(data_path: &PathBuf) -> Result<PathBuf, String> {
    Ok(app_root_from_data_path(data_path).join("media"))
}

fn media_extension_from_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/webp" => "webp",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/heic" => "heic",
        "image/heif" => "heif",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "audio/wav" | "audio/wave" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/aiff" => "aiff",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

fn media_marker_from_id(media_id: &str) -> String {
    format!("{MEDIA_REF_PREFIX}{media_id}")
}

fn media_id_from_marker(value: &str) -> Option<&str> {
    value.trim().strip_prefix(MEDIA_REF_PREFIX)
}

fn persist_media_bytes(data_path: &PathBuf, mime: &str, raw: &[u8]) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    if raw.is_empty() {
        return Err("media payload is empty".to_string());
    }
    let mut hasher = Sha256::new();
    hasher.update(raw);
    let hash = format!("{:x}", hasher.finalize());
    let ext = media_extension_from_mime(mime);
    let media_id = format!("{hash}.{ext}");
    let dir = media_storage_dir_from_data_path(data_path)?;
    fs::create_dir_all(&dir).map_err(|err| format!("Create media directory failed: {err}"))?;
    let path = dir.join(&media_id);
    if !path.exists() {
        fs::write(&path, raw).map_err(|err| format!("Write media file failed: {err}"))?;
    }
    Ok(media_id)
}

fn resolve_stored_binary_base64(data_path: &PathBuf, stored: &str) -> Result<String, String> {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let Some(media_id) = media_id_from_marker(trimmed) else {
        return Ok(trimmed.to_string());
    };
    if let Some(hit) = media_base64_cache_get(trimmed) {
        return Ok(hit);
    }
    let dir = media_storage_dir_from_data_path(data_path)?;
    let path = dir.join(media_id);
    let raw = fs::read(&path).map_err(|err| {
        format!(
            "Read media file failed ({}): {err}",
            path.to_string_lossy()
        )
    })?;
    let encoded = B64.encode(raw);
    media_base64_cache_put(trimmed.to_string(), encoded.clone());
    Ok(encoded)
}

fn externalize_stored_binary_base64(
    data_path: &PathBuf,
    mime: &str,
    stored: &str,
) -> Result<String, String> {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if media_id_from_marker(trimmed).is_some() {
        return Ok(trimmed.to_string());
    }
    let raw = B64
        .decode(trimmed)
        .map_err(|err| format!("Decode media base64 failed: {err}"))?;
    let media_id = persist_media_bytes(data_path, mime, &raw)?;
    Ok(media_marker_from_id(&media_id))
}

fn externalize_message_parts_to_media_refs(
    parts: &mut [MessagePart],
    data_path: &PathBuf,
) -> Result<bool, String> {
    let mut changed = false;
    for part in parts {
        match part {
            MessagePart::Image {
                mime,
                bytes_base64,
                ..
            }
            | MessagePart::Audio {
                mime,
                bytes_base64,
                ..
            } => {
                let next = externalize_stored_binary_base64(data_path, mime, bytes_base64)?;
                if *bytes_base64 != next {
                    *bytes_base64 = next;
                    changed = true;
                }
            }
            MessagePart::Text { .. } => {}
        }
    }
    Ok(changed)
}

fn externalize_message_parts_to_media_refs_lossy(parts: &mut [MessagePart], data_path: &PathBuf) -> bool {
    let mut changed = false;
    for part in parts {
        match part {
            MessagePart::Image {
                mime,
                bytes_base64,
                ..
            }
            | MessagePart::Audio {
                mime,
                bytes_base64,
                ..
            } => {
                let Ok(next) = externalize_stored_binary_base64(data_path, mime, bytes_base64) else {
                    continue;
                };
                if *bytes_base64 != next {
                    *bytes_base64 = next;
                    changed = true;
                }
            }
            MessagePart::Text { .. } => {}
        }
    }
    changed
}

fn materialize_message_parts_from_media_refs(parts: &mut [MessagePart], data_path: &PathBuf) {
    for part in parts {
        match part {
            MessagePart::Image { bytes_base64, .. } | MessagePart::Audio { bytes_base64, .. } => {
                if media_id_from_marker(bytes_base64).is_none() {
                    continue;
                }
                match resolve_stored_binary_base64(data_path, bytes_base64) {
                    Ok(decoded) => *bytes_base64 = decoded,
                    Err(_) => {
                        bytes_base64.clear();
                    }
                }
            }
            MessagePart::Text { .. } => {}
        }
    }
}

fn materialize_chat_message_parts_from_media_refs(messages: &mut [ChatMessage], data_path: &PathBuf) {
    for message in messages {
        materialize_message_parts_from_media_refs(&mut message.parts, data_path);
    }
}

// app data layout + migration logic moved to features/config/app_data_layout.rs

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
            .unwrap_or(RequestFormat::OpenAI)
            .is_openai_style();

        if enabled && request_format_ok {
            if debug_cfg.api_key.trim().is_empty() {
                return Err(".debug/api-key.json exists but apiKey is empty.".to_string());
            }
            return Ok(ResolvedApiConfig {
                request_format: RequestFormat::OpenAI,
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
        request_format: selected.request_format,
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
    if data.image_text_cache.len() <= MAX_IMAGE_TEXT_CACHE_ENTRIES {
        return;
    }
    let Some((oldest_idx, _)) = data
        .image_text_cache
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.updated_at.cmp(&b.updated_at))
    else {
        return;
    };
    data.image_text_cache.remove(oldest_idx);
}

fn is_openai_style_request_format(request_format: RequestFormat) -> bool {
    request_format.is_openai_style()
}
