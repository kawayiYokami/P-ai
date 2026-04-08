fn replace_disabled_multimodal_with_text(
    prepared: &mut PreparedPrompt,
    enable_image: bool,
    enable_audio: bool,
) -> (usize, usize, usize, usize) {
    let mut history_images = 0usize;
    let mut history_audios = 0usize;
    let mut latest_images = 0usize;
    let mut latest_audios = 0usize;

    for hm in &mut prepared.history_messages {
        if hm.role != "user" {
            continue;
        }
        if !enable_image && !hm.images.is_empty() {
            let count = hm.images.len();
            history_images += count;
            hm.images.clear();
        }
        if !enable_audio && !hm.audios.is_empty() {
            let count = hm.audios.len();
            history_audios += count;
            hm.audios.clear();
        }
    }

    if !enable_image && !prepared.latest_images.is_empty() {
        latest_images = prepared.latest_images.len();
        prepared.latest_images.clear();
    }
    if !enable_audio && !prepared.latest_audios.is_empty() {
        latest_audios = prepared.latest_audios.len();
        prepared.latest_audios.clear();
    }

    (history_images, history_audios, latest_images, latest_audios)
}

fn prepared_has_any_history_image(prepared: &PreparedPrompt) -> bool {
    prepared
        .history_messages
        .iter()
        .any(|hm| hm.role == "user" && !hm.images.is_empty())
}

fn append_unavailable_tool_notices_to_prepared(
    prepared: &mut PreparedPrompt,
    notices: &[String],
) {
    let merged = notices
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if merged.is_empty() {
        return;
    }
    let block = prompt_xml_block(
        "unavailable tool notices",
        merged.iter().map(|item| item.to_string()).collect::<Vec<_>>().join("\n"),
    );
    if prepared.latest_user_extra_text.trim().is_empty() {
        prepared.latest_user_extra_text = block;
    } else {
        prepared.latest_user_extra_text = format!(
            "{}\n\n{}",
            prepared.latest_user_extra_text.trim(),
            block
        );
    }
}

fn apply_cached_system_message_user_fallback_to_prepared(
    prepared: &mut PreparedPrompt,
    base_url: &str,
    app_state: Option<&AppState>,
) {
    if provider_system_message_user_fallback(app_state, base_url)
        && move_system_preamble_to_user_prompt(prepared)
    {
        runtime_log_info(format!(
            "[聊天] base_url={} 已在本次运行内启用 system->user 降级，当前回合直接改写提示词",
            base_url
        ));
    }
}

async fn retry_openai_responses_with_system_message_user_fallback(
    api_config: &ResolvedApiConfig,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    model_name: &str,
    err: String,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    chat_session_key: &str,
    request_log: &mut Value,
    tool_manifest_for_log: &mut Option<Value>,
    allow_tools: bool,
) -> Result<ModelReply, String> {
    if let Err(mark_err) =
        provider_mark_system_message_user_fallback(app_state, &api_config.base_url)
    {
        runtime_log_warn(format!(
            "[聊天] 标记本次运行内 system->user 降级失败: base_url={}, err={}",
            api_config.base_url, mark_err
        ));
    }
    let mut fallback = prepared;
    if !move_system_preamble_to_user_prompt(&mut fallback) {
        return Err(err);
    }
    runtime_log_info(format!(
        "[聊天] 检测到上游不支持 system message，已在本次运行内切换 system->user 降级重试: base_url={}, model={}, err={}",
        api_config.base_url, model_name, err
    ));
    *request_log = prepared_prompt_to_equivalent_request_json(
        &fallback,
        model_name,
        api_config.temperature,
        api_config.max_output_tokens,
    );
    if allow_tools {
        let tool_assembly = prepare_openai_style_tool_assembly(
            app_config,
            selected_api,
            agent,
            &mut fallback,
            app_state,
            chat_session_key,
            tool_manifest_for_log,
        )
        .await?;
        execute_openai_style_request(
            api_config,
            selected_api,
            model_name,
            fallback,
            tool_assembly,
            false,
            on_delta,
            max_tool_iterations,
            app_state,
            auto_compaction_context,
            chat_session_key,
        )
        .await
    } else {
        call_model_openai_responses(api_config, model_name, fallback, Some(on_delta)).await
    }
}

