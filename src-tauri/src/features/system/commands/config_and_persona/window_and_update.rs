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
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("在 {}:{} {} 获取对话状态锁失败：{err}", file!(), line!(), module_path!()))?;
    let mut result = state_read_config_cached(&state)?;
    normalize_app_config(&mut result);
    let workspace_changed = ensure_default_shell_workspace_in_config(&mut result, &state);
    if workspace_changed {
        state_write_config_cached(&state, &result)?;
    }
    let data = state_read_app_data_cached(&state)?;
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut result, &mut runtime_data)?;
    drop(guard);
    Ok(result)
}

fn read_app_bootstrap_snapshot(state: &AppState) -> Result<AppBootstrapSnapshot, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("在 {}:{} {} 获取对话状态锁失败：{err}", file!(), line!(), module_path!()))?;
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
    drop(guard);
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

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("在 {}:{} {} 获取对话状态锁失败：{err}", file!(), line!(), module_path!()))?;

    let mut data = state_read_app_data_cached(&state)?;
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
        mark_prompt_cache_rebuild_for_departments(&state, &departments_changed);
    }
    if shell_workspaces_changed {
        mark_prompt_cache_rebuild_for_all_environments(&state);
    }
    if let Some(agent_id) = assistant_department_agent_id(&config) {
        if data.assistant_department_agent_id != agent_id {
            data.assistant_department_agent_id = agent_id;
            state_write_runtime_state_cached(&state, &build_runtime_state_file(&data))?;
        }
    }
    register_hotkey_from_config(&app, &config)?;
    let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;
    drop(guard);
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(runtime_config)
}
