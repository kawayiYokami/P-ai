fn lock_remote_im_contact_runtime_states(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, std::collections::HashMap<String, RemoteImContactRuntimeState>>, String>
{
    match state.remote_im_contact_runtime_states.lock() {
        Ok(states) => Ok(states),
        Err(poisoned) => {
            runtime_log_warn(
                "[远程联系人状态机] 运行时锁中毒，已恢复并继续处理当前业务".to_string(),
            );
            Ok(poisoned.into_inner())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteImReplyInspectionPath {
    Mention,
    NonMention,
}

/// 一次巡检的输入：区间、长度上限、路径。
///
/// 区间两端都来自游标与消息库，不含任何批次状态；处理完就丢弃。
#[derive(Clone)]
struct RemoteImReplyDebounceReady {
    start_message_id: String,
    end_message_id: String,
    focus: bool,
    max_chars: u32,
    path: RemoteImReplyInspectionPath,
    event: ChatPendingEvent,
}

fn remote_im_event_latest_user_message(event: &ChatPendingEvent) -> Option<&ChatMessage> {
    event
        .messages
        .iter()
        .rev()
        .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
}

fn remote_im_event_hits_wake(contact: &RemoteImContact, event: &ChatPendingEvent) -> bool {
    let Some(message) = remote_im_event_latest_user_message(event) else {
        return false;
    };
    remote_im_keyword_matched(contact, &render_message_content_for_model(message))
}

fn remote_im_contact_is_muted(state: &AppState, contact_id: &str) -> Result<bool, String> {
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
    let Some(runtime) = runtime_states.get_mut(contact_id) else {
        return Ok(false);
    };
    let Some(mute_until) = runtime.mute_until.clone() else {
        return Ok(false);
    };
    if remote_im_is_mute_expired(&mute_until, now_utc()) {
        runtime.mute_until = None;
        return Ok(false);
    }
    Ok(true)
}

fn clear_remote_im_debounces_for_contact(
    state: &AppState,
    contact_id: &str,
) -> Result<(), String> {
    let key = remote_im_group_reply_state_key(state, contact_id);
    lock_remote_im_inspection_schedule_store()
        .next_inspection
        .remove(&key);
    Ok(())
}

/// 算这一轮的等待时间。
///
/// - 点名加速：加速的对象是**当前这一轮还要等多久**，把剩余等待缩短到「点名加速秒数」，
///   绝不往后推（剩余 20 秒 → 3 秒；剩余 1 秒 → 保持 1 秒；剩余 100ms → 保持 100ms）。
///   当前没有排期项（第一条消息就是点名）时没有可缩短的剩余时间，直接用点名秒数。
/// - 常规巡检周期：带随机波动，不因新消息重置。
fn remote_im_group_reply_timer_delay(
    pacing: &RemoteImGroupReplyPacing,
    kind: RemoteImGroupReplyTimerKind,
    remaining: Option<std::time::Duration>,
) -> std::time::Duration {
    match kind {
        RemoteImGroupReplyTimerKind::Mention => {
            let mention_delay =
                std::time::Duration::from_secs(pacing.assistant_debounce_seconds);
            match remaining {
                Some(remaining) => mention_delay.min(remaining),
                None => mention_delay,
            }
        }
        RemoteImGroupReplyTimerKind::NonMention => remote_im_group_reply_inspection_delay(
            pacing,
            remote_im_group_reply_random_sample(),
        ),
    }
}

fn remote_im_group_reply_schedule_action(
    state: &AppState,
    action: RemoteImGroupReplyTimerAction,
) {
    #[cfg(test)]
    if action.delay >= std::time::Duration::from_secs(300) {
        return;
    }
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(action.delay).await;
        remote_im_group_reply_handle_timer(&state, action).await;
    });
}

/// 排下一轮巡检：写排期 + 起一次性定时器。
///
/// 定时器链就是「什么时候看下一轮」的唯一载体：每次排期都覆盖上一条，
/// 定时器醒来时按到期时刻认领，被覆盖的那一枪自动作废（不需要版本号）。
fn remote_im_group_reply_schedule_inspection(
    state: &AppState,
    contact: &RemoteImContact,
    kind: RemoteImGroupReplyTimerKind,
    anchor_message_id: &str,
    event: ChatPendingEvent,
    reason: &str,
) {
    let pacing = effective_remote_im_group_reply_pacing(state, contact);
    let key = remote_im_group_reply_state_key(state, &contact.id);
    // 剩余时间与排期写入在同一把锁内取与写：点名加速要看到「当前这一轮还要等多久」。
    let (delay, due_at) = {
        let mut store = lock_remote_im_inspection_schedule_store();
        let remaining = store.next_inspection.get(&key).map(|current| {
            current
                .due_at
                .saturating_duration_since(std::time::Instant::now())
        });
        let delay = remote_im_group_reply_timer_delay(&pacing, kind, remaining);
        let due_at = std::time::Instant::now() + delay;
        store.next_inspection.insert(
            key.clone(),
            RemoteImInspectionSchedule {
                kind,
                due_at,
                anchor_message_id: anchor_message_id.to_string(),
                event,
            },
        );
        (delay, due_at)
    };
    runtime_log_info(format!(
        "[群聊巡检] 已安排：联系人={}，路径={:?}，等待毫秒={}，reason={}",
        remote_im_contact_log_label(contact),
        kind,
        delay.as_millis(),
        reason
    ));
    remote_im_group_reply_schedule_action(
        state,
        RemoteImGroupReplyTimerAction {
            state_key: key,
            contact_id: contact.id.clone(),
            kind,
            due_at,
            delay,
        },
    );
}

/// 重置巡检剩余时间：这一轮没看成（能量不足、起点或区间读不出）时重新起倒计时，游标不动。
///
/// 这不是「排下一轮」——一轮正常结束后什么都不做，下一次巡检只由新消息入库起。
fn remote_im_group_reply_reset_countdown(
    state: &AppState,
    contact: &RemoteImContact,
    kind: RemoteImGroupReplyTimerKind,
    anchor_message_id: &str,
    event: ChatPendingEvent,
    reason: &str,
) {
    remote_im_group_reply_schedule_inspection(
        state,
        contact,
        kind,
        anchor_message_id,
        event,
        reason,
    );
}

fn remote_im_group_reply_reconfigure_contact(
    state: &AppState,
    contact: &RemoteImContact,
) -> Result<(), String> {
    let key = remote_im_group_reply_state_key(state, &contact.id);
    let current = {
        let store = lock_remote_im_inspection_schedule_store();
        store
            .next_inspection
            .get(&key)
            .map(|current| (current.kind, current.anchor_message_id.clone(), current.event.clone()))
    };
    // 没有巡检剩余时间就不动：下一轮由新消息入库起时会用新节奏。
    let Some((kind, anchor_message_id, event)) = current else {
        return Ok(());
    };
    remote_im_group_reply_schedule_inspection(
        state,
        contact,
        kind,
        &anchor_message_id,
        event,
        "行为配置变更，按新节奏重排",
    );
    Ok(())
}

fn remote_im_enforce_mute_side_effects(
    state: &AppState,
    contact_id: &str,
    reason: &str,
) {
    if let Err(err) = clear_remote_im_debounces_for_contact(state, contact_id) {
        runtime_log_warn(format!(
            "[群聊巡检] 闭嘴清理降级，contact_id={}，error={}",
            contact_id, err
        ));
    }
    let aborted = match abort_remote_im_reply_delegates_for_contact(state, contact_id, reason) {
        Ok(aborted) => aborted,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程联系人状态机] 闭嘴终止委托降级，contact_id={}，reason={}，error={}",
                contact_id, reason, err
            ));
            0
        }
    };
    runtime_log_info(format!(
        "[远程联系人状态机] 闭嘴善后 完成: contact_id={}, aborted_delegate_count={}, reason={}",
        contact_id, aborted, reason
    ));
}

