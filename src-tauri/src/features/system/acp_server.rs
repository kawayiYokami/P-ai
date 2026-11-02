// ==================== ACP stdio agent server ====================
//
// `p-ai acp`
//
// 把 P-ai 暴露为 ACP（Agent Client Protocol）agent，让 Multica 等外部
// 编排平台能像驱动 kimi/traecli 一样驱动 P-ai 领任务干活。
//
// 传输：stdin/stdout 上的 JSON-RPC 2.0（每行一帧）。
// 协议面：
//   - `initialize`        → 声明 agent 能力（loadSession=true）
//   - `session/new`       → 以 params.cwd 建一个 P-ai 会话（conversation），
//                           sessionId 即 conversation_id，返回模型目录
//   - `session/resume`    → 校验会话仍存在后原样返回
//   - `session/set_model` → 把模型切换到指定 api_config
//   - `session/prompt`    → 注入一条用户消息并驱动一轮完整对话，
//                           过程中以 session/update 通知流式回传
//                           文本/思考/工具事件，结束时返回 end_turn
//
// 模型目录来自 P-ai 自身配置的 api_configs（模型供应商在 P-ai 内管理，
// Multica 只选模型、不碰凭据）。工作区 = session/new 的 cwd，P-ai 的
// exec/读写工具都在该目录内干活。
//
// v1 已知限制：session/new 携带的 stdio MCP server 仅记录日志、暂不挂载
// （P-ai 的 MCP 清单仍来自工作区目录）；http/sse 条目由对端按能力声明过滤。
//
// 实现：无头环境复用 headless_cli 的整套基础设施（enter_mode 工具策略、
// wait_mcp_ready、后台任务收尾），只把「一次性单轮」换成「多轮会话」。

const ACP_SERVER_SUBCOMMAND: &str = "acp";

/// Multica 的 ACP 客户端协议版本（hermesClient 固定发 1）。
const ACP_PROTOCOL_VERSION: u64 = 1;

/// stdout 写锁：响应由主循环写、session/update 通知由 delta 回调在
/// 调度器任务上写，两路必须串行化避免半帧交错。
static ACP_STDOUT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn acp_write_frame(value: &serde_json::Value) -> Result<(), String> {
    use std::io::Write as _;
    let line = serde_json::to_string(value).map_err(|err| format!("ACP 帧序列化失败: {err}"))?;
    let _guard = ACP_STDOUT_LOCK.lock();
    {
        let mut out = std::io::stdout().lock();
        writeln!(out, "{line}").map_err(|err| format!("ACP 帧写出失败: {err}"))?;
        out.flush().map_err(|err| format!("ACP 帧冲刷失败: {err}"))?;
    }
    Ok(())
}

fn acp_rpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

/// 判断本次调用是否是 ACP server 子命令（`p-ai acp`）。
fn acp_server_invocation(args: &[String]) -> bool {
    args.get(1)
        .map(|value| value == ACP_SERVER_SUBCOMMAND)
        .unwrap_or(false)
}

/// ACP 入口：返回进程退出码。干净退出 0，致命错误 1。
fn acp_server_enter() -> i32 {
    init_backend_file_logging();
    let runtime = match install_tauri_async_runtime() {
        Ok(runtime) => runtime,
        Err(err) => return acp_server_fail(&err),
    };
    // 调度 future 体量大，必须投递到运行时工作线程（8MB 栈）上轮询；
    // 直接 block_on 会在主线程默认栈上轮询并栈溢出（同 headless CLI）。
    let join = runtime.spawn(acp_server_main());
    match runtime.block_on(join) {
        Ok(code) => code,
        Err(err) => acp_server_fail(&format!("ACP server 任务异常: {err}")),
    }
}

fn acp_server_fail(message: &str) -> i32 {
    use std::io::Write as _;
    // main.rs 把 eprintln! 重定义成了 runtime_log_info，这里必须显式写 stderr。
    let _ = writeln!(std::io::stderr(), "{message}");
    1
}

struct AcpSessionState {
    conversation_id: String,
    model_config_id: Option<String>,
}

