#[derive(Debug, Clone)]
struct CallPolicy {
    scene: &'static str,
    timeout_secs: Option<u64>,
    json_only: bool,
}

#[derive(Debug, Clone)]
struct ModelCallLogParts {
    scene: &'static str,
    request_format: RequestFormat,
    provider_name: String,
    model_name: String,
    base_url: String,
    headers: Vec<LlmRoundLogHeader>,
    tools: Option<Value>,
    response: Option<Value>,
    error: Option<String>,
    elapsed_ms: u64,
    timeline: Option<Vec<LlmRoundLogStage>>,
}

#[derive(Debug, Clone)]
struct ModelCallExecutionResult {
    result: Result<ModelReply, String>,
    log_parts: ModelCallLogParts,
}

struct ProviderSerialGuard {
    provider_id: String,
    model_name: String,
    acquired_at: std::time::Instant,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl Drop for ProviderSerialGuard {
    fn drop(&mut self) {
        let held_ms = self
            .acquired_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        runtime_log_info(format!(
            "[推理串行] 完成并释放供应商串行门: provider_id={}, model={}, held_ms={}",
            self.provider_id, self.model_name, held_ms
        ));
    }
}

fn push_model_call_log_parts(state: Option<&AppState>, execution: &ModelCallExecutionResult) {
    push_llm_round_log(
        state,
        None,
        None,
        execution.log_parts.scene,
        execution.log_parts.request_format,
        &execution.log_parts.provider_name,
        &execution.log_parts.model_name,
        &execution.log_parts.base_url,
        execution.log_parts.headers.clone(),
        execution.log_parts.tools.clone(),
        execution.log_parts.response.clone(),
        execution.log_parts.error.clone(),
        execution.log_parts.elapsed_ms,
        execution.log_parts.timeline.clone(),
    );
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

const PROVIDER_STREAMING_DISABLED_TTL_SECS: i64 = 10 * 60;

fn provider_base_url_cache_key(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn provider_streaming_cache_key(
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> String {
    let normalized_base_url = provider_base_url_cache_key(base_url);
    let normalized_model = model_name.trim();
    format!(
        "{}|{}|{}",
        request_format.as_str(),
        normalized_base_url,
        normalized_model
    )
}

fn prune_expired_provider_streaming_disabled_cache(
    cache: &mut std::collections::HashMap<String, i64>,
) {
    let now_ts = now_utc().unix_timestamp();
    cache.retain(|_, expires_at| *expires_at > now_ts);
}

fn provider_streaming_disabled_cached(
    state: Option<&AppState>,
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> bool {
    let Some(app_state) = state else {
        return false;
    };
    let key = provider_streaming_cache_key(request_format, base_url, model_name);
    let Ok(mut cache) = app_state.provider_streaming_disabled_keys.lock() else {
        return false;
    };
    prune_expired_provider_streaming_disabled_cache(&mut cache);
    cache.contains_key(&key)
}

fn provider_streaming_disabled(
    state: Option<&AppState>,
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> bool {
    provider_streaming_disabled_cached(state, request_format, base_url, model_name)
}

fn provider_mark_streaming_disabled(
    state: Option<&AppState>,
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> Result<(), String> {
    let Some(app_state) = state else {
        return Ok(());
    };
    let key = provider_streaming_cache_key(request_format, base_url, model_name);
    let Ok(mut cache) = app_state.provider_streaming_disabled_keys.lock() else {
        return Err("Failed to lock provider streaming disabled cache".to_string());
    };
    prune_expired_provider_streaming_disabled_cache(&mut cache);
    let expires_at = now_utc()
        .unix_timestamp()
        .saturating_add(PROVIDER_STREAMING_DISABLED_TTL_SECS);
    cache.insert(key, expires_at);
    Ok(())
}

fn provider_clear_streaming_disabled(
    state: Option<&AppState>,
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> Result<(), String> {
    let Some(app_state) = state else {
        return Ok(());
    };
    let key = provider_streaming_cache_key(request_format, base_url, model_name);
    let Ok(mut cache) = app_state.provider_streaming_disabled_keys.lock() else {
        return Err("Failed to lock provider streaming disabled cache".to_string());
    };
    prune_expired_provider_streaming_disabled_cache(&mut cache);
    cache.remove(&key);
    Ok(())
}

fn provider_system_message_user_fallback_cached(state: Option<&AppState>, base_url: &str) -> bool {
    let Some(app_state) = state else {
        return false;
    };
    let key = provider_base_url_cache_key(base_url);
    let Ok(cache) = app_state.provider_system_message_user_fallback_keys.lock() else {
        return false;
    };
    cache.contains(&key)
}

fn provider_system_message_user_fallback(state: Option<&AppState>, base_url: &str) -> bool {
    provider_system_message_user_fallback_cached(state, base_url)
}

async fn maybe_acquire_provider_serial_guard(
    state: Option<&AppState>,
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
) -> Result<Option<ProviderSerialGuard>, String> {
    if resolved_api.allow_concurrent_requests {
        return Ok(None);
    }
    let Some(app_state) = state else {
        return Ok(None);
    };
    let Some(provider_id) = resolved_api
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    runtime_log_info(format!(
        "[推理串行] 开始等待供应商串行门: provider_id={}, model={}",
        provider_id, model_name
    ));
    let gate = {
        let mut gates = app_state.provider_request_gates.lock().await;
        gates.entry(provider_id.to_string())
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    let wait_started = std::time::Instant::now();
    let guard = gate.lock_owned().await;
    let waited_ms = wait_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    runtime_log_info(format!(
        "[推理串行] 已进入供应商串行门: provider_id={}, model={}, waited_ms={}",
        provider_id, model_name, waited_ms
    ));
    Ok(Some(ProviderSerialGuard {
        provider_id: provider_id.to_string(),
        model_name: model_name.to_string(),
        acquired_at: std::time::Instant::now(),
        _guard: guard,
    }))
}

fn provider_mark_system_message_user_fallback(
    state: Option<&AppState>,
    base_url: &str,
) -> Result<(), String> {
    let Some(app_state) = state else {
        return Ok(());
    };
    let key = provider_base_url_cache_key(base_url);
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
    prepared_prompt_prepend_latest_user_extra_block(prepared, block);
    true
}

fn is_streaming_request_payload_format_error(err: &str) -> bool {
    let normalized = err.to_ascii_lowercase();
    (normalized.contains("request body")
        || normalized.contains("invalid request body")
        || normalized.contains("failed to deserialize the json body")
        || normalized.contains("body validation error")
        || normalized.contains("invalid type")
        || normalized.contains("expected a string")
        || normalized.contains("expected a map")
        || normalized.contains("expected a sequence")
        || normalized.contains("missing required parameter"))
        && !normalized.contains("timed out")
        && !normalized.contains("gateway timeout")
        && !normalized.contains("status code '5")
}

fn request_format_supports_non_stream_fallback(format: RequestFormat) -> bool {
    format.is_genai_chat() || format.is_auto()
}

async fn invoke_model_by_format(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    if resolved_api.request_format.is_genai_chat() || resolved_api.request_format.is_auto() {
        return call_model_openai_stream(resolved_api, model_name, prepared, app_state).await;
    }
    Err(format!(
        "Request format '{}' is not supported for inference gateway.",
        resolved_api.request_format
    ))
}

async fn invoke_model_non_stream_by_format(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    if resolved_api.request_format.is_genai_chat() || resolved_api.request_format.is_auto() {
        return call_model_openai_non_stream(resolved_api, model_name, prepared, app_state).await;
    }
    invoke_model_by_format(resolved_api, model_name, prepared, app_state).await
}

async fn invoke_model_by_format_with_timeout(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    timeout_secs: u64,
    scene: &str,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_by_format(resolved_api, model_name, prepared, app_state),
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
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_non_stream_by_format(resolved_api, model_name, prepared, app_state),
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
) -> ModelCallExecutionResult {
    let started_at = std::time::Instant::now();
    let mut prepared = prepared;
    let stream_cache_key = provider_streaming_cache_key(
        resolved_api.request_format,
        &resolved_api.base_url,
        model_name,
    );
    if resolved_api.request_format.is_openai_responses_family()
        && provider_system_message_user_fallback(app_state, &resolved_api.base_url)
        && move_system_preamble_to_user_prompt(&mut prepared)
    {
        runtime_log_info(format!(
            "[推理] key={}, scene={} 已在本次运行内启用 system->user 降级，当前回合直接改写提示词",
            stream_cache_key, policy.scene
        ));
    }
    let headers = masked_auth_headers(&resolved_api.api_key);
    if policy.json_only {
        // json_only only constrains output contract + caller-side JSON parse.
        // It must not implicitly force non-stream because some upstreams require stream=true.
    }
    let prefer_non_stream = provider_streaming_disabled(
        app_state,
        resolved_api.request_format,
        &resolved_api.base_url,
        model_name,
    );
    let first_result = if prefer_non_stream {
        if let Some(timeout_secs) = policy.timeout_secs {
            invoke_model_non_stream_by_format_with_timeout(
                resolved_api,
                model_name,
                prepared.clone(),
                timeout_secs,
                policy.scene,
                app_state,
            )
            .await
        } else {
            invoke_model_non_stream_by_format(resolved_api, model_name, prepared.clone(), app_state)
                .await
        }
    } else {
        if let Some(timeout_secs) = policy.timeout_secs {
            invoke_model_by_format_with_timeout(
                resolved_api,
                model_name,
                prepared.clone(),
                timeout_secs,
                policy.scene,
                app_state,
            )
            .await
        } else {
            invoke_model_by_format(resolved_api, model_name, prepared.clone(), app_state).await
        }
    };
    let stream_first_attempt_succeeded = !prefer_non_stream
        && request_format_supports_non_stream_fallback(resolved_api.request_format)
        && first_result.is_ok();
    let result = match first_result {
        Ok(reply) => Ok(reply),
        Err(err)
            if resolved_api.request_format.is_openai_responses_family()
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
                if let Some(timeout_secs) = policy.timeout_secs {
                    invoke_model_by_format_with_timeout(
                        resolved_api,
                        model_name,
                        fallback,
                        timeout_secs,
                        policy.scene,
                        app_state,
                    )
                    .await
                } else {
                    invoke_model_by_format(resolved_api, model_name, fallback, app_state).await
                }
            }
        }
        Err(err)
            if !prefer_non_stream
                && request_format_supports_non_stream_fallback(resolved_api.request_format)
                && is_streaming_request_payload_format_error(&err) =>
        {
            if let Err(mark_err) = provider_mark_streaming_disabled(
                app_state,
                resolved_api.request_format,
                &resolved_api.base_url,
                model_name,
            ) {
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
                    app_state,
                )
                .await
            } else {
                invoke_model_non_stream_by_format(resolved_api, model_name, prepared, app_state)
                    .await
            }
        }
        Err(err) => Err(err),
    };
    if stream_first_attempt_succeeded {
        if let Err(clear_err) = provider_clear_streaming_disabled(
            app_state,
            resolved_api.request_format,
            &resolved_api.base_url,
            model_name,
        ) {
            runtime_log_warn(format!(
                "[推理] 清理流式降级缓存失败: key={}, scene={}, err={}",
                stream_cache_key, policy.scene, clear_err
            ));
        }
    }
    let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let provider_name = resolved_api
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown-provider")
        .to_string();
    let log_parts = match &result {
        Ok(reply) => ModelCallLogParts {
            scene: policy.scene,
            request_format: resolved_api.request_format,
            provider_name: provider_name.clone(),
            model_name: model_name.to_string(),
            base_url: resolved_api.base_url.clone(),
            headers,
            tools: None,
            response: Some(model_reply_to_log_value(reply)),
            error: None,
            elapsed_ms,
            timeline: None,
        },
        Err(err) => ModelCallLogParts {
            scene: policy.scene,
            request_format: resolved_api.request_format,
            provider_name,
            model_name: model_name.to_string(),
            base_url: resolved_api.base_url.clone(),
            headers,
            tools: None,
            response: None,
            error: Some(err.clone()),
            elapsed_ms,
            timeline: None,
        },
    };
    ModelCallExecutionResult { result, log_parts }
}

async fn call_archive_summary_model_with_timeout(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    prepared: PreparedPrompt,
    timeout_secs: u64,
) -> ModelCallExecutionResult {
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
        assert!(is_streaming_request_payload_format_error(
            "ProviderError: Invalid status code 400 Bad Request with message: {\"detail\":\"failed to deserialize the json body\"}"
        ));
        assert!(is_streaming_request_payload_format_error(
            "ProviderError: Invalid status code 400 Bad Request with message: {\"detail\":\"Invalid request body: expected a string\"}"
        ));
        assert!(!is_streaming_request_payload_format_error(
            "streaming failed: ResponseError: Failed to parse JSON: missing field `role`"
        ));
        assert!(!is_streaming_request_payload_format_error(
            "streaming failed: message_start unexpected"
        ));
        assert!(!is_streaming_request_payload_format_error(
            "Request failed with status code '504 Gateway Timeout'"
        ));
        assert!(!is_streaming_request_payload_format_error("request timed out"));
    }

    #[test]
    fn provider_cache_key_should_include_format_base_url_and_model() {
        let key = provider_streaming_cache_key(
            RequestFormat::OpenAI,
            "https://api.moonshot.cn/v1/",
            "kimi-k2.5",
        );
        assert_eq!(key, "openai|https://api.moonshot.cn/v1|kimi-k2.5");
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
            latest_user_extra_blocks: vec!["原有补充".to_string()],
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
