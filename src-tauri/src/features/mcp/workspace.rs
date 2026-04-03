#[derive(Debug, Clone)]
struct McpWorkspaceLoadError {
    item: String,
    error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolPoliciesFile {
    #[serde(default)]
    server_id: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    tools: Vec<McpToolPolicy>,
}

fn llm_workspace_mcp_root_at(workspace_root: &Path) -> PathBuf {
    workspace_root.join("mcp")
}

fn llm_workspace_mcp_root(state: &AppState) -> Result<PathBuf, String> {
    Ok(llm_workspace_mcp_root_at(&configured_workspace_root_path(state)?))
}

fn llm_workspace_mcp_servers_dir(state: &AppState) -> Result<PathBuf, String> {
    Ok(llm_workspace_mcp_root(state)?.join("servers"))
}

fn llm_workspace_mcp_policies_dir(state: &AppState) -> Result<PathBuf, String> {
    Ok(llm_workspace_mcp_root(state)?.join("policies"))
}

fn ensure_workspace_mcp_layout_at_root(workspace_root: &Path) -> Result<(), String> {
    let mcp_root = llm_workspace_mcp_root_at(workspace_root);
    let mcp_servers = mcp_root.join("servers");
    let mcp_policies = mcp_root.join("policies");
    fs::create_dir_all(&mcp_servers)
        .map_err(|err| format!("Create MCP servers dir failed ({}): {err}", mcp_servers.display()))?;
    fs::create_dir_all(&mcp_policies)
        .map_err(|err| format!("Create MCP policies dir failed ({}): {err}", mcp_policies.display()))?;
    let legacy_readme = mcp_root.join("README.md");
    if legacy_readme.exists() {
        let _ = fs::remove_file(&legacy_readme);
    }
    Ok(())
}

fn ensure_workspace_mcp_layout(state: &AppState) -> Result<(), String> {
    let root = ensure_workspace_root_ready(&configured_workspace_root_path(state)?)?;
    ensure_workspace_mcp_layout_at_root(&root)
}

fn sanitize_mcp_server_id_for_filename(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').trim().to_string();
    if trimmed.is_empty() {
        "mcp_server".to_string()
    } else {
        trimmed
    }
}

fn mcp_server_file_path(state: &AppState, server_id: &str) -> Result<PathBuf, String> {
    let file = format!("{}.json", sanitize_mcp_server_id_for_filename(server_id));
    Ok(llm_workspace_mcp_servers_dir(state)?.join(file))
}

fn mcp_tool_policies_file_path(state: &AppState, server_id: &str) -> Result<PathBuf, String> {
    let file = format!("{}.json", sanitize_mcp_server_id_for_filename(server_id));
    Ok(llm_workspace_mcp_policies_dir(state)?.join(file))
}

fn parse_workspace_mcp_server_from_file(path: &PathBuf) -> Result<McpServerConfig, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("Read MCP file failed ({}): {err}", path.display()))?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|err| format!("Parse JSON failed ({}): {err}", path.display()))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("MCP file must be JSON object ({})", path.display()))?;
    if object.is_empty() {
        return Err(format!("MCP file is empty object ({})", path.display()));
    }

    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("mcp-server")
        .to_string();
    let id = stem;
    let definition_json = value_get_string(&value, "definitionJson")
        .or_else(|| value_get_string(&value, "definition_json"))
        .unwrap_or_else(|| serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string()));
    let parsed_name = parse_mcp_server_definition(&definition_json)
        .map(|(name, _)| name)
        .unwrap_or_else(|_| id.clone());
    let name = value_get_string(&value, "name")
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(parsed_name);

    Ok(McpServerConfig {
        id,
        name,
        enabled: false,
        definition_json,
        tool_policies: Vec::new(),
        cached_tools: Vec::new(),
        last_status: String::new(),
        last_error: String::new(),
        updated_at: String::new(),
    })
}

