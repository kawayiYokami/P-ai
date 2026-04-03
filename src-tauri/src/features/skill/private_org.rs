use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivatePersonaFile {
    id: String,
    name: String,
    #[serde(alias = "prompt", alias = "systemPrompt")]
    system_prompt: String,
    #[serde(default = "default_agent_tools")]
    tools: Vec<ApiToolConfig>,
    #[serde(default)]
    avatar_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivateDepartmentFile {
    id: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    guide: String,
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PrivateOrganizationMergeResult {
    pub private_agents_loaded: Vec<String>,
    pub private_agents_failed: Vec<WorkspaceLoadError>,
    pub private_departments_loaded: Vec<String>,
    pub private_departments_failed: Vec<WorkspaceLoadError>,
}

fn private_workspace_root_from_config(data_path: &PathBuf, config: &AppConfig) -> PathBuf {
    config
        .shell_workspaces
        .iter()
        .find_map(|workspace| {
            let path = normalize_terminal_path_input_for_current_platform(workspace.path.trim());
            if workspace.name.trim().is_empty() || path.is_empty() {
                return None;
            }
            let candidate = PathBuf::from(&path);
            if candidate.is_absolute() {
                Some(candidate)
            } else {
                Some(app_root_from_data_path(data_path).join("llm-workspace").join(candidate))
            }
        })
        .unwrap_or_else(|| app_root_from_data_path(data_path).join("llm-workspace"))
}

fn private_organization_root_from_config(data_path: &PathBuf, config: &AppConfig) -> PathBuf {
    private_workspace_root_from_config(data_path, config).join("private-organization")
}

fn private_personas_root_from_config(data_path: &PathBuf, config: &AppConfig) -> PathBuf {
    private_organization_root_from_config(data_path, config).join("personas")
}

fn private_departments_root_from_config(data_path: &PathBuf, config: &AppConfig) -> PathBuf {
    private_organization_root_from_config(data_path, config).join("departments")
}

pub(crate) fn ensure_workspace_private_organization_layout(state: &AppState) -> Result<(), String> {
    let workspace_root = ensure_workspace_root_ready(&configured_workspace_root_path(state)?)?;
    ensure_workspace_private_organization_layout_at_root(&workspace_root)
}

pub(crate) fn ensure_workspace_private_organization_layout_at_root(workspace_root: &Path) -> Result<(), String> {
    let root = workspace_root.join("private-organization");
    let personas = root.join("personas");
    let departments = root.join("departments");
    fs::create_dir_all(&personas)
        .map_err(|err| format!("Create private personas dir failed ({}): {err}", personas.display()))?;
    fs::create_dir_all(&departments)
        .map_err(|err| format!("Create private departments dir failed ({}): {err}", departments.display()))?;
    Ok(())
}

fn json_files_sorted(dir: &PathBuf) -> Result<Vec<PathBuf>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = fs::read_dir(dir)
        .map_err(|err| format!("Read private org dir failed ({}): {err}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|v| v.to_str()).unwrap_or_default().eq_ignore_ascii_case("json"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn reserved_private_persona_id(id: &str) -> bool {
    matches!(id, DEFAULT_AGENT_ID | USER_PERSONA_ID | SYSTEM_PERSONA_ID)
}

fn reserved_private_department_id(id: &str) -> bool {
    id == ASSISTANT_DEPARTMENT_ID
}

fn default_private_department_api_config_id(base_config: &AppConfig) -> String {
    assistant_department(base_config)
        .map(|item| item.api_config_id.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = base_config.assistant_department_api_config_id.trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        })
        .or_else(|| {
            base_config
                .api_configs
                .iter()
                .find(|api| api.enable_text && api.request_format.is_chat_text())
                .map(|api| api.id.clone())
        })
        .unwrap_or_default()
}

fn load_private_agents_from_workspace(
    data_path: &PathBuf,
    base_config: &AppConfig,
    base_agents: &[AgentProfile],
) -> Result<(Vec<AgentProfile>, Vec<String>, Vec<WorkspaceLoadError>), String> {
    let mut merged = base_agents.to_vec();
    let mut loaded = Vec::<String>::new();
    let mut errors = Vec::<WorkspaceLoadError>::new();
    let mut seen_private_ids = std::collections::HashSet::<String>::new();
    let root = private_personas_root_from_config(data_path, base_config);
    for path in json_files_sorted(&root)? {
        let raw = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(err) => {
                errors.push(WorkspaceLoadError {
                    item: path.to_string_lossy().to_string(),
                    error: format!("读取私有人格 JSON 失败: {err}"),
                });
                continue;
            }
        };
        let file = match serde_json::from_str::<PrivatePersonaFile>(&raw) {
            Ok(value) => value,
            Err(err) => {
                errors.push(WorkspaceLoadError {
                    item: path.to_string_lossy().to_string(),
                    error: format!("解析私有人格 JSON 失败: {err}"),
                });
                continue;
            }
        };
        let id = file.id.trim().to_string();
        if id.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: "私有人格 id 不能为空".to_string(),
            });
            continue;
        }
        if reserved_private_persona_id(&id) {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有人格不能使用保留 id: {id}"),
            });
            continue;
        }
        if base_agents.iter().any(|item| item.id == id) {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有人格 id 与主配置冲突: {id}"),
            });
            continue;
        }
        if !seen_private_ids.insert(id.clone()) {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有人格 id 重复: {id}"),
            });
            continue;
        }
        let name = file.name.trim().to_string();
        if name.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有人格 name 不能为空，id={id}"),
            });
            continue;
        }
        let system_prompt = file.system_prompt.trim().to_string();
        if system_prompt.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有人格 prompt 不能为空，id={id}"),
            });
            continue;
        }
        let now = now_iso();
        let mut agent = AgentProfile {
            id: id.clone(),
            name,
            system_prompt,
            tools: file.tools,
            created_at: now.clone(),
            updated_at: now,
            avatar_path: file.avatar_path.and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            }),
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            source: default_private_workspace_source(),
            scope: default_assistant_private_scope(),
        };
        normalize_agent_tools(&mut agent);
        merged.push(agent);
        loaded.push(id);
    }
    Ok((merged, loaded, errors))
}

