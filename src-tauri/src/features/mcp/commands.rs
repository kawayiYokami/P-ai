fn normalize_mcp_server_input(input: McpServerInput) -> Result<McpServerConfig, String> {
    let id = input.id.trim().to_string();
    if id.is_empty() {
        return Err("MCP server id is required".to_string());
    }
    let input_name = input.name.trim().to_string();
    let raw_definition_json = input.definition_json.trim();
    if raw_definition_json.is_empty() {
        return Err("MCP definition JSON is required".to_string());
    }
    let definition_json = normalize_mcp_definition_member_names(raw_definition_json)?;
    // 卡片 = 一组 MCP：definitionJson 可包含多个服务器，保存时规范化成员名。
    // 组内首个成员名作为解析名兜底
    let parsed_name = parse_mcp_definition_servers(&definition_json)
        .ok()
        .and_then(|parsed| parsed.servers.into_iter().next().map(|(name, _)| name))
        .unwrap_or_else(|| id.clone());
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
        oauth_capable: false,
        has_oauth_token: false,
        tool_policies: Vec::new(),
        cached_tools: Vec::new(),
        instructions: String::new(),
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
    // 握手说明是进程内内存态：连过才有，重启后为空。空值不送出去，前端按空回落地址显示。
    server.instructions = mcp_server_instructions_get(&server.id).unwrap_or_default();
    server
}

fn load_server_by_id(state: &AppState, server_id: &str) -> Result<McpServerConfig, String> {
    load_workspace_mcp_servers(state)?
        .into_iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| format!("MCP server '{}' not found", server_id))
}

fn list_tools_from_runtime(server: &McpServerConfig) -> Vec<McpToolDescriptor> {
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
    Vec::new()
}

const MCP_SUPERVISOR_STDIO_CONCURRENCY: usize = 3;
const MCP_SUPERVISOR_REMOTE_CONCURRENCY: usize = 20;

fn mcp_supervisor_stdio_semaphore() -> Arc<tokio::sync::Semaphore> {
    static SEMAPHORE: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    SEMAPHORE
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MCP_SUPERVISOR_STDIO_CONCURRENCY)))
        .clone()
}

fn mcp_supervisor_remote_semaphore() -> Arc<tokio::sync::Semaphore> {
    static SEMAPHORE: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    SEMAPHORE
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MCP_SUPERVISOR_REMOTE_CONCURRENCY)))
        .clone()
}

fn mcp_supervisor_semaphore_for_server(server: &McpServerConfig) -> Arc<tokio::sync::Semaphore> {
    // 组内任一成员为远程（streamable HTTP / SSE）时按远程并发控制
    let has_remote = parse_mcp_group_definitions(server)
        .map(|members| {
            members.iter().any(|(_, _, parsed)| {
                matches!(
                    parsed.transport,
                    McpTransportKind::StreamableHttp | McpTransportKind::Sse
                )
            })
        })
        .unwrap_or(false);
    if has_remote {
        mcp_supervisor_remote_semaphore()
    } else {
        mcp_supervisor_stdio_semaphore()
    }
}

fn mcp_runtime_state_mark_starting(server: &McpServerConfig) {
    let cached_tools = list_tools_from_runtime(server);
    mcp_runtime_state_set(&server.id, true, "starting", "", cached_tools);
}

fn mcp_runtime_state_mark_probe_failure(server: &McpServerConfig, status: &str, error: &str) {
    let cached_tools = list_tools_from_runtime(server);
    let effective_status = if status == "auth_required" {
        "auth_required"
    } else if cached_tools.is_empty() {
        status
    } else {
        "stale"
    };
    mcp_runtime_state_set(&server.id, true, effective_status, error, cached_tools);
}

fn mcp_current_server_matches_probe(
    state: &AppState,
    probe_server: &McpServerConfig,
    trigger: &str,
) -> Option<McpServerConfig> {
    match load_server_by_id(state, &probe_server.id) {
        Ok(current) => {
            if !current.enabled {
                runtime_log_warn(format!(
                    "[MCP监管] 跳过提交 server_id={} trigger={} reason=disabled",
                    probe_server.id, trigger
                ));
                return None;
            }
            if current.definition_json != probe_server.definition_json {
                runtime_log_warn(format!(
                    "[MCP监管] 跳过提交 server_id={} trigger={} reason=definition_changed",
                    probe_server.id, trigger
                ));
                return None;
            }
            Some(current)
        }
        Err(err) => {
            runtime_log_warn(format!(
                "[MCP监管] 跳过提交 server_id={} trigger={} reason=missing error={}",
                probe_server.id, trigger, err
            ));
            None
        }
    }
}

