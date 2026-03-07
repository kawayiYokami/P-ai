async fn call_model_anthropic_with_tools(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    mut tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let mut client_builder: anthropic::ClientBuilder =
        anthropic::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.trim().is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Anthropic client via rig: {err}"))?;

    let tools = std::mem::take(&mut tool_assembly.tools);
    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .tools(tools)
        .build();

    run_unified_tool_loop(
        agent,
        prepared,
        on_delta,
        max_tool_iterations,
        false,
        tool_abort_state,
        chat_session_key,
    )
    .await
}
