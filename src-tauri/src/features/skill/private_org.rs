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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    resident_skill_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    optional_skill_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    api_config_ids: Vec<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    include_system_rules: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    child_agent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default_agent_permission_control")]
    permission_control: AgentPermissionControl,
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PrivateOrganizationMergeResult {
    pub private_agents_loaded: Vec<String>,
    pub private_agents_failed: Vec<WorkspaceLoadError>,
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

pub(crate) fn ensure_workspace_private_organization_layout(state: &AppState) -> Result<(), String> {
    let workspace_root = ensure_workspace_root_ready(&configured_workspace_root_path(state)?)?;
    ensure_workspace_private_organization_layout_at_root(&workspace_root)
}

pub(crate) fn ensure_workspace_private_organization_layout_at_root(workspace_root: &Path) -> Result<(), String> {
    let root = workspace_root.join("private-organization");
    let personas = root.join("personas");
    fs::create_dir_all(&personas)
        .map_err(|err| format!("Create private personas dir failed ({}): {err}", personas.display()))?;
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

fn sanitize_private_org_filename(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let normalized = out.trim_matches('_').trim();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    }
}

fn collect_existing_private_persona_paths(
    data_path: &PathBuf,
    base_config: &AppConfig,
) -> Result<std::collections::HashMap<String, PathBuf>, String> {
    let root = private_personas_root_from_config(data_path, base_config);
    let mut by_id = std::collections::HashMap::<String, PathBuf>::new();
    for path in json_files_sorted(&root)? {
        let raw = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let file = match serde_json::from_str::<PrivatePersonaFile>(&raw) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let id = file.id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        by_id.insert(id, path);
    }
    Ok(by_id)
}

pub(crate) fn is_private_workspace_source(source: &str) -> bool {
    source.trim() == default_private_workspace_source()
}

/// 私有 JSON 中的字符串列表：去空白、丢弃空项、保持首次出现顺序去重。
fn normalize_private_string_list(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::<String>::new();
    for value in values {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.clone()) {
            out.push(trimmed);
        }
    }
    out
}

/// 与 `AgentPermissionControl::default()` 等价时视为未配置，序列化时省略。
fn is_default_agent_permission_control(value: &AgentPermissionControl) -> bool {
    *value == AgentPermissionControl::default()
}

fn reserved_private_persona_id(id: &str) -> bool {
    matches!(id, DEFAULT_AGENT_ID | USER_PERSONA_ID | SYSTEM_PERSONA_ID)
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
                errors.push(WorkspaceLoadError::with_hint(
                    path.to_string_lossy().to_string(),
                    format!("读取私有人格 JSON 失败: {err}"),
                    "确认文件存在且当前进程有读取权限；修复后重新调用 reload。",
                ));
                continue;
            }
        };
        let file = match serde_json::from_str::<PrivatePersonaFile>(&raw) {
            Ok(value) => value,
            Err(err) => {
                errors.push(WorkspaceLoadError::with_hint(
                    path.to_string_lossy().to_string(),
                    format!("解析私有人格 JSON 失败: {err}"),
                    "修正 JSON 语法，并确保包含 id、name、prompt/systemPrompt 字段。",
                ));
                continue;
            }
        };
        let id = file.id.trim().to_string();
        if id.is_empty() {
            errors.push(WorkspaceLoadError::with_hint(
                path.to_string_lossy().to_string(),
                "私有人格 id 不能为空",
                "为该私有人格填写非空 id，建议使用小写字母、数字和短横线。",
            ));
            continue;
        }
        if reserved_private_persona_id(&id) {
            errors.push(WorkspaceLoadError::with_hint(
                path.to_string_lossy().to_string(),
                format!("私有人格不能使用保留 id: {id}"),
                "修改该私有人格 id，不能使用内置用户、系统或默认助理人格 id。",
            ));
            continue;
        }
        if base_agents.iter().any(|item| item.id == id) {
            errors.push(WorkspaceLoadError::with_hint(
                path.to_string_lossy().to_string(),
                format!("私有人格 id 与主配置冲突: {id}"),
                "修改该私有人格 id，或删除/禁用对应 JSON；私有组织不能复用主配置人格 id。",
            ));
            continue;
        }
        if !seen_private_ids.insert(id.clone()) {
            errors.push(WorkspaceLoadError::with_hint(
                path.to_string_lossy().to_string(),
                format!("私有人格 id 重复: {id}"),
                "确保 private-organization/personas 下每个 JSON 的 id 唯一。",
            ));
            continue;
        }
        let name = file.name.trim().to_string();
        if name.is_empty() {
            errors.push(WorkspaceLoadError::with_hint(
                path.to_string_lossy().to_string(),
                format!("私有人格 name 不能为空，id={id}"),
                "填写用于展示的人格 name。",
            ));
            continue;
        }
        let system_prompt = file.system_prompt.trim().to_string();
        if system_prompt.is_empty() {
            errors.push(WorkspaceLoadError::with_hint(
                path.to_string_lossy().to_string(),
                format!("私有人格 prompt 不能为空，id={id}"),
                "填写 prompt 或 systemPrompt，描述该人格的职责与行为约束。",
            ));
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
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: default_private_workspace_source(),
            scope: default_assistant_private_scope(),
            summary: file.summary.trim().to_string(),
            resident_skill_names: normalize_private_string_list(file.resident_skill_names),
            optional_skill_names: normalize_private_string_list(file.optional_skill_names),
            include_system_rules: file.include_system_rules,
            api_config_ids: normalize_private_string_list(file.api_config_ids),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: file.permission_control,
            child_agent_ids: normalize_private_string_list(file.child_agent_ids),
        };
        normalize_agent_tools(&mut agent);
        merged.push(agent);
        loaded.push(id);
    }
    Ok((merged, loaded, errors))
}