fn mcp_check_and_extract_auth_challenge(err: &str) -> Option<String> {
    // 只认连接层从 WWW-Authenticate 头构造的 auth_required: 前缀；
    // 普通 401 文本（如 SSE transport、token 过期）不含 challenge 参数，不能当作 OAuth 挑战。
    //
    // 前缀后面必须真的带 challenge：服务器回了空值的 WWW-Authenticate 时，
    // 连接层会把空串塞进 AuthRequiredError，渲染出来就是 "auth_required: "。
    // 那不是 OAuth 服务，当成挑战会把普通 401 误标成需要授权登录。
    let idx = err.find("auth_required:")?;
    let after = &err[idx + "auth_required:".len()..];
    let challenge = after.split(" | ").next().unwrap_or(after).trim();
    if challenge.is_empty() {
        return None;
    }
    Some(challenge.to_string())
}

/// 成功探测之后，是否要清掉历史遗留的 oauth_capable 标记。
///
/// 探测成功说明这个服务现在不需要 OAuth；误判留下的标记会让卡片上的「OAuth 登录」
/// 按钮一直挂在连得好好的服务上。但有凭证的服务必须保留标记，否则下次 token 失效时
/// 就没人知道它还能重新授权。
fn mcp_stale_oauth_mark_should_clear(
    oauth_capable: bool,
    has_oauth_credentials: bool,
) -> bool {
    oauth_capable && !has_oauth_credentials
}

/// 失败探测之后，是否要清掉历史遗留的 oauth_capable 标记。
///
/// 只有「非挑战的失败」能证明这个服务不需要 OAuth：它回了非挑战的 4xx/5xx，
/// 说明它没在要授权，只是连不上（没配 key、key 错、网关不认）。这类服务如果一直失败，
/// 陈旧的标记就永远没有机会被成功路径清掉，卡片上的黄标会一直挂着。
///
/// `timeout` 不清：超时的服务可能正好是要 OAuth 的，证据不足。
/// `auth_required` 更不能清：那是真挑战。
fn mcp_stale_oauth_mark_should_clear_after_failure(
    runtime_status: &str,
    oauth_capable: bool,
    has_oauth_credentials: bool,
) -> bool {
    runtime_status == "failed"
        && mcp_stale_oauth_mark_should_clear(oauth_capable, has_oauth_credentials)
}

/// 清掉陈旧的 oauth_capable 标记：写盘 + 清内存挑战缓存，返回是否真的清了。
///
/// 判据由调用方给：探测成功用 [`mcp_stale_oauth_mark_should_clear`]，
/// 探测失败用 [`mcp_stale_oauth_mark_should_clear_after_failure`]。
fn mcp_clear_stale_oauth_capable_mark(state: &AppState, server_id: &str) -> bool {
    match set_workspace_mcp_policy_oauth_capable(state, server_id, false) {
        Ok(()) => {
            mcp_oauth_clear_cached_challenge(server_id);
            runtime_log_info(format!(
                "[MCP OAuth] 清理陈旧标记，任务=clear_oauth_capable，server_id={}，原因=探测结果证明不需要 OAuth",
                server_id
            ));
            true
        }
        Err(err) => {
            runtime_log_warn(format!(
                "[MCP OAuth] 清理陈旧标记失败，server_id={}，error={}",
                server_id, err
            ));
            false
        }
    }
}

fn mcp_status_from_runtime_error(error: &str) -> &'static str {
    if mcp_check_and_extract_auth_challenge(error).is_some() {
        "auth_required"
    } else if error.to_ascii_lowercase().contains("timed out") || error.contains("超时") {
        "timeout"
    } else {
        "failed"
    }
}

