fn frontend_tool_definition(function: ProviderToolDefinition) -> FrontendToolDefinition {
    FrontendToolDefinition {
        kind: "function".to_string(),
        function: FrontendToolFunctionDefinition {
            name: function.name,
            description: function.description,
            parameters: function.parameters,
        },
    }
}

fn frontend_tool_definition_from_mcp(def: rmcp::model::Tool) -> FrontendToolDefinition {
    FrontendToolDefinition {
        kind: "function".to_string(),
        function: FrontendToolFunctionDefinition {
            name: def.name.to_string(),
            description: def.description.unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object(def.input_schema.as_ref().clone()),
        },
    }
}

fn mcp_host_test_binary_candidate(current_exe: &std::path::Path) -> Option<PathBuf> {
    let deps_dir = current_exe.parent()?;
    if deps_dir.file_name()?.to_str()? != "deps" {
        return None;
    }
    let mut binary_name = "p-ai".to_string();
    if let Some(ext) = current_exe.extension().and_then(|value| value.to_str()) {
        if !ext.trim().is_empty() {
            binary_name.push('.');
            binary_name.push_str(ext);
        }
    }
    let candidate = deps_dir.parent()?.join(binary_name);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn mcp_host_executable_path() -> Result<PathBuf, String> {
    for env_key in ["P_AI_MCP_HOST_EXE", "CARGO_BIN_EXE_p-ai", "CARGO_BIN_EXE_p_ai"] {
        if let Some(value) = std::env::var_os(env_key) {
            let candidate = PathBuf::from(value);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    let current_exe = std::env::current_exe()
        .map_err(|err| format!("Resolve current executable for MCP frontend failed: {err}"))?;
    if let Some(candidate) = mcp_host_test_binary_candidate(&current_exe) {
        return Ok(candidate);
    }
    Ok(current_exe)
}

fn new_mcp_host_command() -> Result<tokio::process::Command, String> {
    let exe = mcp_host_executable_path()?;
    Ok(tokio::process::Command::new(exe))
}

async fn list_mcp_tool_definitions(
    cmd: tokio::process::Command,
    scene: &str,
) -> Result<Vec<rmcp::model::Tool>, String> {
    let transport = rmcp::transport::TokioChildProcess::new(cmd)
        .map_err(|err| format!("Start {scene} MCP child process failed: {err}"))?;
    let client = ()
        .serve(transport)
        .await
        .map_err(|err| format!("Connect to {scene} MCP server failed: {err}"))?;
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List {scene} MCP tools failed: {err}"))?;
    if let Err(err) = client.cancel().await {
        runtime_log_debug(format!(
            "[前台工具目录] 取消 MCP client 失败: scene={}, error={}",
            scene, err
        ));
    }
    Ok(defs)
}

async fn frontend_mcp_tool_definition(
    cmd: tokio::process::Command,
    tool_name: &str,
    scene: &str,
) -> Result<FrontendToolDefinition, String> {
    let defs = list_mcp_tool_definitions(cmd, scene).await?;
    defs.into_iter()
        .find(|def| def.name.as_ref() == tool_name)
        .map(frontend_tool_definition_from_mcp)
        .ok_or_else(|| format!("{scene} MCP tool definition not found: {tool_name}"))
}

async fn frontend_operate_tool_definition() -> Result<FrontendToolDefinition, String> {
    let mut cmd = new_mcp_host_command()?;
    cmd.arg(MCP_OPERATE_SERVER_FLAG);
    frontend_mcp_tool_definition(cmd, MCP_OPERATE_TOOL_NAME, "frontend operate").await
}

async fn frontend_read_file_tool_definition(
    preview_session_id: &str,
    preview_api_id: &str,
) -> Result<FrontendToolDefinition, String> {
    let mut cmd = new_mcp_host_command()?;
    cmd.arg(MCP_READ_FILE_SERVER_FLAG);
    cmd.arg(MCP_READ_FILE_SESSION_FLAG).arg(preview_session_id);
    cmd.arg(MCP_READ_FILE_API_FLAG).arg(preview_api_id);
    frontend_mcp_tool_definition(cmd, MCP_READ_FILE_TOOL_NAME, "frontend read_file").await
}

async fn builtin_tool_definitions_for_frontend(
    state: &AppState,
) -> Vec<FrontendToolDefinition> {
    let preview_session_id = "__frontend_tool_preview__".to_string();
    let preview_api_id = "__frontend_tool_preview__".to_string();
    let preview_agent_id = DEFAULT_AGENT_ID.to_string();
    let operate_definition = match frontend_operate_tool_definition().await {
        Ok(definition) => Some(definition),
        Err(err) => {
            runtime_log_info(format!(
                "[前台工具目录] 读取 operate MCP 定义失败: error={}",
                err
            ));
            None
        }
    };
    let read_file_definition = match frontend_read_file_tool_definition(&preview_session_id, &preview_api_id)
        .await
    {
        Ok(definition) => Some(definition),
        Err(err) => {
            runtime_log_info(format!(
                "[前台工具目录] 读取 read_file MCP 定义失败: preview_session_id={}, preview_api_id={}, error={}",
                preview_session_id, preview_api_id, err
            ));
            None
        }
    };
    let mut out = vec![
        frontend_tool_definition(
            BuiltinFetchTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinBingSearchTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinRememberTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinRecallTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinCommandTool {
                app_state: state.clone(),
                api_config_id: preview_api_id.clone(),
                agent_id: preview_agent_id,
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinPlanTool.provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinTerminalExecTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinApplyPatchTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinTodoTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinTaskTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinDelegateTool {
                app_state: state.clone(),
                session_id: preview_session_id,
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinRemoteImSendTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
    ];
    if let Some(def) = operate_definition {
        out.insert(4, def);
    }
    if let Some(def) = read_file_definition {
        out.insert(7, def);
    }
    out
}

#[tauri::command]
async fn list_tool_catalog(state: State<'_, AppState>) -> Result<Vec<FrontendToolDefinition>, String> {
    Ok(builtin_tool_definitions_for_frontend(state.inner()).await)
}

#[cfg(test)]
mod tool_catalog_tests {
    use super::*;

    fn frontend_definition_json(definition: &FrontendToolDefinition) -> serde_json::Value {
        serde_json::to_value(definition).expect("serialize frontend tool definition should succeed")
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for tool catalog tests should succeed")
            .block_on(future)
    }

    async fn raw_operate_mcp_definition_for_test() -> Result<FrontendToolDefinition, String> {
        let mut cmd = new_mcp_host_command()?;
        cmd.arg(MCP_OPERATE_SERVER_FLAG);
        frontend_mcp_tool_definition(cmd, MCP_OPERATE_TOOL_NAME, "tool catalog test operate").await
    }

    async fn raw_read_file_mcp_definition_for_test() -> Result<FrontendToolDefinition, String> {
        let mut cmd = new_mcp_host_command()?;
        cmd.arg(MCP_READ_FILE_SERVER_FLAG);
        cmd.arg(MCP_READ_FILE_SESSION_FLAG)
            .arg("__frontend_tool_preview__");
        cmd.arg(MCP_READ_FILE_API_FLAG)
            .arg("__frontend_tool_preview__");
        frontend_mcp_tool_definition(
            cmd,
            MCP_READ_FILE_TOOL_NAME,
            "tool catalog test read_file",
        )
        .await
    }

    async fn catalog_tool_definition_by_name(tool_name: &str) -> Result<FrontendToolDefinition, String> {
        let state = AppState::new()?;
        builtin_tool_definitions_for_frontend(&state)
            .await
            .into_iter()
            .find(|definition| definition.function.name == tool_name)
            .ok_or_else(|| format!("frontend catalog tool definition not found: {tool_name}"))
    }

    #[test]
    fn frontend_catalog_tools_should_match_runtime_definitions() {
        let operate_catalog = block_on(catalog_tool_definition_by_name(MCP_OPERATE_TOOL_NAME))
            .expect("load operate definition from frontend catalog should succeed");
        let operate_runtime = block_on(raw_operate_mcp_definition_for_test())
            .expect("load operate definition from runtime MCP should succeed");
        assert_eq!(
            frontend_definition_json(&operate_catalog),
            frontend_definition_json(&operate_runtime),
            "frontend catalog operate definition drifted from runtime MCP definition"
        );

        let read_file_catalog = block_on(catalog_tool_definition_by_name(MCP_READ_FILE_TOOL_NAME))
            .expect("load read_file definition from frontend catalog should succeed");
        let read_file_runtime = block_on(raw_read_file_mcp_definition_for_test())
            .expect("load read_file definition from runtime MCP should succeed");
        assert_eq!(
            frontend_definition_json(&read_file_catalog),
            frontend_definition_json(&read_file_runtime),
            "frontend catalog read_file definition drifted from runtime MCP definition"
        );

        let todo_catalog = block_on(catalog_tool_definition_by_name(TODO_TOOL_NAME))
            .expect("load todo definition from frontend catalog should succeed");
        let todo_runtime = frontend_tool_definition(
            BuiltinTodoTool {
                app_state: AppState::new().expect("create app state for todo definition"),
                session_id: "__frontend_tool_preview__".to_string(),
            }
            .provider_tool_definition(),
        );
        assert_eq!(
            frontend_definition_json(&todo_catalog),
            frontend_definition_json(&todo_runtime),
            "frontend catalog todo definition drifted from runtime builtin definition"
        );
    }
}
