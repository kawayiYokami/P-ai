#[derive(Debug, Clone)]
struct CallPolicy {
    scene: &'static str,
    timeout_secs: Option<u64>,
    json_only: bool,
}

#[allow(dead_code)]
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
    /// 压缩重启时交给外层调度上下文的压缩保留消息；仅重启路径使用。
    compaction_preserved_messages: Option<CompactionPreservedMessages>,
}

struct ProviderConcurrencyGuard {
    provider_id: String,
    model_name: String,
    acquired_at: std::time::Instant,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Drop for ProviderConcurrencyGuard {
    fn drop(&mut self) {
        let held_ms = self
            .acquired_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        runtime_log_info(format!(
            "[推理并发] 完成并释放供应商并发门: provider_id={}, model={}, held_ms={}",
            self.provider_id, self.model_name, held_ms
        ));
    }
}

fn push_model_call_log_parts(_state: Option<&AppState>, _execution: &ModelCallExecutionResult) {
    // 已切换至调度事件：旧推理网关日志不再写入
}

fn elapsed_ms_u64(started_at: std::time::Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn fast_request_response_text_from_reply(reply: &ModelReply) -> String {
    let final_response_text = reply.final_response_text.trim();
    if !final_response_text.is_empty() {
        return final_response_text.to_string();
    }
    reply.assistant_text.trim().to_string()
}

fn prepared_prompt_to_fast_request_text(prepared: &PreparedPrompt) -> String {
    let mut blocks = Vec::<String>::new();
    if !prepared.preamble.trim().is_empty() {
        blocks.push(format!("system:\n{}", prepared.preamble.trim()));
    }
    for (index, message) in prepared.history_messages.iter().enumerate() {
        let mut message_blocks = Vec::<String>::new();
        if !message.text.trim().is_empty() {
            message_blocks.push(message.text.trim().to_string());
        }
        for extra in &message.extra_text_blocks {
            if !extra.trim().is_empty() {
                message_blocks.push(extra.trim().to_string());
            }
        }
        if !message_blocks.is_empty() {
            blocks.push(format!(
                "history {} {}:\n{}",
                index + 1,
                message.role.trim(),
                message_blocks.join("\n\n")
            ));
        }
    }
    let mut latest_blocks = Vec::<String>::new();
    for item in [
        prepared.latest_user_meta_text.trim(),
        prepared.latest_user_text.trim(),
        prepared.latest_user_extra_text.trim(),
    ] {
        if !item.is_empty() {
            latest_blocks.push(item.to_string());
        }
    }
    for extra in &prepared.latest_user_extra_blocks {
        if !extra.trim().is_empty() {
            latest_blocks.push(extra.trim().to_string());
        }
    }
    if !prepared.latest_images.is_empty() {
        latest_blocks.push(format!("images: {}", prepared.latest_images.len()));
    }
    if !prepared.latest_audios.is_empty() {
        latest_blocks.push(format!("audios: {}", prepared.latest_audios.len()));
    }
    if !latest_blocks.is_empty() {
        blocks.push(format!("user:\n{}", latest_blocks.join("\n\n")));
    }
    blocks.join("\n\n---\n\n")
}

fn build_fast_request_turn(
    kind: &str,
    request_text: &str,
    response_text: &str,
    success: bool,
    error: Option<String>,
    model_name: Option<String>,
    duration_ms: Option<u64>,
) -> FastRequestTurn {
    FastRequestTurn {
        id: Uuid::new_v4().to_string(),
        kind: kind.trim().to_string(),
        request_text: request_text.trim().to_string(),
        response_text: response_text.trim().to_string(),
        success,
        error: error
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        model_name: model_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        duration_ms,
        created_at: now_iso(),
    }
}

fn record_fast_request_turn_best_effort(
    state: &AppState,
    conversation_id: &str,
    turn: FastRequestTurn,
) {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return;
    }
    let kind = turn.kind.clone();
    match conversation_service_v2()
        .append_fast_request_turn_if_unarchived_exists(state, normalized_conversation_id, turn)
    {
        Ok(true) => remote_im_request_24h_maintenance_for_conversation(
            state.clone(),
            normalized_conversation_id,
        ),
        Ok(false) => {}
        Err(err) => runtime_log_warn(format!(
            "[快速请求记录] 失败，任务=追加会话快速请求记录，conversation_id={}，kind={}，error={}",
            normalized_conversation_id,
            kind,
            err
        )),
    }
}

#[derive(Debug, Clone)]
struct FastRequestRecordTarget {
    conversation_id: String,
    kind: &'static str,
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

async fn maybe_acquire_provider_concurrency_guard(
    state: Option<&AppState>,
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
) -> Result<Option<ProviderConcurrencyGuard>, String> {
    let effective_max: Option<usize> = if let Some(max) = resolved_api.max_concurrent_requests {
        Some(max.max(1) as usize)
    } else if resolved_api.allow_concurrent_requests {
        return Ok(None);
    } else {
        Some(1)
    };
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

    let max = effective_max.unwrap_or(1);
    runtime_log_info(format!(
        "[推理并发] 开始等待供应商并发门: provider_id={}, model={}, max={}",
        provider_id, model_name, max
    ));
    let gate = {
        let mut gates = app_state.provider_request_gates.lock().await;
        match gates.get(provider_id) {
            Some(gate) if gate.limit == max => gate.clone(),
            _ => {
                let gate = std::sync::Arc::new(ProviderRequestGate {
                    limit: max,
                    semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(max)),
                });
                gates.insert(provider_id.to_string(), gate.clone());
                gate
            }
        }
    };
    let wait_started = std::time::Instant::now();
    let permit = gate
        .semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|err| format!("Semaphore acquire failed: {err}"))?;
    let waited_ms = wait_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    runtime_log_info(format!(
        "[推理并发] 已进入供应商并发门: provider_id={}, model={}, waited_ms={}",
        provider_id, model_name, waited_ms
    ));
    Ok(Some(ProviderConcurrencyGuard {
        provider_id: provider_id.to_string(),
        model_name: model_name.to_string(),
        acquired_at: std::time::Instant::now(),
        _permit: permit,
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
    tool_definitions: Vec<ProviderToolDefinition>,
) -> Result<ModelReply, String> {
    if resolved_api.request_format.is_genai_chat() || resolved_api.request_format.is_auto() {
        if tool_definitions.is_empty() {
            return call_model_genai_stream(resolved_api, model_name, prepared, app_state, None)
                .await;
        }
        return call_model_genai_stream_with_tools(
            resolved_api,
            model_name,
            prepared,
            tool_definitions,
            app_state,
            None,
        )
        .await;
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
    tool_definitions: Vec<ProviderToolDefinition>,
) -> Result<ModelReply, String> {
    if resolved_api.request_format.is_genai_chat() || resolved_api.request_format.is_auto() {
        if tool_definitions.is_empty() {
            return call_model_genai_non_stream(resolved_api, model_name, prepared, app_state, None)
                .await;
        }
        return call_model_genai_non_stream_with_definitions(
            resolved_api,
            model_name,
            prepared,
            tool_definitions,
            app_state,
            None,
        )
        .await;
    }
    invoke_model_by_format(resolved_api, model_name, prepared, app_state, tool_definitions).await
}

async fn invoke_model_by_format_with_timeout(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    timeout_secs: u64,
    scene: &str,
    app_state: Option<&AppState>,
    tool_definitions: Vec<ProviderToolDefinition>,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_by_format(resolved_api, model_name, prepared, app_state, tool_definitions),
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
    tool_definitions: Vec<ProviderToolDefinition>,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_non_stream_by_format(
            resolved_api,
            model_name,
            prepared,
            app_state,
            tool_definitions,
        ),
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
    tool_definitions: Vec<ProviderToolDefinition>,
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
        runtime_log_warn(format!(
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
                tool_definitions.clone(),
            )
            .await
        } else {
            invoke_model_non_stream_by_format(
                resolved_api,
                model_name,
                prepared.clone(),
                app_state,
                tool_definitions.clone(),
            )
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
                tool_definitions.clone(),
            )
            .await
        } else {
            invoke_model_by_format(
                resolved_api,
                model_name,
                prepared.clone(),
                app_state,
                tool_definitions.clone(),
            )
            .await
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
                runtime_log_warn(format!(
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
                        tool_definitions.clone(),
                    )
                    .await
                } else {
                    invoke_model_by_format(
                        resolved_api,
                        model_name,
                        fallback,
                        app_state,
                        tool_definitions.clone(),
                    )
                    .await
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
            runtime_log_error(format!(
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
                    tool_definitions,
                )
                .await
            } else {
                invoke_model_non_stream_by_format(
                    resolved_api,
                    model_name,
                    prepared,
                    app_state,
                    tool_definitions,
                )
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
    ModelCallExecutionResult {
        result,
        log_parts,
        compaction_preserved_messages: None,
    }
}

fn quick_json_prepared_prompt(prompt: &str) -> PreparedPrompt {
    PreparedPrompt {
        preamble: "只返回一个 JSON 对象，不要解释，不要 Markdown，不要代码块。".to_string(),
        history_messages: Vec::new(),
        latest_user_text: prompt.trim().to_string(),
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    }
}

fn parse_quick_model_json_response(
    raw: &str,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("快速模型返回为空，未得到 JSON".to_string());
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if required_fields.is_empty() {
            return Ok(value);
        }
        if value
            .as_object()
            .map(|object| {
                required_fields
                    .iter()
                    .all(|field| object.contains_key(*field))
            })
            .unwrap_or(false)
        {
            return Ok(value);
        }
    }
    extract_best_json_object_value(trimmed, "---JSON---", required_fields, optional_fields)
        .ok_or_else(|| {
            if required_fields.is_empty() {
                "快速模型未返回可解析的 JSON 对象".to_string()
            } else {
                format!(
                    "快速模型未返回包含必要字段的 JSON 对象：{}",
                    required_fields.join(", ")
                )
            }
        })
}

fn resolved_model_name_for_quick_request(
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
) -> String {
    if selected_api.model.trim().is_empty() {
        resolved_api.model.clone()
    } else {
        selected_api.model.trim().to_string()
    }
}

fn quick_request_adapter_kind(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
) -> genai::adapter::AdapterKind {
    let default_adapter = if resolved_api.request_format.is_openai_responses_family() {
        genai::adapter::AdapterKind::OpenAIResp
    } else {
        resolved_api
            .request_format
            .genai_adapter_kind()
            .or_else(|| {
                resolved_api
                    .request_format
                    .is_auto()
                    .then(|| resolve_model_adapter_for_auto(model_name))
            })
            .unwrap_or_else(|| provider_openai_chat_adapter_kind(resolved_api, model_name))
    };
    resolve_provider_genai_adapter_kind(resolved_api, model_name, default_adapter)
}

#[derive(Debug, Clone)]
struct QuickModelJsonCallOutput {
    value: Value,
    raw_text: String,
    model_name: String,
    duration_ms: u64,
}

#[derive(Debug, Clone)]
struct QuickModelJsonCallError {
    message: String,
    raw_text: Option<String>,
    model_name: Option<String>,
    duration_ms: Option<u64>,
}

impl QuickModelJsonCallError {
    fn from_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            raw_text: None,
            model_name: None,
            duration_ms: None,
        }
    }
}

async fn invoke_quick_model_reply_with_prepared_prompt(
    state: &AppState,
    api_config_id: &str,
    prepared: PreparedPrompt,
    timeout_secs: Option<u64>,
) -> Result<ModelReply, String> {
    let app_config = state_read_config_cached(state)?;
    let selected_api = resolve_selected_api_config(&app_config, Some(api_config_id))
        .ok_or_else(|| format!("快速模型配置不存在：{}", api_config_id))?;
    if !selected_api.enable_text || !selected_api.request_format.is_chat_text() {
        return Err("快速模型不支持文本对话".to_string());
    }
    let resolved_api = resolve_api_config(&app_config, Some(api_config_id))?;
    let model_name = resolved_model_name_for_quick_request(&selected_api, &resolved_api);
    let request_future = async {
        let resolved_api = resolve_request_api_config(&resolved_api).await?;
        let request_api_key = consume_api_key_for_request(&resolved_api);
        let adapter_kind = quick_request_adapter_kind(&resolved_api, &model_name);
        let service_target = build_provider_genai_service_target(
            &resolved_api,
            adapter_kind,
            &model_name,
            request_api_key.clone(),
        );
        let request = build_provider_genai_request(&prepared)?;
        let options = build_provider_genai_chat_options(&resolved_api, adapter_kind, true, false, None);
        let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
            &resolved_api,
            &model_name,
            request_api_key,
            service_target,
        )?;
        let mut stream = client
            .exec_chat_stream(model_spec, request, Some(&options))
            .await
            .map_err(|err| format!("快速模型流式请求失败：{err}"))?
            .stream;
        collect_streaming_model_reply_genai(&mut stream, None, None, None, None, None).await
    };
    if let Some(timeout_secs) = timeout_secs {
        let call_started = std::time::Instant::now();
        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), request_future)
            .await
            .map_err(|_| {
                format!(
                    "快速模型请求超时 (elapsed={}ms, timeout={}s)",
                    call_started.elapsed().as_millis(),
                    timeout_secs
                )
            })?
    } else {
        request_future.await
    }
}