fn mcp_start_supervisor_probe_for_server(state: AppState, server: McpServerConfig, trigger: &'static str) {
    let semaphore = mcp_supervisor_semaphore_for_server(&server);
    tauri::async_runtime::spawn(async move {
        let permit = match semaphore.acquire_owned().await {
            Ok(permit) => permit,
            Err(err) => {
                runtime_log_warn(format!(
                    "[MCP监管] 跳过 server_id={} trigger={} reason=semaphore_closed error={}",
                    server.id, trigger, err
                ));
                return;
            }
        };
        let _permit = permit;
        mcp_probe_server_tools_background(state, server, trigger).await;
    });
}

fn mcp_start_supervisor_probe_all_from_policy(state: AppState, trigger: &'static str) -> Result<(), String> {
    let servers = load_workspace_mcp_servers(&state)?;
    let mut started = 0usize;
    for server in servers.into_iter() {
        if server.enabled {
            mcp_runtime_state_mark_starting(&server);
            mcp_start_supervisor_probe_for_server(state.clone(), server, trigger);
            started += 1;
        } else {
            mcp_runtime_state_set(&server.id, false, "disabled", "", Vec::new());
        }
    }
    runtime_log_info(format!(
        "[MCP监管] 开始 trigger={} enabled_servers={} stdio_concurrency={} remote_concurrency={}",
        trigger,
        started,
        MCP_SUPERVISOR_STDIO_CONCURRENCY,
        MCP_SUPERVISOR_REMOTE_CONCURRENCY
    ));
    Ok(())
}

