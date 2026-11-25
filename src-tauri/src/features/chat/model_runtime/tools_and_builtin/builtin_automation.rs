// ==================== automation ====================
//
// 会话自动化入口。两个命令：
//   get     给出本会话的脚本目录、激活令牌、激活端点与一份照着做就行的指南，
//           外加清单里现在有什么、各自跑没跑。
//   reload  改完 automations.json 后调用：重读当前会话的清单并起停。
//
// 工具只说「什么时候用我」；怎么做写在回执的 guide 里，一次交代完。

const AUTOMATION_COMMAND_GET: &str = "get";
const AUTOMATION_COMMAND_RELOAD: &str = "reload";

/// 回执里的指南：精炼、通用，突出单行 JSON 通信与 ASCII 转义规避本地编码差异。
fn automation_guide(endpoint: &str, automation_dir: &str, token: &str) -> String {
    let mut out = format!("automationDir: {automation_dir}\n");
    out.push_str("1. 消息投递（默认通道）：脚本发现事件需要通知本会话时，直接在标准输出（stdout）打印单行 JSON：print(json.dumps({\"message\": \"通知正文\"}))。单行合法 JSON 且 message 为非空字符串时，PAI 自动将其作为正式消息投递进当前会话。无需令牌与网络配置。\n");
    out.push_str("2. 编码规范：系统默认编码可能不是 UTF-8（如部分环境默认 GBK）。严禁直接 print 裸中文字符；向会话发消息务必使用标准库序列化单行 JSON（例如 Python 的 json.dumps 默认将非 ASCII 字符转义为 \\u 纯 ASCII 文本），天然跨平台兼容，彻底避免本地编码解码失败。\n");
    out.push_str(&format!(
        "3. 脚本与配置：脚本放进 automationDir。在 {automation_dir}/automations.json 中配置：\n   {{\"tasks\":[{{\"id\":\"check\",\"command\":\"python poll.py\",\"intervalSeconds\":300}}]}}\n   command 为完整命令行；不填 intervalSeconds 则为常驻脚本。修改清单后调用 reload 立即生效。\n"
    ));
    out.push_str("4. 状态与停用：调用 get 查看 tasks 运行态（running、lastRun 包含最近退出码与耗时）以及 recentLog（最近 20 条日志原文）。停用时从 automations.json 移除对应任务并调用 reload。\n");
    if endpoint.is_empty() {
        out.push_str("（备用通道：若外部独立服务无法输出 stdout，需先在设置中开启网络访问，再调 get 获取 web 端点）\n");
    } else {
        out.push_str(&format!(
            "（备用通道：若外部独立服务无法输出 stdout，可走 web 端点：curl -X POST {endpoint} -H \"Authorization: Bearer {token}\" -H \"Content-Type: application/json\" -d '{{\"message\":\"通知正文\"}}'）\n"
        ));
    }
    out
}

fn automation_command_get(state: &AppState, session_id: &str) -> Result<Value, String> {
    let conversation_id = session_tool_source_conversation_id(session_id)?;
    let (record, secret) = activation_token_create(state, &conversation_id)?;
    let automation_dir =
        automation_dir_for_conversation(&state.data_path, &conversation_id).display().to_string();
    let endpoint = activation_api_endpoint_url().unwrap_or_default();
    let tasks = conversation_automation_describe(state, &conversation_id)?;
    let recent_log = automation_recent_log_lines(&conversation_id, AUTOMATION_RECENT_LOG_LINES);
    runtime_log_info(format!(
        "[自动化] 完成 任务=下发自动化入口 会话={} 令牌号={} 日志条数={}",
        conversation_id,
        record.token_id,
        recent_log.len()
    ));
    Ok(serde_json::json!({
        "automationDir": automation_dir,
        "token": secret,
        "endpoint": endpoint,
        "guide": automation_guide(&endpoint, &automation_dir, &secret),
        "tasks": tasks,
        "recentLog": recent_log,
    }))
}

fn automation_command_reload(state: &AppState, session_id: &str) -> Result<Value, String> {
    let conversation_id = session_tool_source_conversation_id(session_id)?;
    // 只重读当前会话：别的会话在跑什么，与这次调用无关。
    let outcome = conversation_automation_reload_conversation(state, &conversation_id)?;
    Ok(serde_json::json!({
        "ok": outcome.errors.is_empty(),
        "tasks": outcome.tasks,
        "errors": outcome.errors,
    }))
}

fn builtin_automation(
    state: &AppState,
    session_id: &str,
    args: AutomationToolArgs,
) -> Result<Value, String> {
    match args.command.trim() {
        AUTOMATION_COMMAND_GET => automation_command_get(state, session_id),
        AUTOMATION_COMMAND_RELOAD => automation_command_reload(state, session_id),
        other => Err(format!(
            "command 只接受 {AUTOMATION_COMMAND_GET} 或 {AUTOMATION_COMMAND_RELOAD}，收到: {other}"
        )),
    }
}