/// 排期存在 = 该联系人已经入场、巡检链还在跑。
fn remote_im_group_reply_has_active_inspection(state: &AppState, contact_id: &str) -> bool {
    let key = remote_im_group_reply_state_key(state, contact_id);
    lock_remote_im_inspection_schedule_store()
        .next_inspection
        .contains_key(&key)
}

fn remote_im_group_reply_contact_latest(    state: &AppState,
    contact_id: &str,
) -> Result<RemoteImContact, String> {
    state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("群聊联系人已不存在：{contact_id}"))
}

fn remote_im_group_reply_message_matches_contact(
    message: &ChatMessage,
    contact: &RemoteImContact,
) -> bool {
    message.role.trim().eq_ignore_ascii_case("user")
        && message_origin_string(message, "kind") == Some("remote_im")
        && message_origin_string(message, "channel_id") == Some(contact.channel_id.trim())
        && message_origin_string(message, "contact_type")
            == Some(contact.remote_contact_type.trim())
        && message_origin_string(message, "contact_id") == Some(contact.remote_contact_id.trim())
}

/// 从 `after_sequence` 之后读到当前最新一条，返回区间、区间里最后一条联系人消息（没有则为 None）、
/// 以及是否命中关注词。`after_sequence = None` 表示从会话第一条开始。
///
/// 区间不设人为上限：能量长期为负导致积压时，按分页读完整段。
fn read_remote_im_group_reply_range_to_latest(
    state: &AppState,
    contact: &RemoteImContact,
    conversation_id: &str,
    after_sequence: Option<i64>,
) -> Result<(Vec<ChatMessage>, Option<String>, bool), String> {
    const RANGE_PAGE_SIZE: usize = 100;
    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    ensure_chat_store_conversation_readable(state, conversation_id, &paths)?;
    let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error(
            "conversation_mutation_gate",
            file!(),
            line!(),
            module_path!(),
            &err,
        )
    })?;
    let mut range = Vec::<ChatMessage>::new();
    let mut cursor = after_sequence;
    loop {
        let Some(page) = message_store::chat_store_read_messages_from_sequence(
            &paths,
            cursor,
            RANGE_PAGE_SIZE,
        )?
        else {
            break;
        };
        if page.messages.is_empty() {
            break;
        }
        let has_more = page.has_more;
        let mut last_message_id = String::new();
        for message in page.messages {
            last_message_id = message.id.clone();
            range.push(message);
        }
        if !has_more || last_message_id.is_empty() {
            break;
        }
        cursor = message_store::chat_store_read_message_sequence(&paths, &last_message_id)?
            .map(|sequence| sequence as i64);
        if cursor.is_none() {
            break;
        }
    }
    let end_message_id = range
        .iter()
        .rev()
        .find(|message| remote_im_group_reply_message_matches_contact(message, contact))
        .map(|message| message.id.clone());
    let focus = range
        .iter()
        .filter(|message| remote_im_group_reply_message_matches_contact(message, contact))
        .map(render_message_content_for_model)
        .any(|text| remote_im_group_reply_focus_matches(state, contact, &text));
    Ok((range, end_message_id, focus))
}

