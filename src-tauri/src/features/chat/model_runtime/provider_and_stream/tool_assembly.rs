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

fn delegate_tool_runtime_disabled_reason(
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> Option<String> {
    let state = app_state?;
    let (_, _, conversation_id) = delegate_parse_session_parts(tool_session_id);
    let conversation_id = conversation_id?;
    match delegate_runtime_thread_get(state, &conversation_id) {
        Ok(Some(_)) => Some("委托线程中禁止再次调用 delegate".to_string()),
        Ok(None) => None,
        Err(err) => {
            eprintln!(
                "[工具] delegate 已禁用：委托运行时查询失败: session_id={}, conversation_id={}, error={}",
                tool_session_id,
                conversation_id,
                err
            );
            Some("delegate 运行态检查失败，已禁止在当前委托线程中继续委托".to_string())
        }
    }
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
    let has_command = tool_enabled(selected_api, agent, current_department, "command");
    let has_exec = tool_enabled(selected_api, agent, current_department, "exec");
    let has_apply_patch = tool_enabled(selected_api, agent, current_department, "apply_patch");
    let has_task = tool_enabled(selected_api, agent, current_department, "task");
    let has_delegate_base = tool_enabled(selected_api, agent, current_department, "delegate");
    let delegate_runtime_reason = if has_delegate_base {
        delegate_tool_runtime_disabled_reason(app_state, tool_session_id)
    } else {
        None
    };
    let has_delegate = has_delegate_base && delegate_runtime_reason.is_none();
    let has_remote_im_send = tool_enabled(selected_api, agent, current_department, "remote_im_send");
    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    let mut tool_manifest = Vec::<Value>::new();
    let mut mcp_screenshot_client: Option<ScreenshotMcpClient> = None;

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
        tools.push(Box::new(BuiltinRememberTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "remember", true, true, None));
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
        tools.push(Box::new(BuiltinRecallTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "recall", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "recall",
            false,
            false,
            department_reason("recall").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_screenshot {
        match try_attach_desktop_screenshot_mcp_tool(&mut tools).await {
            Ok(client) => {
                mcp_screenshot_client = Some(client);
                tool_manifest.push(tool_manifest_item("builtin_mcp", "screenshot", true, true, None));
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
                    tool_manifest.push(tool_manifest_item("mcp_runtime", &name, true, true, None));
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

    if has_command {
        let state = app_state
            .ok_or_else(|| "command requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinCommandTool {
            app_state: state,
            api_config_id: selected_api.id.clone(),
            agent_id: agent.id.clone(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "command",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "command",
            false,
            false,
            department_reason("command")
                .or_else(|| Some("当前人格未启用该工具".to_string())),
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
        tool_manifest.push(tool_manifest_item("builtin", "exec", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "exec",
            false,
            false,
            department_reason("exec").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_apply_patch {
        let state = app_state
            .ok_or_else(|| "apply_patch requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinApplyPatchTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item("builtin", "apply_patch", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "apply_patch",
            false,
            false,
            department_reason("apply_patch").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_task {
        let state = app_state
            .ok_or_else(|| "task requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTaskTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "task", true, true, None));
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
        tool_manifest.push(tool_manifest_item("builtin", "delegate", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "delegate",
            false,
            false,
            delegate_runtime_reason
                .or_else(|| department_reason("delegate"))
                .or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_remote_im_send {
        let state = app_state
            .ok_or_else(|| "remote_im_send requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinRemoteImSendTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "remote_im_send", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "remote_im_send",
            false,
            false,
            department_reason("remote_im_send")
                .or_else(|| Some("当前人格未启用该工具".to_string())),
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