fn load_workspace_mcp_servers_with_errors(
    state: &AppState,
) -> Result<(Vec<McpServerConfig>, Vec<McpWorkspaceLoadError>), String> {
    ensure_workspace_mcp_layout(state)?;
    let mut servers = Vec::<McpServerConfig>::new();
    let mut errors = Vec::<McpWorkspaceLoadError>::new();
    let servers_dir = llm_workspace_mcp_servers_dir(state)?;
    let mut files = fs::read_dir(&servers_dir)
        .map_err(|err| format!("Read MCP servers dir failed ({}): {err}", servers_dir.display()))?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|v| v.to_str()).map(|v| v.eq_ignore_ascii_case("json")).unwrap_or(false))
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        match parse_workspace_mcp_server_from_file(&file) {
            Ok(server) => servers.push(server),
            Err(err) => errors.push(McpWorkspaceLoadError {
                item: file.to_string_lossy().to_string(),
                error: err,
            }),
        }
    }
    Ok((servers, errors))
}

fn normalize_mcp_tool_policies(raw: Vec<McpToolPolicy>) -> Vec<McpToolPolicy> {
    let mut out = Vec::<McpToolPolicy>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for item in raw {
        let name = item.tool_name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let key = name.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(McpToolPolicy {
            tool_name: name,
            enabled: item.enabled,
        });
    }
    out
}

fn load_workspace_mcp_server_policy(
    state: &AppState,
    server_id: &str,
) -> Result<McpToolPoliciesFile, String> {
    ensure_workspace_mcp_layout(state)?;
    let path = mcp_tool_policies_file_path(state, server_id)?;
    if !path.exists() {
        return Ok(McpToolPoliciesFile {
            server_id: server_id.to_string(),
            enabled: false,
            tools: Vec::new(),
        });
    }
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Read MCP policy file failed ({}): {err}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(McpToolPoliciesFile {
            server_id: server_id.to_string(),
            enabled: false,
            tools: Vec::new(),
        });
    }
    let mut parsed: McpToolPoliciesFile = serde_json::from_str(&content)
        .map_err(|err| format!("Parse MCP policy JSON failed ({}): {err}", path.display()))?;
    if parsed.server_id.trim().is_empty() {
        parsed.server_id = server_id.to_string();
    }
    parsed.tools = normalize_mcp_tool_policies(parsed.tools);
    Ok(parsed)
}

fn load_workspace_mcp_tool_policies(
    state: &AppState,
    server_id: &str,
) -> Result<Vec<McpToolPolicy>, String> {
    Ok(load_workspace_mcp_server_policy(state, server_id)?.tools)
}

fn save_workspace_mcp_tool_policies(
    state: &AppState,
    server_id: &str,
    tools: &[McpToolPolicy],
) -> Result<(), String> {
    let mut payload = load_workspace_mcp_server_policy(state, server_id)?;
    payload.tools = normalize_mcp_tool_policies(tools.to_vec());
    save_workspace_mcp_server_policy(state, server_id, &payload)
}

fn save_workspace_mcp_server_policy(
    state: &AppState,
    server_id: &str,
    payload: &McpToolPoliciesFile,
) -> Result<(), String> {
    ensure_workspace_mcp_layout(state)?;
    let path = mcp_tool_policies_file_path(state, server_id)?;
    let out = McpToolPoliciesFile {
        server_id: if payload.server_id.trim().is_empty() {
            server_id.to_string()
        } else {
            payload.server_id.trim().to_string()
        },
        enabled: payload.enabled,
        tools: normalize_mcp_tool_policies(payload.tools.clone()),
    };
    let text = serde_json::to_string_pretty(&out)
        .map_err(|err| format!("序列化 MCP 策略 JSON 失败：{err}"))?;
    fs::write(&path, text)
        .map_err(|err| format!("Write MCP policy file failed ({}): {err}", path.display()))
}

