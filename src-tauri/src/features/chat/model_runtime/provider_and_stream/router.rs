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

async fn dispatch_openai_style_call(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    app_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    if selected_api.request_format.is_deepseek_kimi() {
        call_model_deepseek_with_tools(
            api_config,
            selected_api,
            model_name,
            prepared,
            tool_assembly,
            on_delta,
            max_tool_iterations,
            app_state,
            chat_session_key,
        )
        .await
    } else if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
        call_model_openai_responses_with_tools(
            api_config,
            model_name,
            prepared,
            tool_assembly,
            on_delta,
            max_tool_iterations,
            app_state,
            chat_session_key,
        )
        .await
    } else {
        call_model_openai_with_tools(
            api_config,
            model_name,
            prepared,
            tool_assembly,
            on_delta,
            max_tool_iterations,
            app_state,
            chat_session_key,
        )
        .await
    }
}

async fn call_model_openai_style(
    api_config: &ResolvedApiConfig,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
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
    let mut request_log = prepared_prompt_to_equivalent_request_json(
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
            let tool_assembly =
                assemble_runtime_tools(app_config, selected_api, agent, app_state, chat_session_key).await?;
            if tool_assembly.tools.is_empty() {
                call_model_gemini_rig_style(api_config, model_name, prepared).await
            } else {
                tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
                call_model_gemini_with_tools(
                    api_config,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                    app_state,
                    chat_session_key,
                )
                .await
            }
        } else {
            call_model_gemini_rig_style(api_config, model_name, prepared).await
        }
    } else if selected_api.request_format.is_anthropic() {
        if selected_api.enable_tools
            && prepared.latest_images.is_empty()
            && prepared.latest_audios.is_empty()
        {
            let tool_assembly =
                assemble_runtime_tools(app_config, selected_api, agent, app_state, chat_session_key).await?;
            if tool_assembly.tools.is_empty() {
                call_model_anthropic_rig_style(api_config, model_name, prepared).await
            } else {
                tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
                call_model_anthropic_with_tools(
                    api_config,
                    model_name,
                    prepared,
                    tool_assembly,
                    on_delta,
                    max_tool_iterations,
                    app_state,
                    chat_session_key,
                )
                .await
            }
        } else {
            call_model_anthropic_rig_style(api_config, model_name, prepared).await
        }
    } else if is_openai_style_request_format(selected_api.request_format)
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        if selected_api.enable_tools {
            let tool_assembly =
                assemble_runtime_tools(app_config, selected_api, agent, app_state, chat_session_key).await?;
            tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
            dispatch_openai_style_call(
                api_config,
                selected_api,
                model_name,
                prepared,
                tool_assembly,
                on_delta,
                max_tool_iterations,
                app_state,
                chat_session_key,
            )
            .await
        } else if selected_api.request_format.is_deepseek_kimi() {
            call_model_deepseek_rig_style(api_config, model_name, prepared, Some(on_delta)).await
        } else if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
            call_model_openai_responses_rig_style(api_config, model_name, prepared, Some(on_delta))
                .await
        } else {
            call_model_openai_rig_style(api_config, model_name, prepared).await
        }
    } else {
        let original = prepared.clone();
        let rig_result = if selected_api.request_format.is_deepseek_kimi() {
            call_model_deepseek_rig_style(api_config, model_name, prepared, Some(on_delta)).await
        } else if matches!(selected_api.request_format, RequestFormat::OpenAIResponses) {
            call_model_openai_responses_rig_style(api_config, model_name, prepared, Some(on_delta))
                .await
        } else {
            call_model_openai_rig_style(api_config, model_name, prepared).await
        };
        match rig_result {
            Ok(reply) => Ok(reply),
            Err(err)
                if ( !original.latest_images.is_empty() || prepared_has_any_history_image(&original))
                    && is_image_unsupported_error(&err) =>
            {
                eprintln!(
                    "[CHAT] Model rejected image input, fallback to text-only request. error={}",
                    err
                );
                let mut fallback = original;
                let _ = replace_disabled_multimodal_with_text(&mut fallback, false, true);
                request_log = prepared_prompt_to_equivalent_request_json(
                    &fallback,
                    model_name,
                    api_config.temperature,
                );
                if selected_api.enable_tools {
                    let tool_assembly =
                        assemble_runtime_tools(app_config, selected_api, agent, app_state, chat_session_key).await?;
                    tool_manifest_for_log = Some(Value::Array(tool_assembly.tool_manifest.clone()));
                    dispatch_openai_style_call(
                        api_config,
                        selected_api,
                        model_name,
                        fallback,
                        tool_assembly,
                        on_delta,
                        max_tool_iterations,
                        app_state,
                        chat_session_key,
                    )
                    .await
                } else if selected_api.request_format.is_deepseek_kimi() {
                    call_model_deepseek_rig_style(api_config, model_name, fallback, Some(on_delta))
                        .await
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