async fn invoke_quick_model_json_result_with_prepared_prompt(
    state: &AppState,
    scene: &'static str,
    prepared: PreparedPrompt,
    timeout_secs: Option<u64>,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<QuickModelJsonCallOutput, QuickModelJsonCallError> {
    let quick_api_config_id = current_tool_review_api_config_id(state)
        .map_err(QuickModelJsonCallError::from_message)?
        .ok_or_else(|| QuickModelJsonCallError::from_message("未配置快速模型"))?;
    invoke_model_json_result_with_api_config_id(
        state,
        &quick_api_config_id,
        scene,
        prepared,
        timeout_secs,
        required_fields,
        optional_fields,
    )
    .await
}

async fn invoke_model_json_result_with_api_config_id(
    state: &AppState,
    api_config_id: &str,
    scene: &'static str,
    prepared: PreparedPrompt,
    timeout_secs: Option<u64>,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<QuickModelJsonCallOutput, QuickModelJsonCallError> {
    let app_config =
        state_read_config_cached(state).map_err(QuickModelJsonCallError::from_message)?;
    let selected_api = resolve_selected_api_config(&app_config, Some(api_config_id))
        .ok_or_else(|| {
            QuickModelJsonCallError::from_message(format!("模型配置不存在：{api_config_id}"))
        })?;
    let resolved_api = resolve_api_config(&app_config, Some(api_config_id))
        .map_err(QuickModelJsonCallError::from_message)?;
    let model_name = resolved_model_name_for_quick_request(&selected_api, &resolved_api);
    let _ = scene;
    let started_at = std::time::Instant::now();
    let reply = invoke_quick_model_reply_with_prepared_prompt(
        state,
        api_config_id,
        prepared,
        timeout_secs,
    )
    .await
    .map_err(|message| QuickModelJsonCallError {
        message,
        raw_text: None,
        model_name: Some(model_name.clone()),
        duration_ms: Some(elapsed_ms_u64(started_at)),
    })?;
    let duration_ms = elapsed_ms_u64(started_at);
    let raw_text = fast_request_response_text_from_reply(&reply);
    let value = parse_quick_model_json_response(&raw_text, required_fields, optional_fields)
        .map_err(|message| QuickModelJsonCallError {
            message,
            raw_text: Some(raw_text.clone()),
            model_name: Some(model_name.clone()),
            duration_ms: Some(duration_ms),
        })?;
    Ok(QuickModelJsonCallOutput {
        value,
        raw_text,
        model_name,
        duration_ms,
    })
}

#[allow(dead_code)]
async fn invoke_quick_model_json_with_prepared_prompt(
    state: &AppState,
    scene: &'static str,
    prepared: PreparedPrompt,
    timeout_secs: Option<u64>,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<Value, String> {
    invoke_quick_model_json_result_with_prepared_prompt(
        state,
        scene,
        prepared,
        timeout_secs,
        required_fields,
        optional_fields,
    )
    .await
    .map(|output| output.value)
    .map_err(|err| err.message)
}

async fn invoke_quick_model_json_result(
    state: &AppState,
    scene: &'static str,
    prompt: &str,
    timeout_secs: Option<u64>,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<QuickModelJsonCallOutput, QuickModelJsonCallError> {
    if prompt.trim().is_empty() {
        return Err(QuickModelJsonCallError::from_message(
            "快速模型 JSON 请求提示词不能为空",
        ));
    }
    invoke_quick_model_json_result_with_prepared_prompt(
        state,
        scene,
        quick_json_prepared_prompt(prompt),
        timeout_secs,
        required_fields,
        optional_fields,
    )
    .await
}

#[allow(dead_code)]
async fn invoke_quick_model_json(
    state: &AppState,
    scene: &'static str,
    prompt: &str,
    timeout_secs: Option<u64>,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<Value, String> {
    if prompt.trim().is_empty() {
        return Err("快速模型 JSON 请求提示词不能为空".to_string());
    }
    invoke_quick_model_json_with_prepared_prompt(
        state,
        scene,
        quick_json_prepared_prompt(prompt),
        timeout_secs,
        required_fields,
        optional_fields,
    )
    .await
}

/// 用专家模型（对话设置中的专家模型）做一次 JSON 输出请求
async fn invoke_expert_model_json_result(
    state: &AppState,
    scene: &'static str,
    prompt: &str,
    timeout_secs: Option<u64>,
    required_fields: &[&str],
    optional_fields: &[&str],
) -> Result<QuickModelJsonCallOutput, QuickModelJsonCallError> {
    if prompt.trim().is_empty() {
        return Err(QuickModelJsonCallError::from_message(
            "专家模型 JSON 请求提示词不能为空",
        ));
    }
    let app_config =
        state_read_config_cached(state).map_err(QuickModelJsonCallError::from_message)?;
    let expert_id = app_config.expert_api_config_id.trim().to_string();
    if expert_id.is_empty() {
        return Err(QuickModelJsonCallError::from_message(
            "未配置专家模型（请在对话设置中配置专家模型）",
        ));
    }
    invoke_model_json_result_with_api_config_id(
        state,
        &expert_id,
        scene,
        quick_json_prepared_prompt(prompt),
        timeout_secs,
        required_fields,
        optional_fields,
    )
    .await
}

async fn call_archive_summary_model_with_timeout(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    prepared: PreparedPrompt,
    timeout_secs: u64,
    tool_definitions: Vec<ProviderToolDefinition>,
) -> ModelCallExecutionResult {
    invoke_model_with_policy(
        resolved_api,
        &selected_api.model,
        prepared,
        CallPolicy::archive_json(timeout_secs),
        Some(state),
        tool_definitions,
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

    #[test]
    fn parse_quick_model_json_response_should_accept_direct_json_value() {
        let parsed = parse_quick_model_json_response(
            r#"{"allow":true,"review_opinion":"可以执行"}"#,
            &["allow"],
            &["review_opinion"],
        )
        .expect("parse quick json");

        assert_eq!(parsed["allow"], true);
        assert_eq!(parsed["review_opinion"], "可以执行");
    }

    #[test]
    fn parse_quick_model_json_response_should_extract_wrapped_json_object() {
        let parsed = parse_quick_model_json_response(
            "说明\n```json\n{\"has_topic\":true,\"title\":\"任务语义\"}\n```",
            &["has_topic"],
            &["title"],
        )
        .expect("extract quick json");

        assert_eq!(parsed["has_topic"], true);
        assert_eq!(parsed["title"], "任务语义");
    }

    #[test]
    fn parse_quick_model_json_response_should_require_fields() {
        let err = parse_quick_model_json_response(
            r#"{"title":"缺字段"}"#,
            &["has_topic"],
            &["title"],
        )
        .expect_err("missing required field should fail");

        assert!(err.contains("必要字段"));
    }
}
