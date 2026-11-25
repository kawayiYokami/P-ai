// ==================== 定时器调度器 ====================
// 到点之后的两种去处：
//   - direct：往目标会话投一条系统引导事件，前台当场跑一轮；
//   - delegate：在目标会话（全局定时器则是系统通知会话）名下起一场独立委托。
//
// 全局定时器没有前台会话可激活，因此只能走委托——这一条在写入时就已锁定。

fn timer_scheduler_notify_changed(state: &AppState) {
    state.timer_scheduler_notify.notify_one();
}

fn timer_publish_changed_event(
    state: &AppState,
    kind: &str,
    timer_id: &str,
    timer: Option<&TimerRecord>,
) {
    // 业务只更新运行时；事件发布收敛在监控总线，payload 仅脏标记，前端按需拉快照
    let conversation_id = timer
        .and_then(|entry| entry.conversation_id.clone())
        .unwrap_or_default();
    monitor_publish_changed(
        state,
        MonitorDomain::Timer,
        kind,
        &conversation_id,
        timer_id,
    );
}

/// 目标会话是否还能收这次触发：归档、删除、委托会话都不再投递。
fn timer_conversation_available_for_dispatch(
    state: &AppState,
    conversation_id: &str,
) -> bool {
    match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(meta) => {
            meta.status.trim() != "archived"
                && meta
                    .archived_at
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                && meta.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE
        }
        Err(_) => false,
    }
}

fn timer_dispatch_agent_id(state: &AppState, conversation_id: Option<&str>) -> String {
    let from_conversation = conversation_id
        .and_then(|conversation_id| {
            conversation_service_v2()
                .get_conversation_meta(state, conversation_id)
                .ok()
        })
        .map(|meta| meta.agent_id.trim().to_string())
        .filter(|value| !value.is_empty() && value != SYSTEM_PERSONA_ID);
    from_conversation.unwrap_or_else(|| DEFAULT_AGENT_ID.to_string())
}

