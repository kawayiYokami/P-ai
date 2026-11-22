fn remote_im_event_latest_message_id(event: &ChatPendingEvent) -> Option<String> {
    event.messages.last().map(|message| message.id.clone())
}

fn remote_im_update_checkpoint_latest_seen_in_list(
    checkpoints: &mut Vec<RemoteImContactCheckpoint>,
    contact_id: &str,
    message_id: Option<&str>,
    now: &str,
) {
    let checkpoint = remote_im_contact_checkpoint_mut_in_list(checkpoints, contact_id);
    remote_im_update_checkpoint_latest_seen_in_checkpoint(checkpoint, message_id, now);
}

fn remote_im_update_checkpoint_latest_seen_in_checkpoint(
    checkpoint: &mut RemoteImContactCheckpoint,
    message_id: Option<&str>,
    now: &str,
) {
    // 只记「最后看到哪条」，入站即写，不参与巡检判定。
    // 巡检游标是另一个字段（processed_cursor_message_id），只能由推进游标方法写；
    // 两者绝不能合并：合并等于每次入站都把游标推到最新，巡检永远读到空区间。
    checkpoint.latest_seen_message_id = message_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(checkpoint.latest_seen_message_id.clone());
    checkpoint.updated_at = Some(now.to_string());
}

fn remote_im_handle_persisted_event_after_history_flush_runtime(
    state: &AppState,
    contacts: &[RemoteImContact],
    checkpoints: &mut Vec<RemoteImContactCheckpoint>,
    conversation: &mut Conversation,
    event: &ChatPendingEvent,
    now: &str,
    activated_contacts_in_batch: &mut std::collections::HashSet<String>,
) -> Result<bool, String> {
    let Some(sender) = event.sender_info.as_ref() else {
        return Ok(false);
    };
    let Some(contact) = remote_im_contact_by_source_in_runtime(contacts, sender).cloned() else {
        return Ok(false);
    };
    let latest_message_id = remote_im_event_latest_message_id(event);
    remote_im_update_checkpoint_latest_seen_in_list(
        checkpoints,
        &contact.id,
        latest_message_id.as_deref(),
        now,
    );
    if !event.activate_assistant {
        remote_im_append_contact_log(
            &contact,
            "info",
            format!(
                "[联系人状态] 历史落地: contact={}, activate=否, reason=event_gate_blocked",
                remote_im_contact_log_label(&contact)
            ),
        );
        return Ok(false);
    }

    let message_text = event
        .messages
        .last()
        .map(render_message_content_for_model)
        .unwrap_or_default();
    let (should_activate, entry_reason) = match remote_im_prepare_enqueue_runtime_state(
        state,
        &contact,
        &message_text,
    ) {
        Ok(result) => result,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 历史落地后入场判定失败，本次不启动巡检，contact_id={}, conversation_id={}, error={}",
                contact.id, conversation.id, err
            ));
            (false, "入场判定失败，仅保留已入库消息".to_string())
        }
    };
    if !should_activate {
        remote_im_append_contact_log(
            &contact,
            "info",
            format!(
                "[联系人状态] 历史落地: contact={}, activate=否, reason={}",
                remote_im_contact_log_label(&contact),
                entry_reason
            ),
        );
        return Ok(false);
    }

    let (
        previous_presence,
        previous_work,
        previous_pending,
        current_presence,
        current_work,
        current_pending,
        state_reason,
    ) = {
        let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
        let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
        let previous_presence = runtime.presence_state;
        let previous_work = runtime.work_state;
        let previous_pending = runtime.has_pending;
        let state_reason = match runtime.presence_state {
            RemoteImPresenceState::Away => {
                format!("{}；已入场，开始首轮巡检", entry_reason)
            }
            RemoteImPresenceState::Present => {
                format!("{}；已入场，开始在场巡检", entry_reason)
            }
        };
        (
            previous_presence,
            previous_work,
            previous_pending,
            runtime.presence_state,
            runtime.work_state,
            runtime.has_pending,
            state_reason,
        )
    };

    if should_activate {
        // 已入场的远程事件才会进入巡检；同一批后续消息由批次状态追加，
        // 不会因为联系人仍处于在场状态而让新的未入场消息混入。
        activated_contacts_in_batch.insert(format!("{}:{}", contact.id, event.id));
        runtime_log_info(format!(
            "[远程联系人状态机] 激活调度 开始: contact_id={}, conversation_id={}",
            contact.id, conversation.id
        ));
    }
    remote_im_append_contact_log(
        &contact,
        "info",
        format!(
            "[联系人状态] 历史落地: contact={}, presence={} -> {}, work={} -> {}, pending={} -> {}, activate={}, reason={}",
            remote_im_contact_log_label(&contact),
            remote_im_presence_state_label(previous_presence),
            remote_im_presence_state_label(current_presence),
            remote_im_work_state_label(previous_work),
            remote_im_work_state_label(current_work),
            remote_im_yes_no(previous_pending),
            remote_im_yes_no(current_pending),
            remote_im_yes_no(should_activate),
            state_reason
        ),
    );
    Ok(should_activate)
}

