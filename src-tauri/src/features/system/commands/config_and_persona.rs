#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "main")
}

#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut result = read_config(&state.config_path)?;
    normalize_app_config(&mut result);
    drop(guard);
    Ok(result)
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

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    write_config(&state.config_path, &config)?;
    register_hotkey_from_config(&app, &config)?;
    drop(guard);
    Ok(config)
}

#[tauri::command]
fn load_agents(state: State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

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
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
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
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(data.agents)
}

#[tauri::command]
fn load_chat_settings(state: State<'_, AppState>) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

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
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

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

    Ok(ChatSettings {
        selected_agent_id: input.selected_agent_id,
        user_alias: data.user_alias,
        response_style_id: data.response_style_id,
    })
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
    let base = state
        .data_path
        .parent()
        .ok_or_else(|| "App data path has no parent directory".to_string())?;
    Ok(base.join("avatars"))
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
        .map_err(|_| "Failed to lock state mutex".to_string())?;
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
        .map_err(|_| "Failed to lock state mutex".to_string())?;
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
) -> Result<AvatarDataUrlOutput, String> {
    if input.path.trim().is_empty() {
        return Ok(AvatarDataUrlOutput {
            data_url: String::new(),
        });
    }
    let bytes = fs::read(&input.path)
        .map_err(|err| format!("Read avatar file failed: {err}"))?;
    let base64 = B64.encode(bytes);
    Ok(AvatarDataUrlOutput {
        data_url: format!("data:image/webp;base64,{base64}"),
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
        .map_err(|_| "Failed to lock state mutex".to_string())?;
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
    state: State<'_, AppState>,
) -> Result<ConversationApiSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut config = read_config(&state.config_path)?;
    config.chat_api_config_id = input.chat_api_config_id.clone();
    config.vision_api_config_id = input.vision_api_config_id.clone();
    config.stt_api_config_id = input.stt_api_config_id.clone();
    config.stt_auto_send = input.stt_auto_send;
    normalize_app_config(&mut config);
    write_config(&state.config_path, &config)?;
    drop(guard);

    Ok(ConversationApiSettings {
        chat_api_config_id: config.chat_api_config_id,
        vision_api_config_id: config.vision_api_config_id,
        stt_api_config_id: config.stt_api_config_id,
        stt_auto_send: config.stt_auto_send,
    })
}

#[tauri::command]
fn get_chat_snapshot(
    _input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let effective_agent_id = if data
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
    let idx = ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id);
    let conversation = &data.conversations[idx];

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();
    let latest_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .cloned();

    if defaults_changed || data.conversations.len() != before_len {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);

    Ok(ChatSnapshot {
        conversation_id: conversation.id.clone(),
        latest_user,
        latest_assistant,
        active_message_count: conversation.messages.len(),
    })
}

#[tauri::command]
fn get_active_conversation_messages(
    _input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let effective_agent_id = if data
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
    let idx = ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id);
    let messages = data.conversations[idx].messages.clone();

    if defaults_changed || data.conversations.len() != before_len {
        write_app_data(&state.data_path, &data)?;
    }
    drop(guard);
    Ok(messages)
}

