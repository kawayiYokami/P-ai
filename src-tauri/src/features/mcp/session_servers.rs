// ==================== 会话级 MCP 服务器 ====================
//
// 由 ACP 客户端在 `session/new` / `session/load` 里带入的 MCP 服务器。
// 只活在内存里：`llm-workspace/mcp/servers/` 是持久全局目录，写进去会污染
// 其他会话，编辑器关掉之后还留在那里。
//
// 装配完全复用现成那条路径：`load_workspace_mcp_servers` 末尾把这张表追加进去，
// 于是工具 schema 缓存、目录块、get_tool、执行器照旧工作，不需要为 ACP 单开分支。

fn session_mcp_store() -> &'static Mutex<Vec<McpServerConfig>> {
    static STORE: OnceLock<Mutex<Vec<McpServerConfig>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// 当前进程内存里挂着的会话级 MCP 服务器。
fn session_mcp_servers_snapshot() -> Vec<McpServerConfig> {
    match session_mcp_store().lock() {
        Ok(guard) => guard.clone(),
        Err(err) => {
            runtime_log_warn(format!("[会话级MCP] 读取失败，锁已损坏：{err}"));
            Vec::new()
        }
    }
}

/// 挂上一批会话级服务器：连上、列出工具、写进运行态，再刷新工具 schema。
///
/// 单个服务器失败不影响其他服务器：编辑器带来的一条坏定义不该让整场会话开不起来。
/// 失败原因原样返回给调用方记日志，不静默吞掉。
async fn session_mcp_attach(state: &AppState, servers: Vec<McpServerConfig>) -> Vec<String> {
    let mut errors = Vec::<String>::new();
    let mut attached = Vec::<McpServerConfig>::new();
    for server in servers {
        let started = std::time::Instant::now();
        match mcp_list_server_tools_runtime(&server).await {
            Ok(tools) => {
                runtime_log_info(format!(
                    "[会话级MCP] 完成，任务=挂载服务器，server_id={}，name={}，tools={}，elapsed_ms={}",
                    server.id,
                    server.name,
                    tools.len(),
                    started.elapsed().as_millis()
                ));
                mcp_runtime_state_set(&server.id, true, "ready", "", tools);
                attached.push(server);
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[会话级MCP] 失败，任务=挂载服务器，server_id={}，name={}，error={err}",
                    server.id, server.name
                ));
                errors.push(format!("{}: {err}", server.name.trim()));
            }
        }
    }
    if !attached.is_empty() {
        match session_mcp_store().lock() {
            Ok(mut guard) => guard.extend(attached),
            Err(err) => runtime_log_error(format!(
                "[会话级MCP] 写入失败，任务=登记服务器，原因=锁已损坏：{err}"
            )),
        }
        refresh_global_tool_schema_cache(state);
    }
    errors
}

/// 收掉全部会话级服务器：断开客户端、清运行态、清表。
///
/// ACP 连接结束（编辑器关掉 agent 进程）时调用，避免留下孤儿进程。
async fn session_mcp_dispose_all(state: &AppState) {
    let servers = match session_mcp_store().lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(err) => {
            runtime_log_error(format!(
                "[会话级MCP] 清理失败，任务=读取服务器表，原因=锁已损坏：{err}"
            ));
            return;
        }
    };
    if servers.is_empty() {
        return;
    }
    for server in &servers {
        mcp_disconnect_cached_client(&server.id).await;
        mcp_runtime_state_set(&server.id, false, "stopped", "", Vec::new());
    }
    runtime_log_info(format!(
        "[会话级MCP] 完成，任务=清理服务器，count={}",
        servers.len()
    ));
    refresh_global_tool_schema_cache(state);
}
