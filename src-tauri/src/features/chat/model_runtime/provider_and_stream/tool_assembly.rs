fn tool_manifest_item(
    source: &str,
    name: &str,
    enabled: bool,
    attached: bool,
    reason: Option<String>,
) -> Value {
    serde_json::json!({
        "source": source,
        "name": name,
        "enabled": enabled,
        "attached": attached,
        "reason": reason
    })
}

async fn assemble_runtime_tools(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> Result<RuntimeToolAssembly, String> {
    let current_department = department_for_agent_id(app_config, &agent.id);
    let department_reason = |tool_id: &str| tool_restricted_by_department(current_department, tool_id);
    let has_fetch = tool_enabled(selected_api, agent, current_department, "fetch");
    let has_websearch = tool_enabled(selected_api, agent, current_department, "websearch");
    let has_remember = tool_enabled(selected_api, agent, current_department, "remember");
    let has_recall = tool_enabled(selected_api, agent, current_department, "recall");
    let has_screenshot = tool_enabled(selected_api, agent, current_department, "screenshot");
    let has_wait = tool_enabled(selected_api, agent, current_department, "wait");
    let has_refresh_mcp_skills = tool_enabled(selected_api, agent, current_department, "reload");
    let has_exec = tool_enabled(selected_api, agent, current_department, "exec");
    let has_task = tool_enabled(selected_api, agent, current_department, "task");
    let has_delegate = tool_enabled(selected_api, agent, current_department, "delegate");
    let has_handoff = tool_enabled(selected_api, agent, current_department, "handoff");

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    let mut tool_manifest = Vec::<Value>::new();

    tool_manifest.push(tool_manifest_item(
        "builtin",
        "fetch",
        has_fetch,
        has_fetch,
        if has_fetch {
            None
        } else {
            department_reason("fetch").or_else(|| Some("当前人格未启用该工具".to_string()))
        },
    ));
    if has_fetch {
        tools.push(Box::new(BuiltinFetchTool));
    }

    tool_manifest.push(tool_manifest_item(
        "builtin",
        "websearch",
        has_websearch,
        has_websearch,
        if has_websearch {
            None
        } else {
            department_reason("websearch").or_else(|| Some("当前人格未启用该工具".to_string()))
        },
    ));
    if has_websearch {
        tools.push(Box::new(BuiltinBingSearchTool));
    }

    if has_remember {
        let state = app_state
            .ok_or_else(|| "remember requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinRememberTool {
            app_state: state,
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "remember",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "remember",
            false,
            false,
            department_reason("remember").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_recall {
        let state = app_state
            .ok_or_else(|| "recall requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinRecallTool {
            app_state: state,
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "recall",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "recall",
            false,
            false,
            department_reason("recall").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    let mut mcp_screenshot_client: Option<ScreenshotMcpClient> = None;
    if has_screenshot {
        match try_attach_desktop_screenshot_mcp_tool(&mut tools).await {
            Ok(client) => {
                mcp_screenshot_client = Some(client);
                tool_manifest.push(tool_manifest_item(
                    "builtin_mcp",
                    "screenshot",
                    true,
                    true,
                    None,
                ));
            }
            Err(err) => {
                eprintln!("[MCP] screenshot degraded to disabled: {err}");
                tool_manifest.push(tool_manifest_item(
                    "builtin_mcp",
                    "screenshot",
                    true,
                    false,
                    Some(format!("MCP attach failed: {err}")),
                ));
            }
        }
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin_mcp",
            "screenshot",
            false,
            false,
            department_reason("screenshot").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    match attach_enabled_mcp_tools_for_runtime(&mut tools, app_state).await {
        Ok(names) => {
            if names.is_empty() {
                tool_manifest.push(tool_manifest_item(
                    "mcp_runtime",
                    "(none)",
                    true,
                    false,
                    Some("no enabled MCP tools attached".to_string()),
                ));
            } else {
                for name in names {
                    tool_manifest.push(tool_manifest_item(
                        "mcp_runtime",
                        &name,
                        true,
                        true,
                        None,
                    ));
                }
            }
        }
        Err(err) => {
            tool_manifest.push(tool_manifest_item(
                "mcp_runtime",
                "(attach)",
                true,
                false,
                Some(err.clone()),
            ));
            eprintln!("[MCP] attach runtime tools skipped: {err}");
        }
    }

    if has_wait {
        tools.push(Box::new(BuiltinDesktopWaitTool));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "wait",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "wait",
            false,
            false,
            department_reason("wait").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_refresh_mcp_skills {
        let state = app_state
            .ok_or_else(|| "reload requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinRefreshMcpAndSkillsTool { app_state: state }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "reload",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "reload",
            false,
            false,
            department_reason("reload").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_exec {
        let state = app_state
            .ok_or_else(|| "shell_exec requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTerminalExecTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "exec",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "exec",
            false,
            false,
            department_reason("exec").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_task {
        let state = app_state
            .ok_or_else(|| "task requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTaskTool { app_state: state }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "task",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "task",
            false,
            false,
            department_reason("task").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_delegate {
        let state = app_state
            .ok_or_else(|| "delegate requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinDelegateTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "delegate",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "delegate",
            false,
            false,
            department_reason("delegate").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_handoff {
        let state = app_state
            .ok_or_else(|| "handoff requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinHandoffTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "handoff",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "handoff",
            false,
            false,
            department_reason("handoff").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    Ok(RuntimeToolAssembly {
        tools,
        tool_manifest,
        _mcp_screenshot_client: mcp_screenshot_client,
    })
}

async fn try_attach_desktop_screenshot_mcp_tool(
    tools: &mut Vec<Box<dyn ToolDyn>>,
) -> Result<ScreenshotMcpClient, String> {
    let exe = std::env::current_exe()
        .map_err(|err| format!("Resolve current executable for MCP screenshot failed: {err}"))?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg(MCP_SCREENSHOT_SERVER_FLAG);
    let transport = rmcp::transport::TokioChildProcess::new(cmd)
        .map_err(|err| format!("Start MCP screenshot child process failed: {err}"))?;

    let client = ().serve(transport).await.map_err(|err| {
        format!("Connect to MCP screenshot server failed: {err}")
    })?;
    let sink = client.peer().clone();
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP screenshot tools failed: {err}"))?;

    let mut attached = false;
    for def in defs {
        if def.name.as_ref() != MCP_SCREENSHOT_TOOL_NAME {
            continue;
        }
        tools.push(Box::new(rig::tool::rmcp::McpTool::from_mcp_server(
            def,
            sink.clone(),
        )));
        attached = true;
        break;
    }

    if !attached {
        return Err("MCP screenshot server did not expose desktop_screenshot tool".to_string());
    }
    Ok(client)
}


