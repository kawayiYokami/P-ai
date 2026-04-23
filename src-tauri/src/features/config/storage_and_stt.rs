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

fn api_endpoint_id(provider_id: &str, model_id: &str) -> String {
    format!(
        "{}::{}",
        provider_id.trim().to_string(),
        model_id.trim().to_string()
    )
}

fn parse_api_endpoint_id(endpoint_id: &str) -> Option<(String, String)> {
    let trimmed = endpoint_id.trim();
    let (provider_id, model_id) = trimmed.split_once("::")?;
    let provider_id = provider_id.trim().to_string();
    let model_id = model_id.trim().to_string();
    if provider_id.is_empty() || model_id.is_empty() {
        return None;
    }
    Some((provider_id, model_id))
}

fn provider_key_cursor_state() -> &'static Mutex<std::collections::HashMap<String, usize>> {
    static CURSORS: OnceLock<Mutex<std::collections::HashMap<String, usize>>> = OnceLock::new();
    CURSORS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn peek_provider_api_key(provider: &ApiProviderConfig) -> String {
    let keys = provider
        .api_keys
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return String::new();
    }
    let Ok(guard) = provider_key_cursor_state().lock() else {
        let idx = (provider.key_cursor as usize) % keys.len();
        return keys[idx].to_string();
    };
    let idx = guard
        .get(&provider.id)
        .copied()
        .unwrap_or((provider.key_cursor as usize) % keys.len())
        % keys.len();
    keys[idx].to_string()
}

fn select_and_advance_provider_api_key(provider: &ApiProviderConfig) -> String {
    let keys = provider
        .api_keys
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return String::new();
    }
    let Ok(mut guard) = provider_key_cursor_state().lock() else {
        let idx = (provider.key_cursor as usize) % keys.len();
        return keys[idx].to_string();
    };
    let entry = guard
        .entry(provider.id.clone())
        .or_insert((provider.key_cursor as usize) % keys.len());
    let idx = *entry % keys.len();
    let selected = keys[idx].to_string();
    *entry = (idx + 1) % keys.len();
    selected
}

fn consume_api_key_for_request(resolved_api: &ResolvedApiConfig) -> String {
    let keys = resolved_api
        .provider_api_keys
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keys.is_empty() || resolved_api.provider_id.is_none() {
        return resolved_api.api_key.trim().to_string();
    }
    let provider = ApiProviderConfig {
        id: resolved_api.provider_id.clone().unwrap_or_default(),
        name: String::new(),
        request_format: resolved_api.request_format,
        allow_concurrent_requests: resolved_api.allow_concurrent_requests,
        enable_text: true,
        enable_image: false,
        enable_audio: false,
        enable_tools: false,
        tools: Vec::new(),
        base_url: resolved_api.base_url.clone(),
        codex_auth_mode: default_codex_auth_mode(),
        codex_local_auth_path: default_codex_local_auth_path(),
        api_keys: resolved_api.provider_api_keys.clone(),
        key_cursor: resolved_api.provider_key_cursor as u32,
        cached_model_options: Vec::new(),
        models: Vec::new(),
        failure_retry_count: 0,
    };
    let selected = select_and_advance_provider_api_key(&provider);
    if selected.trim().is_empty() {
        resolved_api.api_key.trim().to_string()
    } else {
        selected
    }
}

