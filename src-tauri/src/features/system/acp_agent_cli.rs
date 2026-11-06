// ==================== ACP agent 子命令 ====================
//
// `p-ai acp`：把 PAI 作为 ACP（Agent Client Protocol）agent，由编辑器以子进程拉起，
// 走 JSON-RPC over stdio。这一层只是协议壳——协议帧交给官方 crate，
// 整轮调度仍走 `send_chat_message_inner`，不另造 agent loop。
//
// stdout 只允许出现协议帧：所有日志经 runtime_log_* 走 stderr 与后端日志文件，
// 本文件里不得出现任何 `println!`。

const ACP_CLI_SUBCOMMAND: &str = "acp";

use agent_client_protocol::schema::v1::{
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionConfigValueId,
};

/// 一轮的取消控制。
///
/// `turn_id` 是这一轮的身份：`session/cancel` 只作用于**持有该身份**的那一轮，
/// 不按会话或时间推断归属，避免上一轮的取消信号落到下一轮头上。
#[derive(Debug)]
struct AcpTurnControl {
    turn_id: String,
    /// 本轮调度的请求号。停止链路按身份指认对象，不带它就什么都停不下，
    /// 所以这一轮的请求号在轮次创建时定下来，取消时原样带回去。
    request_id: String,
    token: tokio_util::sync::CancellationToken,
}

impl AcpTurnControl {
    fn new() -> Self {
        Self {
            turn_id: format!("acp-turn-{}", Uuid::new_v4()),
            request_id: format!("acp-{}", Uuid::new_v4()),
            token: tokio_util::sync::CancellationToken::new(),
        }
    }
}

/// 一个 ACP session 在 PAI 这边的落地信息。
#[derive(Clone, Debug)]
struct AcpSessionState {
    conversation_id: String,
    agent_id: String,
    model_config_id: String,
    /// 本会话是否有轮次正在跑。同一会话同一时刻只允许一轮：无头轮次绕过桌面端的
    /// `conversation_scheduler` 直接写会话历史，两轮并行会同时往同一份记录里写。
    prompt_in_flight: bool,
    /// 正在跑的那一轮的取消控制。轮次结束后清空，不留到下一轮。
    turn: Option<std::sync::Arc<AcpTurnControl>>,
}

type AcpSessionRegistry =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, AcpSessionState>>>;

/// `session/prompt` 抢占本会话在途轮次的结果。
#[derive(Debug)]
enum AcpPromptClaim {
    /// 占位成功，可以开始跑本轮。
    Ready(AcpSessionState),
    /// 会话不存在。
    Unknown,
    /// 同一会话已有一轮在跑。
    Busy,
    /// 会话表锁不可用（中毒）。
    Unavailable,
}

/// 抢占本会话的在途轮次。同一会话只允许一轮，抢占失败由调用方回错误。
fn acp_claim_prompt_slot(sessions: &AcpSessionRegistry, session_id: &str) -> AcpPromptClaim {
    let mut registry = match sessions.lock() {
        Ok(guard) => guard,
        Err(_) => return AcpPromptClaim::Unavailable,
    };
    let Some(session) = registry.get_mut(session_id) else {
        return AcpPromptClaim::Unknown;
    };
    if session.prompt_in_flight {
        return AcpPromptClaim::Busy;
    }
    session.prompt_in_flight = true;
    session.turn = Some(std::sync::Arc::new(AcpTurnControl::new()));
    AcpPromptClaim::Ready(session.clone())
}

/// 会话在途轮次的占位。轮次结束——成功、失败、提前返回都算——由 `Drop` 复位。
///
/// 用 RAII 而不是在每个返回点手写复位：这条路径出口有好几个，漏一个就会把会话
/// 永久卡在「本轮尚未结束」。
struct AcpPromptSlot {
    sessions: AcpSessionRegistry,
    conversation_id: String,
}

impl Drop for AcpPromptSlot {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.sessions.lock() {
            if let Some(session) = registry.get_mut(&self.conversation_id) {
                session.prompt_in_flight = false;
                // 取消控制是这一轮的身份，轮次结束必须一起丢掉。留着它，
                // 一个迟到的 `session/cancel` 就会打中下一轮。
                session.turn = None;
            }
        }
    }
}

/// 取本会话当前在途轮次的取消控制；会话不存在或没有在途轮次时给 `None`。
fn acp_current_turn(
    sessions: &AcpSessionRegistry,
    session_id: &str,
) -> Result<Option<std::sync::Arc<AcpTurnControl>>, String> {
    let registry = sessions
        .lock()
        .map_err(|_| "会话表不可用".to_string())?;
    Ok(registry.get(session_id).and_then(|session| session.turn.clone()))
}

/// `session/cancel`：中止指定会话当前正在跑的那一轮。
///
/// 这是通知，没有响应帧可回——任何失败都只能记日志。会话不存在、轮次已经结束
/// 都属于正常竞态（编辑器发 cancel 时那一轮可能刚好收尾），安静丢弃，
/// 绝不能在会话上留下任何会影响下一轮的东西。
///
/// 真正的中止动作不在这里做，而是由那一轮自己做的：通知只按轮次身份去拍
/// `CancellationToken`，轮次收到才去调停止链路。这样「谁被取消」由身份决定，
/// 不由会话或时间推断。
fn acp_handle_cancel_notification(
    sessions: &AcpSessionRegistry,
    notif: agent_client_protocol::schema::v1::CancelNotification,
) -> Result<(), agent_client_protocol::Error> {
    let session_id = notif.session_id.0.to_string();
    let turn = match acp_current_turn(sessions, &session_id) {
        Ok(Some(turn)) => turn,
        Ok(None) => {
            runtime_log_info(format!(
                "[ACP] 跳过，任务=取消轮次，session_id={session_id}，原因=该会话没有在途轮次"
            ));
            return Ok(());
        }
        Err(err) => {
            runtime_log_error(format!(
                "[ACP] 失败，任务=取消轮次，session_id={session_id}，error={err}"
            ));
            return Ok(());
        }
    };
    turn.token.cancel();
    runtime_log_info(format!(
        "[ACP] 受理，任务=取消轮次，session_id={session_id}，turn_id={}",
        turn.turn_id
    ));
    Ok(())
}

/// 让 PAI 真正停下这一轮：复用桌面端的停止链路。
///
/// 自己伪造一个中止结果比这省事，但会把已经流出去的内容留在内存里不落盘、
/// 也不中止正在跑的工具句柄。停止链路已经处理了这些，直接调它。
///
/// 停止链路按身份指认对象，`request_id` 是这一轮唯一的凭据：轮次创建时就定下，
/// 一路带到调度层，取消时原样送回，才停得到本轮而不是别人的。
async fn acp_request_stop(
    state: &AppState,
    session: &AcpSessionState,
    turn: &AcpTurnControl,
) -> Result<(), String> {
    let input = StopChatRequest {
        session: SessionSelector {
            api_config_id: Some(session.model_config_id.clone()),
            agent_id: session.agent_id.clone(),
            conversation_id: Some(session.conversation_id.clone()),
        },
        request_id: Some(turn.request_id.clone()),
        activation_id: None,
        assistant_message_id: None,
    };
    let result = stop_chat_message_inner(input, state).await?;
    runtime_log_info(format!(
        "[ACP] 完成，任务=停止轮次，session_id={}，request_id={}，aborted={}，persisted={}",
        session.conversation_id, turn.request_id, result.aborted, result.persisted
    ));
    Ok(())
}

/// 判断本次调用是否是 ACP agent 子命令（`p-ai acp`）。
fn acp_cli_invocation(args: &[String]) -> bool {
    args.get(1)
        .map(|value| value == ACP_CLI_SUBCOMMAND)
        .unwrap_or(false)
}

/// ACP 入口：返回进程退出码。协议连接正常收口为 0。
fn run_acp_agent_cli() -> i32 {
    // 工具预设与 `p-ai run` 保持一致（禁用委托）；但不抑制 stderr——
    // ACP 要求日志只能走 stderr，stdout 留给协议帧。
    headless_cli_enter_tool_preset(false);
    init_backend_file_logging();
    let runtime = match install_tauri_async_runtime() {
        Ok(runtime) => runtime,
        Err(err) => return acp_cli_fail(&err),
    };
    // 调度 future 体量大，与无头 CLI 一样投递到运行时工作线程上轮询。
    let join = runtime.spawn(acp_serve());
    match runtime.block_on(join) {
        Ok(Ok(())) => 0,
        Ok(Err(err)) => acp_cli_fail(&err),
        Err(err) => acp_cli_fail(&format!("ACP 运行任务异常: {err}")),
    }
}

fn acp_cli_fail(message: &str) -> i32 {
    use std::io::Write as _;
    // main.rs 把 eprintln! 重定义成了 runtime_log_info，这里必须显式写 stderr。
    let _ = writeln!(std::io::stderr(), "{message}");
    1
}

