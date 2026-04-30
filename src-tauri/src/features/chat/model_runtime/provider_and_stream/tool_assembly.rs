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

fn tool_schema_cache_store() -> &'static Mutex<Option<Vec<ProviderToolDefinition>>> {
    static STORE: OnceLock<Mutex<Option<Vec<ProviderToolDefinition>>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn tool_schema_definition_to_manifest_item(definition: &ProviderToolDefinition) -> Value {
    tool_manifest_item("schema_cache", &definition.name, true, true, None)
}

fn operate_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        MCP_OPERATE_TOOL_NAME,
        "统一桌面脚本工具。入参只有 script:string，一行一个动作。\n可用语法：\nmouse <button> click @x,y [repeat=n] [delay=s] [pre_delay=s] [press=s]\nmouse scroll_up [repeat=n] [delay=s] [pre_delay=s]\nmouse scroll_down [repeat=n] [delay=s] [pre_delay=s]\nkey <combo> [repeat=n] [delay=s] [pre_delay=s] [press=s]\ntext \"内容\" [repeat=n] [delay=s] [pre_delay=s]\nwait <seconds>\nscreenshot [focused_window] [region=@x,y,w,h] [save=\"绝对路径\"] [quality=1..100]\n参数说明：button=left|right|middle|back|forward；combo 用 + 连接按键，如 Control+L、Control+Shift+P、Enter；x/y/w/h 为 0~1 百分比坐标；repeat=1~100；delay/pre_delay/press=0~300 秒；save 必须是绝对路径；quality 默认 75。规则：screenshot 对模型只保留最新一张，旧画面视为已经离去。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "script": {
                    "type": "string",
                    "description": "桌面脚本文本，一行一个动作。"
                }
            },
            "required": ["script"]
        }),
    )
}

fn read_file_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        MCP_READ_FILE_TOOL_NAME,
        "读取本地文件内容。自动识别文本、图片、PDF 与 Office 文件；absolute_path 必须是绝对路径；分页统一使用 start/count：对文本、代码、Office 等非 PDF 内容，start 表示起始行，count 表示返回行数；对 PDF，start 表示起始页，count 表示返回页数。搜索工具若返回命中行号，可直接把该行号附近换算成 start 继续读取。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "absolute_path": {
                    "type": "string",
                    "description": "要读取的本地文件绝对路径。"
                },
                "start": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "分页起点。对文本、Office 等非 PDF 内容按行计数；对 PDF 按页计数。默认从 0 开始。"
                },
                "count": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "分页大小。对文本、Office 等非 PDF 内容表示返回多少行；对 PDF 表示返回多少页。"
                }
            },
            "required": ["absolute_path"]
        }),
    )
}

fn build_global_tool_schema_cache(state: &AppState) -> Vec<ProviderToolDefinition> {
    let preview_session_id = "__tool_schema_cache__".to_string();
    let preview_api_id = "__tool_schema_cache__".to_string();
    let preview_agent_id = DEFAULT_AGENT_ID.to_string();
    let mut definitions = vec![
        BuiltinFetchTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinBingSearchTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinRememberTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinRecallTool { app_state: state.clone() }.provider_tool_definition(),
        operate_provider_tool_definition(),
        BuiltinReloadTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinOrganizeContextTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            api_config_id: preview_api_id.clone(),
            agent_id: preview_agent_id,
        }
        .provider_tool_definition(),
        BuiltinWaitTool.provider_tool_definition(),
        read_file_provider_tool_definition(),
        BuiltinTerminalExecTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinApplyPatchTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinPlanTool.provider_tool_definition(),
        BuiltinTodoTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinTaskTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinDelegateTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinMemeTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinContactReplyTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinContactSendFilesTool {
            app_state: state.clone(),
            session_id: preview_session_id,
        }
        .provider_tool_definition(),
        BuiltinContactNoReplyTool.provider_tool_definition(),
    ];

    match load_workspace_mcp_servers(state) {
        Ok(servers) => {
            for server in servers.into_iter().filter(|server| server.enabled) {
                for tool in list_tools_from_runtime_or_policy(&server) {
                    definitions.push(ProviderToolDefinition::new(
                        tool.tool_name,
                        tool.description,
                        tool.parameters,
                    ));
                }
            }
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 加载 MCP 配置失败: {err}")),
    }

    definitions.sort_by(|a, b| a.name.cmp(&b.name));
    definitions.dedup_by(|a, b| a.name == b.name);
    definitions
}