fn migrate_legacy_api_configs_into_providers(config: &mut AppConfig) {
    let only_default_placeholder_providers = !config.api_providers.is_empty()
        && config
            .api_providers
            .iter()
            .all(is_default_placeholder_provider);
    if !config.api_providers.is_empty() && !only_default_placeholder_providers {
        return;
    }
    if config.api_configs.is_empty() {
        config.api_providers = default_api_providers();
        return;
    }

    config.api_providers = config
        .api_configs
        .iter()
        .map(|legacy| {
            let model_id = format!("{}-model-default", legacy.id.trim());
            ApiProviderConfig {
                id: legacy.id.clone(),
                name: legacy.name.clone(),
                request_format: legacy.request_format,
                allow_concurrent_requests: legacy.allow_concurrent_requests,
                enable_text: legacy.enable_text,
                enable_image: legacy.enable_image,
                enable_audio: legacy.enable_audio,
                enable_tools: legacy.enable_tools,
                tools: legacy.tools.clone(),
                base_url: legacy.base_url.clone(),
                codex_auth_mode: legacy.codex_auth_mode.clone(),
                codex_local_auth_path: legacy.codex_local_auth_path.clone(),
                api_keys: legacy
                    .api_key
                    .trim()
                    .is_empty()
                    .then(Vec::new)
                    .unwrap_or_else(|| vec![legacy.api_key.clone()]),
                key_cursor: 0,
                cached_model_options: legacy
                    .model
                    .trim()
                    .is_empty()
                    .then(Vec::new)
                    .unwrap_or_else(|| vec![legacy.model.clone()]),
                models: vec![ApiModelConfig {
                    id: model_id,
                    model: legacy.model.clone(),
                    enable_image: legacy.enable_image,
                    enable_tools: legacy.enable_tools,
                    reasoning_effort: legacy.reasoning_effort.clone(),
                    temperature: legacy.temperature,
                    custom_temperature_enabled: legacy.custom_temperature_enabled,
                    context_window_tokens: legacy.context_window_tokens,
                    max_output_tokens: legacy.max_output_tokens,
                    custom_max_output_tokens_enabled: legacy.custom_max_output_tokens_enabled,
                }],
                failure_retry_count: legacy.failure_retry_count,
            }
        })
        .collect();
}

fn is_default_placeholder_provider(provider: &ApiProviderConfig) -> bool {
    let Some(model) = provider.models.first() else {
        return false;
    };
    let default_tools = default_api_tools();
    let tools_match = provider.tools.len() == default_tools.len()
        && provider
            .tools
            .iter()
            .zip(default_tools.iter())
            .all(|(left, right)| {
                left.id == right.id
                    && left.command == right.command
                    && left.args == right.args
                    && left.enabled == right.enabled
                    && left.values == right.values
            });
    provider.id == "default-provider-openai"
        && provider.name == "Default OpenAI"
        && matches!(provider.request_format, RequestFormat::OpenAI)
        && !provider.allow_concurrent_requests
        && provider.enable_text
        && !provider.enable_image
        && !provider.enable_audio
        && provider.enable_tools
        && tools_match
        && provider.base_url == "https://api.openai.com/v1"
        && provider.codex_auth_mode == default_codex_auth_mode()
        && provider.codex_local_auth_path == default_codex_local_auth_path()
        && provider.api_keys.is_empty()
        && provider.key_cursor == 0
        && provider.cached_model_options == vec!["gpt-4o-mini".to_string()]
        && provider.models.len() == 1
        && model.id == "default-model"
        && model.model == "gpt-4o-mini"
        && !model.enable_image
        && model.enable_tools
        && model.reasoning_effort == default_reasoning_effort()
        && (model.temperature - default_api_temperature()).abs() < f64::EPSILON
        && !model.custom_temperature_enabled
        && model.context_window_tokens == default_context_window_tokens()
        && model.max_output_tokens == default_max_output_tokens()
        && !model.custom_max_output_tokens_enabled
        && provider.failure_retry_count == default_failure_retry_count()
}

fn provider_first_endpoint_id(provider: &ApiProviderConfig) -> Option<String> {
    provider.models.iter().find_map(|model| {
        (!model.model.trim().is_empty()).then(|| api_endpoint_id(&provider.id, &model.id))
    })
}

fn remap_legacy_api_config_id_to_endpoint(config: &AppConfig, raw_id: &str) -> String {
    let trimmed = raw_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if config.api_configs.iter().any(|api| api.id == trimmed) {
        return trimmed.to_string();
    }
    config
        .api_providers
        .iter()
        .find(|provider| provider.id.trim() == trimmed)
        .and_then(provider_first_endpoint_id)
        .unwrap_or_else(|| trimmed.to_string())
}

