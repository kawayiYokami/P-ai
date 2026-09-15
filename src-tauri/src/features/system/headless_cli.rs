// ==================== 无头 CLI 子命令 ====================
//
// `p-ai run "<task>" --model <api_config_id> [--minimal-tools]`
//
// 不建 Tauri 窗口，直接构造 headless AppState 跑完一轮对话，
// 把最后一条非空 assistant 文本写 stdout。

const HEADLESS_CLI_SUBCOMMAND: &str = "run";

/// CLI 模式下允许挂载的编程用内置工具白名单。
const HEADLESS_CLI_PROGRAMMING_TOOLS: &[&str] = &[
    "exec",
    "background",
    "read",
    "write",
    "update",
    "delete",
    "move",
    "plan",
    "todo",
];

#[derive(Clone, Copy)]
struct HeadlessCliToolPreset {
    /// true 表示只挂载编程用内置工具白名单。
    minimal: bool,
}

/// 进程级 CLI 工具预设。只有 CLI 入口会写入；桌面路径从不写入，行为不变。
static HEADLESS_CLI_TOOL_PRESET: std::sync::OnceLock<HeadlessCliToolPreset> =
    std::sync::OnceLock::new();

/// CLI 模式开关：只有 CLI 入口会置位，桌面路径恒为 false。
static HEADLESS_CLI_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// CLI 模式下运行时日志不再写 stderr（仍写入后端日志文件），stderr 只留给错误信息。
fn headless_cli_console_logging_suppressed() -> bool {
    HEADLESS_CLI_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// 进入 CLI 模式：在初始化日志与运行时之前调用，确保启动期日志也不落到 stderr。
fn headless_cli_enter_mode(minimal_tools: bool) {
    HEADLESS_CLI_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = HEADLESS_CLI_TOOL_PRESET.set(HeadlessCliToolPreset { minimal: minimal_tools });
}

/// 工具策略层的接入点：CLI 模式下拒绝委托工具；最小档再收敛到白名单。
fn headless_cli_tool_denied_reason(tool_name: &str) -> Option<String> {
    headless_cli_tool_denied_reason_for(HEADLESS_CLI_TOOL_PRESET.get().copied(), tool_name)
}

fn headless_cli_tool_denied_reason_for(
    preset: Option<HeadlessCliToolPreset>,
    tool_name: &str,
) -> Option<String> {
    let preset = preset?;
    let tool_name = tool_name.trim();
    if tool_name == "delegate" {
        return Some("CLI 模式未启用委托工具".to_string());
    }
    if preset.minimal && !HEADLESS_CLI_PROGRAMMING_TOOLS.contains(&tool_name) {
        return Some(format!("CLI 最小工具集模式未启用工具 `{tool_name}`"));
    }
    None
}

struct HeadlessCliArgs {
    task: String,
    model_config_id: String,
    minimal_tools: bool,
}

/// 判断本次调用是否是无头 CLI 子命令（`p-ai run ...`）。
fn headless_cli_invocation(args: &[String]) -> bool {
    args.get(1).map(|value| value == HEADLESS_CLI_SUBCOMMAND).unwrap_or(false)
}

fn parse_headless_cli_args(args: &[String]) -> Result<HeadlessCliArgs, String> {
    let mut model_config_id = String::new();
    let mut minimal_tools = false;
    let mut task_parts: Vec<String> = Vec::new();
    let mut iter = args.iter().skip(2);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--model" => {
                let value = iter.next().ok_or_else(|| "--model 缺少取值".to_string())?;
                if value.trim().is_empty() {
                    return Err("--model 取值不能为空".to_string());
                }
                model_config_id = value.trim().to_string();
            }
            "--minimal-tools" => minimal_tools = true,
            other if other.starts_with("--") => {
                return Err(format!("未知参数: {other}"));
            }
            other => task_parts.push(other.to_string()),
        }
    }
    let task = task_parts.join(" ");
    if task.trim().is_empty() {
        return Err("缺少任务文本，用法：p-ai run \"<task>\" --model <api_config_id>".to_string());
    }
    if model_config_id.is_empty() {
        return Err("缺少 --model，用法：p-ai run \"<task>\" --model <api_config_id>".to_string());
    }
    Ok(HeadlessCliArgs {
        task,
        model_config_id,
        minimal_tools,
    })
}

