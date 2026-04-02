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
        eprintln!("[系统] 聊天窗口激活状态变更，active=false");
    }
    set_record_hotkey_probe_chat_window_active(active);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubUpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    release_url: String,
    update_source: String,
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

const GITHUB_RELEASE_API: &str = "https://api.github.com/repos/kawayiYokami/P-ai/releases/latest";
const GITEE_RELEASE_API: &str = "https://gitee.com/api/v5/repos/yokami618/P-ai/releases/latest";
const GITHUB_RELEASE_PAGE: &str = "https://github.com/kawayiYokami/P-ai/releases/latest";
const GITEE_RELEASE_PAGE: &str = "https://gitee.com/yokami618/P-ai/releases";
const GITHUB_REPO_PAGE: &str = "https://github.com/kawayiYokami/P-ai";
const GITEE_REPO_PAGE: &str = "https://gitee.com/yokami618/P-ai";

fn set_preferred_release_source(state: &AppState, source: &str) {
    if let Ok(mut slot) = state.preferred_release_source.lock() {
        *slot = source.to_string();
    }
}

fn get_preferred_release_source(state: &AppState) -> String {
    state
        .preferred_release_source
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_else(|_| "github".to_string())
}

async fn probe_release_source_once(state: &AppState) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            eprintln!("[更新源] 初始化探测客户端失败: {}", err);
            return;
        }
    };
    let github_ok = client
        .get(GITHUB_RELEASE_API)
        .header(reqwest::header::USER_AGENT, "p-ai/startup-probe")
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false);
    let gitee_ok = client
        .get(GITEE_RELEASE_API)
        .header(reqwest::header::USER_AGENT, "p-ai/startup-probe")
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false);
    let selected = if github_ok {
        "github"
    } else if gitee_ok {
        "gitee"
    } else {
        "github"
    };
    set_preferred_release_source(state, selected);
    eprintln!(
        "[更新源] 启动探测完成: github_ok={}, gitee_ok={}, selected={}",
        github_ok, gitee_ok, selected
    );
}

async fn fetch_latest_release_from(
    api_url: &str,
    release_fallback_url: &str,
    current_version: &str,
) -> Result<(String, String), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|err| format!("Build update checker client failed: {err}"))?;
    let response = client
        .get(api_url)
        .header(
            reqwest::header::USER_AGENT,
            format!("p-ai/{current_version}"),
        )
        .header(reqwest::header::ACCEPT, "application/json")
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
        .or_else(|| payload.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|v| v.trim_start_matches(['v', 'V']))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "release version is empty".to_string())?
        .to_string();
    let release_url = payload
        .get("html_url")
        .or_else(|| payload.get("url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(release_fallback_url)
        .to_string();
    Ok((latest_version, release_url))
}

#[tauri::command]
async fn check_github_update(state: State<'_, AppState>) -> Result<GithubUpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let preferred = get_preferred_release_source(state.inner());
    let primary = if preferred == "gitee" {
        ("gitee", GITEE_RELEASE_API, GITEE_RELEASE_PAGE)
    } else {
        ("github", GITHUB_RELEASE_API, GITHUB_RELEASE_PAGE)
    };
    let secondary = if primary.0 == "github" {
        ("gitee", GITEE_RELEASE_API, GITEE_RELEASE_PAGE)
    } else {
        ("github", GITHUB_RELEASE_API, GITHUB_RELEASE_PAGE)
    };
    let (source, latest_version, release_url) =
        match fetch_latest_release_from(primary.1, primary.2, &current_version).await {
            Ok((latest_version, release_url)) => {
                set_preferred_release_source(state.inner(), primary.0);
                (primary.0.to_string(), latest_version, release_url)
            }
            Err(first_err) => {
                match fetch_latest_release_from(secondary.1, secondary.2, &current_version).await {
                    Ok((latest_version, release_url)) => {
                        set_preferred_release_source(state.inner(), secondary.0);
                        (secondary.0.to_string(), latest_version, release_url)
                    }
                    Err(second_err) => {
                        return Err(format!(
                            "Check update failed: primary={} err={}, secondary={} err={}",
                            primary.0, first_err, secondary.0, second_err
                        ));
                    }
                }
            }
        };

    Ok(GithubUpdateInfo {
        current_version: current_version.clone(),
        latest_version: latest_version.clone(),
        has_update: is_newer_version(&current_version, &latest_version),
        release_url,
        update_source: source,
    })
}

#[tauri::command]
fn get_project_repository_url(state: State<'_, AppState>) -> String {
    if get_preferred_release_source(state.inner()) == "gitee" {
        GITEE_REPO_PAGE.to_string()
    } else {
        GITHUB_REPO_PAGE.to_string()
    }
}

#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut result = state_read_config_cached(&state)?;
    normalize_app_config(&mut result);
    let workspace_changed = ensure_default_shell_workspace_in_config(&mut result, &state);
    if workspace_changed {
        state_write_config_cached(&state, &result)?;
    }
    let mut data = state_read_app_data_cached(&state)?;
    let changed = ensure_default_agent(&mut data);
    if changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut result, &mut runtime_data)?;
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
    let _ = ensure_default_shell_workspace_in_config(&mut config, &state);
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = state_read_app_data_cached(&state)?;
    let _ = ensure_default_agent(&mut data);
    let base_config = state_read_config_cached(&state)?;
    let (_private_agent_ids, private_department_ids) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    config.departments.retain(|item| !private_department_ids.contains(&item.id));
    validate_department_names_unique(&config)?;
    state_write_config_cached(&state, &config)?;
    if let Some(agent_id) = assistant_department_agent_id(&config) {
        if data.assistant_department_agent_id != agent_id {
            data.assistant_department_agent_id = agent_id;
            state_write_app_data_cached(&state, &data)?;
        }
    }
    register_hotkey_from_config(&app, &config)?;
    let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;
    drop(guard);
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(runtime_config)
}