struct AcpServer {
    state: Option<AppState>,
    assistant_agent_id: Option<String>,
    mcp_ready_done: bool,
    sessions: std::collections::HashMap<String, AcpSessionState>,
}

impl AcpServer {
    fn new() -> Self {
        Self {
            state: None,
            assistant_agent_id: None,
            mcp_ready_done: false,
            sessions: std::collections::HashMap::new(),
        }
    }

    /// 惰性初始化：第一次收到请求时构造 AppState、加载工作区并缓存主助理。
    /// 失败时返回 Err，由调用方以 JSON-RPC error 回应。
    async fn ensure_ready(&mut self) -> Result<(), String> {
        if self.state.is_some() {
            return Ok(());
        }
        let state = AppState::new()?;
        // 无头入口不走前端就绪回调，必须主动加载工作区：否则 skills 目录
        // 不铺、快照缓存为空、prompt 里没有 skill 索引，MCP 也不会启动。
        let workspace_result = load_workspace(&state)
            .await
            .map_err(|err| format!("加载工作区（skill/MCP）失败: {err}"))?;
        log_workspace_load_result("[工作区加载]", &workspace_result);
        let runtime_org = load_runtime_organization_snapshot(&state)?;
        let agent_id = state_service_get_assistant_agent_id(&state)?;
        if !runtime_org
            .agents
            .iter()
            .any(|agent| agent.id == agent_id && !agent.is_built_in_user)
        {
            return Err(format!("主助理人格不可用: agent_id={agent_id}"));
        }
        self.assistant_agent_id = Some(agent_id);
        self.state = Some(state);
        Ok(())
    }

    fn state(&self) -> Result<&AppState, String> {
        self.state
            .as_ref()
            .ok_or_else(|| "ACP server 尚未初始化".to_string())
    }

    /// P-ai 配置里的模型目录：每个 api_config 是一个可选模型端点。
    fn model_catalog(&self) -> Vec<serde_json::Value> {
        let Ok(state) = self.state() else {
            return Vec::new();
        };
        let Ok(config) = state_read_config_cached(state) else {
            return Vec::new();
        };
        config
            .api_configs
            .iter()
            .map(|api| {
                serde_json::json!({
                    "modelId": api.id,
                    "name": if api.name.trim().is_empty() {
                        api.model.clone()
                    } else {
                        api.name.clone()
                    }
                })
            })
            .collect()
    }

    /// ACP 会话主工作区：取 session/new 的 cwd（而非进程 cwd），让 P-ai
    /// 的 exec/读写工具落在 Multica 派发的任务工作目录内。
    /// 无头环境没有审批入口，直接给 full_access；可写范围仍是授权工作区集合。
    fn acp_shell_workspaces(cwd: &std::path::Path) -> Result<Vec<ShellWorkspaceConfig>, String> {
        let current_dir = cwd
            .canonicalize()
            .map_err(|err| format!("解析会话工作目录失败 ({}): {err}", cwd.display()))?;
        let path_text = current_dir.to_string_lossy().into_owned();
        if path_text.trim() != path_text.as_str() {
            return Err(format!(
                "会话工作目录路径首尾包含空白，无法作为工作区: {path_text}"
            ));
        }
        Ok(vec![ShellWorkspaceConfig {
            id: "acp-main-workspace".to_string(),
            name: shell_workspace_display_name_fallback(&current_dir),
            path: terminal_path_for_user(&current_dir),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: false,
        }])
    }

