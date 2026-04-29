fn normalize_mcp_server_input(input: McpServerInput) -> Result<McpServerConfig, String> {
    let id = input.id.trim().to_string();
    if id.is_empty() {
        return Err("MCP server id is required".to_string());
    }
    let input_name = input.name.trim().to_string();
    let definition_json = input.definition_json.trim().to_string();
    if definition_json.is_empty() {
        return Err("MCP definition JSON is required".to_string());
    }
    let parsed_name = parse_mcp_server_definition(&definition_json)
        .map(|(name, _)| name)
        .unwrap_or_else(|_| id.clone());
    let name = if input_name.is_empty() {
        parsed_name
    } else {
        input_name
    };

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

fn overlay_runtime_state_on_server(mut server: McpServerConfig) -> McpServerConfig {
    if let Some(runtime) = mcp_runtime_state_get(&server.id) {
        server.enabled = runtime.deployed;
        server.last_status = runtime.last_status;
        server.last_error = runtime.last_error;
        server.updated_at = runtime.updated_at;
        server.cached_tools = runtime
            .tools
            .iter()
            .map(|t| McpCachedTool {
                tool_name: t.tool_name.clone(),
                description: t.description.clone(),
            })
            .collect();
    }
    server
}

fn load_server_by_id(state: &AppState, server_id: &str) -> Result<McpServerConfig, String> {
    load_workspace_mcp_servers(state)?
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| format!("MCP server '{}' not found", server_id))
}

fn list_tools_from_runtime_or_policy(server: &McpServerConfig) -> Vec<McpToolDescriptor> {
    if let Some(runtime) = mcp_runtime_state_get(&server.id) {
        return runtime
            .tools
            .into_iter()
            .map(|tool| {
                let enabled = mcp_policy_enabled_for_tool(server, &tool.tool_name)
                    && mcp_tool_allowed_by_definition(server, &tool.tool_name);
                McpToolDescriptor { enabled, ..tool }
            })
            .collect();
    }
    server
        .tool_policies
        .iter()
        .map(|policy| McpToolDescriptor {
            tool_name: policy.tool_name.clone(),
            description: String::new(),
            enabled: mcp_tool_allowed_by_definition(server, &policy.tool_name) && policy.enabled,
            parameters: serde_json::Value::Object(serde_json::Map::new()),
        })
        .collect()
}

async fn mcp_redeploy_all_from_policy(state: &AppState) -> Result<Vec<WorkspaceLoadError>, String> {
    let servers = load_workspace_mcp_servers(state)?;

    for server in &servers {
        mcp_disconnect_cached_client(&server.id).await;
        mcp_runtime_state_set(&server.id, false, "stopped", "", Vec::new());
    }

    let mut deploy_errors = Vec::<WorkspaceLoadError>::new();
    for server in servers.into_iter().filter(|s| s.enabled) {
        mcp_runtime_state_set(&server.id, false, "deploying", "", Vec::new());
        let tools = match tokio::time::timeout(
            std::time::Duration::from_secs(TOOL_RUNTIME_CHECK_TIMEOUT_SECS),
            mcp_list_server_tools_runtime(&server),
        )
        .await
        {
            Ok(Ok(tools)) => tools,
            Ok(Err(err)) => {
                mcp_runtime_state_set(&server.id, false, "failed", &err, Vec::new());
                deploy_errors.push(WorkspaceLoadError {
                    item: server.id.clone(),
                    error: err,
                });
                continue;
            }
            Err(_) => {
                let err = format!("MCP 工具检验超时: server_id={}, server_name={}", server.id, server.name);
                runtime_log_warn(format!(
                    "[MCP] reload 期间工具检验超时: server_id={}, server_name={}",
                    server.id, server.name
                ));
                mcp_runtime_state_set(&server.id, false, "failed", &err, Vec::new());
                deploy_errors.push(WorkspaceLoadError {
                    item: server.id.clone(),
                    error: err,
                });
                continue;
            }
        };

        let discovered_names = tools
            .iter()
            .map(|t| t.tool_name.clone())
            .collect::<Vec<_>>();
        let merged_policies =
            merge_workspace_mcp_tool_policies_with_new_tools(state, &server.id, &discovered_names)?;

        let mut server_with_policies = server.clone();
        server_with_policies.tool_policies = merged_policies;
        let final_tools = tools
            .into_iter()
            .map(|tool| {
                let enabled = mcp_policy_enabled_for_tool(&server_with_policies, &tool.tool_name)
                    && mcp_tool_allowed_by_definition(&server_with_policies, &tool.tool_name);
                McpToolDescriptor { enabled, ..tool }
            })
            .collect::<Vec<_>>();

        mcp_runtime_state_set(&server.id, true, "deployed", "", final_tools);
    }

    Ok(deploy_errors)
}

#[tauri::command]
fn mcp_list_servers(state: State<'_, AppState>) -> Result<Vec<McpServerConfig>, String> {
    let mut out = load_workspace_mcp_servers(&state)?;
    for item in &mut out {
        *item = overlay_runtime_state_on_server(item.clone());
    }
    Ok(out)
}