fn remote_im_finalize_round_completion(
    state: &AppState,
    activated_sources: &[RemoteImActivationSource],
    reply_decision: Option<&str>,
    reply_target: Option<&RemoteImReplyTarget>,
    failed_error: Option<&str>,
    finished_at: &str,
) -> Result<Vec<RemoteImActivationSource>, String> {
    if activated_sources.is_empty() {
        return Ok(Vec::new());
    }
    // query-before-lock：先完成联系人查询与渠道行为配置读取，再获取运行态锁，
    // 避免锁内执行同步 SQLite 读取（与 remote_im_finalize_async_send_result 保持一致）。
    let mut resolved_contacts = Vec::<(RemoteImActivationSource, RemoteImContact, u64)>::new();
    for source in activated_sources {
        let Some(contact) = state_service_find_remote_im_contact_by_identity(
            state,
            &source.channel_id,
            &source.remote_contact_type,
            &source.remote_contact_id,
        )?
        else {
            continue;
        };
        let patience_seconds =
            remote_im_channel_behavior_settings_for_contact(state, &contact).patience_seconds;
        resolved_contacts.push((source.clone(), contact, patience_seconds));
    }
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
    let mut follow_up_sources = Vec::<RemoteImActivationSource>::new();
    let mut presence_timeouts = std::collections::HashMap::<String, u64>::new();
    let mut dashboard_contact_ids = std::collections::HashSet::<String>::new();
    for (source, contact, patience_seconds) in resolved_contacts {
        let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
        let previous_presence = runtime.presence_state;
        let previous_work = runtime.work_state;
        let previous_pending = runtime.has_pending;
        let previous_no_reply_count = runtime.consecutive_no_reply_count;
        runtime.work_state = RemoteImWorkState::Idle;
        let decision_label = match reply_decision.unwrap_or("").trim() {
            "" => "send_async",
            value => value,
        };
        if let Some(error) = failed_error {
            runtime_log_error(format!(
                "[远程联系人状态机] 轮次结束 失败: contact_id={}, presence={:?}->{:?}, pending={}, error={}",
                contact.id,
                previous_presence,
                runtime.presence_state,
                previous_pending,
                error
            ));
            remote_im_append_contact_log(
                &contact,
                "warn",
                format!(
                    "[联系人状态] 轮次收尾失败: contact={}, decision={}, presence={} -> {}, work={} -> {}, pending={} -> {}, error={}",
                    remote_im_contact_log_label(&contact),
                    decision_label,
                    remote_im_presence_state_label(previous_presence),
                    remote_im_presence_state_label(runtime.presence_state),
                    remote_im_work_state_label(previous_work),
                    remote_im_work_state_label(runtime.work_state),
                    remote_im_yes_no(previous_pending),
                    remote_im_yes_no(runtime.has_pending),
                    error
                ),
            );
            continue;
        }
        let should_follow_up_after_round = previous_pending;
        match decision_label {
            "reply" | "send_files" | "send" | "reply_async" => {
                let target_matched = reply_target
                    .map(|target| remote_im_contact_matches_reply_target(&source, target))
                    .unwrap_or(activated_sources.len() == 1);
                runtime.presence_state = RemoteImPresenceState::Present;
                runtime.consecutive_no_reply_count = 0;
                if target_matched {
                    runtime.last_success_reply_at = Some(finished_at.to_string());
                }
            }
            "no_reply" => {
                runtime.consecutive_no_reply_count =
                    runtime.consecutive_no_reply_count.saturating_add(1);
                if runtime.has_pending {
                    runtime.presence_state = RemoteImPresenceState::Present;
                } else if runtime.consecutive_no_reply_count >= 2 {
                    runtime.presence_state = RemoteImPresenceState::Away;
                } else if let Some(last_success_at) = runtime.last_success_reply_at.as_deref() {
                    let elapsed_seconds = parse_iso(last_success_at)
                        .map(|last| (now_utc() - last).whole_seconds().max(0) as u64)
                        .unwrap_or_default();
                    if elapsed_seconds > patience_seconds {
                        runtime.presence_state = RemoteImPresenceState::Away;
                    } else {
                        runtime.presence_state = RemoteImPresenceState::Present;
                    }
                } else {
                    runtime.presence_state = RemoteImPresenceState::Present;
                }
            }
            "send_async" | "" => {
                runtime.presence_state = RemoteImPresenceState::Present;
                runtime.consecutive_no_reply_count = 0;
            }
            _ => {}
        }
        if should_follow_up_after_round {
            runtime.has_pending = false;
            runtime.presence_state = RemoteImPresenceState::Present;
            follow_up_sources.push(source.clone());
        }
        if runtime.presence_state == RemoteImPresenceState::Present {
            runtime.last_presence_at = Some(finished_at.to_string());
            presence_timeouts.insert(contact.id.clone(), patience_seconds);
        }
        dashboard_contact_ids.insert(contact.id.clone());
        runtime_log_info(format!(
            "[远程联系人状态机] 轮次结束 完成: contact_id={}, decision={}, presence={:?}->{:?}, pending={}->{}, no_reply_count={}->{}, follow_up={}, last_success_reply_at={}",
            contact.id,
            decision_label,
            previous_presence,
            runtime.presence_state,
            previous_pending,
            runtime.has_pending,
            previous_no_reply_count,
            runtime.consecutive_no_reply_count,
            should_follow_up_after_round,
            runtime.last_success_reply_at.as_deref().unwrap_or("")
        ));
        remote_im_append_contact_log(
            &contact,
            "info",
            format!(
                "[联系人状态] 轮次结束: contact={}, decision={}, presence={} -> {}, work={} -> {}, pending={} -> {}, no_reply_count={} -> {}, follow_up={}, last_success_reply_at={}",
                remote_im_contact_log_label(&contact),
                decision_label,
                remote_im_presence_state_label(previous_presence),
                remote_im_presence_state_label(runtime.presence_state),
                remote_im_work_state_label(previous_work),
                remote_im_work_state_label(runtime.work_state),
                remote_im_yes_no(previous_pending),
                remote_im_yes_no(runtime.has_pending),
                previous_no_reply_count,
                runtime.consecutive_no_reply_count,
                remote_im_yes_no(should_follow_up_after_round),
                runtime.last_success_reply_at.as_deref().unwrap_or("")
            ),
        );
    }
    drop(runtime_states);
    for contact_id in dashboard_contact_ids {
        remote_im_emit_contact_dashboard_snapshot(state, &contact_id);
    }
    for (contact_id, patience_seconds) in presence_timeouts {
        remote_im_schedule_presence_timeout(state, &contact_id, patience_seconds)?;
    }
    Ok(follow_up_sources)
}

fn remote_im_finalize_async_send_result(
    state: &AppState,
    source: &RemoteImActivationSource,
    send_ok: bool,
    now: &str,
) -> Result<(), String> {
    let Some(contact) = state_service_find_remote_im_contact_by_identity(
        state,
        &source.channel_id,
        &source.remote_contact_type,
        &source.remote_contact_id,
    )?
    else {
        return Ok(());
    };
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;
    // 外发成功或失败都算「刚活跃过」：刷新耐心时间，在场状态没变就不产生在场日志。
    remote_im_apply_contact_runtime_state(&mut runtime_states, &contact, "应答外发完成", |runtime| {
        runtime.presence_state = RemoteImPresenceState::Present;
        runtime.last_presence_at = Some(now.to_string());
        runtime.consecutive_no_reply_count = 0;
        if send_ok {
            runtime.last_success_reply_at = Some(now.to_string());
        }
    });
    drop(runtime_states);
    remote_im_emit_contact_dashboard_snapshot(state, &contact.id);
    remote_im_schedule_presence_timeout(
        state,
        &contact.id,
        remote_im_channel_behavior_settings_for_contact(state, &contact).patience_seconds,
    )?;
    Ok(())
}
