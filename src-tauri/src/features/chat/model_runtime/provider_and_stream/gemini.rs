async fn call_model_gemini_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    mut tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let mut client_builder = gemini::Client::builder().api_key(&api_config.api_key);
    let normalized_base = normalize_gemini_rig_base_url(&api_config.base_url);
    if !normalized_base.trim().is_empty() {
        client_builder = client_builder.base_url(&normalized_base);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Gemini client via rig: {err}"))?;

    // 全部设为 BLOCK_NONE：这是用户自主使用的桌面 AI 助手，
    // 不应由 API 层面的 safety filter 截断用户的合法请求。
    let gemini_safety_settings = serde_json::json!({
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" },
            { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE" }
        ]
    });

    let tools = std::mem::take(&mut tool_assembly.tools);
    let agent_builder = client.agent(model_name).preamble(&prepared.preamble);
    let agent_builder = if let Some(temperature) = api_config.temperature {
        agent_builder.temperature(temperature)
    } else {
        agent_builder
    };
    let agent_builder = if let Some(max_output_tokens) = api_config.max_output_tokens {
        agent_builder.max_tokens(max_output_tokens as u64)
    } else {
        agent_builder
    };
    let agent = agent_builder
        .additional_params(gemini_safety_settings)
        .tools(tools)
        .build();

    run_unified_tool_loop(
        agent,
        prepared,
        ToolCallProtocolFamily::Gemini,
        selected_api,
        api_config,
        auto_compaction_context,
        on_delta,
        max_tool_iterations,
        false,
        tool_abort_state,
        chat_session_key,
    )
    .await
}
