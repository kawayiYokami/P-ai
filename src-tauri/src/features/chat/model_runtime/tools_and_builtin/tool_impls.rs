#[derive(Debug, Clone)]
struct BuiltinFetchTool {
    app_state: AppState,
}

impl RuntimeToolMetadata for BuiltinFetchTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "fetch",
            "静态网页抓取工具。抓取网页内容并提取正文。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "url": { "type": "string", "description": "要抓取的网页地址" },
                "max_length": { "type": "integer", "description": "返回内容的最大字符数", "default": 1800 }
              },
              "required": ["url"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinFetchTool {
    const NAME: &'static str = "fetch";
    type Args = FetchToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=fetch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        let result = builtin_fetch(&self.app_state, &args.url, args.max_length.unwrap_or(1800))
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=fetch result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 fetch 执行失败: 错误={err}")),
        }
        result
        })
    }
}

#[derive(Debug, Clone)]
struct BuiltinBingSearchTool {
    app_state: AppState,
}

impl RuntimeToolMetadata for BuiltinBingSearchTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "websearch",
            "搜索互联网内容。优先使用其他可用的联网搜索或抓取工具；仅在没有其他网络搜索或抓取能力时再使用。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "搜索关键词或问题" }
              },
              "required": ["query"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinBingSearchTool {
    const NAME: &'static str = "websearch";
    type Args = BingSearchToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=websearch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        let result = builtin_bing_search(&self.app_state, &args.query)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=websearch result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => {
                runtime_log_error(format!("[工具执行] 内置工具 websearch 执行失败: 错误={err}"))
            }
        }
        result
        })
    }
}

#[derive(Debug, Clone)]
struct BuiltinRememberTool {
    app_state: AppState,
    memory_context: MemoryAgentContext,
}

impl RuntimeToolMetadata for BuiltinRememberTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "remember",
            "保存与用户相关、长期有价值的记忆。禁止保存密码、密钥等敏感信息。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "action": {
                  "type": "string",
                  "enum": ["create", "update", "merge"],
                  "description": "记忆动作。create=新增一条记忆；update=更新一条已有记忆；merge=把多条旧记忆合并为一条新记忆。"
                },
                "sourceMemoryIds": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "源记忆 ID，使用 recall 记忆板里的短编号。create 必须传空数组或省略；update 必须正好 1 个；merge 至少 2 个。"
                },
                "memory": {
                  "type": "object",
                  "description": "目标记忆内容。create 时是新记忆；update 时是 sourceMemoryIds[0] 的新版本；merge 时是多条源记忆合并后的结果。",
                  "properties": {
                    "memoryType": {
                      "type": "string",
                      "enum": ["knowledge", "skill", "emotion", "event"],
                      "description": "记忆类型。knowledge=稳定认知或事实，skill=做事方法或能力，emotion=稳定情绪偏好或态度，event=发生过的事件。"
                    },
                    "judgment": {
                      "type": "string",
                      "description": "记忆本体。用一句独立、清楚、可检索的判断句写出真正要记住的内容。"
                    },
                    "reasoning": {
                      "type": "string",
                      "description": "支撑 judgment 的依据或背景，可为空。只写理由、证据、来源，不要写流程话术。"
                    },
                    "tags": {
                      "type": "array",
                      "items": { "type": "string" },
                      "description": "检索锚点列表，用于后续命中提示板。每一项都必须是独立、紧凑、稳定、可检索的词元；不要写整句，不要写短语拼接，不要把多个语义塞进同一个 tag。"
                    }
                  },
                  "required": ["memoryType", "judgment", "tags"]
                }
              },
              "required": ["action", "memory"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinRememberTool {
    const NAME: &'static str = "remember";
    type Args = MemorySaveToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(3))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        let args_json = serde_json::json!({
            "action": args.action,
            "sourceMemoryIds": args.source_memory_ids,
            "memory": args.memory,
        });
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=remember args={}",
            debug_value_snippet(&args_json, 240)
        ));
        let result = builtin_memory_save(&self.app_state, &self.memory_context, args_json)
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=remember result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => {
                runtime_log_error(format!("[工具执行] 内置工具 remember 执行失败: 错误={err}"))
            }
        }
        result
        })
    }
}

#[derive(Debug, Clone)]
struct BuiltinRecallTool {
    app_state: AppState,
    memory_context: MemoryAgentContext,
}

impl RuntimeToolMetadata for BuiltinRecallTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "recall",
            "回忆记忆，并返回可直接注入提示词的记忆板。query 和 time 可选；结果应用 offset/limit 分页。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "可选的回忆查询文本；不传或为空时返回全部可见记忆。传入时先按 query 过滤相关记忆。" },
                "time": { "type": "string", "description": "可选的时间过滤。传 YYYY 表示该年，传 YYYY-MM 表示该月，传 YYYY-MM-DD 表示该日。" },
                "offset": { "type": "integer", "minimum": 0, "description": "跳过多少条结果。默认 0。" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50, "description": "返回多少条结果。默认 7，最大 50。" }
              },
              "required": []
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinRecallTool {
    const NAME: &'static str = "recall";
    type Args = RecallToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=recall args={}",
            debug_value_snippet(&args_json, 240)
        ));
        let result = builtin_recall(
            &self.app_state,
            &self.memory_context,
            args.query.as_deref().unwrap_or(""),
            args.time.as_deref(),
            args.offset,
            args.limit,
        )
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=recall result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 recall 执行失败: 错误={err}")),
        }
        result
        })
    }
}