/// 巡检到点：取游标 → 读区间 → 推进游标 → 交给判断 → 排下一轮。
///
/// 能量门在读范围之前：非点名能量不足只重置巡检时间，游标与消息都不动。
async fn remote_im_group_reply_handle_timer(
    state: &AppState,
    action: RemoteImGroupReplyTimerAction,
) {
    // 到点即结束这一次观察：闹钟项在这里就消失，观察员休眠。
    // 判断、委托、外发都不属于巡检——期间入库的新消息看到没有闹钟会起新一轮，正确性由游标保证。
    let entry = {
        let mut store = lock_remote_im_inspection_schedule_store();
        let Some(current) = store.next_inspection.get(&action.state_key) else {
            return;
        };
        if current.due_at != action.due_at {
            return;
        }
        let current = current.clone();
        store.next_inspection.remove(&action.state_key);
        current
    };
    let contact = match remote_im_group_reply_contact_latest(state, &action.contact_id) {
        Ok(contact) => contact,
        Err(err) => {
            if err.starts_with("群聊联系人已不存在") {
                runtime_log_info(format!(
                    "[群聊巡检] 联系人已删除，清理巡检排期，contact_id={}",
                    action.contact_id
                ));
                let _ = clear_remote_im_debounces_for_contact(state, &action.contact_id);
                return;
            }
            runtime_log_warn(format!(
                "[群聊巡检] 跳过，contact_id={}，reason=读取联系人失败，游标未动，等新消息入库起下一轮，error={}",
                action.contact_id, err
            ));
            return;
        }
    };
    match remote_im_contact_is_muted(state, &contact.id) {
        Ok(true) => {
            runtime_log_info(format!(
                "[群聊巡检] 跳过，contact_id={}，reason=联系人处于闭嘴状态",
                contact.id
            ));
            let _ = clear_remote_im_debounces_for_contact(state, &contact.id);
            return;
        }
        Ok(false) => {}
        Err(err) => runtime_log_warn(format!(
            "[群聊巡检] 闭嘴状态读取失败，发送前还会再校验，contact_id={}，error={}",
            contact.id, err
        )),
    }
    // 能量门在读范围之前：点名只跳过能量与回复冷却（收发许可、渠道、闭嘴照旧），
    // 非点名不足只重置巡检时间，游标与消息都不动。
    let mention_path = action.kind == RemoteImGroupReplyTimerKind::Mention;
    let (max_chars, energy_label) = {
        let state_for_blocking = state.clone();
        let contact_for_blocking = contact.clone();
        let gate = match tokio::task::spawn_blocking(move || {
            remote_im_group_reply_gate(
                &state_for_blocking,
                &contact_for_blocking,
                mention_path,
                mention_path,
            )
        })
        .await
        {
            Ok(result) => result,
            Err(err) => {
                remote_im_group_reply_reset_countdown(
                    state,
                    &contact,
                    action.kind,
                    &entry.anchor_message_id,
                    entry.event.clone(),
                    &format!("能量门控任务失败：{err}"),
                );
                return;
            }
        };
        match gate {
            Ok(gate) if !gate.allowed => {
                runtime_log_info(format!(
                    "[群聊巡检] 跳过，contact_id={}，reason={}，处理=只重置巡检时间，游标不动",
                    contact.id, gate.reason
                ));
                remote_im_group_reply_reset_countdown(
                    state,
                    &contact,
                    action.kind,
                    &entry.anchor_message_id,
                    entry.event.clone(),
                    &gate.reason,
                );
                return;
            }
            Ok(gate) => (
                gate.max_chars,
                if mention_path {
                    "跳过".to_string()
                } else {
                    format!("{:.2}", gate.energy)
                },
            ),
            Err(err) => {
                remote_im_group_reply_reset_countdown(
                    state,
                    &contact,
                    action.kind,
                    &entry.anchor_message_id,
                    entry.event.clone(),
                    &format!("读取能量或冷却状态失败：{err}"),
                );
                return;
            }
        }
    };
    let conversation_id = entry.event.conversation_id.clone();
    // 取旧游标：游标为空时用入场起点当区间起点（游标唯一写入口负责落库）。
    let mut previous = match remote_im_contact_processed_cursor(state, &contact.id) {
        Ok(value) => value,
        Err(err) => {
            remote_im_group_reply_reset_countdown(
                state,
                &contact,
                RemoteImGroupReplyTimerKind::NonMention,
                &entry.anchor_message_id,
                entry.event.clone(),
                &format!("读取已处理游标失败：{err}"),
            );
            return;
        }
    };
    if previous.is_none() {
        match remote_im_advance_contact_cursor(
            state,
            &contact,
            &conversation_id,
            &entry.anchor_message_id,
        ) {
            Ok(_) => previous = Some(entry.anchor_message_id.clone()),
            Err(err) => {
                runtime_log_warn(format!(
                    "[群聊巡检] 跳过，contact_id={}，reason=入场起点无法成为游标起点，error={}",
                    contact.id, err
                ));
            }
        }
    }
    // 读取起点：游标消息存在就用它；已被压缩/清理时从最新摘要之后读（无摘要则从会话开头）。
    // 不修复旧游标，也不在读取失败时停在原地。
    let origin = match remote_im_cursor_read_origin_sequence(
        state,
        &conversation_id,
        previous.as_deref(),
    ) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_warn(format!(
                "[群聊巡检] 跳过，contact_id={}，reason=读取巡检起点失败，处理=重排下一轮，error={}",
                contact.id, err
            ));
            remote_im_group_reply_reset_countdown(
                state,
                &contact,
                RemoteImGroupReplyTimerKind::NonMention,
                &entry.anchor_message_id,
                entry.event.clone(),
                "读取巡检起点失败",
            );
            return;
        }
    };
    let (range, end_message_id, focus) =
        match read_remote_im_group_reply_range_to_latest(state, &contact, &conversation_id, origin) {
            Ok(value) => value,
            Err(err) => {
                runtime_log_warn(format!(
                    "[群聊巡检] 跳过，contact_id={}，reason=区间读取失败，处理=重排下一轮，previous={}，error={}",
                    contact.id,
                    previous.as_deref().unwrap_or(""),
                    err
                ));
                remote_im_group_reply_reset_countdown(
                    state,
                    &contact,
                    RemoteImGroupReplyTimerKind::NonMention,
                    &entry.anchor_message_id,
                    entry.event.clone(),
                    "区间读取失败",
                );
                return;
            }
        };
    let Some(end_message_id) = end_message_id else {
        runtime_log_info(format!(
            "[群聊巡检] 跳过，contact_id={}，reason=区间内没有新的联系人消息",
            contact.id
        ));
        return;
    };
    // 推进游标：内部扫区间结算正负词，外部看不到也不参与。
    match remote_im_advance_contact_cursor(state, &contact, &conversation_id, &end_message_id) {
        Ok(true) => {}
        Ok(false) => {
            runtime_log_warn(format!(
                "[群聊巡检] 跳过，contact_id={}，reason=游标未推进（幂等或条件不满足），previous={}，target={}",
                contact.id,
                previous.as_deref().unwrap_or(""),
                end_message_id
            ));
            return;
        }
        Err(err) => {
            runtime_log_warn(format!(
                "[群聊巡检] 跳过，contact_id={}，reason=推进游标失败，previous={}，target={}，error={}",
                contact.id,
                previous.as_deref().unwrap_or(""),
                end_message_id,
                err
            ));
            return;
        }
    }
    let Some(start_message_id) = range.first().map(|message| message.id.clone()) else {
        return;
    };
    let batch_preview = range
        .iter()
        .rev()
        .find(|message| remote_im_group_reply_message_matches_contact(message, &contact))
        .map(render_message_content_for_model)
        .map(|text| remote_im_preview_text(&text, 100))
        .unwrap_or_else(|| "（无文本）".to_string());
    runtime_log_info(format!(
        "[群聊巡检] 开始：联系人={}，内容={}，路径={:?}，能量={}，回复上限={}字，区间消息数={}",
        remote_im_contact_log_label(&contact),
        batch_preview,
        action.kind,
        energy_label,
        max_chars,
        range.len()
    ));
    let ready = RemoteImReplyDebounceReady {
        start_message_id,
        end_message_id,
        focus,
        max_chars,
        path: match action.kind {
            RemoteImGroupReplyTimerKind::Mention => RemoteImReplyInspectionPath::Mention,
            RemoteImGroupReplyTimerKind::NonMention => RemoteImReplyInspectionPath::NonMention,
        },
        event: entry.event,
    };
    // 判断结果不决定巡检是否继续：这一轮在推进游标、交付判断之后已经结束，观察员休眠。
    match process_remote_im_reply_debounce(state, ready).await {
        Ok(_) => {}
        Err(err) => runtime_log_warn(format!(
            "[群聊巡检] 降级，contact_id={}，reason=判断失败但巡检已结束，error={}",
            contact.id, err
        )),
    }
}