fn timer_build_direct_message(record: &TimerRecordStored) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "system".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text {
            text: format!("定时提醒：{}", record.message.trim()),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "messageKind": "timer_trigger",
            "timerId": record.timer_id.trim(),
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// direct：往目标会话投一条系统引导事件，前台当场激活一轮。
fn timer_dispatch_direct(
    state: &AppState,
    record: &TimerRecordStored,
    conversation_id: &str,
    model_config_id: &str,
) -> Result<ChatEventIngress, String> {
    let agent_id = timer_dispatch_agent_id(state, Some(conversation_id));
    let mut runtime_context = runtime_context_new("timer_trigger", "timer_due");
    runtime_context.request_id = Some(format!("timer-dispatch-{}", Uuid::new_v4()));
    runtime_context.dispatch_id = Some(format!("timer-trigger-{}", Uuid::new_v4()));
    runtime_context.origin_conversation_id = Some(conversation_id.to_string());
    runtime_context.target_conversation_id = Some(conversation_id.to_string());
    runtime_context.root_conversation_id = Some(conversation_id.to_string());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.model_config_id = Some(model_config_id.to_string());
    let event = ChatPendingEvent {
        id: format!("timer-event-{}", Uuid::new_v4()),
        conversation_id: conversation_id.to_string(),
        created_at: now_iso(),
        created_at_ms: now_unix_ms(),
        source: ChatEventSource::System,
        queue_mode: ChatQueueMode::Guided,
        messages: vec![timer_build_direct_message(record)],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: agent_id.clone(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    ingress_chat_event(state, event)
}

/// delegate：在目标会话名下起一场独立委托，跑完写回该会话。
fn timer_dispatch_delegate(
    state: &AppState,
    record: &TimerRecordStored,
    target_conversation_id: &str,
) -> Result<String, String> {
    let agent_id = timer_dispatch_agent_id(state, Some(target_conversation_id));
    let compact = record.message.trim().chars().take(24).collect::<String>();
    let title = if compact.trim().is_empty() {
        format!("定时器：{}", record.timer_id.trim())
    } else {
        format!("定时器：{}", compact)
    };
    let instruction = format!(
        "{}\n\n这是定时器到点触发的任务，请在独立委托里完成，完成后直接汇报结果。",
        record.message.trim()
    );
    let delegate = delegate_create_record(
        state,
        DELEGATE_TOOL_KIND_DELEGATE,
        target_conversation_id,
        None,
        &agent_id,
        &agent_id,
        &title,
        instruction.clone(),
        title.clone(),
        "完成定时器任务，并直接汇报结果。".to_string(),
        instruction,
        false,
        vec![agent_id.clone()],
    )?;
    let delegate_id = delegate.delegate_id.clone();
    let model_config_ids = runtime_agent_by_id(&load_runtime_organization_snapshot(state)?, &agent_id)
        .and_then(|agent| agent_primary_chat_api_config_id(&load_runtime_organization_snapshot(state).ok()?.config, agent))
        .into_iter()
        .collect::<Vec<_>>();
    spawn_delegate_task(
        state.clone(),
        delegate,
        target_conversation_id.to_string(),
        model_config_ids,
        None,
        None,
    );
    Ok(delegate_id)
}

async fn timer_dispatch_due(
    state: &AppState,
    record: &TimerRecordStored,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let conversation_id = record.conversation_id.clone();
    if let Some(conversation_id) = conversation_id.as_deref() {
        if !timer_conversation_available_for_dispatch(state, conversation_id) {
            // 目标会话没了：停用定时器，别让它每分钟重试一次。
            let mut disabled = record.clone();
            disabled.enabled = false;
            disabled.next_run_at_utc = None;
            timer_store_write_records(&state.data_path, &disabled)?;
            runtime_log_warn(format!(
                "[定时器] 停止 任务=目标会话不可用 定时器={} 会话={conversation_id}",
                record.timer_id
            ));
            timer_publish_changed_event(
                state,
                "updated",
                &record.timer_id,
                Some(&timer_record_view_from_stored(&disabled)),
            );
            return Ok(());
        }
    }
    let model_config_id = {
        let snapshot = load_runtime_organization_snapshot(state)?;
        let agent_id = timer_dispatch_agent_id(state, conversation_id.as_deref());
        runtime_agent_by_id(&snapshot, &agent_id)
            .and_then(|agent| agent_primary_chat_api_config_id(&snapshot.config, agent))
            .ok_or_else(|| format!("定时器 {}= 没有可用模型，人格={agent_id}", record.timer_id))?
    };

    match (record.dispatch_mode, conversation_id.as_deref()) {
        (TimerDispatchMode::Direct, Some(conversation_id)) => {
            let ingress = timer_dispatch_direct(state, record, conversation_id, &model_config_id)?;
            let updated = timer_store_mark_triggered(&state.data_path, record)?;
            trigger_chat_event_after_ingress(state, ingress);
            timer_publish_changed_event(
                state,
                "triggered",
                &record.timer_id,
                Some(&timer_record_view_from_stored(&updated)),
            );
        }
        (_, Some(conversation_id)) => {
            let delegate_id = timer_dispatch_delegate(state, record, conversation_id)?;
            let updated = timer_store_mark_triggered(&state.data_path, record)?;
            runtime_log_info(format!(
                "[定时器] 完成 任务=到点发起委托 定时器={} 会话={conversation_id} 委托={delegate_id}",
                record.timer_id
            ));
            timer_publish_changed_event(
                state,
                "triggered",
                &record.timer_id,
                Some(&timer_record_view_from_stored(&updated)),
            );
        }
        (_, None) => {
            // 全局定时器：在系统通知会话里起委托。
            conversation_service_v2().ensure_system_notification_conversation(state)?;
            let delegate_id =
                timer_dispatch_delegate(state, record, SYSTEM_NOTIFICATION_CONVERSATION_ID)?;
            let updated = timer_store_mark_triggered(&state.data_path, record)?;
            runtime_log_info(format!(
                "[定时器] 完成 任务=全局定时器发起委托 定时器={} 委托={delegate_id}",
                record.timer_id
            ));
            timer_publish_changed_event(
                state,
                "triggered",
                &record.timer_id,
                Some(&timer_record_view_from_stored(&updated)),
            );
        }
    }
    runtime_log_debug(format!(
        "[定时器] 触发 定时器={} 模式={} 耗时={}ms",
        record.timer_id,
        record.dispatch_mode.as_str(),
        started_at.elapsed().as_millis()
    ));
    Ok(())
}

async fn timer_scheduler_tick(state: &AppState) -> Result<(), String> {
    let records = timer_store_list_records(&state.data_path)?;
    let now = now_utc();
    for record in records {
        if !record.enabled {
            continue;
        }
        // 缺 next_run_at 的行（迁移来的老数据）先补算，再判到点。
        let record = if record.next_run_at_utc.is_none() {
            match timer_store_recompute_next_run(&state.data_path, &record) {
                Ok(updated) => updated,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[定时器] 跳过 任务=补算下一次触发 定时器={} 原因={err}",
                        record.timer_id
                    ));
                    continue;
                }
            }
        } else {
            record
        };
        if !timer_is_due(&record, now) {
            continue;
        }
        if let Err(err) = timer_dispatch_due(state, &record).await {
            runtime_log_error(format!(
                "[定时器] 失败 任务=到点触发 定时器={} 原因={err}",
                record.timer_id
            ));
            // 失败行不能停在过去：推到退避窗口之后，否则下一拍又是同一个到点判定。
            match timer_store_mark_dispatch_failure(&state.data_path, &record) {
                Ok(updated) => timer_publish_changed_event(
                    state,
                    "updated",
                    &record.timer_id,
                    Some(&timer_record_view_from_stored(&updated)),
                ),
                Err(backoff_err) => runtime_log_warn(format!(
                    "[定时器] 跳过 任务=派发失败退避 定时器={} 原因={backoff_err}",
                    record.timer_id
                )),
            }
        }
    }
    Ok(())
}