fn load_private_departments_from_workspace(
    data_path: &PathBuf,
    base_config: &AppConfig,
    merged_agents: &[AgentProfile],
) -> Result<(Vec<DepartmentConfig>, Vec<String>, Vec<WorkspaceLoadError>), String> {
    let mut merged = base_config.departments.clone();
    let mut loaded = Vec::<String>::new();
    let mut errors = Vec::<WorkspaceLoadError>::new();
    let mut seen_private_ids = std::collections::HashSet::<String>::new();
    let root = private_departments_root_from_config(data_path, base_config);
    for path in json_files_sorted(&root)? {
        let raw = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(err) => {
                errors.push(WorkspaceLoadError {
                    item: path.to_string_lossy().to_string(),
                    error: format!("读取私有部门 JSON 失败: {err}"),
                });
                continue;
            }
        };
        let file = match serde_json::from_str::<PrivateDepartmentFile>(&raw) {
            Ok(value) => value,
            Err(err) => {
                errors.push(WorkspaceLoadError {
                    item: path.to_string_lossy().to_string(),
                    error: format!("解析私有部门 JSON 失败: {err}"),
                });
                continue;
            }
        };
        let id = file.id.trim().to_string();
        if id.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: "私有部门 id 不能为空".to_string(),
            });
            continue;
        }
        if reserved_private_department_id(&id) {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门不能使用保留 id: {id}"),
            });
            continue;
        }
        if base_config.departments.iter().any(|item| item.id == id) {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门 id 与主配置冲突: {id}"),
            });
            continue;
        }
        if !seen_private_ids.insert(id.clone()) {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门 id 重复: {id}"),
            });
            continue;
        }
        let name = file.name.trim().to_string();
        if name.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门 name 不能为空，id={id}"),
            });
            continue;
        }
        let api_config_id = file
            .api_config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| default_private_department_api_config_id(base_config));
        if api_config_id.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门没有可用的默认文本模型，id={id}"),
            });
            continue;
        }
        if !base_config
            .api_configs
            .iter()
            .any(|api| api.id == api_config_id && api.enable_text && api.request_format.is_chat_text())
        {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门引用了不可用的文本模型，id={id}, apiConfigId={api_config_id}"),
            });
            continue;
        }
        let agent_ids = file
            .agent_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if agent_ids.is_empty() {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门必须至少绑定一个人格，id={id}"),
            });
            continue;
        }
        let missing_agent = agent_ids.iter().find(|agent_id| {
            !merged_agents
                .iter()
                .any(|agent| agent.id == **agent_id && !agent.is_built_in_user && !agent.is_built_in_system)
        });
        if let Some(missing_agent) = missing_agent {
            errors.push(WorkspaceLoadError {
                item: path.to_string_lossy().to_string(),
                error: format!("私有部门引用了不存在的人格，departmentId={id}, agentId={missing_agent}"),
            });
            continue;
        }
        let now = now_iso();
        merged.push(DepartmentConfig {
            id: id.clone(),
            name,
            summary: file.summary.trim().to_string(),
            guide: file.guide.trim().to_string(),
            api_config_ids: vec![api_config_id.clone()],
            api_config_id,
            agent_ids,
            created_at: now.clone(),
            updated_at: now,
            order_index: (merged.len() as i64) + 1,
            is_built_in_assistant: false,
            source: default_private_workspace_source(),
            scope: default_assistant_private_scope(),
        });
        loaded.push(id);
    }
    Ok((merged, loaded, errors))
}

pub(crate) fn merge_private_organization_into_runtime(
    data_path: &PathBuf,
    config: &mut AppConfig,
    agents: &mut Vec<AgentProfile>,
) -> Result<PrivateOrganizationMergeResult, String> {
    let (merged_agents, private_agents_loaded, private_agents_failed) =
        load_private_agents_from_workspace(data_path, config, agents)?;
    let (merged_departments, private_departments_loaded, private_departments_failed) =
        load_private_departments_from_workspace(data_path, config, &merged_agents)?;
    *agents = merged_agents;
    config.departments = merged_departments;
    Ok(PrivateOrganizationMergeResult {
        private_agents_loaded,
        private_agents_failed,
        private_departments_loaded,
        private_departments_failed,
    })
}

pub(crate) fn merge_private_organization_into_runtime_data(
    data_path: &PathBuf,
    config: &mut AppConfig,
    data: &mut AppData,
) -> Result<PrivateOrganizationMergeResult, String> {
    merge_private_organization_into_runtime(data_path, config, &mut data.agents)
}

pub(crate) fn runtime_private_organization_ids(
    data_path: &PathBuf,
    config: &AppConfig,
    agents: &[AgentProfile],
) -> Result<(std::collections::HashSet<String>, std::collections::HashSet<String>), String> {
    let mut config_clone = config.clone();
    let mut agents_clone = agents.to_vec();
    let result =
        merge_private_organization_into_runtime(data_path, &mut config_clone, &mut agents_clone)?;
    Ok((
        result.private_agents_loaded.into_iter().collect(),
        result.private_departments_loaded.into_iter().collect(),
    ))
}
