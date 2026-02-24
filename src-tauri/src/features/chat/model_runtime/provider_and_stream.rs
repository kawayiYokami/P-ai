fn shell_switch_workspace_enabled_for_session(
    selected_api: &ApiConfig,
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> bool {
    if !tool_enabled(selected_api, "shell-switch-workspace") {
        return false;
    }
    if let Some(state) = app_state {
        if terminal_session_has_locked_root(state, tool_session_id) {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScreenshotForwardPayload {
    mime: String,
    base64: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct ScreenshotArtifactEntry {
    mime: String,
    base64: String,
    width: u32,
    height: u32,
    created_seq: u64,
}

const SCREENSHOT_ARTIFACT_MAX_ITEMS: usize = 24;

fn screenshot_artifact_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, ScreenshotArtifactEntry>> {
    static CACHE: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, ScreenshotArtifactEntry>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn next_screenshot_artifact_seq() -> u64 {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn screenshot_artifact_cache_put(payload: &ScreenshotForwardPayload) -> String {
    let artifact_id = Uuid::new_v4().to_string();
    let entry = ScreenshotArtifactEntry {
        mime: payload.mime.clone(),
        base64: payload.base64.clone(),
        width: payload.width,
        height: payload.height,
        created_seq: next_screenshot_artifact_seq(),
    };
    let cache = screenshot_artifact_cache();
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= SCREENSHOT_ARTIFACT_MAX_ITEMS {
            if let Some(oldest_key) = guard
                .iter()
                .min_by_key(|(_, value)| value.created_seq)
                .map(|(key, _)| key.clone())
            {
                let _ = guard.remove(&oldest_key);
            }
        }
        guard.insert(artifact_id.clone(), entry);
    }
    artifact_id
}

fn screenshot_artifact_cache_get(artifact_id: &str) -> Option<ScreenshotArtifactEntry> {
    let cache = screenshot_artifact_cache();
    let guard = cache.lock().ok()?;
    guard.get(artifact_id).cloned()
}

fn clear_screenshot_artifact_cache() {
    if let Ok(mut guard) = screenshot_artifact_cache().lock() {
        guard.clear();
    }
}

fn compact_screenshot_tool_result(
    tool_result: &str,
    artifact_id: &str,
) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(tool_result) else {
        return tool_result.to_string();
    };
    if let Some(obj) = value.as_object_mut() {
        if obj.get("imageBase64").is_some() {
            obj.insert(
                "imageBase64".to_string(),
                Value::String(format!("<cached:{}>", artifact_id)),
            );
        }
        if let Some(parts) = obj.get_mut("parts").and_then(Value::as_array_mut) {
            for part in parts {
                if let Some(map) = part.as_object_mut() {
                    if map.get("data").is_some() {
                        map.insert(
                            "data".to_string(),
                            Value::String(format!("<cached:{}>", artifact_id)),
                        );
                    }
                }
            }
        }
        obj.insert(
            "screenshotArtifact".to_string(),
            serde_json::json!({
                "id": artifact_id,
                "maxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS
            }),
        );
        if let Some(response_obj) = obj.get_mut("response").and_then(Value::as_object_mut) {
            response_obj.insert(
                "screenshotArtifactId".to_string(),
                Value::String(artifact_id.to_string()),
            );
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| tool_result.to_string())
}

fn enrich_screenshot_tool_result_with_cache(
    tool_name: &str,
    tool_result: &str,
) -> (String, Option<(ScreenshotForwardPayload, String)>) {
    let Some(payload) = screenshot_forward_payload_from_tool_result(tool_name, tool_result) else {
        return (tool_result.to_string(), None);
    };
    let artifact_id = screenshot_artifact_cache_put(&payload);
    let compacted = compact_screenshot_tool_result(tool_result, &artifact_id);
    (compacted, Some((payload, artifact_id)))
}

fn screenshot_forward_payload_from_tool_result(
    tool_name: &str,
    tool_result: &str,
) -> Option<ScreenshotForwardPayload> {
    if !matches!(tool_name, "desktop_screenshot" | "desktop-screenshot") {
        return None;
    }
    let value = serde_json::from_str::<Value>(tool_result).ok()?;
    let image_base64 = value
        .get("imageBase64")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("parts")
                .and_then(Value::as_array)
                .and_then(|parts| {
                    parts.iter().find_map(|part| {
                        let is_image = part
                            .get("type")
                            .and_then(Value::as_str)
                            .map(|t| t.eq_ignore_ascii_case("image"))
                            .unwrap_or(false);
                        if !is_image {
                            return None;
                        }
                        part.get("data")
                            .and_then(Value::as_str)
                            .map(ToString::to_string)
                    })
                })
        })?;
    if image_base64.is_empty() {
        return None;
    }
    let mime = value
        .get("imageMime")
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("parts")
                .and_then(Value::as_array)
                .and_then(|parts| {
                    parts.iter().find_map(|part| {
                        let is_image = part
                            .get("type")
                            .and_then(Value::as_str)
                            .map(|t| t.eq_ignore_ascii_case("image"))
                            .unwrap_or(false);
                        if !is_image {
                            return None;
                        }
                        part.get("mimeType")
                            .and_then(Value::as_str)
                            .filter(|m| !m.trim().is_empty())
                            .map(ToString::to_string)
                    })
                })
        })
        .unwrap_or_else(|| "image/webp".to_string());
    let width = value
        .get("width")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let height = value
        .get("height")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    Some(ScreenshotForwardPayload {
        mime,
        base64: image_base64,
        width,
        height,
    })
}

fn screenshot_forward_notice(payload: &ScreenshotForwardPayload) -> String {
    if payload.width > 0 && payload.height > 0 {
        format!(
            "截图工具已执行，以下图片来自工具结果（{}x{}），将作为用户消息转发，请注意鉴别。",
            payload.width, payload.height
        )
    } else {
        "截图工具已执行，以下图片来自工具结果，将作为用户消息转发，请注意鉴别。".to_string()
    }
}

fn sanitize_tool_result_for_history(tool_name: &str, tool_result: &str) -> String {
    if !matches!(tool_name, "desktop_screenshot" | "desktop-screenshot") {
        return tool_result.to_string();
    }
    let Ok(mut value) = serde_json::from_str::<Value>(tool_result) else {
        return tool_result.to_string();
    };
    if let Some(obj) = value.as_object_mut() {
        if let Some(image_b64) = obj.get("imageBase64").and_then(Value::as_str) {
            obj.insert(
                "imageBase64".to_string(),
                Value::String(format!("<omitted:{} chars>", image_b64.len())),
            );
        }
        if let Some(parts) = obj.get_mut("parts").and_then(Value::as_array_mut) {
            for part in parts {
                let data_len = part
                    .get("data")
                    .and_then(Value::as_str)
                    .map(|data| data.len());
                if let Some(len) = data_len {
                    if let Some(map) = part.as_object_mut() {
                        map.insert(
                            "data".to_string(),
                            Value::String(format!("<omitted:{} chars>", len)),
                        );
                    }
                }
            }
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| tool_result.to_string())
}

type ScreenshotMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

struct RuntimeToolAssembly {
    tools: Vec<Box<dyn ToolDyn>>,
    tool_manifest: Vec<Value>,
    _mcp_screenshot_client: Option<ScreenshotMcpClient>,
    _dynamic_mcp_clients: Vec<DynamicMcpClient>,
}

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

async fn assemble_runtime_tools(
    selected_api: &ApiConfig,
    app_state: Option<&AppState>,
    tool_session_id: &str,
) -> Result<RuntimeToolAssembly, String> {
    let has_fetch = tool_enabled(selected_api, "fetch");
    let has_bing = tool_enabled(selected_api, "bing-search");
    let has_memory = tool_enabled(selected_api, "memory-save");
    let has_desktop_screenshot = tool_enabled(selected_api, "desktop-screenshot");
    let has_desktop_wait = tool_enabled(selected_api, "desktop-wait");
    let has_shell_switch_workspace =
        shell_switch_workspace_enabled_for_session(selected_api, app_state, tool_session_id);
    let has_shell_exec = tool_enabled(selected_api, "shell-exec");

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    let mut tool_manifest = Vec::<Value>::new();

    tool_manifest.push(tool_manifest_item(
        "builtin",
        "fetch",
        has_fetch,
        has_fetch,
        if has_fetch {
            None
        } else {
            Some("disabled in api tools config".to_string())
        },
    ));
    if has_fetch {
        tools.push(Box::new(BuiltinFetchTool));
    }

    tool_manifest.push(tool_manifest_item(
        "builtin",
        "bing-search",
        has_bing,
        has_bing,
        if has_bing {
            None
        } else {
            Some("disabled in api tools config".to_string())
        },
    ));
    if has_bing {
        tools.push(Box::new(BuiltinBingSearchTool));
    }

    if has_memory {
        let state = app_state
            .ok_or_else(|| "memory_save requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveTool {
            app_state: state.clone(),
        }));
        tools.push(Box::new(BuiltinMemorySaveBatchTool { app_state: state }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "memory-save",
            true,
            true,
            None,
        ));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "memory-save-batch",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "memory-save",
            false,
            false,
            Some("disabled in api tools config".to_string()),
        ));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "memory-save-batch",
            false,
            false,
            Some("disabled in api tools config".to_string()),
        ));
    }

    let mut mcp_screenshot_client: Option<ScreenshotMcpClient> = None;
    if has_desktop_screenshot {
        let client = try_attach_desktop_screenshot_mcp_tool(&mut tools)
            .await
            .map_err(|err| {
                format!("desktop_screenshot MCP tool is unavailable (no builtin fallback): {err}")
            })?;
        mcp_screenshot_client = Some(client);
        tool_manifest.push(tool_manifest_item(
            "builtin_mcp",
            "desktop-screenshot",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin_mcp",
            "desktop-screenshot",
            false,
            false,
            Some("disabled in api tools config".to_string()),
        ));
    }

    let dynamic_mcp_clients = match attach_enabled_mcp_tools_for_runtime(&mut tools, app_state).await {
        Ok((clients, names)) => {
            if names.is_empty() {
                tool_manifest.push(tool_manifest_item(
                    "mcp_runtime",
                    "(none)",
                    true,
                    false,
                    Some("no enabled MCP tools attached".to_string()),
                ));
            } else {
                for name in names {
                    tool_manifest.push(tool_manifest_item(
                        "mcp_runtime",
                        &name,
                        true,
                        true,
                        None,
                    ));
                }
            }
            clients
        }
        Err(err) => {
            tool_manifest.push(tool_manifest_item(
                "mcp_runtime",
                "(attach)",
                true,
                false,
                Some(err.clone()),
            ));
            eprintln!("[MCP] attach runtime tools skipped: {err}");
            Vec::new()
        }
    };

    if has_desktop_wait {
        tools.push(Box::new(BuiltinDesktopWaitTool));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "desktop-wait",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "desktop-wait",
            false,
            false,
            Some("disabled in api tools config".to_string()),
        ));
    }

    if has_shell_switch_workspace {
        let state = app_state
            .ok_or_else(|| "shell_switch_workspace requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinShellSwitchWorkspaceTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "shell-switch-workspace",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "shell-switch-workspace",
            false,
            false,
            Some("disabled in api tools config or locked workspace".to_string()),
        ));
    }

    if has_shell_exec {
        let state = app_state
            .ok_or_else(|| "shell_exec requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTerminalExecTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "shell-exec",
            true,
            true,
            None,
        ));
    } else {
        tool_manifest.push(tool_manifest_item(
            "builtin",
            "shell-exec",
            false,
            false,
            Some("disabled in api tools config".to_string()),
        ));
    }

    Ok(RuntimeToolAssembly {
        tools,
        tool_manifest,
        _mcp_screenshot_client: mcp_screenshot_client,
        _dynamic_mcp_clients: dynamic_mcp_clients,
    })
}