/// CLI 入口：返回进程退出码。正常结束 0，否则 1。
fn run_headless_cli(parsed: HeadlessCliArgs) -> i32 {
    init_backend_file_logging();
    let runtime = match install_tauri_async_runtime() {
        Ok(runtime) => runtime,
        Err(err) => return headless_cli_fail(&err),
    };
    // 调度 future 体量大，必须投递到运行时工作线程（8MB 栈）上轮询；
    // 直接 block_on 会在主线程默认栈上轮询并栈溢出。
    let join = runtime.spawn(headless_cli_run_once(parsed));
    match runtime.block_on(join) {
        Ok(Ok(())) => 0,
        Ok(Err(err)) => headless_cli_fail(&err),
        Err(err) => headless_cli_fail(&format!("无头运行任务异常: {err}")),
    }
}

fn headless_cli_fail(message: &str) -> i32 {
    use std::io::Write as _;
    // main.rs 把 eprintln! 重定义成了 runtime_log_info，这里必须显式写 stderr。
    let _ = writeln!(std::io::stderr(), "{message}");
    1
}

/// CLI 会话的主工作区：取调用方进程的当前目录，让模型在任务目录内读写。
/// 路径随后会经面向用户输入的规范化链（含 trim），因此这里先 canonicalize，
/// 并拒绝首尾带空白的路径，避免文件系统来源的路径被静默改写后落到别处。
/// 无头环境没有审批入口，故直接给 full_access；可写范围仍是授权工作区集合（该目录 + 配置里的助理工作区）。
fn headless_cli_shell_workspaces() -> Result<Vec<ShellWorkspaceConfig>, String> {
    let raw = std::env::current_dir().map_err(|err| format!("读取当前工作目录失败: {err}"))?;
    let current_dir = raw
        .canonicalize()
        .map_err(|err| format!("解析当前工作目录失败 ({}): {err}", raw.display()))?;
    let path_text = current_dir.to_string_lossy().into_owned();
    if path_text.trim() != path_text.as_str() {
        return Err(format!(
            "当前工作目录路径首尾包含空白，无法作为工作区: {path_text}"
        ));
    }
    Ok(vec![ShellWorkspaceConfig {
        id: "cli-main-workspace".to_string(),
        name: shell_workspace_display_name_fallback(&current_dir),
        path: terminal_path_for_user(&current_dir),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        built_in: false,
    }])
}

