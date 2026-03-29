#[derive(Debug, Clone)]
struct CallPolicy {
    scene: &'static str,
    timeout_secs: Option<u64>,
    json_only: bool,
}

impl CallPolicy {
    fn archive_json(timeout_secs: u64) -> Self {
        Self {
            scene: "Archive summary",
            timeout_secs: Some(timeout_secs),
            json_only: true,
        }
    }

}

fn provider_streaming_cache_key(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn provider_streaming_disabled_cached(state: Option<&AppState>, base_url: &str) -> bool {
    let Some(app_state) = state else {
        return false;
    };
    let key = provider_streaming_cache_key(base_url);
    let Ok(cache) = app_state.provider_streaming_disabled_keys.lock() else {
        return false;
    };
    cache.contains(&key)
}

fn provider_streaming_disabled(state: Option<&AppState>, base_url: &str) -> bool {
    provider_streaming_disabled_cached(state, base_url)
}

fn provider_mark_streaming_disabled(state: Option<&AppState>, base_url: &str) -> Result<(), String> {
    let Some(app_state) = state else {
        return Ok(());
    };
    let key = provider_streaming_cache_key(base_url);
    let Ok(mut cache) = app_state.provider_streaming_disabled_keys.lock() else {
        return Err("Failed to lock provider streaming disabled cache".to_string());
    };
    cache.insert(key);
    Ok(())
}

fn provider_system_message_user_fallback_cached(state: Option<&AppState>, base_url: &str) -> bool {
    let Some(app_state) = state else {
        return false;
    };
    let key = provider_streaming_cache_key(base_url);
    let Ok(cache) = app_state.provider_system_message_user_fallback_keys.lock() else {
        return false;
    };
    cache.contains(&key)
}

fn provider_system_message_user_fallback(state: Option<&AppState>, base_url: &str) -> bool {
    provider_system_message_user_fallback_cached(state, base_url)
}

fn provider_mark_system_message_user_fallback(
    state: Option<&AppState>,
    base_url: &str,
) -> Result<(), String> {
    let Some(app_state) = state else {
        return Ok(());
    };
    let key = provider_streaming_cache_key(base_url);
    let Ok(mut cache) = app_state.provider_system_message_user_fallback_keys.lock() else {
        return Err("Failed to lock provider system message fallback cache".to_string());
    };
    cache.insert(key);
    Ok(())
}

fn is_system_message_not_allowed_error(err: &str) -> bool {
    let normalized = err.to_ascii_lowercase();
    normalized.contains("system messages are not allowed")
        || normalized.contains("system message is not allowed")
}

fn move_system_preamble_to_user_prompt(prepared: &mut PreparedPrompt) -> bool {
    let preamble = prepared.preamble.trim().to_string();
    if preamble.is_empty() {
        return false;
    }
    let block = prompt_xml_block("system prompt", preamble);
    prepared.preamble.clear();
    if prepared.latest_user_extra_text.trim().is_empty() {
        prepared.latest_user_extra_text = block;
    } else {
        prepared.latest_user_extra_text = format!(
            "{}\n\n{}",
            block,
            prepared.latest_user_extra_text.trim()
        );
    }
    true
}

#[cfg(test)]
fn is_streaming_format_error(err: &str) -> bool {
    err.contains("missing field `role`")
        || err.contains("message_start")
        || err.contains("message_delta")
        || err.contains("Failed to parse JSON")
        || err.contains("streaming failed")
}

fn request_format_supports_non_stream_fallback(format: RequestFormat) -> bool {
    matches!(format, RequestFormat::OpenAI | RequestFormat::DeepSeekKimi)
}

async fn invoke_model_rig_by_format(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    match resolved_api.request_format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
            call_model_openai_rig_style(resolved_api, model_name, prepared).await
        }
        RequestFormat::OpenAIResponses => {
            call_model_openai_responses_rig_style(resolved_api, model_name, prepared, None).await
        }
        RequestFormat::Gemini => call_model_gemini_rig_style(resolved_api, model_name, prepared).await,
        RequestFormat::Anthropic => {
            call_model_anthropic_rig_style(resolved_api, model_name, prepared).await
        }
        RequestFormat::OpenAITts
        | RequestFormat::OpenAIStt
        | RequestFormat::GeminiEmbedding
        | RequestFormat::OpenAIEmbedding
        | RequestFormat::OpenAIRerank => Err(format!(
            "Request format '{}' is not supported for this non-stream inference.",
            resolved_api.request_format
        )),
    }
}