#[derive(Debug, Clone)]
struct BuiltinTerminalExecTool {
    app_state: AppState,
    session_id: String,
    executor_agent_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinConfigTool {
    app_state: AppState,
}

#[derive(Debug, Clone)]
enum ConfigToolRuntimeEffect {
    None,
    McpDeploy { server_id: String },
    McpUndeploy { server_id: String },
    McpRemove { server_id: String },
    McpRestartIfEnabled { server_id: String },
    WorkspaceReload { reason: &'static str },
}

fn invalidate_config_tool_runtime_caches(state: &AppState) -> Result<(), String> {
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = None;
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = None;
    *state
        .cached_agents
        .lock()
        .map_err(|_| "Failed to lock cached agents".to_string())? = None;
    *state
        .cached_agents_mtime
        .lock()
        .map_err(|_| "Failed to lock cached agents mtime".to_string())? = None;
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = None;
    *state
        .cached_app_data_signature
        .lock()
        .map_err(|_| "Failed to lock cached app data signature".to_string())? = None;
    clear_terminal_config_allowed_workspaces_cache_for_state(state);
    clear_global_tool_schema_cache();
    if let Err(err) = clear_hidden_skill_snapshot_cache(state) {
        runtime_log_warn(format!("[config工具] 清空技能快照缓存失败: {err}"));
    }
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);
    Ok(())
}

fn config_tool_split_command(command: &str) -> Vec<String> {
    pai_config_tool::split_command_line(command).unwrap_or_default()
}

/// 判断 config 命令是否只读查询命令（不产生配置改动副作用）。
/// 保守策略：只把明确的查询命令（help / ls / get / example）判为只读，其余一律视为写命令，
/// 宁可多重建缓存，不漏重建。
fn config_tool_command_is_readonly(command: &str) -> bool {
    let parts = config_tool_split_command(command);
    match parts.first().map(String::as_str) {
        Some("help") | Some("--help") | Some("-h") | Some("approot") => true,
        Some("store") => store_command_is_readonly(&parts[1..]),
        Some(_) => matches!(
            parts.get(1).map(String::as_str),
            Some("ls") | Some("get") | Some("example") | Some("tree")
        ),
        None => false,
    }
}

fn config_tool_resolve_mcp_server_id(state: &AppState, selector: &str) -> String {
    load_workspace_mcp_servers(state)
        .ok()
        .and_then(|servers| {
            servers.into_iter().find_map(|server| {
                if server.id == selector || server.name == selector {
                    Some(server.id)
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| selector.to_string())
}

fn config_tool_runtime_effect_for_command(
    state: &AppState,
    command: &str,
) -> ConfigToolRuntimeEffect {
    let parts = config_tool_split_command(command);
    match (
        parts.first().map(String::as_str),
        parts.get(1).map(String::as_str),
    ) {
        (Some("mcp"), Some("enable")) => parts
            .get(2)
            .map(|selector| ConfigToolRuntimeEffect::McpDeploy {
                server_id: config_tool_resolve_mcp_server_id(state, selector),
            })
            .unwrap_or(ConfigToolRuntimeEffect::None),
        (Some("mcp"), Some("disable")) => parts
            .get(2)
            .map(|selector| ConfigToolRuntimeEffect::McpUndeploy {
                server_id: config_tool_resolve_mcp_server_id(state, selector),
            })
            .unwrap_or(ConfigToolRuntimeEffect::None),
        (Some("mcp"), Some("delete")) => parts
            .get(2)
            .map(|selector| ConfigToolRuntimeEffect::McpRemove {
                server_id: config_tool_resolve_mcp_server_id(state, selector),
            })
            .unwrap_or(ConfigToolRuntimeEffect::None),
        (Some("mcp"), Some("update")) => parts
            .get(2)
            .map(|selector| ConfigToolRuntimeEffect::McpRestartIfEnabled {
                server_id: config_tool_resolve_mcp_server_id(state, selector),
            })
            .unwrap_or(ConfigToolRuntimeEffect::None),
        (Some("skill"), Some("update")) => ConfigToolRuntimeEffect::WorkspaceReload {
            reason: "skill_update",
        },
        (Some("skill"), Some("delete")) => ConfigToolRuntimeEffect::WorkspaceReload {
            reason: "skill_delete",
        },
        _ => ConfigToolRuntimeEffect::None,
    }
}

fn config_tool_mcp_start_server(
    state: &AppState,
    server_id: &str,
    trigger: &'static str,
) -> Result<Value, String> {
    let server = load_server_by_id(state, server_id)?;
    set_workspace_mcp_policy_enabled(state, server_id, true)?;
    mcp_runtime_state_mark_starting(&server);
    mcp_start_supervisor_probe_for_server(state.clone(), server.clone(), trigger);
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);
    Ok(serde_json::json!({
        "type": "mcpDeploy",
        "serverId": server.id,
        "status": "starting"
    }))
}

async fn config_tool_mcp_stop_server(
    state: &AppState,
    server_id: &str,
) -> Result<Value, String> {
    let server = load_server_by_id(state, server_id)?;
    set_workspace_mcp_policy_enabled(state, server_id, false)?;
    mcp_disconnect_cached_client(server_id).await;
    mcp_runtime_state_set(server_id, false, "stopped", "", Vec::new());
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);
    Ok(serde_json::json!({
        "type": "mcpUndeploy",
        "serverId": server.id,
        "status": "stopped"
    }))
}

async fn config_tool_mcp_remove_runtime(
    state: &AppState,
    server_id: &str,
) -> Result<Value, String> {
    mcp_disconnect_cached_client(server_id).await;
    mcp_runtime_state_remove(server_id);
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);
    Ok(serde_json::json!({
        "type": "mcpRemove",
        "serverId": server_id,
        "status": "removed"
    }))
}

async fn config_tool_mcp_restart_if_enabled(
    state: &AppState,
    server_id: &str,
) -> Result<Value, String> {
    let server = load_server_by_id(state, server_id)?;
    mcp_disconnect_cached_client(server_id).await;
    if server.enabled {
        mcp_runtime_state_mark_starting(&server);
        mcp_start_supervisor_probe_for_server(state.clone(), server.clone(), "config_tool_update");
        refresh_global_tool_schema_cache(state);
        mark_prompt_cache_rebuild_for_all_final_system_sources(state);
        Ok(serde_json::json!({
            "type": "mcpRestart",
            "serverId": server.id,
            "status": "starting"
        }))
    } else {
        mcp_runtime_state_set(server_id, false, "disabled", "", Vec::new());
        refresh_global_tool_schema_cache(state);
        mark_prompt_cache_rebuild_for_all_final_system_sources(state);
        Ok(serde_json::json!({
            "type": "mcpRestart",
            "serverId": server.id,
            "status": "disabled"
        }))
    }
}

async fn apply_config_tool_runtime_effect(
    state: &AppState,
    effect: ConfigToolRuntimeEffect,
) -> Result<Option<Value>, String> {
    match effect {
        ConfigToolRuntimeEffect::None => Ok(None),
        ConfigToolRuntimeEffect::McpDeploy { server_id } => {
            config_tool_mcp_start_server(state, &server_id, "config_tool_enable").map(Some)
        }
        ConfigToolRuntimeEffect::McpUndeploy { server_id } => {
            config_tool_mcp_stop_server(state, &server_id).await.map(Some)
        }
        ConfigToolRuntimeEffect::McpRemove { server_id } => {
            config_tool_mcp_remove_runtime(state, &server_id)
                .await
                .map(Some)
        }
        ConfigToolRuntimeEffect::McpRestartIfEnabled { server_id } => {
            config_tool_mcp_restart_if_enabled(state, &server_id)
                .await
                .map(Some)
        }
        ConfigToolRuntimeEffect::WorkspaceReload { reason } => {
            let reload_result = reload_workspace(state).await?;
            log_workspace_load_result("[config工具][reload]", &reload_result);
            Ok(Some(serde_json::json!({
                "type": "workspaceReload",
                "reason": reason,
                "ok": reload_result.ok,
                "status": reload_result.status,
                "mcpLoaded": reload_result.mcp_loaded,
                "mcpFailed": reload_result.mcp_failed,
                "skillsLoaded": reload_result.skills_loaded,
                "skillsFailed": reload_result.skills_failed,
                "privateAgentsLoaded": reload_result.private_agents_loaded,
                "privateAgentsFailed": reload_result.private_agents_failed,
                "loadedSummary": reload_result.loaded_summary,
                "failedSummary": reload_result.failed_summary,
                "repairSummary": reload_result.repair_summary,
                "repairItems": reload_result.repair_items,
                "needsRepair": reload_result.needs_repair
            })))
        }
    }
}

impl RuntimeToolMetadata for BuiltinTerminalExecTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "exec",
            terminal_exec_tool_description(&terminal_shell_for_state(&self.app_state)),
            serde_json::json!({
              "type": "object",
              "properties": {
                "command": { "type": "string", "description": "要执行的一次性 shell 命令。" },
                "description": { "type": "string", "description": "必填，本次命令的简短说明，用于向用户解释执行意图并在权限审批卡中呈现；后台任务也会用于列表展示与完成写回。" },
                "mode": { "type": "string", "enum": ["wait", "background"], "default": "wait", "description": "执行模式。wait=当前调用等待结果；background=立即返回后台任务 id。" },
                "timeout_ms": { "type": "integer", "minimum": 1, "default": 300000, "description": "命令超时时间，单位毫秒；未指定时默认 300000ms，超时后回收本次进程树。长耗时检查/构建应显式传入足够大的值。" },
                "commitment": { "type": "string", "description": "危险命令确认承诺。平时留空；仅当 exec 返回 blockedReason=local_rule_blocked 且 message 要求确认时，向用户说明危险性并取得明确许可后，填入返回中的 commitmentHint 文案再重新调用。" }
              },
              "required": ["command", "description"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeToolMetadata for BuiltinConfigTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "config",
            "这是 PAI 配置工具。当用户要求你修改 PAI 的设置时使用，例如人格、组织树、MCP、Skill。入参只有 command:string。使用方法：先调用 `help`，工具会返回类似 shell help 的命令指南；再按指南逐条执行查看、生成样例、检查、预览差异或更新配置。当前不开放供应商配置命令。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "command": { "type": "string", "description": "要执行的一条 config 命令字符串。" }
              },
              "required": ["command"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinConfigTool {
    const NAME: &'static str = "config";
    type Args = ConfigToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(30))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let args_value = serde_json::to_value(&args).unwrap_or(Value::Null);
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=config args={}",
                debug_value_snippet(&args_value, 240)
            ));
            let runtime_effect =
                config_tool_runtime_effect_for_command(&self.app_state, &args.command);
            let command_parts = config_tool_split_command(&args.command);
            // 商店命令要联网、要读运行态缓存，交给能力商店的异步实现；其余命令仍走同步的配置工具。
            let output = if command_parts.first().map(String::as_str) == Some("store") {
                run_store_command(&self.app_state, &command_parts[1..])
                    .await
                    .map_err(ToolInvokeError::from)?
            } else {
                pai_config_tool::run_command_with_paths(
                    self.app_state.config_path.clone(),
                    self.app_state.data_path.clone(),
                    configured_workspace_root_path(&self.app_state)
                        .unwrap_or_else(|_| self.app_state.llm_workspace_path.clone()),
                    &args.command,
                )
                .map_err(ToolInvokeError::from)?
            };
            if !config_tool_command_is_readonly(&args.command) {
                invalidate_config_tool_runtime_caches(&self.app_state).map_err(ToolInvokeError::from)?;
            }
            let runtime_effect = apply_config_tool_runtime_effect(&self.app_state, runtime_effect)
                .await
                .map_err(ToolInvokeError::from)?;
            let mut result = match serde_json::from_str::<Value>(output.trim()) {
                Ok(parsed) => serde_json::json!({
                    "ok": true,
                    "command": args.command,
                    "result": parsed
                }),
                Err(_) => serde_json::json!({
                    "ok": true,
                    "command": args.command,
                    "result": output
                }),
            };
            if let Some(runtime_effect) = runtime_effect {
                if let Some(object) = result.as_object_mut() {
                    object.insert("runtimeEffect".to_string(), runtime_effect);
                }
            }
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=config result={}",
                debug_value_snippet(&result, 240)
            ));
            Ok(result)
        })
    }
}