fn refresh_global_tool_schema_cache(state: &AppState) -> Vec<ProviderToolDefinition> {
    let definitions = build_global_tool_schema_cache(state);
    match tool_schema_cache_store().lock() {
        Ok(mut guard) => {
            *guard = Some(definitions.clone());
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 刷新失败，缓存锁已损坏: {err}")),
    }
    definitions
}

fn read_global_tool_schema_cache(_state: Option<&AppState>) -> Vec<ProviderToolDefinition> {
    match tool_schema_cache_store().lock() {
        Ok(guard) => {
            if let Some(definitions) = guard.as_ref() {
                return definitions.clone();
            }
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 读取失败，缓存锁已损坏: {err}")),
    }
    Vec::new()
}

async fn assemble_runtime_tools(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> Result<RuntimeToolAssembly, String> {
    let tool_definitions = read_global_tool_schema_cache(app_state);
    let tool_manifest = tool_definitions
        .iter()
        .map(tool_schema_definition_to_manifest_item)
        .collect::<Vec<_>>();
    let mut tools: Vec<Box<dyn RuntimeToolDyn>> = Vec::new();
    if selected_api.enable_tools {
        push_runtime_tool_executors(
            &mut tools,
            app_state,
            selected_api.id.as_str(),
            agent.id.as_str(),
            tool_session_id,
        )?;
    }
    let _ = app_config;
    let _ = agent;
    Ok(RuntimeToolAssembly {
        tools,
        tool_definitions,
        tool_manifest,
        unavailable_tool_notices: Vec::new(),
    })
}

fn push_runtime_tool_executors(
    tools: &mut Vec<Box<dyn RuntimeToolDyn>>,
    app_state: Option<&AppState>,
    api_config_id: &str,
    agent_id: &str,
    tool_session_id: &str,
) -> Result<(), String> {
    let state = app_state
        .ok_or_else(|| "runtime tool execution requires app state".to_string())?
        .clone();
    tools.push(Box::new(BuiltinFetchTool { app_state: state.clone() }));
    tools.push(Box::new(BuiltinBingSearchTool { app_state: state.clone() }));
    tools.push(Box::new(BuiltinRememberTool { app_state: state.clone() }));
    tools.push(Box::new(BuiltinRecallTool { app_state: state.clone() }));
    tools.push(Box::new(LazyOperateMcpTool {
        app_state: state.clone(),
    }));
    tools.push(Box::new(BuiltinReloadTool { app_state: state.clone() }));
    tools.push(Box::new(BuiltinOrganizeContextTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
        api_config_id: api_config_id.to_string(),
        agent_id: agent_id.to_string(),
    }));
    tools.push(Box::new(BuiltinWaitTool));
    tools.push(Box::new(LazyReadFileMcpTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
        api_config_id: api_config_id.to_string(),
    }));
    tools.push(Box::new(BuiltinTerminalExecTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinApplyPatchTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinPlanTool));
    tools.push(Box::new(BuiltinTodoTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinTaskTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinDelegateTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinMemeTool { app_state: state.clone() }));
    tools.push(Box::new(BuiltinContactReplyTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinContactSendFilesTool {
        app_state: state.clone(),
        session_id: tool_session_id.to_string(),
    }));
    tools.push(Box::new(BuiltinContactNoReplyTool));
    push_cached_mcp_runtime_tools(tools, &state);
    Ok(())
}

fn push_cached_mcp_runtime_tools(tools: &mut Vec<Box<dyn RuntimeToolDyn>>, state: &AppState) {
    let servers = match load_workspace_mcp_servers(state) {
        Ok(servers) => servers,
        Err(err) => {
            runtime_log_warn(format!("[MCP] 装配 MCP 工具执行器失败，加载配置失败: {err}"));
            return;
        }
    };
    let existing_names = tools.iter().map(|tool| tool.name()).collect::<HashSet<_>>();
    let mut added_names = HashSet::<String>::new();
    for server in servers.into_iter().filter(|server| server.enabled) {
        for descriptor in list_tools_from_runtime_or_policy(&server).into_iter().filter(|tool| tool.enabled) {
            if existing_names.contains(&descriptor.tool_name) || !added_names.insert(descriptor.tool_name.clone()) {
                continue;
            }
            let input_schema = Arc::new(match descriptor.parameters {
                Value::Object(map) => map,
                _ => serde_json::Map::new(),
            });
            let definition = rmcp::model::Tool::new(
                descriptor.tool_name.clone(),
                descriptor.description,
                input_schema,
            );
            tools.push(Box::new(CachedMcpRuntimeTool {
                server: server.clone(),
                definition,
            }));
        }
    }
}

#[derive(Debug, Clone)]
struct LazyOperateMcpTool {
    app_state: AppState,
}

#[derive(Debug, Clone)]
struct LazyReadFileMcpTool {
    app_state: AppState,
    session_id: String,
    api_config_id: String,
}

const TOOL_RUNTIME_CHECK_TIMEOUT_SECS: u64 = 8;
const MCP_TOOL_RUNTIME_EXEC_TIMEOUT_SECS: u64 = 300;

fn tool_unavailable_timeout_result(tool_name: &str) -> ProviderToolResult {
    ProviderToolResult::error(format!(
        "工具 `{tool_name}` 当前不可用：可用性检查超时。请先调用 reload 重建工具缓存后再试。"
    ))
}

fn tool_unavailable_error_result(tool_name: &str, err: String) -> ProviderToolResult {
    ProviderToolResult::error(format!(
        "工具 `{tool_name}` 当前不可用：{err}。请先调用 reload 重建工具缓存后再试。"
    ))
}

fn tool_execution_timeout_result(tool_name: &str) -> ProviderToolResult {
    ProviderToolResult::error(format!("工具 `{tool_name}` 执行超时，已取消本次调用。"))
}

async fn cancel_lazy_mcp_client(
    tool_name: &str,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
) -> Result<(), String> {
    client.cancel().await.map(|_| ()).map_err(|err| {
        let message = format!("取消 MCP 工具 `{tool_name}` 连接失败: {err}");
        runtime_log_warn(format!("[工具执行] {message}"));
        message
    })
}

fn trigger_tool_schema_reload_after_timeout(app_state: AppState, tool_name: &'static str) {
    tokio::spawn(async move {
        runtime_log_warn(format!(
            "[工具Schema缓存] 工具检验超时，开始后台 reload: tool={tool_name}"
        ));
        if let Err(err) = builtin_reload(&app_state).await {
            runtime_log_warn(format!(
                "[工具Schema缓存] 工具检验超时后的后台 reload 失败: tool={tool_name}, error={err}"
            ));
        }
    });
}

async fn call_lazy_operate_mcp_tool(
    app_state: AppState,
    args_json: String,
) -> Result<ProviderToolResult, String> {
    let timeout = std::time::Duration::from_secs(TOOL_RUNTIME_CHECK_TIMEOUT_SECS);
    let connect = tokio::time::timeout(timeout, connect_operate_mcp_tool()).await;
    let (definition, client) = match connect {
        Ok(Ok(value)) => value,
        Ok(Err(err)) => return Ok(tool_unavailable_error_result(MCP_OPERATE_TOOL_NAME, err)),
        Err(_) => {
            trigger_tool_schema_reload_after_timeout(app_state, MCP_OPERATE_TOOL_NAME);
            return Ok(tool_unavailable_timeout_result(MCP_OPERATE_TOOL_NAME));
        }
    };
    let tool = McpRuntimeTool {
        definition,
        client: client.peer().clone(),
    };
    let execute_timeout = std::time::Duration::from_secs(MCP_TOOL_RUNTIME_EXEC_TIMEOUT_SECS);
    let result = match tokio::time::timeout(execute_timeout, tool.call_json(args_json)).await {
        Ok(result) => result,
        Err(_) => {
            runtime_log_warn(format!(
                "[工具执行] MCP 工具执行超时: tool={}, timeout_ms={}",
                MCP_OPERATE_TOOL_NAME,
                execute_timeout.as_millis()
            ));
            Ok(tool_execution_timeout_result(MCP_OPERATE_TOOL_NAME))
        }
    };
    cancel_lazy_mcp_client(MCP_OPERATE_TOOL_NAME, client).await?;
    result
}

async fn call_lazy_read_file_mcp_tool(
    app_state: AppState,
    session_id: String,
    api_config_id: String,
    args_json: String,
) -> Result<ProviderToolResult, String> {
    let timeout = std::time::Duration::from_secs(TOOL_RUNTIME_CHECK_TIMEOUT_SECS);
    let connect = tokio::time::timeout(
        timeout,
        connect_read_file_mcp_tool(&session_id, &api_config_id),
    )
    .await;
    let (definition, client) = match connect {
        Ok(Ok(value)) => value,
        Ok(Err(err)) => return Ok(tool_unavailable_error_result(MCP_READ_FILE_TOOL_NAME, err)),
        Err(_) => {
            trigger_tool_schema_reload_after_timeout(app_state, MCP_READ_FILE_TOOL_NAME);
            return Ok(tool_unavailable_timeout_result(MCP_READ_FILE_TOOL_NAME));
        }
    };
    let tool = McpRuntimeTool {
        definition,
        client: client.peer().clone(),
    };
    let execute_timeout = std::time::Duration::from_secs(MCP_TOOL_RUNTIME_EXEC_TIMEOUT_SECS);
    let result = match tokio::time::timeout(execute_timeout, tool.call_json(args_json)).await {
        Ok(result) => result,
        Err(_) => {
            runtime_log_warn(format!(
                "[工具执行] MCP 工具执行超时: tool={}, timeout_ms={}",
                MCP_READ_FILE_TOOL_NAME,
                execute_timeout.as_millis()
            ));
            Ok(tool_execution_timeout_result(MCP_READ_FILE_TOOL_NAME))
        }
    };
    cancel_lazy_mcp_client(MCP_READ_FILE_TOOL_NAME, client).await?;
    result
}

impl RuntimeToolDyn for LazyOperateMcpTool {
    fn name(&self) -> String {
        MCP_OPERATE_TOOL_NAME.to_string()
    }

    fn is_mcp_tool(&self) -> bool {
        true
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        let app_state = self.app_state.clone();
        Box::pin(async move { call_lazy_operate_mcp_tool(app_state, args_json).await })
    }
}

impl RuntimeToolDyn for LazyReadFileMcpTool {
    fn name(&self) -> String {
        MCP_READ_FILE_TOOL_NAME.to_string()
    }

    fn is_mcp_tool(&self) -> bool {
        true
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        let app_state = self.app_state.clone();
        let session_id = self.session_id.clone();
        let api_config_id = self.api_config_id.clone();
        Box::pin(async move {
            call_lazy_read_file_mcp_tool(app_state, session_id, api_config_id, args_json).await
        })
    }
}

async fn connect_read_file_mcp_tool(
    tool_session_id: &str,
    api_config_id: &str,
) -> Result<(rmcp::model::Tool, rmcp::service::RunningService<rmcp::RoleClient, ()>), String> {
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
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP read_file tools failed: {err}"))?;

    for def in defs {
        if def.name.as_ref() != MCP_READ_FILE_TOOL_NAME {
            continue;
        }
        return Ok((def, client));
    }

    Err("MCP read_file server did not expose read_file tool".to_string())
}

async fn connect_operate_mcp_tool(
) -> Result<(rmcp::model::Tool, rmcp::service::RunningService<rmcp::RoleClient, ()>), String> {
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
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP operate tools failed: {err}"))?;

    for def in defs {
        if def.name.as_ref() != MCP_OPERATE_TOOL_NAME {
            continue;
        }
        return Ok((def, client));
    }

    Err("MCP operate server did not expose operate tool".to_string())
}