async fn headless_cli_run_once(args: HeadlessCliArgs) -> Result<(), String> {
    let locked_model_id = args.model_config_id.trim().to_string();
    let state = AppState::new()?;
    // 无头入口不走前端就绪回调，必须在此主动加载工作区：否则 skills 目录不铺、
    // 快照缓存为空、prompt 里没有 skill 索引，MCP 也不会启动。
    let workspace_result = load_workspace(&state)
        .await
        .map_err(|err| format!("加载工作区（skill/MCP）失败: {err}"))?;
    log_workspace_load_result("[工作区加载]", &workspace_result);
    let runtime_org = load_runtime_organization_snapshot(&state)?;
    let app_config = runtime_org.config.clone();
    if !app_config
        .api_configs
        .iter()
        .any(|api| api.id == locked_model_id)
    {
        return Err(format!("指定的模型配置不存在: {locked_model_id}"));
    }
    let agent_id = state_service_get_assistant_agent_id(&state)?;
    if !runtime_org
        .agents
        .iter()
        .any(|agent| agent.id == agent_id && !agent.is_built_in_user)
    {
        return Err(format!("主助理人格不可用: agent_id={agent_id}"));
    }

    // 等到这里再等 MCP 探测落定：等待只需早于首轮工具装配，不必早于参数校验，
    // 否则 --model 写错这种常见误操作也要先付出整个探测耗时。
    headless_cli_wait_mcp_ready(&state).await;

    let title: String = args.task.chars().take(30).collect();
    let conversation = create_unarchived_conversation_inner(
        CreateUnarchivedConversationInput {
            api_config_id: Some(locked_model_id.clone()),
            agent_id: Some(agent_id.clone()),
            title: Some(title),
            copy_source_conversation_id: None,
            shell_workspaces: Some(headless_cli_shell_workspaces()?),
            shell_work_mode: None,
            shell_work_branch: None,
            shell_autonomous_mode: None,
            is_draft: None,
        },
        &state,
    )
    .await?;
    let conversation_id = conversation.conversation_id;

    let request_id = format!("cli-{}", Uuid::new_v4());
    let user_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text {
            text: args.task.clone(),
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
        .await?;

    let mut runtime_context = runtime_context_new("cli_headless", "cli_run");
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(request_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.clone());
    runtime_context.target_conversation_id = Some(conversation_id.clone());
    runtime_context.root_conversation_id = Some(conversation_id.clone());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.model_config_id = Some(locked_model_id.clone());

    let request = SendChatRequest {
        trigger_only: true,
        session: Some(SessionSelector {
            api_config_id: Some(locked_model_id),
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
        trace_id: Some(request_id),
        assistant_message_id: None,
        oldest_queue_created_at: None,
        remote_im_activation_sources: Vec::new(),
        runtime_context: Some(runtime_context),
    };

    let channel = tauri::ipc::Channel::<AssistantDeltaEvent>::new(|_| Ok(()));
    let result = send_chat_message_inner(request, &state, &channel).await?;
    let final_text = if result.final_response_text.trim().is_empty() {
        result.assistant_text
    } else {
        result.final_response_text
    };
    // 先落 stdout 再等后台：调用方稍后用 timeout 收尾时，回答已经交付出去。
    println!("{}", final_text.trim());
    headless_cli_wait_background_tasks(&state, &conversation_id).await;
    Ok(())
}

/// 等待已启用 MCP 服务器的探测落定。
///
/// `load_workspace` 只负责派发探测（内部 detached spawn），返回时运行态仍停在 `starting`；
/// 而工具装配仅在 schema 缓存为空时才重建，内置工具又保证缓存非空，所以那一轮会固定拿不到
/// MCP 工具。CLI 只有一轮对话，必须在发首轮前等到探测完成。
///
/// 启用清单只取一次：探测是否落定只由运行态决定，没必要每轮重扫目录并重解析 JSON
/// （那会在存在坏 definition 文件时被 200ms 轮询放大成成百条重复告警）。
///
/// 兜底上限：探测自身有连接(30s)/请求(60s)超时，但存在「探测任务提前 return 而运行态停在
/// `starting`」的路径（例如探测期间该服务器的 definition 被改写），届时状态可能永远不落终态。
/// 所以这里设一个上限，超过只记警告并继续，让模型带着非 MCP 工具作答，而不是把整个 CLI 挂死。
const HEADLESS_CLI_MCP_READY_WAIT_LIMIT: std::time::Duration =
    std::time::Duration::from_secs(120);

async fn headless_cli_wait_mcp_ready(state: &AppState) {
    let enabled_ids: Vec<String> = match load_workspace_mcp_servers(state) {
        Ok(servers) => servers
            .into_iter()
            .filter(|server| server.enabled)
            .map(|server| server.id)
            .collect(),
        Err(err) => {
            runtime_log_warn(format!(
                "[工作区加载] 读取 MCP 服务器清单失败，跳过探测等待：{err}"
            ));
            Vec::new()
        }
    };

    let started = std::time::Instant::now();
    let mut logged_pending = false;
    while enabled_ids.iter().any(|id| {
        mcp_runtime_state_get(id)
            .map(|runtime| runtime.last_status == "starting")
            .unwrap_or(false)
    }) {
        if !logged_pending {
            runtime_log_info("[工作区加载] 等待 MCP 探测落定".to_string());
            logged_pending = true;
        }
        if started.elapsed() >= HEADLESS_CLI_MCP_READY_WAIT_LIMIT {
            runtime_log_warn(format!(
                "[工作区加载] 等待 MCP 探测超过上限，继续执行（本轮可能缺少 MCP 工具），elapsed_ms={}",
                started.elapsed().as_millis()
            ));
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    // 探测完成时的 refresh 与状态翻转之间存在极短竞态，这里显式刷新一次，
    // 确保首轮装配拿到的 schema 缓存已包含 MCP 工具。
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);
}

/// 收尾清账：等本会话仍在运行的后台 shell 任务自然结束。
///
/// 不设超时、不 kill：限时由调用方控制（外层 timeout）。常驻服务（如 dev server）
/// 会一直等下去，这是预期行为，CLI 不替调用方做主。
async fn headless_cli_wait_background_tasks(state: &AppState, conversation_id: &str) {
    let conversation_id = conversation_id.trim();
    loop {
        let running = {
            let tasks = state.terminal_background_shell_tasks.lock().await;
            tasks
                .values()
                .filter(|task| task.conversation_id.trim() == conversation_id)
                .filter(|task| match task.status.lock() {
                    Ok(status) => !terminal_background_shell_is_terminal(*status),
                    // 状态锁损坏时不再等待，避免无意义地挂住进程。
                    Err(_) => false,
                })
                .count()
        };
        if running == 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

#[cfg(test)]
mod headless_cli_tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn invocation_only_matches_run_subcommand() {
        assert!(headless_cli_invocation(&argv(&["p-ai", "run", "hi"])));
        assert!(!headless_cli_invocation(&argv(&["p-ai"])));
        assert!(!headless_cli_invocation(&argv(&["p-ai", "runs"])));
    }

    #[test]
    fn parse_rejects_missing_task_or_model() {
        assert!(parse_headless_cli_args(&argv(&["p-ai", "run", "--model", "a::b"])).is_err());
        assert!(parse_headless_cli_args(&argv(&["p-ai", "run", "   ", "--model", "a::b"])).is_err());
        assert!(parse_headless_cli_args(&argv(&["p-ai", "run", "hi"])).is_err());
        assert!(parse_headless_cli_args(&argv(&["p-ai", "run", "hi", "--bogus"])).is_err());
    }

    #[test]
    fn parse_collects_task_and_flags() {
        let parsed = parse_headless_cli_args(&argv(&[
            "p-ai",
            "run",
            "修复",
            "构建",
            "--model",
            "a::b",
            "--minimal-tools",
        ]))
        .expect("parse");
        assert_eq!(parsed.task, "修复 构建");
        assert_eq!(parsed.model_config_id, "a::b");
        assert!(parsed.minimal_tools);
    }

    #[test]
    fn desktop_path_never_denies_tools() {
        assert!(headless_cli_tool_denied_reason_for(None, "delegate").is_none());
        assert!(headless_cli_tool_denied_reason_for(None, "websearch").is_none());
    }

    #[test]
    fn cli_default_denies_delegate_but_keeps_others() {
        let preset = Some(HeadlessCliToolPreset { minimal: false });
        assert!(headless_cli_tool_denied_reason_for(preset, "delegate").is_some());
        assert!(headless_cli_tool_denied_reason_for(preset, "websearch").is_none());
    }

    #[test]
    fn minimal_preset_keeps_only_programming_tools() {
        let preset = Some(HeadlessCliToolPreset { minimal: true });
        for tool in ["exec", "read", "write", "update", "delete", "move", "plan", "todo", "background"] {
            assert!(
                headless_cli_tool_denied_reason_for(preset, tool).is_none(),
                "工具 {tool} 不应被最小档拒绝"
            );
        }
        for tool in ["delegate", "websearch", "fetch", "operate", "windows", "remember", "task"] {
            assert!(
                headless_cli_tool_denied_reason_for(preset, tool).is_some(),
                "工具 {tool} 应被最小档拒绝"
            );
        }
    }
}
