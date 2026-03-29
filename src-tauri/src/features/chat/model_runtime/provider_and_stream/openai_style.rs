async fn call_model_openai_with_tools(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    mut tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.trim().is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;
    let tools = std::mem::take(&mut tool_assembly.tools);
    let agent_builder = client.clone().completions_api().agent(model_name);
    let agent_builder = if prepared.preamble.trim().is_empty() {
        agent_builder
    } else {
        agent_builder.preamble(&prepared.preamble)
    };
    let agent = agent_builder
        .temperature(api_config.temperature)
        .max_tokens(api_config.max_output_tokens as u64)
        .tools(tools)
        .build();
    run_unified_tool_loop(
        agent,
        prepared,
        ToolCallProtocolFamily::OpenAiChatLike,
        on_delta,
        max_tool_iterations,
        false,
        tool_abort_state,
        chat_session_key,
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
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.trim().is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;
    let tools = std::mem::take(&mut tool_assembly.tools);
    let agent_builder = client.agent(model_name);
    let agent_builder = if prepared.preamble.trim().is_empty() {
        agent_builder
    } else {
        agent_builder.preamble(&prepared.preamble)
    };
    let agent = agent_builder
        .temperature(api_config.temperature)
        .max_tokens(api_config.max_output_tokens as u64)
        .tools(tools)
        .build();
    run_unified_tool_loop(
        agent,
        prepared,
        ToolCallProtocolFamily::OpenAiResponses,
        on_delta,
        max_tool_iterations,
        false,
        tool_abort_state,
        chat_session_key,
    )
    .await
}
