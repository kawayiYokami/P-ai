fn conversation_has_visible_title(conversation: &Conversation) -> bool {
    !conversation.title.trim().is_empty()
        || conversation_has_auto_title_blocking_summary_title(conversation)
}

fn conversation_has_visible_title_from_store(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(conversation_meta) => conversation_meta,
        Err(err) if err.contains("CONV_NOT_FOUND") => return Ok(false),
        Err(err) => return Err(err),
    };
    if !conversation_meta.title.trim().is_empty() {
        return Ok(true);
    }
    if conversation_meta.latest_summary_title.is_none() {
        return Ok(false);
    }
    let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    ensure_chat_store_conversation_readable(state, conversation_id, &store_paths)?;
    Ok(message_store::chat_store_read_latest_compaction_message(&store_paths)?
        .as_ref()
        .is_some_and(summary_context_message_title_blocks_auto_title))
}

async fn conversation_has_visible_title_from_store_async(
    state: AppState,
    conversation_id: String,
) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        conversation_has_visible_title_from_store(&state, &conversation_id)
    })
    .await
    .map_err(|err| format!("会话标题存储检查任务失败：{err}"))?
}

fn auto_title_generation_inflight(
) -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static INFLIGHT: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashSet<String>>,
    > = std::sync::OnceLock::new();
    INFLIGHT.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn auto_title_generation_try_mark_inflight(conversation_id: &str) -> bool {
    let normalized_id = conversation_id.trim();
    if normalized_id.is_empty() {
        return false;
    }
    match auto_title_generation_inflight().lock() {
        Ok(mut guard) => guard.insert(normalized_id.to_string()),
        Err(_) => false,
    }
}

fn auto_title_generation_clear_inflight(conversation_id: &str) {
    if let Ok(mut guard) = auto_title_generation_inflight().lock() {
        guard.remove(conversation_id.trim());
    }
}

fn should_schedule_conversation_auto_title_generation(
    conversation: &Conversation,
    latest_user_text: &str,
) -> bool {
    let char_count = latest_user_text.trim().chars().count();
    conversation_is_local_normal_chat(conversation)
        && !conversation_has_visible_title(conversation)
        && (10..=100).contains(&char_count)
}

fn build_auto_conversation_title_prompt(user_message: &str) -> String {
    format!(
        "你是会话话题探测器。只根据本次用户发言判断是否能提取明确话题。\
只能输出 JSON，不要解释，不要 Markdown。\
能提取则返回 {{\"has_topic\":true,\"title\":\"简洁标题\"}}。\
不能提取、内容模糊、寒暄或承接上文但无法独立成题，则返回 {{\"has_topic\":false,\"title\":\"\"}}。\
title 尽量 10 个汉字以内，绝不超过 20 个字，不要引号外文本，不要收尾标点。\n\n用户发言：\n{}",
        user_message.trim()
    )
}

#[cfg(test)]
fn parse_auto_conversation_title_probe_result(raw: &str) -> Option<String> {
    let value = parse_quick_model_json_response(raw, &["has_topic"], &["title"]).ok()?;
    parse_auto_conversation_title_probe_value(&value)
}