async fn try_attach_desktop_screenshot_mcp_tool(
    tools: &mut Vec<Box<dyn ToolDyn>>,
) -> Result<ScreenshotMcpClient, String> {
    let exe = std::env::current_exe()
        .map_err(|err| format!("Resolve current executable for MCP screenshot failed: {err}"))?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg(MCP_SCREENSHOT_SERVER_FLAG);
    let transport = rmcp::transport::TokioChildProcess::new(cmd)
        .map_err(|err| format!("Start MCP screenshot child process failed: {err}"))?;

    let client = ().serve(transport).await.map_err(|err| {
        format!("Connect to MCP screenshot server failed: {err}")
    })?;
    let sink = client.peer().clone();
    let defs = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP screenshot tools failed: {err}"))?;

    let mut attached = false;
    for def in defs {
        if def.name.as_ref() != MCP_SCREENSHOT_TOOL_NAME {
            continue;
        }
        tools.push(Box::new(rig::tool::rmcp::McpTool::from_mcp_server(
            def,
            sink.clone(),
        )));
        attached = true;
    }

    if !attached {
        return Err("MCP screenshot server did not expose desktop_screenshot tool".to_string());
    }
    Ok(client)
}

async fn call_model_openai_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    mut tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
) -> Result<ModelReply, String> {
    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;
    let tools = std::mem::take(&mut tool_assembly.tools);
    let agent = client
        .clone()
        .completions_api()
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .tools(tools)
        .build();
    run_openai_tool_loop(
        agent,
        prepared,
        on_delta,
        max_tool_iterations,
        selected_api.request_format.is_deepseek_kimi(),
    )
    .await
}