async fn acp_serve() -> Result<(), String> {
    let state = AppState::new()?;
    // 与无头 CLI 同一条启动骨架：不起窗口，但要把 skill / MCP / 工作区铺好，
    // 否则首轮工具装配拿不到 MCP 工具，prompt 里也没有 skill 索引。
    let workspace_result = load_workspace(&state)
        .await
        .map_err(|err| format!("加载工作区（skill/MCP）失败: {err}"))?;
    log_workspace_load_result("[工作区加载]", &workspace_result);
    headless_cli_wait_mcp_ready(&state).await;

    let sessions: AcpSessionRegistry =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let state_for_new = state.clone();
    let sessions_for_new = sessions.clone();
    let state_for_config = state.clone();
    let sessions_for_config = sessions.clone();
    let state_for_load = state.clone();
    let sessions_for_load = sessions.clone();
    let state_for_prompt = state.clone();
    let sessions_for_prompt = sessions.clone();
    let sessions_for_cancel = sessions.clone();

    agent_client_protocol::Agent
        .builder()
        .name("p-ai")
        .on_receive_request(
            async |req: agent_client_protocol::schema::v1::InitializeRequest, responder, _cx| {
                responder.respond(acp_initialize_response(req))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: agent_client_protocol::schema::v1::NewSessionRequest, responder, _cx| {
                acp_handle_new_session(
                    state_for_new.clone(),
                    sessions_for_new.clone(),
                    req,
                    responder,
                )
                .await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: agent_client_protocol::schema::v1::LoadSessionRequest,
                        responder,
                        cx| {
                acp_handle_load_session(
                    state_for_load.clone(),
                    sessions_for_load.clone(),
                    req,
                    responder,
                    cx,
                )
                .await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: agent_client_protocol::schema::v1::SetSessionConfigOptionRequest,
                        responder,
                        _cx| {
                acp_handle_set_config_option(
                    state_for_config.clone(),
                    sessions_for_config.clone(),
                    req,
                    responder,
                )
                .await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: agent_client_protocol::schema::v1::PromptRequest, responder, cx| {
                acp_handle_prompt(
                    state_for_prompt.clone(),
                    sessions_for_prompt.clone(),
                    req,
                    responder,
                    cx,
                )
                .await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |notif: agent_client_protocol::schema::v1::CancelNotification, _cx| {
                acp_handle_cancel_notification(&sessions_for_cancel, notif)
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_to(agent_client_protocol::Stdio::new())
        .await
        .map_err(|err| format!("ACP 连接结束: {err}"))?;
    // 连接收掉：客户端带来的会话级 MCP 服务器也要一起收，别留孤儿进程。
    session_mcp_dispose_all(&state).await;
    Ok(())
}

/// 能力只报真做到的那部分：`loadSession` 已实现，图片与 MCP over ACP 尚未实现。
fn acp_initialize_response(
    req: agent_client_protocol::schema::v1::InitializeRequest,
) -> agent_client_protocol::schema::v1::InitializeResponse {
    use agent_client_protocol::schema::v1::{AgentCapabilities, InitializeResponse};
    InitializeResponse::new(req.protocol_version)
        .agent_capabilities(AgentCapabilities::new().load_session(true))
}

fn acp_internal_error(message: impl Into<String>) -> agent_client_protocol::Error {
    use agent_client_protocol::schema::v1::ErrorCode;
    agent_client_protocol::Error::new(ErrorCode::InternalError.into(), message)
}

/// 参数不合法：会话不存在、配置项不认识、模型 id 不在配置里。
fn acp_invalid_params(message: impl Into<String>) -> agent_client_protocol::Error {
    use agent_client_protocol::schema::v1::ErrorCode;
    agent_client_protocol::Error::new(ErrorCode::InvalidParams.into(), message)
}

/// ACP 里模型选项的固定 id，客户端按这个 id 提交切换请求。
const ACP_MODEL_CONFIG_ID: &str = "model";

/// 选项显示名：优先配置名，为空退到模型标识，再退到配置 id。
fn acp_model_option_label(api: &ApiConfig) -> String {
    let name = api.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let model = api.model.trim();
    if !model.is_empty() {
        return model.to_string();
    }
    api.id.trim().to_string()
}

/// 把配置里可用的聊天模型列成 ACP 的 `Model` 分类下拉。
///
/// 选项值就是 `api_config_id`，`current_model_config_id` 指本会话当前所用的模型。
/// 过滤用 `is_text_chat_api`（与 IDE 侧模型下拉同一判据）：语音、嵌入、重排这些
/// 配置不是聊天模型，选中它们调度会直接失败，不能出现在下拉里。
fn acp_model_config_options(
    config: &AppConfig,
    current_model_config_id: &str,
) -> Result<Vec<SessionConfigOption>, String> {
    let options: Vec<SessionConfigSelectOption> = config
        .api_configs
        .iter()
        .filter(|api| !api.id.trim().is_empty() && is_text_chat_api(api))
        .map(|api| {
            SessionConfigSelectOption::new(
                SessionConfigValueId::new(api.id.trim().to_string()),
                acp_model_option_label(api),
            )
        })
        .collect();
    if options.is_empty() {
        return Err("配置里没有可用的模型".to_string());
    }
    let current = current_model_config_id.trim();
    if current.is_empty() {
        return Err("会话没有正在使用的模型".to_string());
    }
    // 协议要求 `currentValue` 必然是 `options` 里的某一项。放行一个不在选项里的值，
    // 客户端下拉会显示空白、点不动；宁可当场报错，也不要发出违反协议的帧。
    if !options
        .iter()
        .any(|item| item.value.0.to_string() == current)
    {
        return Err(format!("会话当前模型不在可选范围内: {current}"));
    }
    let mut option = SessionConfigOption::select(
        ACP_MODEL_CONFIG_ID,
        "模型",
        SessionConfigValueId::new(current.to_string()),
        options,
    );
    option.category = Some(SessionConfigOptionCategory::Model);
    Ok(vec![option])
}

/// 校验切换请求：只接受配置里真实存在、且是文本聊天模型的 `api_config_id`。
fn acp_resolve_model_switch(config: &AppConfig, requested: &str) -> Result<String, String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err("模型 id 不能为空".to_string());
    }
    let Some(api) = config
        .api_configs
        .iter()
        .find(|api| api.id.trim() == requested)
    else {
        return Err(format!("模型配置不存在: {requested}"));
    };
    if !is_text_chat_api(api) {
        return Err(format!("不是文本聊天模型，不能用于会话: {requested}"));
    }
    Ok(api.id.trim().to_string())
}

/// 纯决策：算出切换后的模型与新选项集。
///
/// 换不动就返回错误，调用方据此拒绝请求、既不落盘也不改内存，
/// 会话会停在原模型上。
fn acp_apply_model_switch(
    config: &AppConfig,
    requested: &str,
) -> Result<(String, Vec<SessionConfigOption>), String> {
    let next = acp_resolve_model_switch(config, requested)?;
    let options = acp_model_config_options(config, &next)?;
    Ok((next, options))
}

async fn acp_handle_new_session(
    state: AppState,
    sessions: AcpSessionRegistry,
    req: agent_client_protocol::schema::v1::NewSessionRequest,
    responder: agent_client_protocol::Responder<agent_client_protocol::schema::v1::NewSessionResponse>,
) -> Result<(), agent_client_protocol::Error> {
    // 参数校验放在最前：cwd 不成立时连会话都不该建，更不能先读一轮配置再报错。
    let workspaces = match acp_new_session_workspaces(&req.cwd) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!(
                "[ACP] 新建会话失败，任务=解析会话工作目录，原因={}",
                err.message
            ));
            return responder.respond_with_error(err);
        }
    };
    let model_config_id = match acp_selected_model_config_id(&state) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!("[ACP] 新建会话失败，任务=读取模型配置：{err}"));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    let agent_id = match state_service_get_assistant_agent_id(&state) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!("[ACP] 新建会话失败，任务=读取主助理人格：{err}"));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    // 选项要在建会话之前算好：列不出模型就不该建出一个没有模型可用的会话。
    let config_options = match state_read_config_cached(&state)
        .map_err(|err| format!("读取配置失败: {err}"))
        .and_then(|config| acp_model_config_options(&config, &model_config_id))
    {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!("[ACP] 新建会话失败，任务=列举模型选项：{err}"));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    let title = req
        .cwd
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "ACP 会话".to_string());
    let conversation = match create_unarchived_conversation_inner(
        CreateUnarchivedConversationInput {
            api_config_id: Some(model_config_id.clone()),
            agent_id: Some(agent_id.clone()),
            title: Some(title),
            copy_source_conversation_id: None,
            shell_workspaces: Some(workspaces),
            shell_work_mode: None,
            shell_work_branch: None,
            shell_autonomous_mode: None,
            conversation_kind: Some(CONVERSATION_KIND_ACP.to_string()),
            is_draft: None,
        },
        &state,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!("[ACP] 新建会话失败，任务=创建会话：{err}"));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    let conversation_id = conversation.conversation_id;
    runtime_log_info(format!(
        "[ACP] 完成，任务=新建会话，session_id={conversation_id}，cwd={}，model={}",
        req.cwd.display(),
        model_config_id
    ));
    if let Ok(mut registry) = sessions.lock() {
        registry.insert(
            conversation_id.clone(),
            AcpSessionState {
                conversation_id: conversation_id.clone(),
                agent_id,
                model_config_id,
                prompt_in_flight: false,
                turn: None,
            },
        );
    }
    // 客户端带来的 MCP 服务器在这里挂上：只进内存，不写 llm-workspace/mcp/servers/。
    // 先挂再回响应，客户端拿到 session_id 时工具已经可发现。
    acp_mount_session_mcp(&state, &conversation_id, &req.mcp_servers).await;
    // ACP 建会话不经过前端命令层，前端拿不到这次创建的返回值。主动推一次概览刷新，
    // 否则这条会话只有在应用重启后才会出现在列表里——搜索能命中、列表里却点不开。
    let state_for_overview = state.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(&state_for_overview) {
            runtime_log_warn(format!("[ACP] 概览刷新失败，任务=新建会话：{err}"));
        }
    });
    let mut response = agent_client_protocol::schema::v1::NewSessionResponse::new(
        agent_client_protocol::schema::v1::SessionId::new(conversation_id),
    );
    response.config_options = Some(config_options);
    responder.respond(response)
}

/// 把 ACP 的 `mcpServers` 翻成 PAI 的服务器配置。
///
/// 三种传输与 PAI 定义 JSON 一一对应，这里只搬字段，不补猜：
/// - stdio → `command` / `args` / `env`
/// - http / sse → `type` / `url` / `headers`
///
/// 返回 `(可用配置, 无法转换的原因)`：一条坏定义不该让整场会话开不起来，
/// 但原因必须原样带回给调用方记日志，不能静默丢掉。
fn acp_session_mcp_servers(
    session_id: &str,
    servers: &[agent_client_protocol::schema::v1::McpServer],
) -> (Vec<McpServerConfig>, Vec<String>) {
    use agent_client_protocol::schema::v1::McpServer;

    let headers_map = |items: &[agent_client_protocol::schema::v1::HttpHeader]| {
        items
            .iter()
            .map(|item| (item.name.clone(), item.value.clone()))
            .collect::<std::collections::BTreeMap<String, String>>()
    };

    let mut configs = Vec::<McpServerConfig>::new();
    let mut errors = Vec::<String>::new();
    for (index, server) in servers.iter().enumerate() {
        let (raw_name, entry) = match server {
            McpServer::Stdio(stdio) => {
                let env = stdio
                    .env
                    .iter()
                    .map(|item| (item.name.clone(), item.value.clone()))
                    .collect::<std::collections::BTreeMap<String, String>>();
                (
                    stdio.name.clone(),
                    serde_json::json!({
                        "command": stdio.command.to_string_lossy().into_owned(),
                        "args": stdio.args,
                        "env": env,
                    }),
                )
            }
            McpServer::Http(http) => (
                http.name.clone(),
                serde_json::json!({
                    "type": "http",
                    "url": http.url,
                    "headers": headers_map(&http.headers),
                }),
            ),
            McpServer::Sse(sse) => (
                sse.name.clone(),
                serde_json::json!({
                    "type": "sse",
                    "url": sse.url,
                    "headers": headers_map(&sse.headers),
                }),
            ),
            other => {
                errors.push(format!("不支持的 MCP 传输类型：{other:?}"));
                continue;
            }
        };
        let label = {
            let trimmed = raw_name.trim();
            if trimmed.is_empty() {
                format!("acp-server-{index}")
            } else {
                trimmed.to_string()
            }
        };
        let mut members = serde_json::Map::new();
        members.insert(label.clone(), entry);
        let definition_json = serde_json::json!({
            "mcpServers": serde_json::Value::Object(members)
        })
        .to_string();
        configs.push(McpServerConfig {
            id: format!("acp-{}-{index}", acp_mcp_id_slug(session_id)),
            name: label,
            enabled: true,
            definition_json,
            oauth_capable: false,
            has_oauth_token: false,
            tool_policies: Vec::new(),
            cached_tools: Vec::new(),
            last_status: String::new(),
            last_error: String::new(),
            updated_at: String::new(),
        });
    }
    (configs, errors)
}