async fn mcp_probe_server_tools_background(
    state: AppState,
    server: McpServerConfig,
    trigger: &'static str,
) {
    let started = std::time::Instant::now();
    runtime_log_info(format!(
        "[MCP监管] 开始 server_id={} trigger={}",
        server.id, trigger
    ));
    let tools_res = mcp_list_server_tools_runtime(&server).await;

    let tools = match tools_res {
        Ok(tools) => tools,
        Err(err) => {
            let Some(mut current_server) = mcp_current_server_matches_probe(&state, &server, trigger) else {
                mcp_disconnect_cached_client_if_definition(&server.id, &server.definition_json).await;
                return;
            };
            if let Some(challenge) = mcp_check_and_extract_auth_challenge(&err) {
                let _ = set_workspace_mcp_policy_oauth_capable(&state, &current_server.id, true);
                current_server.oauth_capable = true;
                let challenge_trimmed = challenge.trim();
                if !challenge_trimmed.is_empty() {
                    mcp_oauth_set_cached_challenge(&current_server.id, challenge_trimmed);
                }
                mcp_runtime_state_mark_probe_failure(&current_server, "auth_required", "需要 OAuth 授权登录");
                runtime_log_info(format!(
                    "[MCP OAuth] 探测到需要授权登录 server_id={} trigger={}",
                    current_server.id, trigger
                ));
                return;
            }
            let status = mcp_status_from_runtime_error(&err);
            // 非挑战的失败说明它不是要授权，而是真的连不上：陈旧标记要清掉，
            // 否则一直 401 的服务永远等不到成功路径，黄标会一直挂着。
            if mcp_stale_oauth_mark_should_clear_after_failure(
                status,
                current_server.oauth_capable,
                mcp_oauth_has_credentials(&state, &current_server.id),
            ) {
                let _ = mcp_clear_stale_oauth_capable_mark(&state, &current_server.id);
            }
            mcp_runtime_state_mark_probe_failure(&current_server, status, &err);
            let label = if status == "timeout" { "超时" } else { "失败" };
            runtime_log_warn(format!(
                "[MCP监管] {} server_id={} trigger={} duration_ms={} error={}",
                label,
                server.id,
                trigger,
                started.elapsed().as_millis(),
                err
            ));
            return;
        }
    };

    let Some(current_server) = mcp_current_server_matches_probe(&state, &server, trigger) else {
        mcp_disconnect_cached_client_if_definition(&server.id, &server.definition_json).await;
        return;
    };
    let discovered_names = tools
        .iter()
        .map(|t| t.tool_name.clone())
        .collect::<Vec<_>>();
    let merged_policies = match merge_workspace_mcp_tool_policies_with_new_tools(
        &state,
        &current_server.id,
        &discovered_names,
    ) {
        Ok(policies) => policies,
        Err(err) => {
            let Some(current_server) = mcp_current_server_matches_probe(&state, &server, trigger) else {
                mcp_disconnect_cached_client_if_definition(&server.id, &server.definition_json).await;
                return;
            };
            mcp_runtime_state_mark_probe_failure(&current_server, "failed", &err);
            runtime_log_warn(format!(
                "[MCP监管] 失败 server_id={} trigger={} stage=merge_policy duration_ms={} error={}",
                server.id,
                trigger,
                started.elapsed().as_millis(),
                err
            ));
            return;
        }
    };

    let Some(mut server_with_policies) = mcp_current_server_matches_probe(&state, &server, trigger) else {
        mcp_disconnect_cached_client_if_definition(&server.id, &server.definition_json).await;
        return;
    };
    server_with_policies.tool_policies = merged_policies;
    let final_tools = tools
        .into_iter()
        .map(|tool| {
            let enabled = mcp_policy_enabled_for_tool(&server_with_policies, &tool.tool_name)
                && mcp_tool_allowed_by_definition(&server_with_policies, &tool.tool_name);
            McpToolDescriptor { enabled, ..tool }
        })
        .collect::<Vec<_>>();
    let tool_count = final_tools.len();
    // 探测成功之后，历史误判留下的 oauth_capable 标记要一起清掉（有凭证的除外）。
    if mcp_stale_oauth_mark_should_clear(
        server_with_policies.oauth_capable,
        mcp_oauth_has_credentials(&state, &server_with_policies.id),
    ) {
        let _ = mcp_clear_stale_oauth_capable_mark(&state, &server_with_policies.id);
    }
    mcp_runtime_state_set(&server_with_policies.id, true, "ready", "", final_tools);
    refresh_global_tool_schema_cache(&state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(&state);
    runtime_log_info(format!(
        "[MCP监管] 完成 server_id={} trigger={} tools={} duration_ms={}",
        server.id,
        trigger,
        tool_count,
        started.elapsed().as_millis()
    ));
}

#[tauri::command]
fn mcp_list_servers(state: State<'_, AppState>) -> Result<Vec<McpServerConfig>, String> {
    mcp_list_servers_inner(state.inner())
}

fn mcp_list_servers_inner(state: &AppState) -> Result<Vec<McpServerConfig>, String> {
    let mut out = load_workspace_mcp_servers(state)?;
    for item in &mut out {
        *item = overlay_runtime_state_on_server(item.clone());
    }
    Ok(out)
}

#[tauri::command]
fn mcp_validate_definition(
    input: McpDefinitionValidateInput,
) -> Result<McpDefinitionValidateResult, String> {
    mcp_validate_definition_inner(input)
}

fn mcp_validate_definition_inner(
    input: McpDefinitionValidateInput,
) -> Result<McpDefinitionValidateResult, String> {
    let _schema = mcp_definition_json_schema();
    let (servers, issues) = validate_mcp_definition_servers(&input.definition_json);
    let server_count = servers.len();
    let first_transport = servers
        .first()
        .and_then(|(name, obj)| {
            parse_mcp_server_definition_from_value(name, obj)
                .ok()
                .map(|parsed| parsed.transport.as_str().to_string())
        });
    let first_name = servers.first().map(|(name, _)| name.clone());

    if issues.is_empty() {
        Ok(McpDefinitionValidateResult {
            ok: true,
            transport: first_transport,
            server_name: first_name,
            message: format!("MCP definition is valid ({server_count} server(s))"),
            schema_version: None,
            error_code: None,
            details: Vec::new(),
            issues: Vec::new(),
            migrated_definition_json: None,
        })
    } else {
        Ok(McpDefinitionValidateResult {
            ok: false,
            transport: None,
            server_name: None,
            message: "MCP definition does not match required schema".to_string(),
            schema_version: None,
            error_code: Some("schema_validation_failed".to_string()),
            details: issues.iter().map(|i| i.message.clone()).collect(),
            issues,
            migrated_definition_json: None,
        })
    }
}

#[tauri::command]
fn mcp_save_server(
    input: McpServerInput,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    mcp_save_server_inner(input, state.inner())
}

fn mcp_save_server_inner(
    input: McpServerInput,
    state: &AppState,
) -> Result<McpServerConfig, String> {
    let next = normalize_mcp_server_input(input)?;
    save_workspace_mcp_server(state, &next)?;
    let mut saved = load_server_by_id(state, &next.id)?;
    saved = overlay_runtime_state_on_server(saved);

    Ok(saved)
}

#[tauri::command]
async fn mcp_remove_server(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    mcp_remove_server_inner(input, state.inner()).await
}

async fn mcp_remove_server_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<bool, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    let removed = remove_workspace_mcp_server(state, server_id)?;
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
    mcp_list_server_tools_inner(input, state.inner()).await
}

