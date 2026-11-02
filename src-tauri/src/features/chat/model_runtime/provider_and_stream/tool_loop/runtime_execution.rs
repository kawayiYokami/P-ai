/// 需要做通用归一化的适配器：OpenAI 兼容系（含 DeepSeek/Kimi/WorkBuddy 等）。
/// Anthropic 与 Gemini 各有自己的 schema 契约，不在这里统一处理。
fn tool_schema_normalization_applies(adapter_kind: genai::adapter::AdapterKind) -> bool {
    matches!(
        adapter_kind,
        genai::adapter::AdapterKind::OpenAI
            | genai::adapter::AdapterKind::OpenAIResp
            | genai::adapter::AdapterKind::DeepSeek
            | genai::adapter::AdapterKind::Moonshot
            | genai::adapter::AdapterKind::Kimi
            | genai::adapter::AdapterKind::BigModel
            | genai::adapter::AdapterKind::MiniMax
            | genai::adapter::AdapterKind::Aliyun
            | genai::adapter::AdapterKind::Mimo
            | genai::adapter::AdapterKind::OpenCodeGo
            | genai::adapter::AdapterKind::OpenRouter
            | genai::adapter::AdapterKind::Groq
            | genai::adapter::AdapterKind::Together
            | genai::adapter::AdapterKind::Fireworks
            | genai::adapter::AdapterKind::Nebius
            | genai::adapter::AdapterKind::Baidu
            | genai::adapter::AdapterKind::Cohere
    )
}

pub async fn runtime_tool_definitions_for_genai(
    definitions: &[ProviderToolDefinition],
    adapter_kind: genai::adapter::AdapterKind,
) -> Result<Vec<genai::chat::Tool>, String> {
    let mut out = Vec::<genai::chat::Tool>::new();
    for definition in definitions {
        let mut genai_tool = genai::chat::Tool::new(definition.name.clone());
        if !definition.description.trim().is_empty() {
            genai_tool = genai_tool.with_description(definition.description.clone());
        }
        let mut parameters = definition.parameters.clone();
        if matches!(adapter_kind, genai::adapter::AdapterKind::Gemini | genai::adapter::AdapterKind::Vertex) {
            gemini_to_openapi_schema(&mut parameters);
        } else if tool_schema_normalization_applies(adapter_kind) {
            // OpenAI 兼容通道原样转发，上游校验器对 $schema、联合 type 这类超集写法
            // 容忍度不一；内置工具不带这些字段，第三方 MCP 的 schema 常带，统一收敛。
            normalize_tool_schema(&mut parameters);
        }
        genai_tool = genai_tool.with_schema(parameters);
        out.push(genai_tool);
    }
    Ok(out)
}

/// 没有 timeout_override 的工具也必须有一个上限。
///
/// `RuntimeToolDyn::timeout_override` 默认返回 None，只给少数内置工具实现，于是委托、MCP、
/// 以及其它第三方工具一直是裸 await：上游挂住就无限等，只能等用户手点停止。这条兜底不负责
/// 精细控制，只负责让「无限等」变成「有限等一次」；需要更紧的超时仍由各工具的 override 决定。
const DEFAULT_RUNTIME_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(900);

/// 调用方自己在参数里声明了 timeout_ms 时，兜底只作为「收尾余量」加在后面。
/// 终端类工具本来就允许模型显式传大值跑长构建，兜底若比它短，会把合法任务砍在半路。
const RUNTIME_TOOL_TIMEOUT_REQUESTED_SLACK_MS: u64 = 60_000;

/// 本次工具调用的兜底超时。
fn runtime_tool_fallback_timeout(tool_args: &str) -> std::time::Duration {
    let requested_ms = serde_json::from_str::<Value>(tool_args)
        .ok()
        .and_then(|args| args.get("timeout_ms").and_then(Value::as_u64))
        .filter(|ms| *ms > 0);
    match requested_ms.map(std::time::Duration::from_millis) {
        // 只在模型显式要的时间比兜底更长时才让路，并留出余量让工具自己的超时先报错。
        Some(requested) if requested > DEFAULT_RUNTIME_TOOL_TIMEOUT => requested
            .saturating_add(std::time::Duration::from_millis(
                RUNTIME_TOOL_TIMEOUT_REQUESTED_SLACK_MS,
            )),
        _ => DEFAULT_RUNTIME_TOOL_TIMEOUT,
    }
}