#[tauri::command]
fn mcp_validate_definition(
    input: McpDefinitionValidateInput,
) -> Result<McpDefinitionValidateResult, String> {
    let _schema = mcp_definition_json_schema();
    match normalize_mcp_definition_for_validation(&input.definition_json) {
        Ok((normalized_value, migrated)) => {
            let normalized_text = serde_json::to_string(&normalized_value)
                .map_err(|err| format!("序列化标准化 MCP 定义失败：{err}"))?;
            let (name, parsed) = parse_mcp_server_definition(&normalized_text)?;
            let _ = migrated;
            let message = "MCP definition is valid".to_string();
            Ok(McpDefinitionValidateResult {
                ok: true,
                transport: Some(parsed.transport.as_str().to_string()),
                server_name: Some(name),
                message,
                schema_version: None,
                error_code: None,
                details: Vec::new(),
                migrated_definition_json: None,
            })
        }
        Err(err) => Ok(McpDefinitionValidateResult {
            ok: false,
            transport: None,
            server_name: None,
            message: err.message,
            schema_version: None,
            error_code: Some(err.code),
            details: err.details,
            migrated_definition_json: None,
        }),
    }
}

#[tauri::command]
fn mcp_save_server(
    input: McpServerInput,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    let next = normalize_mcp_server_input(input)?;
    save_workspace_mcp_server(&state, &next)?;
    let mut saved = load_server_by_id(&state, &next.id)?;
    saved = overlay_runtime_state_on_server(saved);

    Ok(saved)
}

#[tauri::command]
async fn mcp_remove_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    let removed = remove_workspace_mcp_server(&state, server_id)?;
    if removed {
        mcp_disconnect_cached_client(server_id).await;
        mcp_runtime_state_remove(server_id);
    }
    Ok(removed)
}

#[tauri::command]
async fn mcp_list_server_tools(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let server = {
        let server = load_server_by_id(&state, server_id)?;
        server
    };

    let started = std::time::Instant::now();
    let tools = mcp_list_server_tools_runtime(&server).await?;

    Ok(McpListServerToolsResult {
        server_id: server.id,
        tools,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
fn mcp_list_server_tools_cached(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let server = {
        let server = load_server_by_id(&state, server_id)?;
        server
    };

    let started = std::time::Instant::now();
    let tools = list_tools_from_runtime_or_policy(&server);

    Ok(McpListServerToolsResult {
        server_id: server.id,
        tools,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
async fn mcp_deploy_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let server = {
        let server = load_server_by_id(&state, server_id)?;
        set_workspace_mcp_policy_enabled(&state, server_id, true)?;
        server
    };

    mcp_runtime_state_set(server_id, false, "deploying", "", Vec::new());
    let started = std::time::Instant::now();
    let tools_res = mcp_list_server_tools_runtime(&server).await;

    let tools = match tools_res {
        Ok(tools) => tools,
        Err(err) => {
            mcp_runtime_state_set(server_id, false, "failed", &err, Vec::new());
            return Err(err);
        }
    };

    let discovered_names = tools
        .iter()
        .map(|t| t.tool_name.clone())
        .collect::<Vec<_>>();
    let merged_policies = {
        let policies = merge_workspace_mcp_tool_policies_with_new_tools(&state, server_id, &discovered_names)?;
        policies
    };

    let mut server_with_policies = server.clone();
    server_with_policies.tool_policies = merged_policies;
    let final_tools = tools
        .into_iter()
        .map(|tool| {
            let enabled = mcp_policy_enabled_for_tool(&server_with_policies, &tool.tool_name)
                && mcp_tool_allowed_by_definition(&server_with_policies, &tool.tool_name);
            McpToolDescriptor { enabled, ..tool }
        })
        .collect::<Vec<_>>();

    mcp_runtime_state_set(server_id, true, "deployed", "", final_tools.clone());
    Ok(McpListServerToolsResult {
        server_id: server.id,
        tools: final_tools,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
async fn mcp_undeploy_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    {
        let _ = load_server_by_id(&state, server_id)?;
        set_workspace_mcp_policy_enabled(&state, server_id, false)?;
    }
    mcp_disconnect_cached_client(server_id).await;
    mcp_runtime_state_set(server_id, false, "stopped", "", Vec::new());

    let mut out = load_server_by_id(&state, server_id)?;
    out = overlay_runtime_state_on_server(out);
    Ok(out)
}

#[tauri::command]
fn mcp_set_tool_enabled(
    input: McpSetToolEnabledInput,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    let server_id = input.server_id.trim();
    let tool_name = input.tool_name.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    if tool_name.is_empty() {
        return Err("toolName is required".to_string());
    }

    let policies = {
        let _ = load_server_by_id(&state, server_id)?;
        let mut policies = load_workspace_mcp_tool_policies(&state, server_id)?;
        if let Some(policy) = policies.iter_mut().find(|p| p.tool_name == tool_name) {
            policy.enabled = input.enabled;
        } else {
            policies.push(McpToolPolicy {
                tool_name: tool_name.to_string(),
                enabled: input.enabled,
            });
        }
        save_workspace_mcp_tool_policies(&state, server_id, &policies)?;
        policies
    };

    mcp_runtime_state_set_tool_enabled(server_id, tool_name, input.enabled);

    let mut server = load_server_by_id(&state, server_id)?;
    server.tool_policies = policies;
    server = overlay_runtime_state_on_server(server);

    Ok(server)
}

#[tauri::command]
fn mcp_open_workspace_dir(state: State<'_, AppState>) -> Result<String, String> {
    open_mcp_workspace_dir(&state)
}