async fn dispatch_openai_style_call(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    app_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
        call_model_openai_responses_with_tools(
            api_config,
            selected_api,
            model_name,
            prepared,
            tool_assembly,
            on_delta,
            max_tool_iterations,
            app_state,
            auto_compaction_context,
            chat_session_key,
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
            app_state,
            auto_compaction_context,
            chat_session_key,
        )
        .await
    }
}

async fn prepare_openai_style_tool_assembly(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    prepared: &mut PreparedPrompt,
    app_state: Option<&AppState>,
    chat_session_key: &str,
    tool_manifest_for_log: &mut Option<Value>,
) -> Result<Option<RuntimeToolAssembly>, String> {
    if !selected_api.enable_tools {
        return Ok(None);
    }
    let tool_assembly = assemble_runtime_tools(
        app_config,
        selected_api,
        agent,
        app_state,
        chat_session_key,
    )
    .await?;
    append_unavailable_tool_notices_to_prepared(
        prepared,
        &tool_assembly.unavailable_tool_notices,
    );
    *tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
    Ok(Some(tool_assembly))
}

async fn execute_openai_style_request(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: Option<RuntimeToolAssembly>,
    prefer_non_stream: bool,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    app_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    match tool_assembly {
        Some(tool_assembly) if !tool_assembly.tools.is_empty() => {
            if prefer_non_stream {
                match selected_api.request_format {
                    RequestFormat::OpenAI => {
                        call_model_openai_non_stream_with_tools(
                            api_config,
                            selected_api,
                            model_name,
                            prepared,
                            tool_assembly,
                            on_delta,
                            max_tool_iterations,
                            app_state,
                            auto_compaction_context,
                            chat_session_key,
                        )
                        .await
                    }
                    _ => Err(format!(
                        "Request format '{}' does not support non-stream fallback.",
                        selected_api.request_format
                    )),
                }
            } else {
                dispatch_openai_style_call(
                    api_config,
                    selected_api,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                    app_state,
                    auto_compaction_context,
                    chat_session_key,
                )
                .await
            }
        }
        _ => {
            if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
                call_model_openai_responses(api_config, model_name, prepared, Some(on_delta)).await
            } else if prefer_non_stream {
                call_model_openai_non_stream(api_config, model_name, prepared).await
            } else {
                call_model_openai_stream(api_config, model_name, prepared).await
            }
        }
    }
}

async fn call_openai_style_non_stream_fallback(
    api_config: &ResolvedApiConfig,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    model_name: &str,
    mut prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    chat_session_key: &str,
    tool_manifest_for_log: &mut Option<Value>,
) -> Result<ModelReply, String> {
    let tool_assembly = prepare_openai_style_tool_assembly(
        app_config,
        selected_api,
        agent,
        &mut prepared,
        app_state,
        chat_session_key,
        tool_manifest_for_log,
    )
    .await?;
    execute_openai_style_request(
        api_config,
        selected_api,
        model_name,
        prepared,
        tool_assembly,
        true,
        on_delta,
        max_tool_iterations,
        app_state,
        auto_compaction_context,
        chat_session_key,
    )
    .await
}