async fn call_runtime_tool_by_name(
    tools: &[Box<dyn RuntimeToolDyn>],
    tool_name: &str,
    tool_args: &str,
) -> Result<ProviderToolResult, String> {
    call_runtime_tool_by_name_with_default_timeout(
        tools,
        tool_name,
        tool_args,
        runtime_tool_fallback_timeout(tool_args),
    )
    .await
}

/// 兜底超时可注入的版本：生产路径用 DEFAULT_RUNTIME_TOOL_TIMEOUT，
/// 测试需要一个能立刻到点的值才能证明「没有 override 的工具同样会被掐断」。
async fn call_runtime_tool_by_name_with_default_timeout(
    tools: &[Box<dyn RuntimeToolDyn>],
    tool_name: &str,
    tool_args: &str,
    default_timeout: std::time::Duration,
) -> Result<ProviderToolResult, String> {
    let Some(tool) = tools.iter().find(|tool| {
        let name = tool.name();
        name == tool_name || (tool_name == "read_file" && name == READ_TOOL_NAME)
    }) else {
        // MCP 折叠后，未拿取或写错名都会走到这里：只建议不自动装，必须按精确名单重拿。
        let known: Vec<String> = read_global_tool_schema_cache(None)
            .into_iter()
            .filter(|tool| {
                matches!(tool.source, CachedRuntimeToolSource::Mcp { .. })
                    && tool.compatibility_error.is_none()
            })
            .map(|tool| tool.definition.name)
            .collect();
        let suggestions = suggest_similar_mcp_tool_names(tool_name, &known);
        let mut message = format!("没有匹配到工具：{tool_name}");
        if !suggestions.is_empty() {
            message.push_str(&format!("，你是否想要匹配{}？", suggestions.join("、")));
        }
        message.push_str("；请按目录中的精确工具名调 get_tool 拿取，下轮再用。");
        return Err(message);
    };
    // 走到这里代表工具确实要被调用：缺名工具的早退不算执行，不能污染重试决策。
    note_runtime_tool_executed();
    let timeout_override = tool.timeout_override(tool_args);
    let timeout = timeout_override.unwrap_or(default_timeout);
    match tokio::time::timeout(timeout, tool.call_json(tool_args.to_string())).await {
        Ok(Err(err)) if tool_name == READ_MEDIA_TOOL_NAME && err.trim() == "解析超时" => {
            Ok(ProviderToolResult::text("解析超时"))
        }
        Ok(result) => result,
        Err(_) => {
            runtime_log_warn(format!(
                "[工具执行] 工具执行超时: tool={}, kind={}, timeout_ms={}, timeout_source={}",
                tool_name,
                if tool.is_mcp_tool() { "mcp" } else { "builtin" },
                timeout.as_millis(),
                if timeout_override.is_some() {
                    "override"
                } else {
                    "default"
                }
            ));
            if tool_name == READ_MEDIA_TOOL_NAME {
                return Ok(ProviderToolResult::text("解析超时"));
            }
            Ok(ProviderToolResult::error(tool_failure_result_text(
                tool_name,
                &format!("工具执行超时，timeout_ms={}", timeout.as_millis()),
            )))
        }
    }
}

fn normalize_runtime_tool_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn runtime_tool_by_name<'a>(
    tools: &'a [Box<dyn RuntimeToolDyn>],
    tool_name: &str,
) -> Option<&'a Box<dyn RuntimeToolDyn>> {
    tools.iter().find(|tool| {
        let name = tool.name();
        name == tool_name || (tool_name == "read_file" && name == READ_TOOL_NAME)
    })
}

fn runtime_tool_definition_by_name<'a>(
    definitions: &'a [ProviderToolDefinition],
    tool_name: &str,
) -> Option<&'a ProviderToolDefinition> {
    definitions.iter().find(|definition| {
        definition.name == tool_name || (tool_name == "read_file" && definition.name == READ_TOOL_NAME)
    })
}

