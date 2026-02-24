#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "main")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubUpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    release_url: String,
}

fn parse_version_parts(input: &str) -> Vec<u64> {
    let cleaned = input
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or_default();
    cleaned
        .split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    let a = parse_version_parts(current);
    let b = parse_version_parts(latest);
    let max_len = a.len().max(b.len());
    for idx in 0..max_len {
        let av = *a.get(idx).unwrap_or(&0);
        let bv = *b.get(idx).unwrap_or(&0);
        if bv > av {
            return true;
        }
        if bv < av {
            return false;
        }
    }
    false
}

#[tauri::command]
async fn check_github_update() -> Result<GithubUpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let api_url = "https://api.github.com/repos/kawayiYokami/Easy-call-ai/releases/latest";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build update checker client failed: {err}"))?;
    let response = client
        .get(api_url)
        .header(
            reqwest::header::USER_AGENT,
            format!("easy-call-ai/{current_version}"),
        )
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("Request latest release failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Update API returned {}",
            response.status().as_u16()
        ));
    }
    let payload = response
        .json::<Value>()
        .await
        .map_err(|err| format!("Parse update response failed: {err}"))?;
    let latest_version = payload
        .get("tag_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "GitHub release tag_name is empty".to_string())?
        .to_string();
    let release_url = payload
        .get("html_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("https://github.com/kawayiYokami/Easy-call-ai/releases/latest")
        .to_string();

    Ok(GithubUpdateInfo {
        current_version: current_version.clone(),
        latest_version: latest_version.clone(),
        has_update: is_newer_version(&current_version, &latest_version),
        release_url,
    })
}

#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut result = read_config(&state.config_path)?;
    normalize_app_config(&mut result);
    ensure_default_shell_workspace_in_config(&mut result, &state);
    drop(guard);
    Ok(result)
}

#[tauri::command]
fn list_system_fonts() -> Result<Vec<String>, String> {
    let mut families = font_kit::source::SystemSource::new()
        .all_families()
        .map_err(|err| format!("List system fonts failed: {err}"))?;
    families.sort_by_key(|name| name.to_ascii_lowercase());
    families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(families)
}

#[tauri::command]
fn save_config(
    config: AppConfig,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    if config.api_configs.is_empty() {
        return Err("At least one API config must be configured.".to_string());
    }
    let mut config = config;
    normalize_app_config(&mut config);
    ensure_default_shell_workspace_in_config(&mut config, &state);

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    write_config(&state.config_path, &config)?;
    register_hotkey_from_config(&app, &config)?;
    drop(guard);
    let _ = app.emit("easy-call:config-updated", &config);
    Ok(config)
}

#[tauri::command]
fn load_agents(state: State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = read_app_data(&state.data_path)?;
    let changed = ensure_default_agent(&mut data);
    if changed {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);
    Ok(data.agents)
}

