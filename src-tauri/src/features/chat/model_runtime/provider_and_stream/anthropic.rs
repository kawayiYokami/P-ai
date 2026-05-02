async fn call_model_anthropic_with_tools(
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
    let adapter_kind = resolve_provider_genai_adapter_kind(api_config, model_name, genai::adapter::AdapterKind::Anthropic);
    run_genai_tool_loop(
        api_config,
        model_name,
        prepared,
        tool_assembly,
        adapter_kind,
        selected_api,
        api_config,
        auto_compaction_context,
        on_delta,
        max_tool_iterations,
        tool_abort_state,
        chat_session_key,
    )
    .await
}