async fn invoke_model_non_stream_by_format(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    match resolved_api.request_format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
            call_model_openai_non_stream_rig_style(resolved_api, model_name, prepared).await
        }
        _ => invoke_model_rig_by_format(resolved_api, model_name, prepared).await,
    }
}

async fn invoke_model_rig_by_format_with_timeout(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    timeout_secs: u64,
    scene: &str,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_rig_by_format(resolved_api, model_name, prepared),
    )
    .await
    .map_err(|_| {
        format!(
            "{scene} request timed out (elapsed={}ms, timeout={}s)",
            call_started.elapsed().as_millis(),
            timeout_secs
        )
    })?
}

async fn invoke_model_non_stream_by_format_with_timeout(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    timeout_secs: u64,
    scene: &str,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_non_stream_by_format(resolved_api, model_name, prepared),
    )
    .await
    .map_err(|_| {
        format!(
            "{scene} request timed out (elapsed={}ms, timeout={}s)",
            call_started.elapsed().as_millis(),
            timeout_secs
        )
    })?
}

async fn invoke_model_with_policy(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    policy: CallPolicy,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let started_at = std::time::Instant::now();
    let mut prepared = prepared;
    let stream_cache_key = provider_streaming_cache_key(&resolved_api.base_url);
    if matches!(resolved_api.request_format, RequestFormat::OpenAIResponses)
        && provider_system_message_user_fallback(app_state, &resolved_api.base_url)
        && move_system_preamble_to_user_prompt(&mut prepared)
    {
        runtime_log_info(format!(
            "[推理] key={}, scene={} 已在本次运行内启用 system->user 降级，当前回合直接改写提示词",
            stream_cache_key, policy.scene
        ));
    }
    let mut request_log = prepared_prompt_to_equivalent_request_json(
        &prepared,
        model_name,
        resolved_api.temperature,
    );
    let headers = masked_auth_headers(&resolved_api.api_key);
    if policy.json_only {
        // json_only is enforced by prompt contract + caller-side JSON parse.
    }
    let prefer_non_stream = provider_streaming_disabled(app_state, &resolved_api.base_url);
    let first_result = if prefer_non_stream {
        if let Some(timeout_secs) = policy.timeout_secs {
            invoke_model_non_stream_by_format_with_timeout(
                resolved_api,
                model_name,
                prepared.clone(),
                timeout_secs,
                policy.scene,
            )
            .await
        } else {
            invoke_model_non_stream_by_format(resolved_api, model_name, prepared.clone()).await
        }
    } else {
        if let Some(timeout_secs) = policy.timeout_secs {
            invoke_model_rig_by_format_with_timeout(
                resolved_api,
                model_name,
                prepared.clone(),
                timeout_secs,
                policy.scene,
            )
            .await
        } else {
            invoke_model_rig_by_format(resolved_api, model_name, prepared.clone()).await
        }
    };
    let result = match first_result {
        Ok(reply) => Ok(reply),
        Err(err)
            if matches!(resolved_api.request_format, RequestFormat::OpenAIResponses)
                && is_system_message_not_allowed_error(&err) =>
        {
            if let Err(mark_err) =
                provider_mark_system_message_user_fallback(app_state, &resolved_api.base_url)
            {
                runtime_log_warn(format!(
                    "[推理] 标记本次运行内 system->user 降级失败: key={}, scene={}, err={}",
                    stream_cache_key, policy.scene, mark_err
                ));
            }
            let mut fallback = prepared;
            if !move_system_preamble_to_user_prompt(&mut fallback) {
                Err(err)
            } else {
                runtime_log_info(format!(
                    "[推理] 检测到上游不支持 system message，已在本次运行内切换 system->user 降级重试: key={}, scene={}, err={}",
                    stream_cache_key, policy.scene, err
                ));
                request_log = prepared_prompt_to_equivalent_request_json(
                    &fallback,
                    model_name,
                    resolved_api.temperature,
                );
                if let Some(timeout_secs) = policy.timeout_secs {
                    invoke_model_rig_by_format_with_timeout(
                        resolved_api,
                        model_name,
                        fallback,
                        timeout_secs,
                        policy.scene,
                    )
                    .await
                } else {
                    invoke_model_rig_by_format(resolved_api, model_name, fallback).await
                }
            }
        }
        Err(err)
            if !prefer_non_stream
                && request_format_supports_non_stream_fallback(resolved_api.request_format) =>
        {
            if let Err(mark_err) = provider_mark_streaming_disabled(app_state, &resolved_api.base_url) {
                runtime_log_warn(format!(
                    "[推理] 标记本次运行内非流式 base_url 失败: key={}, scene={}, err={}",
                    stream_cache_key, policy.scene, mark_err
                ));
            }
            runtime_log_info(format!(
                "[推理] 流式失败，已在本次运行内切换为非流式: key={}, scene={}, err={}",
                stream_cache_key, policy.scene, err
            ));
            if let Some(timeout_secs) = policy.timeout_secs {
                invoke_model_non_stream_by_format_with_timeout(
                    resolved_api,
                    model_name,
                    prepared,
                    timeout_secs,
                    policy.scene,
                )
                .await
            } else {
                invoke_model_non_stream_by_format(resolved_api, model_name, prepared).await
            }
        }
        Err(err) => Err(err),
    };
    let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match &result {
        Ok(reply) => {
            push_llm_round_log(
                app_state,
                None,
                policy.scene,
                resolved_api.request_format,
                policy.scene,
                model_name,
                &resolved_api.base_url,
                headers,
                None,
                request_log,
                Some(model_reply_to_log_value(reply)),
                None,
                elapsed_ms,
                None,
            );
        }
        Err(err) => {
            push_llm_round_log(
                app_state,
                None,
                policy.scene,
                resolved_api.request_format,
                policy.scene,
                model_name,
                &resolved_api.base_url,
                headers,
                None,
                request_log,
                None,
                Some(err.clone()),
                elapsed_ms,
                None,
            );
        }
    }
    result
}

