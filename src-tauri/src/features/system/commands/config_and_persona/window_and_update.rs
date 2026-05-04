#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "main")
}

#[tauri::command]
fn show_chat_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "chat")
}

#[tauri::command]
fn show_archives_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "archives")
}

#[tauri::command]
fn show_quick_setup_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "quick-setup")
}

#[tauri::command]
fn complete_quick_setup_and_open_chat(app: AppHandle) -> Result<(), String> {
    show_window(&app, "chat")?;
    if let Some(window) = app.get_webview_window("quick-setup") {
        let _ = window.hide();
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebviewZoomUpdatedPayload {
    percent: u32,
}

fn emit_webview_zoom_percent_updated(app: &AppHandle, percent: u32) {
    let _ = app.emit(
        "easy-call:webview-zoom-updated",
        WebviewZoomUpdatedPayload { percent },
    );
}

fn apply_webview_zoom_percent(app: &AppHandle, percent: u32) -> Result<u32, String> {
    let normalized = normalize_webview_zoom_percent(percent);
    let scale_factor = normalized as f64 / 100.0;
    let mut failed = Vec::new();
    for (label, window) in app.webview_windows() {
        if let Err(err) = window.set_zoom(scale_factor) {
            failed.push(format!("{label}: {err}"));
        }
    }
    if !failed.is_empty() {
        return Err(format!("应用界面缩放失败：{}", failed.join("；")));
    }
    Ok(normalized)
}

#[tauri::command]
fn set_webview_zoom_percent(percent: u32, app: AppHandle) -> Result<u32, String> {
    let normalized = apply_webview_zoom_percent(&app, percent)?;
    emit_webview_zoom_percent_updated(&app, normalized);
    Ok(normalized)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetachedChatWindowInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetachedChatWindowOutput {
    conversation_id: String,
    window_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    main_conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetachedChatWindowInfo {
    detached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_label: Option<String>,
}

#[tauri::command]
fn detach_current_conversation_to_window(
    input: DetachedChatWindowInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DetachedChatWindowOutput, String> {
    let conversation_id = input.conversation_id.trim();
    eprintln!(
        "[独立聊天窗口] 收到独立窗口请求：conversation_id={}",
        conversation_id
    );
    if conversation_id.is_empty() {
        eprintln!("[独立聊天窗口] 拒绝：当前没有可独立出去的会话");
        return Err("当前没有可独立出去的会话。".to_string());
    }
    if get_conversation_runtime_state(&state, conversation_id)? == MainSessionState::OrganizingContext {
        eprintln!(
            "[独立聊天窗口] 拒绝：会话正在整理上下文 conversation_id={}",
            conversation_id
        );
        return Err("当前会话正在整理上下文，暂时不能独立出去。".to_string());
    }
    let runtime = state_read_runtime_state_cached(&state)?;
    let main_conversation_id = runtime
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if main_conversation_id.as_deref() == Some(conversation_id) {
        eprintln!(
            "[独立聊天窗口] 拒绝：主会话不能独立打开 conversation_id={}",
            conversation_id
        );
        return Err("主会话不能独立打开，请选择一个子会话。".to_string());
    }

    let conversation = state_read_conversation_cached(&state, conversation_id)?;
    if !conversation.summary.trim().is_empty()
        || (!conversation_visible_in_foreground_lists(&conversation)
            && !conversation_is_remote_im_contact(&conversation))
    {
        eprintln!(
            "[独立聊天窗口] 拒绝：会话不在前台列表 conversation_id={}",
            conversation_id
        );
        return Err("只能独立打开未归档前台会话或远程联系人会话。".to_string());
    }
    let title = if conversation.title.trim().is_empty() {
        conversation_preview_title(&conversation)
    } else {
        conversation.title.clone()
    };
    eprintln!(
        "[独立聊天窗口] 准备创建窗口：conversation_id={}，title={}",
        conversation_id,
        title
    );
    let window_label = open_detached_chat_window(&app, conversation_id, Some(&title))?;
    eprintln!(
        "[独立聊天窗口] 窗口请求已登记：conversation_id={}，window_label={}",
        conversation_id,
        window_label
    );
    emit_unarchived_conversation_overview_updated_from_state(&state)?;
    Ok(DetachedChatWindowOutput {
        conversation_id: conversation_id.to_string(),
        window_label,
        main_conversation_id,
    })
}

#[tauri::command]
fn get_detached_chat_window_info(window: tauri::Window) -> Result<DetachedChatWindowInfo, String> {
    let label = window.label().trim().to_string();
    if !is_detached_chat_window_label(&label) {
        return Ok(DetachedChatWindowInfo {
            detached: false,
            conversation_id: None,
            window_label: Some(label),
        });
    }
    Ok(DetachedChatWindowInfo {
        detached: true,
        conversation_id: detached_chat_conversation_for_window(&label),
        window_label: Some(label),
    })
}

#[tauri::command]
fn focus_detached_chat_window_by_conversation(
    input: DetachedChatWindowInput,
    app: AppHandle,
) -> Result<bool, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let Some(label) = detached_chat_window_for_conversation(conversation_id) else {
        return Ok(false);
    };
    if app.get_webview_window(&label).is_none() {
        let _ = unregister_detached_chat_window_by_label(&label);
        return Ok(false);
    }
    focus_detached_chat_window(&app, &label)?;
    Ok(true)
}

#[tauri::command]
fn set_chat_window_active(active: bool) {
    static CHAT_WINDOW_INACTIVE_LOGGED_ONCE: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    if !active && !CHAT_WINDOW_INACTIVE_LOGGED_ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!("[系统] 聊天窗口激活状态变更：跳过");
    }
    set_record_hotkey_probe_chat_window_active(active);
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
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

fn validate_department_names_unique(config: &AppConfig) -> Result<(), String> {
    let mut seen = std::collections::HashSet::<String>::new();
    for department in &config.departments {
        let name = department.name.trim();
        if name.is_empty() {
            return Err("部门名称不能为空".to_string());
        }
        let key = name.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(format!("部门名称不能重复：{name}"));
        }
    }
    Ok(())
}

fn runtime_config_with_private_organization(
    state: &AppState,
    config: &AppConfig,
    data: &AppData,
) -> Result<AppConfig, String> {
    let mut runtime_config = config.clone();
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut runtime_config, &mut runtime_data)?;
    Ok(runtime_config)
}

fn runtime_agents_with_private_organization(
    state: &AppState,
    config: &AppConfig,
    data: &AppData,
) -> Result<Vec<AgentProfile>, String> {
    let mut runtime_config = config.clone();
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut runtime_config, &mut runtime_data)?;
    Ok(runtime_data.agents)
}

fn private_agent_operation_error(agent_id: &str) -> String {
    format!("当前人格来自私有工作区，不能直接在主配置中修改：{agent_id}")
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

const GITHUB_REPO_PAGE: &str = "https://github.com/kawayiYokami/P-ai";

fn set_preferred_release_source(state: &AppState, source: &str) {
    match state.preferred_release_source.lock() {
        Ok(mut slot) => {
            *slot = source.to_string();
        }
        Err(err) => {
            eprintln!(
                "set_preferred_release_source 锁定 preferred_release_source 失败：source={}, err={}",
                source,
                err
            );
        }
    }
}

async fn probe_release_source_once(state: &AppState) {
    set_preferred_release_source(state, "github");
}

#[tauri::command]
fn get_project_repository_url(_state: State<'_, AppState>) -> String {
    GITHUB_REPO_PAGE.to_string()
}

#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let mut result = state_read_config_cached(&state)?;
    normalize_app_config(&mut result);
    let workspace_changed = ensure_default_shell_workspace_in_config(&mut result, &state);
    if workspace_changed {
        state_write_config_cached(&state, &result)?;
    }
    let mut runtime_data = state_read_agents_runtime_snapshot(&state)?;
    merge_private_organization_into_runtime_data(&state.data_path, &mut result, &mut runtime_data)?;
    Ok(result)
}

fn read_app_bootstrap_snapshot(state: &AppState) -> Result<AppBootstrapSnapshot, String> {
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    let workspace_changed = ensure_default_shell_workspace_in_config(&mut config, state);
    if workspace_changed {
        state_write_config_cached(state, &config)?;
    }
    let mut data = state_read_agents_runtime_snapshot(state)?;
    let assistant_agent_id =
        assistant_department_agent_id(&config).unwrap_or_else(default_assistant_department_agent_id);
    let runtime_changed = if data.assistant_department_agent_id != assistant_agent_id {
        data.assistant_department_agent_id = assistant_agent_id;
        true
    } else {
        false
    };
    if runtime_changed {
        state_write_runtime_state_cached(state, &build_runtime_state_file(&data))?;
    }
    let runtime_config = runtime_config_with_private_organization(state, &config, &data)?;
    let mut runtime_config_for_agents = config.clone();
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(
        &state.data_path,
        &mut runtime_config_for_agents,
        &mut runtime_data,
    )?;
    let chat_settings = ChatSettings {
        assistant_department_agent_id: data.assistant_department_agent_id.clone(),
        user_alias: user_persona_name(&runtime_data),
        response_style_id: data.response_style_id.clone(),
        pdf_read_mode: data.pdf_read_mode.clone(),
        background_voice_screenshot_keywords: data.background_voice_screenshot_keywords.clone(),
        background_voice_screenshot_mode: data.background_voice_screenshot_mode.clone(),
        instruction_presets: data.instruction_presets.clone(),
    };
    Ok(AppBootstrapSnapshot {
        config: runtime_config,
        agents: runtime_data.agents,
        chat_settings,
    })
}

#[tauri::command]
fn load_app_bootstrap_snapshot(state: State<'_, AppState>) -> Result<AppBootstrapSnapshot, String> {
    read_app_bootstrap_snapshot(&state)
}

#[tauri::command]
fn list_system_fonts() -> Result<Vec<String>, String> {
    let mut families = font_kit::source::SystemSource::new()
        .all_families()
        .map_err(|err| format!("列出系统字体失败：{err}"))?;
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
        return Err("至少需要配置一个 API 配置。".to_string());
    }
    let mut config = config;
    normalize_app_config(&mut config);
    let _ = ensure_default_shell_workspace_in_config(&mut config, &state);
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);

    let mut data = state_read_agents_runtime_snapshot(&state)?;
    let base_config = state_read_config_cached(&state)?;
    let departments_changed = {
        let old_by_id = base_config
            .departments
            .iter()
            .map(|item| (item.id.clone(), item.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        let new_by_id = config
            .departments
            .iter()
            .map(|item| (item.id.clone(), item.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        old_by_id
            .keys()
            .chain(new_by_id.keys())
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .filter(|id| old_by_id.get(id) != new_by_id.get(id))
            .collect::<Vec<_>>()
    };
    let shell_workspaces_changed = base_config.shell_workspaces != config.shell_workspaces;
    let (_private_agent_ids, private_department_ids) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    config.departments.retain(|item| !private_department_ids.contains(&item.id));
    validate_department_names_unique(&config)?;
    state_write_config_cached(&state, &config)?;
    if !departments_changed.is_empty() {
        mark_prompt_cache_rebuild_for_system_sources_by_departments(
            &state,
            &departments_changed,
        );
    }
    if shell_workspaces_changed {
        mark_prompt_cache_rebuild_for_all_system_environments(&state);
    }
    if let Some(agent_id) = assistant_department_agent_id(&config) {
        if data.assistant_department_agent_id != agent_id {
            data.assistant_department_agent_id = agent_id;
            state_write_runtime_state_cached(&state, &build_runtime_state_file(&data))?;
        }
    }
    register_hotkey_from_config(&app, &config)?;
    match apply_webview_zoom_percent(&app, config.webview_zoom_percent) {
        Ok(percent) => emit_webview_zoom_percent_updated(&app, percent),
        Err(err) => eprintln!("[外观] 应用界面缩放失败：{}", err),
    }
    let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(runtime_config)
}