async fn call_model_openai_responses_with_tools(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    mut tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
) -> Result<ModelReply, String> {
    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;
    let tools = std::mem::take(&mut tool_assembly.tools);
    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .tools(tools)
        .build();
    run_openai_tool_loop(agent, prepared, on_delta, max_tool_iterations, false).await
}

async fn run_openai_tool_loop<M>(
    agent: rig::agent::Agent<M>,
    prepared: PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    include_reasoning_before_tool_calls: bool,
) -> Result<ModelReply, String>
where
    M: rig::completion::CompletionModel,
    <M as rig::completion::CompletionModel>::StreamingResponse: rig::completion::GetTokenUsage,
{
    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut prompt_blocks = vec![UserContent::text(prepared.latest_user_text.clone())];
    if !prepared.latest_user_time_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_time_text.clone()));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }
    let current_prompt_content = OneOrMany::many(prompt_blocks)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let mut current_prompt: RigMessage = RigMessage::User {
        content: current_prompt_content,
    };
    let mut chat_history = Vec::<RigMessage>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let mut user_blocks = vec![UserContent::text(hm.text.clone())];
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    user_blocks.push(UserContent::text(time_text.clone()));
                }
            }
            chat_history.push(RigMessage::User {
                content: OneOrMany::many(user_blocks)
                    .map_err(|_| "Failed to build user history message".to_string())?,
            });
        } else if hm.role == "assistant" {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::text(hm.text.clone())),
            });
        }
    }

    let max_rounds = std::cmp::max(1usize, max_tool_iterations);
    for _ in 0..max_rounds {
        let mut stream = agent
            .stream_completion(current_prompt.clone(), chat_history.clone())
            .await
            .map_err(|err| format!("rig stream completion build failed: {err}"))?
            .stream()
            .await
            .map_err(|err| format!("rig stream start failed: {err}"))?;

        chat_history.push(current_prompt.clone());

        let mut turn_text = String::new();
        let mut turn_reasoning = String::new();
        let mut tool_calls = Vec::<AssistantContent>::new();
        let mut tool_results = Vec::<(String, String, Option<String>, String)>::new();
        let mut did_call_tool = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamedAssistantContent::Text(text)) => {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: text.text.clone(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                    turn_text.push_str(&text.text);
                }
                Ok(StreamedAssistantContent::ToolCall {
                    tool_call,
                    internal_call_id: _,
                }) => {
                    did_call_tool = true;
                    let tool_call_id = tool_call.id.clone();
                    let tool_name = tool_call.function.name.clone();
                    let tool_args_value = tool_call.function.arguments.clone();
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "running",
                        &format!("正在调用工具：{}", tool_name),
                    );
                    let tool_args = match &tool_args_value {
                        Value::String(raw) => raw.clone(),
                        other => other.to_string(),
                    };
                    let tool_result = match agent
                        .tool_server_handle
                        .call_tool(&tool_name, &tool_args)
                        .await
                    {
                        Ok(output) => {
                            send_tool_status_event(
                                on_delta,
                                &tool_name,
                                "done",
                                &format!("工具调用完成：{}", tool_name),
                            );
                            output
                        }
                        Err(err) => {
                            let err_text = err.to_string();
                            send_tool_status_event(
                                on_delta,
                                &tool_name,
                                "failed",
                                &format!("工具调用失败：{} ({})", tool_name, err_text),
                            );
                            serde_json::json!({
                                "ok": false,
                                "tool": tool_name,
                                "error": err_text
                            })
                            .to_string()
                        }
                    };
                    tool_history_events.push(serde_json::json!({
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [
                            {
                                "id": tool_call_id,
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": tool_args_value
                                }
                            }
                        ]
                    }));
                    let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result);
                    tool_history_events.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": history_content
                    }));

                    tool_calls.push(AssistantContent::ToolCall(tool_call.clone()));
                    tool_results.push((tool_name, tool_call.id, tool_call.call_id, tool_result));
                }
                Ok(StreamedAssistantContent::Final(_)) => {}
                Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                    let merged = reasoning.reasoning.join("\n");
                    if !merged.is_empty() {
                        if !turn_reasoning.is_empty() {
                            turn_reasoning.push('\n');
                        }
                        turn_reasoning.push_str(&merged);
                        full_reasoning_standard.push_str(&merged);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: merged,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                    if !reasoning.is_empty() {
                        turn_reasoning.push_str(&reasoning);
                        full_reasoning_standard.push_str(&reasoning);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: reasoning,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
                Err(err) => return Err(format!("rig streaming failed: {err}")),
            }
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }

        if !did_call_tool {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                tool_history_events,
            });
        }

        if !tool_calls.is_empty() {
            let mut assistant_items = Vec::<AssistantContent>::new();
            if include_reasoning_before_tool_calls {
                assistant_items.push(AssistantContent::reasoning(turn_reasoning.clone()));
            }
            assistant_items.extend(tool_calls);
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(assistant_items)
                    .map_err(|_| "Failed to build assistant tool-call message".to_string())?,
            });
        }

        for (tool_name, tool_id, call_id, tool_result) in tool_results {
            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result);
            let result_content = OneOrMany::one(ToolResultContent::text(tool_result_for_model));
            let user_content = if let Some(call_id) = call_id {
                UserContent::tool_result_with_call_id(tool_id, call_id, result_content)
            } else {
                UserContent::tool_result(tool_id, result_content)
            };
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(user_content),
            });
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        mime: payload.mime.clone(),
                        base64: payload.base64.clone(),
                        width: payload.width,
                        height: payload.height,
                        created_seq: 0,
                    },
                );
                let forwarded = OneOrMany::many(vec![
                    UserContent::text(notice),
                    UserContent::image_base64(
                        cached.base64,
                        image_media_type_from_mime(&cached.mime),
                        Some(ImageDetail::Auto),
                    ),
                ])
                .map_err(|_| "Failed to build screenshot forward user message".to_string())?;
                chat_history.push(RigMessage::User { content: forwarded });
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotWidth": cached.width,
                    "screenshotHeight": cached.height
                }));
            }
        }

        current_prompt = chat_history
            .pop()
            .ok_or_else(|| "Tool call turn ended with empty chat history".to_string())?;
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        "工具调用达到上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events,
    })
}