impl RuntimeValueTool for BuiltinTerminalExecTool {
    const NAME: &'static str = "exec";
    type Args = TerminalExecToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        // `exec` may wait indefinitely for explicit user approval before the
        // command is allowed to start. The real process timeout is enforced
        // inside `builtin_shell_exec`; adding another runtime-level timeout
        // here would let approval wait expire and cause repeated re-prompts.
        None
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=exec args={}",
            debug_value_snippet(&args_json, 240)
        ));
        let resolved_action = args
            .action
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("run");
        let resolved_mode = args
            .mode
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("wait");
        let resolved_command = args.command.as_deref().map(str::trim).unwrap_or("");
        if resolved_action == "run" && resolved_command.is_empty() {
            return Err(ToolInvokeError::from("exec.command is required".to_string()));
        }
        ensure_saddler_exec_allowed(
            &self.app_state,
            &self.session_id,
            &self.executor_agent_id,
            resolved_command,
        )
        .map_err(ToolInvokeError::from)?;
        let resolved_description = args
            .description
            .as_deref()
            .map(str::trim)
            .unwrap_or("");
        let result = builtin_shell_exec(
            &self.app_state,
            &self.session_id,
            resolved_action,
            resolved_mode,
            resolved_command,
            resolved_description,
            args.timeout_ms,
            args.commitment.as_deref(),
        )
        .await
        .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => {
                runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=exec result={}",
                    debug_value_snippet(v, 240)
                ));
                runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=exec summary={}",
                    debug_exec_result_summary(v)
                ));
            }
            Err(err) => {
                runtime_log_error(format!("[工具执行] 内置工具 exec 执行失败: 错误={err}"))
            }
        }
        result
        })
    }
}