async fn call_archive_summary_model_with_timeout(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    prepared: PreparedPrompt,
    timeout_secs: u64,
) -> Result<ModelReply, String> {
    invoke_model_with_policy(
        resolved_api,
        &selected_api.model,
        prepared,
        CallPolicy::archive_json(timeout_secs),
        Some(state),
    )
    .await
}

#[cfg(test)]
mod inference_gateway_tests {
    use super::*;

    #[test]
    fn streaming_error_detector_should_match_known_patterns() {
        assert!(is_streaming_format_error(
            "rig streaming failed: ResponseError: Failed to parse JSON: missing field `role`"
        ));
        assert!(is_streaming_format_error(
            "streaming failed: message_start unexpected"
        ));
        assert!(!is_streaming_format_error("request timed out"));
    }

    #[test]
    fn provider_cache_key_should_keep_raw_base_url() {
        let key = provider_streaming_cache_key("https://api.moonshot.cn/v1/");
        assert_eq!(key, "https://api.moonshot.cn/v1");
    }

    #[test]
    fn system_message_error_detector_should_match_known_patterns() {
        assert!(is_system_message_not_allowed_error(
            "ProviderError: Invalid status code 400 Bad Request with message: {\"detail\":\"System messages are not allowed\"}"
        ));
        assert!(is_system_message_not_allowed_error(
            "system message is not allowed for this upstream"
        ));
        assert!(!is_system_message_not_allowed_error("streaming failed"));
    }

    #[test]
    fn move_system_preamble_to_user_prompt_should_clear_preamble_and_prepend_extra() {
        let mut prepared = PreparedPrompt {
            preamble: "你是严谨助手".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "你好".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: "原有补充".to_string(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        assert!(move_system_preamble_to_user_prompt(&mut prepared));
        assert!(prepared.preamble.is_empty());
        assert!(prepared.latest_user_extra_text.contains("你是严谨助手"));
        assert!(prepared.latest_user_extra_text.starts_with("<system prompt>"));
        assert!(prepared.latest_user_extra_text.ends_with("原有补充"));
    }
}