async fn call_model_gemini_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_session_id: &str,
) -> Result<ModelReply, String> {
    let has_fetch = tool_enabled(selected_api, "fetch");
    let has_bing = tool_enabled(selected_api, "bing-search");
    let has_memory = tool_enabled(selected_api, "memory-save");
    let has_desktop_screenshot = tool_enabled(selected_api, "desktop-screenshot");
    let has_desktop_wait = tool_enabled(selected_api, "desktop-wait");
    let has_shell_switch_workspace =
        shell_switch_workspace_enabled_for_session(selected_api, app_state, tool_session_id);
    let has_shell_exec = tool_enabled(selected_api, "shell-exec");
    if !has_fetch
        && !has_bing
        && !has_memory
        && !has_desktop_screenshot
        && !has_desktop_wait
        && !has_shell_switch_workspace
        && !has_shell_exec
    {
        return call_model_gemini_rig_style(api_config, model_name, prepared).await;
    }

    let mut client_builder = gemini::Client::builder().api_key(&api_config.api_key);
    let normalized_base = normalize_gemini_rig_base_url(&api_config.base_url);
    if !normalized_base.is_empty() {
        client_builder = client_builder.base_url(&normalized_base);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Gemini client via rig: {err}"))?;

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    if has_fetch {
        tools.push(Box::new(BuiltinFetchTool));
    }
    if has_bing {
        tools.push(Box::new(BuiltinBingSearchTool));
    }
    if has_memory {
        let state = app_state
            .ok_or_else(|| "memory_save requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveTool { app_state: state }));
        let state = app_state
            .ok_or_else(|| "memory_save_batch requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveBatchTool { app_state: state }));
    }
    let mut _mcp_screenshot_client: Option<ScreenshotMcpClient> = None;
    if has_desktop_screenshot {
        let client = try_attach_desktop_screenshot_mcp_tool(&mut tools)
            .await
            .map_err(|err| {
                format!("desktop_screenshot MCP tool is unavailable (no builtin fallback): {err}")
            })?;
        _mcp_screenshot_client = Some(client);
    }
    let _dynamic_mcp_clients = match attach_enabled_mcp_tools_for_runtime(&mut tools, app_state).await {
        Ok((v, _)) => v,
        Err(err) => {
            eprintln!("[MCP] attach runtime tools skipped: {err}");
            Vec::new()
        }
    };
    if has_desktop_wait {
        tools.push(Box::new(BuiltinDesktopWaitTool));
    }
    if has_shell_switch_workspace {
        let state = app_state
            .ok_or_else(|| "shell_switch_workspace requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinShellSwitchWorkspaceTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
    }
    if has_shell_exec {
        let state = app_state
            .ok_or_else(|| "shell_exec requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTerminalExecTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
    }

    let gemini_safety_settings = serde_json::json!({
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" }
        ]
    });

    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .additional_params(gemini_safety_settings)
        .tools(tools)
        .build();

    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut prompt_blocks = vec![UserContent::text(prepared.latest_user_text.clone())];
    if !prepared.latest_user_time_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_time_text.clone()));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }
    let current_prompt_content = OneOrMany::many(prompt_blocks)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let mut current_prompt: RigMessage = RigMessage::User {
        content: current_prompt_content,
    };
    let mut chat_history = Vec::<RigMessage>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let mut user_blocks = vec![UserContent::text(hm.text.clone())];
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    user_blocks.push(UserContent::text(time_text.clone()));
                }
            }
            chat_history.push(RigMessage::User {
                content: OneOrMany::many(user_blocks)
                    .map_err(|_| "Failed to build user history message".to_string())?,
            });
        } else if hm.role == "assistant" {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::text(hm.text.clone())),
            });
        }
    }

    for _ in 0..max_tool_iterations {
        let mut stream = agent
            .stream_completion(current_prompt.clone(), chat_history.clone())
            .await
            .map_err(|err| format!("rig stream completion build failed: {err}"))?
            .stream()
            .await
            .map_err(|err| format!("rig stream start failed: {err}"))?;

        chat_history.push(current_prompt.clone());

        let mut turn_text = String::new();
        let mut tool_calls = Vec::<AssistantContent>::new();
        let mut tool_results = Vec::<(String, String, Option<String>, String)>::new();
        let mut did_call_tool = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamedAssistantContent::Text(text)) => {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: text.text.clone(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                    turn_text.push_str(&text.text);
                }
                Ok(StreamedAssistantContent::ToolCall {
                    tool_call,
                    internal_call_id: _,
                }) => {
                    did_call_tool = true;
                    let tool_call_id = tool_call.id.clone();
                    let tool_name = tool_call.function.name.clone();
                    let tool_args_value = tool_call.function.arguments.clone();
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "running",
                        &format!("正在调用工具：{}", tool_name),
                    );
                    let tool_args = match &tool_args_value {
                        Value::String(raw) => raw.clone(),
                        other => other.to_string(),
                    };
                    let tool_result = agent
                        .tool_server_handle
                        .call_tool(&tool_name, &tool_args)
                        .await
                        .map_err(|err| {
                            send_tool_status_event(
                                on_delta,
                                &tool_name,
                                "failed",
                                &format!("工具调用失败：{} ({err})", tool_name),
                            );
                            format!("Tool call '{}' failed: {err}", tool_name)
                        })?;
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "done",
                        &format!("工具调用完成：{}", tool_name),
                    );
                    let assistant_tool_event = serde_json::json!({
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [
                            {
                                "id": tool_call_id,
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": tool_args_value
                                }
                            }
                        ]
                    });
                    tool_history_events.push(assistant_tool_event);
                    let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result);
                    let tool_event = serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": history_content
                    });
                    tool_history_events.push(tool_event);

                    tool_calls.push(AssistantContent::ToolCall(tool_call.clone()));
                    tool_results.push((tool_name, tool_call.id, tool_call.call_id, tool_result));
                }
                Ok(StreamedAssistantContent::Final(_)) => {}
                Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                    let merged = reasoning.reasoning.join("\n");
                    if !merged.is_empty() {
                        full_reasoning_standard.push_str(&merged);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: merged,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                    if !reasoning.is_empty() {
                        full_reasoning_standard.push_str(&reasoning);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: reasoning,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
                Err(err) => return Err(format!("rig streaming failed: {err}")),
            }
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }

        if !did_call_tool {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                tool_history_events,
            });
        }

        if !tool_calls.is_empty() {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(tool_calls)
                    .map_err(|_| "Failed to build assistant tool-call message".to_string())?,
            });
        }

        for (tool_name, tool_id, call_id, tool_result) in tool_results {
            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result);
            let result_content = OneOrMany::one(ToolResultContent::text(tool_result_for_model));
            let user_content = if let Some(call_id) = call_id {
                UserContent::tool_result_with_call_id(tool_id, call_id, result_content)
            } else {
                UserContent::tool_result(tool_id, result_content)
            };
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(user_content),
            });
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        mime: payload.mime.clone(),
                        base64: payload.base64.clone(),
                        width: payload.width,
                        height: payload.height,
                        created_seq: 0,
                    },
                );
                let forwarded = OneOrMany::many(vec![
                    UserContent::text(notice),
                    UserContent::image_base64(
                        cached.base64,
                        image_media_type_from_mime(&cached.mime),
                        Some(ImageDetail::Auto),
                    ),
                ])
                .map_err(|_| "Failed to build screenshot forward user message".to_string())?;
                chat_history.push(RigMessage::User { content: forwarded });
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotWidth": cached.width,
                    "screenshotHeight": cached.height
                }));
            }
        }

        current_prompt = chat_history
            .pop()
            .ok_or_else(|| "Tool call turn ended with empty chat history".to_string())?;
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        "工具调用达到上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events,
    })
}