fn timer_scheduler_next_wake_delay(state: &AppState) -> Result<Option<std::time::Duration>, String> {
    let records = timer_store_list_records(&state.data_path)?;
    let now = now_utc();
    let mut next_future_due = None::<OffsetDateTime>;
    for record in records.iter().filter(|record| record.enabled) {
        let Some(next_run_at) = record
            .next_run_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
        else {
            continue;
        };
        if next_run_at <= now {
            return Ok(Some(std::time::Duration::ZERO));
        }
        next_future_due = Some(
            next_future_due
                .map(|current| current.min(next_run_at))
                .unwrap_or(next_run_at),
        );
    }
    let Some(next_due) = next_future_due else {
        return Ok(None);
    };
    let millis = (next_due - now).whole_milliseconds();
    let millis = u64::try_from(millis).unwrap_or(u64::MAX);
    Ok(Some(std::time::Duration::from_millis(millis)))
}

/// 把「下一次到点还有多久」收敛成一次真正要等的时长。
///
/// 两个硬约束：不超过兜底复查间隔，不低于 `TIMER_SCHEDULER_MIN_WAIT_MILLIS`。
/// 后者是为了杜绝零间隔自旋——到点判定与派发之间任何一次失败都会让下一拍的等待时长回到 0，
/// 没有下限的话整条调度循环会把一个核吃满。
fn timer_scheduler_clamp_wait(delay: Option<std::time::Duration>) -> std::time::Duration {
    let fallback = std::time::Duration::from_secs(TIMER_SCHEDULER_FALLBACK_SECONDS);
    let wait_duration = delay.map(|delay| delay.min(fallback)).unwrap_or(fallback);
    wait_duration.max(std::time::Duration::from_millis(TIMER_SCHEDULER_MIN_WAIT_MILLIS))
}

async fn timer_scheduler_wait(state: &AppState) -> Result<(), String> {
    let next_delay = timer_scheduler_next_wake_delay(state)?;
    let wait_duration = timer_scheduler_clamp_wait(next_delay);
    let sleep = tokio::time::sleep(wait_duration);
    tokio::pin!(sleep);
    tokio::select! {
        _ = &mut sleep => {}
        _ = state.timer_scheduler_notify.notified() => {}
    }
    Ok(())
}

fn start_timer_scheduler(state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            let tick_started_at = std::time::Instant::now();
            if let Err(err) = timer_scheduler_tick(&state).await {
                runtime_log_error(format!(
                    "[定时器] 调度轮询失败 错误={} 耗时={}ms",
                    err,
                    tick_started_at.elapsed().as_millis()
                ));
            }
            if let Err(err) = timer_scheduler_wait(&state).await {
                runtime_log_error(format!(
                    "[定时器] 等待下一次触发失败 错误={} 耗时={}ms",
                    err,
                    tick_started_at.elapsed().as_millis()
                ));
                tokio::time::sleep(std::time::Duration::from_secs(
                    TIMER_SCHEDULER_FALLBACK_SECONDS,
                ))
                .await;
            }
        }
    });
}