fn parse_auto_conversation_title_probe_value(value: &Value) -> Option<String> {
    let has_topic = value
        .get("has_topic")
        .or_else(|| value.get("hasTopic"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !has_topic {
        return None;
    }
    value
        .get("title")
        .and_then(Value::as_str)
        .and_then(normalize_summary_context_title)
}

fn sync_codex_conversation_request_key(
    resolved_api: &mut ResolvedApiConfig,
    conversation_id: &str,
) {
    let stable_key = conversation_id.trim();
    if stable_key.is_empty() {
        return;
    }
    if resolved_api.request_format.is_openai_responses_family() {
        resolved_api.prompt_cache_key = Some(stable_key.to_string());
    }
    if resolved_api.request_format.is_codex() {
        upsert_api_extra_header(resolved_api, "Session-Id", stable_key);
    }
}

async fn run_auto_conversation_title_generation(
    state: &AppState,
    conversation_id: &str,
    user_message: &str,
) -> Result<String, String> {
    let prompt = build_auto_conversation_title_prompt(user_message);
    let output = match invoke_quick_model_json_result(
        state,
        "Conversation auto title",
        &prompt,
        Some(12),
        &["has_topic"],
        &["title"],
    )
    .await
    {
        Ok(output) => output,
        Err(err) => {
            record_fast_request_turn_best_effort(
                state,
                conversation_id,
                build_fast_request_turn(
                    "title_generation",
                    &prompt,
                    err.raw_text.as_deref().unwrap_or(""),
                    false,
                    Some(err.message.clone()),
                    err.model_name,
                    err.duration_ms,
                ),
            );
            return Err(err.message);
        }
    };
    let title = parse_auto_conversation_title_probe_value(&output.value);
    let error = title
        .is_none()
        .then(|| "快速模型未返回有效标题".to_string());
    record_fast_request_turn_best_effort(
        state,
        conversation_id,
        build_fast_request_turn(
            "title_generation",
            &prompt,
            &output.raw_text,
            title.is_some(),
            error.clone(),
            Some(output.model_name),
            Some(output.duration_ms),
        ),
    );
    title.ok_or_else(|| error.unwrap_or_else(|| "快速模型未返回有效标题".to_string()))
}

fn spawn_conversation_auto_title_generation(
    state: AppState,
    conversation_id: String,
    user_message: String,
) {
    tauri::async_runtime::spawn(async move {
        let started_at = std::time::Instant::now();
        let conversation_id = conversation_id.trim().to_string();
        if conversation_id.is_empty() {
            return;
        }
        if !auto_title_generation_try_mark_inflight(&conversation_id) {
            return;
        }
        let result = async {
            if conversation_has_visible_title_from_store_async(
                state.clone(),
                conversation_id.clone(),
            )
            .await?
            {
                return Ok::<(), String>(());
            }
            // 未配置快速模型＝禁用快速请求：静默跳过，不打 warn、不写失败记录。
            if current_tool_review_api_config_id(&state)?.is_none() {
                runtime_log_info(format!(
                    "[会话标题] 跳过，任务=自动生成标题，conversation_id={conversation_id}，reason=未配置快速模型"
                ));
                return Ok(());
            }
            match run_auto_conversation_title_generation(&state, &conversation_id, &user_message).await {
                Ok(title) => {
                    if conversation_has_visible_title_from_store_async(
                        state.clone(),
                        conversation_id.clone(),
                    )
                    .await?
                    {
                        return Ok(());
                    }
                    match conversation_service_v2()
                        .update_latest_summary_title(&state, &conversation_id, &title)
                        .await
                    {
                        Ok(changed) => {
                            if changed {
                                if let Err(err) =
                                    emit_unarchived_conversation_overview_item_updated_from_state(
                                        &state,
                                        &conversation_id,
                                    )
                                {
                                    runtime_log_warn(format!(
                                        "[会话标题] 警告，任务=刷新会话概览，conversation_id={}，error={}",
                                        conversation_id, err
                                    ));
                                }
                            }
                            runtime_log_info(format!(
                                "[会话标题] 完成，任务=自动生成标题，conversation_id={}，title={}，changed={}，elapsed_ms={}",
                                conversation_id,
                                title,
                                changed,
                                started_at.elapsed().as_millis()
                            ));
                            Ok(())
                        }
                        Err(err) => Err(err),
                    }
                }
                Err(err) => Err(err),
            }
        }
        .await;
        if let Err(err) = result {
            runtime_log_warn(format!(
                "[会话标题] 跳过，任务=自动生成标题，conversation_id={}，error={}，elapsed_ms={}",
                conversation_id,
                err,
                started_at.elapsed().as_millis()
            ));
        }
        auto_title_generation_clear_inflight(&conversation_id);
    });
}