async fn mcp_list_server_tools_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let server = {
        let server = load_server_by_id(state, server_id)?;
        server
    };

    let started = std::time::Instant::now();
    mcp_runtime_state_mark_starting(&server);
    let tools = match mcp_list_server_tools_runtime(&server).await {
        Ok(tools) => tools,
        Err(err) => {
            if let Some(challenge) = mcp_check_and_extract_auth_challenge(&err) {
                let _ = set_workspace_mcp_policy_oauth_capable(state, &server.id, true);
                let challenge_trimmed = challenge.trim();
                if !challenge_trimmed.is_empty() {
                    mcp_oauth_set_cached_challenge(&server.id, challenge_trimmed);
                }
                mcp_runtime_state_mark_probe_failure(&server, "auth_required", "需要 OAuth 授权登录");
                return Err("需要 OAuth 授权登录".to_string());
            }
            let status = mcp_status_from_runtime_error(&err);
            // 同上：非挑战的失败要清陈旧标记，timeout 与真挑战都不清。
            if mcp_stale_oauth_mark_should_clear_after_failure(
                status,
                server.oauth_capable,
                mcp_oauth_has_credentials(state, &server.id),
            ) {
                let _ = mcp_clear_stale_oauth_capable_mark(state, &server.id);
            }
            mcp_runtime_state_mark_probe_failure(&server, status, &err);
            return Err(err);
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
    // 列工具成功说明这个服务当前不需要 OAuth：清掉历史误判留下的标记（有凭证的除外）。
    if mcp_stale_oauth_mark_should_clear(
        server.oauth_capable,
        mcp_oauth_has_credentials(state, &server.id),
    ) {
        let _ = mcp_clear_stale_oauth_capable_mark(state, &server.id);
    }
    mcp_runtime_state_set(&server.id, true, "ready", "", final_tools.clone());
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);

    Ok(McpListServerToolsResult {
        server_id: server.id,
        tools: final_tools,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
fn mcp_list_server_tools_cached(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpListServerToolsResult, String> {
    mcp_list_server_tools_cached_inner(input, state.inner())
}

fn mcp_list_server_tools_cached_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let server = {
        let server = load_server_by_id(state, server_id)?;
        server
    };

    let started = std::time::Instant::now();
    let tools = list_tools_from_runtime(&server);

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
    mcp_deploy_server_inner(input, state.inner()).await
}

async fn mcp_deploy_server_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<McpListServerToolsResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }

    let server = {
        let server = load_server_by_id(state, server_id)?;
        set_workspace_mcp_policy_enabled(state, server_id, true)?;
        server
    };

    let started = std::time::Instant::now();
    mcp_runtime_state_mark_starting(&server);
    mcp_start_supervisor_probe_for_server(state.clone(), server.clone(), "manual_deploy");
    let final_tools = list_tools_from_runtime(&server);
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
    mcp_undeploy_server_inner(input, state.inner()).await
}

async fn mcp_undeploy_server_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<McpServerConfig, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    {
        let _ = load_server_by_id(state, server_id)?;
        set_workspace_mcp_policy_enabled(state, server_id, false)?;
    }
    mcp_disconnect_cached_client(server_id).await;
    mcp_runtime_state_set(server_id, false, "stopped", "", Vec::new());

    let mut out = load_server_by_id(state, server_id)?;
    out = overlay_runtime_state_on_server(out);
    Ok(out)
}

#[tauri::command]
fn mcp_set_tool_enabled(
    input: McpSetToolEnabledInput,
    state: State<'_, AppState>,
) -> Result<McpServerConfig, String> {
    mcp_set_tool_enabled_inner(input, state.inner())
}