/// 服务器 id 里只留安全字符：它会进客户端缓存的键，也进日志。
fn acp_mcp_id_slug(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(12)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// 把客户端带来的 MCP 服务器挂到本进程，并把失败原因写进日志。
async fn acp_mount_session_mcp(
    state: &AppState,
    session_id: &str,
    servers: &[agent_client_protocol::schema::v1::McpServer],
) {
    if servers.is_empty() {
        return;
    }
    let (configs, translate_errors) = acp_session_mcp_servers(session_id, servers);
    for err in translate_errors {
        runtime_log_warn(format!(
            "[ACP] 跳过，任务=解析 MCP 服务器，session_id={session_id}，原因={err}"
        ));
    }
    let mount_errors = session_mcp_attach(state, configs).await;
    for err in mount_errors {
        runtime_log_warn(format!(
            "[ACP] 跳过，任务=挂载 MCP 服务器，session_id={session_id}，原因={err}"
        ));
    }
}

/// `session/load`：把已有会话接回来，并把历史回放给客户端。
///
/// 回放只走分页读取，不整读会话消息：先取最近一页，再按 oldest 锚点往回翻，
/// 拼成时间正序。回放的是用户与助理的正文；工具调用消息、压缩摘要这类没有
/// 对应历史呈现位的类型不在此还原，数量会写进日志，不静默吞掉。
async fn acp_handle_load_session(
    state: AppState,
    sessions: AcpSessionRegistry,
    req: agent_client_protocol::schema::v1::LoadSessionRequest,
    responder: agent_client_protocol::Responder<
        agent_client_protocol::schema::v1::LoadSessionResponse,
    >,
    cx: agent_client_protocol::ConnectionTo<agent_client_protocol::Client>,
) -> Result<(), agent_client_protocol::Error> {
    use agent_client_protocol::schema::v1::LoadSessionResponse;

    let session_id_text = req.session_id.0.to_string();
    // cwd 的校验口径与 `session/new` 完全一致：不是目录就是调用方的参数问题。
    // 载入不改变已有会话的工作区——会话属于它原本那个目录，工具仍落在那里。
    if let Err(err) = acp_new_session_workspaces(&req.cwd) {
        runtime_log_error(format!(
            "[ACP] 载入会话失败，任务=解析会话工作目录，session_id={session_id_text}，原因={}",
            err.message
        ));
        return responder.respond_with_error(err);
    }
    let config = match state_read_config_cached(&state) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!(
                "[ACP] 载入会话失败，任务=读取配置，session_id={session_id_text}，error={err}"
            ));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    // 会话不存在是参数问题：客户端传了一个本进程没见过的 id。
    let meta = match conversation_service_v2().get_conversation_meta(&state, &session_id_text) {
        Ok(meta) => meta,
        Err(err) => {
            runtime_log_warn(format!(
                "[ACP] 拒绝，任务=载入会话，session_id={session_id_text}，原因=会话不存在（{err}）"
            ));
            return responder.respond_with_error(acp_invalid_params(format!(
                "会话不存在: {session_id_text}"
            )));
        }
    };
    // 会话自己记住的模型优先，但必须仍是文本聊天模型；不成立才退到全局选择。
    let stored_model = meta
        .preferred_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter(|id| {
            config
                .api_configs
                .iter()
                .any(|api| api.id.trim() == *id && is_text_chat_api(api))
        })
        .map(ToOwned::to_owned);
    let model_config_id = match stored_model {
        Some(id) => id,
        None => match acp_pick_session_model_config_id(&config) {
            Ok(id) => id,
            Err(err) => {
                runtime_log_error(format!(
                    "[ACP] 载入会话失败，任务=选择模型，session_id={session_id_text}，原因={err}"
                ));
                return responder.respond_with_error(acp_internal_error(err));
            }
        },
    };
    let config_options = match acp_model_config_options(&config, &model_config_id) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!(
                "[ACP] 载入会话失败，任务=列举模型选项，session_id={session_id_text}，原因={err}"
            ));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    let agent_id = meta.agent_id.clone();
    if !meta.shell_workspaces.iter().any(|item| {
        item.path.trim() == terminal_path_for_user(&req.cwd)
    }) {
        // 不静默：让运维日志能看出「客户端要的目录」与「会话记录的目录」不同。
        runtime_log_info(format!(
            "[ACP] 提示，任务=载入会话，session_id={session_id_text}，cwd={}，会话工作区={:?}",
            req.cwd.display(),
            meta.shell_workspaces
                .iter()
                .map(|item| item.path.clone())
                .collect::<Vec<_>>()
        ));
    }

    let messages = match acp_replay_messages(&state, &session_id_text) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!(
                "[ACP] 载入会话失败，任务=读取历史，session_id={session_id_text}，error={err}"
            ));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    let mut replayed = 0usize;
    let mut skipped = 0usize;
    for message in &messages {
        let Some(notification) =
            acp_replay_update(&message.role, &message.parts, &req.session_id)
        else {
            skipped += 1;
            continue;
        };
        if let Err(err) = cx.send_notification(notification) {
            runtime_log_warn(format!(
                "[ACP] 发送回放帧失败，session_id={session_id_text}，error={err}"
            ));
        }
        replayed += 1;
    }
    runtime_log_info(format!(
        "[ACP] 完成，任务=回放历史，session_id={session_id_text}，回放={replayed}，未回放={skipped}"
    ));

    // 载入的会话此后要能直接接 prompt：登记进会话表，轮次状态一律从零开始。
    if let Ok(mut registry) = sessions.lock() {
        registry.insert(
            session_id_text.clone(),
            AcpSessionState {
                conversation_id: session_id_text.clone(),
                agent_id,
                model_config_id: model_config_id.clone(),
                prompt_in_flight: false,
                turn: None,
            },
        );
    }
    acp_mount_session_mcp(&state, &session_id_text, &req.mcp_servers).await;
    runtime_log_info(format!(
        "[ACP] 完成，任务=载入会话，session_id={session_id_text}，model={model_config_id}"
    ));
    responder.respond(LoadSessionResponse::new().config_options(config_options))
}

/// 读一场会话的全部历史，时间正序。返回的是分页拼起来的消息，不是整读。
///
/// 上限存在的意义只是防止把会话读成一次大 IO：回放本来就是个长动作。
fn acp_replay_messages(state: &AppState, conversation_id: &str) -> Result<Vec<ChatMessage>, String> {
    const PAGE: usize = 100;
    const MAX_MESSAGES: usize = 2000;

    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    let Some(page) = message_store::chat_store_read_recent_messages_page_cached(&paths, PAGE)? else {
        return Ok(Vec::new());
    };
    let mut collected = page.messages;
    let mut has_more = page.has_more;
    while has_more && collected.len() < MAX_MESSAGES {
        let Some(oldest_id) = collected.first().map(|message| message.id.clone()) else {
            break;
        };
        let Some(page) =
            message_store::chat_store_read_messages_before(&paths, &oldest_id, PAGE)?
        else {
            break;
        };
        if page.messages.is_empty() {
            break;
        }
        has_more = page.has_more;
        let mut merged = page.messages;
        merged.extend(collected);
        collected = merged;
    }
    if has_more {
        runtime_log_warn(format!(
            "[ACP] 截断，任务=回放历史，session_id={conversation_id}，已回放={}，原因=超过回放上限 {MAX_MESSAGES}",
            collected.len()
        ));
    }
    Ok(collected)
}

/// 把一条历史消息转成回放帧。没有正文、或类型不适合回放的返回 `None`。
fn acp_replay_update(
    role: &str,
    parts: &[MessagePart],
    session_id: &agent_client_protocol::schema::v1::SessionId,
) -> Option<agent_client_protocol::schema::v1::SessionNotification> {
    use agent_client_protocol::schema::v1::{
        ContentBlock, ContentChunk, SessionNotification, SessionUpdate, TextContent,
    };

    let text = parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() {
        return None;
    }
    let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(text)));
    let update = match role.trim().to_ascii_lowercase().as_str() {
        "user" => SessionUpdate::UserMessageChunk(chunk),
        "assistant" => SessionUpdate::AgentMessageChunk(chunk),
        _ => return None,
    };
    Some(SessionNotification::new(session_id.clone(), update))
}

/// `session/set_config_option`：切换本会话使用的模型。
///
/// 模型是会话属性，落到 `conversation.api_config_id`；全局 `selected_api_config_id`
/// 是桌面端默认值，这里不动。
async fn acp_handle_set_config_option(
    state: AppState,
    sessions: AcpSessionRegistry,
    req: agent_client_protocol::schema::v1::SetSessionConfigOptionRequest,
    responder: agent_client_protocol::Responder<
        agent_client_protocol::schema::v1::SetSessionConfigOptionResponse,
    >,
) -> Result<(), agent_client_protocol::Error> {
    use agent_client_protocol::schema::v1::SetSessionConfigOptionResponse;

    let session_id = req.session_id.0.to_string();
    let session = match sessions.lock() {
        Ok(registry) => registry.get(&session_id).cloned(),
        Err(err) => {
            runtime_log_error(format!("[ACP] 切换模型失败，任务=读取会话表：{err}"));
            return responder.respond_with_error(acp_internal_error("会话表不可用"));
        }
    };
    let Some(session) = session else {
        return responder.respond_with_error(acp_invalid_params(format!(
            "会话不存在: {session_id}"
        )));
    };

    let config_id = req.config_id.0.to_string();
    if config_id != ACP_MODEL_CONFIG_ID {
        return responder.respond_with_error(acp_invalid_params(format!(
            "不支持的配置项: {config_id}，当前只有 {ACP_MODEL_CONFIG_ID}"
        )));
    }
    let Some(requested) = req.value.as_value_id().map(|value| value.0.to_string()) else {
        return responder.respond_with_error(acp_invalid_params("模型选项需要一个字符串值"));
    };

    let config = match state_read_config_cached(&state) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_error(format!("[ACP] 切换模型失败，任务=读取配置：{err}"));
            return responder.respond_with_error(acp_internal_error(err));
        }
    };
    // 校验不过就到此为止：既不落盘也不改内存，会话停在原模型上。
    let (next_model_config_id, config_options) = match acp_apply_model_switch(&config, &requested) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_warn(format!(
                "[ACP] 拒绝，任务=切换模型，session_id={session_id}，原因={err}"
            ));
            return responder.respond_with_error(acp_invalid_params(err));
        }
    };

    if let Err(err) = conversation_service_v2().set_preferred_api_config_id(
        &state,
        &session.conversation_id,
        Some(next_model_config_id.clone()),
    ) {
        runtime_log_error(format!(
            "[ACP] 失败，任务=保存会话模型，session_id={session_id}，error={err}"
        ));
        return responder.respond_with_error(acp_internal_error(err));
    }
    if let Ok(mut registry) = sessions.lock() {
        if let Some(entry) = registry.get_mut(&session_id) {
            entry.model_config_id = next_model_config_id.clone();
        }
    }
    runtime_log_info(format!(
        "[ACP] 完成，任务=切换模型，session_id={session_id}，from={}，to={next_model_config_id}",
        session.model_config_id
    ));
    responder.respond(SetSessionConfigOptionResponse::new(config_options))
}

/// `session/prompt`：先在协议循环里做占用检查，再把这一轮派到连接任务上跑。
///
/// 必须在这里就返回：框架会 `await` 每个请求的 handler，整轮留在这里等待的话，
/// 同一连接在整轮期间读不到下一帧——「本轮尚未结束」的第二帧会被一直阻塞到本轮
/// 结束，互斥形同虚设，客户端也被卡住。
async fn acp_handle_prompt(
    state: AppState,
    sessions: AcpSessionRegistry,
    req: agent_client_protocol::schema::v1::PromptRequest,
    responder: agent_client_protocol::Responder<agent_client_protocol::schema::v1::PromptResponse>,
    cx: agent_client_protocol::ConnectionTo<agent_client_protocol::Client>,
) -> Result<(), agent_client_protocol::Error> {
    let session_id_text = req.session_id.0.to_string();
    // 内容块校验先于占位：不支持的类型要立刻回错误，连轮次都不该开。
    let prompt_text = match acp_prompt_text(&req.prompt) {
        Ok(text) => text,
        Err(err) => {
            runtime_log_warn(format!(
                "[ACP] 拒绝，任务=解析提示词，session_id={session_id_text}，原因={err}"
            ));
            return responder.respond_with_error(acp_invalid_params(err));
        }
    };
    let session = match acp_claim_prompt_slot(&sessions, &session_id_text) {
        AcpPromptClaim::Ready(session) => session,
        AcpPromptClaim::Unavailable => {
            // 锁中毒与「会话不存在」是两件不同的事，不能都折成本地未知 session。
            runtime_log_error(format!(
                "[ACP] 轮次失败，任务=读取会话表，session_id={session_id_text}，原因=会话表不可用"
            ));
            return responder.respond_with_error(acp_internal_error("会话表不可用"));
        }
        AcpPromptClaim::Unknown => {
            runtime_log_error(format!(
                "[ACP] 轮次失败，任务=定位会话，session_id={session_id_text}"
            ));
            return responder.respond_with_error(acp_invalid_params(format!(
                "未知的 session_id: {session_id_text}"
            )));
        }
        AcpPromptClaim::Busy => {
            // 会话历史没有并发写保护，排队又会让客户端干等，所以直接拒。
            runtime_log_warn(format!(
                "[ACP] 拒绝，任务=处理轮次，session_id={session_id_text}，原因=该会话本轮尚未结束"
            ));
            return responder.respond_with_error(acp_invalid_params(format!(
                "该会话本轮尚未结束，不能并发处理新的提示词: {session_id_text}"
            )));
        }
    };
    // 占位随轮次一起交给连接任务，轮次结束时由 Drop 复位。
    let slot = AcpPromptSlot {
        sessions: sessions.clone(),
        conversation_id: session.conversation_id.clone(),
    };
    // 占位成功必然带上了本轮的身份；缺了说明会话表状态不一致，
    // 不能开一个「取消不了」的轮次。
    let turn = match session.turn.clone() {
        Some(turn) => turn,
        None => {
            drop(slot);
            runtime_log_error(format!(
                "[ACP] 轮次失败，任务=读取取消控制，session_id={session_id_text}，原因=占位未带轮次身份"
            ));
            return responder.respond_with_error(acp_internal_error("轮次取消控制缺失"));
        }
    };

    if prompt_text.trim().is_empty() {
        drop(slot);
        runtime_log_warn(format!(
            "[ACP] 跳过，任务=处理轮次，session_id={session_id_text}，原因=提示词没有文本内容"
        ));
        return responder.respond(agent_client_protocol::schema::v1::PromptResponse::new(
            agent_client_protocol::schema::v1::StopReason::EndTurn,
        ));
    }

    let session_id = req.session_id;
    let cx_for_turn = cx.clone();
    cx.spawn(async move {
        let _slot = slot;
        // 轮次内的错误已经通过 responder 回给客户端，这里只记日志：
        // 让任务返回 Err 会把整个连接拆掉，代价远大于一次失败的轮次。
        if let Err(err) =
            acp_run_prompt_turn(state, session, session_id, prompt_text, turn, responder, cx_for_turn)
                .await
        {
            runtime_log_error(format!("[ACP] 失败，任务=派发轮次：{err}"));
        }
        Ok(())
    })?;
    Ok(())
}