    async fn handle_session_new(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, (i64, String)> {
        self.ensure_ready()
            .await
            .map_err(|err| (-32000i64, err))?;
        let state = self.state().map_err(|err| (-32000i64, err))?;

        let cwd_text = params
            .get("cwd")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|dir| dir.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
        if cwd_text.is_empty() {
            return Err((-32602i64, "session/new 缺少有效的 cwd".to_string()));
        }

        // v1 限制：记录对端带来的 MCP server，不挂载（见文件头注释）。
        if let Some(servers) = params.get("mcpServers").and_then(serde_json::Value::as_array) {
            if !servers.is_empty() {
                runtime_log_warn(format!(
                    "[ACP] session/new 携带 {} 个 MCP server，当前版本暂不挂载（P-ai 的 MCP 清单仍来自工作区目录）",
                    servers.len()
                ));
            }
        }

        let agent_id = self
            .assistant_agent_id
            .clone()
            .ok_or_else(|| (-32000i64, "主助理人格未初始化".to_string()))?;
        let conversation = create_unarchived_conversation_inner(
            CreateUnarchivedConversationInput {
                api_config_id: None,
                agent_id: Some(agent_id),
                title: Some("Multica".to_string()),
                copy_source_conversation_id: None,
                shell_workspaces: Some(
                    Self::acp_shell_workspaces(std::path::Path::new(&cwd_text))
                        .map_err(|err| (-32602i64, err))?,
                ),
                shell_work_mode: None,
                shell_work_branch: None,
                shell_autonomous_mode: None,
                is_draft: None,
            },
            state,
        )
        .await
        .map_err(|err| (-32000i64, format!("创建会话失败: {err}")))?;
        let session_id = conversation.conversation_id;

        // P-ai 主会话的发送路径只看「单次指定 + 会话首选模型」（见
        // core_send_inner.build_chat_candidate_api_ids），无头会话没有输入
        // 面板可选模型；会话创建时默认取人格主模型，人格未配置主模型时
        // 首选为空，session/prompt 会以「未指定会话模型」失败。这里兜底：
        // 若会话没有首选模型，取目录里第一个可用的文本聊天模型补上；
        // 客户端随后仍可用 session/set_model 覆盖。
        // （放在 sessions.insert 之前：state 借用自 self，insert 的可变
        // 借用必须等 state 用完。）
        let needs_model_fallback = match conversation_service_v2()
            .get_conversation_meta(state, &session_id)
        {
            Ok(meta) => meta.preferred_api_config_id.is_none(),
            Err(_) => true,
        };
        if needs_model_fallback {
            let fallback_model_id = state_read_config_cached(state)
                .ok()
                .and_then(|config| {
                    config
                        .api_configs
                        .iter()
                        .find(|api| is_text_chat_api(api))
                        .map(|api| api.id.clone())
                });
            match fallback_model_id {
                Some(model_id) => {
                    runtime_log_info(format!(
                        "[ACP] session/new 会话无首选模型，回退目录首个可用文本聊天模型: {model_id}"
                    ));
                    conversation_service_v2()
                        .set_preferred_api_config_id(state, &session_id, Some(model_id))
                        .map_err(|err| {
                            (-32000i64, format!("设置会话默认模型失败: {err}"))
                        })?;
                }
                None => {
                    return Err((
                        -32000i64,
                        "P-ai 配置中没有可用的文本聊天模型，无法创建 ACP 会话".to_string(),
                    ));
                }
            }
        }

        self.sessions.insert(
            session_id.clone(),
            AcpSessionState {
                conversation_id: session_id.clone(),
                model_config_id: None,
            },
        );

        let models = self.model_catalog();
        Ok(serde_json::json!({
            "sessionId": session_id,
            "models": { "availableModels": models }
        }))
    }

    async fn handle_session_resume(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, (i64, String)> {
        self.ensure_ready()
            .await
            .map_err(|err| (-32000i64, err))?;
        let state = self.state().map_err(|err| (-32000i64, err))?;
        let session_id = params
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| (-32602i64, "session/resume 缺少 sessionId".to_string()))?
            .to_string();
        // 会话必须仍然存在：Multica 依赖这里的明确错误来识别「已失效的
        // resume 指针」并回退到新会话。
        conversation_service_v2()
            .get_conversation_meta(state, &session_id)
            .map_err(|err| (-32602i64, format!("Session not found: {session_id} ({err})")))?;
        self.sessions.entry(session_id.clone()).or_insert(AcpSessionState {
            conversation_id: session_id.clone(),
            model_config_id: None,
        });
        let models = self.model_catalog();
        Ok(serde_json::json!({
            "sessionId": session_id,
            "models": { "availableModels": models }
        }))
    }

    async fn handle_session_set_model(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, (i64, String)> {
        self.ensure_ready()
            .await
            .map_err(|err| (-32000i64, err))?;
        let state = self.state().map_err(|err| (-32000i64, err))?.clone();
        let session_id = params
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let model_id = params
            .get("modelId")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| (-32602i64, "session/set_model 缺少 modelId".to_string()))?
            .to_string();

        // 模型必须是 P-ai 配置里真实存在的 api_config：静默回退默认模型
        // 会让用户以为选择生效了，实际跑在别的模型上。
        let exists = state_read_config_cached(&state)
            .map(|config| config.api_configs.iter().any(|api| api.id == model_id))
            .unwrap_or(false);
        if !exists {
            return Err((-32602i64, format!("model not available: {model_id}")));
        }

        let Some(session) = self.sessions.get_mut(&session_id) else {
            return Err((-32602i64, format!("Session not found: {session_id}")));
        };
        session.model_config_id = Some(model_id.clone());
        // 会话已存在时同步落库；会话尚未创建时（不可能——session/new 即建会话）
        // 仅内存记录即可。
        conversation_service_v2()
            .set_preferred_api_config_id(&state, &session_id, Some(model_id))
            .map_err(|err| (-32000i64, format!("切换模型失败: {err}")))?;
        Ok(serde_json::json!({}))
    }

    async fn handle_session_prompt(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, (i64, String)> {
        self.ensure_ready()
            .await
            .map_err(|err| (-32000i64, err))?;
        let session_id = params
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let Some(session) = self.sessions.get(&session_id) else {
            return Err((-32602i64, format!("Session not found: {session_id}")));
        };
        let conversation_id = session.conversation_id.clone();
        let model_config_id = session.model_config_id.clone();

        let text = params
            .get("prompt")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| {
                        item.get("type").and_then(serde_json::Value::as_str)
                            == Some("text")
                    })
                    .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err((-32602i64, "session/prompt 缺少文本内容".to_string()));
        }

        // 等待 MCP 探测落定（仅首轮）：早于首轮工具装配即可。
        if !self.mcp_ready_done {
            let state = self.state().map_err(|err| (-32000i64, err))?;
            headless_cli_wait_mcp_ready(state).await;
            self.mcp_ready_done = true;
        }

        let state = self.state().map_err(|err| (-32000i64, err))?.clone();
        let agent_id = self
            .assistant_agent_id
            .clone()
            .ok_or_else(|| (-32000i64, "主助理人格未初始化".to_string()))?;

        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text,
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        conversation_service_v2()
            .append_user_message(
                &state,
                &UserMessageAppendInput {
                    conversation_id: conversation_id.clone(),
                    message: user_message,
                    memory_recall_ids: Vec::new(),
                },
            )
            .await
            .map_err(|err| (-32000i64, format!("写入用户消息失败: {err}")))?;

        let request_id = format!("acp-{}", Uuid::new_v4());
        let mut runtime_context = runtime_context_new("acp_server", "acp_prompt");
        runtime_context.request_id = Some(request_id.clone());
        runtime_context.dispatch_id = Some(request_id.clone());
        runtime_context.origin_conversation_id = Some(conversation_id.clone());
        runtime_context.target_conversation_id = Some(conversation_id.clone());
        runtime_context.root_conversation_id = Some(conversation_id.clone());
        runtime_context.executor_agent_id = Some(agent_id.clone());
        runtime_context.model_config_id = model_config_id.clone();

        let session_for_stream = session_id.clone();
        let request = SendChatRequest {
            trigger_only: true,
            session: Some(SessionSelector {
                api_config_id: model_config_id,
                agent_id,
                conversation_id: Some(conversation_id.clone()),
            }),
            payload: ChatInputPayload {
                text: None,
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
            speaker_agent_id: None,
            trace_id: Some(request_id.clone()),
            assistant_message_id: None,
            oldest_queue_created_at: None,
            remote_im_activation_sources: Vec::new(),
            runtime_context: Some(runtime_context),
        };

        // 流式桥：把调度器的 AssistantDeltaEvent 转成 ACP session/update
        // 通知。回调用的是本会话的 sessionId，因此即使并发多会话也不会串台。
        let channel = tauri::ipc::Channel::<AssistantDeltaEvent>::new(move |body| {
            let parsed = match body {
                tauri::ipc::InvokeResponseBody::Json(json) => {
                    serde_json::from_str::<AssistantDeltaEvent>(&json).ok()
                }
                tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                    serde_json::from_slice::<AssistantDeltaEvent>(&bytes).ok()
                }
            };
            if let Some(event) = parsed {
                acp_forward_delta_event(&session_for_stream, &event);
            }
            Ok(())
        });

        let result = send_chat_message_inner(request, &state, &channel)
            .await
            .map_err(|err| (-32000i64, err))?;
        let _ = result; // 最终文本已通过 delta 流回对端；此处仅确认本轮成功
        Ok(serde_json::json!({ "stopReason": "end_turn" }))
    }
}

