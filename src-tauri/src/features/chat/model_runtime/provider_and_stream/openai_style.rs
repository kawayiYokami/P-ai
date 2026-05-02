async fn call_model_openai_with_tools(
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
    let adapter_kind = resolve_provider_genai_adapter_kind(api_config, model_name, genai::adapter::AdapterKind::OpenAI);
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

fn openai_style_adapter_kind_for_deepseek_kimi(
    api_config: &ResolvedApiConfig,
    model_name: &str,
) -> genai::adapter::AdapterKind {
    let base_url = api_config.base_url.to_ascii_lowercase();
    let model_name = model_name.to_ascii_lowercase();
    if base_url.contains("deepseek")
        || base_url.contains("moonshot")
        || model_name.contains("deepseek")
        || model_name.contains("kimi")
    {
        genai::adapter::AdapterKind::DeepSeek
    } else {
        genai::adapter::AdapterKind::OpenAI
    }
}

async fn call_model_openai_responses_with_tools(
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
    let adapter_kind = resolve_provider_genai_adapter_kind(api_config, model_name, genai::adapter::AdapterKind::OpenAIResp);
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

async fn call_model_deepseek_kimi_with_tools(
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
    let adapter_kind = resolve_provider_genai_adapter_kind(
        api_config,
        model_name,
        openai_style_adapter_kind_for_deepseek_kimi(api_config, model_name),
    );
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

async fn call_model_openai_non_stream_with_tools(
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
    let adapter_kind = resolve_provider_genai_adapter_kind(api_config, model_name, genai::adapter::AdapterKind::OpenAI);
    run_genai_tool_loop_non_stream(
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

async fn call_model_deepseek_kimi_non_stream_with_tools(
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
    let adapter_kind = resolve_provider_genai_adapter_kind(
        api_config,
        model_name,
        openai_style_adapter_kind_for_deepseek_kimi(api_config, model_name),
    );
    run_genai_tool_loop_non_stream(
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