fn text_contains_runtime_tool_keyword(text: &str, keyword: &str) -> bool {
    let keyword = keyword.trim().to_ascii_lowercase();
    if keyword.is_empty() {
        return false;
    }
    let text = text.to_ascii_lowercase();
    let mut start = 0usize;
    while let Some(offset) = text[start..].find(&keyword) {
        let idx = start + offset;
        let before = text[..idx]
            .chars()
            .next_back()
            .map(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(false);
        let after_idx = idx + keyword.len();
        let after = text[after_idx..]
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(false);
        if !before && !after {
            return true;
        }
        start = after_idx;
    }
    false
}

fn mcp_tool_definition_looks_mutating(definition: Option<&ProviderToolDefinition>, tool_name: &str) -> bool {
    let mut haystacks = vec![tool_name.to_string()];
    if let Some(definition) = definition {
        haystacks.push(definition.description.clone());
        haystacks.push(definition.parameters.to_string());
    }
    let text = haystacks.join("\n");
    const SERIAL_WORDS: &[&str] = &[
        "shell", "exec", "terminal", "command", "edit", "write", "patch", "apply", "file",
        "filesystem", "fs", "delete", "remove", "move", "rename", "create", "save",
        "update", "replace", "insert", "append", "modify", "mkdir", "rmdir",
    ];
    SERIAL_WORDS
        .iter()
        .any(|keyword| text_contains_runtime_tool_keyword(&text, keyword))
}

fn runtime_tool_call_requires_serial_execution(
    tools: &[Box<dyn RuntimeToolDyn>],
    definitions: &[ProviderToolDefinition],
    tool_name: &str,
) -> bool {
    let normalized = normalize_runtime_tool_name(tool_name);
        if matches!(
        normalized.as_str(),
            "exec"
            | "shell_exec"
            | "config"
            | "write"
            | "delete"
            | "update"
            | "move"
            | "todo"
            | "task"
            | "progress"
            | "question"
            | "remember"
            | "plan"
            | "image_generate"
            | "image_edit"
            | "image_to_video"
            | "remote_im_send"
            | "contact_send_files"
    ) {
        return true;
    }
    let Some(tool) = runtime_tool_by_name(tools, tool_name) else {
        return false;
    };
    if !tool.is_mcp_tool() {
        return false;
    }
    let definition = runtime_tool_definition_by_name(definitions, tool_name);
    mcp_tool_definition_looks_mutating(definition, tool_name)
}

fn prepared_tool_call_from_genai(tool_call: genai::chat::ToolCall) -> PreparedToolCall {
    let genai::chat::ToolCall {
        call_id,
        fn_name,
        fn_arguments,
        ..
    } = tool_call;
    let tool_args = unwrap_tool_call_arguments(&fn_arguments);
    PreparedToolCall {
        tool_call_id: call_id,
        tool_name: fn_name,
        tool_args,
    }
}

fn split_prepared_tool_calls_into_execution_batches(
    tools: &[Box<dyn RuntimeToolDyn>],
    definitions: &[ProviderToolDefinition],
    tool_calls: Vec<PreparedToolCall>,
) -> Vec<PreparedToolCallBatch> {
    let mut batches = Vec::<PreparedToolCallBatch>::new();
    let mut pending_parallel_calls = Vec::<PreparedToolCall>::new();

    for call in tool_calls {
        if runtime_tool_call_requires_serial_execution(tools, definitions, &call.tool_name) {
            if !pending_parallel_calls.is_empty() {
                batches.push(PreparedToolCallBatch {
                    calls: std::mem::take(&mut pending_parallel_calls),
                });
            }
            batches.push(PreparedToolCallBatch { calls: vec![call] });
        } else {
            pending_parallel_calls.push(call);
        }
    }

    if !pending_parallel_calls.is_empty() {
        batches.push(PreparedToolCallBatch {
            calls: pending_parallel_calls,
        });
    }

    batches
}

async fn execute_prepared_tool_call(
    tools: &[Box<dyn RuntimeToolDyn>],
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    call: PreparedToolCall,
) -> Result<ExecutedToolCall, String> {
    let tool_result = match call_runtime_tool_by_name(tools, &call.tool_name, &call.tool_args).await {
        Ok(output) => {
            let status_message = if output.is_error {
                format!("工具返回错误结果：{}", call.tool_name)
            } else {
                format!("工具调用完成：{}", call.tool_name)
            };
            send_tool_status_event(
                on_delta,
                &call.tool_name,
                if output.is_error { "failed" } else { "done" },
                Some(call.tool_args.as_str()),
                Some(call.tool_call_id.as_str()),
                &status_message,
            );
            output
        }
        Err(err) => {
            if err == CHAT_ABORTED_BY_USER_ERROR {
                return Err(err);
            }
            let err_text = err.to_string();
            send_tool_status_event(
                on_delta,
                &call.tool_name,
                "failed",
                Some(call.tool_args.as_str()),
                Some(call.tool_call_id.as_str()),
                &format!("工具调用失败：{} ({})", call.tool_name, err_text),
            );
            ProviderToolResult::error(tool_failure_result_text(&call.tool_name, &err_text))
        }
    };
    Ok(ExecutedToolCall {
        tool_call_id: call.tool_call_id,
        tool_name: call.tool_name,
        tool_args: call.tool_args,
        tool_result,
    })
}

/// 一组工具的执行结果：跑完的照常交付，被打断的只记 id。
///
/// 结果与错误是两件事，必须同时交付：组内出现错误时 `executed` 里已经跑完的调用
/// 仍然是真的执行结果，调用方要先落库、再把这个错误原样上抛。
struct ExecutedToolGroup {
    executed: Vec<ExecutedToolCall>,
    /// 没有结果可交付的调用：被打断的，或调用自身以错误结束的。
    aborted_call_ids: Vec<String>,
    /// 让整组无法继续的原始错误；已完成的调用结果不受影响。
    error: Option<String>,
}

/// 组内单个调用的结局。打断只作用于还没返回的调用，已完成的调用仍然交付结果。
enum GroupCallOutcome {
    Finished(Result<ExecutedToolCall, String>),
    Aborted,
}

/// 把组内逐个结局整理成可交付结果：完成的照常返回，被打断的只记 id。
/// 关键在于打断不会吞掉同一批里已经跑完的工具结果。
fn collect_group_outcomes(
    outcomes: Vec<GroupCallOutcome>,
    calls_snapshot: &[(String, String)],
) -> ExecutedToolGroup {
    let mut executed = Vec::<ExecutedToolCall>::new();
    let mut aborted_call_ids = Vec::<String>::new();
    let mut error: Option<String> = None;
    for (index, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            GroupCallOutcome::Finished(Ok(call)) => executed.push(call),
            GroupCallOutcome::Finished(Err(err)) => {
                // 这个调用没有结果可交付，但它的占位必须补上结束事实，
                // 否则历史里会一直停在「已发起调用，尚未返回结果」。
                if let Some((_, tool_call_id)) = calls_snapshot.get(index) {
                    aborted_call_ids.push(tool_call_id.clone());
                }
                if error.is_none() {
                    error = Some(err);
                }
            }
            GroupCallOutcome::Aborted => {
                if let Some((_, tool_call_id)) = calls_snapshot.get(index) {
                    aborted_call_ids.push(tool_call_id.clone());
                }
            }
        }
    }
    ExecutedToolGroup {
        executed,
        aborted_call_ids,
        error,
    }
}

