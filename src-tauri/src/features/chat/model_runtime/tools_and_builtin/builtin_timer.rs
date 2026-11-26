// ==================== 定时器工具 ====================
// 定时器只回答一件事：什么时候、在哪个会话、把哪句话再递进去。
// 没有标题、进度、待办、结论文档——那些都是任务式账本，已经随旧 task 体系一起退场。

/// 委托线程里禁止增删定时器：定时器到点会起委托，委托里再建定时器就是自激。
fn timer_tool_reject_delegate_thread(
    app_state: &AppState,
    session_id: &str,
) -> Result<(), String> {
    let Some(conversation_id) = delegate_session_conversation_id(session_id) else {
        return Ok(());
    };
    let delegate_conversation = conversation_service_v2()
        .get_conversation_meta(app_state, &conversation_id)
        .ok()
        .map(|conversation_meta| {
            conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE
        })
        .or_else(|| {
            delegate_runtime_thread_conversation_get(app_state, &conversation_id)
                .ok()
                .flatten()
                .map(|conversation| conversation_is_delegate(&conversation))
        })
        .unwrap_or(false);
    if delegate_conversation {
        return Err("委托线程中禁止增删定时器，避免定时器与委托互相触发。".to_string());
    }
    Ok(())
}

/// 定时器工具只能管「本会话」：入参指定了具体会话，就必须是当前会话；
/// 传别的会话一律拒绝——否则一段 prompt 就能把定时任务塞进别人的会话。
///
/// 空串/未传＝全局定时器（没有具体会话），不受这条约束。
fn timer_tool_assert_conversation_in_scope(
    session_id: &str,
    requested_conversation_id: Option<&str>,
) -> Result<(), String> {
    let Some(requested) = requested_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    if delegate_session_conversation_id(session_id).as_deref() == Some(requested) {
        return Ok(());
    }
    Err(format!(
        "定时器只能挂在当前会话或全局，禁止对其它会话操作（请求会话={requested}）"
    ))
}

/// 已有的定时器也只能由它所属的会话来改：短号是连号，猜一个号就能动别人的定时器。
fn timer_tool_assert_timer_in_scope(
    state: &AppState,
    session_id: &str,
    timer_id: &str,
) -> Result<(), String> {
    let record = timer_store_get_record(&state.data_path, timer_id)?;
    timer_tool_assert_conversation_in_scope(session_id, record.conversation_id.as_deref())
}

/// 工具只该看见自己能管的那批：本会话的，加上没有会话归属的全局定时器。
/// 全量返回会把别的会话的定时器也倒给模型，还会给出一堆它无权修改的条目。
fn timer_tool_visible_timers(
    timers: Vec<TimerRecord>,
    scope_conversation_id: Option<&str>,
) -> Vec<TimerRecord> {
    let scope = scope_conversation_id.map(str::trim).filter(|id| !id.is_empty());
    timers
        .into_iter()
        .filter(|timer| {
            let target = timer
                .conversation_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            target.is_empty() || Some(target) == scope
        })
        .collect()
}

async fn builtin_timer(
    app_state: &AppState,
    session_id: &str,
    args: TimerToolArgs,
) -> Result<Value, String> {
    let bound_conversation_id = delegate_session_conversation_id(session_id);
    match args.action.trim() {
        "list" => {
            let timers = timer_store_list(&app_state.data_path)?;
            let visible = timer_tool_visible_timers(timers, bound_conversation_id.as_deref());
            serde_json::to_value(visible).map_err(|err| format!("序列化定时器列表失败：{err}"))
        }
        "create" => {
            timer_tool_reject_delegate_thread(app_state, session_id)?;
            timer_tool_assert_conversation_in_scope(session_id, args.conversation_id.as_deref())?;
            // 显式给了 conversationId 就按它来（空串＝全局定时器）；没给才落在当前会话。
            let conversation_id = match args.conversation_id.as_deref() {
                Some(value) => Some(value.to_string()),
                None => bound_conversation_id,
            };
            let input = TimerCreateInput {
                message: args.message.clone().unwrap_or_default(),
                conversation_id,
                dispatch_mode: args.dispatch_mode,
                run_at: args.run_at.clone(),
                cron: args.cron.clone(),
                end_at: args.end_at.clone(),
            };
            let timer = timer_store_create(app_state, &input)?;
            runtime_log_info(format!(
                "[工具] 状态=完成 动作=timer.create 定时器={} 目标会话={} 调度方式={} 下次触发={}",
                timer.id,
                timer.conversation_id.as_deref().unwrap_or("<全局>"),
                timer.dispatch_mode.as_str(),
                timer.next_run_at.as_deref().unwrap_or("<无>"),
            ));
            timer_scheduler_notify_changed(app_state);
            timer_publish_changed_event(app_state, "created", &timer.id, Some(&timer));
            serde_json::to_value(timer).map_err(|err| format!("序列化定时器失败：{err}"))
        }
        "update" => {
            timer_tool_reject_delegate_thread(app_state, session_id)?;
            let timer_id = args
                .timer_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "timer.timerId is required for action=update".to_string())?;
            timer_tool_assert_conversation_in_scope(session_id, args.conversation_id.as_deref())?;
            timer_tool_assert_timer_in_scope(app_state, session_id, timer_id)?;
            let input = TimerUpdateInput {
                timer_id: timer_id.to_string(),
                message: args.message.clone(),
                conversation_id: args.conversation_id.clone(),
                dispatch_mode: args.dispatch_mode,
                enabled: args.enabled,
                run_at: args.run_at.clone(),
                cron: args.cron.clone(),
                end_at: args.end_at.clone(),
            };
            let timer = timer_store_update(app_state, &input)?;
            runtime_log_info(format!(
                "[工具] 状态=完成 动作=timer.update 定时器={} 启用={} 下次触发={}",
                timer.id,
                timer.enabled,
                timer.next_run_at.as_deref().unwrap_or("<无>"),
            ));
            timer_scheduler_notify_changed(app_state);
            timer_publish_changed_event(app_state, "updated", &timer.id, Some(&timer));
            serde_json::to_value(timer).map_err(|err| format!("序列化定时器失败：{err}"))
        }
        "delete" => {
            timer_tool_reject_delegate_thread(app_state, session_id)?;
            let timer_id = args
                .timer_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "timer.timerId is required for action=delete".to_string())?;
            timer_tool_assert_timer_in_scope(app_state, session_id, timer_id)?;
            timer_store_delete(&app_state.data_path, timer_id)?;
            runtime_log_info(format!("[工具] 状态=完成 动作=timer.delete 定时器={timer_id}"));
            timer_scheduler_notify_changed(app_state);
            timer_publish_changed_event(app_state, "deleted", timer_id, None);
            Ok(serde_json::json!({ "deleted": timer_id }))
        }
        _ => Err("timer.action must be one of: list, create, update, delete".to_string()),
    }
}