/// 跑完一轮：构造流式回调、调用共享轮次装配、把结果回给客户端。
///
/// 由 `acp_handle_prompt` 派到连接任务里执行，不在协议输入循环上等待——
/// 否则该连接在整轮期间读不到下一帧，同一会话的第二帧会一直被阻塞到本轮结束，
/// 互斥检查永远没有机会命中。
async fn acp_run_prompt_turn(
    state: AppState,
    session: AcpSessionState,
    session_id: agent_client_protocol::schema::v1::SessionId,
    prompt_text: String,
    turn: std::sync::Arc<AcpTurnControl>,
    responder: agent_client_protocol::Responder<agent_client_protocol::schema::v1::PromptResponse>,
    cx: agent_client_protocol::ConnectionTo<agent_client_protocol::Client>,
) -> Result<(), agent_client_protocol::Error> {
    let session_id_text = session_id.0.to_string();

    let sent_chars = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sent_chars_for_channel = sent_chars.clone();
    // 工具调用 id → 已上报过的进度。ACP 要求同一调用的状态只推进不重复，
    // 而 PAI 每个 delta 都带全量 streamBlocks 快照，必须自己 diff。
    let tool_states: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, AcpToolProgress>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let tool_states_for_channel = tool_states.clone();
    let session_id_for_channel = session_id.clone();
    let cx_for_channel = cx.clone();
    let channel = tauri::ipc::Channel::<AssistantDeltaEvent>::new(move |body| {
        let raw = match body {
            tauri::ipc::InvokeResponseBody::Json(text) => text,
            tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                String::from_utf8_lossy(&bytes).into_owned()
            }
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Ok(());
        };
        let updates = match tool_states_for_channel.lock() {
            Ok(mut states) => acp_updates_from_event(&value, &mut states),
            Err(_) => Vec::new(),
        };
        for update in updates {
            if let agent_client_protocol::schema::v1::SessionUpdate::AgentMessageChunk(chunk) = &update
            {
                if let agent_client_protocol::schema::v1::ContentBlock::Text(text) = &chunk.content {
                    sent_chars_for_channel
                        .fetch_add(text.text.chars().count(), std::sync::atomic::Ordering::Relaxed);
                }
            }
            let notification = agent_client_protocol::schema::v1::SessionNotification::new(
                session_id_for_channel.clone(),
                update,
            );
            if let Err(err) = cx_for_channel.send_notification(notification) {
                runtime_log_warn(format!("[ACP] 发送 session/update 失败：{err}"));
            }
        }
        Ok(())
    });

    let turn_result = headless_cli_run_turn(
        &state,
        &session.conversation_id,
        &session.agent_id,
        &session.model_config_id,
        &prompt_text,
        &turn.request_id,
        "acp_agent",
        "acp_prompt",
        &channel,
    );
    tokio::pin!(turn_result);

    // 监听取消：只在轮次还活着的时候拍一次停止，然后照旧等轮次自然收尾。
    // 不能在这里直接 drop 掉轮次 future —— 那会把 responder 一起扔掉，
    // 客户端等不到任何响应帧。
    let mut stop_requested = false;
    let result = loop {
        tokio::select! {
            biased;
            _ = turn.token.cancelled(), if !stop_requested => {
                stop_requested = true;
                runtime_log_info(format!(
                    "[ACP] 受理，任务=停止轮次，session_id={session_id_text}，turn_id={}",
                    turn.turn_id
                ));
                if let Err(err) = acp_request_stop(&state, &session, &turn).await {
                    runtime_log_error(format!(
                        "[ACP] 失败，任务=停止轮次，session_id={session_id_text}，error={err}"
                    ));
                }
            }
            outcome = &mut turn_result => break outcome,
        }
    };

    if stop_requested {
        // 被打断的一轮一律回 cancelled：协议对 `session/cancel` 的回应就是它。
        match &result {
            Ok(_) => runtime_log_info(format!(
                "[ACP] 完成，任务=取消轮次，session_id={session_id_text}，turn_id={}，结果=已中止",
                turn.turn_id
            )),
            Err(err) => runtime_log_info(format!(
                "[ACP] 完成，任务=取消轮次，session_id={session_id_text}，turn_id={}，结果=已中止（轮次返回错误：{err}）",
                turn.turn_id
            )),
        }
        return responder.respond(agent_client_protocol::schema::v1::PromptResponse::new(
            agent_client_protocol::schema::v1::StopReason::Cancelled,
        ));
    }

    match result {
        Ok(final_text) => {
            // 非流式路径不会产生文本增量。若整轮一条文本 chunk 都没发出去，
            // 这里补一整段，避免客户端看到「有工具调用、没有回答」。
            let already_sent = sent_chars.load(std::sync::atomic::Ordering::Relaxed);
            let trimmed = final_text.trim();
            if already_sent == 0 && !trimmed.is_empty() {
                let update = agent_client_protocol::schema::v1::SessionUpdate::AgentMessageChunk(
                    agent_client_protocol::schema::v1::ContentChunk::new(
                        agent_client_protocol::schema::v1::ContentBlock::Text(
                            agent_client_protocol::schema::v1::TextContent::new(trimmed),
                        ),
                    ),
                );
                let notification = agent_client_protocol::schema::v1::SessionNotification::new(
                    session_id.clone(),
                    update,
                );
                if let Err(err) = cx.send_notification(notification) {
                    runtime_log_warn(format!("[ACP] 补发回答失败：{err}"));
                }
            }
            runtime_log_info(format!(
                "[ACP] 完成，任务=处理轮次，session_id={session_id_text}，sent_chars={already_sent}，final_chars={}",
                trimmed.chars().count()
            ));
            responder.respond(agent_client_protocol::schema::v1::PromptResponse::new(
                agent_client_protocol::schema::v1::StopReason::EndTurn,
            ))
        }
        Err(err) => {
            runtime_log_error(format!(
                "[ACP] 失败，任务=处理轮次，session_id={session_id_text}，error={err}"
            ));
            responder.respond_with_error(acp_internal_error(err))
        }
    }
}

/// 会话模型：沿用配置里当前选中的聊天模型；不可用时退到第一条可用的聊天配置。
///
/// 判据与下拉、切换校验一致（`is_text_chat_api`）。少这一道过滤，全局恰好选中语音或
/// 嵌入配置时，新会话会被锁到非聊天模型——`currentValue` 落在 `options` 之外违反协议，
/// 首轮 prompt 也必然失败。
fn acp_pick_session_model_config_id(config: &AppConfig) -> Result<String, String> {
    let selected = config.selected_api_config_id.trim();
    if !selected.is_empty()
        && config
            .api_configs
            .iter()
            .any(|api| api.id.trim() == selected && is_text_chat_api(api))
    {
        return Ok(selected.to_string());
    }
    config
        .api_configs
        .iter()
        .find(|api| !api.id.trim().is_empty() && is_text_chat_api(api))
        .map(|api| api.id.trim().to_string())
        .ok_or_else(|| "配置里没有可用的聊天模型".to_string())
}

fn acp_selected_model_config_id(state: &AppState) -> Result<String, String> {
    acp_pick_session_model_config_id(&state_read_config_cached(state)?)
}

/// `session/new` 的工作区入口：把 `cwd` 解析成主工作区，失败统一折成 `InvalidParams`。
///
/// 这是 `session/new` 的第一道校验，在建会话之前跑。目录不存在、传的是文件、路径
/// 首尾含空白，都是调用方参数问题，不是 agent 内部故障——回 `InternalError` 会让
/// 客户端当成 agent 挂了，重试逻辑也会走错分支。
fn acp_new_session_workspaces(
    cwd: &std::path::Path,
) -> Result<Vec<ShellWorkspaceConfig>, agent_client_protocol::Error> {
    acp_shell_workspaces(cwd).map_err(|err| acp_invalid_params(err))
}

/// ACP 的 `cwd` 映射成 PAI 的主工作区，与无头 CLI 用进程 cwd 的做法一致。
fn acp_shell_workspaces(cwd: &std::path::Path) -> Result<Vec<ShellWorkspaceConfig>, String> {
    let canonical = cwd
        .canonicalize()
        .map_err(|err| format!("解析会话工作目录失败 ({}): {err}", cwd.display()))?;
    // `canonicalize` 对文件也会成功。不在这里拦一道，文件路径会被当成工作区收下，
    // 会话照建、日志照写「完成」，工具却静默跑回默认工作区——看着成功，地方不对。
    if !canonical.is_dir() {
        return Err(format!(
            "会话工作目录不是目录 ({}): 需要传入目录而不是文件",
            terminal_path_for_user(&canonical)
        ));
    }
    let path_text = canonical.to_string_lossy().into_owned();
    if path_text.trim() != path_text.as_str() {
        return Err(format!(
            "会话工作目录路径首尾包含空白，无法作为工作区: {}",
            terminal_path_for_user(&canonical)
        ));
    }
    Ok(vec![ShellWorkspaceConfig {
        id: "acp-main-workspace".to_string(),
        name: shell_workspace_display_name_fallback(&canonical),
        path: terminal_path_for_user(&canonical),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        built_in: false,
    }])
}

/// 把提示词内容块转成文本。不支持的类型必须明确报错，不能静默丢。
///
/// - 文本块：拼接；
/// - 资源链接：协议要求所有 agent 必须支持，转成一行引用交给模型；
/// - 图片、音频、内嵌资源：本轮没有声明对应能力（`promptCapabilities` 全为 false），
///   回错误。静默丢掉会让模型答非所问，客户端还以为什么都送到了。
fn acp_prompt_text(
    blocks: &[agent_client_protocol::schema::v1::ContentBlock],
) -> Result<String, String> {
    use agent_client_protocol::schema::v1::ContentBlock;
    let mut parts: Vec<String> = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text(text) => parts.push(text.text.clone()),
            ContentBlock::ResourceLink(link) => {
                let name = link.name.trim();
                let uri = link.uri.trim();
                parts.push(if name.is_empty() {
                    format!("[资源链接] {uri}")
                } else {
                    format!("[资源链接] {name}: {uri}")
                });
            }
            ContentBlock::Image(_) => {
                return Err(
                    "本轮不支持图片内容块：promptCapabilities.image 未声明，请改写成本地文件路径"
                        .to_string(),
                );
            }
            ContentBlock::Audio(_) => {
                return Err(
                    "本轮不支持音频内容块：promptCapabilities.audio 未声明".to_string(),
                );
            }
            ContentBlock::Resource(_) => {
                return Err(
                    "本轮不支持内嵌资源内容块：promptCapabilities.embeddedContext 未声明"
                        .to_string(),
                );
            }
            other => {
                return Err(format!("本轮不支持的提示词内容块类型：{other:?}"));
            }
        }
    }
    Ok(parts.join("\n\n"))
}