/// 从 stdin 读一行（阻塞）。锁在本函数内获取与释放，绝不跨 await。
fn acp_read_line_into(line: &mut String) -> std::io::Result<usize> {
    use std::io::BufRead as _;
    let mut lock = std::io::stdin().lock();
    lock.read_line(line)
}

/// 把一条 AssistantDeltaEvent 投影成 ACP session/update 通知。
///
/// - `kind=None` 的非空 delta → agent_message_chunk（正文文本流）
/// - `activity_reasoning_delta` → agent_thought_chunk（思考流）
/// - `tool_status` → tool_call + tool_call_update（调度进度，成对发送，
///   让对端 UI 显示一次性的阶段说明而不是永远 pending 的工具）
/// - 其余 kind（round_completed / context_usage_update / stream_rebind_required
///   等）为 P-ai 内部事件，不外发。
fn acp_forward_delta_event(session_id: &str, event: &AssistantDeltaEvent) {
    let kind = event.kind.as_deref();
    let update = match kind {
        None | Some("") | Some("delta") => {
            if event.delta.is_empty() {
                return;
            }
            serde_json::json!({
                "sessionUpdate": "agent_message_chunk",
                "content": { "type": "text", "text": event.delta }
            })
        }
        Some("activity_reasoning_delta") => {
            if event.delta.is_empty() {
                return;
            }
            serde_json::json!({
                "sessionUpdate": "agent_thought_chunk",
                "content": { "type": "text", "text": event.delta }
            })
        }
        Some("tool_status") => {
            let title = event
                .message
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "P-ai 正在处理".to_string());
            let tool_call_id = format!(
                "pai-status-{}",
                event
                    .tool_call_id
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| {
                        // 阶段事件没有稳定 id：用请求 id 兜底，保证帧间可关联。
                        event
                            .request_id
                            .clone()
                            .unwrap_or_else(|| Uuid::new_v4().to_string())
                    })
            );
            // in_progress 起手、随即 completed 收尾：成对发送。
            let _ = acp_write_frame(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "session/update",
                "params": {
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": tool_call_id,
                        "title": title,
                        "kind": "other"
                    }
                }
            }));
            serde_json::json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": tool_call_id,
                "status": "completed"
            })
        }
        _ => return,
    };
    let _ = acp_write_frame(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "update": update
        }
    }));
}