fn mcp_set_tool_enabled_inner(
    input: McpSetToolEnabledInput,
    state: &AppState,
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
        let _ = load_server_by_id(state, server_id)?;
        let mut policies = load_workspace_mcp_tool_policies(state, server_id)?;
        if let Some(policy) = policies.iter_mut().find(|p| p.tool_name == tool_name) {
            policy.enabled = input.enabled;
        } else {
            policies.push(McpToolPolicy {
                tool_name: tool_name.to_string(),
                enabled: input.enabled,
            });
        }
        save_workspace_mcp_tool_policies(state, server_id, &policies)?;
        policies
    };

    mcp_runtime_state_set_tool_enabled(server_id, tool_name, input.enabled);

    let mut server = load_server_by_id(state, server_id)?;
    server.tool_policies = policies;
    server = overlay_runtime_state_on_server(server);

    Ok(server)
}

#[tauri::command]
fn mcp_open_workspace_dir(state: State<'_, AppState>) -> Result<String, String> {
    open_mcp_workspace_dir(&state)
}

// ========== AI 修复 MCP 格式（专家模型 + 脱敏还原） ==========

const MCP_FIX_REDACTED_PREFIX: &str = "__PAI_REDACTED_";

/// 把 definitionJson 中敏感字段值替换为占位符，返回 (脱敏文本, 占位符→原值映射)
fn redact_mcp_definition_sensitive_values(definition_json: &str) -> (String, Vec<(String, String)>) {
    let mut mapping = Vec::<(String, String)>::new();
    let Ok(mut value) = serde_json::from_str::<Value>(definition_json) else {
        return (definition_json.to_string(), mapping);
    };
    let mut counter = 0usize;
    redact_mcp_value(&mut value, &mut counter, &mut mapping);
    let text = serde_json::to_string(&value).unwrap_or_else(|_| definition_json.to_string());
    (text, mapping)
}

fn redact_mcp_value(
    value: &mut Value,
    counter: &mut usize,
    mapping: &mut Vec<(String, String)>,
) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    for key in ["env", "headers", "httpHeaders"] {
        if let Some(map) = obj.get_mut(key).and_then(Value::as_object_mut) {
            for v in map.values_mut() {
                let Some(text) = v.as_str() else {
                    continue;
                };
                if text.starts_with(MCP_FIX_REDACTED_PREFIX) {
                    continue;
                }
                *counter += 1;
                let placeholder = format!("{MCP_FIX_REDACTED_PREFIX}{counter}");
                mapping.push((placeholder.clone(), text.to_string()));
                *v = Value::String(placeholder);
            }
        }
    }
}

fn restore_mcp_definition_sensitive_values(
    fixed_json: &str,
    mapping: &[(String, String)],
) -> String {
    let mut text = fixed_json.to_string();
    for (placeholder, original) in mapping {
        text = text.replace(placeholder, original);
    }
    text
}

fn build_mcp_fix_prompt(definition_json: &str, issues: &[McpValidationIssue]) -> String {
    let mut issue_lines = String::new();
    for issue in issues {
        let server = issue
            .server_name
            .as_deref()
            .map(|name| format!("[{name}] "))
            .unwrap_or_default();
        issue_lines.push_str(&format!("- {server}{} ({})\n", issue.message, issue.code));
    }
    if issue_lines.is_empty() {
        issue_lines.push_str("- 无\n");
    }
    format!(
        "你是 MCP 配置修复专家。下面是用户粘贴的 MCP 服务器配置 JSON 与校验错误列表。\n\
         请修复该 JSON，使其成为合法的 MCP 配置（支持 mcpServers 对象、平铺命名对象、数组等任意常见格式）。\n\
         要求：\n\
         1. 只修复格式与结构问题，不要更改服务器名称、command、url、args、env 等字段的值\n\
         2. 不要新增或删除服务器，不要改变服务器数量\n\
         3. 以 __PAI_REDACTED_ 开头的值是敏感占位符，必须原样保留，不要改动\n\
         4. 输出格式必须是 JSON 对象：{{\"definition\": <修复后的完整 MCP 配置 JSON>}}\n\n\
         原始 JSON：\n{definition_json}\n\n\
         校验错误：\n{issue_lines}"
    )
}

#[tauri::command]
async fn mcp_fix_definition(
    input: McpFixDefinitionInput,
    state: State<'_, AppState>,
) -> Result<McpFixDefinitionResult, String> {
    mcp_fix_definition_inner(input, state.inner()).await
}