async fn call_model_anthropic_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_session_id: &str,
) -> Result<ModelReply, String> {
    let has_fetch = tool_enabled(selected_api, "fetch");
    let has_bing = tool_enabled(selected_api, "bing-search");
    let has_memory = tool_enabled(selected_api, "memory-save");
    let has_desktop_screenshot = tool_enabled(selected_api, "desktop-screenshot");
    let has_desktop_wait = tool_enabled(selected_api, "desktop-wait");
    let has_shell_switch_workspace =
        shell_switch_workspace_enabled_for_session(selected_api, app_state, tool_session_id);
    let has_shell_exec = tool_enabled(selected_api, "shell-exec");
    if !has_fetch
        && !has_bing
        && !has_memory
        && !has_desktop_screenshot
        && !has_desktop_wait
        && !has_shell_switch_workspace
        && !has_shell_exec
    {
        return call_model_anthropic_rig_style(api_config, model_name, prepared).await;
    }

    let mut client_builder: anthropic::ClientBuilder =
        anthropic::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Anthropic client via rig: {err}"))?;

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    if has_fetch {
        tools.push(Box::new(BuiltinFetchTool));
    }
    if has_bing {
        tools.push(Box::new(BuiltinBingSearchTool));
    }
    if has_memory {
        let state = app_state
            .ok_or_else(|| "memory_save requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveTool { app_state: state }));
        let state = app_state
            .ok_or_else(|| "memory_save_batch requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveBatchTool { app_state: state }));
    }
    let mut _mcp_screenshot_client: Option<ScreenshotMcpClient> = None;
    if has_desktop_screenshot {
        let client = try_attach_desktop_screenshot_mcp_tool(&mut tools)
            .await
            .map_err(|err| {
                format!("desktop_screenshot MCP tool is unavailable (no builtin fallback): {err}")
            })?;
        _mcp_screenshot_client = Some(client);
    }
    let _dynamic_mcp_clients = match attach_enabled_mcp_tools_for_runtime(&mut tools, app_state).await {
        Ok((v, _)) => v,
        Err(err) => {
            eprintln!("[MCP] attach runtime tools skipped: {err}");
            Vec::new()
        }
    };
    if has_desktop_wait {
        tools.push(Box::new(BuiltinDesktopWaitTool));
    }
    if has_shell_switch_workspace {
        let state = app_state
            .ok_or_else(|| "shell_switch_workspace requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinShellSwitchWorkspaceTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
    }
    if has_shell_exec {
        let state = app_state
            .ok_or_else(|| "shell_exec requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinTerminalExecTool {
            app_state: state,
            session_id: tool_session_id.to_string(),
        }));
    }

    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .tools(tools)
        .build();

    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut prompt_blocks = vec![UserContent::text(prepared.latest_user_text.clone())];
    if !prepared.latest_user_time_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_time_text.clone()));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }
    let current_prompt_content = OneOrMany::many(prompt_blocks)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let mut current_prompt: RigMessage = RigMessage::User {
        content: current_prompt_content,
    };
    let mut chat_history = Vec::<RigMessage>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let mut user_blocks = vec![UserContent::text(hm.text.clone())];
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    user_blocks.push(UserContent::text(time_text.clone()));
                }
            }
            chat_history.push(RigMessage::User {
                content: OneOrMany::many(user_blocks)
                    .map_err(|_| "Failed to build user history message".to_string())?,
            });
        } else if hm.role == "assistant" {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::text(hm.text.clone())),
            });
        }
    }

    for _ in 0..max_tool_iterations {
        let mut stream = agent
            .stream_completion(current_prompt.clone(), chat_history.clone())
            .await
            .map_err(|err| format!("rig stream completion build failed: {err}"))?
            .stream()
            .await
            .map_err(|err| format!("rig stream start failed: {err}"))?;

        chat_history.push(current_prompt.clone());

        let mut turn_text = String::new();
        let mut tool_calls = Vec::<AssistantContent>::new();
        let mut tool_results = Vec::<(String, String, Option<String>, String)>::new();
        let mut did_call_tool = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamedAssistantContent::Text(text)) => {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: text.text.clone(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                    turn_text.push_str(&text.text);
                }
                Ok(StreamedAssistantContent::ToolCall {
                    tool_call,
                    internal_call_id: _,
                }) => {
                    did_call_tool = true;
                    let tool_call_id = tool_call.id.clone();
                    let tool_name = tool_call.function.name.clone();
                    let tool_args_value = tool_call.function.arguments.clone();
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "running",
                        &format!("正在调用工具：{}", tool_name),
                    );
                    let tool_args = match &tool_args_value {
                        Value::String(raw) => raw.clone(),
                        other => other.to_string(),
                    };
                    let tool_result = agent
                        .tool_server_handle
                        .call_tool(&tool_name, &tool_args)
                        .await
                        .map_err(|err| {
                            send_tool_status_event(
                                on_delta,
                                &tool_name,
                                "failed",
                                &format!("工具调用失败：{} ({err})", tool_name),
                            );
                            format!("Tool call '{}' failed: {err}", tool_name)
                        })?;
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "done",
                        &format!("工具调用完成：{}", tool_name),
                    );
                    let assistant_tool_event = serde_json::json!({
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [
                            {
                                "id": tool_call_id,
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": tool_args_value
                                }
                            }
                        ]
                    });
                    tool_history_events.push(assistant_tool_event);
                    let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result);
                    let tool_event = serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": history_content
                    });
                    tool_history_events.push(tool_event);

                    tool_calls.push(AssistantContent::ToolCall(tool_call.clone()));
                    tool_results.push((tool_name, tool_call.id, tool_call.call_id, tool_result));
                }
                Ok(StreamedAssistantContent::Final(_)) => {}
                Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                    let merged = reasoning.reasoning.join("\n");
                    if !merged.is_empty() {
                        full_reasoning_standard.push_str(&merged);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: merged,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                    if !reasoning.is_empty() {
                        full_reasoning_standard.push_str(&reasoning);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: reasoning,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
                Err(err) => return Err(format!("rig streaming failed: {err}")),
            }
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }

        if !did_call_tool {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                tool_history_events,
            });
        }

        if !tool_calls.is_empty() {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(tool_calls)
                    .map_err(|_| "Failed to build assistant tool-call message".to_string())?,
            });
        }

        for (tool_name, tool_id, call_id, tool_result) in tool_results {
            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result);
            let result_content = OneOrMany::one(ToolResultContent::text(tool_result_for_model));
            let user_content = if let Some(call_id) = call_id {
                UserContent::tool_result_with_call_id(tool_id, call_id, result_content)
            } else {
                UserContent::tool_result(tool_id, result_content)
            };
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(user_content),
            });
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        mime: payload.mime.clone(),
                        base64: payload.base64.clone(),
                        width: payload.width,
                        height: payload.height,
                        created_seq: 0,
                    },
                );
                let forwarded = OneOrMany::many(vec![
                    UserContent::text(notice),
                    UserContent::image_base64(
                        cached.base64,
                        image_media_type_from_mime(&cached.mime),
                        Some(ImageDetail::Auto),
                    ),
                ])
                .map_err(|_| "Failed to build screenshot forward user message".to_string())?;
                chat_history.push(RigMessage::User { content: forwarded });
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotWidth": cached.width,
                    "screenshotHeight": cached.height
                }));
            }
        }

        current_prompt = chat_history
            .pop()
            .ok_or_else(|| "Tool call turn ended with empty chat history".to_string())?;
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        "工具调用达到上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events,
    })
}