#[tauri::command]
fn save_agents(
    input: SaveAgentsInput,
    state: State<'_, AppState>,
) -> Result<Vec<AgentProfile>, String> {
    if input.agents.is_empty() {
        return Err("At least one agent is required.".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = read_app_data(&state.data_path)?;
    let previous_agents = data.agents.clone();
    let existing_user_persona = data
        .agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID)
        .cloned();
    data.agents = input.agents;
    if !data.agents.iter().any(|a| a.id == USER_PERSONA_ID) {
        if let Some(user_persona) = existing_user_persona {
            data.agents.push(user_persona);
        }
    }
    ensure_default_agent(&mut data);

    let next_ids = data
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect::<std::collections::HashSet<_>>();
    let previous_by_id = previous_agents
        .iter()
        .map(|a| (a.id.clone(), a))
        .collect::<std::collections::HashMap<_, _>>();
    let removed_agent_ids = previous_agents
        .iter()
        .filter(|a| !a.is_built_in_user && a.id != USER_PERSONA_ID)
        .filter(|a| !next_ids.contains(&a.id))
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    let disabled_private_memory_agent_ids = data
        .agents
        .iter()
        .filter(|a| !a.is_built_in_user && a.id != USER_PERSONA_ID)
        .filter(|a| {
            previous_by_id
                .get(&a.id)
                .map(|old| old.private_memory_enabled && !a.private_memory_enabled)
                .unwrap_or(false)
        })
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();

    for agent_id in &removed_agent_ids {
        let export = memory_store_export_agent_private_memories(&state.data_path, agent_id)?;
        let deleted = memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id)?;
        eprintln!(
            "[PERSONA] removed agent private memories exported+deleted. agent={}, exported={}, path={}, deleted={}",
            agent_id, export.count, export.path, deleted
        );
    }
    for agent_id in &disabled_private_memory_agent_ids {
        let export = memory_store_export_agent_private_memories(&state.data_path, agent_id)?;
        let deleted = memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id)?;
        eprintln!(
            "[PERSONA] private memory disabled, exported+deleted. agent={}, exported={}, path={}, deleted={}",
            agent_id, export.count, export.path, deleted
        );
    }

    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(data.agents)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAgentMemoriesInput {
    agent_id: String,
    memories: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAgentMemoriesResult {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPrivateMemoryCountInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPrivateMemoryCountResult {
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAgentPrivateMemoryEnabledInput {
    agent_id: String,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAgentPrivateMemoryEnabledResult {
    agent_id: String,
    enabled: bool,
    exported_count: usize,
    deleted_count: usize,
    export_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAgentPrivateMemoriesInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAgentPrivateMemoriesResult {
    count: usize,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisableAgentPrivateMemoryInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisableAgentPrivateMemoryResult {
    agent_id: String,
    enabled: bool,
    deleted_count: usize,
}

#[tauri::command]
fn get_agent_private_memory_count(
    input: AgentPrivateMemoryCountInput,
    state: State<'_, AppState>,
) -> Result<AgentPrivateMemoryCountResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }
    Ok(AgentPrivateMemoryCountResult {
        count: memory_store_count_private_memories_by_agent(&state.data_path, agent_id)?,
    })
}

#[tauri::command]
fn set_agent_private_memory_enabled(
    input: SetAgentPrivateMemoryEnabledInput,
    state: State<'_, AppState>,
) -> Result<SetAgentPrivateMemoryEnabledResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    let agent_idx = data
        .agents
        .iter()
        .position(|a| a.id == agent_id && !a.is_built_in_user)
        .ok_or_else(|| format!("Agent '{}' not found.", agent_id))?;

    let current = data.agents[agent_idx].private_memory_enabled;
    if current == input.enabled {
        drop(guard);
        return Ok(SetAgentPrivateMemoryEnabledResult {
            agent_id: agent_id.to_string(),
            enabled: current,
            exported_count: 0,
            deleted_count: 0,
            export_path: None,
        });
    }

    if input.enabled {
        data.agents[agent_idx].private_memory_enabled = true;
        write_app_data(&state.data_path, &data)?;
        drop(guard);
        return Ok(SetAgentPrivateMemoryEnabledResult {
            agent_id: agent_id.to_string(),
            enabled: true,
            exported_count: 0,
            deleted_count: 0,
            export_path: None,
        });
    }

    let export = memory_store_export_agent_private_memories(&state.data_path, agent_id)?;
    let deleted = memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id)?;
    data.agents[agent_idx].private_memory_enabled = false;
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(SetAgentPrivateMemoryEnabledResult {
        agent_id: agent_id.to_string(),
        enabled: false,
        exported_count: export.count,
        deleted_count: deleted,
        export_path: Some(export.path),
    })
}

#[tauri::command]
fn export_agent_private_memories(
    input: ExportAgentPrivateMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ExportAgentPrivateMemoriesResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }
    let export = memory_store_export_agent_private_memories(&state.data_path, agent_id)?;
    Ok(ExportAgentPrivateMemoriesResult {
        count: export.count,
        path: export.path,
    })
}

#[tauri::command]
fn disable_agent_private_memory(
    input: DisableAgentPrivateMemoryInput,
    state: State<'_, AppState>,
) -> Result<DisableAgentPrivateMemoryResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    let agent_idx = data
        .agents
        .iter()
        .position(|a| a.id == agent_id && !a.is_built_in_user)
        .ok_or_else(|| format!("Agent '{}' not found.", agent_id))?;

    if !data.agents[agent_idx].private_memory_enabled {
        drop(guard);
        return Ok(DisableAgentPrivateMemoryResult {
            agent_id: agent_id.to_string(),
            enabled: false,
            deleted_count: 0,
        });
    }

    let deleted = memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id)?;
    data.agents[agent_idx].private_memory_enabled = false;
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(DisableAgentPrivateMemoryResult {
        agent_id: agent_id.to_string(),
        enabled: false,
        deleted_count: deleted,
    })
}