async fn mcp_fix_definition_inner(
    input: McpFixDefinitionInput,
    state: &AppState,
) -> Result<McpFixDefinitionResult, String> {
    let definition_json = input.definition_json.trim().to_string();
    if definition_json.is_empty() {
        return Err("MCP definition JSON is required".to_string());
    }
    let (_, current_issues) = validate_mcp_definition_servers(&definition_json);
    if current_issues.is_empty() {
        return Ok(McpFixDefinitionResult {
            ok: true,
            fixed_definition_json: Some(definition_json),
            message: "配置已合法，无需修复".to_string(),
            issues: Vec::new(),
            model_name: None,
        });
    }

    let (redacted_json, mapping) = redact_mcp_definition_sensitive_values(&definition_json);
    let prompt = build_mcp_fix_prompt(&redacted_json, &current_issues);
    let output = invoke_expert_model_json_result(
        state,
        "mcp_fix_definition",
        &prompt,
        Some(MCP_REQUEST_TIMEOUT_SECS),
        &["definition"],
        &[],
    )
    .await
    .map_err(|err| format!("AI 修复请求失败: {}", err.message))?;

    let definition_value = output
        .value
        .get("definition")
        .cloned()
        .ok_or_else(|| "AI 修复结果缺少 definition 字段".to_string())?;
    let fixed_raw = match definition_value {
        Value::String(text) => text,
        other => serde_json::to_string(&other)
            .map_err(|err| format!("序列化 AI 修复结果失败: {err}"))?,
    };
    let fixed_json = restore_mcp_definition_sensitive_values(&fixed_raw, &mapping);

    // 修复结果必须可解析且校验通过
    let (_, fixed_issues) = validate_mcp_definition_servers(&fixed_json);
    if !fixed_issues.is_empty() {
        return Ok(McpFixDefinitionResult {
            ok: false,
            fixed_definition_json: Some(fixed_json),
            message: format!("AI 修复完成但仍有问题（模型：{}）", output.model_name),
            issues: fixed_issues,
            model_name: Some(output.model_name),
        });
    }

    Ok(McpFixDefinitionResult {
        ok: true,
        fixed_definition_json: Some(fixed_json),
        message: format!("AI 修复完成（模型：{}）", output.model_name),
        issues: Vec::new(),
        model_name: Some(output.model_name),
    })
}

#[tauri::command]
async fn mcp_oauth_login(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpOAuthStatusResult, String> {
    mcp_oauth_login_inner(input, state.inner()).await
}

async fn mcp_oauth_login_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<McpOAuthStatusResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    mcp_oauth_start_login(state, server_id).await
}

#[tauri::command]
fn mcp_oauth_cancel(input: McpServerIdInput) -> Result<bool, String> {
    mcp_oauth_cancel_inner(input)
}

fn mcp_oauth_cancel_inner(input: McpServerIdInput) -> Result<bool, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    Ok(mcp_oauth_cancel_login_session(server_id))
}

#[tauri::command]
fn mcp_oauth_status(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<McpOAuthStatusResult, String> {
    mcp_oauth_status_inner(input, state.inner())
}

fn mcp_oauth_status_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<McpOAuthStatusResult, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    let has_credentials = mcp_oauth_has_credentials(state, server_id);
    Ok(mcp_oauth_get_session_status(server_id, has_credentials))
}

#[tauri::command]
async fn mcp_oauth_clear_credentials(
    input: McpServerIdInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    mcp_oauth_clear_credentials_inner(input, state.inner()).await
}

async fn mcp_oauth_clear_credentials_inner(
    input: McpServerIdInput,
    state: &AppState,
) -> Result<bool, String> {
    let server_id = input.server_id.trim();
    if server_id.is_empty() {
        return Err("serverId is required".to_string());
    }
    let deleted = mcp_oauth_delete_credential(state, server_id)?;
    mcp_oauth_clear_cached_challenge(server_id);
    mcp_disconnect_cached_client(server_id).await;
    // 保留部署：policy.enabled 不变，运行时标记为待授权。
    // 重启后监管探测同样会因缺少凭据得到 auth_required，与重启前 UI 一致。
    mcp_runtime_state_set(server_id, true, "auth_required", "需要 OAuth 授权登录", Vec::new());
    runtime_log_info(format!("[MCP OAuth] 清除凭据完成 server_id={}", server_id));
    Ok(deleted)
}