async fn call_model_openai_style(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_session_id: &str,
) -> Result<ModelReply, String> {
    let mut prepared = prepared;
    if !selected_api.enable_image && !prepared.latest_images.is_empty() {
        prepared.latest_images.clear();
    }
    if !selected_api.enable_audio && !prepared.latest_audios.is_empty() {
        prepared.latest_audios.clear();
    }
    let started_at = std::time::Instant::now();
    let request_log = prepared_prompt_to_equivalent_request_json(
        &prepared,
        model_name,
        api_config.temperature,
    );
    let headers = masked_auth_headers(&api_config.api_key);
    let mut tool_manifest_for_log: Option<Value> = None;
    let result = if selected_api.request_format.is_gemini() {
        if selected_api.enable_tools
            && prepared.latest_images.is_empty()
            && prepared.latest_audios.is_empty()
        {
            call_model_gemini_with_tools(
                api_config,
                selected_api,
                model_name,
                prepared,
                app_state,
                on_delta,
                max_tool_iterations,
                tool_session_id,
            )
            .await
        } else {
            call_model_gemini_rig_style(api_config, model_name, prepared).await
        }
    } else if selected_api.request_format.is_anthropic() {
        if selected_api.enable_tools
            && prepared.latest_images.is_empty()
            && prepared.latest_audios.is_empty()
        {
            call_model_anthropic_with_tools(
                api_config,
                selected_api,
                model_name,
                prepared,
                app_state,
                on_delta,
                max_tool_iterations,
                tool_session_id,
            )
            .await
        } else {
            call_model_anthropic_rig_style(api_config, model_name, prepared).await
        }
    } else if is_openai_style_request_format(selected_api.request_format)
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        if selected_api.enable_tools {
            let tool_assembly =
                assemble_runtime_tools(selected_api, app_state, tool_session_id).await?;
            tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
            if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
                call_model_openai_responses_with_tools(
                    api_config,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                )
                .await
            } else {
                call_model_openai_with_tools(
                    api_config,
                    selected_api,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                )
                .await
            }
        } else if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
            call_model_openai_responses_rig_style(api_config, model_name, prepared, Some(on_delta))
                .await
        } else {
            call_model_openai_rig_style(api_config, model_name, prepared).await
        }
    } else {
        let original = prepared.clone();
        let rig_result = if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
            call_model_openai_responses_rig_style(api_config, model_name, prepared, Some(on_delta))
                .await
        } else {
            call_model_openai_rig_style(api_config, model_name, prepared).await
        };
        match rig_result {
            Ok(reply) => Ok(reply),
            Err(err)
                if !original.latest_images.is_empty() && is_image_unsupported_error(&err) =>
            {
                eprintln!(
                    "[CHAT] Model rejected image input, fallback to text-only request. error={}",
                    err
                );
                let mut fallback = original;
                fallback.latest_images.clear();
                fallback.latest_audios.clear();
                if selected_api.enable_tools {
                    let tool_assembly =
                        assemble_runtime_tools(selected_api, app_state, tool_session_id).await?;
                    tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
                    if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
                        call_model_openai_responses_with_tools(
                            api_config,
                            model_name,
                            fallback,
                            tool_assembly,
                            on_delta,
                            max_tool_iterations,
                        )
                        .await
                    } else {
                        call_model_openai_with_tools(
                            api_config,
                            selected_api,
                            model_name,
                            fallback,
                            tool_assembly,
                            on_delta,
                            max_tool_iterations,
                        )
                        .await
                    }
                } else if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
                    call_model_openai_responses_rig_style(
                        api_config,
                        model_name,
                        fallback,
                        Some(on_delta),
                    )
                    .await
                } else {
                    call_model_openai_rig_style(api_config, model_name, fallback).await
                }
            }
            Err(err) => Err(err),
        }
    };
    let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match &result {
        Ok(reply) => {
            push_llm_round_log(
                app_state,
                "chat",
                selected_api.request_format,
                &selected_api.name,
                model_name,
                &api_config.base_url,
                headers,
                tool_manifest_for_log.clone(),
                request_log,
                Some(model_reply_to_log_value(reply)),
                None,
                elapsed_ms,
            );
        }
        Err(err) => {
            push_llm_round_log(
                app_state,
                "chat",
                selected_api.request_format,
                &selected_api.name,
                model_name,
                &api_config.base_url,
                headers,
                tool_manifest_for_log,
                request_log,
                None,
                Some(err.clone()),
                elapsed_ms,
            );
        }
    }
    result
}

