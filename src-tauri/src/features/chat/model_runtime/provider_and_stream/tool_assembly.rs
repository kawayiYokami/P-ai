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

fn tool_session_is_remote_im_contact_conversation(
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> bool {
    let Some(state) = app_state else {
        return false;
    };
    let Some((_, conversation_id)) = tool_session_id.split_once("::") else {
        return false;
    };
    conversation_service()
        .is_remote_im_contact_conversation(state, conversation_id)
        .unwrap_or(false)
}

fn tool_session_has_bound_remote_im_contact(
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> bool {
    let Some(state) = app_state else {
        return false;
    };
    let Some((_, conversation_id)) = tool_session_id.split_once("::") else {
        return false;
    };
    get_conversation_remote_im_activation_sources(state, conversation_id)
        .ok()
        .and_then(|sources| resolve_bound_remote_im_activation_source(&sources))
        .is_some()
}

async fn assemble_runtime_tools(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> Result<RuntimeToolAssembly, String> {
    // 约束：高风险系统能力优先走 MCP 形态，以统一协议、媒体转发和权限边界。
    // 但像 todo 这种强依赖主进程会话状态与 UI 推送的工具，保留为进程内 builtin，
    // 避免再引入跨进程状态同步与双写问题。
    let current_department = department_for_agent_id(app_config, &agent.id);
    let department_reason = |tool_id: &str| tool_restricted_by_department(current_department, tool_id);
    let is_remote_im_contact_conversation =
        tool_session_is_remote_im_contact_conversation(app_state, tool_session_id);
    let has_bound_remote_im_contact =
        tool_session_has_bound_remote_im_contact(app_state, tool_session_id);
    let is_local_conversation = !is_remote_im_contact_conversation;
    let tools_globally_enabled = selected_api.enable_tools;
    let has_fetch = tool_enabled(selected_api, agent, current_department, "fetch");
    let has_websearch = tool_enabled(selected_api, agent, current_department, "websearch");
    let has_remember = tools_globally_enabled;
    let has_recall = tools_globally_enabled;
    let has_operate = tool_enabled(selected_api, agent, current_department, "operate");
    let has_reload = tool_enabled(selected_api, agent, current_department, "reload");
    let has_organize_context =
        tool_enabled(selected_api, agent, current_department, "organize_context");
    let has_wait = tool_enabled(selected_api, agent, current_department, "wait");
    let has_exec = tool_enabled(selected_api, agent, current_department, "exec");
    let has_read_file = tool_enabled(selected_api, agent, current_department, "read_file");
    let has_apply_patch = tool_enabled(selected_api, agent, current_department, "apply_patch");
    let has_plan = tools_globally_enabled && is_local_conversation;
    let has_task = tool_enabled(selected_api, agent, current_department, "task");
    let has_todo = tools_globally_enabled;
    let has_delegate_base = tool_enabled(selected_api, agent, current_department, "delegate");
    let has_meme = tool_enabled(selected_api, agent, current_department, "meme");
    let has_contact_reply = tools_globally_enabled && has_bound_remote_im_contact;
    let has_contact_send_files = tools_globally_enabled && has_bound_remote_im_contact;
    let has_contact_no_reply = tools_globally_enabled && has_bound_remote_im_contact;
    let delegate_runtime_reason = if has_delegate_base {
        delegate_tool_runtime_disabled_reason(app_state, tool_session_id)
    } else {
        None
    };
    let has_delegate = has_delegate_base && delegate_runtime_reason.is_none();
    let mut tools: Vec<Box<dyn RuntimeToolDyn>> = Vec::new();
    let mut tool_manifest = Vec::<Value>::new();
    let mut unavailable_tool_notices = Vec::<String>::new();
    let mut mcp_read_file_client: Option<ReadFileMcpClient> = None;
    let mut mcp_operate_client: Option<OperateMcpClient> = None;

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
        let state = app_state
            .ok_or_else(|| "fetch requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinFetchTool { app_state: state }));
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
        let state = app_state
            .ok_or_else(|| "websearch requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinBingSearchTool { app_state: state }));
    }

    if has_remember {
        let state = app_state
            .ok_or_else(|| "remember requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinRememberTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "remember", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item("builtin", "remember", false, false, Some("当前模型未启用工具调用。".to_string())));
    }

    if has_recall {
        let state = app_state
            .ok_or_else(|| "recall requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinRecallTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "recall", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item("builtin", "recall", false, false, Some("当前模型未启用工具调用。".to_string())));
    }

    if has_operate {
        match try_attach_operate_mcp_tool(&mut tools).await {
            Ok(client) => {
                mcp_operate_client = Some(client);
                tool_manifest.push(tool_manifest_item("builtin_mcp", "operate", true, true, None));
            }
            Err(err) => {
                eprintln!("[MCP] operate degraded to disabled: {err}");
                unavailable_tool_notices.push(format!(
                    "工具 `operate` MCP 挂载失败：{}。",
                    err
                ));
                tool_manifest.push(tool_manifest_item(
                    "builtin_mcp",
                    "operate",
                    true,
                    false,
                    Some(format!("MCP attach failed: {err}")),
                ));
            }
        }
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin_mcp",
            "operate",
            false,
            false,
            department_reason("operate").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    match attach_enabled_mcp_tools_for_runtime(&mut tools, app_state, current_department).await {
        Ok(outcome) => {
            unavailable_tool_notices.extend(outcome.unavailable_tool_notices);
            if outcome.attached_tool_names.is_empty() {
                tool_manifest.push(tool_manifest_item(
                    "mcp_runtime",
                    "(none)",
                    true,
                    false,
                    Some("no enabled MCP tools attached".to_string()),
                ));
            } else {
                for name in outcome.attached_tool_names {
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

    if has_reload {
        let state = app_state
            .ok_or_else(|| "reload requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinReloadTool { app_state: state }));
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
            department_reason("reload")
                .or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_organize_context {
        let state = app_state
            .ok_or_else(|| "organize_context requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinOrganizeContextTool {
            app_state: state,
            api_config_id: selected_api.id.clone(),
            agent_id: agent.id.clone(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "organize_context",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "organize_context",
            false,
            false,
            department_reason("organize_context")
                .or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_wait {
        tools.push(Box::new(BuiltinWaitTool));
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
            department_reason("wait")
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

    if has_read_file {
        match try_attach_read_file_mcp_tool(
            &mut tools,
            tool_session_id,
            selected_api.id.as_str(),
        )
        .await
        {
            Ok(client) => {
                mcp_read_file_client = Some(client);
                tool_manifest.push(tool_manifest_item("builtin_mcp", "read_file", true, true, None));
            }
            Err(err) => {
                eprintln!(
                    "[MCP] 失败，任务=read_file，触发条件=MCP 附加失败，error={}",
                    err
                );
                unavailable_tool_notices.push(format!(
                    "工具 `read_file` MCP 挂载失败：{}。",
                    err
                ));
                tool_manifest.push(tool_manifest_item(
                    "builtin_mcp",
                    "read_file",
                    true,
                    false,
                    Some(format!(
                        "MCP 附加失败（任务=read_file，触发条件=MCP 附加失败，error={}）",
                        err
                    )),
                ));
            }
        }
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin_mcp",
            "read_file",
            false,
            false,
            department_reason("read_file").or_else(|| Some("当前人格未启用该工具".to_string())),
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

    if has_plan {
        tools.push(Box::new(BuiltinPlanTool));
        tool_manifest.push(tool_manifest_item("builtin", "plan", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "plan",
            false,
            false,
            if !tools_globally_enabled {
                Some("当前模型未启用工具调用。".to_string())
            } else {
                Some("plan 仅在本地会话中可用。".to_string())
            },
        ));
    }

    if has_todo {
        let state = app_state
            .ok_or_else(|| "todo requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTodoTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item("builtin", "todo", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item("builtin", "todo", false, false, Some("当前模型未启用工具调用。".to_string())));
    }

    if has_task {
        let state = app_state
            .ok_or_else(|| "task requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTaskTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
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

    if has_meme {
        let state = app_state
            .ok_or_else(|| "meme requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemeTool { app_state: state }));
        tool_manifest.push(tool_manifest_item("builtin", "meme", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "meme",
            false,
            false,
            department_reason("meme").or_else(|| Some("当前人格未启用该工具".to_string())),
        ));
    }

    if has_contact_reply {
        let state = app_state
            .ok_or_else(|| "contact_reply requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinContactReplyTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item("builtin", "contact_reply", true, true, None));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "contact_reply",
            false,
            false,
            if !tools_globally_enabled {
                Some("当前模型未启用工具调用。".to_string())
            } else {
                Some("contact_reply 仅在本轮已绑定联系人时可用。".to_string())
            },
        ));
    }

    if has_contact_send_files {
        let state = app_state
            .ok_or_else(|| "contact_send_files requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinContactSendFilesTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "contact_send_files",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "contact_send_files",
            false,
            false,
            if !tools_globally_enabled {
                Some("当前模型未启用工具调用。".to_string())
            } else {
                Some("contact_send_files 仅在本轮已绑定联系人时可用。".to_string())
            },
        ));
    }

    if has_contact_no_reply {
        tools.push(Box::new(BuiltinContactNoReplyTool));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "contact_no_reply",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "contact_no_reply",
            false,
            false,
            if !tools_globally_enabled {
                Some("当前模型未启用工具调用。".to_string())
            } else {
                Some("contact_no_reply 仅在本轮已绑定联系人时可用。".to_string())
            },
        ));
    }

    Ok(RuntimeToolAssembly {
        tools,
        tool_manifest,
        unavailable_tool_notices,
        _mcp_read_file_client: mcp_read_file_client,
        _mcp_operate_client: mcp_operate_client,
    })
}

async fn try_attach_read_file_mcp_tool(
    tools: &mut Vec<Box<dyn RuntimeToolDyn>>,
    tool_session_id: &str,
    api_config_id: &str,
) -> Result<ReadFileMcpClient, String> {
    let exe = std::env::current_exe()
        .map_err(|err| format!("Resolve current executable for MCP read_file failed: {err}"))?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg(MCP_READ_FILE_SERVER_FLAG);
    cmd.arg(MCP_READ_FILE_SESSION_FLAG).arg(tool_session_id);
    cmd.arg(MCP_READ_FILE_API_FLAG).arg(api_config_id);
    let transport = rmcp::transport::TokioChildProcess::new(cmd)
        .map_err(|err| format!("Start MCP read_file child process failed: {err}"))?;

    let client = ().serve(transport).await.map_err(|err| {
        format!("Connect to MCP read_file server failed: {err}")
    })?;
    let sink = client.peer().clone();
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP read_file tools failed: {err}"))?;

    let mut attached = false;
    for def in defs {
        if def.name.as_ref() != MCP_READ_FILE_TOOL_NAME {
            continue;
        }
        tools.push(boxed_mcp_runtime_tool(def, sink.clone()));
        attached = true;
        break;
    }

    if !attached {
        return Err("MCP read_file server did not expose read_file tool".to_string());
    }
    Ok(client)
}

async fn try_attach_operate_mcp_tool(
    tools: &mut Vec<Box<dyn RuntimeToolDyn>>,
) -> Result<OperateMcpClient, String> {
    let exe = std::env::current_exe()
        .map_err(|err| format!("Resolve current executable for MCP operate failed: {err}"))?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg(MCP_OPERATE_SERVER_FLAG);
    let transport = rmcp::transport::TokioChildProcess::new(cmd)
        .map_err(|err| format!("Start MCP operate child process failed: {err}"))?;

    let client = ()
        .serve(transport)
        .await
        .map_err(|err| format!("Connect to MCP operate server failed: {err}"))?;
    let sink = client.peer().clone();
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP operate tools failed: {err}"))?;

    let mut attached = false;
    for def in defs {
        if def.name.as_ref() != MCP_OPERATE_TOOL_NAME {
            continue;
        }
        tools.push(boxed_mcp_runtime_tool(def, sink.clone()));
        attached = true;
        break;
    }

    if !attached {
        return Err("MCP operate server did not expose operate tool".to_string());
    }
    Ok(client)
}