/// ACP 主循环：逐行读 JSON-RPC 请求、分发、写回响应。
/// 串行处理——Multica 的 ACP 客户端（hermesClient）对请求也是逐个等待
/// 响应的，流式更新经 delta 回调异步写出，不占用本循环。
async fn acp_server_main() -> i32 {
    let mut server = AcpServer::new();
    let mut line = String::new();

    loop {
        line.clear();
        // stdin 锁不能跨 await 持有（StdinLock 非 Send）：每次读行临时
        // 加锁、读完即放。Stdin 内部自带全局缓冲，逐行读取不会丢数据。
        match acp_read_line_into(&mut line) {
            Ok(0) => return 0, // stdin 关闭：对端结束，干净退出
            Ok(_) => {}
            Err(err) => {
                runtime_log_error(format!("[ACP] 读取 stdin 失败: {err}"));
                return 1;
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(frame) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            let _ = acp_write_frame(&acp_rpc_error(
                serde_json::Value::Null,
                -32700,
                "parse error: 请求不是合法 JSON",
            ));
            continue;
        };
        // 通知（无 id）一律忽略：本实现不发起也不消费对端通知。
        let Some(id) = frame.get("id").cloned() else {
            continue;
        };
        let method = frame
            .get("method")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let params = frame.get("params").cloned().unwrap_or(serde_json::json!({}));

        let response = match method.as_str() {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": ACP_PROTOCOL_VERSION,
                    "agentInfo": {
                        "name": "p-ai",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "agentCapabilities": { "loadSession": true }
                }
            }),
            "session/new" => match server.handle_session_new(&params).await {
                Ok(result) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err((code, message)) => acp_rpc_error(id, code, &message),
            },
            "session/resume" => match server.handle_session_resume(&params).await {
                Ok(result) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err((code, message)) => acp_rpc_error(id, code, &message),
            },
            "session/set_model" => match server.handle_session_set_model(&params).await {
                Ok(result) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err((code, message)) => acp_rpc_error(id, code, &message),
            },
            "session/prompt" => match server.handle_session_prompt(&params).await {
                Ok(result) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err((code, message)) => acp_rpc_error(id, code, &message),
            },
            other => acp_rpc_error(
                id,
                -32601,
                &format!("method not found: {other}"),
            ),
        };
        if let Err(err) = acp_write_frame(&response) {
            runtime_log_error(format!("[ACP] 写回响应失败: {err}"));
            return 1;
        }
    }
}