#[tauri::command]
fn import_agent_memories(
    input: ImportAgentMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ImportAgentMemoriesResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data
        .agents
        .iter()
        .any(|a| a.id == agent_id && !a.is_built_in_user)
    {
        drop(guard);
        return Err(format!("Agent '{}' not found.", agent_id));
    }
    drop(guard);

    let stats = memory_store_import_memories_for_agent(&state.data_path, &input.memories, agent_id)?;
    Ok(ImportAgentMemoriesResult {
        imported_count: stats.imported_count,
        created_count: stats.created_count,
        merged_count: stats.merged_count,
        total_count: stats.total_count,
    })
}

#[tauri::command]
fn load_chat_settings(state: State<'_, AppState>) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = read_app_data(&state.data_path)?;
    let changed = ensure_default_agent(&mut data);
    if changed {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);

    Ok(ChatSettings {
        selected_agent_id: data.selected_agent_id.clone(),
        user_alias: user_persona_name(&data),
        response_style_id: data.response_style_id.clone(),
    })
}

#[tauri::command]
fn save_chat_settings(
    input: ChatSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data
        .agents
        .iter()
        .any(|a| a.id == input.selected_agent_id && !a.is_built_in_user)
    {
        return Err("Selected agent not found.".to_string());
    }
    data.selected_agent_id = input.selected_agent_id.clone();
    data.user_alias = user_persona_name(&data);
    data.response_style_id = normalize_response_style_id(&input.response_style_id);
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    let payload = ChatSettings {
        selected_agent_id: input.selected_agent_id,
        user_alias: data.user_alias,
        response_style_id: data.response_style_id,
    };

    let _ = app.emit("easy-call:chat-settings-updated", &payload);

    Ok(payload)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveAgentAvatarInput {
    agent_id: String,
    mime: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearAgentAvatarInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvatarDataPathInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncTrayIconInput {
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvatarMeta {
    path: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvatarDataUrlOutput {
    data_url: String,
}

fn avatar_storage_dir(state: &AppState) -> Result<PathBuf, String> {
    Ok(app_root_from_data_path(&state.data_path).join("avatars"))
}

fn sanitize_avatar_key(value: &str) -> String {
    let trimmed = value.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let normalized = out.trim_matches('_');
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized.to_string()
    }
}

fn normalize_avatar_bytes_to_webp(raw: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(raw)
        .map_err(|err| format!("Decode avatar image failed: {err}"))?;
    let resized = image.resize_to_fill(128, 128, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::<u8>::new();
    let mut cursor = Cursor::new(&mut out);
    resized
        .write_to(&mut cursor, ImageFormat::WebP)
        .map_err(|err| format!("Encode avatar as webp failed: {err}"))?;
    Ok(out)
}

#[tauri::command]
fn save_agent_avatar(
    input: SaveAgentAvatarInput,
    state: State<'_, AppState>,
) -> Result<AvatarMeta, String> {
    if input.agent_id.trim().is_empty() {
        return Err("agentId is required".to_string());
    }
    if input.bytes_base64.trim().is_empty() {
        return Err("avatar payload is empty".to_string());
    }
    if !input.mime.trim().starts_with("image/") {
        return Err("avatar mime must be image/*".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    let _ = ensure_default_agent(&mut data);

    let idx = data
        .agents
        .iter()
        .position(|a| a.id == input.agent_id)
        .ok_or_else(|| "Agent not found".to_string())?;

    let raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode avatar base64 failed: {err}"))?;
    let webp = normalize_avatar_bytes_to_webp(&raw)?;

    let dir = avatar_storage_dir(&state)?;
    fs::create_dir_all(&dir).map_err(|err| format!("Create avatar directory failed: {err}"))?;
    let safe_id = sanitize_avatar_key(&input.agent_id);
    let path = dir.join(format!("agent-{safe_id}.webp"));
    fs::write(&path, webp).map_err(|err| format!("Write avatar file failed: {err}"))?;

    let now = now_iso();
    data.agents[idx].avatar_path = Some(path.to_string_lossy().to_string());
    data.agents[idx].avatar_updated_at = Some(now.clone());
    data.agents[idx].updated_at = now.clone();
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(AvatarMeta {
        path: path.to_string_lossy().to_string(),
        updated_at: now,
    })
}

#[tauri::command]
fn clear_agent_avatar(
    input: ClearAgentAvatarInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if input.agent_id.trim().is_empty() {
        return Err("agentId is required".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    let _ = ensure_default_agent(&mut data);
    let idx = data
        .agents
        .iter()
        .position(|a| a.id == input.agent_id)
        .ok_or_else(|| "Agent not found".to_string())?;

    if let Some(path) = data.agents[idx].avatar_path.as_deref() {
        let p = PathBuf::from(path);
        if p.exists() {
            let _ = fs::remove_file(p);
        }
    }
    data.agents[idx].avatar_path = None;
    data.agents[idx].avatar_updated_at = None;
    data.agents[idx].updated_at = now_iso();
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(())
}

#[tauri::command]
fn read_avatar_data_url(
    input: AvatarDataPathInput,
    state: State<'_, AppState>,
) -> Result<AvatarDataUrlOutput, String> {
    if input.path.trim().is_empty() {
        return Ok(AvatarDataUrlOutput {
            data_url: String::new(),
        });
    }
    let avatars_dir = avatar_storage_dir(&state)?;
    let root = fs::canonicalize(&avatars_dir).map_err(|err| {
        format!(
            "Resolve avatar root failed ({}): {err}",
            avatars_dir.to_string_lossy()
        )
    })?;
    let target = fs::canonicalize(input.path.trim()).map_err(|err| {
        format!("Resolve avatar path failed ({}): {err}", input.path.trim())
    })?;
    if !target.starts_with(&root) {
        return Err("Avatar path is outside allowed avatar directory.".to_string());
    }
    let metadata = fs::metadata(&target)
        .map_err(|err| format!("Read avatar metadata failed: {err}"))?;
    if !metadata.is_file() {
        return Err("Avatar path must be a regular file.".to_string());
    }
    let ext = target
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "webp" => "image/webp",
        "png" => "image/png",
        _ => return Err("Avatar file type is not allowed (only .webp/.png).".to_string()),
    };
    let bytes = fs::read(&target)
        .map_err(|err| format!("Read avatar file failed: {err}"))?;
    let base64 = B64.encode(bytes);
    Ok(AvatarDataUrlOutput {
        data_url: format!("data:{mime};base64,{base64}"),
    })
}

#[tauri::command]
fn sync_tray_icon(
    input: SyncTrayIconInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    let changed = ensure_default_agent(&mut data);
    if changed {
        write_app_data(&state.data_path, &data)?;
    }
    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(data.selected_agent_id.as_str());
    let avatar_path = data
        .agents
        .iter()
        .find(|a| a.id == target_agent_id)
        .and_then(|a| a.avatar_path.clone());
    drop(guard);
    sync_tray_icon_from_avatar_path(&app, avatar_path.as_deref())
}

#[tauri::command]
fn save_conversation_api_settings(
    input: ConversationApiSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationApiSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut config = read_config(&state.config_path)?;
    config.chat_api_config_id = input.chat_api_config_id.clone();
    config.vision_api_config_id = input.vision_api_config_id.clone();
    config.stt_api_config_id = input.stt_api_config_id.clone();
    config.stt_auto_send = input.stt_auto_send;
    normalize_app_config(&mut config);
    write_config(&state.config_path, &config)?;
    drop(guard);

    let payload = ConversationApiSettings {
        chat_api_config_id: config.chat_api_config_id,
        vision_api_config_id: config.vision_api_config_id,
        stt_api_config_id: config.stt_api_config_id,
        stt_auto_send: config.stt_auto_send,
    };

    let _ = app.emit("easy-call:conversation-api-updated", &payload);

    Ok(payload)
}

#[tauri::command]
fn get_chat_snapshot(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = read_config(&state.config_path)?;

    let mut data = read_app_data(&state.data_path)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let requested_agent_id = input.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if data
        .agents
        .iter()
        .any(|a| a.id == data.selected_agent_id && !a.is_built_in_user)
    {
        data.selected_agent_id.clone()
    } else {
        data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let before_len = data.conversations.len();
    let idx = if let Some(existing_idx) =
        latest_active_conversation_index(&data, "", &effective_agent_id)
    {
        existing_idx
    } else {
        let api_config = resolve_selected_api_config(&app_config, None)
            .ok_or_else(|| "No API config available".to_string())?;
        ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id)
    };
    let conversation = &data.conversations[idx];

    let mut latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();
    let mut latest_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .cloned();

    if defaults_changed || data.conversations.len() != before_len {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);

    if let Some(message) = latest_user.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }
    if let Some(message) = latest_assistant.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }

    Ok(ChatSnapshot {
        conversation_id: conversation.id.clone(),
        latest_user,
        latest_assistant,
        active_message_count: conversation.messages.len(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnarchivedConversationSummary {
    conversation_id: String,
    title: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_message_at: Option<String>,
    message_count: usize,
    agent_id: String,
    api_config_id: String,
}

fn conversation_preview_title(conversation: &Conversation) -> String {
    let text = conversation
        .messages
        .iter()
        .find(|m| m.role == "user")
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text } => Some(text.trim()),
                    _ => None,
                })
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let compact = clean_text(text.trim());
    if compact.is_empty() {
        "无内容".to_string()
    } else {
        compact.chars().take(12).collect::<String>()
    }
}

#[tauri::command]
fn list_unarchived_conversations(state: State<'_, AppState>) -> Result<Vec<UnarchivedConversationSummary>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    let defaults_changed = ensure_default_agent(&mut data);

    let mut summaries = data
        .conversations
        .iter()
        .filter(|c| c.summary.trim().is_empty())
        .map(|c| {
            let last_message_at = c.messages.last().map(|m| m.created_at.clone());
            UnarchivedConversationSummary {
                conversation_id: c.id.clone(),
                title: if c.title.trim().is_empty() {
                    conversation_preview_title(c)
                } else {
                    c.title.clone()
                },
                updated_at: c.updated_at.clone(),
                last_message_at,
                message_count: c.messages.len(),
                agent_id: c.agent_id.clone(),
                api_config_id: c.api_config_id.clone(),
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| {
        let bk = b
            .last_message_at
            .as_deref()
            .unwrap_or(b.updated_at.as_str());
        let ak = a
            .last_message_at
            .as_deref()
            .unwrap_or(a.updated_at.as_str());
        bk.cmp(ak).then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    if defaults_changed {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);
    Ok(summaries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationMessagesInput {
    conversation_id: String,
}

#[tauri::command]
fn get_unarchived_conversation_messages(
    input: GetUnarchivedConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let mut messages = data
        .conversations
        .iter()
        .find(|c| c.summary.trim().is_empty() && c.id == conversation_id)
        .map(|c| c.messages.clone())
        .ok_or_else(|| "Unarchived conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationInput {
    conversation_id: String,
}

#[tauri::command]
fn delete_unarchived_conversation(
    input: DeleteUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = read_app_data(&state.data_path)?;
    let before = data.conversations.len();
    data.conversations
        .retain(|c| !(c.summary.trim().is_empty() && c.id == conversation_id));
    if data.conversations.len() == before {
        drop(guard);
        return Err("Unarchived conversation not found.".to_string());
    }
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(())
}

#[tauri::command]
fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = read_config(&state.config_path)?;

    let mut data = read_app_data(&state.data_path)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let requested_agent_id = input.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if data
        .agents
        .iter()
        .any(|a| a.id == data.selected_agent_id && !a.is_built_in_user)
    {
        data.selected_agent_id.clone()
    } else {
        data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let before_len = data.conversations.len();
    let idx = if let Some(existing_idx) =
        latest_active_conversation_index(&data, "", &effective_agent_id)
    {
        existing_idx
    } else {
        let api_config = resolve_selected_api_config(&app_config, None)
            .ok_or_else(|| "No API config available".to_string())?;
        ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id)
    };
    let mut messages = data.conversations[idx].messages.clone();

    if defaults_changed || data.conversations.len() != before_len {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationInput {
    session: SessionSelector,
    message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationResult {
    removed_count: usize,
    remaining_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    recalled_user_message: Option<ChatMessage>,
}

#[tauri::command]
fn rewind_conversation_from_message(
    input: RewindConversationInput,
    state: State<'_, AppState>,
) -> Result<RewindConversationResult, String> {
    let message_id = input.message_id.trim();
    if message_id.is_empty() {
        return Err("messageId is required.".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = read_config(&state.config_path)?;

    let mut data = read_app_data(&state.data_path)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let requested_agent_id = input.session.agent_id.trim();
    if requested_agent_id.is_empty() {
        return Err("agentId is required.".to_string());
    }
    if !data
        .agents
        .iter()
        .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }

    let before_len = data.conversations.len();
    let idx = latest_active_conversation_index(&data, "", requested_agent_id)
        .ok_or_else(|| "No active conversation found for current agent.".to_string())?;
    let conversation = data
        .conversations
        .get_mut(idx)
        .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?;

    let remove_from = conversation
        .messages
        .iter()
        .position(|m| m.id == message_id && m.role == "user")
        .ok_or_else(|| "Target user message not found in active conversation.".to_string())?;

    let mut recalled_user_message = conversation.messages.get(remove_from).cloned();
    let removed_count = conversation.messages.len().saturating_sub(remove_from);
    conversation.messages.truncate(remove_from);
    conversation.updated_at = now_iso();
    conversation.last_user_at = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.created_at.clone());
    conversation.last_assistant_at = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .map(|m| m.created_at.clone());
    let context_window_tokens = app_config
        .api_configs
        .iter()
        .find(|api| api.id == conversation.api_config_id)
        .map(|api| api.context_window_tokens)
        .unwrap_or(128000);
    conversation.last_context_usage_ratio = if conversation.messages.is_empty() {
        0.0
    } else {
        compute_context_usage_ratio(conversation, context_window_tokens)
    };

    if defaults_changed || data.conversations.len() != before_len || removed_count > 0 {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);

    if let Some(message) = recalled_user_message.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }

    Ok(RewindConversationResult {
        removed_count,
        remaining_count: remove_from,
        recalled_user_message,
    })
}
