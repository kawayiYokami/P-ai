/// 联系人「已处理游标」：巡检读到哪条消息。
///
/// 口径：
/// - 这是游标**唯一写入口**。内部依次完成：扫旧游标到新游标之间的消息 → 匹配正负词改能量分 →
///   单调推进游标。外部调用方看不到、也不参与其中任何一步。
/// - 单调：新游标必须晚于旧游标；任何无法确认的情况都保持原值并告警，绝不向前跳。
/// - 压缩后原游标消息可能已不存在：保持原值，由下一次读取重新定位。
const REMOTE_IM_CURSOR_PAGE_SIZE: usize = 100;

fn remote_im_contact_processed_cursor(
    state: &AppState,
    contact_id: &str,
) -> Result<Option<String>, String> {
    Ok(state_service_get_remote_im_contact_checkpoint(state, contact_id)?
        .and_then(|checkpoint| checkpoint.processed_cursor_message_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn remote_im_cursor_checkpoint(state: &AppState, contact_id: &str) -> Result<RemoteImContactCheckpoint, String> {
    Ok(state_service_get_remote_im_contact_checkpoint(state, contact_id)?.unwrap_or(
        RemoteImContactCheckpoint {
            contact_id: contact_id.to_string(),
            ..RemoteImContactCheckpoint::default()
        },
    ))
}

/// 确定本次巡检的读取起点：返回「从哪个 sequence 之后开始读」，`None` 表示从会话第一条开始。
///
/// 口径：
/// - 游标消息存在 → 从该消息之后读，原语义不变。
/// - 游标消息不存在（被压缩或清理）→ 从最新压缩摘要之后的第一条开始，摘要本身不算区间、
///   不参与正负词；没有摘要时从会话第一条开始。旧游标本身不修复，也不停在原地。
fn remote_im_cursor_read_origin_sequence(
    state: &AppState,
    conversation_id: &str,
    previous_cursor: Option<&str>,
) -> Result<Option<i64>, String> {
    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    if let Some(cursor) = previous_cursor {
        if let Some(sequence) = message_store::chat_store_read_message_sequence(&paths, cursor)? {
            return Ok(Some(sequence as i64));
        }
        runtime_log_warn(format!(
            "[群聊巡检][游标] 原游标消息已不存在，改为从最新摘要之后重新读，cursor={cursor}"
        ));
    }
    let summary_sequence = match message_store::chat_store_read_latest_compaction_message(&paths)? {
        Some(summary) => message_store::chat_store_read_message_sequence(&paths, &summary.id)?
            .map(|sequence| sequence as i64),
        None => None,
    };
    Ok(summary_sequence)
}

/// 读某个 sequence 之后到 `until_message_id`（含）的消息；读不到目标时返回已读部分，由调用方判定。
///
/// 分页读取，不整读会话：第一页按 sequence 定位，之后按上一页最后一条消息的 sequence 续读。
fn remote_im_read_cursor_range(
    state: &AppState,
    conversation_id: &str,
    after_sequence: Option<i64>,
    until_message_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    let mut collected = Vec::<ChatMessage>::new();
    let mut anchor_sequence = after_sequence;
    loop {
        let Some(page) = message_store::chat_store_read_messages_from_sequence(
            &paths,
            anchor_sequence,
            REMOTE_IM_CURSOR_PAGE_SIZE,
        )?
        else {
            break;
        };
        if page.messages.is_empty() {
            break;
        }
        let has_more = page.has_more;
        let mut reached_target = false;
        let mut last_message_id = String::new();
        for message in page.messages {
            last_message_id = message.id.clone();
            let is_target = message.id.trim() == until_message_id;
            collected.push(message);
            if is_target {
                reached_target = true;
                break;
            }
        }
        if reached_target || !has_more || last_message_id.is_empty() {
            break;
        }
        anchor_sequence =
            message_store::chat_store_read_message_sequence(&paths, &last_message_id)?
                .map(|sequence| sequence as i64);
        if anchor_sequence.is_none() {
            break;
        }
    }
    Ok(collected)
}

/// 推进游标：唯一的写入口。
///
/// 返回 true 表示推进成功；false 表示幂等或条件不满足（已记日志，不算错误）。
fn remote_im_advance_contact_cursor(
    state: &AppState,
    contact: &RemoteImContact,
    conversation_id: &str,
    new_cursor_message_id: &str,
) -> Result<bool, String> {
    let new_cursor_message_id = new_cursor_message_id.trim();
    if new_cursor_message_id.is_empty() {
        return Err("推进游标失败：新游标消息为空".to_string());
    }
    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    if message_store::chat_store_read_message_by_id(&paths, new_cursor_message_id)?.is_none() {
        runtime_log_warn(format!(
            "[群聊巡检][游标] 跳过，任务=推进游标，contact_id={}，reason=新游标消息不存在，target={}",
            contact.id, new_cursor_message_id
        ));
        return Ok(false);
    }
    let mut checkpoint = remote_im_cursor_checkpoint(state, &contact.id)?;
    let previous_cursor = checkpoint
        .processed_cursor_message_id
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if previous_cursor.as_deref() == Some(new_cursor_message_id) {
        return Ok(false);
    }
    // 起点统一走「游标存在用游标、不存在从最新摘要之后（无摘要则会话开头）」。
    let origin =
        remote_im_cursor_read_origin_sequence(state, conversation_id, previous_cursor.as_deref())?;
    let range =
        remote_im_read_cursor_range(state, conversation_id, origin, new_cursor_message_id)?;
    let reached_target = range
        .last()
        .map(|message| message.id.trim() == new_cursor_message_id)
        .unwrap_or(false);
    if !reached_target {
        runtime_log_warn(format!(
            "[群聊巡检][游标] 跳过，任务=推进游标，contact_id={}，reason=区间未覆盖目标（可能倒退或消息缺失），处理=保持原值，previous={}，target={}",
            contact.id,
            previous_cursor.as_deref().unwrap_or(""),
            new_cursor_message_id
        ));
        return Ok(false);
    }
    if !range.is_empty() {
        // 正负词只读群成员消息，不读助理自己的；结算在这里完成，外部不可见。
        let inbound_messages = range
            .iter()
            .filter(|message| remote_im_group_reply_message_matches_contact(message, contact))
            .cloned()
            .collect::<Vec<_>>();
        remote_im_apply_group_energy_for_messages(state, contact, &inbound_messages)?;
        checkpoint = remote_im_cursor_checkpoint(state, &contact.id)?;
    }
    checkpoint.processed_cursor_message_id = Some(new_cursor_message_id.to_string());
    checkpoint.processed_cursor_updated_at = Some(now_iso());
    remote_im_bump_checkpoint_atomic_revision(&mut checkpoint);
    checkpoint.updated_at = Some(now_iso());
    state_service_set_remote_im_contact_checkpoint(state, &checkpoint)?;
    Ok(true)
}