#[tauri::command]
fn load_agents(state: State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let changed = ensure_default_agent(&mut data);
    if changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut config, &mut runtime_data)?;
    drop(guard);
    Ok(runtime_data.agents)
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let base_config = read_config(&state.config_path)?;
    let mut data = state_read_app_data_cached(&state)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    let previous_agents = data.agents.clone();
    let existing_user_persona = data
        .agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID)
        .cloned();
    let existing_system_persona = data
        .agents
        .iter()
        .find(|a| a.id == SYSTEM_PERSONA_ID)
        .cloned();
    data.agents = input
        .agents
        .into_iter()
        .filter(|agent| !private_agent_ids.contains(&agent.id))
        .collect();
    if !data.agents.iter().any(|a| a.id == USER_PERSONA_ID) {
        if let Some(user_persona) = existing_user_persona {
            data.agents.push(user_persona);
        }
    }
    if !data.agents.iter().any(|a| a.id == SYSTEM_PERSONA_ID) {
        if let Some(system_persona) = existing_system_persona {
            data.agents.push(system_persona);
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
        .filter(|a| !a.is_built_in_user && !a.is_built_in_system && a.id != USER_PERSONA_ID && a.id != SYSTEM_PERSONA_ID)
        .filter(|a| !next_ids.contains(&a.id))
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    let disabled_private_memory_agent_ids = data
        .agents
        .iter()
        .filter(|a| !a.is_built_in_user && !a.is_built_in_system && a.id != USER_PERSONA_ID && a.id != SYSTEM_PERSONA_ID)
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

    state_write_app_data_cached(&state, &data)?;
    let mut config = state_read_config_cached(&state)?;
    let runtime_agents = runtime_agents_with_private_organization(&state, &config, &data)?;
    let valid_agent_ids = runtime_agents
        .iter()
        .filter(|a| !a.is_built_in_user)
        .map(|a| a.id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut config_changed = false;
    for dept in &mut config.departments {
        let original_len = dept.agent_ids.len();
        dept.agent_ids.retain(|id| valid_agent_ids.contains(id));
        if dept.id == ASSISTANT_DEPARTMENT_ID && dept.agent_ids.is_empty() {
            dept.agent_ids.push(data.assistant_department_agent_id.clone());
        }
        if dept.agent_ids.len() != original_len || (dept.id == ASSISTANT_DEPARTMENT_ID && dept.agent_ids.first() != Some(&data.assistant_department_agent_id)) {
            config_changed = true;
            if dept.id == ASSISTANT_DEPARTMENT_ID {
                dept.agent_ids = vec![data.assistant_department_agent_id.clone()];
            }
            dept.updated_at = now_iso();
        }
    }
    if config_changed {
        normalize_app_config(&mut config);
        state_write_config_cached(&state, &config)?;
    }
    let runtime_agents = runtime_agents_with_private_organization(&state, &config, &data)?;
    drop(guard);
    Ok(runtime_agents)
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
    let config = read_config(&state.config_path)?;
    let data = state_read_app_data_cached(&state)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &config, &data.agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    if private_agent_ids.contains(agent_id) {
        drop(guard);
        return Err(private_agent_operation_error(agent_id));
    }

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
        state_write_app_data_cached(&state, &data)?;
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
    state_write_app_data_cached(&state, &data)?;
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
    let config = read_config(&state.config_path)?;
    let data = state_read_app_data_cached(&state)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &config, &data.agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    if private_agent_ids.contains(agent_id) {
        drop(guard);
        return Err(private_agent_operation_error(agent_id));
    }

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
    state_write_app_data_cached(&state, &data)?;
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    if private_agent_ids.contains(agent_id) {
        drop(guard);
        return Err(private_agent_operation_error(agent_id));
    }
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut config = read_config(&state.config_path)?;
    let mut data = state_read_app_data_cached(&state)?;
    let changed = ensure_default_agent(&mut data);
    let assistant_agent_id = assistant_department_agent_id(&config).unwrap_or_else(default_assistant_department_agent_id);
    let runtime_changed = if data.assistant_department_agent_id != assistant_agent_id {
        data.assistant_department_agent_id = assistant_agent_id.clone();
        true
    } else {
        false
    };
    if changed || runtime_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut config, &mut runtime_data)?;
    drop(guard);

    Ok(ChatSettings {
        assistant_department_agent_id: data.assistant_department_agent_id.clone(),
        user_alias: user_persona_name(&runtime_data),
        response_style_id: data.response_style_id.clone(),
        pdf_read_mode: data.pdf_read_mode.clone(),
        background_voice_screenshot_keywords: data.background_voice_screenshot_keywords.clone(),
        background_voice_screenshot_mode: data.background_voice_screenshot_mode.clone(),
    })
}

#[tauri::command]
fn save_chat_settings(
    input: ChatSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    let config = read_config(&state.config_path)?;
    let mut runtime_config = config.clone();
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut runtime_config, &mut runtime_data)?;
    let target_agent_id = assistant_department_agent_id(&config).unwrap_or_else(|| input.assistant_department_agent_id.clone());
    if !runtime_data
        .agents
        .iter()
        .any(|a| a.id == target_agent_id && !a.is_built_in_user)
    {
        return Err("Selected agent not found.".to_string());
    }
    data.assistant_department_agent_id = target_agent_id.clone();
    data.user_alias = user_persona_name(&data);
    data.response_style_id = normalize_response_style_id(&input.response_style_id);
    data.pdf_read_mode = normalize_pdf_read_mode(&input.pdf_read_mode);
    data.background_voice_screenshot_keywords = input
        .background_voice_screenshot_keywords
        .trim()
        .to_string();
    data.background_voice_screenshot_mode =
        normalize_background_voice_screenshot_mode(&input.background_voice_screenshot_mode);
    state_write_app_data_cached(&state, &data)?;
    drop(guard);

    let payload = ChatSettings {
        assistant_department_agent_id: target_agent_id,
        user_alias: data.user_alias,
        response_style_id: data.response_style_id,
        pdf_read_mode: data.pdf_read_mode,
        background_voice_screenshot_keywords: data.background_voice_screenshot_keywords,
        background_voice_screenshot_mode: data.background_voice_screenshot_mode,
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let _ = ensure_default_agent(&mut data);
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    if private_agent_ids.contains(input.agent_id.trim()) {
        drop(guard);
        return Err(private_agent_operation_error(input.agent_id.trim()));
    }

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
    state_write_app_data_cached(&state, &data)?;
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let _ = ensure_default_agent(&mut data);
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    if private_agent_ids.contains(input.agent_id.trim()) {
        drop(guard);
        return Err(private_agent_operation_error(input.agent_id.trim()));
    }
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
    state_write_app_data_cached(&state, &data)?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatImageDataUrlInput {
    media_ref: String,
    mime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatImageDataUrlOutput {
    data_url: String,
}

#[tauri::command]
fn read_chat_image_data_url(
    input: ChatImageDataUrlInput,
    state: State<'_, AppState>,
) -> Result<ChatImageDataUrlOutput, String> {
    let media_ref = input.media_ref.trim();
    if media_ref.is_empty() {
        return Ok(ChatImageDataUrlOutput {
            data_url: String::new(),
        });
    }
    if media_id_from_marker(media_ref).is_none() {
        return Err("Chat image mediaRef is invalid.".to_string());
    }
    let mime = input.mime.trim().to_ascii_lowercase();
    if !mime.starts_with("image/") {
        return Err("Chat image mime is invalid.".to_string());
    }
    let base64 = resolve_stored_binary_base64(&state.data_path, media_ref)?;
    Ok(ChatImageDataUrlOutput {
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let changed = ensure_default_agent(&mut data);
    if changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(data.assistant_department_agent_id.as_str());
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut config = state_read_config_cached(&state)?;
    config.assistant_department_api_config_id = input.assistant_department_api_config_id.clone();
    config.vision_api_config_id = input.vision_api_config_id.clone();
    config.stt_api_config_id = input.stt_api_config_id.clone();
    config.stt_auto_send = input.stt_auto_send;
    normalize_app_config(&mut config);
    let assistant_api_config_id = config.assistant_department_api_config_id.clone();
    if let Some(dept) = assistant_department_mut(&mut config) {
        let cleaned = assistant_api_config_id.trim();
        dept.api_config_ids = if cleaned.is_empty() {
            Vec::new()
        } else {
            vec![cleaned.to_string()]
        };
        dept.api_config_id = if cleaned.is_empty() {
            String::new()
        } else {
            cleaned.to_string()
        };
        dept.updated_at = now_iso();
    }
    state_write_config_cached(&state, &config)?;
    drop(guard);

    let payload = ConversationApiSettings {
        assistant_department_api_config_id: config.assistant_department_api_config_id,
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;

    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    if defaults_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
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

    if data.conversations.len() != before_len {
        state_write_app_data_cached(&state, &data)?;
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
struct ConversationPreviewMessage {
    message_id: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text_preview: String,
    #[serde(default)]
    has_image: bool,
    #[serde(default)]
    has_pdf: bool,
    #[serde(default)]
    has_audio: bool,
    #[serde(default)]
    has_attachment: bool,
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
    workspace_label: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_main_conversation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_state: Option<MainSessionState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    preview_messages: Vec<ConversationPreviewMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DelegateConversationSummary {
    conversation_id: String,
    title: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_message_at: Option<String>,
    message_count: usize,
    agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
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

fn build_conversation_preview_text(message: &ChatMessage) -> String {
    let text = message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    clean_text(text.trim())
}

fn conversation_message_has_attachment(message: &ChatMessage) -> bool {
    message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("attachments"))
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

fn build_conversation_preview_messages(
    conversation: &Conversation,
    limit: usize,
) -> Vec<ConversationPreviewMessage> {
    let mut selected = conversation
        .messages
        .iter()
        .filter(|message| {
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant" | "tool"
            )
        })
        .rev()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    selected.reverse();
    selected
        .into_iter()
        .map(|message| {
            let mut has_image = false;
            let mut has_pdf = false;
            let mut has_audio = false;
            for part in &message.parts {
                match part {
                    MessagePart::Image { mime, .. } => {
                        if mime.trim().eq_ignore_ascii_case("application/pdf") {
                            has_pdf = true;
                        } else {
                            has_image = true;
                        }
                    }
                    MessagePart::Audio { .. } => {
                        has_audio = true;
                    }
                    MessagePart::Text { .. } => {}
                }
            }
            ConversationPreviewMessage {
                message_id: message.id.clone(),
                role: message.role.clone(),
                speaker_agent_id: message
                    .speaker_agent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                created_at: Some(message.created_at.clone())
                    .filter(|value| !value.trim().is_empty()),
                text_preview: build_conversation_preview_text(&message),
                has_image,
                has_pdf,
                has_audio,
                has_attachment: conversation_message_has_attachment(&message),
            }
        })
        .collect()
}

fn workspace_label_for_unarchived_conversation(
    state: &AppState,
    conversation: &Conversation,
) -> String {
    if let Some(path) = terminal_workspace_path_from_conversation(conversation) {
        return resolve_workspace_display_name(state, &path);
    }
    "默认工作空间".to_string()
}

fn build_unarchived_conversation_summary(
    state: &AppState,
    main_conversation_id: &str,
    conversation: &Conversation,
) -> UnarchivedConversationSummary {
    let last_message_at = conversation.messages.last().map(|m| m.created_at.clone());
    UnarchivedConversationSummary {
        conversation_id: conversation.id.clone(),
        title: if conversation.title.trim().is_empty() {
            conversation_preview_title(conversation)
        } else {
            conversation.title.clone()
        },
        updated_at: conversation.updated_at.clone(),
        last_message_at,
        message_count: conversation.messages.len(),
        agent_id: conversation.agent_id.clone(),
        workspace_label: workspace_label_for_unarchived_conversation(state, conversation),
        is_active: conversation.status.trim() == "active",
        is_main_conversation: conversation.id.trim() == main_conversation_id,
        runtime_state: unarchived_conversation_runtime_state(state, &conversation.id),
        preview_messages: build_conversation_preview_messages(conversation, 2),
    }
}

fn collect_unarchived_conversation_summaries(
    state: &AppState,
    data: &AppData,
) -> Vec<UnarchivedConversationSummary> {
    let main_conversation_id = data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let mut summaries = data
        .conversations
        .iter()
        .filter(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
        })
        .map(|conversation| build_unarchived_conversation_summary(state, &main_conversation_id, conversation))
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| {
        if a.is_main_conversation != b.is_main_conversation {
            return b.is_main_conversation.cmp(&a.is_main_conversation);
        }
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
    summaries
}

fn delegate_conversation_summary_from_runtime_thread(
    thread: &DelegateRuntimeThread,
) -> DelegateConversationSummary {
    let last_message_at = thread
        .conversation
        .messages
        .last()
        .map(|m| m.created_at.clone());
    DelegateConversationSummary {
        conversation_id: thread.delegate_id.clone(),
        title: if thread.title.trim().is_empty() {
            conversation_preview_title(&thread.conversation)
        } else {
            thread.title.clone()
        },
        updated_at: thread.conversation.updated_at.clone(),
        last_message_at,
        message_count: thread.conversation.messages.len(),
        agent_id: thread.target_agent_id.clone(),
        delegate_id: Some(thread.delegate_id.clone()),
        root_conversation_id: Some(thread.root_conversation_id.clone()),
        archived_at: thread.archived_at.clone(),
    }
}

fn unarchived_conversation_runtime_state(
    state: &AppState,
    conversation_id: &str,
) -> Option<MainSessionState> {
    match get_conversation_runtime_state(state, conversation_id) {
        Ok(MainSessionState::Idle) => None,
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!(
                "[会话] 读取运行态失败，任务=unarchived_runtime_state，conversation_id={}，error={}",
                conversation_id, err
            );
            None
        }
    }
}

fn ensure_unarchived_conversation_not_organizing(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    if get_conversation_runtime_state(state, conversation_id)? == MainSessionState::OrganizingContext {
        return Err("当前会话正在后台归档或整理上下文，暂时不能切换。".to_string());
    }
    Ok(())
}

#[tauri::command]
fn list_unarchived_conversations(state: State<'_, AppState>) -> Result<Vec<UnarchivedConversationSummary>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let summaries = collect_unarchived_conversation_summaries(state.inner(), &data);
    if defaults_changed || normalized_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);
    Ok(summaries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetActiveUnarchivedConversationInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetActiveUnarchivedConversationOutput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchActiveConversationSnapshotInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchActiveConversationSnapshotOutput {
    conversation_id: String,
    messages: Vec<ChatMessage>,
    has_more_history: bool,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnarchivedConversationOverviewUpdatedPayload {
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_conversation_id: Option<String>,
}

fn build_unarchived_conversation_overview_payload(
    state: &AppState,
    data: &AppData,
) -> UnarchivedConversationOverviewUpdatedPayload {
    let unarchived_conversations = collect_unarchived_conversation_summaries(state, data);
    let preferred_conversation_id = unarchived_conversations
        .first()
        .map(|item| item.conversation_id.clone());
    UnarchivedConversationOverviewUpdatedPayload {
        unarchived_conversations,
        preferred_conversation_id,
    }
}

fn emit_unarchived_conversation_overview_updated_payload(
    state: &AppState,
    payload: &UnarchivedConversationOverviewUpdatedPayload,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        eprintln!("[会话概览] 推送跳过：无法获取 app_handle");
        return;
    };
    if let Err(err) = app_handle.emit(CHAT_CONVERSATION_OVERVIEW_UPDATED_EVENT, payload) {
        eprintln!("[会话概览] 推送失败：错误={}", err);
    }
}

fn emit_unarchived_conversation_overview_updated_from_state(state: &AppState) -> Result<(), String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    if defaults_changed || normalized_changed {
        state_write_app_data_cached(state, &data)?;
    }
    let payload = build_unarchived_conversation_overview_payload(state, &data);
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state, &payload);
    Ok(())
}

#[tauri::command]
fn set_active_unarchived_conversation(
    input: SetActiveUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<SetActiveUnarchivedConversationOutput, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let requested_conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut target_idx = requested_conversation_id.and_then(|conversation_id| {
        data.conversations.iter().position(|item| {
            item.id == conversation_id
                && item.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(item)
        })
    });
    if target_idx.is_none() {
        target_idx = latest_active_conversation_index(&data, "", "")
            .or_else(|| {
                data.conversations
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        item.summary.trim().is_empty()
                            && conversation_visible_in_foreground_lists(item)
                    })
                    .max_by(|(idx_a, a), (idx_b, b)| {
                        let a_updated = a.updated_at.trim();
                        let b_updated = b.updated_at.trim();
                        let a_created = a.created_at.trim();
                        let b_created = b.created_at.trim();
                        a_updated
                            .cmp(b_updated)
                            .then_with(|| a_created.cmp(b_created))
                            .then_with(|| idx_a.cmp(idx_b))
                    })
                    .map(|(idx, _)| idx)
            });
    }
    if target_idx.is_none() {
        let api_config = resolve_selected_api_config(&app_config, None)
            .ok_or_else(|| "No API config available".to_string())?;
        target_idx = Some(ensure_active_conversation_index(&mut data, &api_config.id, ""));
    }
    let target_idx = target_idx.ok_or_else(|| "Unarchived conversation not found.".to_string())?;
    let conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    ensure_unarchived_conversation_not_organizing(state.inner(), &conversation_id)?;

    let mut changed = defaults_changed || normalized_changed;
    for (_idx, conversation) in data.conversations.iter_mut().enumerate() {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        let target_status = "active";
        if conversation.status.trim() != target_status {
            conversation.status = target_status.to_string();
            changed = true;
        }
    }
    if changed {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);
    Ok(SetActiveUnarchivedConversationOutput { conversation_id })
}

#[tauri::command]
fn switch_active_conversation_snapshot(
    input: SwitchActiveConversationSnapshotInput,
    state: State<'_, AppState>,
) -> Result<SwitchActiveConversationSnapshotOutput, String> {
    const SWITCH_SNAPSHOT_RECENT_LIMIT: usize = 50;
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    let requested_conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let effective_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| data.assistant_department_agent_id.clone());
    let target_idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;
    let target_conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    ensure_unarchived_conversation_not_organizing(state.inner(), &target_conversation_id)?;

    let mut changed = defaults_changed || normalized_changed;
    for (_idx, conversation) in data.conversations.iter_mut().enumerate() {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        let target_status = "active";
        if conversation.status.trim() != target_status {
            conversation.status = target_status.to_string();
            changed = true;
        }
    }

    let conversation_id = data
        .conversations
        .get(target_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
    let all_messages = data
        .conversations
        .get(target_idx)
        .map(|item| item.messages.clone())
        .ok_or_else(|| "Unarchived conversation messages not found.".to_string())?;
    let total_messages = all_messages.len();
    let start = total_messages.saturating_sub(SWITCH_SNAPSHOT_RECENT_LIMIT);
    let mut messages = all_messages[start..].to_vec();
    let has_more_history = start > 0;
    let unarchived_conversations = collect_unarchived_conversation_summaries(state.inner(), &data);

    if changed {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);

    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);

    Ok(SwitchActiveConversationSnapshotOutput {
        conversation_id,
        messages,
        has_more_history,
        unarchived_conversations,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationInput {
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationOutput {
    conversation_id: String,
}

#[tauri::command]
fn create_unarchived_conversation(
    input: CreateUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<CreateUnarchivedConversationOutput, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);

    let api_config_id = input
        .api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| resolve_selected_api_config(&app_config, None).map(|item| item.id.clone()))
        .ok_or_else(|| "No API config available".to_string())?;
    let conversation_title = input
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    for conversation in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        conversation.status = "active".to_string();
    }
    let conversation = build_foreground_chat_conversation_record(
        &state.data_path,
        &data,
        &api_config_id,
        &data.assistant_department_agent_id,
        conversation_title,
    );
    let conversation_id = conversation.id.clone();
    data.conversations.push(conversation);
    if data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        data.main_conversation_id = Some(conversation_id.clone());
    }
    let overview_payload = build_unarchived_conversation_overview_payload(state.inner(), &data);
    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);

    Ok(CreateUnarchivedConversationOutput { conversation_id })
}

#[tauri::command]
fn list_delegate_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<DelegateConversationSummary>, String> {
    let mut threads = delegate_runtime_thread_list(state.inner())?;
    threads.extend(delegate_recent_thread_list(state.inner())?);
    let mut summaries = threads
        .iter()
        .map(delegate_conversation_summary_from_runtime_thread)
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| {
        let bk = b
            .archived_at
            .as_deref()
            .or(b.last_message_at.as_deref())
            .unwrap_or(b.updated_at.as_str());
        let ak = a
            .archived_at
            .as_deref()
            .or(a.last_message_at.as_deref())
            .unwrap_or(a.updated_at.as_str());
        bk.cmp(ak).then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    Ok(summaries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationMessagesInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationRecentMessagesInput {
    conversation_id: String,
    #[serde(default = "default_recent_unarchived_message_limit")]
    limit: usize,
}

fn default_recent_unarchived_message_limit() -> usize {
    5
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
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let requested_messages = data
        .conversations
        .iter()
        .find(|c| c.summary.trim().is_empty() && conversation_visible_in_foreground_lists(c) && c.id == conversation_id)
        .map(|c| c.messages.clone());
    let fallback_messages = latest_active_conversation_index(&data, "", "")
        .and_then(|idx| data.conversations.get(idx))
        .map(|c| c.messages.clone());
    let mut messages = requested_messages
        .or(fallback_messages)
        .ok_or_else(|| "Unarchived conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[tauri::command]
fn get_unarchived_conversation_recent_messages(
    input: GetUnarchivedConversationRecentMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 50);
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let mut messages = data
        .conversations
        .iter()
        .find(|c| c.summary.trim().is_empty() && conversation_visible_in_foreground_lists(c) && c.id == conversation_id)
        .map(|c| {
            let total = c.messages.len();
            let start = total.saturating_sub(limit);
            c.messages[start..].to_vec()
        })
        .ok_or_else(|| "Unarchived conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetDelegateConversationMessagesInput {
    conversation_id: String,
}

#[tauri::command]
fn get_delegate_conversation_messages(
    input: GetDelegateConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let mut messages = delegate_runtime_thread_conversation_get_any(state.inner(), conversation_id)?
        .map(|conversation| conversation.messages.clone())
        .ok_or_else(|| "Delegate conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationOutput {
    deleted_conversation_id: String,
    active_conversation_id: String,
}

#[tauri::command]
fn delete_unarchived_conversation(
    input: DeleteUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<DeleteUnarchivedConversationOutput, String> {
    eprintln!("[会话] 请求删除当前未归档主会话");
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let before = data.conversations.len();
    data.conversations.retain(|conversation| {
        !(conversation.id == conversation_id
            && conversation.summary.trim().is_empty()
            && conversation_visible_in_foreground_lists(conversation))
    });
    if data.conversations.len() == before {
        drop(guard);
        return Err("Unarchived conversation not found.".to_string());
    }
    let _ = normalize_main_conversation_marker(&mut data, "");
    if latest_main_conversation_index(&data, "").is_none() {
        let _ = ensure_active_foreground_conversation_index_atomic(
            &mut data,
            &state.data_path,
            "",
            "",
        );
    }
    let _ = normalize_single_active_main_conversation(&mut data);
    let active_conversation_id = data
        .conversations
        .iter()
        .find(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
                && conversation.status.trim() == "active"
        })
        .map(|conversation| conversation.id.clone())
        .or_else(|| {
            data.conversations
                .iter()
                .find(|conversation| {
                    conversation.summary.trim().is_empty() && conversation_visible_in_foreground_lists(conversation)
                })
                .map(|conversation| conversation.id.clone())
        })
        .unwrap_or_default();
    let overview_payload = build_unarchived_conversation_overview_payload(state.inner(), &data);
    state_write_app_data_cached(&state, &data)?;
    eprintln!(
        "[会话] 已删除未归档主会话: conversation_id={}",
        conversation_id
    );
    drop(guard);
    emit_unarchived_conversation_overview_updated_payload(state.inner(), &overview_payload);
    cleanup_pdf_session_memory_cache_for_conversation(conversation_id);
    Ok(DeleteUnarchivedConversationOutput {
        deleted_conversation_id: conversation_id.to_string(),
        active_conversation_id,
    })
}

#[tauri::command]
fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;

    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    if defaults_changed || normalized_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let before_len = data.conversations.len();
    let requested_conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;

    let mut messages = data.conversations[idx].messages.clone();

    if data.conversations.len() != before_len {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesBeforeInput {
    session: SessionSelector,
    before_message_id: String,
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesBeforeOutput {
    messages: Vec<ChatMessage>,
    has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesAfterInput {
    session: SessionSelector,
    after_message_id: String,
    #[serde(default = "default_message_page_limit")]
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesAfterOutput {
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestConversationMessagesAfterAsyncInput {
    conversation_id: String,
    #[serde(default)]
    after_message_id: Option<String>,
    #[serde(default = "default_recent_unarchived_message_limit")]
    fallback_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestConversationMessagesAfterAsyncOutput {
    accepted: bool,
    request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMessagesAfterAsyncPayload {
    request_id: String,
    conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_message_id: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn default_message_page_limit() -> usize {
    100
}

fn resolve_unarchived_conversation_index_with_fallback(
    data: &mut AppData,
    app_config: &AppConfig,
    effective_agent_id: &str,
    requested_conversation_id: Option<&str>,
) -> Result<usize, String> {
    if let Some(conversation_id) = requested_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(idx) = data.conversations.iter().position(|item| {
            item.id == conversation_id
                && item.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(item)
        }) {
            return Ok(idx);
        }
        runtime_log_warn(format!(
            "[解析对话索引] 请求的conversation_id不存在，终止本次读取: '{}' (agent_id: '{}')",
            conversation_id, effective_agent_id
        ));
        return Err(format!(
            "Requested conversation not found: {conversation_id}"
        ));
    }

    if let Some(existing_idx) = latest_active_conversation_index(data, "", effective_agent_id) {
        return Ok(existing_idx);
    }

    let api_config = resolve_selected_api_config(app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;
    Ok(ensure_active_conversation_index(
        data,
        &api_config.id,
        effective_agent_id,
    ))
}

fn resolve_unarchived_conversation_messages_after(
    state: &AppState,
    conversation_id: &str,
    after_message_id: Option<&str>,
    fallback_limit: usize,
) -> Result<(Vec<ChatMessage>, Option<String>), String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(state)?;
    let conversation = data
        .conversations
        .iter()
        .find(|item| {
            item.id == conversation_id
                && item.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(item)
        })
        .ok_or_else(|| format!("Unarchived conversation not found: {conversation_id}"))?;
    let messages = conversation.messages.clone();
    drop(guard);

    let trimmed_after = after_message_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (mut page, fallback_mode) = if let Some(after_id) = trimmed_after {
        if let Some(after_idx) = messages.iter().position(|item| item.id == after_id) {
            (messages[(after_idx + 1)..].to_vec(), None)
        } else {
            let start = messages.len().saturating_sub(fallback_limit);
            (messages[start..].to_vec(), Some("recent_limit".to_string()))
        }
    } else {
        let start = messages.len().saturating_sub(fallback_limit);
        (messages[start..].to_vec(), Some("recent_limit".to_string()))
    };
    materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
    Ok((page, fallback_mode))
}

#[tauri::command]
fn get_active_conversation_messages_before(
    input: GetActiveConversationMessagesBeforeInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationMessagesBeforeOutput, String> {
    let before_message_id = input.before_message_id.trim();
    if before_message_id.is_empty() {
        return Err("beforeMessageId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 100);

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    if defaults_changed || normalized_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.session.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let before_len = data.conversations.len();
    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;

    let messages = data.conversations[idx].messages.clone();
    if data.conversations.len() != before_len {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);

    let before_idx = messages
        .iter()
        .position(|item| item.id == before_message_id)
        .ok_or_else(|| format!("beforeMessageId not found: {before_message_id}"))?;

    let start = before_idx.saturating_sub(limit);
    let has_more = start > 0;
    let mut page = messages[start..before_idx].to_vec();
    materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
    Ok(GetActiveConversationMessagesBeforeOutput {
        messages: page,
        has_more,
    })
}

#[tauri::command]
fn get_active_conversation_messages_after(
    input: GetActiveConversationMessagesAfterInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationMessagesAfterOutput, String> {
    let after_message_id = input.after_message_id.trim();
    if after_message_id.is_empty() {
        return Err("afterMessageId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 200);

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);
    if defaults_changed || normalized_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.session.agent_id.trim();
    if !requested_agent_id.is_empty()
        && !runtime_data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }
    let effective_agent_id = if !requested_agent_id.is_empty() {
        requested_agent_id.to_string()
    } else if runtime_data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        runtime_data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let before_len = data.conversations.len();
    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = resolve_unarchived_conversation_index_with_fallback(
        &mut data,
        &app_config,
        &effective_agent_id,
        requested_conversation_id,
    )?;

    let messages = data.conversations[idx].messages.clone();
    if data.conversations.len() != before_len {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);

    let after_idx = messages
        .iter()
        .position(|item| item.id == after_message_id)
        .ok_or_else(|| format!("afterMessageId not found: {after_message_id}"))?;

    let end = (after_idx + 1 + limit).min(messages.len());
    let mut page = messages[(after_idx + 1)..end].to_vec();
    materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
    Ok(GetActiveConversationMessagesAfterOutput {
        messages: page,
    })
}

#[tauri::command]
fn request_conversation_messages_after_async(
    input: RequestConversationMessagesAfterAsyncInput,
    state: State<'_, AppState>,
) -> Result<RequestConversationMessagesAfterAsyncOutput, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let request_id = Uuid::new_v4().to_string();
    let after_message_id = input
        .after_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let fallback_limit = input.fallback_limit.clamp(1, 50);
    let state_clone = state.inner().clone();
    let request_id_clone = request_id.clone();
    eprintln!(
        "[聊天推送] 收到异步补消息请求: request_id={}, conversation_id={}, after_message_id={}, fallback_limit={}",
        request_id,
        conversation_id,
        after_message_id.as_deref().unwrap_or(""),
        fallback_limit
    );
    tauri::async_runtime::spawn(async move {
        let payload = match resolve_unarchived_conversation_messages_after(
            &state_clone,
            &conversation_id,
            after_message_id.as_deref(),
            fallback_limit,
        ) {
            Ok((messages, fallback_mode)) => {
                eprintln!(
                    "[聊天推送] 异步补消息完成: request_id={}, conversation_id={}, message_count={}, fallback_mode={}",
                    request_id_clone,
                    conversation_id,
                    messages.len(),
                    fallback_mode.as_deref().unwrap_or("")
                );
                ConversationMessagesAfterAsyncPayload {
                    request_id: request_id_clone.clone(),
                    conversation_id: conversation_id.clone(),
                    after_message_id: after_message_id.clone(),
                    messages,
                    fallback_mode,
                    error: None,
                }
            }
            Err(error) => {
                eprintln!(
                    "[聊天推送] 异步补消息失败: request_id={}, conversation_id={}, error={}",
                    request_id_clone, conversation_id, error
                );
                ConversationMessagesAfterAsyncPayload {
                    request_id: request_id_clone.clone(),
                    conversation_id: conversation_id.clone(),
                    after_message_id: after_message_id.clone(),
                    messages: Vec::new(),
                    fallback_mode: None,
                    error: Some(error),
                }
            }
        };
        let app_handle = match state_clone.app_handle.lock() {
            Ok(guard) => guard.as_ref().cloned(),
            Err(_) => None,
        };
        let Some(app_handle) = app_handle else {
            eprintln!(
                "[聊天推送] 异步补消息 emit 跳过: app_handle unavailable, request_id={}, conversation_id={}",
                request_id_clone, conversation_id
            );
            return;
        };
        match app_handle.emit("easy-call:conversation-messages-after-synced", &payload) {
            Ok(_) => eprintln!(
                "[聊天推送] 异步补消息 emit 成功: request_id={}, conversation_id={}",
                request_id_clone, conversation_id
            ),
            Err(err) => eprintln!(
                "[聊天推送] 异步补消息 emit 失败: request_id={}, conversation_id={}, error={}",
                request_id_clone, conversation_id, err
            ),
        }
    });

    Ok(RequestConversationMessagesAfterAsyncOutput {
        accepted: true,
        request_id,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationInput {
    session: SessionSelector,
    message_id: String,
    #[serde(default)]
    undo_apply_patch: bool,
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
    let started_at = std::time::Instant::now();
    let message_id = input.message_id.trim();
    runtime_log_info(format!(
        "[会话撤回] 开始，任务=rewind_conversation_from_message，message_id={}，undo_apply_patch={}",
        message_id, input.undo_apply_patch
    ));
    if message_id.is_empty() {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=rewind_conversation_from_message，reason=message_id_empty，duration_ms={}",
            elapsed_ms
        ));
        return Err("messageId is required.".to_string());
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut app_config = read_config(&state.config_path)?;

    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    if defaults_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut runtime_data)?;
    let requested_agent_id = input.session.agent_id.trim();
    if requested_agent_id.is_empty() {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=rewind_conversation_from_message，reason=agent_id_empty，duration_ms={}",
            elapsed_ms
        ));
        return Err("agentId is required.".to_string());
    }
    if !runtime_data
        .agents
        .iter()
        .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=rewind_conversation_from_message，reason=agent_not_found，agent_id={}，duration_ms={}",
            requested_agent_id, elapsed_ms
        ));
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }

    let before_len = data.conversations.len();
    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let idx = if let Some(conversation_id) = requested_conversation_id {
        data.conversations
            .iter()
            .position(|item| {
                item.id == conversation_id
                    && item.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(item)
            })
            .ok_or_else(|| {
                format!("Target conversation not found or unavailable, conversationId={conversation_id}")
            })?
    } else {
        latest_active_conversation_index(&data, "", requested_agent_id)
            .ok_or_else(|| "No conversation found for current agent.".to_string())?
    };
    let conversation = data
        .conversations
        .get_mut(idx)
        .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?;

    let remove_from = conversation
        .messages
        .iter()
        .position(|m| m.id == message_id && m.role == "user")
        .ok_or_else(|| "Target user message not found in active conversation.".to_string())?;
    runtime_log_info(format!(
        "[会话撤回] 命中目标，任务=rewind_conversation_from_message，conversation_id={}，remove_from={}，messages_total={}",
        conversation.id,
        remove_from,
        conversation.messages.len()
    ));

    let mut recalled_user_message = conversation.messages.get(remove_from).cloned();
    let removed_count = conversation.messages.len().saturating_sub(remove_from);
    let removed_messages = conversation.messages[remove_from..].to_vec();
    if input.undo_apply_patch {
        runtime_log_info(format!(
            "[会话撤回] 开始工具逆向，任务=rewind_conversation_from_message，removed_messages={}，message_id={}",
            removed_messages.len(),
            message_id
        ));
        let undone_patch_count = match try_undo_apply_patch_from_removed_messages(state.inner(), &removed_messages) {
            Ok(value) => value,
            Err(err) => {
                let elapsed_ms = started_at.elapsed().as_millis();
                runtime_log_error(format!(
                    "[会话撤回] 失败，任务=rewind_conversation_from_message，stage=undo_apply_patch，message_id={}，duration_ms={}，error={}",
                    message_id, elapsed_ms, err
                ));
                return Err(err);
            }
        };
        runtime_log_info(format!(
            "[会话撤回] 工具逆向处理，任务=rewind_conversation_from_message，patches={}，message_id={}",
            undone_patch_count, message_id
        ));
        if undone_patch_count > 0 {
            eprintln!(
                "[会话撤回] 已执行 apply_patch 反向撤回: patches={}, message_id={}",
                undone_patch_count,
                message_id
            );
        }
    }
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
    conversation.last_context_usage_ratio = 0.0;
    conversation.last_effective_prompt_tokens = 0;

    if data.conversations.len() != before_len || removed_count > 0 {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);

    if let Some(message) = recalled_user_message.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }
    let elapsed_ms = started_at.elapsed().as_millis();
    runtime_log_info(format!(
        "[会话撤回] 完成，任务=rewind_conversation_from_message，removed_count={}，remaining_count={}，duration_ms={}",
        removed_count, remove_from, elapsed_ms
    ));

    Ok(RewindConversationResult {
        removed_count,
        remaining_count: remove_from,
        recalled_user_message,
    })
}