async fn call_model_openai_style(
    api_config: &ResolvedApiConfig,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let mut prepared = prepared;
    let _ = replace_disabled_multimodal_with_text(
        &mut prepared,
        selected_api.enable_image,
        selected_api.enable_audio,
    );
    let started_at = std::time::Instant::now();
    if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
        apply_cached_system_message_user_fallback_to_prepared(
            &mut prepared,
            &api_config.base_url,
            app_state,
        );
    }
    let mut request_log = prepared_prompt_to_equivalent_request_json(
        &prepared,
        model_name,
        api_config.temperature,
        api_config.max_output_tokens,
    );
    let headers = masked_auth_headers(&api_config.api_key);
    let mut tool_manifest_for_log: Option<Value> = None;
    let supports_non_stream_fallback =
        request_format_supports_non_stream_fallback(selected_api.request_format);
    let prefer_non_stream = supports_non_stream_fallback
        && provider_streaming_disabled(
            app_state,
            selected_api.request_format,
            &api_config.base_url,
            model_name,
        );
    let result = if selected_api.request_format.is_gemini() {
        if selected_api.enable_tools
            && prepared.latest_images.is_empty()
            && prepared.latest_audios.is_empty()
        {
            let tool_assembly =
                assemble_runtime_tools(app_config, selected_api, agent, app_state, chat_session_key).await?;
            append_unavailable_tool_notices_to_prepared(
                &mut prepared,
                &tool_assembly.unavailable_tool_notices,
            );
            if tool_assembly.tools.is_empty() {
                call_model_gemini(api_config, model_name, prepared).await
            } else {
                tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
                call_model_gemini_with_tools(
                    api_config,
                    selected_api,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                    app_state,
                    auto_compaction_context,
                    chat_session_key,
                )
                .await
            }
        } else {
            call_model_gemini(api_config, model_name, prepared).await
        }
    } else if selected_api.request_format.is_anthropic() {
        if selected_api.enable_tools
            && prepared.latest_images.is_empty()
            && prepared.latest_audios.is_empty()
        {
            let tool_assembly =
                assemble_runtime_tools(app_config, selected_api, agent, app_state, chat_session_key).await?;
            append_unavailable_tool_notices_to_prepared(
                &mut prepared,
                &tool_assembly.unavailable_tool_notices,
            );
            if tool_assembly.tools.is_empty() {
                call_model_anthropic(api_config, model_name, prepared).await
            } else {
                tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
                call_model_anthropic_with_tools(
                    api_config,
                    selected_api,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                    app_state,
                    auto_compaction_context,
                    chat_session_key,
                )
                .await
            }
        } else {
            call_model_anthropic(api_config, model_name, prepared).await
        }
    } else if is_openai_style_request_format(selected_api.request_format) {
        if prefer_non_stream {
            if selected_api.enable_tools {
                runtime_log_info(format!(
                    "[聊天] base_url={} 已在本次运行内禁用流式，当前回合改用非流式请求并尽量保留工具调用",
                    api_config.base_url
                ));
            }
            call_openai_style_non_stream_fallback(
                api_config,
                app_config,
                selected_api,
                agent,
                model_name,
                prepared,
                app_state,
                auto_compaction_context,
                on_delta,
                max_tool_iterations,
                chat_session_key,
                &mut tool_manifest_for_log,
            )
            .await
        } else {
            let tool_assembly = prepare_openai_style_tool_assembly(
                app_config,
                selected_api,
                agent,
                &mut prepared,
                app_state,
                chat_session_key,
                &mut tool_manifest_for_log,
            )
            .await?;
            let stream_result = execute_openai_style_request(
                api_config,
                selected_api,
                model_name,
                prepared.clone(),
                tool_assembly,
                false,
                on_delta,
                max_tool_iterations,
                app_state,
                auto_compaction_context,
                chat_session_key,
            )
            .await;
            match stream_result {
                Ok(reply) => {
                    if supports_non_stream_fallback {
                        if let Err(clear_err) = provider_clear_streaming_disabled(
                            app_state,
                            selected_api.request_format,
                            &api_config.base_url,
                            model_name,
                        ) {
                            runtime_log_warn(format!(
                                "[聊天] 清理流式降级缓存失败: base_url={}, model={}, err={}",
                                api_config.base_url, model_name, clear_err
                            ));
                        }
                    }
                    Ok(reply)
                }
                Err(err)
                    if matches!(selected_api.request_format, RequestFormat::OpenAIResponses)
                        && is_system_message_not_allowed_error(&err) =>
                {
                    retry_openai_responses_with_system_message_user_fallback(
                        api_config,
                        app_config,
                        selected_api,
                        agent,
                        model_name,
                        err,
                        prepared,
                        app_state,
                        auto_compaction_context,
                        on_delta,
                        max_tool_iterations,
                        chat_session_key,
                        &mut request_log,
                        &mut tool_manifest_for_log,
                        true,
                    )
                    .await
                }
                Err(err)
                    if supports_non_stream_fallback
                        && !is_image_unsupported_error(&err)
                        && is_streaming_format_error(&err) =>
                {
                    if let Err(mark_err) =
                        provider_mark_streaming_disabled(
                            app_state,
                            selected_api.request_format,
                            &api_config.base_url,
                            model_name,
                        )
                    {
                        runtime_log_warn(format!(
                            "[聊天] 标记本次运行内非流式 base_url 失败: base_url={}, err={}",
                            api_config.base_url, mark_err
                        ));
                    }
                    runtime_log_info(format!(
                        "[聊天] 流式失败，已在本次运行内切换非流式重试: base_url={}, model={}, err={}",
                        api_config.base_url, model_name, err
                    ));
                    if selected_api.enable_tools {
                        runtime_log_info(format!(
                            "[聊天] 当前 API 已启用工具，非流式兜底将继续尝试工具调用: base_url={}",
                            api_config.base_url
                        ));
                    }
                    call_openai_style_non_stream_fallback(
                        api_config,
                        app_config,
                        selected_api,
                        agent,
                        model_name,
                        prepared,
                        app_state,
                        auto_compaction_context,
                        on_delta,
                        max_tool_iterations,
                        chat_session_key,
                        &mut tool_manifest_for_log,
                    )
                    .await
                }
                Err(err)
                    if (!prepared.latest_images.is_empty() || prepared_has_any_history_image(&prepared))
                        && is_image_unsupported_error(&err) =>
                {
                    runtime_log_info(format!(
                        "[聊天] 模型不支持图片输入，回退到纯文本请求: err={}",
                        err
                    ));
                    let mut fallback = prepared;
                    let _ = replace_disabled_multimodal_with_text(&mut fallback, false, true);
                    request_log = prepared_prompt_to_equivalent_request_json(
                        &fallback,
                        model_name,
                        api_config.temperature,
                        api_config.max_output_tokens,
                    );
                    let tool_assembly = prepare_openai_style_tool_assembly(
                        app_config,
                        selected_api,
                        agent,
                        &mut fallback,
                        app_state,
                        chat_session_key,
                        &mut tool_manifest_for_log,
                    )
                    .await?;
                    execute_openai_style_request(
                        api_config,
                        selected_api,
                        model_name,
                        fallback,
                        tool_assembly,
                        prefer_non_stream,
                        on_delta,
                        max_tool_iterations,
                        app_state,
                        auto_compaction_context,
                        chat_session_key,
                    )
                    .await
                }
                Err(err) => Err(err),
            }
        }
    } else {
        Err(format!(
            "Request format '{}' is not implemented in chat router yet.",
            selected_api.request_format
        ))
    };
    let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match &result {
        Ok(reply) => {
            push_llm_round_log(
                app_state,
                Some(format!("round-{chat_session_key}")),
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
                Some(vec![LlmRoundLogStage {
                    stage: "model_round_total".to_string(),
                    elapsed_ms,
                    since_prev_ms: elapsed_ms,
                }]),
            );
        }
        Err(err) => {
            push_llm_round_log(
                app_state,
                Some(format!("round-{chat_session_key}")),
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
                Some(vec![LlmRoundLogStage {
                    stage: "model_round_total".to_string(),
                    elapsed_ms,
                    since_prev_ms: elapsed_ms,
                }]),
            );
        }
    }
    result
}