pub(crate) fn merge_private_organization_into_runtime(
    data_path: &PathBuf,
    config: &AppConfig,
    agents: &mut Vec<AgentProfile>,
) -> Result<PrivateOrganizationMergeResult, String> {
    let (merged_agents, private_agents_loaded, private_agents_failed) =
        load_private_agents_from_workspace(data_path, config, agents)?;
    *agents = merged_agents;
    Ok(PrivateOrganizationMergeResult {
        private_agents_loaded,
        private_agents_failed,
    })
}

pub(crate) fn merge_private_organization_into_runtime_data(
    data_path: &PathBuf,
    config: &AppConfig,
    data: &mut AppData,
) -> Result<PrivateOrganizationMergeResult, String> {
    merge_private_organization_into_runtime(data_path, config, &mut data.agents)
}

pub(crate) fn sync_private_agents_to_workspace(
    data_path: &PathBuf,
    base_config: &AppConfig,
    agents: &[AgentProfile],
) -> Result<(), String> {
    let root = private_personas_root_from_config(data_path, base_config);
    fs::create_dir_all(&root)
        .map_err(|err| format!("Create private personas dir failed ({}): {err}", root.display()))?;
    let mut existing_paths = collect_existing_private_persona_paths(data_path, base_config)?;
    for agent in agents.iter().filter(|agent| is_private_workspace_source(&agent.source)) {
        let file = PrivatePersonaFile {
            id: agent.id.clone(),
            name: agent.name.clone(),
            system_prompt: agent.system_prompt.clone(),
            tools: agent.tools.clone(),
            avatar_path: agent.avatar_path.clone(),
            summary: agent.summary.clone(),
            resident_skill_names: agent.resident_skill_names.clone(),
            optional_skill_names: agent.optional_skill_names.clone(),
            include_system_rules: agent.include_system_rules,
            api_config_ids: agent.api_config_ids.clone(),
            child_agent_ids: agent.child_agent_ids.clone(),
            permission_control: agent.permission_control.clone(),
        };
        let path = existing_paths.remove(&agent.id).unwrap_or_else(|| {
            root.join(format!(
                "{}.json",
                sanitize_private_org_filename(&agent.id, "persona")
            ))
        });
        let text = serde_json::to_string_pretty(&file)
            .map_err(|err| format!("序列化私有人格 JSON 失败，id={}：{err}", agent.id))?;
        fs::write(&path, text)
            .map_err(|err| format!("写入私有人格 JSON 失败 ({}): {err}", path.display()))?;
    }
    for stale_path in existing_paths.into_values() {
        if stale_path.exists() {
            fs::remove_file(&stale_path)
                .map_err(|err| format!("删除已移除的私有人格 JSON 失败 ({}): {err}", stale_path.display()))?;
        }
    }
    Ok(())
}

pub(crate) fn runtime_private_organization_ids(
    data_path: &PathBuf,
    config: &AppConfig,
    agents: &[AgentProfile],
) -> Result<std::collections::HashSet<String>, String> {
    let mut agents_clone = agents.to_vec();
    let result = merge_private_organization_into_runtime(data_path, config, &mut agents_clone)?;
    Ok(result.private_agents_loaded.into_iter().collect())
}

#[cfg(test)]
mod private_persona_file_tests {
    use super::*;

    #[test]
    fn private_persona_file_should_parse_organization_fields() {
        let raw = r#"{
  "id": "market-watcher",
  "name": "市场观察员",
  "systemPrompt": "职责",
  "summary": "简介",
  "apiConfigIds": ["api-a"],
  "childAgentIds": ["child-a"],
  "residentSkillNames": ["code-review"],
  "optionalSkillNames": ["mcp-setup"],
  "permissionControl": {
    "enabled": true,
    "mode": "blacklist",
    "builtinToolNames": ["task"],
    "skillNames": [],
    "mcpToolNames": []
  }
}"#;
        let file: PrivatePersonaFile = serde_json::from_str(raw).expect("parse private persona");
        assert_eq!(file.summary, "简介");
        assert_eq!(file.api_config_ids, vec!["api-a".to_string()]);
        assert_eq!(file.child_agent_ids, vec!["child-a".to_string()]);
        assert_eq!(file.resident_skill_names, vec!["code-review".to_string()]);
        assert_eq!(file.optional_skill_names, vec!["mcp-setup".to_string()]);
        assert!(file.permission_control.enabled);
        assert_eq!(file.permission_control.builtin_tool_names, vec!["task".to_string()]);
    }

    #[test]
    fn private_persona_file_should_omit_empty_optional_fields_on_serialize() {
        let file = PrivatePersonaFile {
            id: "x".to_string(),
            name: "x".to_string(),
            system_prompt: "p".to_string(),
            tools: default_agent_tools(),
            avatar_path: None,
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            child_agent_ids: Vec::new(),
            permission_control: AgentPermissionControl::default(),
        };
        let text = serde_json::to_string(&file).expect("serialize private persona");
        assert!(!text.contains("summary"));
        assert!(!text.contains("childAgentIds"));
        assert!(!text.contains("permissionControl"));
        assert!(!text.contains("includeSystemRules"));
    }

    #[test]
    fn normalize_private_string_list_should_trim_and_dedupe() {
        let out = normalize_private_string_list(vec![
            " a ".to_string(),
            "".to_string(),
            "a".to_string(),
            "b".to_string(),
        ]);
        assert_eq!(out, vec!["a".to_string(), "b".to_string()]);
    }
}