fn observe_remote_im_persisted_event(
    state: &AppState,
    contact: &RemoteImContact,
    event: &ChatPendingEvent,
) {
    let Some(message) = remote_im_event_latest_user_message(event) else {
        return;
    };
    match remote_im_contact_is_muted(state, &contact.id) {
        Ok(true) => {
            let _ = clear_remote_im_debounces_for_contact(state, &contact.id);
            return;
        }
        Ok(false) => {}
        Err(err) => {
            runtime_log_warn(format!(
                "[群聊巡检] 闭嘴状态读取失败，已保留排期并在发送前再次校验，contact_id={}，message_id={}，error={}",
                contact.id, message.id, err
            ));
        }
    }
    let hits_mention = remote_im_event_hits_wake(contact, event);
    let key = remote_im_group_reply_state_key(state, &contact.id);
    let mut reference = event.clone();
    reference.messages.clear();
    let mut schedule = None::<(RemoteImGroupReplyTimerKind, String, ChatPendingEvent)>;
    {
        let mut store = lock_remote_im_inspection_schedule_store();
        match store.next_inspection.get_mut(&key) {
            None => {
                // 还没有排期：以这条消息为入场起点排第一轮（起点固定后不再移动）。
                let kind = if hits_mention {
                    RemoteImGroupReplyTimerKind::Mention
                } else {
                    RemoteImGroupReplyTimerKind::NonMention
                };
                schedule = Some((kind, message.id.clone(), reference));
            }
            Some(current) => {
                if hits_mention && current.kind == RemoteImGroupReplyTimerKind::NonMention {
                    // 等常规周期时突然被点名：把这一轮升级成点名，提前看。
                    schedule = Some((
                        RemoteImGroupReplyTimerKind::Mention,
                        current.anchor_message_id.clone(),
                        current.event.clone(),
                    ));
                }
                // 其余情况（常规周期已排、点名已排）：静默入库，绝不重置闹钟。
            }
        }
    }
    if let Some((kind, anchor_message_id, event)) = schedule {
        runtime_log_info(format!(
            "[群聊巡检] 已安排：联系人={}，内容={}，路径={:?}",
            remote_im_contact_log_label(contact),
            remote_im_preview_text(&render_message_content_for_model(message), 100),
            kind
        ));
        remote_im_group_reply_schedule_inspection(
            state,
            contact,
            kind,
            &anchor_message_id,
            event,
            "入站消息",
        );
    }
}
