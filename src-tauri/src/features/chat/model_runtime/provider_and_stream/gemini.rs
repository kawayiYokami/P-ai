async fn call_model_gemini_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    run_genai_tool_loop(
        api_config,
        model_name,
        prepared,
        tool_assembly,
        ToolCallProtocolFamily::Gemini,
        genai::adapter::AdapterKind::Gemini,
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