/// 一个工具调用在 ACP 侧已经上报过什么。
///
/// `status` 是上一次上报的状态，`args_sent` / `result_sent` 记录参数与结果是否已经
/// 随帧带出。流式里这两项常常晚于首帧到达——首帧可能只有工具名，参数在下一帧才
/// 补齐，结果要等工具执行完——所以不能只看状态是否变化，还要看有没有新内容可补。
#[derive(Debug, Clone, Default)]
struct AcpToolProgress {
    status: String,
    args_sent: bool,
    result_sent: bool,
}

impl AcpToolProgress {
    fn new(status: String, args_sent: bool, result_sent: bool) -> Self {
        Self {
            status,
            args_sent,
            result_sent,
        }
    }
}

/// 把流式里的参数/结果文本转成 ACP 的 `rawInput` / `rawOutput`：能解析成 JSON
/// 就按结构给出去，否则原样作为字符串。
fn acp_raw_json(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        serde_json::from_str::<serde_json::Value>(trimmed)
            .unwrap_or_else(|_| serde_json::Value::String(trimmed.to_string())),
    )
}

/// `assistant_tool_result` 事件本身不带工具名与 id，只在 `message` 里装一条
/// role=tool 的历史事件。这里把它拆出来，用于补上工具的输出。
fn acp_tool_result_payload(event: &serde_json::Value) -> Option<(String, String)> {
    let message = event.get("message").and_then(serde_json::Value::as_str)?;
    let parsed: serde_json::Value = serde_json::from_str(message).ok()?;
    if parsed.get("role").and_then(serde_json::Value::as_str) != Some("tool") {
        return None;
    }
    let id = parsed
        .get("tool_call_id")
        .and_then(serde_json::Value::as_str)?
        .trim();
    if id.is_empty() {
        return None;
    }
    let content = parsed
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((id.to_string(), content))
}

/// 把工具输出补到既有卡片上。输出按原样给出去，不裁剪空白——它是工具的原始产出。
fn acp_apply_tool_result(
    tool_call_id: &str,
    content: &str,
    tool_states: &mut std::collections::HashMap<String, AcpToolProgress>,
    mut updates: Vec<agent_client_protocol::schema::v1::SessionUpdate>,
) -> Vec<agent_client_protocol::schema::v1::SessionUpdate> {
    use agent_client_protocol::schema::v1::{
        SessionUpdate, ToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
    };

    let output = Some(serde_json::Value::String(content.to_string()));
    match tool_states.get(tool_call_id).cloned() {
        Some(previous) => {
            if previous.result_sent {
                return updates;
            }
            let mut fields = ToolCallUpdateFields::default();
            // 失败是终态。上游会先发 `toolStatus = "failed"`，紧接着发同 id 的
            // `assistant_tool_result`（内容是错误文本）；这里若把状态改回 Completed，
            // 编辑器看到的每一次工具报错都会显示成「成功完成」。
            let previous_status = acp_tool_status(&previous.status);
            let keep_failed = previous_status == ToolCallStatus::Failed;
            if !keep_failed && previous_status != ToolCallStatus::Completed {
                fields.status = Some(ToolCallStatus::Completed);
            }
            fields.raw_output = output;
            tool_states.insert(
                tool_call_id.to_string(),
                AcpToolProgress::new(
                    if keep_failed {
                        previous.status.clone()
                    } else {
                        "done".to_string()
                    },
                    previous.args_sent,
                    true,
                ),
            );
            updates.push(SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new(tool_call_id.to_string()),
                fields,
            )));
        }
        None => {
            // 没发过卡片就收到了结果：补一张已完成的卡片，别让工具凭空消失。
            let mut call = ToolCall::new(
                ToolCallId::new(tool_call_id.to_string()),
                acp_tool_title(""),
            );
            call.status = ToolCallStatus::Completed;
            call.raw_output = output;
            tool_states.insert(
                tool_call_id.to_string(),
                AcpToolProgress::new("done".to_string(), false, true),
            );
            updates.push(SessionUpdate::ToolCall(call));
        }
    }
    updates
}

/// 一帧 delta 事件 → 若干 ACP `session/update`。
///
/// 文本与推理取顶层的 `delta` / `kind`；工具进度取 `streamCache.streamBlocks`——
/// 工具名、参数、结果、状态都在那里，而 `AssistantDeltaEvent` 对 `stream_cache`
/// 标了 `skip_deserializing`，按结构体反序列化会整块丢掉，所以这里直接处理原始 JSON。
///
/// `tool_states` 做按 id 的状态去重：同一调用的 `tool_call` 只发一次，之后只在
/// 状态前进时发 `tool_call_update`。
fn acp_updates_from_event(
    event: &serde_json::Value,
    tool_states: &mut std::collections::HashMap<String, AcpToolProgress>,
) -> Vec<agent_client_protocol::schema::v1::SessionUpdate> {
    use agent_client_protocol::schema::v1::{
        ContentBlock, ContentChunk, SessionUpdate, TextContent,
    };

    let mut updates: Vec<SessionUpdate> = Vec::new();
    let kind = event
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let delta = event
        .get("delta")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    match kind {
        "" if !delta.is_empty() => updates.push(SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::Text(TextContent::new(delta))),
        )),
        "activity_reasoning_delta" if !delta.is_empty() => {
            updates.push(SessionUpdate::AgentThoughtChunk(ContentChunk::new(
                ContentBlock::Text(TextContent::new(delta)),
            )))
        }
        _ => {}
    }

    let Some(blocks) = event
        .get("streamCache")
        .and_then(|cache| cache.get("streamBlocks"))
        .and_then(serde_json::Value::as_array)
    else {
        return acp_tool_updates_from_delta(event, tool_states, updates);
    };
    let mut seen_tool = false;
    for block in blocks {
        let Some(tools) = block.get("tools").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for tool in tools {
            seen_tool = true;
            acp_push_tool_updates_from_block(tool, tool_states, &mut updates);
        }
    }
    if !seen_tool {
        // streamCache 里没有工具块：回退到 delta 字段，保证工具卡片仍能出现。
        return acp_tool_updates_from_delta(event, tool_states, updates);
    }
    updates
}

/// 处理一个 `streamBlocks[].tools[]` 条目的状态推进。
fn acp_push_tool_updates_from_block(
    tool: &serde_json::Value,
    tool_states: &mut std::collections::HashMap<String, AcpToolProgress>,
    updates: &mut Vec<agent_client_protocol::schema::v1::SessionUpdate>,
) {
    use agent_client_protocol::schema::v1::{
        SessionUpdate, ToolCall, ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
    };

    let Some(raw_id) = tool
        .get("toolCallId")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    let status_key = tool
        .get("status")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("doing")
        .to_string();
    let call_status = acp_tool_status(&status_key);
    let result_text = tool
        .get("resultText")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let args_text = tool
        .get("argsText")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    let args_value = acp_raw_json(args_text);
    let result_value = acp_raw_json(result_text);

    match tool_states.get(raw_id).cloned() {
        None => {
            let mut call = ToolCall::new(ToolCallId::new(raw_id.to_string()), acp_tool_title(name));
            if !name.is_empty() {
                call.name = Some(name.to_string());
            }
            call.kind = acp_tool_kind(name);
            call.status = call_status;
            call.raw_input = args_value.clone();
            call.raw_output = result_value.clone();
            tool_states.insert(
                raw_id.to_string(),
                AcpToolProgress::new(status_key, args_value.is_some(), result_value.is_some()),
            );
            updates.push(SessionUpdate::ToolCall(call));
        }
        Some(previous) => {
            let status_changed = previous.status != status_key;
            let args_new = args_value.is_some() && !previous.args_sent;
            let result_new = result_value.is_some() && !previous.result_sent;
            if !status_changed && !args_new && !result_new {
                return;
            }
            let mut fields = ToolCallUpdateFields::default();
            if status_changed {
                fields.status = Some(call_status);
            }
            if args_new {
                fields.raw_input = args_value;
            }
            if result_new {
                fields.raw_output = result_value;
            }
            tool_states.insert(
                raw_id.to_string(),
                AcpToolProgress::new(
                    status_key,
                    previous.args_sent || args_new,
                    previous.result_sent || result_new,
                ),
            );
            updates.push(SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new(raw_id.to_string()),
                fields,
            )));
        }
    }
}

/// `streamCache` 缺席时的工具兜底：delta 事件仍带 `tool_name` / `tool_call_id` /
/// `tool_status`，够发出工具卡片与状态推进。参数（`tool_args`）在主链路从不被填充，
/// 这条路径上拿不到，只能留空。
fn acp_tool_updates_from_delta(
    event: &serde_json::Value,
    tool_states: &mut std::collections::HashMap<String, AcpToolProgress>,
    mut updates: Vec<agent_client_protocol::schema::v1::SessionUpdate>,
) -> Vec<agent_client_protocol::schema::v1::SessionUpdate> {
    use agent_client_protocol::schema::v1::{
        SessionUpdate, ToolCall, ToolCallId, ToolCallUpdate, ToolCallUpdateFields,
    };

    let kind = event
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !matches!(
        kind,
        "assistant_tool_event" | "assistant_tool_result" | "tool_status"
    ) {
        return updates;
    }
    // 结果事件把输出藏在 message 里，先按这条路径处理；处理不了再走状态推导。
    if kind == "assistant_tool_result" {
        if let Some((result_id, content)) = acp_tool_result_payload(event) {
            return acp_apply_tool_result(&result_id, &content, tool_states, updates);
        }
    }
    let tool_name = acp_json_str(event, &["toolName", "tool_name"]).unwrap_or("");
    let raw_id = acp_json_str(event, &["toolCallId", "tool_call_id"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            format!(
                "delta-{}",
                if tool_name.is_empty() { "tool" } else { tool_name }
            )
        });
    let status_text = acp_json_str(event, &["toolStatus", "tool_status"])
        .map(str::trim)
        .unwrap_or("");
    let status_key = if !status_text.is_empty() {
        status_text.to_string()
    } else if kind == "assistant_tool_result" {
        "done".to_string()
    } else {
        "doing".to_string()
    };

    let known = tool_states.get(&raw_id).cloned();
    // 没有工具名的首帧只是「准备调用」的占位，不构成可展示的调用。
    if known.is_none() && tool_name.is_empty() {
        return updates;
    }
    let call_status = acp_tool_status(&status_key);
    // `tool_args` 在主链路当前从不填充，这里仍读一次：将来采集侧补上，参数就自动带出。
    let args_value = acp_raw_json(acp_json_str(event, &["toolArgs", "tool_args"]).unwrap_or(""));
    match known {
        None => {
            let mut call =
                ToolCall::new(ToolCallId::new(raw_id.clone()), acp_tool_title(tool_name));
            call.name = Some(tool_name.to_string());
            call.kind = acp_tool_kind(tool_name);
            call.status = call_status;
            call.raw_input = args_value.clone();
            tool_states.insert(
                raw_id,
                AcpToolProgress::new(status_key, args_value.is_some(), false),
            );
            updates.push(SessionUpdate::ToolCall(call));
        }
        Some(previous) => {
            // 与块路径同一判据：状态没变，但参数是新的，也必须补发。
            // 只看状态是否变化会把「首帧建卡、参数后到」的那一帧整帧丢掉，
            // 卡片参数就永久为空。
            let status_changed = previous.status != status_key;
            let args_new = args_value.is_some() && !previous.args_sent;
            if !status_changed && !args_new {
                return updates;
            }
            let mut fields = ToolCallUpdateFields::default();
            if status_changed {
                fields.status = Some(call_status);
            }
            if args_new {
                fields.raw_input = args_value;
            }
            tool_states.insert(
                raw_id.clone(),
                AcpToolProgress::new(
                    status_key,
                    previous.args_sent || args_new,
                    previous.result_sent,
                ),
            );
            updates.push(SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new(raw_id),
                fields,
            )));
        }
    }
    updates
}