#[derive(Debug, Clone)]
struct BuiltinWriteFileTool {
    app_state: AppState,
    session_id: String,
    executor_agent_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinDeleteFileTool {
    app_state: AppState,
    session_id: String,
    executor_agent_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinUpdateFileTool {
    app_state: AppState,
    session_id: String,
    executor_agent_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinMoveFileTool {
    app_state: AppState,
    session_id: String,
    executor_agent_id: String,
}

fn path_is_within_directory(path: &std::path::Path, directory: &std::path::Path) -> bool {
    let normalized_path = terminal_normalize_for_access_check(path);
    let normalized_directory = terminal_normalize_for_access_check(directory);
    normalized_path == normalized_directory || normalized_path.starts_with(normalized_directory)
}

fn ensure_saddler_file_target_allowed(
    state: &AppState,
    session_id: &str,
    executor_agent_id: &str,
    raw_path: &str,
) -> Result<(), String> {
    if executor_agent_id.trim() != SADDLER_AGENT_ID {
        return Ok(());
    }
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let cwd = resolve_terminal_cwd(state, &normalized_session, None)?;
    let target = apply_patch_resolve_path(&cwd, raw_path)?;
    let pai_dir = terminal_normalize_for_access_check(&cwd.join(".pai"));
    if path_is_within_directory(&target, &pai_dir) {
        Ok(())
    } else {
        Err("saddler 人格只能在当前项目 .pai/ 目录下写入或更新能力资产".to_string())
    }
}

fn ensure_saddler_exec_allowed(
    state: &AppState,
    session_id: &str,
    executor_agent_id: &str,
    command: &str,
) -> Result<(), String> {
    if executor_agent_id.trim() != SADDLER_AGENT_ID {
        return Ok(());
    }
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let cwd = resolve_terminal_cwd(state, &normalized_session, None)?;
    let runtime_shell = terminal_shell_for_state(state);
    let analysis = terminal_analyze_command(&cwd, command, &runtime_shell.kind);
    if terminal_command_is_read_whitelist(command, &runtime_shell.kind, &analysis) {
        return Ok(());
    }
    let pai_dir = terminal_normalize_for_access_check(&cwd.join(".pai"));
    let write_targets = analysis.write_target_paths();
    if write_targets.is_empty() {
        return Err("saddler 人格的 exec 只能执行只读命令，或写入目标明确位于当前项目 .pai/ 目录下的命令".to_string());
    }
    if write_targets
        .iter()
        .all(|path| path_is_within_directory(path, &pai_dir))
    {
        Ok(())
    } else {
        Err("saddler 人格的 exec 写入目标必须全部位于当前项目 .pai/ 目录下".to_string())
    }
}

impl RuntimeToolMetadata for BuiltinWriteFileTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "write",
            "新增文件或整写一个完整文件。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "path": { "type": "string", "description": "目标文件的绝对路径。" },
                "content": { "type": "string", "description": "要写入文件的完整内容。" },
                "overwrite": { "type": "boolean", "description": "是否允许覆盖已有文件。默认 false；只有在你明确要整文件替换已有内容时才设为 true。" }
              },
              "required": ["path", "content"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinWriteFileTool {
    const NAME: &'static str = "write";
    type Args = WriteFileToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=write args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        ensure_saddler_file_target_allowed(
            &self.app_state,
            &self.session_id,
            &self.executor_agent_id,
            &args.path,
        )
        .map_err(ToolInvokeError::from)?;
        let result = builtin_write_file(&self.app_state, &self.session_id, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=write result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 write 执行失败: 错误={err}")),
        }
        result
        })
    }
}

