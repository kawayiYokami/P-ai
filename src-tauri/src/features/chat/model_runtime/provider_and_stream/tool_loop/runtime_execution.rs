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
        }
        genai_tool = genai_tool.with_schema(parameters);
        out.push(genai_tool);
    }
    Ok(out)
}

async fn call_runtime_tool_by_name(
    tools: &[Box<dyn RuntimeToolDyn>],
    tool_name: &str,
    tool_args: &str,
) -> Result<ProviderToolResult, String> {
    let Some(tool) = tools.iter().find(|tool| {
        let name = tool.name();
        name == tool_name || (tool_name == "read_file" && name == READ_TOOL_NAME)
    }) else {
        return Err(format!("未找到工具：{tool_name}"));
    };
    if let Some(timeout) = tool.timeout_override(tool_args) {
        match tokio::time::timeout(timeout, tool.call_json(tool_args.to_string())).await {
            Ok(Err(err)) if tool_name == READ_MEDIA_TOOL_NAME && err.trim() == "解析超时" => {
                Ok(ProviderToolResult::text("解析超时"))
            }
            Ok(result) => result,
            Err(_) => {
                runtime_log_warn(format!(
                    "[工具执行] 工具执行超时: tool={}, kind={}, timeout_ms={}",
                    tool_name,
                    if tool.is_mcp_tool() { "mcp" } else { "builtin" },
                    timeout.as_millis()
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
    } else {
        tool.call_json(tool_args.to_string()).await
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
            | "remember"
            | "plan"
            | "image_generate"
            | "image_edit"
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

async fn execute_prepared_tool_call_group_inner(
    tools: &[Box<dyn RuntimeToolDyn>],
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    calls: Vec<PreparedToolCall>,
) -> Result<Vec<ExecutedToolCall>, String> {
    let futures = calls
        .into_iter()
        .map(|call| execute_prepared_tool_call(tools, on_delta, call))
        .collect::<Vec<_>>();
    let mut output = Vec::<ExecutedToolCall>::new();
    for result in futures_util::future::join_all(futures).await {
        output.push(result?);
    }
    Ok(output)
}

async fn execute_prepared_tool_call_group(
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
    tools: &[Box<dyn RuntimeToolDyn>],
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    request_id: Option<&str>,
    calls: Vec<PreparedToolCall>,
) -> Result<Vec<ExecutedToolCall>, String> {
    if calls.is_empty() {
        return Ok(Vec::new());
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
    let run_group = execute_prepared_tool_call_group_inner(tools, on_delta, calls);
    let executed_result: Result<Vec<ExecutedToolCall>, String> = if let Some(state) = tool_abort_state {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        register_inflight_tool_abort_handle(state, chat_session_key, abort_handle)?;
        let result = futures_util::future::Abortable::new(run_group, abort_registration).await;
        if let Err(err) = clear_inflight_tool_abort_handle(state, chat_session_key) {
            runtime_log_error(format!(
                "[聊天] 清理进行中工具组中断句柄失败 (session={}): {}",
                chat_session_key, err
            ));
        }
        match result {
            Ok(inner) => inner,
            Err(_) => {
                runtime_log_info(format!(
                    "[聊天] 用户中止工具组调用 (session={})",
                    chat_session_key
                ));
                Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
            }
        }
    } else {
        run_group.await
    };
    if let Some(state) = tool_abort_state {
        match executed_result.as_ref() {
            Ok(executed) => {
                for exec in executed {
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
            }
            Err(err) => {
                let is_aborted = err == CHAT_ABORTED_BY_USER_ERROR;
                if calls_snapshot.is_empty() {
                    let detail = serde_json::json!({
                        "isError": true,
                        "isAborted": is_aborted,
                        "error": err,
                    });
                    let _ = schedule_event_push_to_latest_run(
                        state,
                        chat_session_key,
                        "tool_result",
                        0,
                        Some(false),
                        detail,
                    );
                } else {
                    for (tool_name, tool_call_id) in &calls_snapshot {
                        let detail = serde_json::json!({
                            "toolName": tool_name,
                            "toolCallId": tool_call_id,
                            "isError": true,
                            "isAborted": is_aborted,
                            "error": err,
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
            }
        }
    }
    executed_result
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
