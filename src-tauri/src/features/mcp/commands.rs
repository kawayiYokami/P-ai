fn now_iso_or_empty() -> String {
    now_iso()
}

fn normalize_mcp_server_input(input: McpServerInput) -> Result<McpServerConfig, String> {
    let id = input.id.trim().to_string();
    if id.is_empty() {
        return Err("MCP server id is required".to_string());
    }
    let mut name = input.name.trim().to_string();
    if name.is_empty() {
        name = id.clone();
    }
    let definition_json = input.definition_json.trim().to_string();
    if definition_json.is_empty() {
        return Err("MCP definition JSON is required".to_string());
    }
    let (normalized_value, migrated) =
        normalize_mcp_definition_for_validation(&definition_json).map_err(|err| {
            let detail = if err.details.is_empty() {
                String::new()
            } else {
                format!(" | details: {}", err.details.join(" ; "))
            };
            format!("{} ({}){}", err.message, err.code, detail)
        })?;
    let normalized_definition_json = migrated.unwrap_or_else(|| {
        serde_json::to_string_pretty(&normalized_value)
            .unwrap_or_else(|_| definition_json.clone())
    });
    let (parsed_name, _parsed) = parse_mcp_server_definition(&normalized_definition_json)?;
    if name.trim().is_empty() {
        name = parsed_name;
    }

    Ok(McpServerConfig {
        id,
        name,
        enabled: input.enabled,
        definition_json: normalized_definition_json,
        tool_policies: Vec::new(),
        last_status: "saved".to_string(),
        last_error: String::new(),
        updated_at: now_iso_or_empty(),
    })
}

fn merge_mcp_server_preserving_policies(
    current: Option<&McpServerConfig>,
    mut next: McpServerConfig,
) -> McpServerConfig {
    if let Some(old) = current {
        if next.tool_policies.is_empty() {
            next.tool_policies = old.tool_policies.clone();
        }
        if next.last_status.trim().is_empty() {
            next.last_status = old.last_status.clone();
        }
        if next.last_error.trim().is_empty() {
            next.last_error = old.last_error.clone();
        }
    }
    next
}

#[tauri::command]
fn mcp_list_servers(state: State<'_, AppState>) -> Result<Vec<McpServerConfig>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);
    drop(guard);
    Ok(config.mcp_servers)
}

#[tauri::command]
fn mcp_validate_definition(
    input: McpDefinitionValidateInput,
) -> Result<McpDefinitionValidateResult, String> {
    let _schema = mcp_definition_json_schema();
    match normalize_mcp_definition_for_validation(&input.definition_json) {
        Ok((normalized_value, migrated)) => {
            let normalized_text = serde_json::to_string(&normalized_value)
                .map_err(|err| format!("Serialize normalized MCP definition failed: {err}"))?;
            let (name, parsed) = parse_mcp_server_definition(&normalized_text)?;
            let message = if migrated.is_some() {
                "MCP definition is valid and was auto-migrated to version 1.0".to_string()
            } else {
                "MCP definition is valid".to_string()
            };
            Ok(McpDefinitionValidateResult {
                ok: true,
                transport: Some(parsed.transport.as_str().to_string()),
                server_name: Some(name),
                message,
                schema_version: Some(MCP_SPEC_VERSION_SUPPORTED.to_string()),
                error_code: None,
                details: Vec::new(),
                migrated_definition_json: migrated,
            })
        }
        Err(err) => Ok(McpDefinitionValidateResult {
            ok: false,
            transport: None,
            server_name: None,
            message: err.message,
            schema_version: Some(MCP_SPEC_VERSION_SUPPORTED.to_string()),
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

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);

    let existing = config.mcp_servers.iter().find(|s| s.id == next.id).cloned();
    let merged = merge_mcp_server_preserving_policies(existing.as_ref(), next);

    if let Some(pos) = config.mcp_servers.iter().position(|s| s.id == merged.id) {
        config.mcp_servers[pos] = merged.clone();
    } else {
        config.mcp_servers.push(merged.clone());
    }
    write_config(&state.config_path, &config)?;
    drop(guard);

    Ok(merged)
}

#[tauri::command]
fn mcp_remove_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);

    let before = config.mcp_servers.len();
    config.mcp_servers.retain(|s| s.id != server_id);
    let removed = config.mcp_servers.len() != before;
    if removed {
        write_config(&state.config_path, &config)?;
    }
    drop(guard);
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
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut config = read_config(&state.config_path)?;
        normalize_app_config(&mut config);
        let server = config
            .mcp_servers
            .iter()
            .find(|s| s.id == server_id)
            .cloned()
            .ok_or_else(|| format!("MCP server '{}' not found", server_id))?;
        drop(guard);
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
async fn mcp_deploy_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let mut server = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut config = read_config(&state.config_path)?;
        normalize_app_config(&mut config);
        let mut server = config
            .mcp_servers
            .iter()
            .find(|s| s.id == server_id)
            .cloned()
            .ok_or_else(|| format!("MCP server '{}' not found", server_id))?;
        server.enabled = true;
        server.last_status = "deploying".to_string();
        server.last_error.clear();
        server.updated_at = now_iso_or_empty();
        if let Some(pos) = config.mcp_servers.iter().position(|s| s.id == server_id) {
            config.mcp_servers[pos] = server.clone();
        }
        write_config(&state.config_path, &config)?;
        drop(guard);
        server
    };

    let started = std::time::Instant::now();
    let tools_res = mcp_list_server_tools_runtime(&server).await;

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);

    if let Some(pos) = config.mcp_servers.iter().position(|s| s.id == server_id) {
        match &tools_res {
            Ok(_) => {
                config.mcp_servers[pos].last_status = "deployed".to_string();
                config.mcp_servers[pos].last_error.clear();
                config.mcp_servers[pos].enabled = true;
            }
            Err(err) => {
                config.mcp_servers[pos].last_status = "failed".to_string();
                config.mcp_servers[pos].last_error = err.clone();
                config.mcp_servers[pos].enabled = false;
            }
        }
        config.mcp_servers[pos].updated_at = now_iso_or_empty();
        server = config.mcp_servers[pos].clone();
    }
    write_config(&state.config_path, &config)?;
    drop(guard);

    let tools = tools_res?;
    Ok(McpListServerToolsResult {
        server_id: server.id,
        tools,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
fn mcp_undeploy_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);

    let pos = config
        .mcp_servers
        .iter()
        .position(|s| s.id == server_id)
        .ok_or_else(|| format!("MCP server '{}' not found", server_id))?;

    config.mcp_servers[pos].enabled = false;
    config.mcp_servers[pos].last_status = "stopped".to_string();
    config.mcp_servers[pos].last_error.clear();
    config.mcp_servers[pos].updated_at = now_iso_or_empty();

    let updated = config.mcp_servers[pos].clone();
    write_config(&state.config_path, &config)?;
    drop(guard);

    Ok(updated)
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

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);

    let pos = config
        .mcp_servers
        .iter()
        .position(|s| s.id == server_id)
        .ok_or_else(|| format!("MCP server '{}' not found", server_id))?;
    let server = &mut config.mcp_servers[pos];

    if let Some(policy) = server
        .tool_policies
        .iter_mut()
        .find(|p| p.tool_name == tool_name)
    {
        policy.enabled = input.enabled;
    } else {
        server.tool_policies.push(McpToolPolicy {
            tool_name: tool_name.to_string(),
            enabled: input.enabled,
            description: String::new(),
        });
    }
    server.updated_at = now_iso_or_empty();

    let out = server.clone();
    write_config(&state.config_path, &config)?;
    drop(guard);

    Ok(out)
}