fn merge_workspace_mcp_tool_policies_with_new_tools(
    state: &AppState,
    server_id: &str,
    discovered_tool_names: &[String],
) -> Result<Vec<McpToolPolicy>, String> {
    let mut policy_file = load_workspace_mcp_server_policy(state, server_id)?;
    let mut policies = policy_file.tools.clone();
    let mut seen = policies
        .iter()
        .map(|p| p.tool_name.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();
    let mut changed = false;
    for name in discovered_tool_names {
        let tool_name = name.trim().to_string();
        if tool_name.is_empty() {
            continue;
        }
        let key = tool_name.to_ascii_lowercase();
        if seen.insert(key) {
            policies.push(McpToolPolicy {
                tool_name,
                enabled: true,
            });
            changed = true;
        }
    }
    if changed {
        policy_file.tools = policies.clone();
        save_workspace_mcp_server_policy(state, server_id, &policy_file)?;
    }
    Ok(policies)
}

fn set_workspace_mcp_policy_enabled(
    state: &AppState,
    server_id: &str,
    enabled: bool,
) -> Result<(), String> {
    let mut policy = load_workspace_mcp_server_policy(state, server_id)?;
    policy.enabled = enabled;
    save_workspace_mcp_server_policy(state, server_id, &policy)
}

fn load_workspace_mcp_servers(state: &AppState) -> Result<Vec<McpServerConfig>, String> {
    let (mut servers, errors) = load_workspace_mcp_servers_with_errors(state)?;
    for err in errors {
        eprintln!("[MCP] skip invalid file: {} | {}", err.item, err.error);
    }
    for server in &mut servers {
        let policy = load_workspace_mcp_server_policy(state, &server.id)?;
        server.enabled = policy.enabled;
        server.tool_policies = policy.tools;
    }
    Ok(servers)
}

fn save_workspace_mcp_server(state: &AppState, server: &McpServerConfig) -> Result<(), String> {
    ensure_workspace_mcp_layout(state)?;
    let value: Value = serde_json::from_str(&server.definition_json)
        .map_err(|err| format!("Parse MCP definition JSON failed before write: {err}"))?;
    let payload = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("序列化 MCP 定义 JSON 失败：{err}"))?;
    let target = mcp_server_file_path(state, &server.id)?;
    fs::write(&target, payload)
        .map_err(|err| format!("Write MCP file failed ({}): {err}", target.display()))
}

fn remove_workspace_mcp_server(state: &AppState, server_id: &str) -> Result<bool, String> {
    ensure_workspace_mcp_layout(state)?;
    let target = mcp_server_file_path(state, server_id)?;
    let mut removed = false;
    if target.exists() {
        fs::remove_file(&target)
            .map_err(|err| format!("Delete MCP file failed ({}): {err}", target.display()))?;
        removed = true;
    } else {
        let servers_dir = llm_workspace_mcp_servers_dir(state)?;
        for path in fs::read_dir(&servers_dir)
            .map_err(|err| format!("Read MCP servers dir failed ({}): {err}", servers_dir.display()))?
            .filter_map(|entry| entry.ok().map(|v| v.path()))
            .filter(|path| path.is_file())
            .filter(|path| path.extension().and_then(|v| v.to_str()).map(|v| v.eq_ignore_ascii_case("json")).unwrap_or(false))
        {
            if let Ok(server) = parse_workspace_mcp_server_from_file(&path) {
                if server.id == server_id {
                    fs::remove_file(&path)
                        .map_err(|err| format!("Delete MCP file failed ({}): {err}", path.display()))?;
                    removed = true;
                    break;
                }
            }
        }
    }

    let policy_path = mcp_tool_policies_file_path(state, server_id)?;
    if policy_path.exists() {
        let _ = fs::remove_file(&policy_path);
    }
    Ok(removed)
}

fn open_path_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .status()
            .map_err(|err| format!("Open in explorer failed: {err}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .status()
            .map_err(|err| format!("Open in Finder failed: {err}"))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .status()
            .map_err(|err| format!("Open in file manager failed: {err}"))?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Open in file manager is not supported on this platform".to_string())
}

fn open_mcp_workspace_dir(state: &AppState) -> Result<String, String> {
    ensure_workspace_mcp_layout(state)?;
    let path = llm_workspace_mcp_root(state)?;
    open_path_in_file_manager(&path)?;
    Ok(path.to_string_lossy().to_string())
}