fn expand_api_configs_from_providers(config: &mut AppConfig) {
    let mut expanded = Vec::<ApiConfig>::new();
    for provider in &config.api_providers {
        for model in &provider.models {
            if model.model.trim().is_empty() {
                continue;
            }
            expanded.push(ApiConfig {
                id: api_endpoint_id(&provider.id, &model.id),
                name: format!("{}/{}", provider.name.trim(), model.model.trim()),
                request_format: provider.request_format,
                allow_concurrent_requests: provider.allow_concurrent_requests,
                enable_text: provider.enable_text,
                enable_image: model.enable_image,
                enable_audio: provider.enable_audio,
                enable_tools: model.enable_tools,
                tools: provider.tools.clone(),
                base_url: provider.base_url.clone(),
                api_key: provider.api_keys.first().cloned().unwrap_or_default(),
                codex_auth_mode: provider.codex_auth_mode.clone(),
                codex_local_auth_path: provider.codex_local_auth_path.clone(),
                model: model.model.clone(),
                reasoning_effort: model.reasoning_effort.clone(),
                temperature: model.temperature,
                custom_temperature_enabled: model.custom_temperature_enabled,
                context_window_tokens: model.context_window_tokens,
                max_output_tokens: model.max_output_tokens,
                custom_max_output_tokens_enabled: model.custom_max_output_tokens_enabled,
                failure_retry_count: provider.failure_retry_count,
            });
        }
    }
    if expanded.is_empty() {
        let default_provider = ApiProviderConfig::default();
        config.api_providers = vec![default_provider.clone()];
        expanded.push(ApiConfig::default());
    }
    config.api_configs = expanded;
}

fn normalize_tools_list(
    tools: &mut Vec<ApiToolConfig>,
    enable_tools: bool,
    legacy_command_enabled: bool,
) {
    for tool in tools.iter_mut() {
        match tool.id.as_str() {
            "bing-search" => {
                tool.id = "websearch".to_string();
            }
            "desktop-screenshot" | "screenshot" => {
                tool.id = "__merged_into_operate__".to_string();
            }
            "desktop-wait" => {
                tool.id = "wait".to_string();
            }
            "command" => {
                tool.id = "__legacy_command__".to_string();
            }
            "shell-exec" => {
                tool.id = "exec".to_string();
                tool.args = vec!["exec".to_string()];
            }
            "shell-switch-workspace" => {
                tool.id = "__removed_shell_switch_workspace__".to_string();
            }
            _ => {}
        }
    }
    tools.retain(|tool| {
        !matches!(
            tool.id.as_str(),
            "__legacy_command__" | "__removed_shell_switch_workspace__"
                | "__merged_into_operate__"
        )
    });
    tools.sort_by(|a, b| a.id.cmp(&b.id));
    tools.dedup_by(|a, b| a.id == b.id);
    if enable_tools {
        if tools.is_empty() {
            *tools = default_api_tools();
        } else {
            let defaults = default_api_tools();
            for default_tool in defaults {
                if !tools.iter().any(|tool| tool.id == default_tool.id) {
                    tools.push(default_tool);
                }
            }
        }
    }
    if legacy_command_enabled {
        for tool_id in ["reload", "organize_context", "wait"] {
            if let Some(tool) = tools.iter_mut().find(|tool| tool.id == tool_id) {
                tool.enabled = true;
            }
        }
    }
}

fn is_codex_spark_model(model_id: &str) -> bool {
    let normalized = model_id.trim().to_ascii_lowercase();
    normalized.starts_with("gpt-5.3-codex-spark")
        || normalized.contains("-codex-spark")
}