#[cfg(test)]
mod acp_server_tests {
    use super::*;

    #[test]
    fn invocation_only_matches_acp_subcommand() {
        let argv = |items: &[&str]| -> Vec<String> { items.iter().map(|s| s.to_string()).collect() };
        assert!(acp_server_invocation(&argv(&["p-ai", "acp"])));
        assert!(!acp_server_invocation(&argv(&["p-ai"])));
        assert!(!acp_server_invocation(&argv(&["p-ai", "run", "hi"])));
        assert!(!acp_server_invocation(&argv(&["p-ai", "acpx"])));
    }

    #[test]
    fn rpc_error_carries_code_and_message() {
        let frame = acp_rpc_error(serde_json::json!(7), -32602, "bad params");
        assert_eq!(frame["id"], 7);
        assert_eq!(frame["error"]["code"], -32602);
        assert_eq!(frame["error"]["message"], "bad params");
        assert!(frame.get("result").is_none());
    }

    #[test]
    fn delta_forwarding_maps_kinds() {
        // 正文 delta → agent_message_chunk；空 delta 不发帧。
        // 通过 ACP_STDOUT_LOCK 保护下的写帧不可直接断言 stdout，
        // 这里只验证投影逻辑的分支选择不会 panic、空事件被跳过。
        let text_event = AssistantDeltaEvent {
            delta: "hello".to_string(),
            kind: None,
            request_id: None,
            activation_id: None,
            phase_id: None,
            reason: None,
            tool_name: None,
            tool_call_id: None,
            tool_status: None,
            tool_args: None,
            message: None,
            stream_cache: None,
        };
        acp_forward_delta_event("ses_x", &text_event);

        let empty_event = AssistantDeltaEvent {
            delta: String::new(),
            kind: Some("assistant_tool_event".to_string()),
            request_id: None,
            activation_id: None,
            phase_id: None,
            reason: None,
            tool_name: None,
            tool_call_id: None,
            tool_status: None,
            tool_args: None,
            message: None,
            stream_cache: None,
        };
        // 内部事件 kind 不外发（无帧写出路径）。
        acp_forward_delta_event("ses_x", &empty_event);
    }
}
