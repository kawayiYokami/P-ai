include!("core_send_inner/user_message_memory.rs");
fn trim_conversation_for_prompt_request(conversation: &Conversation) -> Conversation {
    let mut trimmed = conversation.clone();
    if trimmed.messages.is_empty() {
        return trimmed;
    }
    let mut start_idx = 0usize;
    for (idx, message) in conversation.messages.iter().enumerate() {
        let should_start_new = idx > 0
            && is_context_compaction_message(message, message.role.trim());
        if should_start_new {
            start_idx = idx;
        }
    }
    trimmed.messages = conversation.messages[start_idx..].to_vec();
    trimmed
}

fn conversation_current_segment_is_compaction_summary_only(conversation: &Conversation) -> bool {
    let Some(segment_start) = conversation.messages.iter().rposition(|message| {
        is_context_compaction_message(message, message.role.trim())
    }) else {
        return false;
    };

    matches!(
        &conversation.messages[segment_start..],
        [message] if is_context_compaction_message(message, message.role.trim())
    )
}

const REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS: i64 = 10;
const REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES: usize = 7;

fn plan_mode_prompt_block() -> &'static str {
    "<plan mode>\n先理解用户目标，调查当前上下文或代码。计划阶段是你和我之间的双向拷问，不是你独自思考后直接写计划。\n\n把计划拆成设计决策树：从根目标开始，沿范围、取舍、架构、数据、交互、风险、验收等分支逐一访谈我；只有父决策已达成共识，才能进入依赖它的子决策。每次只问一个当前最关键的问题，同时给出你的推荐答案、理由、证据和主要替代方案；不要静默替我选择目标、偏好、优先级、可接受风险或验收取舍。\n\n问题可由代码、配置、文档或工具回答时，必须先探索并带着结果继续访谈，不能把可自行查证的工作转嫁给我。我可以回答、补充、否定前提，也可以反过来拷问你的推荐、证据或替代方案；你必须直接回答我的反问，再回到下一个尚未收敛的决策。不要回避质疑，也不要为维护旧方案而辩护。\n\n除非我明确说‘不再追问’或‘直接出计划’，否则在我们确认设计树中会实质改变目标、边界、风险、成本或验收的分支均已收敛前，不得调用 plan.present。对我展示问题、回答、已确认结论和待决定分叉；不要展示内部逐字推理。当目标、约束、现状已清楚后，计划用于对齐需求、边界、风险、术语、测试和最终呈现。得到我明确确认后，再开始修改代码或实施。\n</plan mode>"
}

#[cfg(test)]
mod plan_mode_prompt_tests {
    use super::*;

    #[test]
    fn plan_mode_prompt_requires_a_user_interrogation_round() {
        let prompt = plan_mode_prompt_block();

        assert!(prompt.contains("双向拷问"));
        assert!(prompt.contains("设计决策树"));
        assert!(prompt.contains("推荐答案、理由、证据和主要替代方案"));
        assert!(prompt.contains("反过来拷问你的推荐、证据或替代方案"));
        assert!(prompt.contains("不得调用 plan.present"));
    }
}

fn conversation_latest_user_has_plan_mode_block(
    conversation: &Conversation,
    effective_agent_id: &str,
) -> bool {
    let plan_block = plan_mode_prompt_block().trim();
    conversation
        .messages
        .iter()
        .rev()
        .find(|message| prompt_role_for_message(message, effective_agent_id).as_deref() == Some("user"))
        .map(|message| {
            message
                .extra_text_blocks
                .iter()
                .any(|block| block.trim() == plan_block)
        })
        .unwrap_or(false)
}

include!("core_send_inner/remote_im_auto_send.rs");
include!("core_send_inner/image_fallback.rs");
include!("core_send_inner/auto_title.rs");
fn prepend_required_chat_api_id(
    api_id: Option<&str>,
    candidate_api_ids: &mut Vec<String>,
    app_config: &AppConfig,
) -> Result<(), String> {
    let Some(raw_api_id) = api_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let api_id = resolve_model_role_api_config_id(app_config, raw_api_id)
        .ok_or_else(|| format!("模型角色未配置：api_config_id={raw_api_id}"))?;

    let Some(api_config) = app_config
        .api_configs
        .iter()
        .find(|api| api.id == api_id)
    else {
        return Err(format!("指定模型不存在：api_config_id={api_id}"));
    };

    if !is_text_chat_api(api_config) {
        return Err(format!(
            "指定模型不是可用聊天文本模型：api_config_id={}, request_format={:?}, enable_text={}",
            api_id,
            api_config.request_format,
            api_config.enable_text
        ));
    }

    if let Some(index) = candidate_api_ids.iter().position(|id| id == &api_id) {
        if index > 0 {
            let existing_api_id = candidate_api_ids.remove(index);
            candidate_api_ids.insert(0, existing_api_id);
        }
        return Ok(());
    }

    candidate_api_ids.insert(0, api_id);
    Ok(())
}

fn prepend_optional_preferred_chat_api_id(
    preferred_api_id: Option<&str>,
    candidate_api_ids: &mut Vec<String>,
    app_config: &AppConfig,
) -> Result<bool, String> {
    let Some(raw_preferred_api_id) = preferred_api_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };
    let Some(preferred_api_id) = resolve_model_role_api_config_id(app_config, raw_preferred_api_id) else {
        runtime_log_warn(format!(
            "[会话模型] 跳过，任务=应用会话首选模型，原因=模型角色未配置，api_config_id={}",
            raw_preferred_api_id
        ));
        return Ok(false);
    };

    let Some(api_config) = app_config
        .api_configs
        .iter()
        .find(|api| api.id == preferred_api_id)
    else {
        runtime_log_warn(format!(
            "[会话模型] 跳过，任务=应用会话首选模型，原因=模型不存在，api_config_id={}",
            preferred_api_id
        ));
        return Ok(false);
    };
    if !is_text_chat_api(api_config) {
        runtime_log_warn(format!(
            "[会话模型] 跳过，任务=应用会话首选模型，原因=模型不可用于聊天文本，api_config_id={}，request_format={:?}，enable_text={}",
            preferred_api_id,
            api_config.request_format,
            api_config.enable_text
        ));
        return Ok(false);
    }

    prepend_required_chat_api_id(Some(&preferred_api_id), candidate_api_ids, app_config)?;
    Ok(true)
}

fn build_chat_candidate_api_ids(
    app_config: &AppConfig,
    agent: &AgentProfile,
    requested_api_config_id: Option<&str>,
    conversation_preferred_api_config_id: Option<&str>,
) -> Result<(Vec<String>, bool), String> {
    let mut candidate_api_ids = agent_effective_chat_api_config_ids(app_config, agent);
    let preferred_model_applied = if let Some(requested_api_config_id) =
        requested_api_config_id.map(str::trim).filter(|value| !value.is_empty())
    {
        prepend_required_chat_api_id(Some(requested_api_config_id), &mut candidate_api_ids, app_config)?;
        false
    } else {
        prepend_optional_preferred_chat_api_id(
            conversation_preferred_api_config_id,
            &mut candidate_api_ids,
            app_config,
        )?
    };
    if !agent_model_failure_fallback_enabled(agent) {
        candidate_api_ids.truncate(1);
    }
    Ok((candidate_api_ids, preferred_model_applied))
}

include!("core_send_inner/stream_failure_persistence.rs");
fn main_assistant_activation_should_reject_latest_message(
    latest_message: &ChatMessage,
    assistant_agent_id: &str,
) -> bool {
    latest_message
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        == Some(assistant_agent_id.trim())
}

fn restart_dispatch_round_after_context_compaction(
    state: &AppState,
    runtime_context: &mut RuntimeContext,
    conversation_id: &str,
    agent_id: &str,
    dispatch_reason: &str,
) -> Result<String, String> {
    let request_id = format!("chat-{}", Uuid::new_v4());
    let dispatch_id = Uuid::new_v4().to_string();
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(dispatch_id);
    runtime_context.event_source = runtime_context_trimmed(Some("compaction_restart"));
    runtime_context.dispatch_reason = runtime_context_trimmed(Some(dispatch_reason));
    runtime_context.trusted_prompt_usage = None;

    let stream_started_at = now_iso();
    let stream_started_at_ms = now_unix_ms();
    let assistant_message_id = Uuid::new_v4().to_string();
    let preserved = if runtime_context.compaction_preserved_messages_ready {
        runtime_context.compaction_preserved_messages_ready = false;
        runtime_context.compaction_preserved_messages.take()
    } else {
        None
    };
    conversation_service_v2().bootstrap_streaming_assistant_message(
        state,
        &AssistantMessageBootstrapInput {
            conversation_id: conversation_id.to_string(),
            assistant_message_id: assistant_message_id.clone(),
            speaker_agent_id: agent_id.to_string(),
            created_at: Some(stream_started_at.clone()),
            provider_meta_patch: None,
            compaction_preserved_messages: preserved,
        },
    )?;
    reset_conversation_stream_runtime_cache(
        state,
        conversation_id,
        request_id.as_str(),
        request_id.as_str(),
        agent_id,
        assistant_message_id.as_str(),
        stream_started_at.as_str(),
        stream_started_at_ms,
    )?;
    let activation_reason = resolve_activation_reason(runtime_context);
    emit_round_started_event(
        state,
        conversation_id,
        request_id.as_str(),
        request_id.as_str(),
        assistant_message_id.as_str(),
        activation_reason.as_str(),
        agent_id,
        stream_started_at.as_str(),
        stream_started_at_ms,
    );
    set_conversation_runtime_state_and_emit(
        state,
        conversation_id,
        MainSessionState::AssistantStreaming,
    )?;
    runtime_log_info(format!(
        "[聊天调度] 压缩后新一轮开始事件已发送 conversation_id={} request_id={} assistant_message_id={} agent_id={} reason={}",
        conversation_id, request_id, assistant_message_id, agent_id, activation_reason
    ));
    Ok(assistant_message_id)
}

fn latest_canonical_user_prompt_text(
    conversation: &Conversation,
    current_agent_id: &str,
) -> Option<String> {
    conversation
        .messages
        .iter()
        .rev()
        .find(|message| {
            prompt_role_for_message(message, current_agent_id).as_deref() == Some("user")
        })
        .map(render_prompt_user_text_only)
        .filter(|text| !text.trim().is_empty())
}