impl RuntimeToolMetadata for BuiltinDeleteFileTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "delete",
            "删除整个文件。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "path": { "type": "string", "description": "要删除文件的绝对路径。" }
              },
              "required": ["path"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinDeleteFileTool {
    const NAME: &'static str = "delete";
    type Args = DeleteFileToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=delete args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        ensure_saddler_file_target_allowed(
            &self.app_state,
            &self.session_id,
            &self.executor_agent_id,
            &args.path,
        )
        .map_err(ToolInvokeError::from)?;
        let result = builtin_delete_file(&self.app_state, &self.session_id, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=delete result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 delete 执行失败: 错误={err}")),
        }
        result
        })
    }
}

impl RuntimeToolMetadata for BuiltinUpdateFileTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "update",
            "修改已有文件中的局部内容，通过 oldString 做精确子串替换。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "path": { "type": "string", "description": "目标文件的绝对路径。" },
                "oldString": { "type": "string", "description": "原文件中要精确匹配的内容。" },
                "newString": { "type": "string", "description": "替换后的内容；传空字符串可删除旧内容。" },
                "replaceAll": { "type": "boolean", "description": "是否替换全部命中项；默认 false。" }
              },
              "required": ["path", "oldString", "newString"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinUpdateFileTool {
    const NAME: &'static str = "update";
    type Args = UpdateFileToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=update args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        ensure_saddler_file_target_allowed(
            &self.app_state,
            &self.session_id,
            &self.executor_agent_id,
            &args.path,
        )
        .map_err(ToolInvokeError::from)?;
        let result = builtin_update_file(&self.app_state, &self.session_id, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=update result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 update 执行失败: 错误={err}")),
        }
        result
        })
    }
}

impl RuntimeToolMetadata for BuiltinMoveFileTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "move",
            "移动或重命名整个文件。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "path": { "type": "string", "description": "原文件的绝对路径。" },
                "to": { "type": "string", "description": "目标文件的绝对路径。" }
              },
              "required": ["path", "to"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinMoveFileTool {
    const NAME: &'static str = "move";
    type Args = MoveFileToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=move args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        ensure_saddler_file_target_allowed(
            &self.app_state,
            &self.session_id,
            &self.executor_agent_id,
            &args.path,
        )
        .map_err(ToolInvokeError::from)?;
        ensure_saddler_file_target_allowed(
            &self.app_state,
            &self.session_id,
            &self.executor_agent_id,
            &args.to,
        )
        .map_err(ToolInvokeError::from)?;
        let result = builtin_move_file(&self.app_state, &self.session_id, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=move result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 move 执行失败: 错误={err}")),
        }
        result
        })
    }
}