/// 按候选键名取字符串值。事件是 `#[serde(rename_all = "camelCase")]` 序列化出去的，
/// 所以键名是 `toolName` / `toolCallId` / `toolStatus`，这里同时兼容 snake_case。
fn acp_json_str<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
}

fn acp_tool_title(tool_name: &str) -> String {
    let label = match tool_name {
        "exec" => "执行命令",
        "read" => "读取文件",
        "write" => "写入文件",
        "apply_patch" => "修改文件",
        "websearch" | "fetch" => "检索资料",
        "meme" => "发送表情",
        "team_send" | "team_report" => "联系队员",
        "question" => "发起提问",
        _ => "",
    };
    if !label.is_empty() {
        return label.to_string();
    }
    if tool_name.trim().is_empty() {
        "工具调用".to_string()
    } else {
        format!("调用工具 {tool_name}")
    }
}

fn acp_tool_kind(tool_name: &str) -> agent_client_protocol::schema::v1::ToolKind {
    use agent_client_protocol::schema::v1::ToolKind;
    match tool_name {
        "read" | "read_file" | "read_media" => ToolKind::Read,
        "write" | "update" => ToolKind::Edit,
        "delete" => ToolKind::Delete,
        "move" => ToolKind::Move,
        "exec" | "background" => ToolKind::Execute,
        "websearch" | "fetch" | "deeprecall_search" => ToolKind::Search,
        _ => ToolKind::Other,
    }
}

fn acp_tool_status(status: &str) -> agent_client_protocol::schema::v1::ToolCallStatus {
    use agent_client_protocol::schema::v1::ToolCallStatus;
    match status {
        "done" | "completed" | "success" | "succeeded" => ToolCallStatus::Completed,
        "error" | "failed" | "errored" => ToolCallStatus::Failed,
        _ => ToolCallStatus::InProgress,
    }
}

#[cfg(test)]
mod acp_agent_cli_tests {
    use super::*;
    use agent_client_protocol::schema::v1::{SessionConfigKind, SessionConfigSelectOptions};

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn invocation_only_matches_acp_subcommand() {
        assert!(acp_cli_invocation(&argv(&["p-ai", "acp"])));
        assert!(!acp_cli_invocation(&argv(&["p-ai"])));
        assert!(!acp_cli_invocation(&argv(&["p-ai", "acpx"])));
        assert!(!acp_cli_invocation(&argv(&["p-ai", "run", "acp"])));
    }

    #[test]
    fn prompt_text_should_join_text_blocks() {
        use agent_client_protocol::schema::v1::{ContentBlock, ResourceLink, TextContent};
        let blocks = vec![
            ContentBlock::Text(TextContent::new("第一段")),
            ContentBlock::Text(TextContent::new("第二段")),
        ];
        assert_eq!(
            acp_prompt_text(&blocks).expect("纯文本应能解析"),
            "第一段\n\n第二段"
        );

        // 资源链接是协议要求所有 agent 必须支持的类型：转成引用行，不能丢掉。
        let with_link = vec![
            ContentBlock::Text(TextContent::new("看这个文件")),
            ContentBlock::ResourceLink(ResourceLink::new("main.rs", "file:///src/main.rs")),
        ];
        let text = acp_prompt_text(&with_link).expect("资源链接应能解析");
        assert!(
            text.contains("main.rs") && text.contains("file:///src/main.rs"),
            "链接信息要进提示词，实际 {text}"
        );
    }

    /// 不支持的内容块必须明确报错，不许静默丢。
    #[test]
    fn prompt_with_unsupported_block_should_fail_loudly() {
        use agent_client_protocol::schema::v1::{ContentBlock, ImageContent, TextContent};
        let blocks = vec![
            ContentBlock::Text(TextContent::new("这张图是什么")),
            ContentBlock::Image(ImageContent::new("AAAA", "image/png")),
        ];
        let err = acp_prompt_text(&blocks).expect_err("图片块必须被拒");
        assert!(err.contains("图片"), "错误要说明是哪类内容，实际 {err}");
        assert!(
            err.contains("promptCapabilities.image"),
            "错误要说明原因，实际 {err}"
        );
    }

    #[test]
    fn tool_kind_should_map_known_tools() {
        use agent_client_protocol::schema::v1::ToolKind;
        assert_eq!(acp_tool_kind("read"), ToolKind::Read);
        assert_eq!(acp_tool_kind("write"), ToolKind::Edit);
        assert_eq!(acp_tool_kind("exec"), ToolKind::Execute);
        assert_eq!(acp_tool_kind("websearch"), ToolKind::Search);
        assert_eq!(acp_tool_kind("meme"), ToolKind::Other);
    }

    #[test]
    fn text_delta_should_become_message_chunk() {
        let mut states = std::collections::HashMap::new();
        let event = serde_json::json!({ "delta": "你好", "kind": null });
        let updates = acp_updates_from_event(&event, &mut states);
        assert_eq!(updates.len(), 1);
        match &updates[0] {
            agent_client_protocol::schema::v1::SessionUpdate::AgentMessageChunk(chunk) => {
                match &chunk.content {
                    agent_client_protocol::schema::v1::ContentBlock::Text(text) => {
                        assert_eq!(text.text, "你好")
                    }
                    other => panic!("应为文本块，实际 {other:?}"),
                }
            }
            other => panic!("应为 agent_message_chunk，实际 {other:?}"),
        }
    }

    #[test]
    fn reasoning_delta_should_become_thought_chunk() {
        let mut states = std::collections::HashMap::new();
        let event = serde_json::json!({ "delta": "想一想", "kind": "activity_reasoning_delta" });
        let updates = acp_updates_from_event(&event, &mut states);
        assert!(matches!(
            updates.first(),
            Some(agent_client_protocol::schema::v1::SessionUpdate::AgentThoughtChunk(_))
        ));
    }