fn legacy_attachment_relative_paths_for_prompt(
    payload: &ChatInputPayload,
    used_canonical_latest_user_text: bool,
) -> Vec<String> {
    if used_canonical_latest_user_text {
        Vec::new()
    } else {
        collect_payload_attachment_relative_paths(payload)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryKind {
    Empty,
    RateLimited,
    NonRetryable,
}

#[derive(Debug, Clone, Copy, Default)]
struct RetryBudget {
    empty_used: u8,
    rate_used: u8,
}

const MAX_EMPTY_RETRIES: u8 = 3;
const WAIT_EMPTY_SECS: u64 = 5;
const MAX_RATE_RETRIES: u8 = 3;
const WAIT_RATE_SECS: u64 = 30;

fn is_rate_limited_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    let has_429 = lower
        .split(|c: char| !c.is_ascii_digit())
        .any(|token| token == "429");
    if has_429 {
        return true;
    }
    lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("quota exceeded")
        || lower.contains("rate_limit_exceeded")
}

fn classify_retry_kind_for_result(result: &Result<ModelReply, String>) -> RetryKind {
    match result {
        Ok(reply) => match model_reply_content_state(reply) {
            ModelReplyContentState::Visible => RetryKind::NonRetryable,
            ModelReplyContentState::ReasoningOnly | ModelReplyContentState::Empty => {
                RetryKind::Empty
            }
        },
        Err(err) => {
            if is_rate_limited_error(err) {
                RetryKind::RateLimited
            } else {
                RetryKind::NonRetryable
            }
        }
    }
}

fn retry_policy(kind: RetryKind, budget: &RetryBudget) -> Option<std::time::Duration> {
    match kind {
        RetryKind::Empty => {
            if budget.empty_used < MAX_EMPTY_RETRIES {
                Some(std::time::Duration::from_secs(WAIT_EMPTY_SECS))
            } else {
                None
            }
        }
        RetryKind::RateLimited => {
            if budget.rate_used < MAX_RATE_RETRIES {
                Some(std::time::Duration::from_secs(WAIT_RATE_SECS))
            } else {
                None
            }
        }
        RetryKind::NonRetryable => None,
    }
}

async fn send_chat_message_inner(
    input: SendChatRequest,
    state: &AppState,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {

    let mut runtime_context = input.runtime_context.clone().unwrap_or_default();
    let trace_id = runtime_context_request_id_or_new(
        Some(&runtime_context),
        input.trace_id.as_deref(),
        "chat",
    );
    if runtime_context.request_id.is_none() {
        runtime_context.request_id = Some(trace_id.clone());
    }
    // 本次调度上下文持有的唯一 assistant_message_id。
    // 若上游（如 activate_main_assistant）已创建，则沿用；否则在此生成并创建消息。
    let mut dispatch_assistant_message_id = input
        .assistant_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // ========== 提前初始化流式缓存 ==========
    // 在调度开始时就创建后端流式缓存，避免首回还没开始就切换会话导致丢失流式草稿。
    // 后续 delta 事件到达后会通过 update_conversation_stream_runtime_cache 持续更新缓存内容。
    if runtime_context.remote_im_reply_delegate_id.is_none() {
    if let Some(ref session) = input.session {
        if let Some(cid) = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            let early_agent_id = session.agent_id.trim();
            if !early_agent_id.is_empty() {
                let stream_started_at = now_iso();
                let stream_started_at_ms = now_unix_ms();
                let bootstrap_in_current_dispatch = input
                    .assistant_message_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none();
                if input.trigger_only && bootstrap_in_current_dispatch {
                    if let Some(latest_message) = conversation_service_v2()
                        .get_conversation_recent_messages(state, cid, 1)?
                        .pop()
                    {
                        if main_assistant_activation_should_reject_latest_message(
                            &latest_message,
                            early_agent_id,
                        ) {
                            return Err("当前最后一条消息来自助理自身，无需重复激活。".to_string());
                        }
                    }
                }
                if bootstrap_in_current_dispatch {
                    // 直入 send_chat_message_inner 的路径（如委托）在此创建本轮 assistant 消息。
                    conversation_service_v2().bootstrap_streaming_assistant_message(
                        state,
                        &AssistantMessageBootstrapInput {
                            conversation_id: cid.to_string(),
                            assistant_message_id: dispatch_assistant_message_id.clone(),
                            speaker_agent_id: early_agent_id.to_string(),
                            created_at: Some(stream_started_at.clone()),
                            provider_meta_patch: None,
                            compaction_preserved_messages: None,
                        },
                    )?;
                }
                let _ = reset_conversation_stream_runtime_cache(
                    state,
                    cid,
                    trace_id.as_str(),
                    trace_id.as_str(),
                    early_agent_id,
                    &dispatch_assistant_message_id,
                    &stream_started_at,
                    stream_started_at_ms,
                );
                // 同样更新后端缓存中的可见状态，否则 has_visible_progress 仍为 false，
                // 窗口最小化/最大化恢复时后端快照不会包含投影消息，导致所有气泡丢失。
                // on_delta.send 只发往前端通道，不经过 dispatch loop 的 active_channel 回调，
                // 因此手动调用 update_conversation_stream_runtime_cache。
                let tool_status_event = AssistantDeltaEvent {
                    delta: String::new(),
                    kind: Some("tool_status".to_string()),
                    request_id: Some(trace_id.clone()),
                    activation_id: Some(trace_id.clone()),
                    phase_id: None,
                    reason: None,
                    tool_name: None,
                    tool_call_id: None,
                    tool_status: Some("running".to_string()),
                    tool_args: None,
                    message: Some("正在准备调度...".to_string()),
                    stream_cache: None,
                };
                let _ = update_conversation_stream_runtime_cache(
                    state,
                    cid,
                    &tool_status_event,
                );
                // 发送初始调度状态，让前端也立即建立流式缓存
                // 单发通道到不了 Web（WebSocket 丢弃 Channel），必须再走一次广播；
                // 与分发循环内 tool_status 的处理保持同一语义。
                let _ = on_delta.send(tool_status_event.clone());
                emit_assistant_delta_app_event(
                    state,
                    cid,
                    &assistant_delta_broadcast_event(&tool_status_event),
                );
            }
        }
    }
    }
    if runtime_context.remote_im_reply_delegate_id.is_some() {
        if let Some(ref session) = input.session {
            if let Some(cid) = session
                .conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                let speaker_agent_id = runtime_context
                    .executor_agent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| session.agent_id.trim());
                if !speaker_agent_id.is_empty() {
                    conversation_service_v2().bootstrap_streaming_assistant_message(
                        state,
                        &AssistantMessageBootstrapInput {
                            conversation_id: cid.to_string(),
                            assistant_message_id: dispatch_assistant_message_id.clone(),
                            speaker_agent_id: speaker_agent_id.to_string(),
                            created_at: Some(now_iso()),
                            provider_meta_patch: None,
                            compaction_preserved_messages: None,
                        },
                    )?;
                }
            }
        }
    }
    let oldest_queue_created_at = input
        .oldest_queue_created_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let _queue_wait_ms = oldest_queue_created_at
        .as_deref()
        .and_then(parse_iso)
        .map(|created_at| (now_utc() - created_at).whole_milliseconds())
        .filter(|ms| *ms > 0)
        .map(|ms| ms.min(i128::from(u64::MAX)) as u64);
    let _session_for_log = input.session.clone();
    let remote_im_activation_sources = input.remote_im_activation_sources.clone();
    if runtime_context.bound_remote_im_activation_source.is_none() {
        runtime_context.bound_remote_im_activation_source =
            effective_bound_remote_im_activation_source(None, &remote_im_activation_sources);
    }

    let chat_started_at = std::time::Instant::now();
    let stage_timeline = std::sync::Arc::new(std::sync::Mutex::new(Vec::<LlmRoundLogStage>::new()));
    let stage_timeline_for_chat = stage_timeline.clone();
    let should_record_chat_stage = |stage: &str| -> bool {
        matches!(
            stage,
            "send_chat_message_inner.start"
                | "runtime_and_session_ready"
                | "run.begin"
                | "attachments_processed"
                | "prepare_context.begin"
                | "prepare_context.conversation_lock_wait_done"
                | "prepare_context.skill_snapshot_ready"
                | "prepare_context.workspace_agents_ready"
                | "prepare_context.todo_guide_ready"
                | "prepare_context.im_runtime_ready"
                | "prepare_context.task_board_ready"
                | "prepare_context.todo_board_ready"
                | "prepare_context.attachment_hints_ready"
                | "prepare_context.overrides_built"
                  | "prepare_context.terminal_block_ready"
                  | "prepare_context.prompt_build_begin"
                  | "prepare_context.prompt_fixed_system_ready"
                  | "prepare_context.prompt_conversation_payload_ready"
                  | "prepare_context.prompt_system_cache_hit"
                  | "prepare_context.prompt_system_cache_rebuilt"
                  | "prepare_context.prompt_system_finalize_ready"
                  | "prepare_context.prompt_built"
                | "prepare_context.prompt_tokens_estimated"
                | "prepare_context.done"
                | "pre_send_archive_checked"
                | "prompt_ready"
                | "model_reply_ready"
                | "assistant_final_append.start"
                | "assistant_final_append.finish"
                | "assistant_message_persist_scheduled"
                | "send_chat_message_inner.finish"
        ) || stage.starts_with("model_request.start[")
            || stage.starts_with("model_request.finish[")
    };
    let describe_chat_stage = |stage: &str| -> String {
        let title = if stage == "send_chat_message_inner.start" {
            "开始发送消息".to_string()
        } else if stage == "runtime_and_session_ready" {
            "运行时与会话准备完成".to_string()
        } else if stage == "run.begin" {
            "进入执行阶段".to_string()
        } else if stage == "attachments_processed" {
            "附件处理完成".to_string()
        } else if stage == "prepare_context.begin" {
            "开始准备请求上下文".to_string()
        } else if stage == "prepare_context.conversation_lock_wait_done" {
            "会话锁等待完成".to_string()
        } else if stage == "prepare_context.skill_snapshot_ready" {
            "技能快照准备完成".to_string()
        } else if stage == "prepare_context.workspace_agents_ready" {
            "AGENTS 注入准备完成".to_string()
        } else if stage == "prepare_context.todo_guide_ready" {
            "Todo 指南准备完成".to_string()
        } else if stage == "prepare_context.im_runtime_ready" {
            "IM 运行块准备完成".to_string()
        } else if stage == "prepare_context.task_board_ready" {
            "任务板准备完成".to_string()
        } else if stage == "prepare_context.todo_board_ready" {
            "会话 Todo 板准备完成".to_string()
        } else if stage == "prepare_context.attachment_hints_ready" {
            "附件提示块准备完成".to_string()
        } else if stage == "prepare_context.overrides_built" {
            "提示词附加块准备完成".to_string()
        } else if stage == "prepare_context.terminal_block_ready" {
            "终端环境块准备完成".to_string()
        } else if stage == "prepare_context.prompt_build_begin" {
            "开始生成提示词主结构".to_string()
        } else if stage == "prepare_context.prompt_fixed_system_ready" {
            "主结构前置整理完成".to_string()
        } else if stage == "prepare_context.prompt_conversation_payload_ready" {
            "对话侧提示词生成完成".to_string()
        } else if stage == "prepare_context.prompt_system_cache_hit" {
            "系统提示词缓存命中".to_string()
        } else if stage == "prepare_context.prompt_system_cache_rebuilt" {
            "系统提示词缓存重建完成".to_string()
        } else if stage == "prepare_context.prompt_system_finalize_ready" {
            "系统提示词收口完成".to_string()
        } else if stage == "prepare_context.prompt_built" {
            "提示词主结构生成完成".to_string()
        } else if stage == "prepare_context.prompt_tokens_estimated" {
            "提示词 token 估算完成".to_string()
        } else if stage == "prepare_context.done" {
            "请求上下文准备完成".to_string()
        } else if stage == "pre_send_archive_checked" {
            "发送前归档检查完成".to_string()
        } else if stage == "prompt_ready" {
            "提示词准备完成".to_string()
        } else if stage.starts_with("model_request.start[") {
            "模型请求开始".to_string()
        } else if stage.starts_with("model_request.finish[") {
            "模型请求完成".to_string()
        } else if stage == "model_reply_ready" {
            "模型回复已就绪".to_string()
        } else if stage == "assistant_final_append.start" {
            "final assistant 写入开始".to_string()
        } else if stage == "assistant_final_append.finish" {
            "final assistant 写入完成".to_string()
        } else if stage == "assistant_message_persist_scheduled" {
            "助理消息持久化已调度".to_string()
        } else if stage == "send_chat_message_inner.finish" {
            "发送消息结束".to_string()
        } else {
            "未命名阶段".to_string()
        };
        title
    };
    let conversation_id_for_work_status = input
        .session
        .as_ref()
        .and_then(|s| s.conversation_id.clone())
        .unwrap_or_default();
    let request_id_for_work_status = trace_id.clone();
    let emit_conversation_work_status = |status: &str| {
        let conversation_id = conversation_id_for_work_status.trim();
        if conversation_id.is_empty() {
            return;
        }
        if let Some(app_handle) = state.app_handle.lock().ok().and_then(|guard| guard.clone()) {
            let _ = app_handle.emit("conversation_work_status", serde_json::json!({
                "conversationId": conversation_id,
                "requestId": request_id_for_work_status,
                "status": status
            }));
        }
    };
    emit_conversation_work_status("working");
    let log_chat_stage = |stage: &str| {
        if !should_record_chat_stage(stage) {
            return;
        }
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut timeline) = stage_timeline_for_chat.lock() {
            let since_prev_ms = timeline
                .last()
                .map(|last| elapsed_ms.saturating_sub(last.elapsed_ms))
                .unwrap_or(elapsed_ms);
            timeline.push(LlmRoundLogStage {
                stage: stage.to_string(),
                elapsed_ms,
                since_prev_ms,
                detail: None,
            });
        }
    };
    let flush_chat_timeline = |reason: &str| {
        let Ok(timeline) = stage_timeline.lock() else {
            return;
        };
        if timeline.is_empty() {
            return;
        }
        let summary = timeline
            .iter()
            .map(|item| {
                format!(
                    "{}:{}ms（较上阶段 +{}ms）",
                    describe_chat_stage(&item.stage),
                    item.elapsed_ms,
                    item.since_prev_ms
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        runtime_log_info(format!(
            "[聊天耗时] 汇总 原因={}，阶段={}",
            reason,
            summary
        ));
    };
    log_chat_stage("send_chat_message_inner.start");

    let trigger_only = input.trigger_only;
    let requested_agent_id = runtime_context
        .executor_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            input
                .session
                .as_ref()
                .map(|s| s.agent_id.trim())
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned)
        });
    let requested_api_config_id = input
        .session
        .as_ref()
        .and_then(|s| s.api_config_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let requested_conversation_id = input
        .session
        .as_ref()
        .and_then(|s| s.conversation_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    // 调度事件：仅委托接入，普通会话由 is_delegate 过滤直接 no-op
    {
        let _ = schedule_event_run_start_with_instant(
            state,
            &runtime_context,
            requested_conversation_id.as_deref().unwrap_or(""),
            &trace_id,
            &now_log_local_rfc3339(),
            Some(chat_started_at),
            0,
            serde_json::json!({
                "traceId": trace_id,
                "triggerOnly": trigger_only,
            }),
        );
    }

    // ========== 工作树预检：首轮发送时自动创建 .pai/.worktree/{sessionId} ==========
    if let Some(cid) = requested_conversation_id.as_deref() {
        if let Ok(Some(conv)) = conversation_service_v2().try_get_conversation_snapshot(state, cid) {
            if normalize_shell_work_mode_text(&conv.shell_work_mode) == SHELL_WORK_MODE_WORKTREE {
                let effective_conv = if conv.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT {
                    if let Some(parent_id) = conv.parent_conversation_id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                        if let Ok(parent_conv) = conversation_service_v2().get_conversation_metadata_record(state, parent_id) {
                            let mut merged = conv.clone();
                            merged.shell_workspaces = parent_conv.shell_workspaces.clone();
                            merged.shell_work_mode = parent_conv.shell_work_mode.clone();
                            merged.shell_work_branch =
                                normalize_shell_work_branch_text(&parent_conv.shell_work_branch);
                            merged
                        } else {
                            conv.clone()
                        }
                    } else {
                        conv.clone()
                    }
                } else {
                    conv.clone()
                };
                let cid_owned = cid.to_string();
                let state_clone = state.clone();
                let ensure_result = ensure_conversation_worktree(&state_clone, &effective_conv).await;
                if let Err(err) = ensure_result {
                    runtime_log_warn(format!(
                        "[工作树] 失败，任务=创建工作树，conversation_id={}，error={}，已中止本次请求",
                        cid_owned, err
                    ));
                    return Err(format!(
                        "工作树创建失败：{}，已中止本次请求，请检查 Git 状态或切换为目录模式后重试",
                        err
                    ));
                }
            }
        }
    }

    #[derive(Clone)]
    struct ConversationPrepareSnapshot {
        agents: Vec<AgentProfile>,
        response_style_id: String,
        user_name: String,
        user_intro: String,
        storage_conversation_before: Conversation,
        prompt_conversation_before: Conversation,
        is_remote_im_contact_conversation: bool,
        remote_im_contact_processing_mode: String,
        is_runtime_conversation: bool,
        runtime_conversation_id: Option<String>,
    }

    let runtime_conversation_id = requested_conversation_id
        .as_deref()
        .filter(|conversation_id| {
            delegate_runtime_thread_conversation_get(&state, conversation_id)
                .ok()
                .flatten()
                .is_some()
        })
        .map(ToOwned::to_owned);
    let runtime_conversation = if let Some(conversation_id) = runtime_conversation_id.as_deref() {
        delegate_runtime_thread_conversation_get(&state, conversation_id)?
            .ok_or_else(|| format!("指定临时会话不存在：{conversation_id}"))?
    } else {
        Conversation {
            id: String::new(),
            title: String::new(),
            agent_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: String::new(),
            updated_at: String::new(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            is_draft: false,
            auto_push_remote_contact_id: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            active_goal: None,
            last_error: None,
        }
    };
    let requested_conversation_id_for_prepare = requested_conversation_id.clone();
    let requested_conversation_id_for_build = requested_conversation_id_for_prepare.clone();
    let runtime_conversation_id_for_prepare = runtime_conversation_id.clone();
    let runtime_conversation_for_prepare = runtime_conversation.clone();
    let build_prepare_snapshot_read_only = |
        data: &AppData,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<Option<ConversationPrepareSnapshot>, String> {
        let Some(resolved) =
            conversation_service_v2().resolve_prompt_prepare_conversation_read_only(
                state,
                data,
                &state.data_path,
                runtime_conversation_id_for_prepare.as_deref(),
                &runtime_conversation_for_prepare,
                selected_api,
                effective_agent_id,
                requested_conversation_id_for_build.as_deref(),
            )?
        else {
            return Ok(None);
        };
        Ok(Some(ConversationPrepareSnapshot {
            agents: runtime_agents.to_vec(),
            response_style_id: resolved.response_style_id,
            user_name: resolved.user_name,
            user_intro: resolved.user_intro,
            storage_conversation_before: resolved.conversation_before.clone(),
            prompt_conversation_before: trim_conversation_for_prompt_request(
                &resolved.conversation_before,
            ),
            is_remote_im_contact_conversation: resolved.is_remote_im_contact_conversation,
            remote_im_contact_processing_mode: resolved.remote_im_contact_processing_mode,
            is_runtime_conversation: resolved.is_runtime_conversation,
            runtime_conversation_id: runtime_conversation_id_for_prepare.clone(),
        }))
    };
    let build_prepare_snapshot_for_requested_conversation_read_only = |
        requested_conversation_id: &str,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<Option<ConversationPrepareSnapshot>, String> {
        if runtime_conversation_id_for_prepare.as_deref() == Some(requested_conversation_id) {
            let mut data = AppData::default();
            data.agents = runtime_agents.to_vec();
            return build_prepare_snapshot_read_only(
                &data,
                runtime_agents,
                selected_api,
                effective_agent_id,
            );
        }
        let mut requested_conversation = conversation_service_v2()
            .get_conversation_prompt_context(state, requested_conversation_id)?;
        if conversation_is_archived(&requested_conversation) {
            return Ok(None);
        }
        if requested_conversation.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT {
            if let Some(parent_conversation_id) = requested_conversation
                .parent_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let parent = conversation_service_v2()
                    .get_conversation_metadata_record(state, parent_conversation_id)?;
                // side_chat 的模型固化在自身元数据；目录与权限属于父会话能力，发送时实时读取。
                requested_conversation.shell_workspace_path = parent.shell_workspace_path;
                requested_conversation.shell_workspaces = parent.shell_workspaces;
                requested_conversation.shell_autonomous_mode = parent.shell_autonomous_mode;
                requested_conversation.shell_work_mode = parent.shell_work_mode.clone();
                requested_conversation.shell_work_branch =
                    normalize_shell_work_branch_text(&parent.shell_work_branch);
            }
        }
        let mut data = AppData::default();
        data.agents = runtime_agents.to_vec();
        data.conversations.push(requested_conversation);
        build_prepare_snapshot_read_only(&data, runtime_agents, selected_api, effective_agent_id)
    };
    let build_prepare_snapshot_for_main_conversation_read_only = |
        main_conversation_id: &str,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<Option<ConversationPrepareSnapshot>, String> {
        let main_conversation_meta =
            conversation_service_v2().get_conversation_meta(state, main_conversation_id)?;
        if main_conversation_meta.status.trim() == "archived"
            || main_conversation_meta
                .archived_at
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            || main_conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE
            || main_conversation_meta.conversation_kind.trim()
                == CONVERSATION_KIND_REMOTE_IM_CONTACT
        {
            return Ok(None);
        }
        build_prepare_snapshot_for_requested_conversation_read_only(
            main_conversation_id,
            runtime_agents,
            selected_api,
            effective_agent_id,
        )
    };

    let (
        app_config,
        selected_api,
        resolved_api,
        effective_agent_id,
        candidate_api_ids,
        runtime_main_conversation_id,
        runtime_agents,
        preloaded_prepare_snapshot,
    ) = {
        let prepare_started = std::time::Instant::now();
        let mut prepare_detail_parts = Vec::<String>::new();
        let lock_wait_started = std::time::Instant::now();
        let lock_wait_ms = lock_wait_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("会话锁等待={}ms", lock_wait_ms));
        log_chat_stage("runtime_and_session_ready.lock_wait_done");
        let config_started = std::time::Instant::now();
        let (mut app_config, config_read_detail) = state_read_config_cached_with_detail(state)?;
        let config_read_ms = config_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "配置读取={}ms(source={}, dirty_fast_path={}, mtime_before={}ms, cache_lookup={}ms, disk_read={}ms, mtime_after={}ms, cache_write={}ms, total={}ms)",
            config_read_ms,
            config_read_detail.source,
            config_read_detail.dirty_fast_path,
            config_read_detail.mtime_before_ms,
            config_read_detail.cache_lookup_ms,
            config_read_detail.disk_read_ms,
            config_read_detail.mtime_after_ms,
            config_read_detail.cache_write_ms,
            config_read_detail.total_ms,
        ));
        log_chat_stage("runtime_and_session_ready.config_read_done");
        let app_data_started = std::time::Instant::now();
        let (assistant_agent_id, runtime_main_conversation_id) = {
            let state = state.clone();
            tokio::task::spawn_blocking(move || {
                let agent_id = state_service_get_assistant_agent_id(&state)?;
                let main_conversation_id = state_service_get_main_conversation_id(&state)?;
                Ok::<(String, Option<String>), String>((agent_id, main_conversation_id))
            })
            .await
            .map_err(|err| format!("读取运行时人格与会话 ID 失败：error={err}"))??
        };
        let mut runtime_agents = state_read_agents_cached(state)?;
        let app_data_read_ms = app_data_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "运行时分片读取={}ms(agents={}, assistant_agent_id={})",
            app_data_read_ms,
            runtime_agents.len(),
            assistant_agent_id
        ));
        log_chat_stage("runtime_and_session_ready.app_data_read_done");
        prepare_detail_parts.push(format!("运行时人格列表就绪=0ms(count={})", runtime_agents.len()));
        log_chat_stage("runtime_and_session_ready.runtime_data_cloned");
        let runtime_org_started = std::time::Instant::now();
        let runtime_org = build_runtime_organization_snapshot_from_parts(
            &state.data_path,
            &app_config,
            &runtime_agents,
        )?;
        let runtime_org_ms = runtime_org_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        app_config = runtime_org.config.clone();
        runtime_agents = runtime_org.agents.clone();
        prepare_detail_parts.push(format!("组织运行态构建={}ms", runtime_org_ms));
        log_chat_stage("runtime_and_session_ready.runtime_org_ready");
        let agent_resolve_started = std::time::Instant::now();
        let requested_agent_id_snapshot = requested_agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "缺少执行人格：调度上下文没有固化 agent_id。".to_string())?;
        let effective_agent_id = requested_agent_id_snapshot.to_string();
        if !runtime_agents
            .iter()
            .any(|a| a.id == effective_agent_id && !a.is_built_in_user)
        {
            let effective_agent_name = runtime_agents
                .iter()
                .find(|agent| agent.id == effective_agent_id)
                .map(|agent| agent.name.trim())
                .filter(|name| !name.is_empty())
                .unwrap_or(effective_agent_id.as_str());
            return Err(format!(
                "调度固化人格不存在或不可用：{}（{}）。",
                effective_agent_name, effective_agent_id
            ));
        }
        let agent_resolve_ms = agent_resolve_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("人格解析={}ms", agent_resolve_ms));
        log_chat_stage("runtime_and_session_ready.agent_resolved");
        let candidate_models_started = std::time::Instant::now();
        let effective_agent = runtime_agents
            .iter()
            .find(|agent| agent.id == effective_agent_id)
            .ok_or_else(|| format!("执行人格已经消失：agent_id={effective_agent_id}"))?
            .clone();
        let agent_model_fallback_enabled = agent_model_failure_fallback_enabled(&effective_agent);
        let conversation_meta_for_model_selection = requested_conversation_id_for_prepare
            .as_deref()
            .or(runtime_main_conversation_id.as_deref())
            .and_then(|conversation_id| {
                conversation_service_v2().get_conversation_meta(&state, conversation_id).ok()
            });
        let conversation_preferred_api_config_id = conversation_meta_for_model_selection
            .as_ref()
            .filter(|conversation| {
                conversation.conversation_kind.trim() != CONVERSATION_KIND_REMOTE_IM_CONTACT
            })
            .and_then(|conversation| {
                conversation
                    .preferred_api_config_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            });
        let requested_api_config_id_snapshot = requested_api_config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (candidate_api_ids, preferred_model_applied) = build_chat_candidate_api_ids(
            &app_config,
            &effective_agent,
            requested_api_config_id_snapshot,
            conversation_preferred_api_config_id.as_deref(),
        )?;
        let candidate_models_ms = candidate_models_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("候选模型构建={}ms(count={})", candidate_models_ms, candidate_api_ids.len()));
        runtime_log_info(format!(
            "[会话模型] 调度，任务=构建候选模型，会话ID={}，单次指定模型={}，会话首选模型={}，会话首选已应用={}，人格失败自动切换={}，候选队列={}",
            requested_conversation_id_for_prepare
                .as_deref()
                .or(runtime_main_conversation_id.as_deref())
                .unwrap_or("未知"),
            requested_api_config_id_snapshot.unwrap_or("未指定"),
            conversation_preferred_api_config_id.as_deref().unwrap_or("人格模型"),
            preferred_model_applied,
            agent_model_fallback_enabled,
            candidate_api_ids.join(" -> ")
        ));
        let selected_api_started = std::time::Instant::now();
        let selected_api_id = candidate_api_ids
            .first()
            .cloned()
            .ok_or_else(|| format!("Agent '{}' has no available chat model.", effective_agent_id))?;
        let selected_api = app_config
            .api_configs
            .iter()
            .find(|a| a.id == selected_api_id)
            .cloned()
            .ok_or_else(|| format!("Selected API config '{}' not found.", selected_api_id))?;
        let selected_api_ms = selected_api_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("选定模型查找={}ms(api={})", selected_api_ms, selected_api.id));
        let resolved_api_started = std::time::Instant::now();
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let resolved_api_ms = resolved_api_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "模型配置解析={}ms(format={}, model={})",
            resolved_api_ms,
            resolved_api.request_format.as_str(),
            selected_api.model
        ));
        let prepare_total_ms = prepare_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        runtime_log_info(format!(
            "[会话准备耗时] total={}ms，{}",
            prepare_total_ms,
            prepare_detail_parts.join(" | ")
        ));
        log_chat_stage("runtime_and_session_ready.candidate_models_ready");
        let preloaded_prepare_snapshot_candidate = match requested_conversation_id_for_prepare
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(requested_conversation_id) => {
                build_prepare_snapshot_for_requested_conversation_read_only(
                    requested_conversation_id,
                    &runtime_agents,
                    &selected_api,
                    &effective_agent_id,
                )?
            }
            None => {
                if let Some(main_conversation_id) = runtime_main_conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    build_prepare_snapshot_for_main_conversation_read_only(
                        main_conversation_id,
                        &runtime_agents,
                        &selected_api,
                        &effective_agent_id,
                    )?
                } else {
                    None
                }
            }
        };
        let preloaded_prepare_snapshot = preloaded_prepare_snapshot_candidate;
        (
            app_config,
            selected_api,
            resolved_api,
            effective_agent_id,
            candidate_api_ids,
            runtime_main_conversation_id,
            runtime_agents,
            preloaded_prepare_snapshot,
        )
    };
    runtime_context.executor_agent_id = Some(effective_agent_id.clone());
    log_chat_stage("runtime_and_session_ready");

    // 调度状态更新：运行时与会话解析完成，即将进入上下文构建阶段
    // 单发通道到不了 Web（WebSocket 丢弃 Channel），必须再走一次广播。
    if let Some(cid) = requested_conversation_id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        let early_event = AssistantDeltaEvent {
            delta: String::new(),
            kind: Some("tool_status".to_string()),
            request_id: Some(trace_id.clone()),
            activation_id: Some(trace_id.clone()),
            phase_id: None,
            reason: None,
            tool_name: None,
            tool_call_id: None,
            tool_status: Some("running".to_string()),
            tool_args: None,
            message: Some("正在处理附件与上下文...".to_string()),
            stream_cache: None,
        };
        let _ = on_delta.send(early_event.clone());
        emit_assistant_delta_app_event(state, cid, &assistant_delta_broadcast_event(&early_event));
    }

    let default_chat_key = inflight_chat_key(
        &effective_agent_id,
        requested_conversation_id.as_deref(),
    );
    let chat_key = runtime_context
        .remote_im_reply_delegate_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|delegate_id| format!("remote-im-reply-delegate::{delegate_id}"))
        .unwrap_or_else(|| default_chat_key.clone());
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(previous) = inflight.insert(chat_key.clone(), abort_handle) {
            previous.abort();
        }
    }
    reset_inflight_completed_tool_history(state, &chat_key)?;
    let _ = abort_inflight_tool_abort_handle(state, &chat_key);

    let chat_session_key = runtime_context
        .remote_im_reply_delegate_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|_| {
            requested_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|conversation_id| {
                    format!(
                        "{}::{}::remote_reply_delegate:{}",
                        effective_agent_id,
                        conversation_id,
                        runtime_context
                            .remote_im_reply_delegate_id
                            .as_deref()
                            .unwrap_or_default()
                    )
                })
        })
        .unwrap_or_else(|| chat_key.clone());
    let chat_session_key_for_log = chat_session_key.clone();
    let selected_api_for_log = selected_api.clone();
    let resolved_api_for_log = resolved_api.clone();
    let requested_conversation_id_for_failure_persist = requested_conversation_id.clone();
    let effective_agent_id_for_failure_persist = effective_agent_id.clone();
    let failure_persist_target =
        std::sync::Arc::new(std::sync::Mutex::new(None::<(String, String)>));
    let failure_persist_target_for_run = failure_persist_target.clone();
    let state_for_run = state.clone();
    let stage_timeline_for_run = stage_timeline.clone();
    let trace_id_for_run = trace_id.clone();
    // 调度事件收口所需的克隆（run 闭包会移动原变量）
    let runtime_context_for_schedule_events = runtime_context.clone();
    let requested_conversation_id_for_schedule_events = requested_conversation_id.clone();
    let trace_id_for_schedule_events = trace_id.clone();
    let run = async move {
    let state = state_for_run;
    let log_run_stage = |stage: &str| {
        if !should_record_chat_stage(stage) {
            return;
        }
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut timeline) = stage_timeline_for_run.lock() {
            let since_prev_ms = timeline
                .last()
                .map(|last| elapsed_ms.saturating_sub(last.elapsed_ms))
                .unwrap_or(elapsed_ms);
            timeline.push(LlmRoundLogStage {
                stage: stage.to_string(),
                elapsed_ms,
                since_prev_ms,
                detail: None,
            });
        }
    };
    log_run_stage("run.begin");
    // 调度状态更新：进入模型请求阶段
    // 单发通道到不了 Web（WebSocket 丢弃 Channel），必须再走一次广播。
    if let Some(cid) = requested_conversation_id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        let early_event = AssistantDeltaEvent {
            delta: String::new(),
            kind: Some("tool_status".to_string()),
            request_id: Some(trace_id_for_run.clone()),
            activation_id: Some(trace_id_for_run.clone()),
            phase_id: None,
            reason: None,
            tool_name: None,
            tool_call_id: None,
            tool_status: Some("running".to_string()),
            tool_args: None,
            message: Some("正在进入模型请求阶段...".to_string()),
            stream_cache: None,
        };
        let _ = on_delta.send(early_event.clone());
        emit_assistant_delta_app_event(&state, cid, &assistant_delta_broadcast_event(&early_event));
    }
    if !resolved_api.request_format.is_chat_text() {
        return Err(format!(
            "Request format '{}' is not implemented in chat router yet.",
            resolved_api.request_format
        ));
    }

    let effective_payload = input.payload.clone();

    // 图片能力转换统一在 PreparedPrompt 构建后执行，确保 Chat、Delegate 与远程冻结
    // 消息都使用同一 canonical Attachment、同一标签和同一绝对路径提示。
    log_run_stage("attachments_processed");

    let mut archived_before_send_any = false;
    let mut persist_user_message_on_next_prepare = true;

    let mut preloaded_prepare_snapshot = preloaded_prepare_snapshot;
    'dispatch: loop {
    let mut pending_user_message_append: Option<UserMessageAppendInput> = None;

    let mut prepare_request_context = |persist_user_message: bool| -> Result<_, String> {
        log_run_stage("prepare_context.begin");
        let mut normalized_storage_media_for_prompt: Option<(
            Vec<PreparedBinaryPayload>,
            Vec<PreparedBinaryPayload>,
        )> = None;
        let mut snapshot = if let Some(snapshot) = preloaded_prepare_snapshot.take() {
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else if let Some(requested_conversation_id) = requested_conversation_id_for_prepare
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let snapshot = build_prepare_snapshot_for_requested_conversation_read_only(
                requested_conversation_id,
                &runtime_agents,
                &selected_api,
                &effective_agent_id,
            )?
            .ok_or_else(|| format!("指定会话不存在或不可用：{requested_conversation_id}"))?;
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else if let Some(main_conversation_id) = runtime_main_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let snapshot = build_prepare_snapshot_for_main_conversation_read_only(
                main_conversation_id,
                &runtime_agents,
                &selected_api,
                &effective_agent_id,
            )?
            .ok_or_else(|| format!("系统通知会话不存在或不可用：{main_conversation_id}"))?;
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else {
            runtime_log_warn(format!(
                "[聊天发送] 缺少 conversation_id 且未找到系统通知会话，拒绝构建请求上下文"
            ));
            return Err("缺少 conversation_id".to_string());
        };
        let remote_im_reply_prompt_snapshot_messages = runtime_context
            .remote_im_reply_prompt_snapshot_messages
            .as_ref()
            .filter(|messages| !messages.is_empty())
            .cloned();
        if let Some(messages) = remote_im_reply_prompt_snapshot_messages.as_ref() {
            // 远程应答委托使用启动时冻结的 block（加上已消费引导），不能因并发
            // 委托或后续入站消息重新读取会话当前 block。
            snapshot.prompt_conversation_before.messages = messages.clone();
        }
        log_run_stage("prepare_context.conversation_snapshot_ready");
        if !trigger_only && conversation_is_system_notification(&snapshot.prompt_conversation_before) {
            runtime_log_error(format!(
                "[聊天发送] 拒绝，原因=系统通知会话不支持发言，conversation_id={}",
                snapshot.prompt_conversation_before.id
            ));
            return Err("系统通知会话不支持发言。".to_string());
        }
        log_run_stage("prepare_context.archive_summary_ready");
        log_run_stage("prepare_context.prompt_conversation_ready");
        log_run_stage("prepare_context.base_context_ready");
        let current_agent = snapshot
            .agents
            .iter()
            .find(|agent| agent.id == effective_agent_id.as_str() && !agent.is_built_in_user)
            .cloned()
            .ok_or_else(|| format!("执行人格解析出的人格不可用：agent_id={effective_agent_id}"))?;
        let is_delegate_conversation =
            snapshot.prompt_conversation_before.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE;
        let requested_plan_mode_enabled = get_conversation_plan_mode_enabled(
            &state,
            &snapshot.prompt_conversation_before.id,
        )
        .unwrap_or(snapshot.prompt_conversation_before.plan_mode_enabled);
        let storage_conversation = if trigger_only {
            if let Some(messages) = remote_im_reply_prompt_snapshot_messages {
                let mut conversation = snapshot.storage_conversation_before.clone();
                conversation.messages = messages;
                conversation
            } else {
            snapshot.storage_conversation_before.clone()
            }
        } else if !persist_user_message {
            snapshot.storage_conversation_before.clone()
        } else {
            let mut storage_api = selected_api.clone();
            storage_api.enable_image = true;
            storage_api.enable_audio = true;
            let mut storage_payload = input.payload.clone();
            if let Some(display_text) = input.payload.display_text.as_deref() {
                storage_payload.text = Some(display_text.trim().to_string());
            }
            let user_parts = build_user_parts(&state, &storage_payload, &storage_api)?;
            let storage_image_saved_paths = storage_payload
                .images
                .as_ref()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.saved_path.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let storage_audio_saved_paths = storage_payload
                .audios
                .as_ref()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.saved_path.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            // 附件二进制准备含同步文件读取与图片编码，用 block_in_place 执行，
            // 将当前 worker 标记为 blocking，其他任务可调度到别的线程，避免卡死 executor。
            normalized_storage_media_for_prompt = Some(
                tokio::task::block_in_place(|| {
                    build_prepared_binary_payloads_from_message_parts(
                        &user_parts,
                        &storage_image_saved_paths,
                        &storage_audio_saved_paths,
                    )
                }),
            );
            let mut user_provider_meta =
                provider_meta_without_legacy_attachments(input.payload.provider_meta.clone());
            if let Some(request_id) = runtime_context
                .request_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let mut meta = user_provider_meta.unwrap_or_else(|| serde_json::json!({}));
                if !meta.is_object() {
                    meta = serde_json::json!({});
                }
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert("requestId".to_string(), Value::String(request_id.to_string()));
                }
                user_provider_meta = Some(meta);
            }
            let recall_payload = if is_delegate_conversation {
                UserMessageRecallPayload::default()
            } else {
                let draft_message = ChatMessage {
                    id: String::new(),
                    role: "user".to_string(),
                    created_at: String::new(),
                    speaker_agent_id: None,
                    parts: user_parts.clone(),
                    extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                    meme_annotations: None,
                };
                with_memory_lock(&state, "prepare_context_user_message_recall", || {
                    collect_recall_payload_for_user_message(
                        &state.data_path,
                        &snapshot.agents,
                        &current_agent.id,
                        &draft_message,
                    )
                })?
            };
            if !recall_payload.stored_ids.is_empty() {
                write_retrieved_memory_ids_into_provider_meta(
                    &mut user_provider_meta,
                    &recall_payload.stored_ids,
                );
            }
            log_run_stage("prepare_context.memory_recall_done");
            let now = now_iso();
            // 会话草稿转正：草稿发出第一句话即清除标记转为普通会话。
            // is_draft=false 的存储写回与备用草稿创建统一收敛在 append_user_message
            // （任何用户消息写入都立刻转正），此处只更新内存快照供后续流程使用。
            if !snapshot.is_runtime_conversation && snapshot.storage_conversation_before.is_draft {
                snapshot.storage_conversation_before.is_draft = false;
            }
            let user_message_id = Uuid::new_v4().to_string();
            let git_ghost_snapshot_record = if snapshot.is_runtime_conversation {
                None
            } else {
                tauri::async_runtime::block_on(
                    git_ghost_snapshot::create_main_workspace_git_ghost_snapshot_record(
                        &state,
                        &snapshot.storage_conversation_before,
                        &user_message_id,
                    ),
                )
            };
            if let Some(record) = git_ghost_snapshot_record {
                if let Err(err) = git_ghost_snapshot::write_git_snapshot_record_into_provider_meta(
                    &mut user_provider_meta,
                    &record,
                )
                {
                    runtime_log_error(format!(
                        "[Git幽灵快照] 失败，conversation_id={}，message_id={}，stage=write_provider_meta，error={}",
                        snapshot.storage_conversation_before.id, user_message_id, err
                    ));
                }
            }
            let user_message = ChatMessage {
                id: user_message_id,
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(input.speaker_agent_id.clone().unwrap_or_else(|| USER_PERSONA_ID.to_string())),
                parts: user_parts,
                extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
                provider_meta: user_provider_meta,
                tool_call: None,
                mcp_call: None,
                meme_annotations: None,
            };
            let updated_conversation = append_user_message_to_conversation(
                &state,
                snapshot.storage_conversation_before.clone(),
                user_message.clone(),
                &now,
            );
            log_run_stage("prepare_context.user_message_composed");
            if snapshot.is_runtime_conversation {
                delegate_runtime_thread_conversation_update(
                    &state,
                    snapshot.runtime_conversation_id.as_deref().unwrap_or_default(),
                    updated_conversation.clone(),
                )?;
                log_run_stage("prepare_context.user_message_committed");
                updated_conversation
            } else {
                let mut updated_conversation = updated_conversation;
                for memory_id in &recall_payload.raw_ids {
                    updated_conversation.memory_recall_table.push(memory_id.clone());
                }
                pending_user_message_append = Some(UserMessageAppendInput {
                    conversation_id: snapshot.storage_conversation_before.id.clone(),
                    message: user_message.clone(),
                    memory_recall_ids: recall_payload.raw_ids.clone(),
                });
                log_run_stage("prepare_context.user_message_committed");
                log_run_stage("prepare_context.state_persist_scheduled");
                updated_conversation
            }
        };
        let latest_user_message_for_title = conversation_real_user_message_texts(&storage_conversation)
            .into_iter()
            .last()
            .unwrap_or_default();
        if should_schedule_conversation_auto_title_generation(
            &storage_conversation,
            &latest_user_message_for_title,
        ) {
            spawn_conversation_auto_title_generation(
                state.clone(),
                storage_conversation.id.clone(),
                latest_user_message_for_title,
            );
        }
        let mut conversation = trim_conversation_for_prompt_request(&storage_conversation);
        conversation.agent_id = effective_agent_id.clone();
        let (mut latest_user_text, effective_images, effective_audios) = if trigger_only {
            (
                conversation
                    .messages
                    .iter()
                    .rev()
                    .find(|message| {
                        prompt_role_for_message(message, &current_agent.id).as_deref()
                            == Some("user")
                    })
                    .map(render_prompt_user_text_only)
                    .unwrap_or_default(),
                Vec::new(),
                Vec::new(),
            )
        } else {
            let (prepared_images, prepared_audios) =
                if let Some(prepared_media) = normalized_storage_media_for_prompt.take() {
                    prepared_media
                } else {
                    let latest_user_message = storage_conversation
                        .messages
                        .iter()
                        .rev()
                        .find(|message| {
                            prompt_role_for_message(message, &current_agent.id).as_deref()
                                == Some("user")
                        })
                        .ok_or_else(|| "当前对话没有可供发送的用户消息。".to_string())?;
                    // 历史消息附件读取是同步 IO，用 block_in_place 执行，
                    // 将当前 worker 标记为 blocking，其他任务可调度到别的线程。
                    tokio::task::block_in_place(|| {
                        collect_prompt_media_parts(
                            latest_user_message,
                            Some(&state.data_path),
                        )
                    })
                };
            build_effective_prompt_media_from_prepared(
                &effective_payload,
                &selected_api,
                &prepared_images,
                &prepared_audios,
            )?
        };
        let canonical_latest_user_text =
            latest_canonical_user_prompt_text(&storage_conversation, &current_agent.id);
        let used_canonical_latest_user_text = canonical_latest_user_text.is_some();
        if let Some(canonical_latest_user_text) = canonical_latest_user_text {
            latest_user_text = canonical_latest_user_text;
        }
        let todo_enabled = selected_api.enable_tools
            && builtin_tool_prompt_rule_allowed_in_origin(
                "todo",
                RuntimeToolOriginScope::Unknown,
            );
        let attachment_relative_paths = legacy_attachment_relative_paths_for_prompt(
            &input.payload,
            used_canonical_latest_user_text,
        );
        let has_chat_request_extra_blocks =
            !trigger_only
                && (!is_delegate_conversation
                    || todo_enabled
                    || attachment_relative_paths
                        .iter()
                        .any(|path| !path.trim().is_empty()));
        let chat_overrides = ChatPromptOverrides {
            latest_user_intent: has_chat_request_extra_blocks.then_some(
                LatestUserPayloadIntent::ChatRequest {
                    include_task_board: !is_delegate_conversation,
                    include_todo_board: todo_enabled,
                    attachment_relative_paths,
                },
            ),
            todo_tool_enabled: todo_enabled,
            remote_im_activation_sources: remote_im_activation_sources.clone(),
            latest_images: (!trigger_only).then_some(effective_images.clone()),
            latest_audios: (!trigger_only).then_some(effective_audios.clone()),
            ..Default::default()
        };
        log_run_stage("prepare_context.overrides_built");
        let prompt_mode = if is_delegate_conversation {
            PromptBuildMode::Delegate
        } else {
            PromptBuildMode::Chat
        };
        let chat_overrides = Some(chat_overrides);
        log_run_stage("prepare_context.prompt_build_begin");
        let mut prepared_prompt = build_prepared_prompt_for_mode_with_stage_logger(
            prompt_mode,
            &conversation,
            &current_agent,
            &snapshot.agents,
            &snapshot.user_name,
            &snapshot.user_intro,
            &snapshot.response_style_id,
            &app_config.ui_language,
            Some(&state.data_path),
            None,
            None,
            chat_overrides.clone(),
            Some(&state),
            Some(&log_run_stage),
            Some(&selected_api),
            Some(&resolved_api),
        )?;
        if requested_plan_mode_enabled
            && !conversation_latest_user_has_plan_mode_block(&conversation, &current_agent.id)
        {
            let plan_block = plan_mode_prompt_block().trim();
            let existing_meta = prepared_prompt.latest_user_meta_text.trim();
            prepared_prompt.latest_user_meta_text = if existing_meta.is_empty() {
                plan_block.to_string()
            } else {
                format!("{plan_block}\n{existing_meta}")
            };
        }
        log_run_stage("prepare_context.prompt_built");
        let remote_im_auto_send_source = resolve_remote_im_auto_send_source(
            &state,
            &conversation.id,
            snapshot.is_remote_im_contact_conversation,
            &runtime_context,
            &remote_im_activation_sources,
        )?;
        let tool_loop_auto_compaction_context = if snapshot.is_runtime_conversation
            || runtime_context.remote_im_dynamic_boundary
        {
            None
        } else {
            Some(ToolLoopAutoCompactionContext {
                conversation_id: conversation.id.clone(),
                request_id: runtime_context
                    .request_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                assistant_message_id: Some(dispatch_assistant_message_id.clone()),
                remote_im_reply_delegate_id: runtime_context
                    .remote_im_reply_delegate_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                remote_im_auto_send_source: remote_im_auto_send_source,
                prompt_mode,
                agent: current_agent.clone(),
                agents: snapshot.agents.clone(),
                user_name: snapshot.user_name.clone(),
                user_intro: snapshot.user_intro.clone(),
                response_style_id: snapshot.response_style_id.clone(),
                ui_language: app_config.ui_language.clone(),
                chat_overrides: chat_overrides.clone(),
                trusted_prompt_usage: std::sync::Arc::new(std::sync::Mutex::new(None)),
                compaction_preserved_messages: std::sync::Arc::new(std::sync::Mutex::new(None)),
            })
        };

        // Use persisted API config as the source of truth to avoid stale
        // frontend model overrides after editing/saving config.
        let model_name = selected_api.model.trim().to_string();
        let model_name = if model_name.trim().is_empty() {
            resolved_api.model.clone()
        } else {
            model_name
        };
        let conversation_id = conversation.id.clone();
        if let Ok(mut guard) = failure_persist_target_for_run.lock() {
            *guard = Some((conversation_id.clone(), current_agent.id.clone()));
        }
        let usage_resolution = conversation_prompt_service().resolve_prompt_usage(
            &prepared_prompt,
            &selected_api,
            &current_agent,
            &conversation,
        );
        if usage_resolution.estimated_prompt_tokens.is_some() {
            log_run_stage("prepare_context.prompt_tokens_estimated");
        }
        log_run_stage("prepare_context.done");
        Ok((
            model_name,
            prepared_prompt,
            conversation_id,
            latest_user_text,
            current_agent,
            usage_resolution.estimated_prompt_tokens,
            snapshot.is_remote_im_contact_conversation,
            snapshot.remote_im_contact_processing_mode,
            tool_loop_auto_compaction_context,
            conversation,
            snapshot.is_runtime_conversation,
        ))
    };
    let mut prepared_context = prepare_request_context(persist_user_message_on_next_prepare)?;
    if let Some(append_input) = pending_user_message_append.take() {
        conversation_service_v2()
            .append_user_message(&state, &append_input)
            .await?;
    }
    let ignore_trailing_user_message_for_idle_compaction =
        persist_user_message_on_next_prepare && !trigger_only;
    let prompt_media_changed = apply_prompt_image_fallbacks_to_prepared(
        &state,
        &prepared_context.2,
        &app_config,
        &selected_api,
        &mut prepared_context.1,
    )
    .await?
        | drop_unsupported_prepared_audios(&selected_api, &mut prepared_context.1);
    if prompt_media_changed && prepared_context.5.is_some() {
        prepared_context.5 = Some(conversation_prompt_service().estimate_prepared_prompt_tokens(
            &prepared_context.1,
            &selected_api,
            &prepared_context.4,
        ));
    }
    let conversation_for_compaction = prepared_context.9.clone();
    let current_agent_id_for_compaction = prepared_context.4.id.clone();
    let estimated_prompt_tokens_before_send = prepared_context.5;
    let is_runtime_conversation = prepared_context.10;
    if is_runtime_conversation {
        if let Some(conversation_id) = requested_conversation_id.as_deref() {
            runtime_log_warn(format!(
                "[归档] 发送前检查 跳过: conversation_id={}, reason=delegate_runtime_thread",
                conversation_id
            ));
        }
    } else {
        if runtime_context.remote_im_reply_delegate_id.is_none() {
            if let Some(elapsed_hours) = remote_im_auto_compaction_idle_hours_if_due(
                &conversation_for_compaction,
                ignore_trailing_user_message_for_idle_compaction,
            ) {
            runtime_log_info(format!(
                "[远程联系人压缩] 开始，任务=发送前自动压缩，conversation_id={}, message_count={}, idle_hours={}, threshold_hours={}",
                conversation_for_compaction.id,
                conversation_for_compaction.messages.len(),
                elapsed_hours,
                REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS
            ));
            let _ = on_delta.send(round_completed_delta_event(
                &conversation_for_compaction.id,
                runtime_context.request_id.as_deref(),
                "",
                None,
            ));
            if let Err(err) =
                clear_conversation_stream_runtime_cache(&state, &conversation_for_compaction.id)
            {
                runtime_log_warn(format!(
                    "[聊天流式缓存] 远程联系人发送前自动压缩清理失败 conversation_id={} error={}",
                    conversation_for_compaction.id, err
                ));
            }
            let archive_res = run_context_compaction_pipeline(
                &state,
                &selected_api,
                &resolved_api,
                &conversation_for_compaction,
                &current_agent_id_for_compaction,
                "remote_im_idle_10h",
                "COMPACTION-AUTO",
                &[],
                false,
            )
            .await;
            match archive_res {
                Ok(result) => {
                    archived_before_send_any = archived_before_send_any || result.archived;
                    let done_message = if result
                        .warning
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .is_empty()
                    {
                        "远程联系人会话已自动整理，正在重新开始当前调度...".to_string()
                    } else {
                        format!(
                            "远程联系人会话自动整理完成（{}），正在重新开始当前调度...",
                            result.warning.unwrap_or_default()
                        )
                    };
                    let _ = on_delta.send(round_completed_delta_event(
                        &conversation_for_compaction.id,
                        runtime_context.request_id.as_deref(),
                        &done_message,
                        None,
                    ));
                    dispatch_assistant_message_id = restart_dispatch_round_after_context_compaction(
                        &state,
                        &mut runtime_context,
                        &conversation_for_compaction.id,
                        &current_agent_id_for_compaction,
                        "after_auto_compaction",
                    )?;
                    preloaded_prepare_snapshot = None;
                    persist_user_message_on_next_prepare = false;
                    continue 'dispatch;
                }
                Err(err) => {
                    runtime_log_error(format!(
                        "[远程联系人压缩] 失败，任务=发送前自动压缩，conversation_id={}，idle_hours={}，error={}",
                        conversation_for_compaction.id, elapsed_hours, err
                    ));
                }
            }
            }
        }
        if runtime_context.remote_im_dynamic_boundary
            || runtime_context.remote_im_reply_delegate_id.is_some()
        {
            runtime_log_warn(format!(
                "[远程唤醒压缩] 跳过，任务=发送前自动整理，conversation_id={}，reason={}",
                conversation_for_compaction.id,
                if runtime_context.remote_im_dynamic_boundary {
                    "dynamic_boundary"
                } else {
                    // 普通 A 不能在委托冻结后再以全局 block 执行：并发消息可在
                    // 任意时刻落库。远程应答委托的启动/续调始终以私有快照为准。
                    "remote_reply_delegate_frozen_snapshot"
                }
            ));
        } else {
        let usage_resolution = conversation_prompt_service().prime_runtime_trusted_prompt_usage(
            &mut runtime_context,
            &conversation_for_compaction,
            &prepared_context.1,
            &selected_api,
            &prepared_context.4,
        );
        let latest_real_usage = runtime_context
            .trusted_prompt_usage
            .as_ref()
            .copied()
            .filter(|usage| !usage.estimated);
        let (decision, decision_source) = decide_archive_before_send_from_usage(
            &usage_resolution,
            conversation_for_compaction.last_user_at.as_deref(),
            archive_pipeline_has_assistant_reply(&conversation_for_compaction),
            conversation_current_segment_is_compaction_summary_only(&conversation_for_compaction),
        );
        runtime_log_info(format!(
            "[归档] 发送前检查: should_archive={}, forced={}, reason={}, usage_ratio={:.4}, source={}, latest_real_effective_prompt_tokens={:?}, latest_real_usage_ratio={:?}, estimated_prompt_tokens={:?}, context_window_tokens={}",
            decision.should_archive,
            decision.forced,
            decision.reason,
            decision.usage_ratio,
            decision_source,
            latest_real_usage.map(|usage| usage.effective_prompt_tokens),
            latest_real_usage.map(|usage| usage.context_usage_ratio),
            usage_resolution.estimated_prompt_tokens.or(estimated_prompt_tokens_before_send),
            selected_api.context_window_tokens
        ));
        if decision.should_archive {
            let _ = on_delta.send(round_completed_delta_event(
                &conversation_for_compaction.id,
                runtime_context.request_id.as_deref(),
                "",
                None,
            ));
            if let Err(err) =
                clear_conversation_stream_runtime_cache(&state, &conversation_for_compaction.id)
            {
                runtime_log_warn(format!(
                    "[聊天流式缓存] 发送前压缩清理失败 conversation_id={} reason={} error={}",
                    conversation_for_compaction.id, decision.reason, err
                ));
            }

            let archive_res = run_context_compaction_pipeline(
                &state,
                &selected_api,
                &resolved_api,
                &conversation_for_compaction,
                &current_agent_id_for_compaction,
                &decision.reason,
                "COMPACTION-AUTO",
                &[],
                false,
            )
            .await;

            match archive_res {
                Ok(result) => {
                    archived_before_send_any = archived_before_send_any || result.archived;
                    if decision.forced {
                        let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                            "整理完成，正在重新开始当前调度...".to_string()
                        } else {
                            format!(
                                "整理完成（降级摘要），正在重新开始当前调度：{}",
                                result.warning.unwrap_or_default()
                            )
                        };
                        runtime_log_info(format!(
                            "[上下文整理] 发送前压缩完成 conversation_id={} reason={} message={}",
                            conversation_for_compaction.id, decision.reason, done_message
                        ));
                    }
                    runtime_log_info(format!(
                        "[聊天调度] 发送前整理命中，当前调度闭口并准备重开: conversation_id={}，reason={}",
                        conversation_for_compaction.id,
                        decision.reason
                    ));
                    dispatch_assistant_message_id = restart_dispatch_round_after_context_compaction(
                        &state,
                        &mut runtime_context,
                        &conversation_for_compaction.id,
                        &current_agent_id_for_compaction,
                        "after_auto_compaction",
                    )?;
                    preloaded_prepare_snapshot = None;
                    persist_user_message_on_next_prepare = false;
                    continue 'dispatch;
                }
                Err(err) => {
                    return Err(format!("整理失败：{err}"));
                }
            }
        }
        }
    }
    log_run_stage("pre_send_archive_checked");

    let (
        _primary_model_name,
        prepared_prompt,
        conversation_id,
        latest_user_text,
        current_agent,
        estimated_prompt_tokens,
        is_remote_im_contact_conversation,
        remote_im_contact_processing_mode,
        tool_loop_auto_compaction_context,
        conversation_for_request,
        is_runtime_conversation,
    ) = prepared_context;
    if let Some(context) = tool_loop_auto_compaction_context.as_ref() {
        let mut guard = cache_lock_recover(
            "trusted_prompt_usage",
            &context.trusted_prompt_usage,
        );
        *guard = runtime_context.trusted_prompt_usage;
    }
    log_run_stage("prompt_ready");

    let mut model_reply: Option<ModelReply> = None;
    let mut last_model_usage: Option<Value> = None;
    let mut active_selected_api = selected_api.clone();
    let mut active_resolved_api = resolved_api.clone();
    let mut fallback_errors = Vec::<String>::new();
    let prepared_prompt = prepared_prompt;
    let mut conversation_for_request = conversation_for_request;
    for (candidate_index, candidate_api_id) in candidate_api_ids.iter().enumerate() {
        let candidate_stage = format!(
            "model_candidate.start[candidate_index={},candidate_api_id={}]",
            candidate_index, candidate_api_id
        );
        log_run_stage(&candidate_stage);
        let candidate_selected_api = if candidate_api_id == &selected_api.id {
            selected_api.clone()
        } else {
            match resolve_selected_api_config(&app_config, Some(candidate_api_id.as_str())) {
                Some(api) => api,
                None => {
                    fallback_errors.push(format!("{candidate_api_id}: 候选模型不存在"));
                    continue;
                }
            }
        };
        let mut candidate_resolved_api =
            match resolve_api_config(&app_config, Some(candidate_selected_api.id.as_str())) {
                Ok(api) => api,
                Err(error) => {
                    fallback_errors.push(format!("{}: {}", candidate_selected_api.name, error));
                    continue;
                }
            };
        sync_codex_conversation_request_key(&mut candidate_resolved_api, &conversation_id);
        let candidate_model_name = if candidate_selected_api.model.trim().is_empty() {
            candidate_resolved_api.model.clone()
        } else {
            candidate_selected_api.model.trim().to_string()
        };
        let mut candidate_prepared_prompt = prepared_prompt.clone();
        if let Err(error) = maybe_prepare_aliyun_multimodal_urls_for_candidate(
            &state,
            &candidate_selected_api,
            &mut candidate_resolved_api,
            &candidate_model_name,
            &mut candidate_prepared_prompt,
            &mut conversation_for_request,
            is_runtime_conversation,
            true,
        )
        .await
        {
            fallback_errors.push(format!(
                "{}: 百炼多模态 URL 预处理失败: {}",
                candidate_selected_api.name, error
            ));
            continue;
        }
        let mut retry_budget = RetryBudget::default();
        let mut attempt: usize = 0;
        let candidate_final_error: Option<String>;
        loop {
            attempt += 1;
            let request_start_stage = format!(
                "model_request.start[candidate_api_id={},attempt={}]",
                candidate_selected_api.id, attempt
            );
            log_run_stage(&request_start_stage);
            // 调度事件：模型轮次改由工具循环逐轮打点（tool_loop），此处不再产生聚合轮次。
            let chat_round_execution = call_model_dispatch(
                &candidate_resolved_api,
                &app_config,
                &candidate_selected_api,
                &current_agent,
                &candidate_model_name,
                candidate_prepared_prompt.clone(),
                Some(&state),
                tool_loop_auto_compaction_context.as_ref(),
                on_delta,
                app_config.tool_max_iterations as usize,
                &chat_session_key,
                Some(&conversation_id),
            )
            .await;
            // 调度事件：模型轮次改由工具循环逐轮打点，这里只保留本轮 usage 供收口使用。
            if let Ok(reply) = chat_round_execution.result.as_ref() {
                if let Some(usage) = reply.usage.clone() {
                    last_model_usage = Some(usage);
                }
            }
            let restart_after_compaction = matches!(
                &chat_round_execution.result,
                Err(error) if error == CHAT_DISPATCH_RESTART_AFTER_COMPACTION
            );
            let _round_logs_recorded_internally = chat_round_execution
                .result
                .as_ref()
                .ok()
                .map(|reply| reply.round_logs_recorded_internally)
                .unwrap_or(false);
            // 已切换至调度事件：旧 chat 单轮日志不再写入，仅保留 schedule_event 打点
            let _ = &chat_round_execution.log_parts;
            let request_finish_stage = format!(
                "model_request.finish[candidate_api_id={},attempt={}]",
                candidate_selected_api.id, attempt
            );
            log_run_stage(&request_finish_stage);

            if restart_after_compaction {
                runtime_log_info(format!(
                    "[聊天调度] 续调整理命中，当前调度闭口并准备重开: conversation_id={}",
                    conversation_id
                ));
                runtime_context.compaction_preserved_messages =
                    chat_round_execution.compaction_preserved_messages.clone();
                runtime_context.compaction_preserved_messages_ready =
                    runtime_context.compaction_preserved_messages.is_some();
                dispatch_assistant_message_id = restart_dispatch_round_after_context_compaction(
                    &state,
                    &mut runtime_context,
                    &conversation_id,
                    &current_agent.id,
                    "after_tool_continue_compaction",
                )?;
                preloaded_prepare_snapshot = None;
                persist_user_message_on_next_prepare = false;
                continue 'dispatch;
            }

            // Visible 成功：随栈销毁即清零（RetryBudget为请求内局部变量，不跨请求）
            if let Ok(reply) = &chat_round_execution.result {
                if matches!(
                    model_reply_content_state(reply),
                    ModelReplyContentState::Visible
                ) {
                    active_selected_api = candidate_selected_api.clone();
                    active_resolved_api = candidate_resolved_api.clone();
                    model_reply = Some(reply.clone());
                    candidate_final_error = None;
                    break;
                }
            }

            let retry_kind = classify_retry_kind_for_result(&chat_round_execution.result);
            let (reason_text, final_error_text) = match &chat_round_execution.result {
                Ok(reply) => match model_reply_content_state(reply) {
                    ModelReplyContentState::ReasoningOnly => {
                        runtime_log_warn(format!(
                            "[聊天] 模型返回思考但缺少最终回答，按空回重试: conversation_id={}，api_config_id={}，model={}，attempt={}，activity_reasoning_len={}",
                            conversation_id,
                            candidate_selected_api.id,
                            candidate_selected_api.model,
                            attempt,
                            reply.activity_reasoning_text.chars().count()
                        ));
                        (
                            "模型只返回了思维链但没有最终回答".to_string(),
                            "模型只返回了思维链但没有最终回答，已停止重试；请稍后重试或切换模型。"
                                .to_string(),
                        )
                    }
                    ModelReplyContentState::Empty => {
                        runtime_log_warn(format!(
                            "[聊天] 模型返回空响应，按空回重试: conversation_id={}，api_config_id={}，model={}，attempt={}",
                            conversation_id,
                            candidate_selected_api.id,
                            candidate_selected_api.model,
                            attempt
                        ));
                        (
                            "模型权限/套餐不支持（上游返回空响应）".to_string(),
                            "模型权限/套餐不支持（上游返回空响应），已停止重试，请检查当前 API Key 是否支持该模型，或切换模型。"
                                .to_string(),
                        )
                    }
                    ModelReplyContentState::Visible => unreachable!(),
                },
                Err(error) => {
                    if retry_kind == RetryKind::RateLimited {
                        runtime_log_warn(format!(
                            "[聊天] 命中限流429，按限流重试: conversation_id={}，api_config_id={}，model={}，attempt={}，err={}",
                            conversation_id, candidate_selected_api.id, candidate_selected_api.model, attempt, error
                        ));
                        (
                            "请求过于频繁(429)".to_string(),
                            format!("请求过于频繁(429)，已停止重试：{error}"),
                        )
                    } else {
                        (
                            "模型请求失败".to_string(),
                            format!("模型请求失败，已停止重试：{error}"),
                        )
                    }
                }
            };

            if let Some(wait) = retry_policy(retry_kind, &retry_budget) {
                let (retry_index, max_for_kind, wait_seconds) = match retry_kind {
                    RetryKind::Empty => (
                        (retry_budget.empty_used + 1) as usize,
                        MAX_EMPTY_RETRIES as usize,
                        wait.as_secs(),
                    ),
                    RetryKind::RateLimited => (
                        (retry_budget.rate_used + 1) as usize,
                        MAX_RATE_RETRIES as usize,
                        wait.as_secs(),
                    ),
                    RetryKind::NonRetryable => unreachable!(),
                };
                match retry_kind {
                    RetryKind::Empty => retry_budget.empty_used += 1,
                    RetryKind::RateLimited => retry_budget.rate_used += 1,
                    RetryKind::NonRetryable => {}
                }
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: "".to_string(),
                    kind: Some("tool_status".to_string()),
                    request_id: None,
                    activation_id: runtime_context.request_id.clone(),
                    phase_id: None,
                    reason: None,
                    tool_name: None,
                    tool_call_id: None,
                    tool_status: Some("running".to_string()),
                    tool_args: None,
                    message: Some(format!(
                        "{reason_text}，正在重试 ({retry_index}/{max_for_kind})，等待 {wait_seconds} 秒..."
                    )),
                    stream_cache: None,
                });
                tokio::time::sleep(wait).await;
                continue;
            }
            let total_attempts = attempt;
            candidate_final_error = Some(format!(
                "{final_error_text} (attempted {total_attempts} times)"
            ));
            break;
        }
        if model_reply.is_some() {
            break;
        }
        if let Some(error) = candidate_final_error {
            fallback_errors.push(format!("{}: {}", candidate_selected_api.name, error));
        }
        if candidate_index + 1 < candidate_api_ids.len() {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                request_id: None,
                activation_id: runtime_context.request_id.clone(),
                phase_id: None,
                reason: None,
                tool_name: None,
                tool_call_id: None,
                tool_status: Some("running".to_string()),
                tool_args: None,
                message: Some(format!(
                    "当前模型失败，正在切换到下一个候选模型（{}/{}）...",
                    candidate_index + 2,
                    candidate_api_ids.len()
                )),
                stream_cache: None,
            });
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    let model_reply =
        model_reply.ok_or_else(|| {
            if fallback_errors.is_empty() {
                "模型回复无效：未收到可用内容。".to_string()
            } else {
                format!("所有候选模型均失败：{}", fallback_errors.join(" | "))
            }
        })?;
    let assistant_text = model_reply.assistant_text;
    let final_response_text = model_reply.final_response_text;
    let activity_reasoning_text = model_reply.activity_reasoning_text;
    let assistant_provider_meta_override = model_reply.assistant_provider_meta;
    let tool_history_events = model_reply.tool_history_events;
    let suppress_assistant_message = model_reply.suppress_assistant_message
        || runtime_context
            .remote_im_reply_delegate_id
            .as_deref()
            .map(|delegate_id| !remote_im_reply_delegate_is_active(&state, delegate_id))
            .unwrap_or(false);
    // 群聊长度门二次改写默认禁用：模型回复完成后直接使用原始 assistant_text/final_response_text。
    // 这段逻辑保留在 remote_im_reply_delegate_finalize_group_reply_draft(...)，但不再在默认发送路径调用，
    // 避免在 model_request.finish 之后额外触发一次 30 秒快速模型请求。
    // if !suppress_assistant_message {
    //     if let Some(delegate_id) = runtime_context
    //         .remote_im_reply_delegate_id
    //         .as_deref()
    //         .map(str::trim)
    //         .filter(|value| !value.is_empty())
    //     {
    //         if let Some((_contact_id, dispatch_policy)) =
    //             remote_im_reply_delegate_group_policy(&state, delegate_id)
    //         {
    //             let draft = if assistant_text.trim().is_empty() {
    //                 final_response_text.clone()
    //             } else {
    //                 assistant_text.clone()
    //             };
    //             let final_text = remote_im_reply_delegate_finalize_group_reply_draft(
    //                 &state,
    //                 delegate_id,
    //                 &draft,
    //                 dispatch_policy.max_chars,
    //             )
    //             .await;
    //             assistant_text = final_text.clone();
    //             final_response_text = final_text;
    //         }
    //     }
    // }
    let remote_im_auto_send_source = match resolve_remote_im_auto_send_source(
        &state,
        &conversation_id,
        is_remote_im_contact_conversation,
        &runtime_context,
        &remote_im_activation_sources,
    ) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM][自动发送] 目标解析失败，已降级为不自动发送，conversation_id={}，error={}",
                conversation_id, err
            ));
            None
        }
    };
    let mut remote_im_reply_decision =
        remote_im_extract_reply_decision_from_tool_history(&tool_history_events);
    let pending_remote_im_auto_send_target = match resolve_remote_im_auto_send_target(
        &final_response_text,
        remote_im_auto_send_source
            .as_ref()
            .map(std::slice::from_ref)
            .unwrap_or(&[]),
        remote_im_auto_send_source.is_some(),
    ) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM][自动发送] 发送判定失败，已降级为不自动发送，conversation_id={}，error={}",
                conversation_id, err
            ));
            None
        }
    };
    if let Some(target) = pending_remote_im_auto_send_target.as_ref() {
        if remote_im_reply_decision.is_none() {
            remote_im_reply_decision = Some(RemoteImReplyDecisionSummary {
                action: "send_async".to_string(),
                target: Some(RemoteImReplyTarget {
                    channel_id: target.channel_id.clone(),
                    contact_id: target.remote_contact_id.clone(),
                }),
            });
        }
    }
    let trusted_input_tokens = model_reply.trusted_input_tokens;
    let estimated_prompt_tokens = estimated_prompt_tokens.unwrap_or_else(|| {
        if trusted_input_tokens.is_some() {
            0
        } else {
            conversation_prompt_service().estimate_prepared_prompt_tokens(
                &prepared_prompt,
                &active_selected_api,
                &current_agent,
            )
        }
    });
    let (effective_prompt_tokens, effective_prompt_source) =
        effective_prompt_tokens_from_provider(estimated_prompt_tokens, trusted_input_tokens);
    let context_usage_ratio =
        effective_prompt_tokens as f64 / f64::from(active_selected_api.context_window_tokens.max(1));
    let context_usage_percent = context_usage_ratio.mul_add(100.0, 0.0).round().clamp(0.0, 100.0) as u32;
    conversation_prompt_service().update_runtime_trusted_prompt_usage_from_request(
        &mut runtime_context,
        trusted_input_tokens,
        Some(estimated_prompt_tokens),
        &active_selected_api,
    );

    let assistant_request_messages = assistant_request_sequence_from_tool_history(
        &tool_history_events,
        &assistant_text,
        &activity_reasoning_text,
    );
    let remote_im_conversation_kind = if is_remote_im_contact_conversation {
        "remote_im_contact"
    } else {
        "standard_conversation"
    };
    let dispatch_elapsed_ms = chat_started_at
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let mut provider_meta = {
        if !should_create_assistant_provider_meta(
            &active_selected_api.request_format,
            assistant_provider_meta_override.as_ref(),
            trusted_input_tokens,
            estimated_prompt_tokens,
            remote_im_reply_decision.is_some(),
        ) {
            Some(serde_json::json!({
                "dispatchElapsedMs": dispatch_elapsed_ms
            }))
        } else {
            let mut meta = serde_json::json!({
                "dispatchElapsedMs": dispatch_elapsed_ms
            });
            // 持久化只写本次 call 的真实用量；流式估算不落盘。
            // 工具轮的真实用量随工具调用事件本身携带，此处缺真实值时由聚合侧从事件兜底。
            if let Some(prompt_tokens) = trusted_input_tokens.filter(|value| *value > 0) {
                let context_window = active_selected_api.context_window_tokens.max(1);
                let usage_ratio = prompt_tokens as f64 / f64::from(context_window);
                let usage_percent =
                    usage_ratio.mul_add(100.0, 0.0).round().clamp(0.0, 100.0) as u32;
                if let Some(object) = meta.as_object_mut() {
                    object.insert("providerPromptTokens".to_string(), serde_json::json!(prompt_tokens));
                    object.insert("effectivePromptTokens".to_string(), serde_json::json!(prompt_tokens));
                    object.insert(
                        "effectivePromptSource".to_string(),
                        serde_json::json!("provider"),
                    );
                    object.insert("contextUsageRatio".to_string(), serde_json::json!(usage_ratio));
                    object.insert(
                        "contextUsagePercent".to_string(),
                        serde_json::json!(usage_percent),
                    );
                    object.insert(
                        "contextWindowTokens".to_string(),
                        serde_json::json!(context_window),
                    );
                }
            }
            if let Some(decision) = remote_im_reply_decision.as_ref() {
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert(
                        "remoteImDecision".to_string(),
                        serde_json::json!({
                            "action": decision.action,
                            "processingMode": remote_im_contact_processing_mode,
                            "conversationKind": remote_im_conversation_kind,
                            "activationSourceCount": remote_im_activation_sources.len(),
                            "target": decision.target,
                        }),
                    );
                }
            }
            Some(meta)
        }
    };
    if let Some(extra_meta) = assistant_provider_meta_override {
        let mut merged = provider_meta.take().unwrap_or_else(|| serde_json::json!({}));
        if !merged.is_object() {
            runtime_log_warn(format!(
                "[聊天] 助理 provider_meta 不是对象，合并前已保留原始值: value={}",
                merged
            ));
            let raw_provider_meta = std::mem::replace(&mut merged, serde_json::json!({}));
            merged = serde_json::json!({
                "_raw_provider_meta": raw_provider_meta,
            });
        }
        if let Some(target) = merged.as_object_mut() {
            if let Some(extra_object) = extra_meta.as_object() {
                for (key, value) in extra_object {
                    target.insert(key.clone(), value.clone());
                }
            }
            target.insert(
                "dispatchElapsedMs".to_string(),
                serde_json::json!(dispatch_elapsed_ms),
            );
        }
        provider_meta = Some(merged);
    }
    if let Some(delegate_id) = runtime_context
        .remote_im_reply_delegate_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let trigger_message_id = runtime_context
            .remote_im_reply_trigger_message_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let mut meta = provider_meta.take().unwrap_or_else(|| serde_json::json!({}));
        if !meta.is_object() {
            meta = serde_json::json!({});
        }
        if let Some(object) = meta.as_object_mut() {
            object.insert(
                "remoteImReplyDelegate".to_string(),
                serde_json::json!({
                    "delegateId": delegate_id,
                    "triggerMessageId": trigger_message_id,
                    "outputStage": "final"
                }),
            );
        }
        provider_meta = Some(meta);
    }
    let assistant_message_id = dispatch_assistant_message_id.clone();
    log_run_stage("model_reply_ready");

    let mut persisted_assistant_message: Option<ChatMessage> = None;
    let mut auto_push_remote_contact_id: Option<String> = None;
    {
        if let Ok(conversation_meta) =
            conversation_service_v2().get_conversation_meta(&state, &conversation_id)
        {
            auto_push_remote_contact_id = conversation_meta
                .auto_push_remote_contact_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            if let (Some(delegate_id), Some(trigger_message_id)) = (
                runtime_context
                    .remote_im_reply_delegate_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
                runtime_context
                    .remote_im_reply_trigger_message_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
            ) {
                if remote_im_reply_delegate_is_active(&state, delegate_id) {
                    let visible_texts = remote_im_reply_delegate_visible_texts(&assistant_request_messages);
                    for (index, text) in visible_texts
                    .iter()
                    .take(visible_texts.len().saturating_sub(1))
                    .enumerate()
                {
                    let intermediate_message_id = Uuid::new_v4().to_string();
                    let provider_meta_patch = remote_im_reply_delegate_stage_provider_meta(
                        delegate_id,
                        trigger_message_id,
                        &format!("intermediate_{}", index + 1),
                    );
                    conversation_service_v2().bootstrap_streaming_assistant_message(
                        &state,
                        &AssistantMessageBootstrapInput {
                            conversation_id: conversation_id.clone(),
                            assistant_message_id: intermediate_message_id.clone(),
                            speaker_agent_id: current_agent.id.clone(),
                            created_at: Some(now_iso()),
                            provider_meta_patch: Some(provider_meta_patch.clone()),
                            compaction_preserved_messages: None,
                        },
                    )?;
                    conversation_service_v2().append_final_text_to_assistant_message(
                        &state,
                        &AssistantMessageFinalTextAppendInput {
                            conversation_id: conversation_id.clone(),
                            assistant_message_id: intermediate_message_id.clone(),
                            final_text: text.clone(),
                            reasoning_text: None,
                            provider_meta_patch: Some(provider_meta_patch),
                            meme_annotations: None,
                        },
                    )?;
                    // 未来的自己请停手：这个 message 会镜像进远程应答委托线程，
                    // 属于后端持久化/生成链路。绝对不能读取 frontend_display_only，
                    // 否则工具历史会被展示投影污染后继续进模型/持久化流程。
                    let message = conversation_service_v2()
                        .get_raw_message_by_id(&state, &conversation_id, &intermediate_message_id)
                        .ok();
                    if let Some(message) = message.as_ref() {
                        if let Err(err) = remote_im_reply_delegate_mirror_message(
                            &state,
                            delegate_id,
                            message.clone(),
                            None,
                        ) {
                            runtime_log_warn(format!(
                                "[远程应答委托] 失败，任务=镜像中间正文，delegate_id={}，message_id={}，error={}",
                                delegate_id, intermediate_message_id, err
                            ));
                        }
                        // 远程应答使用空 Channel 执行，需主动通知当前会话视图刷新。
                        emit_conversation_message_appended_event(
                            &state,
                            &conversation_id,
                            message,
                        );
                    }
                    runtime_log_debug(format!(
                        "[远程应答委托] 中间正文仅持久化，等待最终回复统一外发，delegate_id={}，message_id={}",
                        delegate_id, intermediate_message_id
                    ));
                    }
                }
            }
            if !suppress_assistant_message {
                let (final_text, reasoning_text) =
                    extract_final_assistant_text_and_meta(&assistant_request_messages);
                let provider_meta_patch = normalize_assistant_provider_meta(provider_meta);
                let meme_annotations = populate_assistant_meme_annotations(
                    &state,
                    &assistant_message_id,
                    &final_text,
                )?;
                runtime_log_debug(format!(
                    "[表情替换] 提交前，conversation_id={}，assistant_message_id={}，annotation_count={}，tokens=[{}]，final_text={}",
                    conversation_id,
                    assistant_message_id,
                    meme_annotations
                        .as_ref()
                        .map(Vec::len)
                        .unwrap_or(0),
                    meme_annotations
                        .as_ref()
                        .map(|items| items.iter().map(|item| item.meme.trim().to_string()).collect::<Vec<_>>().join(","))
                        .unwrap_or_default(),
                    final_text.replace('\n', "\\n")
                ));
                log_run_stage("assistant_final_append.start");
                let append_final_result = conversation_service_v2().append_final_text_to_assistant_message(
                    &state,
                    &AssistantMessageFinalTextAppendInput {
                        conversation_id: conversation_id.clone(),
                        assistant_message_id: assistant_message_id.clone(),
                        final_text,
                        reasoning_text,
                        provider_meta_patch,
                        meme_annotations,
                    },
                );
                log_run_stage("assistant_final_append.finish");
                append_final_result?;
                // 未来的自己请停手：persisted_assistant_message 后面会用于远程应答镜像、
                // 自动推送和返回结果的内部判断，属于后端链路。绝对不能读取
                // frontend_display_only；真正发前端时由事件/command 出口再投影。
                persisted_assistant_message = conversation_service_v2()
                    .get_raw_message_by_id(&state, &conversation_id, &assistant_message_id)
                    .ok();
                if let (Some(delegate_id), Some(message)) = (
                    runtime_context.remote_im_reply_delegate_id.as_deref(),
                    persisted_assistant_message.as_ref(),
                ) {
                    if let Err(err) = remote_im_reply_delegate_mirror_message(
                        &state,
                        delegate_id,
                        message.clone(),
                        None,
                    ) {
                        runtime_log_warn(format!(
                            "[远程应答委托] 失败，任务=镜像最终正文，delegate_id={}，message_id={}，error={}",
                            delegate_id, message.id, err
                        ));
                    }
                    // 远程应答不向普通聊天 Channel 推送 delta，最终正文须显式广播。
                    emit_conversation_message_appended_event(&state, &conversation_id, message);
                }
                runtime_log_debug(format!(
                    "[表情替换] 提交后，conversation_id={}，assistant_message_id={}，persisted_annotation_count={}，persisted_tokens=[{}]",
                    conversation_id,
                    assistant_message_id,
                    persisted_assistant_message
                        .as_ref()
                        .and_then(|message| message.meme_annotations.as_ref().map(Vec::len))
                        .unwrap_or(0),
                    persisted_assistant_message
                        .as_ref()
                        .and_then(|message| {
                            message.meme_annotations.as_ref().map(|items| {
                                items
                                    .iter()
                                    .map(|item| item.meme.trim().to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                        })
                        .unwrap_or_default()
                ));
            }
        } else if let Some(mut conversation) =
            delegate_runtime_thread_conversation_get(&state, &conversation_id)?
        {
            let now = now_iso();
            if !suppress_assistant_message {
                let mut assistant_message = build_assistant_message_from_request_sequence(
                    assistant_message_id.clone(),
                    &current_agent.id,
                    now.clone(),
                    &assistant_request_messages,
                    provider_meta,
                );
                assistant_message.meme_annotations = populate_assistant_meme_annotations(
                    &state,
                    &assistant_message_id,
                    assistant_message
                        .parts
                        .iter()
                        .find_map(|part| match part {
                            MessagePart::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .unwrap_or(""),
                )?;
                persisted_assistant_message = Some(conversation_upsert_final_assistant_message(
                    &mut conversation,
                    &current_agent.id,
                    assistant_message,
                    &now,
                )?);
            }
            delegate_runtime_thread_conversation_update(&state, &conversation_id, conversation)?;
        }
    }
    log_run_stage("assistant_message_persist_scheduled");

    if !suppress_assistant_message {
        if let Some(remote_contact_id) = auto_push_remote_contact_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let auto_push_content = persisted_assistant_message
                .as_ref()
                .map(render_prompt_message_text)
                .unwrap_or_else(|| assistant_text.clone());
            if !auto_push_content.trim().is_empty() {
                if let Err(err) = conversation_service_v2().enqueue_auto_push_remote_contact_message(
                    &state,
                    &conversation_id,
                    remote_contact_id,
                    &auto_push_content,
                ) {
                    runtime_log_warn(format!(
                        "[自动推送] 失败，任务=会话 assistant 自动推送，conversation_id={}，remote_contact_id={}，error={}",
                        conversation_id,
                        remote_contact_id,
                        err
                    ));
                }
            }
        }
    }

    if remote_im_should_auto_send_after_core_round(&runtime_context) {
        if let Some(activation_source) = pending_remote_im_auto_send_target {
            let final_auto_send_message = persisted_assistant_message.clone().filter(|message| {
                message
                    .parts
                    .iter()
                    .find_map(|part| match part {
                        MessagePart::Text { text, .. } => Some(text.trim()),
                        _ => None,
                    })
                    .map(|text| text == final_response_text.trim())
                    .unwrap_or(false)
            });
            spawn_remote_im_auto_send_contact_assistant_reply(
                state.clone(),
                activation_source,
                conversation_id.clone(),
                final_response_text.clone(),
                final_auto_send_message.clone(),
                persisted_assistant_message.as_ref().map(|message| message.id.clone()),
                None,
            );
        }
    }

        break Ok(SendChatResult {
            conversation_id,
            latest_user_text,
            assistant_text,
            final_response_text,
            archived_before_send: archived_before_send_any,
            assistant_message: persisted_assistant_message,
            provider_prompt_tokens: trusted_input_tokens,
            estimated_prompt_tokens: Some(estimated_prompt_tokens),
            effective_prompt_tokens: Some(effective_prompt_tokens),
            effective_prompt_source: Some(effective_prompt_source.to_string()),
            context_window_tokens: Some(active_selected_api.context_window_tokens),
            max_output_tokens: active_resolved_api.max_output_tokens,
            context_usage_percent: Some(context_usage_percent),
            remote_im_reply_decision: remote_im_reply_decision
                .as_ref()
                .map(|item| item.action.clone()),
            remote_im_reply_target: remote_im_reply_decision.and_then(|item| item.target),
            usage: last_model_usage.clone(),
            activation_request_id: runtime_context.request_id.clone(),
        });
    }
    };

    let result = futures_util::future::Abortable::new(run, abort_registration).await;
    emit_conversation_work_status(match &result {
        Ok(Ok(_)) | Err(_) => "completed",
        Ok(Err(_)) => "error",
    });
    flush_chat_timeline("send_chat_message_inner.finish");
    {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        inflight.remove(&chat_key);
    }
    if let Err(err) = clear_inflight_tool_abort_handle(state, &chat_key) {
        runtime_log_error(format!(
            "[聊天] 清理进行中工具中断句柄失败 (session={}): {}",
            chat_key, err
        ));
    }
    let final_result = match result {
        Ok(inner) => inner,
        Err(_) => {
            runtime_log_info(format!(
                "[聊天] 用户中止聊天请求 (session={})",
                chat_key
            ));
            Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
        }
    };
    let should_clear_completed_tool_history = true;
    if let Err(err) = final_result.as_ref() {
        if err == CHAT_ABORTED_BY_USER_ERROR {
            let failure_persist_target = failure_persist_target
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            let interrupted_conversation_id = failure_persist_target
                .as_ref()
                .map(|(conversation_id, _)| conversation_id.as_str())
                .or(requested_conversation_id_for_failure_persist.as_deref());
            let interrupted_agent_id = failure_persist_target
                .as_ref()
                .map(|(_, agent_id)| agent_id.as_str())
                .unwrap_or(effective_agent_id_for_failure_persist.as_str());
            match persist_aborted_chat_partial_result(
                state,
                interrupted_conversation_id,
                interrupted_agent_id,
                &chat_key,
            ) {
                Ok(Some(interrupted_result)) => emit_round_completed_event(
                    state,
                    interrupted_result.conversation_id.as_str(),
                    &interrupted_result,
                    None,
                    None,
                ),
                Ok(None) => {}
                Err(persist_err) => runtime_log_warn(format!(
                    "[聊天] 用户停止后 partial 收尾失败: session={} error={} persist_error={}",
                    chat_key, err, persist_err
                )),
            }
        } else {
            let failure_persist_target = failure_persist_target
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            let failure_persist_conversation_id = failure_persist_target
                .as_ref()
                .map(|(conversation_id, _)| conversation_id.as_str())
                .or(requested_conversation_id_for_failure_persist.as_deref());
            let failure_persist_agent_id = failure_persist_target
                .as_ref()
                .map(|(_, agent_id)| agent_id.as_str())
                .unwrap_or(effective_agent_id_for_failure_persist.as_str());
            match persist_failed_chat_completed_tool_history(
                state,
                failure_persist_conversation_id,
                failure_persist_agent_id,
                &chat_key,
                err,
            ) {
                Ok(true) => runtime_log_error(format!(
                    "[聊天] 模型失败前工具历史已落盘: session={} error={}",
                    chat_key, err
                )),
                Ok(false) => {}
                Err(persist_err) => runtime_log_warn(format!(
                    "[聊天] 模型失败前工具历史落盘失败: session={} original_error={} persist_error={}",
                    chat_key, err, persist_err
                )),
            }
        }
    }
    if should_clear_completed_tool_history {
        if let Err(err) = clear_inflight_completed_tool_history(state, &chat_key) {
            runtime_log_error(format!(
                "[聊天] 清理已完成工具历史缓存失败 (session={}): {}",
                chat_key, err
            ));
        }
    }
    // 调度事件：收口（全量，收口与 pipeline 日志同作用域，长度全量不裁剪）
    {
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let success = final_result.is_ok();
        let detail = if let Some(err) = final_result.as_ref().err() {
            serde_json::json!({ "error": err })
        } else if let Some(ok) = final_result.as_ref().ok() {
            let raw_text = ok.assistant_text.as_str();
            let preview: String = raw_text.to_string();
            let mut obj = serde_json::json!({
                "assistantTextLength": raw_text.chars().count(),
                "conversationId": ok.conversation_id,
            });
            if !preview.trim().is_empty() {
                if let Some(map) = obj.as_object_mut() {
                    map.insert("textPreview".to_string(), serde_json::json!(preview));
                }
            }
            // usage 全量（若存在），便于 LogTab 重建 — 优先使用模型返回的真实用量
            if let Some(usage) = ok.usage.clone() {
                if let Some(map) = obj.as_object_mut() {
                    map.insert("usage".to_string(), usage);
                }
            } else {
                let mut usage_obj = serde_json::Map::new();
                if let Some(v) = ok.provider_prompt_tokens { usage_obj.insert("providerPromptTokens".to_string(), serde_json::json!(v)); }
                if let Some(v) = ok.estimated_prompt_tokens { usage_obj.insert("estimatedPromptTokens".to_string(), serde_json::json!(v)); }
                if let Some(v) = ok.effective_prompt_tokens { usage_obj.insert("effectivePromptTokens".to_string(), serde_json::json!(v)); }
                if let Some(v) = ok.effective_prompt_source.as_ref() { usage_obj.insert("effectivePromptSource".to_string(), serde_json::json!(v)); }
                if let Some(v) = ok.context_window_tokens { usage_obj.insert("contextWindowTokens".to_string(), serde_json::json!(v)); }
                if let Some(v) = ok.max_output_tokens { usage_obj.insert("maxOutputTokens".to_string(), serde_json::json!(v)); }
                if let Some(v) = ok.context_usage_percent { usage_obj.insert("contextUsagePercent".to_string(), serde_json::json!(v)); }
                if !usage_obj.is_empty() {
                    if let Some(map) = obj.as_object_mut() {
                        map.insert("usage".to_string(), Value::Object(usage_obj));
                    }
                }
            }
            obj
        } else {
            serde_json::json!({})
        };
        let _ = schedule_event_push_if_delegate(
            state,
            &runtime_context_for_schedule_events,
            requested_conversation_id_for_schedule_events
                .as_deref()
                .unwrap_or(chat_session_key_for_log.as_str()),
            &trace_id_for_schedule_events,
            "dispatch_end",
            elapsed_ms,
            Some(success),
            detail,
        );
    }
    let timeline = stage_timeline.lock().ok().map(|items| items.clone());
    let (mut pipeline_headers, pipeline_tools) = latest_chat_round_headers_and_tools(
        state,
        Some(&chat_session_key_for_log),
        resolved_api_for_log.request_format,
        &selected_api_for_log.name,
        &selected_api_for_log.model,
        &resolved_api_for_log.base_url,
    );
    if pipeline_headers.is_empty() {
        pipeline_headers = masked_auth_headers(&selected_api_for_log.api_key);
    }
    // 调度事件：全量 Run 头补齐（同次调度内 headers/baseUrl/tools 仅在 Run 头存一次，不在后续事件重复）
    {
        let _ = schedule_event_update_run_metadata(
            state,
            requested_conversation_id_for_schedule_events
                .as_deref()
                .unwrap_or(chat_session_key_for_log.as_str()),
            &trace_id_for_schedule_events,
            serde_json::json!({
                "traceId": trace_id_for_schedule_events,
                "scene": "chat_pipeline",
                "requestFormat": resolved_api_for_log.request_format.as_str(),
                "provider": selected_api_for_log.name,
                "model": selected_api_for_log.model,
                "baseUrl": resolved_api_for_log.base_url,
                "headers": pipeline_headers,
                "tools": pipeline_tools,
            }),
        );
    }
    // 已切换至调度事件：旧 pipeline 日志不再写入
    let _ = (&trace_id, &chat_session_key_for_log, &timeline, &final_result);
    // 兜底催处理：归档阶段可能短暂阻塞出队，这里在当前轮次结束后补一次调度触发。
    trigger_chat_queue_processing(state);
    final_result
}

fn should_create_assistant_provider_meta(
    request_format: &RequestFormat,
    assistant_provider_meta_override: Option<&Value>,
    trusted_input_tokens: Option<u64>,
    estimated_prompt_tokens: u64,
    remote_im_reply_decision_present: bool,
) -> bool {
    assistant_provider_meta_override.is_some()
        || trusted_input_tokens.is_some()
        || estimated_prompt_tokens > 0
        || remote_im_reply_decision_present
        || matches!(request_format, RequestFormat::DeepSeek | RequestFormat::DeepSeekKimi)
}

#[cfg(test)]
mod core_send_inner_tests {
    use super::*;

    fn test_chat_api(id: &str, enable_image: bool) -> ApiConfig {
        ApiConfig {
            id: id.to_string(),
            name: id.to_string(),
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            enable_text: true,
            enable_image,
            enable_audio: false,
            enable_video: false,
            enable_tools: false,
            tools: vec![],
            base_url: "https://example.com/v1".to_string(),
            api_key: "k".to_string(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            codex_custom_url: None,
            codex_custom_api_key: None,
            codex_originator: default_codex_originator(),
            codex_residency_requirement: None,
            model: format!("model-{id}"),
            reasoning_effort: default_reasoning_effort(),
            temperature: 0.7,
            custom_temperature_enabled: false,
            context_window_tokens: 128_000,
            max_output_tokens: 4_096,
            custom_max_output_tokens_enabled: false,
            failure_retry_count: 0,
        }
    }

    fn test_agent_with_models(
        api_config_ids: Vec<&str>,
        model_failure_fallback_enabled: bool,
    ) -> AgentProfile {
        let ids = api_config_ids
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let mut agent = default_agent();
        agent.id = "agent-a".to_string();
        agent.name = "人格 A".to_string();
        agent.api_config_ids = ids.clone();
        agent.api_config_id = ids.first().cloned().unwrap_or_default();
        agent.model_failure_fallback_enabled = model_failure_fallback_enabled;
        agent
    }

    fn test_rfc3339_hours_ago(hours: i64) -> String {
        (now_utc() - time::Duration::hours(hours))
            .replace_nanosecond(0)
            .expect("strip nanos")
            .format(&Rfc3339)
            .expect("format test timestamp")
    }

    fn test_message_at(id: &str, role: &str, created_at: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            speaker_agent_id: (role == "assistant").then_some("agent-a".to_string()),
            parts: vec![MessagePart::Text {
                text: format!("{role} message {id}"),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn test_compaction_message_at(id: &str, created_at: &str) -> ChatMessage {
        let mut message = test_message_at(id, "user", created_at);
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "scene": "compaction"
            }
        }));
        message
    }

    #[test]
    fn main_assistant_activation_should_reject_latest_message_from_same_agent() {
        let latest = test_message_at("assistant-existing", "assistant", &now_iso());
        assert!(main_assistant_activation_should_reject_latest_message(
            &latest,
            "agent-a",
        ));
    }

    #[test]
    fn main_assistant_activation_should_allow_latest_user_or_other_agent_message() {
        let user_message = test_message_at("user-latest", "user", &now_iso());
        assert!(!main_assistant_activation_should_reject_latest_message(
            &user_message,
            "agent-a",
        ));

        let mut other_assistant_message = test_message_at("assistant-other", "assistant", &now_iso());
        other_assistant_message.speaker_agent_id = Some("agent-b".to_string());
        assert!(!main_assistant_activation_should_reject_latest_message(
            &other_assistant_message,
            "agent-a",
        ));
    }

    #[test]
    fn current_segment_compaction_summary_only_should_ignore_preceding_history() {
        let now = now_iso();
        let mut messages = vec![
            test_message_at("old-user", "user", &now),
            test_message_at("old-assistant", "assistant", &now),
            test_compaction_message_at("summary", &now),
        ];
        let conversation = test_remote_im_conversation_with_messages(messages.clone());

        assert!(conversation_current_segment_is_compaction_summary_only(
            &conversation
        ));

        messages.push(test_message_at("new-user", "user", &now));
        let conversation = test_remote_im_conversation_with_messages(messages);
        assert!(!conversation_current_segment_is_compaction_summary_only(
            &conversation
        ));
    }

    fn test_remote_im_conversation_with_messages(messages: Vec<ChatMessage>) -> Conversation {
        let now = now_iso();
        Conversation {
            id: "remote-conversation-a".to_string(),
            title: "远程联系人".to_string(),
            agent_id: "agent-a".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now,
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages,
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    fn test_prepared_prompt_with_user_image_history(
        history_user_count: usize,
        latest_user_text: &str,
    ) -> PreparedPrompt {
        let history_messages = (0..history_user_count)
            .map(|idx| PreparedHistoryMessage {
                role: "user".to_string(),
                text: format!("user-{idx}"),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: vec![PreparedBinaryPayload {
                    label: format!("图片#{}", idx + 1),
                    mime: "image/png".to_string(),
                    content: B64.encode(format!("image-{idx}").as_bytes()),
                    saved_path: Some(format!("downloads/image-{idx}.png")),
                }],
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            })
            .collect::<Vec<_>>();
        PreparedPrompt {
            preamble: String::new(),
            history_messages,
            latest_user_text: latest_user_text.to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: if latest_user_text.trim().is_empty() {
                Vec::new()
            } else {
                vec![PreparedBinaryPayload {
                    label: "图片#1".to_string(),
                    mime: "image/png".to_string(),
                    content: B64.encode(b"latest"),
                    saved_path: Some("downloads/latest.png".to_string()),
                }]
            },
            latest_audios: Vec::new(),
        }
    }

    #[test]
    fn unsupported_image_capability_should_drop_all_binary_images_without_dropping_text() {
        let mut prepared = test_prepared_prompt_with_user_image_history(2, "latest text");

        assert!(drop_all_prepared_images(&mut prepared));
        assert_eq!(prepared.latest_user_text, "latest text");
        assert!(prepared.latest_images.is_empty());
        assert!(prepared
            .history_messages
            .iter()
            .all(|message| message.images.is_empty()));
    }

    #[test]
    fn unsupported_audio_capability_should_drop_binary_audio_and_continue() {
        let mut prepared = test_prepared_prompt_with_user_image_history(1, "latest text");
        prepared.latest_audios.push(PreparedBinaryPayload {
            label: "附件#1".to_string(),
            mime: "audio/webm".to_string(),
            content: B64.encode(b"audio"),
            saved_path: Some("C:/attachments/audio.webm".to_string()),
        });
        prepared.history_messages[0]
            .audios
            .push(PreparedBinaryPayload {
                label: "附件#1".to_string(),
                mime: "audio/webm".to_string(),
                content: B64.encode(b"history-audio"),
                saved_path: Some("C:/attachments/history.webm".to_string()),
            });
        let api = test_chat_api("text-only", false);

        assert!(drop_unsupported_prepared_audios(&api, &mut prepared));
        assert_eq!(prepared.latest_user_text, "latest text");
        assert!(prepared.latest_audios.is_empty());
        assert!(prepared
            .history_messages
            .iter()
            .all(|message| message.audios.is_empty()));
    }

    #[test]
    fn repeated_prepare_should_keep_canonical_path_notice_exactly_once() {
        let message = ChatMessage {
            id: "user-with-image".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Attachment {
                path: "C:/attachments/repeated.png".to_string(),
                mime: "image/png".to_string(),
                name: "repeated.png".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        let conversation = test_remote_im_conversation_with_messages(vec![message]);

        let first = latest_canonical_user_prompt_text(&conversation, DEFAULT_AGENT_ID)
            .expect("first prepare");
        let second = latest_canonical_user_prompt_text(&conversation, DEFAULT_AGENT_ID)
            .expect("second prepare");

        assert_eq!(first, second);
        assert!(second.contains("[图片#1]\npath: C:/attachments/repeated.png"));
        assert_eq!(second.matches("path: ").count(), 1);
    }

    #[test]
    fn repeated_prepare_should_not_append_legacy_attachment_path_after_canonical_projection() {
        let message = ChatMessage {
            id: "user-with-legacy-payload-image".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Attachment {
                path: "C:/attachments/legacy-repeated.png".to_string(),
                mime: "image/png".to_string(),
                name: "legacy-repeated.png".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        let conversation = test_remote_im_conversation_with_messages(vec![message]);
        let canonical_text = latest_canonical_user_prompt_text(&conversation, DEFAULT_AGENT_ID)
            .expect("canonical projection");
        let legacy_payload = ChatInputPayload {
            text: Some("test".to_string()),
            display_text: None,
            parts: None,
            images: Some(vec![BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: B64.encode(b"legacy-image"),
                saved_path: Some("downloads/legacy-repeated.png".to_string()),
            }]),
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        let legacy_paths = legacy_attachment_relative_paths_for_prompt(
            &legacy_payload,
            !canonical_text.trim().is_empty(),
        );

        assert!(legacy_paths.is_empty());
        assert!(canonical_text.contains(
            "[图片#1]\npath: C:/attachments/legacy-repeated.png"
        ));
        assert_eq!(canonical_text.matches("path: ").count(), 1);
    }

    #[test]
    fn remote_im_auto_compaction_should_skip_when_message_count_below_minimum() {
        let old_time = test_rfc3339_hours_ago(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1);
        let messages = (0..REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES - 1)
            .map(|idx| test_message_at(&format!("m-{idx}"), "user", &old_time))
            .collect::<Vec<_>>();
        let conversation = test_remote_im_conversation_with_messages(messages);

        assert!(remote_im_auto_compaction_idle_hours_if_due(&conversation, false).is_none());
    }

    #[test]
    fn remote_im_auto_compaction_should_use_latest_message_as_idle_boundary_without_new_user() {
        let old_time = test_rfc3339_hours_ago(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1);
        let recent_time = test_rfc3339_hours_ago(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS - 1);
        let mut messages = (0..REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES - 1)
            .map(|idx| test_message_at(&format!("m-{idx}"), "assistant", &old_time))
            .collect::<Vec<_>>();
        messages.push(test_message_at("latest-user", "user", &recent_time));
        let conversation = test_remote_im_conversation_with_messages(messages);

        assert!(remote_im_auto_compaction_idle_hours_if_due(&conversation, false).is_none());
    }

    #[test]
    fn remote_im_auto_compaction_should_ignore_new_trailing_user_messages() {
        let old_time = test_rfc3339_hours_ago(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1);
        let recent_time = test_rfc3339_hours_ago(0);
        let mut messages = (0..REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES - 1)
            .map(|idx| test_message_at(&format!("m-{idx}"), "assistant", &old_time))
            .collect::<Vec<_>>();
        messages.push(test_message_at("current-user", "user", &recent_time));
        let conversation = test_remote_im_conversation_with_messages(messages);

        assert_eq!(
            remote_im_auto_compaction_idle_hours_if_due(&conversation, true),
            Some(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1)
        );
    }

    #[test]
    fn remote_im_auto_compaction_should_not_ignore_trailing_compaction_message() {
        let old_time = test_rfc3339_hours_ago(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1);
        let recent_time = test_rfc3339_hours_ago(0);
        let mut messages = (0..REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES - 1)
            .map(|idx| test_message_at(&format!("m-{idx}"), "assistant", &old_time))
            .collect::<Vec<_>>();
        messages.push(test_compaction_message_at("latest-compaction", &recent_time));
        let conversation = test_remote_im_conversation_with_messages(messages);

        assert!(remote_im_auto_compaction_idle_hours_if_due(&conversation, true).is_none());
    }

    #[test]
    fn remote_im_auto_compaction_should_be_due_after_latest_message_idle_threshold() {
        let old_time = test_rfc3339_hours_ago(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1);
        let messages = (0..REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES)
            .map(|idx| test_message_at(&format!("m-{idx}"), "user", &old_time))
            .collect::<Vec<_>>();
        let conversation = test_remote_im_conversation_with_messages(messages);

        assert_eq!(
            remote_im_auto_compaction_idle_hours_if_due(&conversation, false),
            Some(REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS + 1)
        );
    }

    #[test]
    fn recent_user_image_fallback_plan_should_count_latest_user_first() {
        let prepared = test_prepared_prompt_with_user_image_history(8, "latest");

        let (history_window, latest_in_window) = recent_user_image_fallback_plan(&prepared);

        assert!(latest_in_window);
        assert_eq!(
            history_window,
            vec![false, false, true, true, true, true, true, true]
        );
    }

    #[test]
    fn recent_user_image_fallback_plan_should_use_last_seven_history_users_without_latest() {
        let prepared = test_prepared_prompt_with_user_image_history(8, "");

        let (history_window, latest_in_window) = recent_user_image_fallback_plan(&prepared);

        assert!(!latest_in_window);
        assert_eq!(
            history_window,
            vec![false, true, true, true, true, true, true, true]
        );
    }

    #[test]
    fn prepend_required_chat_api_id_should_move_id_to_front() {
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), test_chat_api("vision-b", true)],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string(), "vision-b".to_string()];

        prepend_required_chat_api_id(Some("vision-b"), &mut candidate_api_ids, &app_config)
            .expect("prepend required api id");

        assert_eq!(
            candidate_api_ids,
            vec!["vision-b".to_string(), "text-a".to_string()]
        );
    }

    #[test]
    fn prepend_required_chat_api_id_should_insert_model_not_in_agent_list() {
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), test_chat_api("vision-b", true)],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string()];

        prepend_required_chat_api_id(Some("vision-b"), &mut candidate_api_ids, &app_config)
            .expect("prepend required api id");

        assert_eq!(
            candidate_api_ids,
            vec!["vision-b".to_string(), "text-a".to_string()]
        );
    }

    #[test]
    fn prepend_optional_preferred_chat_api_id_should_keep_conversation_choice_first_and_dedupe() {
        let app_config = AppConfig {
            api_configs: vec![
                test_chat_api("api-a", false),
                test_chat_api("api-b", false),
                test_chat_api("api-c", false),
            ],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec![
            "api-a".to_string(),
            "api-b".to_string(),
            "api-c".to_string(),
        ];

        let applied = prepend_optional_preferred_chat_api_id(Some("api-c"), &mut candidate_api_ids, &app_config)
            .expect("prioritize conversation model");

        assert!(applied);
        assert_eq!(
            candidate_api_ids,
            vec!["api-c".to_string(), "api-a".to_string(), "api-b".to_string()]
        );
    }

    #[test]
    fn prepend_required_chat_api_id_should_reject_non_chat_or_missing_model() {
        let mut embedding_api = test_chat_api("embed-a", false);
        embedding_api.request_format = RequestFormat::OpenAIEmbedding;
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), embedding_api],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string()];

        let non_chat_err =
            prepend_required_chat_api_id(Some("embed-a"), &mut candidate_api_ids, &app_config)
                .expect_err("non-chat model should be rejected");
        let missing_err =
            prepend_required_chat_api_id(Some("missing"), &mut candidate_api_ids, &app_config)
                .expect_err("missing model should be rejected");

        assert_eq!(candidate_api_ids, vec!["text-a".to_string()]);
        assert!(non_chat_err.contains("不是可用聊天文本模型"));
        assert!(missing_err.contains("模型不存在"));
    }

    #[test]
    fn prepend_optional_preferred_chat_api_id_should_skip_stale_model() {
        let mut disabled_api = test_chat_api("disabled-a", false);
        disabled_api.enable_text = false;
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), disabled_api],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string()];

        let missing_applied =
            prepend_optional_preferred_chat_api_id(Some("missing"), &mut candidate_api_ids, &app_config)
                .expect("missing preferred model should be skipped");
        let disabled_applied =
            prepend_optional_preferred_chat_api_id(Some("disabled-a"), &mut candidate_api_ids, &app_config)
                .expect("disabled preferred model should be skipped");

        assert!(!missing_applied);
        assert!(!disabled_applied);
        assert_eq!(candidate_api_ids, vec!["text-a".to_string()]);
    }

    #[test]
    fn agent_primary_chat_api_config_id_should_resolve_role_for_scheduling() {
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("api-expert", false)],
            api_providers: Vec::new(),
            expert_api_config_id: "api-expert".to_string(),
            ..AppConfig::default()
        };
        let agent = test_agent_with_models(vec![MODEL_ROLE_EXPERT_API_CONFIG_ID], false);

        assert_eq!(
            agent_primary_api_config_id(&agent),
            MODEL_ROLE_EXPERT_API_CONFIG_ID.to_string()
        );
        assert_eq!(
            agent_primary_chat_api_config_id(&app_config, &agent).as_deref(),
            Some("api-expert")
        );

        let (candidate_api_ids, preferred_applied) =
            build_chat_candidate_api_ids(&app_config, &agent, None, None)
                .expect("build candidates");

        assert!(!preferred_applied);
        assert_eq!(candidate_api_ids, vec!["api-expert".to_string()]);
    }

    #[test]
    fn build_chat_candidate_api_ids_should_keep_only_required_model_when_fallback_disabled() {
        let app_config = AppConfig {
            api_configs: vec![
                test_chat_api("api-a", false),
                test_chat_api("api-b", false),
                test_chat_api("api-c", false),
            ],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let agent = test_agent_with_models(vec!["api-a", "api-b"], false);

        let (candidate_api_ids, preferred_applied) = build_chat_candidate_api_ids(
            &app_config,
            &agent,
            Some("api-c"),
            None,
        )
        .expect("build candidates");

        assert!(!preferred_applied);
        assert_eq!(candidate_api_ids, vec!["api-c".to_string()]);
    }

    #[test]
    fn build_chat_candidate_api_ids_should_keep_only_conversation_model_when_fallback_disabled() {
        let app_config = AppConfig {
            api_configs: vec![
                test_chat_api("api-a", false),
                test_chat_api("api-b", false),
                test_chat_api("api-c", false),
            ],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let agent = test_agent_with_models(vec!["api-a", "api-b"], false);

        let (candidate_api_ids, preferred_applied) = build_chat_candidate_api_ids(
            &app_config,
            &agent,
            None,
            Some("api-c"),
        )
        .expect("build candidates");

        assert!(preferred_applied);
        assert_eq!(candidate_api_ids, vec!["api-c".to_string()]);
    }

    #[test]
    fn build_chat_candidate_api_ids_should_keep_only_preferred_model_when_fallback_disabled() {
        let app_config = AppConfig {
            api_configs: vec![
                test_chat_api("api-a", false),
                test_chat_api("api-b", false),
                test_chat_api("api-c", false),
            ],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let agent = test_agent_with_models(vec!["api-a", "api-b"], true);

        let (candidate_api_ids, preferred_applied) = build_chat_candidate_api_ids(
            &app_config,
            &agent,
            None,
            Some("api-c"),
        )
        .expect("build candidates");

        assert!(preferred_applied);
        assert_eq!(candidate_api_ids, vec!["api-c".to_string()]);
    }

    #[test]
    fn build_chat_candidate_api_ids_should_not_fallback_for_private_workspace_agent() {
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("api-a", false), test_chat_api("api-b", false)],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut agent = test_agent_with_models(vec!["api-a", "api-b"], true);
        agent.source = default_private_workspace_source();

        let (candidate_api_ids, preferred_applied) = build_chat_candidate_api_ids(
            &app_config,
            &agent,
            None,
            None,
        )
        .expect("build candidates");

        assert!(!preferred_applied);
        assert_eq!(candidate_api_ids, vec!["api-a".to_string()]);
    }

    #[test]
    fn auto_title_probe_result_should_parse_json_title_only_when_has_topic() {
        assert_eq!(
            parse_auto_conversation_title_probe_result(
                r#"{"has_topic":true,"title":" 自动压缩标题。 "}"#,
            )
            .as_deref(),
            Some("自动压缩标题")
        );
        assert!(parse_auto_conversation_title_probe_result(
            r#"{"has_topic":false,"title":"自动压缩标题"}"#,
        )
        .is_none());
    }

    #[test]
    fn sync_codex_conversation_request_key_should_use_stable_conversation_id() {
        let mut resolved_api = ResolvedApiConfig {
            provider_id: Some("codex-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::Codex,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: vec![("Session-Id".to_string(), "random-uuid".to_string())],
            codex_auth: None,
            codex_custom_api_key: None,
        };

        sync_codex_conversation_request_key(&mut resolved_api, "conversation-123");

        assert_eq!(
            resolved_api.prompt_cache_key.as_deref(),
            Some("conversation-123")
        );
        assert_eq!(
            resolved_api
                .extra_headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("session-id"))
                .map(|(_, value)| value.as_str()),
            Some("conversation-123")
        );
    }

    #[test]
    fn sync_codex_conversation_request_key_should_skip_prompt_cache_key_for_openai_compatible() {
        let mut resolved_api = ResolvedApiConfig {
            provider_id: Some("openai-compatible-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://example.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
            reasoning_effort: None,
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        sync_codex_conversation_request_key(&mut resolved_api, "conversation-123");

        assert_eq!(resolved_api.prompt_cache_key, None);
        assert!(resolved_api.extra_headers.is_empty());
    }

    #[test]
    fn auto_title_schedule_should_require_no_visible_title_and_user_text_length_range() {
        let mut conversation = build_conversation_record(
            "api-a",
            "agent-a",
            "",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );

        assert!(!should_schedule_conversation_auto_title_generation(
            &conversation,
            "太短"
        ));
        assert!(should_schedule_conversation_auto_title_generation(
            &conversation,
            "请帮我检查标题自动生成逻辑"
        ));
        assert!(!should_schedule_conversation_auto_title_generation(
            &conversation,
            &"过长".repeat(51)
        ));

        conversation.title = "用户标题".to_string();
        assert!(!should_schedule_conversation_auto_title_generation(
            &conversation,
            "请帮我检查标题自动生成逻辑"
        ));
    }

    #[test]
    fn auto_title_schedule_should_skip_when_summary_title_exists() {
        let mut conversation = build_conversation_record(
            "api-a",
            "agent-a",
            "",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation
            .messages
            .push(build_initial_summary_context_message(None, Some("已有摘要标题")));

        assert!(!should_schedule_conversation_auto_title_generation(
            &conversation,
            "请帮我检查标题自动生成逻辑"
        ));
    }

    #[test]
    fn auto_title_schedule_should_ignore_branch_source_summary_title() {
        let mut conversation = build_conversation_record(
            "api-a",
            "agent-a",
            "",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation
            .messages
            .push(build_initial_summary_context_message(None, Some("来自原会话的分支")));
        assert!(conversation_update_latest_summary_title_with_source(
            &mut conversation,
            Some("来自原会话的分支"),
            Some(SUMMARY_CONTEXT_TITLE_SOURCE_BRANCH),
        ));

        assert!(should_schedule_conversation_auto_title_generation(
            &conversation,
            "请帮我检查标题自动生成逻辑"
        ));
    }

    fn test_model_reply(
        assistant_text: &str,
        final_response_text: &str,
        activity_reasoning_text: &str,
    ) -> ModelReply {
        ModelReply {
            assistant_text: assistant_text.to_string(),
            final_response_text: final_response_text.to_string(),
            activity_reasoning_text: activity_reasoning_text.to_string(),
            assistant_provider_meta: None,
            tool_history_events: Vec::new(),
            suppress_assistant_message: false,
            trusted_input_tokens: None,
            usage: None,
            round_logs_recorded_internally: false,
        }
    }

    #[test]
    fn conversation_upsert_final_assistant_message_should_update_last_same_assistant() {
        let now = now_iso();
        let mut conversation = Conversation {
            id: "conversation-a".to_string(),
            title: "测试".to_string(),
            agent_id: "agent-a".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: "assistant-existing".to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some("agent-a".to_string()),
                parts: vec![MessagePart::Text {
                    text: String::new(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: Some(vec![
                    serde_json::json!({
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [{
                            "id": "call-1",
                            "type": "function",
                            "function": {
                                "name": "read_file",
                                "arguments": "{}"
                            }
                        }]
                    }),
                    serde_json::json!({
                        "role": "tool",
                        "tool_call_id": "call-1",
                        "content": "ok"
                    }),
                ]),
                mcp_call: None,
            meme_annotations: None,
            }],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        };
        let final_message = build_assistant_message_from_request_sequence(
            "assistant-existing".to_string(),
            "agent-a",
            now.clone(),
            &[
                serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call-1",
                    "content": "ok"
                }),
                serde_json::json!({
                    "role": "assistant",
                    "content": "最终回答"
                }),
            ],
            Some(serde_json::json!({
                "effectivePromptTokens": 128_u64
            })),
        );

        let persisted = conversation_upsert_final_assistant_message(
            &mut conversation,
            "agent-a",
            final_message,
            &now,
        )
        .expect("upsert by assistant_message_id should succeed");

        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(persisted.id, "assistant-existing");
        assert_eq!(
            conversation.messages[0]
                .parts
                .first()
                .and_then(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                }),
            Some("最终回答")
        );
        assert_eq!(
            conversation.messages[0]
                .tool_call
                .as_ref()
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            conversation.messages[0]
                .provider_meta
                .as_ref()
                .and_then(|meta| meta.get("effectivePromptTokens"))
                .and_then(Value::as_u64),
            Some(128)
        );
    }

    #[test]
    fn model_reply_content_state_should_classify_empty_reply_variants() {
        assert_eq!(
            model_reply_content_state(&test_model_reply("", "", "")),
            ModelReplyContentState::Empty
        );
        assert_eq!(
            model_reply_content_state(&test_model_reply("", "", "只有思维链，没有最终回答")),
            ModelReplyContentState::ReasoningOnly
        );
        assert_eq!(
            model_reply_content_state(&test_model_reply("正文", "", "思维链")),
            ModelReplyContentState::Visible
        );
    }

    #[test]
    fn model_reply_content_state_should_keep_provider_meta_visible() {
        let mut reply = test_model_reply("", "", "只有思维链");
        reply.assistant_provider_meta = Some(serde_json::json!({
            "messageKind": "plan_present"
        }));

        assert_eq!(
            model_reply_content_state(&reply),
            ModelReplyContentState::Visible
        );
    }

    #[test]
    fn should_create_assistant_provider_meta_should_preserve_deepseek_meta_container() {
        assert!(should_create_assistant_provider_meta(
            &RequestFormat::DeepSeek,
            None,
            None,
            0,
            false,
        ));
        assert!(!should_create_assistant_provider_meta(
            &RequestFormat::OpenAI,
            None,
            None,
            0,
            false,
        ));
    }

    #[test]
    fn build_effective_prompt_media_from_prepared_should_reuse_normalized_images() {
        let api = test_chat_api("vision-a", true);
        let payload = ChatInputPayload {
            text: Some("看这张图".to_string()),
            display_text: None,
            parts: None,
            images: Some(vec![BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: B64.encode(b"source-png"),
                saved_path: Some("downloads/source.png".to_string()),
            }]),
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };
        let prepared_images = vec![PreparedBinaryPayload {
            label: "图片#1".to_string(),
            mime: "image/webp".to_string(),
            content: B64.encode(b"normalized-webp"),
            saved_path: None,
        }];

        let (latest_user_text, images, audios) = build_effective_prompt_media_from_prepared(
            &payload,
            &api,
            &prepared_images,
            &[],
        )
        .expect("reuse prepared image payload");

        assert_eq!(latest_user_text, "看这张图\n[image]");
        assert!(audios.is_empty());
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime, "image/webp");
        assert_eq!(images[0].content, B64.encode(b"normalized-webp"));
        assert_eq!(images[0].saved_path.as_deref(), Some("downloads/source.png"));
    }

    #[test]
    fn ordered_parts_should_forward_all_prepared_media() {
        let api = test_chat_api("vision-a", true);
        let payload = ChatInputPayload {
            text: None,
            display_text: None,
            parts: Some(vec![
                ChatIngressPart::Text {
                    text: "看图".to_string(),
                },
                ChatIngressPart::Attachment {
                    path: Some("C:/attachments/image.png".to_string()),
                    bytes_base64: None,
                    mime: "image/png".to_string(),
                    name: "image.png".to_string(),
                },
            ]),
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };
        let prepared_images = vec![PreparedBinaryPayload {
            label: "图片#1".to_string(),
            mime: "image/webp".to_string(),
            content: B64.encode(b"normalized-webp"),
            saved_path: Some("C:/attachments/image.png".to_string()),
        }];

        let (text, images, audios) = build_effective_prompt_media_from_prepared(
            &payload,
            &api,
            &prepared_images,
            &[],
        )
        .expect("ordered parts media");

        assert_eq!(text, "看图\n[image]");
        assert_eq!(images.len(), 1);
        assert!(audios.is_empty());
    }

    #[test]
    fn ordered_attachment_only_should_never_fail_when_binary_is_unavailable() {
        let api = test_chat_api("vision-a", true);
        let payload = ChatInputPayload {
            text: None,
            display_text: None,
            parts: Some(vec![ChatIngressPart::Attachment {
                path: Some("C:/attachments/missing.png".to_string()),
                bytes_base64: None,
                mime: "image/png".to_string(),
                name: "missing.png".to_string(),
            }]),
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        let (text, images, audios) =
            build_effective_prompt_media_from_prepared(&payload, &api, &[], &[])
                .expect("attachment-only should degrade");

        assert_eq!(text, "[image]");
        assert!(images.is_empty());
        assert!(audios.is_empty());
    }

    #[test]
    fn invalid_image_binary_should_be_skipped_instead_of_reaching_provider() {
        let prepared = prepared_image_payload_for_llm_request(
            "image/png".to_string(),
            B64.encode(b"not-an-image"),
            Some("C:/attachments/broken.png".to_string()),
            Some("图片#1".to_string()),
        );

        assert!(prepared.is_none());
    }

    #[test]
    fn pdf_binary_should_remain_available_without_image_normalization() {
        let prepared = prepared_image_payload_for_llm_request(
            "application/pdf".to_string(),
            B64.encode(b"%PDF-1.7"),
            Some("C:/attachments/document.pdf".to_string()),
            Some("附件#1".to_string()),
        )
        .expect("pdf payload");

        assert_eq!(prepared.mime, "application/pdf");
        assert_eq!(prepared.label, "附件#1");
    }

    #[test]
    fn prepared_binary_labels_should_follow_original_part_order_with_independent_counters() {
        let root = std::env::temp_dir().join(format!("eca-prepared-labels-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let pdf_path = root.join("document.pdf");
        let image_path = root.join("image.png");
        let text_path = root.join("notes.txt");
        let audio_path = root.join("voice.webm");
        std::fs::write(&pdf_path, b"%PDF-1.7").expect("write pdf");
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([12, 34, 56, 255]),
        ));
        let mut image_cursor = std::io::Cursor::new(Vec::<u8>::new());
        image
            .write_to(&mut image_cursor, image::ImageFormat::Png)
            .expect("encode png");
        std::fs::write(&image_path, image_cursor.into_inner()).expect("write image");
        std::fs::write(&text_path, b"notes").expect("write text");
        std::fs::write(&audio_path, b"audio").expect("write audio");
        let parts = vec![
            MessagePart::Attachment {
                path: pdf_path.to_string_lossy().to_string(),
                mime: "application/pdf".to_string(),
                name: "document.pdf".to_string(),
            },
            MessagePart::Attachment {
                path: image_path.to_string_lossy().to_string(),
                mime: "image/png".to_string(),
                name: "image.png".to_string(),
            },
            MessagePart::Attachment {
                path: text_path.to_string_lossy().to_string(),
                mime: "text/plain".to_string(),
                name: "notes.txt".to_string(),
            },
            MessagePart::Attachment {
                path: audio_path.to_string_lossy().to_string(),
                mime: "audio/webm".to_string(),
                name: "voice.webm".to_string(),
            },
        ];

        let (images, audios) =
            build_prepared_binary_payloads_from_message_parts(&parts, &[], &[]);

        // PDF 不随请求发送二进制，仅保留路径提示；附件编号仍按出现顺序占用。
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].label, "图片#1");
        assert_eq!(audios.len(), 1);
        assert_eq!(audios[0].label, "附件#3");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn collect_payload_attachment_relative_paths_should_use_attachment_meta_as_authority() {
        let payload = ChatInputPayload {
            text: Some("test".to_string()),
            display_text: None,
            parts: None,
            images: Some(vec![BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: B64.encode(b"img"),
                saved_path: Some("downloads/source.png".to_string()),
            }]),
            audios: None,
            attachments: Some(vec![
                AttachmentMetaInput {
                    file_name: "report.pdf".to_string(),
                    path: "downloads/report.pdf".to_string(),
                    mime: "application/pdf".to_string(),
                },
                AttachmentMetaInput {
                    file_name: "source.png".to_string(),
                    path: "downloads/source.png".to_string(),
                    mime: "image/png".to_string(),
                },
            ]),
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        assert_eq!(
            collect_payload_attachment_relative_paths(&payload),
            vec![
                "downloads/report.pdf".to_string(),
                "downloads/source.png".to_string()
            ]
        );
    }

    #[test]
    fn collect_payload_attachment_meta_entries_should_store_each_attachment_once() {
        let payload = ChatInputPayload {
            text: Some("test".to_string()),
            display_text: None,
            parts: None,
            images: Some(vec![BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: B64.encode(b"img"),
                saved_path: Some("downloads/source.png".to_string()),
            }]),
            audios: None,
            attachments: Some(vec![
                AttachmentMetaInput {
                    file_name: "source.png".to_string(),
                    path: "downloads/source.png".to_string(),
                    mime: "image/png".to_string(),
                },
                AttachmentMetaInput {
                    file_name: "source-copy.png".to_string(),
                    path: "downloads/source.png".to_string(),
                    mime: "image/png".to_string(),
                },
            ]),
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        let entries = collect_payload_attachment_meta_entries(&payload);

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].get("relativePath").and_then(Value::as_str),
            Some("downloads/source.png")
        );
    }

    #[test]
    fn provider_meta_attachment_relative_paths_should_ignore_legacy_duplicates() {
        let meta = serde_json::json!({
            "attachments": [
                {
                    "fileName": "source.png",
                    "relativePath": "downloads/source.png",
                    "mime": "image/png"
                },
                {
                    "fileName": "source-copy.png",
                    "relativePath": "downloads/source.png",
                    "mime": "image/png"
                }
            ]
        });

        assert_eq!(
            provider_meta_attachment_relative_paths(&meta),
            vec!["downloads/source.png".to_string()]
        );
    }

    #[test]
    fn image_attachment_reference_label_should_match_attachment_index() {
        let image = BinaryPart {
            mime: "image/png".to_string(),
            bytes_base64: B64.encode(b"img"),
            saved_path: Some("downloads/source.png".to_string()),
        };
        let payload = ChatInputPayload {
            text: Some("test".to_string()),
            display_text: None,
            parts: None,
            images: Some(vec![image.clone()]),
            audios: None,
            attachments: Some(vec![
                AttachmentMetaInput {
                    file_name: "report.pdf".to_string(),
                    path: "downloads/report.pdf".to_string(),
                    mime: "application/pdf".to_string(),
                },
                AttachmentMetaInput {
                    file_name: "source.png".to_string(),
                    path: "downloads/source.png".to_string(),
                    mime: "image/png".to_string(),
                },
            ]),
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        assert_eq!(
            image_attachment_reference_label(&payload, &image, 0),
            "附件#2"
        );
        assert_eq!(
            image_description_block("附件#2", "识别结果"),
            "[附件#2 图片转文]\n识别结果"
        );
    }

    #[test]
    fn collect_payload_attachment_relative_paths_should_fallback_to_image_media_path() {
        let payload = ChatInputPayload {
            text: Some("test".to_string()),
            display_text: None,
            parts: None,
            images: Some(vec![BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: B64.encode(b"img"),
                saved_path: Some("downloads/source.png".to_string()),
            }]),
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        assert_eq!(
            collect_payload_attachment_relative_paths(&payload),
            vec!["downloads/source.png".to_string()]
        );
    }

    #[test]
    fn collect_payload_attachment_relative_paths_should_include_audio_media_too() {
        let payload = ChatInputPayload {
            text: Some("test".to_string()),
            display_text: None,
            parts: None,
            images: None,
            audios: Some(vec![BinaryPart {
                mime: "audio/mp3".to_string(),
                bytes_base64: B64.encode(b"audio"),
                saved_path: Some("downloads/voice.mp3".to_string()),
            }]),
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        assert_eq!(
            collect_payload_attachment_relative_paths(&payload),
            vec!["downloads/voice.mp3".to_string()]
        );
    }

    #[test]
    fn retry_state_should_classify_rate_limited_and_non_retryable() {
        assert!(is_rate_limited_error("HTTP 429 rate limit"));
        assert!(is_rate_limited_error("429 Too Many Requests"));
        assert!(is_rate_limited_error("quota exceeded, please retry"));
        assert!(is_rate_limited_error("rate_limit_exceeded"));
        assert!(is_rate_limited_error("Rate Limit hit: 429"));
        assert!(!is_rate_limited_error("404 Not Found"));
        assert!(!is_rate_limited_error("401 Unauthorized"));
        assert!(!is_rate_limited_error("model not found"));
        assert!(!is_rate_limited_error("500 internal error"));
        // 429 子串边界：1429 不应误判
        assert!(!is_rate_limited_error("error code 1429"));
        assert!(!is_rate_limited_error(""));

        let empty_reply: Result<ModelReply, String> = Ok(test_model_reply("", "", ""));
        assert_eq!(
            classify_retry_kind_for_result(&empty_reply),
            RetryKind::Empty
        );
        let reasoning_only: Result<ModelReply, String> =
            Ok(test_model_reply("", "", "只有思维链"));
        assert_eq!(
            classify_retry_kind_for_result(&reasoning_only),
            RetryKind::Empty
        );
        let visible: Result<ModelReply, String> = Ok(test_model_reply("正文", "", ""));
        assert_eq!(
            classify_retry_kind_for_result(&visible),
            RetryKind::NonRetryable
        );
        let rate_err: Result<ModelReply, String> = Err("429 rate limit".to_string());
        assert_eq!(
            classify_retry_kind_for_result(&rate_err),
            RetryKind::RateLimited
        );
        let not_found: Result<ModelReply, String> = Err("404 Not Found".to_string());
        assert_eq!(
            classify_retry_kind_for_result(&not_found),
            RetryKind::NonRetryable
        );
    }

    #[test]
    fn retry_policy_should_be_independent_per_kind_and_linear() {
        let mut budget = RetryBudget::default();
        // Empty 独立 3 次
        assert_eq!(
            retry_policy(RetryKind::Empty, &budget),
            Some(std::time::Duration::from_secs(WAIT_EMPTY_SECS))
        );
        budget.empty_used = 2;
        assert_eq!(
            retry_policy(RetryKind::Empty, &budget),
            Some(std::time::Duration::from_secs(WAIT_EMPTY_SECS))
        );
        budget.empty_used = 3;
        assert_eq!(retry_policy(RetryKind::Empty, &budget), None);

        // RateLimited 独立 3 次，不受 Empty 影响
        let mut budget2 = RetryBudget {
            empty_used: 3,
            rate_used: 0,
        };
        assert_eq!(
            retry_policy(RetryKind::RateLimited, &budget2),
            Some(std::time::Duration::from_secs(WAIT_RATE_SECS))
        );
        budget2.rate_used = 3;
        assert_eq!(retry_policy(RetryKind::RateLimited, &budget2), None);

        // 线性：混合序列各看各
        let budget3 = RetryBudget {
            empty_used: 1,
            rate_used: 1,
        };
        assert_eq!(
            retry_policy(RetryKind::Empty, &budget3),
            Some(std::time::Duration::from_secs(WAIT_EMPTY_SECS))
        );
        assert_eq!(
            retry_policy(RetryKind::RateLimited, &budget3),
            Some(std::time::Duration::from_secs(WAIT_RATE_SECS))
        );

        // NonRetryable 永远不重试
        assert_eq!(
            retry_policy(RetryKind::NonRetryable, &RetryBudget::default()),
            None
        );
        assert_eq!(
            retry_policy(RetryKind::NonRetryable, &budget3),
            None
        );
    }

    #[test]
    fn retry_budget_should_reset_on_success_via_new_request_scope() {
        // 成功清零 = 请求内局部变量随栈销毁；用“新请求从0开始”来验证语义
        let budget_after_success = RetryBudget::default();
        assert_eq!(budget_after_success.empty_used, 0);
        assert_eq!(budget_after_success.rate_used, 0);
        assert_eq!(
            retry_policy(RetryKind::Empty, &budget_after_success),
            Some(std::time::Duration::from_secs(WAIT_EMPTY_SECS))
        );
        assert_eq!(
            retry_policy(RetryKind::RateLimited, &budget_after_success),
            Some(std::time::Duration::from_secs(WAIT_RATE_SECS))
        );

        // 已耗尽的预算在新请求里应重新可用
        let exhausted = RetryBudget {
            empty_used: MAX_EMPTY_RETRIES,
            rate_used: MAX_RATE_RETRIES,
        };
        assert_eq!(retry_policy(RetryKind::Empty, &exhausted), None);
        assert_eq!(retry_policy(RetryKind::RateLimited, &exhausted), None);
        let fresh = RetryBudget::default();
        assert_eq!(
            retry_policy(RetryKind::Empty, &fresh),
            Some(std::time::Duration::from_secs(WAIT_EMPTY_SECS))
        );
    }

    #[test]
    fn remote_reply_delegate_should_extract_each_visible_assistant_round() {
        let messages = vec![
            serde_json::json!({
                "role": "assistant",
                "content": "先查一下。",
                "tool_calls": [{"id": "tool-1"}]
            }),
            serde_json::json!({"role": "tool", "content": "工具结果"}),
            serde_json::json!({"role": "assistant", "content": "查到了，最终答复。"}),
        ];
        assert_eq!(
            remote_im_reply_delegate_visible_texts(&messages),
            vec!["先查一下。".to_string(), "查到了，最终答复。".to_string()]
        );
        let meta = remote_im_reply_delegate_stage_provider_meta(
            "delegate-1",
            "trigger-1",
            "intermediate_1",
        );
        assert_eq!(
            meta["remoteImReplyDelegate"]["delegateId"],
            serde_json::json!("delegate-1")
        );
        assert_eq!(
            meta["remoteImReplyDelegate"]["triggerMessageId"],
            serde_json::json!("trigger-1")
        );
    }
}