fn normalize_api_tools(config: &mut AppConfig) {
    for provider in &mut config.api_providers {
        provider.enable_audio = false;
        provider.key_cursor = provider.key_cursor.min(1_000_000);
        provider.failure_retry_count = provider.failure_retry_count.clamp(0, 20);
        provider.codex_auth_mode = normalize_codex_auth_mode(&provider.codex_auth_mode);
        provider.codex_local_auth_path =
            normalize_terminal_path_input_for_current_platform(&provider.codex_local_auth_path);
        if provider.codex_local_auth_path.trim().is_empty() {
            provider.codex_local_auth_path = default_codex_local_auth_path();
        }
        provider.api_keys = provider
            .api_keys
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        provider.cached_model_options = provider
            .cached_model_options
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        for model in &mut provider.models {
            model.reasoning_effort = normalize_reasoning_effort(&model.reasoning_effort);
            model.temperature = model.temperature.clamp(0.0, 2.0);
            model.context_window_tokens = model.context_window_tokens.clamp(16_000, 2_000_000);
            model.max_output_tokens = model.max_output_tokens.clamp(256, 32_768);
            if provider.request_format.is_codex() {
                model.enable_image = true;
                model.enable_tools = true;
                model.temperature = default_api_temperature();
                model.custom_temperature_enabled = false;
                model.context_window_tokens =
                    codex_context_window_tokens_for_model(&model.model);
                model.max_output_tokens = default_max_output_tokens();
                model.custom_max_output_tokens_enabled = false;

                if is_codex_spark_model(&model.model) {
                    model.enable_image = false;
                }
            }
        }
        if provider.request_format.is_codex() {
            provider.enable_image = true;
            provider.enable_tools = true;
            provider.base_url = DEFAULT_CODEX_BASE_URL.to_string();
            provider.api_keys.clear();
        }
        let legacy_command_enabled = provider
            .tools
            .iter()
            .any(|tool| tool.id == "command" && tool.enabled);
        normalize_tools_list(
            &mut provider.tools,
            provider.enable_tools,
            legacy_command_enabled,
        );
    }

    for api in &mut config.api_configs {
        api.enable_audio = false;
        api.codex_auth_mode = normalize_codex_auth_mode(&api.codex_auth_mode);
        api.codex_local_auth_path =
            normalize_terminal_path_input_for_current_platform(&api.codex_local_auth_path);
        if api.codex_local_auth_path.trim().is_empty() {
            api.codex_local_auth_path = default_codex_local_auth_path();
        }
        api.reasoning_effort = normalize_reasoning_effort(&api.reasoning_effort);
        api.temperature = api.temperature.clamp(0.0, 2.0);
        api.context_window_tokens = api.context_window_tokens.clamp(16_000, 2_000_000);
        api.max_output_tokens = api.max_output_tokens.clamp(256, 32_768);
        if api.request_format.is_codex() {
            api.enable_image = true;
            api.enable_tools = true;
            api.base_url = DEFAULT_CODEX_BASE_URL.to_string();
            api.api_key.clear();
            api.temperature = default_api_temperature();
            api.custom_temperature_enabled = false;
            api.context_window_tokens = codex_context_window_tokens_for_model(&api.model);
            api.max_output_tokens = default_max_output_tokens();
            api.custom_max_output_tokens_enabled = false;

            if is_codex_spark_model(&api.model) {
                api.enable_image = false;
            }
        }
        api.custom_max_output_tokens_enabled =
            api.request_format.is_anthropic() || api.custom_max_output_tokens_enabled;
        api.failure_retry_count = api.failure_retry_count.clamp(0, 20);
        let legacy_command_enabled = api
            .tools
            .iter()
            .any(|tool| tool.id == "command" && tool.enabled);
        normalize_tools_list(&mut api.tools, api.enable_tools, legacy_command_enabled);
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
    let mut seen_paths = std::collections::HashSet::<String>::new();
    for raw in &config.shell_workspaces {
        let name = raw.name.trim().to_string();
        let mut normalized_path = normalize_terminal_path_input_for_current_platform(&raw.path);
        if name.is_empty() || normalized_path.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(&normalized_path);
        if candidate.is_absolute() {
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.is_dir() {
                    #[cfg(target_os = "windows")]
                    {
                        normalized_path =
                            normalize_windows_path_input(&canonical.to_string_lossy());
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        normalized_path = canonical.to_string_lossy().to_string();
                    }
                }
            }
        }
        let path_key = normalized_path.to_ascii_lowercase();
        if !seen_paths.insert(path_key) {
            continue;
        }
        normalized.push(ShellWorkspaceConfig {
            id: raw.id.trim().to_string(),
            name,
            path: normalized_path,
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: raw.built_in,
        });
    }
    config.shell_workspaces = normalized;
}

