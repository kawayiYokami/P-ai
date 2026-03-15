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
    eprintln!("[系统] 聊天窗口激活状态变更，active={active}");
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
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut result = state_read_config_cached(&state)?;
    normalize_app_config(&mut result);
    ensure_default_shell_workspace_in_config(&mut result, &state);
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
    ensure_default_shell_workspace_in_config(&mut config, &state);
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);

    let guard = state
        .state_lock
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
        .state_lock
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
        .state_lock
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
    let mut config = read_config(&state.config_path)?;
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
        write_config(&state.config_path, &config)?;
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
        .state_lock
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
        .state_lock
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
        .state_lock
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
        .state_lock
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
    state_write_app_data_cached(&state, &data)?;
    drop(guard);

    let payload = ChatSettings {
        assistant_department_agent_id: target_agent_id,
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
        .state_lock
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
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

    let mut config = read_config(&state.config_path)?;
    config.assistant_department_api_config_id = input.assistant_department_api_config_id.clone();
    config.vision_api_config_id = input.vision_api_config_id.clone();
    config.stt_api_config_id = input.stt_api_config_id.clone();
    config.stt_auto_send = input.stt_auto_send;
    normalize_app_config(&mut config);
    let assistant_api_config_id = config.assistant_department_api_config_id.clone();
    if let Some(dept) = assistant_department_mut(&mut config) {
        dept.api_config_id = assistant_api_config_id;
        dept.updated_at = now_iso();
    }
    write_config(&state.config_path, &config)?;
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
        .state_lock
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
    api_config_id: String,
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
        api_config_id: thread.conversation.api_config_id.clone(),
        delegate_id: Some(thread.delegate_id.clone()),
        root_conversation_id: Some(thread.root_conversation_id.clone()),
        archived_at: thread.archived_at.clone(),
    }
}

#[tauri::command]
fn list_unarchived_conversations(state: State<'_, AppState>) -> Result<Vec<UnarchivedConversationSummary>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let defaults_changed = ensure_default_agent(&mut data);
    let normalized_changed = normalize_single_active_main_conversation(&mut data);

    let mut summaries = data
        .conversations
        .iter()
        .filter(|c| c.summary.trim().is_empty() && !conversation_is_delegate(c))
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
    if summaries.len() > 1 {
        summaries.truncate(1);
    }

    if defaults_changed || normalized_changed {
        state_write_app_data_cached(&state, &data)?;
    }
    drop(guard);
    Ok(summaries)
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
    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let requested_messages = data
        .conversations
        .iter()
        .find(|c| c.summary.trim().is_empty() && !conversation_is_delegate(c) && c.id == conversation_id)
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

#[tauri::command]
fn delete_unarchived_conversation(
    state: State<'_, AppState>,
) -> Result<(), String> {
    eprintln!("[会话] 请求删除当前未归档主会话");
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    let before = data.conversations.len();
    let removed_count = data
        .conversations
        .iter()
        .filter(|c| c.summary.trim().is_empty() && !conversation_is_delegate(c))
        .count();
    data.conversations
        .retain(|c| !(c.summary.trim().is_empty() && !conversation_is_delegate(c)));
    if data.conversations.len() == before {
        drop(guard);
        return Err("Unarchived conversation not found.".to_string());
    }
    state_write_app_data_cached(&state, &data)?;
    eprintln!(
        "[会话] 已删除未归档主会话残留: removed_count={}",
        removed_count
    );
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
    let idx = if let Some(conversation_id) = requested_conversation_id {
        if let Some(idx) = data.conversations
            .iter()
            .position(|item| {
                item.id == conversation_id
                    && item.summary.trim().is_empty()
                    && !conversation_is_delegate(item)
            })
        {
            idx
        } else if let Some(existing_idx) =
            latest_active_conversation_index(&data, "", &effective_agent_id)
        {
            eprintln!(
                "[INFO][会话] 指定会话消息读取回退到当前未归档主会话: requested_conversation_id={}, reason=not_found_or_delegate, agent_id={}",
                conversation_id,
                effective_agent_id
            );
            existing_idx
        } else {
            eprintln!(
                "[INFO][会话] 指定会话消息读取未命中，创建新的未归档主会话: requested_conversation_id={}, reason=not_found_and_no_active_main_conversation, agent_id={}",
                conversation_id,
                effective_agent_id
            );
            let api_config = resolve_selected_api_config(&app_config, None)
                .ok_or_else(|| "No API config available".to_string())?;
            ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id)
        }
    } else if let Some(existing_idx) =
        latest_active_conversation_index(&data, "", &effective_agent_id)
    {
        existing_idx
    } else {
        let api_config = resolve_selected_api_config(&app_config, None)
            .ok_or_else(|| "No API config available".to_string())?;
        ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id)
    };
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
        return Err("agentId is required.".to_string());
    }
    if !runtime_data
        .agents
        .iter()
        .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        return Err(format!("Selected agent '{requested_agent_id}' not found."));
    }

    let before_len = data.conversations.len();
    let requested_api_config_id = input
        .session
        .api_config_id
        .as_deref()
        .unwrap_or_default()
        .trim();
    if requested_api_config_id.is_empty() {
        return Err("apiConfigId is required.".to_string());
    }
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
                    && item.status == "active"
                    && item.summary.trim().is_empty()
                    && !conversation_is_delegate(item)
            })
            .ok_or_else(|| {
                format!("Target active conversation not found, conversationId={conversation_id}")
            })?
    } else {
        latest_active_conversation_index(&data, requested_api_config_id, requested_agent_id)
            .ok_or_else(|| "No active conversation found for current agent.".to_string())?
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

    if data.conversations.len() != before_len || removed_count > 0 {
        state_write_app_data_cached(&state, &data)?;
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