    #[test]
    fn tool_call_should_be_announced_once_then_updated_on_status_change() {
        let mut states = std::collections::HashMap::new();
        let doing = serde_json::json!({
            "delta": "",
            "streamCache": { "streamBlocks": [{
                "text": "正文",
                "tools": [{
                    "toolCallId": "call-1",
                    "name": "exec",
                    "argsText": "{\"command\":\"echo hi\"}",
                    "status": "doing"
                }]
            }]}
        });
        let first = acp_updates_from_event(&doing, &mut states);
        assert_eq!(first.len(), 1);
        match &first[0] {
            agent_client_protocol::schema::v1::SessionUpdate::ToolCall(call) => {
                assert_eq!(call.name.as_deref(), Some("exec"));
                assert_eq!(call.kind, agent_client_protocol::schema::v1::ToolKind::Execute);
                assert_eq!(call.status, agent_client_protocol::schema::v1::ToolCallStatus::InProgress);
                assert!(call.raw_input.is_some());
            }
            other => panic!("首次应发 tool_call，实际 {other:?}"),
        }

        // 同一状态重复上报不重复发帧。
        let repeat = acp_updates_from_event(&doing, &mut states);
        assert!(repeat.is_empty(), "状态没变就不该再发帧");

        // 状态前进到 done：只发 update。
        let done = serde_json::json!({
            "delta": "",
            "streamCache": { "streamBlocks": [{
                "text": "正文",
                "tools": [{
                    "toolCallId": "call-1",
                    "name": "exec",
                    "argsText": "{\"command\":\"echo hi\"}",
                    "resultText": "hi",
                    "status": "done"
                }]
            }]}
        });
        let advanced = acp_updates_from_event(&done, &mut states);
        assert_eq!(advanced.len(), 1);
        match &advanced[0] {
            agent_client_protocol::schema::v1::SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::schema::v1::ToolCallStatus::Completed)
                );
                assert!(update.fields.raw_output.is_some());
            }
            other => panic!("状态前进应发 tool_call_update，实际 {other:?}"),
        }
    }

    /// 首帧只有工具名（参数与结果都还没到），后到的帧要把它们补发出来。
    /// 这是兜底路径先建卡片、`streamBlocks` 后补齐时真实会发生顺序。
    #[test]
    fn late_args_and_result_should_be_patched_onto_existing_call() {
        use agent_client_protocol::schema::v1::{SessionUpdate, ToolCallStatus};

        let mut states = std::collections::HashMap::new();
        let announce = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-7",
            "toolStatus": "doing"
        });
        match acp_updates_from_event(&announce, &mut states).first() {
            Some(SessionUpdate::ToolCall(call)) => assert!(
                call.raw_input.is_none(),
                "首帧没有参数时不该凭空造一个"
            ),
            other => panic!("首次应发 tool_call，实际 {other:?}"),
        }

        // 参数到了，状态没变：只补 rawInput，不带 status。
        let with_args = serde_json::json!({
            "delta": "",
            "streamCache": { "streamBlocks": [{
                "text": "正文",
                "tools": [{
                    "toolCallId": "call-7",
                    "name": "exec",
                    "argsText": "{\"command\":\"echo acp-tool-ok\"}",
                    "status": "doing"
                }]
            }]}
        });
        match acp_updates_from_event(&with_args, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert!(update.fields.status.is_none(), "状态没变就不该重复上报");
                assert!(update.fields.raw_input.is_some(), "参数该补上");
            }
            other => panic!("参数后到应发 tool_call_update，实际 {other:?}"),
        }

        // 结果与完成态一起到：status 与 rawOutput 同时给出。
        let finished = serde_json::json!({
            "delta": "",
            "streamCache": { "streamBlocks": [{
                "text": "正文",
                "tools": [{
                    "toolCallId": "call-7",
                    "name": "exec",
                    "argsText": "{\"command\":\"echo acp-tool-ok\"}",
                    "resultText": "acp-tool-ok",
                    "status": "done"
                }]
            }]}
        });
        match acp_updates_from_event(&finished, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));
                assert_eq!(
                    update.fields.raw_output,
                    Some(serde_json::Value::String("acp-tool-ok".to_string()))
                );
            }
            other => panic!("完成应发 tool_call_update，实际 {other:?}"),
        }
    }

    /// 结果事件只在 `message` 里带输出，id 与内容都要从那里拆出来补到卡片上。
    #[test]
    fn tool_result_message_should_patch_raw_output_without_repeating_status() {
        use agent_client_protocol::schema::v1::{SessionUpdate, ToolCallStatus};

        let mut states = std::collections::HashMap::new();
        let running = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-7",
            "toolStatus": "running",
            "toolArgs": "{\"command\":\"echo acp-tool-ok\"}",
            "message": "正在调用工具：exec"
        });
        match acp_updates_from_event(&running, &mut states).first() {
            Some(SessionUpdate::ToolCall(call)) => {
                assert_eq!(
                    call.raw_input,
                    Some(serde_json::json!({"command": "echo acp-tool-ok"}))
                );
            }
            other => panic!("首次应发 tool_call，实际 {other:?}"),
        }

        let finished = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-7",
            "toolStatus": "done",
            "toolArgs": "{\"command\":\"echo acp-tool-ok\"}",
            "message": "工具调用完成：exec"
        });
        match acp_updates_from_event(&finished, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));
                assert!(update.fields.raw_output.is_none(), "这时结果还没到");
            }
            other => panic!("完成态应发 tool_call_update，实际 {other:?}"),
        }

        let result = serde_json::json!({
            "delta": "",
            "kind": "assistant_tool_result",
            "message": serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-7",
                "content": "Exit code: 0\nOutput:\nacp-tool-ok\n",
                "metadata": { "exit_code": 0 }
            })
            .to_string()
        });
        match acp_updates_from_event(&result, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert!(
                    update.fields.status.is_none(),
                    "状态已经报过 completed，不该重复上报"
                );
                match update.fields.raw_output.as_ref() {
                    Some(serde_json::Value::String(text)) => {
                        assert!(text.contains("acp-tool-ok"), "输出该原样带出，实际 {text}")
                    }
                    other => panic!("rawOutput 应为原始输出文本，实际 {other:?}"),
                }
            }
            other => panic!("结果事件应发 tool_call_update，实际 {other:?}"),
        }
    }

    /// `streamCache` 缺席时（主链路当前就是这么跑的），工具卡片与状态推进仍要成立。
    #[test]
    fn tool_event_without_stream_cache_should_still_emit_call_and_complete() {
        use agent_client_protocol::schema::v1::{SessionUpdate, ToolCallStatus};

        let mut states = std::collections::HashMap::new();
        let running = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "tool_name": "exec",
            "tool_call_id": "call-9",
            "tool_status": "running"
        });
        let first = acp_updates_from_event(&running, &mut states);
        match first.first() {
            Some(SessionUpdate::ToolCall(call)) => {
                assert_eq!(call.name.as_deref(), Some("exec"));
                assert_eq!(call.status, ToolCallStatus::InProgress);
            }
            other => panic!("首次应发 tool_call，实际 {other:?}"),
        }

        let done = serde_json::json!({
            "delta": "",
            "kind": "assistant_tool_result",
            "tool_name": "exec",
            "tool_call_id": "call-9"
        });
        let advanced = acp_updates_from_event(&done, &mut states);
        match advanced.first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));
            }
            other => panic!("工具结束后应发 completed，实际 {other:?}"),
        }
    }

    /// 没有工具名的「准备调用」占位不该产出空卡片。
    #[test]
    fn nameless_tool_placeholder_should_not_emit_call() {
        let mut states = std::collections::HashMap::new();
        let placeholder = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "tool_status": "running"
        });
        assert!(acp_updates_from_event(&placeholder, &mut states).is_empty());
    }

    // ==================== 模型选项与切换 ====================

    /// 构造一份测试配置。聊天格式传 `RequestFormat::OpenAI`；
    /// 语音、嵌入这类非聊天格式用来验证过滤。
    fn acp_test_config(entries: &[(&str, &str, &str, RequestFormat)]) -> AppConfig {
        let mut config = AppConfig::default();
        config.api_configs = entries
            .iter()
            .map(|(id, name, model, format)| ApiConfig {
                id: (*id).to_string(),
                name: (*name).to_string(),
                model: (*model).to_string(),
                request_format: *format,
                ..Default::default()
            })
            .collect();
        config
    }

    fn acp_test_selected_value(option: &SessionConfigOption) -> String {
        match &option.kind {
            SessionConfigKind::Select(select) => select.current_value.0.to_string(),
            other => panic!("模型选项应是下拉，实际 {other:?}"),
        }
    }

    fn acp_test_select_values(option: &SessionConfigOption) -> Vec<String> {
        match &option.kind {
            SessionConfigKind::Select(select) => match &select.options {
                SessionConfigSelectOptions::Ungrouped(items) => {
                    items.iter().map(|item| item.value.0.to_string()).collect()
                }
                other => panic!("模型选项应是不分组列表，实际 {other:?}"),
            },
            other => panic!("模型选项应是下拉，实际 {other:?}"),
        }
    }

    #[test]
    fn model_options_should_list_every_config_with_current_value() {
        let config = acp_test_config(&[
            ("a", "配置甲", "m-a", RequestFormat::OpenAI),
            ("b", "配置乙", "m-b", RequestFormat::OpenAI),
            ("c", "", "m-c", RequestFormat::OpenAI),
        ]);
        let options = acp_model_config_options(&config, "b").expect("应能列举选项");
        assert_eq!(options.len(), 1, "当前只报模型这一项");
        let option = &options[0];
        assert_eq!(option.id.0.to_string(), ACP_MODEL_CONFIG_ID);
        assert_eq!(option.category, Some(SessionConfigOptionCategory::Model));
        assert_eq!(
            acp_test_select_values(option),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "全部配置都要在选项里，且保持配置顺序"
        );
        assert_eq!(acp_test_selected_value(option), "b", "当前值应指向会话所用模型");

        // 配置名为空时退到模型标识，不留空显示名。
        match &option.kind {
            SessionConfigKind::Select(select) => match &select.options {
                SessionConfigSelectOptions::Ungrouped(items) => {
                    assert_eq!(items[0].name, "配置甲");
                    assert_eq!(items[2].name, "m-c");
                }
                other => panic!("模型选项应是不分组列表，实际 {other:?}"),
            },
            other => panic!("模型选项应是下拉，实际 {other:?}"),
        }
    }

    #[test]
    fn model_switch_should_move_conversation_to_requested_model() {
        let config = acp_test_config(&[
            ("a", "配置甲", "m-a", RequestFormat::OpenAI),
            ("b", "配置乙", "m-b", RequestFormat::OpenAI),
        ]);
        let (next, options) = acp_apply_model_switch(&config, "b").expect("合法 id 应被接受");
        assert_eq!(next, "b");
        assert_eq!(
            acp_test_selected_value(&options[0]),
            "b",
            "回给客户端的当前值要指向新模型"
        );
    }

    #[test]
    fn model_switch_should_reject_unknown_config_and_keep_session_model() {
        let config = acp_test_config(&[("a", "配置甲", "m-a", RequestFormat::OpenAI)]);
        let err = acp_apply_model_switch(&config, "nope").expect_err("不存在的 id 必须被拒");
        assert!(err.contains("nope"), "错误里要能看出是哪个 id，实际 {err}");

        // 拒绝路径不产出新模型；会话继续停在原模型上。
        let options = acp_model_config_options(&config, "a").expect("仍能列举选项");
        assert_eq!(acp_test_selected_value(&options[0]), "a");
    }

    /// 语音、嵌入、重排不是聊天模型：既不进下拉，也不许切过去。
    #[test]
    fn model_options_should_skip_non_chat_configs() {
        let config = acp_test_config(&[
            ("chat-1", "聊天", "m-chat", RequestFormat::OpenAI),
            ("stt-1", "语音识别", "m-stt", RequestFormat::OpenAIStt),
            ("emb-1", "嵌入", "m-emb", RequestFormat::OpenAIEmbedding),
        ]);
        let options = acp_model_config_options(&config, "chat-1").expect("应能列举选项");
        assert_eq!(
            acp_test_select_values(&options[0]),
            vec!["chat-1".to_string()],
            "只有聊天模型能进下拉"
        );

        let err = acp_apply_model_switch(&config, "stt-1").expect_err("语音配置不能当聊天模型");
        assert!(err.contains("文本聊天"), "错误要说明原因，实际 {err}");
    }

    /// cwd 用例的临时目录：名字带进程 id 与标签，避免并行测试相互踩踏。
    fn acp_test_temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pai-acp-cwd-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("应能创建临时目录");
        dir
    }

    fn acp_test_error_code(err: &agent_client_protocol::Error) -> i32 {
        err.code.into()
    }

    /// `session/new` 的 cwd 指向文件时必须被拒。
    ///
    /// `canonicalize` 对文件也成功，少一道 `is_dir()` 就会让会话照建、日志照写「完成」，
    /// 工具却静默跑回默认工作区——看着成功，地方不对。
    #[test]
    fn new_session_should_reject_file_cwd_with_invalid_params() {
        use agent_client_protocol::schema::v1::ErrorCode;

        let dir = acp_test_temp_dir("file-cwd");
        let file = dir.join("not-a-dir.txt");
        std::fs::write(&file, b"marker").expect("应能写临时文件");

        let err = acp_new_session_workspaces(&file).expect_err("cwd 传文件必须被拒");
        assert_eq!(
            err.code,
            ErrorCode::InvalidParams,
            "参数问题不该报成内部错误"
        );
        assert_eq!(
            acp_test_error_code(&err),
            -32602,
            "客户端按 -32602 判断参数错误"
        );
        assert!(
            err.message.contains("不是目录"),
            "错误要说明原因，实际 {}",
            err.message
        );

        // 同一路径按目录传仍然通过，防止校验写过头。
        let workspaces = acp_new_session_workspaces(&dir).expect("合法目录应通过");
        assert_eq!(workspaces.len(), 1, "只映射一个主工作区");
        assert_eq!(workspaces[0].level, SHELL_WORKSPACE_LEVEL_MAIN);

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// cwd 指向不存在的目录时同样归到参数错误。
    #[test]
    fn new_session_should_reject_missing_cwd_with_invalid_params() {
        use agent_client_protocol::schema::v1::ErrorCode;

        let dir = acp_test_temp_dir("missing-cwd");
        let missing = dir.join("no-such-subdir");

        let err = acp_new_session_workspaces(&missing).expect_err("目录不存在必须被拒");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert_eq!(acp_test_error_code(&err), -32602);
        assert!(
            err.message.contains("解析会话工作目录失败"),
            "错误要指出解析失败，实际 {}",
            err.message
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn acp_test_registry(entries: &[(&str, bool)]) -> AcpSessionRegistry {
        let mut map = std::collections::HashMap::new();
        for (id, in_flight) in entries {
            map.insert(
                (*id).to_string(),
                AcpSessionState {
                    conversation_id: (*id).to_string(),
                    agent_id: "agent-1".to_string(),
                    model_config_id: "m-a".to_string(),
                    prompt_in_flight: *in_flight,
                    turn: None,
                },
            );
        }
        std::sync::Arc::new(std::sync::Mutex::new(map))
    }

    /// 工具已经报过失败后，结果事件只许补输出，不许把状态改回成功。
    #[test]
    fn failed_tool_status_should_not_be_rewritten_to_completed_by_result() {
        use agent_client_protocol::schema::v1::{SessionUpdate, ToolCallStatus};

        let mut states = std::collections::HashMap::new();
        let running = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-9",
            "toolStatus": "running",
            "toolArgs": "{\"command\":\"exit 1\"}",
            "message": "正在调用工具：exec"
        });
        assert!(matches!(
            acp_updates_from_event(&running, &mut states).first(),
            Some(SessionUpdate::ToolCall(_))
        ));

        let failed = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-9",
            "toolStatus": "failed",
            "message": "工具调用失败：exec"
        });
        match acp_updates_from_event(&failed, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => assert_eq!(
                update.fields.status,
                Some(ToolCallStatus::Failed),
                "失败状态要先发出去"
            ),
            other => panic!("失败应发 tool_call_update，实际 {other:?}"),
        }

        let result = serde_json::json!({
            "delta": "",
            "kind": "assistant_tool_result",
            "message": serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-9",
                "content": "Exit code: 1\nOutput:\nboom\n",
                "metadata": { "exit_code": 1 }
            })
            .to_string()
        });
        match acp_updates_from_event(&result, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert!(
                    update.fields.status.is_none(),
                    "失败是终态，结果事件不能把状态改回成功"
                );
                assert!(update.fields.raw_output.is_some(), "输出仍要补上");
            }
            other => panic!("结果应发 tool_call_update，实际 {other:?}"),
        }
    }

    /// 首帧无参数建卡后，状态没变的参数补齐帧同样要发出去。
    #[test]
    fn delta_tool_args_should_be_patched_when_status_unchanged() {
        use agent_client_protocol::schema::v1::SessionUpdate;

        let mut states = std::collections::HashMap::new();
        let first = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-8",
            "toolStatus": "running",
            "message": "正在调用工具：exec"
        });
        match acp_updates_from_event(&first, &mut states).first() {
            Some(SessionUpdate::ToolCall(call)) => {
                assert!(call.raw_input.is_none(), "这一帧还没有参数")
            }
            other => panic!("首帧应发 tool_call，实际 {other:?}"),
        }

        let with_args = serde_json::json!({
            "delta": "",
            "kind": "tool_status",
            "toolName": "exec",
            "toolCallId": "call-8",
            "toolStatus": "running",
            "toolArgs": "{\"command\":\"echo hi\"}",
            "message": "正在调用工具：exec"
        });
        match acp_updates_from_event(&with_args, &mut states).first() {
            Some(SessionUpdate::ToolCallUpdate(update)) => {
                assert!(update.fields.status.is_none(), "状态没变不该重复上报");
                assert_eq!(
                    update.fields.raw_input,
                    Some(serde_json::json!({"command": "echo hi"})),
                    "参数该补到卡片上"
                );
            }
            other => panic!("参数后到应发 tool_call_update，实际 {other:?}"),
        }

        assert!(
            acp_updates_from_event(&with_args, &mut states).is_empty(),
            "同样的内容再来一帧不该重发"
        );
    }

    /// 全局选中语音/嵌入配置时，新会话要退到第一条聊天配置。
    #[test]
    fn session_model_should_skip_non_chat_selected_config() {
        let mut config = acp_test_config(&[
            ("stt-1", "语音", "m-stt", RequestFormat::OpenAIStt),
            ("chat-1", "聊天", "m-chat", RequestFormat::OpenAI),
        ]);
        config.selected_api_config_id = "stt-1".to_string();
        assert_eq!(
            acp_pick_session_model_config_id(&config).expect("应能选出聊天模型"),
            "chat-1",
            "全局选到非聊天配置时，新会话不能锁到它上面"
        );

        config.selected_api_config_id = "chat-1".to_string();
        assert_eq!(
            acp_pick_session_model_config_id(&config).expect("当前就是聊天配置"),
            "chat-1"
        );

        let only_stt = acp_test_config(&[("stt-1", "语音", "m-stt", RequestFormat::OpenAIStt)]);
        let err = acp_pick_session_model_config_id(&only_stt).expect_err("没有聊天模型要报错");
        assert!(err.contains("聊天模型"), "错误要说明原因，实际 {err}");
    }

    /// `currentValue` 必须落在 `options` 里，否则客户端下拉显示空白。
    #[test]
    fn model_options_should_reject_current_value_outside_options() {
        let config = acp_test_config(&[
            ("chat-1", "聊天", "m-chat", RequestFormat::OpenAI),
            ("stt-1", "语音", "m-stt", RequestFormat::OpenAIStt),
        ]);
        let err =
            acp_model_config_options(&config, "stt-1").expect_err("当前值必须落在选项里");
        assert!(err.contains("不在可选范围"), "错误要说明原因，实际 {err}");
    }

    /// 未知会话与中断的会话表是两件事：前者是参数错，后者才是内部故障。
    #[test]
    fn prompt_claim_should_tell_unknown_session_from_broken_registry() {
        let registry = acp_test_registry(&[("s-1", false)]);
        match acp_claim_prompt_slot(&registry, "nope") {
            AcpPromptClaim::Unknown => {}
            other => panic!("未知会话应判为 Unknown，实际 {other:?}"),
        }

        // 另起线程持锁后 panic，把互斥量打成中毒状态。
        let poisoned = acp_test_registry(&[]);
        let clone = poisoned.clone();
        let _ = std::thread::spawn(move || {
            let _guard = clone.lock().expect("首次加锁应成功");
            panic!("故意让互斥量中毒");
        })
        .join();
        match acp_claim_prompt_slot(&poisoned, "s-1") {
            AcpPromptClaim::Unavailable => {}
            other => panic!("锁中毒应判为 Unavailable，实际 {other:?}"),
        }
    }

    /// 同一会话并发两帧 prompt：只跑一轮，第二帧立即被拒；本轮结束后又能进。
    #[test]
    fn second_prompt_on_same_session_should_be_rejected_until_released() {
        let registry = acp_test_registry(&[("s-1", false)]);

        let session = match acp_claim_prompt_slot(&registry, "s-1") {
            AcpPromptClaim::Ready(session) => session,
            other => panic!("第一帧应拿到占位，实际 {other:?}"),
        };
        let slot = AcpPromptSlot {
            sessions: registry.clone(),
            conversation_id: session.conversation_id.clone(),
        };

        match acp_claim_prompt_slot(&registry, "s-1") {
            AcpPromptClaim::Busy => {}
            other => panic!("第二帧应被判为在途，实际 {other:?}"),
        }

        drop(slot);

        match acp_claim_prompt_slot(&registry, "s-1") {
            AcpPromptClaim::Ready(_) => {}
            other => panic!("上一轮结束后应能再次占位，实际 {other:?}"),
        }
    }

    /// 没有在途轮次时收到 cancel：安静丢弃，不报错、不留下任何标记。
    #[test]
    fn cancel_without_running_turn_should_be_dropped_silently() {
        let registry = acp_test_registry(&[("s-1", false)]);

        let result = acp_handle_cancel_notification(
            &registry,
            agent_client_protocol::schema::v1::CancelNotification::new(
                agent_client_protocol::schema::v1::SessionId::new("s-1"),
            ),
        );
        assert!(result.is_ok(), "通知没有响应帧，必须安静收下");
        let turn = acp_current_turn(&registry, "s-1").expect("读取会话表");
        assert!(turn.is_none(), "没有轮次时不应凭空造出取消控制");

        // 会话根本不存在也一样：属于编辑器的正常竞态，不能回任何东西。
        let unknown = acp_handle_cancel_notification(
            &registry,
            agent_client_protocol::schema::v1::CancelNotification::new(
                agent_client_protocol::schema::v1::SessionId::new("nope"),
            ),
        );
        assert!(unknown.is_ok(), "未知会话的 cancel 也要安静丢弃");
    }

    /// 有在途轮次时收到 cancel：只拍中那一轮的身份，信号落到它的 token 上。
    #[test]
    fn cancel_should_hit_the_running_turn_token() {
        let registry = acp_test_registry(&[("s-1", false)]);
        let session = match acp_claim_prompt_slot(&registry, "s-1") {
            AcpPromptClaim::Ready(session) => session,
            other => panic!("第一帧应拿到占位，实际 {other:?}"),
        };
        let turn = acp_current_turn(&registry, "s-1")
            .expect("读取会话表")
            .expect("占位应登记本轮的取消控制");
        assert!(!turn.token.is_cancelled(), "占位时不应已经带取消信号");

        acp_handle_cancel_notification(
            &registry,
            agent_client_protocol::schema::v1::CancelNotification::new(
                agent_client_protocol::schema::v1::SessionId::new("s-1"),
            ),
        )
        .expect("cancel 应被受理");
        assert!(turn.token.is_cancelled(), "在途轮次必须收到取消信号");

        // 会话本身还在，占位仍由这一轮持有，直到它自己结束。
        assert!(
            matches!(
                acp_claim_prompt_slot(&registry, "s-1"),
                AcpPromptClaim::Busy
            ),
            "取消不等于结束，本轮收尾前仍是占位状态"
        );
        let _ = session;
    }

    /// 轮次结束后取消控制必须被清走：迟到的 cancel 不能落进下一轮。
    #[test]
    fn cancel_after_turn_end_should_not_leak_into_next_turn() {
        let registry = acp_test_registry(&[("s-1", false)]);
        let session = match acp_claim_prompt_slot(&registry, "s-1") {
            AcpPromptClaim::Ready(session) => session,
            other => panic!("第一帧应拿到占位，实际 {other:?}"),
        };
        let first_turn = acp_current_turn(&registry, "s-1")
            .expect("读取会话表")
            .expect("占位应登记本轮的取消控制");
        let first_id = first_turn.turn_id.clone();

        let slot = AcpPromptSlot {
            sessions: registry.clone(),
            conversation_id: session.conversation_id.clone(),
        };
        drop(slot);

        assert!(
            acp_current_turn(&registry, "s-1").expect("读取会话表").is_none(),
            "轮次结束要清掉取消控制，否则迟到的 cancel 会打到下一轮"
        );
        acp_handle_cancel_notification(
            &registry,
            agent_client_protocol::schema::v1::CancelNotification::new(
                agent_client_protocol::schema::v1::SessionId::new("s-1"),
            ),
        )
        .expect("轮次已结束的 cancel 也该安静收下");
        assert!(
            !first_turn.token.is_cancelled(),
            "已经结束的那一轮不应被迟到的 cancel 影响"
        );

        let next_turn = match acp_claim_prompt_slot(&registry, "s-1") {
            AcpPromptClaim::Ready(session) => acp_current_turn(&registry, &session.conversation_id)
                .expect("读取会话表")
                .expect("新一轮应登记自己的取消控制"),
            other => panic!("上一轮结束后应能再次占位，实际 {other:?}"),
        };
        assert_ne!(next_turn.turn_id, first_id, "每一轮的身份必须不同");
        assert!(!next_turn.token.is_cancelled(), "新一轮不应继承上一轮的取消信号");
    }

    /// 回放只还原正文：用户与助理各归各的块，工具消息与空正文不产生帧。
    #[test]
    fn replay_update_should_render_body_text_by_role() {
        use agent_client_protocol::schema::v1::{SessionId, SessionUpdate};

        let session_id = SessionId::new("s-1");
        let text = |value: &str| MessagePart::Text {
            text: value.to_string(),
            reasoning_content: None,
        };

        let user = acp_replay_update("user", &[text("第一问"), text("补充")], &session_id)
            .expect("用户正文要回放");
        assert!(
            matches!(user.update, SessionUpdate::UserMessageChunk(ref chunk)
                if matches!(&chunk.content,
                    agent_client_protocol::schema::v1::ContentBlock::Text(item)
                        if item.text == "第一问\n\n补充")),
            "多段正文要按顺序拼起来"
        );

        let assistant = acp_replay_update("assistant", &[text("回答")], &session_id)
            .expect("助理正文要回放");
        assert!(
            matches!(
                assistant.update,
                SessionUpdate::AgentMessageChunk(ref chunk)
                    if matches!(&chunk.content,
                        agent_client_protocol::schema::v1::ContentBlock::Text(item)
                            if item.text == "回答")
            ),
            "助理正文必须走 AgentMessageChunk"
        );

        // 工具消息与空正文没有可回放的正文，不能凭空造帧。
        assert!(acp_replay_update("tool", &[text("工具输出")], &session_id).is_none());
        assert!(acp_replay_update("assistant", &[text("   ")], &session_id).is_none());
        assert!(acp_replay_update("user", &[], &session_id).is_none());
    }

    /// 编辑器带来的 stdio MCP 定义要原样翻成 PAI 的 definition JSON。
    #[test]
    fn session_mcp_servers_should_translate_stdio_definition() {
        use agent_client_protocol::schema::v1::{McpServer, McpServerStdio};

        let servers = vec![McpServer::Stdio(
            McpServerStdio::new("probe", "node").args(vec!["server.js".to_string()]),
        )];
        let (configs, errors) = acp_session_mcp_servers("a6c82ebd-1b69", &servers);
        assert!(errors.is_empty(), "stdio 定义不该有转换错误：{errors:?}");
        assert_eq!(configs.len(), 1, "一条 server 定义对应一条配置");

        let parsed: serde_json::Value =
            serde_json::from_str(&configs[0].definition_json).expect("definition 必须是合法 JSON");
        let member = &parsed["mcpServers"]["probe"];
        assert_eq!(member["command"], "node", "command 要原样带上");
        assert_eq!(member["args"][0], "server.js", "args 要原样带上");
        assert!(configs[0].enabled, "会话级服务器默认可用");
        assert!(
            configs[0].id.starts_with("acp-"),
            "会话级服务器 id 要有独立前缀，避免撞上磁盘上的服务器"
        );
    }
}