/// 执行一组工具。返回结构里始终带着已经跑完的结果，错误只是其中一个字段。
async fn execute_prepared_tool_call_group(
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
    tools: &[Box<dyn RuntimeToolDyn>],
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    request_id: Option<&str>,
    calls: Vec<PreparedToolCall>,
) -> ExecutedToolGroup {
    if calls.is_empty() {
        return ExecutedToolGroup {
            executed: Vec::new(),
            aborted_call_ids: Vec::new(),
            error: None,
        };
    }
    for call in &calls {
        send_stream_rebind_required_event(on_delta, request_id, "tool_start");
        send_tool_status_event(
            on_delta,
            &call.tool_name,
            "running",
            Some(call.tool_args.as_str()),
            Some(call.tool_call_id.as_str()),
            &format!("正在调用工具：{}", call.tool_name),
        );
        // 调度事件：工具调用开始（全量，latest Run，elapsed 由 Store 内 Instant 推导，长度全量不裁剪）
        if let Some(state) = tool_abort_state {
            let arg_preview: String = call.tool_args.clone();
            let mut detail = serde_json::json!({
                "toolName": call.tool_name,
                "toolCallId": call.tool_call_id,
                "argLength": call.tool_args.chars().count(),
            });
            if !arg_preview.trim().is_empty() {
                if let Some(obj) = detail.as_object_mut() {
                    obj.insert("argPreview".to_string(), serde_json::json!(arg_preview));
                }
            }
            let _ = schedule_event_push_to_latest_run(
                state,
                chat_session_key,
                "tool_call",
                0,
                None,
                detail,
            );
        }
    }
    let calls_snapshot: Vec<(String, String)> = calls
        .iter()
        .map(|call| (call.tool_name.clone(), call.tool_call_id.clone()))
        .collect();
    // 中断信号只做通知，不取消整组：已经跑完的调用照常交付结果，
    // 只有还没返回的调用被标记为打断，避免同一批里已完成工具的结果被整批丢掉。
    let abort_signal = if let Some(state) = tool_abort_state {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        // 注册不上等于停止按钮对这组工具失效：不跑这组，把错误交给调用方按原样上抛。
        if let Err(err) =
            register_inflight_tool_abort_handle(state, chat_session_key, abort_handle)
        {
            runtime_log_error(format!(
                "[聊天] 登记工具组中断句柄失败，跳过本组执行 (session={}): {}",
                chat_session_key, err
            ));
            return ExecutedToolGroup {
                executed: Vec::new(),
                aborted_call_ids: Vec::new(),
                error: Some(format!("登记工具组中断句柄失败：{err}")),
            };
        }
        Some(futures_util::future::FutureExt::shared(
            futures_util::future::FutureExt::map(
                futures_util::future::Abortable::new(
                    futures_util::future::pending::<()>(),
                    abort_registration,
                ),
                |_| (),
            ),
        ))
    } else {
        None
    };
    let group_outcomes = {
        let futures = calls
            .into_iter()
            .map(|call| {
                let signal = abort_signal.clone();
                async move {
                    let execution = execute_prepared_tool_call(tools, on_delta, call);
                    match signal {
                        Some(signal) => {
                            futures_util::pin_mut!(execution);
                            match futures_util::future::select(execution, signal).await {
                                futures_util::future::Either::Left((result, _)) => {
                                    GroupCallOutcome::Finished(result)
                                }
                                futures_util::future::Either::Right((_aborted, _)) => {
                                    GroupCallOutcome::Aborted
                                }
                            }
                        }
                        None => GroupCallOutcome::Finished(execution.await),
                    }
                }
            })
            .collect::<Vec<_>>();
        futures_util::future::join_all(futures).await
    };
    if let Some(state) = tool_abort_state {
        if let Err(err) = clear_inflight_tool_abort_handle(state, chat_session_key) {
            runtime_log_error(format!(
                "[聊天] 清理进行中工具组中断句柄失败 (session={}): {}",
                chat_session_key, err
            ));
        }
    }

    let group = collect_group_outcomes(group_outcomes, &calls_snapshot);
    if !group.aborted_call_ids.is_empty() {
        runtime_log_info(format!(
            "[聊天] 工具组未全部返回 (session={}，已完成={}，被打断={}，error={})",
            chat_session_key,
            group.executed.len(),
            group.aborted_call_ids.len(),
            group.error.as_deref().unwrap_or("(none)")
        ));
    }
    if let Some(state) = tool_abort_state {
        for exec in &group.executed {
            let is_error = exec.tool_result.is_error;
            let text_len = match exec.tool_result.parts.iter().find_map(|part| match part {
                ProviderToolResultPart::Text { text } => Some(text.chars().count()),
                _ => None,
            }) {
                Some(len) => len,
                None => 0,
            };
            let text_preview: String = exec.tool_result.parts.iter().find_map(|part| match part {
                ProviderToolResultPart::Text { text } => Some(text.clone()),
                _ => None,
            }).unwrap_or_default();
            let mut detail = serde_json::json!({
                "toolName": exec.tool_name,
                "toolCallId": exec.tool_call_id,
                "isError": is_error,
                "textLength": text_len,
            });
            if !text_preview.trim().is_empty() {
                if let Some(obj) = detail.as_object_mut() {
                    obj.insert("textPreview".to_string(), serde_json::json!(text_preview));
                }
            }
            let _ = schedule_event_push_to_latest_run(
                state,
                chat_session_key,
                "tool_result",
                0,
                Some(!is_error),
                detail,
            );
        }
        // 没有结果可交付的调用只可能是被中止：单调用里其余失败都已转成工具错误结果，
        // 不会以 Err 结束整组。
        for aborted_call_id in &group.aborted_call_ids {
            let tool_name = calls_snapshot
                .iter()
                .find(|(_, tool_call_id)| tool_call_id == aborted_call_id)
                .map(|(tool_name, _)| tool_name.clone())
                .unwrap_or_default();
            let detail = serde_json::json!({
                "toolName": tool_name,
                "toolCallId": aborted_call_id,
                "isError": true,
                "isAborted": true,
                "error": CHAT_ABORTED_BY_USER_ERROR,
            });
            let _ = schedule_event_push_to_latest_run(
                state,
                chat_session_key,
                "tool_result",
                0,
                Some(false),
                detail,
            );
        }
    }
    group
}