async fn describe_image_with_vision_api(
    vision_resolved: &ResolvedApiConfig,
    vision_api: &ApiConfig,
    image: &BinaryPart,
) -> Result<String, String> {
    let mime = image.mime.trim();
    let prepared = PreparedPrompt {
        preamble: "[SYSTEM PROMPT]\n你是图像理解助手。请读取图片中的关键信息并输出简洁中文描述，保留有价值的文本、数字、UI元素与上下文。".to_string(),
        history_messages: Vec::new(),
        latest_user_text: "请识别这张图片并给出可用于后续对话的文本描述。".to_string(),
        latest_user_time_text: String::new(),
        latest_user_system_text: String::new(),
        latest_images: vec![(
            if mime.is_empty() {
                "image/png".to_string()
            } else {
                mime.to_string()
            },
            image.bytes_base64.clone(),
        )],
        latest_audios: Vec::new(),
    };

    let reply = match vision_resolved.request_format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
            call_model_openai_rig_style(vision_resolved, &vision_api.model, prepared).await?
        }
        RequestFormat::OpenAIResponses => {
            call_model_openai_responses_rig_style(
                vision_resolved,
                &vision_api.model,
                prepared,
                None,
            )
            .await?
        }
        RequestFormat::Gemini => {
            call_model_gemini_rig_style(vision_resolved, &vision_api.model, prepared).await?
        }
        RequestFormat::Anthropic => {
            call_model_anthropic_rig_style(vision_resolved, &vision_api.model, prepared).await?
        }
        RequestFormat::OpenAITts
        | RequestFormat::OpenAIStt
        | RequestFormat::GeminiEmbedding
        | RequestFormat::OpenAIEmbedding
        | RequestFormat::OpenAIRerank => {
            return Err(
                format!(
                    "Vision request format '{}' is not supported.",
                    vision_resolved.request_format
                ),
            )
        }
    };
    Ok(reply.assistant_text)
}