#[derive(Debug, Clone)]
struct BuiltinPlanTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinTaskTool {
    app_state: AppState,
    session_id: String,
    api_config_id: String,
    executor_agent_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinTodoTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinCreateGoalTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinUpdateGoalTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinGetSessionTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinBackgroundTool {
    app_state: AppState,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinInformSessionTool {
    app_state: AppState,
    session_id: String,
}

impl RuntimeToolMetadata for BuiltinTodoTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        todo_provider_tool_definition()
    }
}

impl RuntimeToolDyn for BuiltinTodoTool {
    fn name(&self) -> String {
        TODO_TOOL_NAME.to_string()
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        Box::pin(async move {
            let args = parse_runtime_tool_args::<TodoWriteRequest>(&args_json)?;
            let args_value = serde_json::to_value(&args).unwrap_or(Value::Null);
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=todo args={}",
                debug_value_snippet(&args_value, 240)
            ));
            let result = builtin_todo(&self.app_state, &self.session_id, args)
                .map(ProviderToolResult::text);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=todo result={}",
                    debug_text_snippet(&v.output, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 todo 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinPlanTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "plan",
            plan_tool_description(),
            serde_json::json!({
              "type": "object",
              "properties": {
                "action": {
                  "type": "string",
                  "enum": ["present", "complete"],
                  "description": "present 表示提交计划；complete 表示标记该计划已完成"
                },
                "path": {
                  "type": "string",
                  "description": "计划 Markdown 文件路径"
                }
              },
              "required": ["action", "path"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinPlanTool {
    const NAME: &'static str = "plan";
    type Args = PlanToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=plan args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        let result = builtin_plan(&self.app_state, &self.session_id, args)
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=plan result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 plan 执行失败: 错误={err}")),
        }
        result
        })
    }
}

impl RuntimeToolMetadata for BuiltinTaskTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "task",
            "创建和管理会在未来按时间或周期自动触发委托的持久化定时任务。任务到点后会启动委托，委托完成后结果回到来源会话。调度时间、重复频率和结束时间写入 trigger；goal/why/todo 只描述触发后这一次要完成什么。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["list", "get", "create", "complete"], "description": "要执行的动作。" },
                "task_id": { "type": "string", "description": "任务 ID。get、complete 时必填。" },
                "goal": { "type": "string", "description": "任务到点后交给委托完成的单次目标，也是列表标题；只写要做什么，调度信息写入 trigger。" },
                "why": { "type": "string", "description": "为什么要做这件事，用来避免后续推进走偏；只写背景和原因，调度信息写入 trigger。" },
                "todo": { "type": "string", "description": "委托启动时要关注的下一步、范围边界或交付要求；只写本次触发后的执行动作，调度信息写入 trigger。" },
                "completion_state": { "type": "string", "enum": ["completed", "failed_completed"], "description": "complete 时必填。completed 表示完成，failed_completed 表示结束但失败。" },
                "completion_conclusion": { "type": "string", "description": "complete 时填写最终结果、失败原因或阻塞点。" },
                "trigger": {
                  "type": "object",
                  "description": "任务触发时间设置。时间、重复频率、定期/周期性语义统一写在这里。",
                  "properties": {
                    "run_at": { "type": "string", "description": "必填。首次触发时间。RFC3339，保留时区和秒，例如 2026-05-07T20:00:00+08:00。" },
                    "cron_expression": { "type": "string", "description": "可选。标准 Linux/Unix 5 段 cron。留空表示只触发一次，例如 * * * * * 表示每分钟一次。" },
                    "end_at": { "type": "string", "description": "可选。停止时间。RFC3339，保留时区和秒，例如 2026-05-08T08:00:00+08:00；必须晚于 run_at。" }
                  }
                }
              },
              "required": ["action"]
            }),
        )
    }
}