fn runtime_tool_result_followup_message(
    tool_name: &str,
    tool_result: &ProviderToolResult,
    include_images: bool,
) -> Option<genai::chat::ChatMessage> {
    let mut forwarded_parts = Vec::<genai::chat::ContentPart>::new();

    for part in &tool_result.parts {
        match part {
            ProviderToolResultPart::Text { .. } => {}
            ProviderToolResultPart::Image { mime, data_base64, .. } => {
                if !include_images {
                    continue;
                }
                forwarded_parts.push(genai::chat::ContentPart::from_binary_base64(
                    mime.clone(),
                    data_base64.clone(),
                    None,
                ));
            }
            ProviderToolResultPart::Audio { mime, data_base64 } => {
                forwarded_parts.push(genai::chat::ContentPart::from_binary_base64(
                    mime.clone(),
                    data_base64.clone(),
                    None,
                ));
            }
            ProviderToolResultPart::Resource { mime, uri, text } => {
                let mut lines = vec![format!("工具 `{tool_name}` 返回了资源内容。")];
                if let Some(uri) = uri.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                    lines.push(format!("resource uri: {uri}"));
                }
                if let Some(mime) = mime
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    lines.push(format!("resource mime: {mime}"));
                }
                if !text.trim().is_empty() {
                    lines.push(text.clone());
                }
                forwarded_parts.push(genai::chat::ContentPart::from_text(lines.join("\n")));
            }
        }
    }

    if forwarded_parts.is_empty() {
        return None;
    }

    let mut parts = vec![genai::chat::ContentPart::from_text(format!(
        "工具 `{tool_name}` 返回了额外模态内容，以下内容已继续提供给模型。"
    ))];
    parts.extend(forwarded_parts);
    Some(genai::chat::ChatMessage::user(
        genai::chat::MessageContent::from_parts(parts),
    ))
}