fn normalize_terminal_shell_kind(config: &mut AppConfig) {
    let raw = config.terminal_shell_kind.trim().to_ascii_lowercase();
    config.terminal_shell_kind = match raw.as_str() {
        "auto" | "powershell7" | "powershell5" | "git-bash" | "zsh" | "bash" | "sh" => raw,
        _ => "auto".to_string(),
    };
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

fn normalize_remote_im_channels(config: &mut AppConfig) {
    let mut out = Vec::<RemoteImChannelConfig>::new();
    let mut seen_ids = std::collections::HashSet::<String>::new();
    for raw in &config.remote_im_channels {
        let id = raw.id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        let key = id.to_ascii_lowercase();
        if !seen_ids.insert(key) {
            continue;
        }
        let mut name = raw.name.trim().to_string();
        if name.is_empty() {
            name = id.clone();
        }
        let mut credentials = raw.credentials.clone();
        if !credentials.is_object() {
            credentials = serde_json::json!({});
        }
        out.push(RemoteImChannelConfig {
            id,
            name,
            platform: raw.platform.clone(),
            enabled: raw.enabled,
            credentials,
            activate_assistant: raw.activate_assistant,
            receive_files: raw.receive_files,
            streaming_send: raw.streaming_send,
            show_tool_calls: raw.show_tool_calls,
            allow_send_files: raw.allow_send_files,
        });
    }
    config.remote_im_channels = out;
}

fn normalize_provider_non_stream_base_urls(config: &mut AppConfig) {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for raw in &config.provider_non_stream_base_urls {
        let value = raw.trim().trim_end_matches('/').to_string();
        if value.is_empty() {
            continue;
        }
        let key = value.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(value);
    }
    out.sort_by_key(|item| item.to_ascii_lowercase());
    config.provider_non_stream_base_urls = out;
}

fn is_text_chat_api(api: &ApiConfig) -> bool {
    api.enable_text && api.request_format.is_chat_text()
}

fn normalize_departments(config: &mut AppConfig) {
    if config.api_configs.is_empty() {
        return;
    }
    let fallback_api_id = config
        .api_configs
        .iter()
        .find(|api| api.id == config.assistant_department_api_config_id && is_text_chat_api(api))
        .or_else(|| config.api_configs.iter().find(|api| is_text_chat_api(api)))
        .or_else(|| config.api_configs.first())
        .map(|api| api.id.clone())
        .unwrap_or_default();
    let mut out = Vec::<DepartmentConfig>::new();
    let mut seen_ids = std::collections::HashSet::<String>::new();
    let valid_text_chat_api_ids = config
        .api_configs
        .iter()
        .filter(|a| is_text_chat_api(a))
        .map(|a| a.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for raw in &config.departments {
        let id = raw.id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        let key = id.to_ascii_lowercase();
        if !seen_ids.insert(key) {
            continue;
        }
        let mut api_config_ids = department_api_config_ids(raw)
            .into_iter()
            .map(|id| remap_legacy_api_config_id_to_endpoint(config, &id))
            .filter(|id| valid_text_chat_api_ids.contains(id))
            .collect::<Vec<_>>();
        if api_config_ids.is_empty()
            && !raw.api_config_id.trim().is_empty()
            && !fallback_api_id.trim().is_empty()
            && (raw.id == DEPUTY_DEPARTMENT_ID
                || raw.id == REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID)
        {
            api_config_ids.push(fallback_api_id.clone());
        }
        let api_config_id = api_config_ids
            .first()
            .cloned()
            .unwrap_or_default();
        let mut agent_ids = Vec::<String>::new();
        let mut seen_agent_ids = std::collections::HashSet::<String>::new();
        for agent_id in &raw.agent_ids {
            let agent_id = agent_id.trim().to_string();
            if agent_id.is_empty() {
                continue;
            }
            let key = agent_id.to_ascii_lowercase();
            if seen_agent_ids.insert(key) {
                agent_ids.push(agent_id);
            }
        }
        let mut item = DepartmentConfig {
            id: id.clone(),
            name: raw.name.trim().to_string(),
            summary: raw.summary.trim().to_string(),
            guide: raw.guide.trim().to_string(),
            api_config_ids,
            api_config_id,
            agent_ids,
            created_at: raw.created_at.trim().to_string(),
            updated_at: raw.updated_at.trim().to_string(),
            order_index: raw.order_index,
            is_built_in_assistant: raw.is_built_in_assistant || id == ASSISTANT_DEPARTMENT_ID,
            source: if raw.source.trim().is_empty() { default_main_source() } else { raw.source.trim().to_string() },
            scope: if raw.scope.trim().is_empty() { default_global_scope() } else { raw.scope.trim().to_string() },
            permission_control: normalize_department_permission_control(&raw.permission_control),
        };
        if item.name.is_empty() {
            item.name = if item.id == DEPUTY_DEPARTMENT_ID {
                "副手".to_string()
            } else if item.id == REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID {
                "远程客服".to_string()
            } else if item.is_built_in_assistant {
                default_assistant_department_name(&config.ui_language)
            } else {
                format!("部门 {}", out.len() + 1)
            };
        }
        if item.created_at.trim().is_empty() {
            item.created_at = now_iso();
        }
        if item.updated_at.trim().is_empty() {
            item.updated_at = item.created_at.clone();
        }
        out.push(item);
    }

    if !out.iter().any(|item| item.is_built_in_assistant || item.id == ASSISTANT_DEPARTMENT_ID) {
        out.push(default_assistant_department(&fallback_api_id));
    }
    if !out.iter().any(|item| item.id == DEPUTY_DEPARTMENT_ID) {
        out.push(default_deputy_department(&fallback_api_id));
    }
    if !out
        .iter()
        .any(|item| item.id == REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID)
    {
        out.push(default_remote_customer_service_department(&fallback_api_id));
    }

    let normalize_department_api_bindings =
        |item: &mut DepartmentConfig, valid_text_chat_api_ids: &std::collections::HashSet<String>| {
            let ids = department_api_config_ids(item)
                .into_iter()
                .map(|id| remap_legacy_api_config_id_to_endpoint(config, &id))
                .filter(|id| valid_text_chat_api_ids.contains(id))
                .collect::<Vec<_>>();
            item.api_config_ids = ids;
            item.api_config_id = item.api_config_ids.first().cloned().unwrap_or_default();
        };

    for (idx, item) in out.iter_mut().enumerate() {
        item.order_index = (idx as i64) + 1;
        if item.id == ASSISTANT_DEPARTMENT_ID || item.is_built_in_assistant {
            item.id = ASSISTANT_DEPARTMENT_ID.to_string();
            item.is_built_in_assistant = true;
            if item.name.trim().is_empty() {
                item.name = default_assistant_department_name(&config.ui_language);
            }
            normalize_department_api_bindings(item, &valid_text_chat_api_ids);
            if item.agent_ids.is_empty() {
                item.agent_ids = vec![DEFAULT_AGENT_ID.to_string()];
            }
        } else if item.id == DEPUTY_DEPARTMENT_ID {
            if item.name.trim().is_empty() {
                item.name = "副手".to_string();
            }
            if item.summary.trim().is_empty() {
                item.summary = "负责快速执行上级派发的明确任务，强调最小行动与严格边界。".to_string();
            }
            if item.guide.trim().is_empty() {
                item.guide = "你是副手部门。你的核心原则是严格不越权、不擅自扩展需求、不多想。收到上级派发的任务后，用最少的工具调用、最快的速度完成明确目标；若信息不足或任务超出指令边界，就直接说明缺口并等待主部门继续决策。".to_string();
            }
            normalize_department_api_bindings(item, &valid_text_chat_api_ids);
            if item.agent_ids.is_empty() {
                item.agent_ids = vec![DEFAULT_AGENT_ID.to_string()];
            }
        } else if item.id == REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID {
            if item.name.trim().is_empty() {
                item.name = "远程客服".to_string();
            }
            if item.summary.trim().is_empty() {
                item.summary = REMOTE_CUSTOMER_SERVICE_DEPARTMENT_SUMMARY.to_string();
            }
            if item.guide.trim().is_empty() {
                item.guide = REMOTE_CUSTOMER_SERVICE_DEPARTMENT_GUIDE.to_string();
            }
            normalize_department_api_bindings(item, &valid_text_chat_api_ids);
            if item.agent_ids.is_empty() {
                item.agent_ids = vec![DEFAULT_AGENT_ID.to_string()];
            }
        }
    }

    out.sort_by_key(|item| (built_in_department_rank(&item.id), item.order_index));
    for (idx, item) in out.iter_mut().enumerate() {
        item.order_index = (idx as i64) + 1;
    }
    config.departments = out;
    if let Some(dept) = assistant_department(config) {
        config.assistant_department_api_config_id = department_primary_api_config_id(dept);
    }
}

fn normalize_app_config(config: &mut AppConfig) {
    if config.api_configs.is_empty() && config.api_providers.is_empty() {
        *config = AppConfig::default();
        return;
    }
    migrate_legacy_api_configs_into_providers(config);
    expand_api_configs_from_providers(config);
    config.selected_api_config_id =
        remap_legacy_api_config_id_to_endpoint(config, &config.selected_api_config_id);
    config.assistant_department_api_config_id =
        remap_legacy_api_config_id_to_endpoint(config, &config.assistant_department_api_config_id);
    config.vision_api_config_id = config
        .vision_api_config_id
        .as_ref()
        .map(|id| remap_legacy_api_config_id_to_endpoint(config, id))
        .filter(|id| !id.trim().is_empty());
    config.stt_api_config_id = config
        .stt_api_config_id
        .as_ref()
        .map(|id| remap_legacy_api_config_id_to_endpoint(config, id))
        .filter(|id| !id.trim().is_empty());
    ensure_hotkey_config_normalized(config);
    let lang = config.ui_language.trim();
    config.ui_language = match lang {
        "zh-CN" | "en-US" | "zh-TW" => lang.to_string(),
        _ => default_ui_language(),
    };
    // Font compatibility is disabled in UI; ignore persisted custom font values.
    config.ui_font = default_ui_font();

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

    for provider in &mut config.api_providers {
        if matches!(provider.request_format, RequestFormat::Gemini) && !provider.enable_text {
            provider.request_format = RequestFormat::GeminiEmbedding;
        }
    }
    expand_api_configs_from_providers(config);

    if !config
        .api_configs
        .iter()
        .any(|a| a.id == config.selected_api_config_id)
    {
        config.selected_api_config_id = config
            .api_providers
            .iter()
            .find_map(provider_first_endpoint_id)
            .or_else(|| config.api_configs.first().map(|api| api.id.clone()))
            .unwrap_or_default();
    }

    let chat_valid = config.api_configs.iter().any(|a| {
        a.id == config.assistant_department_api_config_id
            && a.enable_text
            && a.request_format.is_chat_text()
    });
    if !chat_valid {
        config.assistant_department_api_config_id.clear();
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

    if config.stt_api_config_id.is_none() {
        config.stt_auto_send = false;
    }
    normalize_terminal_shell_kind(config);
    normalize_shell_workspaces(config);
    normalize_mcp_servers(config);
    normalize_remote_im_channels(config);
    normalize_provider_non_stream_base_urls(config);
    normalize_departments(config);
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
            // 图片在前台显示链路里保留 @media: 引用，由前端按需懒加载；
            // 仅音频维持旧行为，继续展开为 base64。
            MessagePart::Audio { bytes_base64, .. } => {
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
            MessagePart::Image { .. } | MessagePart::Text { .. } => {}
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
        .unwrap_or(app_config.assistant_department_api_config_id.as_str());

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
                provider_id: None,
                provider_api_keys: Vec::new(),
                provider_key_cursor: 0,
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                base_url: debug_cfg.base_url.trim().to_string(),
                api_key: debug_cfg.api_key.trim().to_string(),
                model: debug_cfg.model.trim().to_string(),
                reasoning_effort: None,
                temperature: debug_cfg.temperature.map(|value| value.clamp(0.0, 2.0)),
                max_output_tokens: None,
                extra_headers: Vec::new(),
                codex_auth: None,
            });
        }
    }

    let selected = resolve_selected_api_config(app_config, requested_id).ok_or_else(|| {
        "No API config configured. Please add at least one API config.".to_string()
    })?;

    let selected_provider_id = parse_api_endpoint_id(&selected.id)
        .and_then(|(provider_id, _model_id)| {
            app_config
                .api_providers
                .iter()
                .any(|provider| provider.id == provider_id)
                .then_some(provider_id)
        });
    let selected_provider = selected_provider_id.as_deref().and_then(|provider_id| {
        app_config
            .api_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    });
    let mut selected_api_key = selected_provider
        .map(peek_provider_api_key)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| selected.api_key.trim().to_string());
    let mut extra_headers = Vec::<(String, String)>::new();
    let mut codex_auth = None;
    if selected.request_format.is_codex() {
        let provider = selected_provider.ok_or_else(|| {
            "Codex provider not found. Please save the provider config first.".to_string()
        })?;
        let resolved = read_codex_runtime_auth_snapshot(
            &provider.id,
            &provider.codex_auth_mode,
            &provider.codex_local_auth_path,
        )?;
        if let Some(account_id) = resolved
            .account_id
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            extra_headers.push((
                "ChatGPT-Account-Id".to_string(),
                account_id.to_string(),
            ));
        }
        extra_headers.push(("session_id".to_string(), Uuid::new_v4().to_string()));
        selected_api_key = resolved.access_token.clone();
        codex_auth = Some(resolved);
    }

    if selected_api_key.trim().is_empty() {
        return Err("Selected API config API key is empty. Please fill it in settings.".to_string());
    }

    Ok(ResolvedApiConfig {
        provider_id: selected_provider_id,
        provider_api_keys: selected_provider
            .map(|provider| provider.api_keys.clone())
            .unwrap_or_default(),
        provider_key_cursor: selected_provider
            .map(|provider| provider.key_cursor as usize)
            .unwrap_or(0),
        request_format: selected.request_format,
        allow_concurrent_requests: selected_provider
            .map(|provider| provider.allow_concurrent_requests)
            .unwrap_or(selected.allow_concurrent_requests),
        base_url: selected.base_url.trim().to_string(),
        api_key: selected_api_key,
        model: selected.model.trim().to_string(),
        reasoning_effort: selected
            .request_format
            .is_codex()
            .then(|| normalize_reasoning_effort(&selected.reasoning_effort)),
        temperature: selected
            .custom_temperature_enabled
            .then_some(selected.temperature.clamp(0.0, 2.0))
            .filter(|_| !selected.request_format.is_codex()),
        max_output_tokens: (selected.request_format.is_anthropic()
            || selected.custom_max_output_tokens_enabled)
            .then_some(selected.max_output_tokens.clamp(256, 32_768))
            .filter(|_| !selected.request_format.is_codex()),
        extra_headers,
        codex_auth,
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

#[cfg(test)]
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