impl RuntimeToolMetadata for BuiltinCreateGoalTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "create_goal",
            "在当前会话启动一个长期持续执行的 goal，用于用户希望勿打扰、少询问、自动续跑到完成或阻塞的目标。已有 active goal 时会失败。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "objective": { "type": "string", "description": "用户提供的完整目标内容。" }
              },
              "required": ["objective"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinCreateGoalTool {
    const NAME: &'static str = "create_goal";
    type Args = CreateGoalToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=create_goal args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = goal_create_for_session(&self.app_state, &self.session_id, args)
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=create_goal result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 create_goal 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinUpdateGoalTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "update_goal",
            "将当前会话 active goal 标记为终态。只允许 complete 或 blocked；不要用它记录普通进度。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "status": { "type": "string", "enum": ["complete", "blocked"], "description": "goal 终态。" },
                "evidence": { "type": "string", "description": "status=complete 时必填，说明证明目标已完成的证据。" },
                "blocking_condition": { "type": "string", "description": "status=blocked 时必填，说明连续阻塞的同一条件。" }
              },
              "required": ["status"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinUpdateGoalTool {
    const NAME: &'static str = "update_goal";
    type Args = UpdateGoalToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=update_goal args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = goal_update_for_session(&self.app_state, &self.session_id, args)
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=update_goal result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 update_goal 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinGetSessionTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "get_session",
            "查询可投递的会话。默认返回本地普通未归档会话和远程联系人会话；可用 keyword 按标题、联系人、人格筛选。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "keyword": { "type": "string", "description": "可选，会话检索关键字。" }
              },
              "required": [],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeToolMetadata for BuiltinBackgroundTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "background",
            "统一查询和治理后台工作。当前支持查看本会话 shell 后台运行态与委托状态、查看详情和终止。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["list", "status", "kill"], "description": "后台动作。list=列当前会话运行中的 shell 后台和委托；status=查看某项状态；kill=终止某项后台工作。" },
                "id": { "type": "string", "description": "后台工作 ID。status/kill 时必填。" },
                "limit": { "type": "integer", "description": "可选。返回内容的最大字符数提示。" }
              },
              "required": ["action"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinGetSessionTool {
    const NAME: &'static str = "get_session";
    type Args = GetSessionToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=get_session args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result =
                builtin_get_session(&self.app_state, &self.session_id, args).map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=get_session result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 get_session 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeValueTool for BuiltinBackgroundTool {
    const NAME: &'static str = "background";
    type Args = BackgroundToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=background args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_background(&self.app_state, &self.session_id, args)
                .await
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=background result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 background 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinInformSessionTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "inform_session",
            "向指定会话投递一条系统助理通知。目标为远程联系人时，会同时写入联系人会话并外发到远端。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "session_id": { "type": "string", "description": "目标会话 ID。" },
                "content": { "type": "string", "description": "要通知的正文。" }
              },
              "required": ["session_id", "content"],
              "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinInformSessionTool {
    const NAME: &'static str = "inform_session";
    type Args = InformSessionToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=inform_session args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_inform_session(&self.app_state, &self.session_id, args)
                .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=inform_session result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 inform_session 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeValueTool for BuiltinTaskTool {
    const NAME: &'static str = "task";
    type Args = TaskToolArgsWire;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=task args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        let result = builtin_task(
            &self.app_state,
            &self.session_id,
            &self.api_config_id,
            &self.executor_agent_id,
            args,
        )
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=task result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 task 执行失败: 错误={err}")),
        }
        result
        })
    }
}

#[derive(Debug, Clone)]
struct BuiltinDelegateTool {
    app_state: AppState,
    session_id: String,
    source_agent_id: String,
}

impl RuntimeToolMetadata for BuiltinDelegateTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "delegate",
            "在下级人格开启一个子代理，协助处理当前工作。当当前工作有更匹配的直属下级人格，或子任务能用简明背景独立说明清楚时，应优先发起委托。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "agent_id": { "type": "string", "description": "要委托的下级人格，填「你的直属下级人格」清单里的人格名称（也兼容人格 id）。应选择与当前任务最匹配的直接下级人格。" },
                "mode": { "type": "string", "enum": ["wait", "background"], "description": "委托方式。mode 只表示父调度是否等待结果，不表示是否并发。除非用户明确要求后台运行，否则一律使用 wait。wait 会等待子代理返回结果，多个 wait 委托可以同时发出并等待全部返回；background 会后台运行并稍后写回当前来源会话。", "default": "wait" },
                "why": { "type": "string", "description": "为什么要做、背景材料、已知事实、已有线索或必要上下文。" },
                "goal": { "type": "string", "description": "这次委托要达成的目标，写成明确可执行、可判断完成的任务。" },
                "todo": { "type": "string", "description": "优先关注点、范围边界、交付要求、下一步待办或需要避免的方向。" }
              },
              "required": ["agent_id", "why", "goal", "todo"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinDelegateTool {
    const NAME: &'static str = "delegate";
    type Args = DelegateToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=delegate args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        ));
        let result = builtin_delegate(
            &self.app_state,
            &self.session_id,
            Some(self.source_agent_id.as_str()),
            args,
        )
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => runtime_log_debug(format!(
                "[工具调试] 内置工具执行完成 name=delegate result={}",
                debug_value_snippet(v, 240)
            )),
            Err(err) => runtime_log_error(format!("[工具执行] 内置工具 delegate 执行失败: 错误={err}")),
        }
        result
        })
    }
}

// ==================== deeprecall 深度回忆工具 ====================

/// deeprecall 主入口：对主对话可见，效果是发起一次同步委托，让发起者自己去回忆。
#[derive(Debug, Clone)]
struct BuiltinDeepRecallTool {
    app_state: AppState,
    session_id: String,
    source_agent_id: String,
}