#[cfg(test)]
mod runtime_execution_tool_timeout_tests {
    use super::*;

    /// 永不返回的工具：timeout_override 完全由用例指定，用来区分「有 override」与「靠兜底」。
    struct PendingTool {
        name: &'static str,
        timeout_override: Option<std::time::Duration>,
    }

    impl RuntimeToolDyn for PendingTool {
        fn name(&self) -> String {
            self.name.to_string()
        }

        fn timeout_override(&self, _args_json: &str) -> Option<std::time::Duration> {
            self.timeout_override
        }

        fn call_json(&self, _args_json: String) -> RuntimeToolCallFuture<'_> {
            Box::pin(std::future::pending())
        }
    }

    fn pending_tool(
        name: &'static str,
        timeout_override: Option<std::time::Duration>,
    ) -> Vec<Box<dyn RuntimeToolDyn>> {
        vec![Box::new(PendingTool {
            name,
            timeout_override,
        })]
    }

    fn result_text(result: &ProviderToolResult) -> String {
        result
            .parts
            .iter()
            .find_map(|part| match part {
                ProviderToolResultPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn tool_without_timeout_override_still_times_out() {
        // A5：没有 override 的工具以前是裸 await，上游挂住就无限等。
        // 这里给一个永不返回的工具 + 立刻到点的兜底，断言拿到的是模型可见的失败结果。
        let tools = pending_tool("delegate", None);
        let result = call_runtime_tool_by_name_with_default_timeout(
            &tools,
            "delegate",
            "{}",
            std::time::Duration::from_millis(1),
        )
        .await
        .expect("超时必须变成工具失败结果，而不是把整轮调度打挂");
        assert!(result.is_error);
        let text = result_text(&result);
        assert!(text.contains("工具执行超时"), "实际文案: {text}");
    }

    #[tokio::test]
    async fn tool_timeout_override_wins_over_default() {
        // 兜底不能盖掉更紧的 override：override 1ms + 兜底 30s，必须在 override 处到点。
        let tools = pending_tool("windows", Some(std::time::Duration::from_millis(1)));
        let started = std::time::Instant::now();
        let result = call_runtime_tool_by_name_with_default_timeout(
            &tools,
            "windows",
            "{}",
            std::time::Duration::from_secs(30),
        )
        .await
        .expect("超时必须变成工具失败结果");
        assert!(result.is_error);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "override 未生效，用时 {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn executed_tool_is_counted_through_the_scope() {
        // 重试决策完全依赖这个计数：作用域之外调工具（单元测试、其它调用方）不该报错，
        // 作用域内的真实执行必须带得回来。
        let tools = pending_tool("delegate", Some(std::time::Duration::from_millis(1)));
        let (result, executed) = scoped_runtime_tool_execution_count(call_runtime_tool_by_name(
            &tools, "delegate", "{}",
        ))
        .await;
        assert!(result.is_ok(), "工具应返回超时失败结果");
        assert_eq!(executed, 1);
        note_runtime_tool_executed();
    }

    #[test]
    fn fallback_timeout_respects_explicit_timeout_ms() {
        // 没有任何 timeout_ms：直接吃兜底。
        assert_eq!(
            runtime_tool_fallback_timeout("{}"),
            DEFAULT_RUNTIME_TOOL_TIMEOUT
        );
        assert_eq!(
            runtime_tool_fallback_timeout("not a json"),
            DEFAULT_RUNTIME_TOOL_TIMEOUT
        );
        // 终端工具允许模型传大值跑长构建：兜底绝不能比它短，否则会把合法任务砍在半路。
        assert_eq!(
            runtime_tool_fallback_timeout("{\"timeout_ms\": 1800000}"),
            std::time::Duration::from_millis(1_860_000)
        );
        // 小于兜底的声明不缩水兜底本身。
        assert_eq!(
            runtime_tool_fallback_timeout("{\"timeout_ms\": 1000}"),
            DEFAULT_RUNTIME_TOOL_TIMEOUT
        );
    }

    #[tokio::test]
    async fn missing_tool_is_not_counted_as_executed() {
        // 找不到工具是早退，没有副作用可重复，不能污染「本轮执行过工具」的判定。
        let tools = pending_tool("delegate", None);
        let (result, executed) = scoped_runtime_tool_execution_count(call_runtime_tool_by_name(
            &tools,
            "no_such_tool",
            "{}",
        ))
        .await;
        assert!(result.is_err());
        assert_eq!(executed, 0);
    }
}