impl RuntimeToolMetadata for BuiltinDeepRecallTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "deeprecall",
            "深度回忆：从你自己过去全部聊天记录中找回某个话题的来龙去脉。它会委托你自己的分身，在只读的历史记忆里多轮检索、核对上下文，最后交回一份带来源坐标的回忆报告。适合「当初是怎么定下来的」「那段讨论的前因后果」这类需要串联多次对话、跨越较长时间的问题；当前上下文里已经说清楚的事情不要用它。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "要回忆什么，用一句自然语言说清楚。时间、人物、话题范围等约束都写进这句话里，例如「上个月我们讨论过的部署方案是怎么定下来的」。" }
              },
              "required": ["query"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinDeepRecallTool {
    const NAME: &'static str = "deeprecall";
    type Args = DeepRecallToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=deeprecall args={}",
                debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
            ));
            let result = builtin_deep_recall(
                &self.app_state,
                &self.session_id,
                &self.source_agent_id,
                args,
            )
            .await
            .map_err(ToolInvokeError::from);
            match &result {
                Ok(value) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=deeprecall result={}",
                    debug_value_snippet(value, 240)
                )),
                Err(err) => runtime_log_error(format!(
                    "[工具执行] 内置工具 deeprecall 执行失败: 错误={err}"
                )),
            }
            result
        })
    }
}

/// deeprecall_search：仅委托会话可见，供回忆分身做多轮定位检索。
#[derive(Debug, Clone)]
struct BuiltinDeepRecallSearchTool {
    app_state: AppState,
    session_id: String,
    agent_id: String,
}

impl RuntimeToolMetadata for BuiltinDeepRecallSearchTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "deeprecall_search",
            "在发起者全部历史聊天正文里做关键词检索，只返回命中位置，不返回上下文。每个命中给出「会话序号 / 消息序号」坐标，用 deeprecall_context 展开那段真实对话。检索工具已自动跳过工具调用等非正文内容。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "检索关键词或短语。可换不同说法、同义词、时间线索多次检索。" },
                "limit": { "type": "integer", "description": "返回命中数量上限，默认 10，最大 50。", "minimum": 1, "maximum": 50 }
              },
              "required": ["query"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinDeepRecallSearchTool {
    const NAME: &'static str = "deeprecall_search";
    type Args = DeepRecallSearchToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let result = deep_recall_search(
                &self.app_state,
                &self.session_id,
                &self.agent_id,
                &args.query,
                args.limit,
            )
            .map_err(ToolInvokeError::from);
            if let Err(err) = &result {
                runtime_log_error(format!(
                    "[工具执行] 内置工具 deeprecall_search 执行失败: 错误={err}"
                ));
            }
            result
        })
    }
}

/// deeprecall_context：仅委托会话可见，按坐标展开真实上下文。
#[derive(Debug, Clone)]
struct BuiltinDeepRecallContextTool {
    app_state: AppState,
    session_id: String,
    agent_id: String,
}

impl RuntimeToolMetadata for BuiltinDeepRecallContextTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "deeprecall_context",
            "按「会话序号 + 起止消息序号」展开那段真实对话，用于核对命中消息的来龙去脉。消息序号来自 deeprecall_search 的命中结果，从 1 开始，闭区间。",
            serde_json::json!({
              "type": "object",
              "properties": {
                "conversation_index": { "type": "integer", "description": "会话序号，来自 deeprecall_search 命中结果，从 1 开始。", "minimum": 1 },
                "start_index": { "type": "integer", "description": "起始消息序号（含），从 1 开始。想看命中的前因就把起点往前放。", "minimum": 1 },
                "end_index": { "type": "integer", "description": "结束消息序号（含）。必须不小于 start_index；超出会话末尾会自动截断。单次最多返回 100 条消息。", "minimum": 1 }
              },
              "required": ["conversation_index", "start_index", "end_index"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinDeepRecallContextTool {
    const NAME: &'static str = "deeprecall_context";
    type Args = DeepRecallContextToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let result = deep_recall_context(
                &self.app_state,
                &self.session_id,
                &self.agent_id,
                args.conversation_index,
                args.start_index,
                args.end_index,
            )
            .map_err(ToolInvokeError::from);
            if let Err(err) = &result {
                runtime_log_error(format!(
                    "[工具执行] 内置工具 deeprecall_context 执行失败: 错误={err}"
                ));
            }
            result
        })
    }
}

#[cfg(test)]
mod tool_impls_tests {
    use super::config_tool_command_is_readonly;
    #[test]
    fn config_tool_readonly_command_detection() {
        assert!(config_tool_command_is_readonly("help"));
        assert!(config_tool_command_is_readonly("--help"));
        assert!(config_tool_command_is_readonly("-h"));
        assert!(config_tool_command_is_readonly("agent ls"));
        assert!(config_tool_command_is_readonly("agent get demo-agent"));
        assert!(config_tool_command_is_readonly("agent example"));
        assert!(config_tool_command_is_readonly("agent tree"));
        assert!(config_tool_command_is_readonly("mcp ls"));
        assert!(config_tool_command_is_readonly("mcp get some-server"));
        assert!(!config_tool_command_is_readonly("agent new demo-agent"));
        assert!(!config_tool_command_is_readonly("agent update demo-agent x.json"));
        assert!(!config_tool_command_is_readonly("agent update demo-agent next.json"));
        assert!(!config_tool_command_is_readonly("mcp enable some-server"));
        assert!(config_tool_command_is_readonly("store"));
        assert!(config_tool_command_is_readonly("store ls"));
        assert!(config_tool_command_is_readonly("store search playwright"));
        assert!(!config_tool_command_is_readonly("store install clawhub some/skill"));
        assert!(!config_tool_command_is_readonly(""));
        assert!(!config_tool_command_is_readonly("   "));
    }
}
