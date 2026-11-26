// ==================== 定时器存储（state/state.sqlite 的 timers 表）====================
// 独立成表、单独走 state 库：定时器不参与会话元数据，也不再有第二个物理文件。

const TIMER_STORE_COLUMNS: &str = "timer_id, conversation_id, message, dispatch_mode, enabled, \
     run_at_utc, cron_expression, end_at_utc, next_run_at_utc, last_triggered_at_utc, created_at_utc";

fn timer_store_connection(data_path: &PathBuf) -> Result<rusqlite::Connection, String> {
    state_db_open(data_path)
}

fn timer_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TimerRecordStored> {
    Ok(TimerRecordStored {
        timer_id: row.get(0)?,
        conversation_id: row.get(1)?,
        message: row.get(2)?,
        dispatch_mode: TimerDispatchMode::from_storage(&row.get::<_, String>(3)?),
        enabled: row.get::<_, i64>(4)? != 0,
        run_at_utc: row.get(5)?,
        cron_expression: row.get(6)?,
        end_at_utc: row.get(7)?,
        next_run_at_utc: row.get(8)?,
        last_triggered_at_utc: row.get(9)?,
        created_at_utc: row.get(10)?,
    })
}

fn timer_store_list_records(data_path: &PathBuf) -> Result<Vec<TimerRecordStored>, String> {
    let conn = timer_store_connection(data_path)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {TIMER_STORE_COLUMNS} FROM timers ORDER BY created_at_utc ASC, timer_id ASC"
        ))
        .map_err(|err| format!("准备读取定时器列表失败：{err}"))?;
    let rows = stmt
        .query_map([], timer_record_from_row)
        .map_err(|err| format!("查询定时器列表失败：{err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析定时器列表失败：{err}"))?;
    Ok(rows)
}

fn timer_store_list(data_path: &PathBuf) -> Result<Vec<TimerRecord>, String> {
    Ok(timer_store_list_records(data_path)?
        .iter()
        .map(timer_record_view_from_stored)
        .collect())
}

/// 目标会话已经不可投递（不存在、或已归档）的定时器不再返回：
/// 它没有地方可去，留在界面上只会显示成一条「未知会话」的僵尸记录。
///
/// 判定失败（读不到会话库）时保留原记录——一次读取失败不该把整个列表清空。
fn timer_store_filter_unknown_conversations(
    data_path: &PathBuf,
    timers: Vec<TimerRecord>,
) -> Vec<TimerRecord> {
    timers
        .into_iter()
        .filter(|timer| {
            let conversation_id = timer
                .conversation_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if conversation_id.is_empty() {
                return true;
            }
            match message_store::chat_metadata_store_conversation_status(data_path, conversation_id) {
                Ok(Some(status)) => status.trim() == "active",
                Ok(None) => false,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[定时器] 会话状态判定失败 定时器={} 会话={} 错误={err}",
                        timer.id, conversation_id
                    ));
                    true
                }
            }
        })
        .collect()
}

/// 只保留属于指定会话的定时器；未指定会话时原样返回全量。
/// 无会话归属的全局定时器不属于任何会话，按会话筛选时不会返回。
fn timer_store_filter_by_conversation(
    timers: Vec<TimerRecord>,
    conversation_id: Option<&str>,
) -> Vec<TimerRecord> {
    let target = conversation_id.map(str::trim).unwrap_or_default();
    if target.is_empty() {
        return timers;
    }
    timers
        .into_iter()
        .filter(|timer| {
            timer
                .conversation_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                == target
        })
        .collect()
}

fn timer_store_get_record(
    data_path: &PathBuf,
    timer_id: &str,
) -> Result<TimerRecordStored, String> {
    use rusqlite::OptionalExtension as _;
    let timer_id = timer_id.trim();
    if timer_id.is_empty() {
        return Err("timerId 不能为空".to_string());
    }
    let conn = timer_store_connection(data_path)?;
    let record = conn
        .query_row(
            &format!("SELECT {TIMER_STORE_COLUMNS} FROM timers WHERE timer_id=?1"),
            rusqlite::params![timer_id],
            timer_record_from_row,
        )
        .optional()
        .map_err(|err| format!("读取定时器失败，timerId={timer_id}，error={err}"))?;
    record.ok_or_else(|| format!("未找到定时器：{timer_id}"))
}

/// 写入一行定时器（新建与更新共用）。
fn timer_store_write_records(
    data_path: &PathBuf,
    record: &TimerRecordStored,
) -> Result<(), String> {
    let conn = timer_store_connection(data_path)?;
    conn.execute(
        "INSERT INTO timers(timer_id, conversation_id, message, dispatch_mode, enabled,
                            run_at_utc, cron_expression, end_at_utc, next_run_at_utc,
                            last_triggered_at_utc, created_at_utc)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(timer_id) DO UPDATE SET
             conversation_id=excluded.conversation_id,
             message=excluded.message,
             dispatch_mode=excluded.dispatch_mode,
             enabled=excluded.enabled,
             run_at_utc=excluded.run_at_utc,
             cron_expression=excluded.cron_expression,
             end_at_utc=excluded.end_at_utc,
             next_run_at_utc=excluded.next_run_at_utc,
             last_triggered_at_utc=excluded.last_triggered_at_utc",
        rusqlite::params![
            record.timer_id,
            record.conversation_id,
            record.message,
            record.dispatch_mode.as_str(),
            if record.enabled { 1i64 } else { 0i64 },
            record.run_at_utc,
            record.cron_expression,
            record.end_at_utc,
            record.next_run_at_utc,
            record.last_triggered_at_utc,
            record.created_at_utc,
        ],
    )
    .map_err(|err| format!("写入定时器失败，timerId={}，error={err}", record.timer_id))?;
    Ok(())
}

fn timer_store_delete(data_path: &PathBuf, timer_id: &str) -> Result<(), String> {
    let timer_id = timer_id.trim();
    if timer_id.is_empty() {
        return Err("timerId 不能为空".to_string());
    }
    let conn = timer_store_connection(data_path)?;
    let deleted = conn
        .execute(
            "DELETE FROM timers WHERE timer_id=?1",
            rusqlite::params![timer_id],
        )
        .map_err(|err| format!("删除定时器失败，timerId={timer_id}，error={err}"))?;
    if deleted == 0 {
        return Err(format!("未找到定时器：{timer_id}"));
    }
    Ok(())
}

/// 校验并归一化一句话正文：定时器只承载一句话，空正文没有意义。
fn timer_normalized_message(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err("message 不能为空：定时器就是一句话".to_string());
    }
    Ok(normalized.to_string())
}

fn timer_normalized_cron(value: Option<&str>) -> Result<Option<String>, String> {
    match value.map(str::trim).filter(|item| !item.is_empty()) {
        Some(cron) => timer_normalize_cron_expression(cron).map(Some),
        None => Ok(None),
    }
}

fn timer_normalized_end_at(value: Option<&str>) -> Result<Option<String>, String> {
    match value.map(str::trim).filter(|item| !item.is_empty()) {
        Some(end_at) => timer_normalize_end_at(end_at).map(Some),
        None => Ok(None),
    }
}

/// 目标会话必须真实存在且未被归档；全局定时器（`None`）不需要会话。
fn timer_validate_conversation_id(
    state: &AppState,
    conversation_id: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(conversation_id) = timer_normalize_conversation_id(conversation_id) else {
        return Ok(None);
    };
    let conversation = conversation_service_v2()
        .get_conversation_meta(state, &conversation_id)
        .map_err(|_| format!("目标会话不存在：{conversation_id}"))?;
    if conversation.status.trim() == "archived"
        || conversation
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
    {
        return Err(format!("目标会话已归档：{conversation_id}"));
    }
    Ok(Some(conversation_id))
}

fn timer_store_create(
    state: &AppState,
    input: &TimerCreateInput,
) -> Result<TimerRecord, String> {
    let message = timer_normalized_message(&input.message)?;
    let conversation_id = timer_validate_conversation_id(state, input.conversation_id.as_deref())?;
    let dispatch_mode =
        timer_normalize_dispatch_mode(conversation_id.as_deref(), input.dispatch_mode.unwrap_or(TimerDispatchMode::Direct));
    let run_at_utc = match input.run_at.as_deref().map(str::trim).filter(|item| !item.is_empty()) {
        Some(run_at) => timer_normalize_run_at(run_at)?,
        // 没给起始时间就从现在开始：单次定时器等价于「立刻排一次」。
        None => normalize_time_for_utc_storage(now_utc())
            .map_err(|err| format!("生成定时器起始时间失败：{err}"))?,
    };
    let cron = timer_normalized_cron(input.cron.as_deref())?;
    let end_at_utc = timer_normalized_end_at(input.end_at.as_deref())?;
    let next_run_at_utc = timer_compute_next_run_at_utc(
        &run_at_utc,
        cron.as_deref(),
        end_at_utc.as_deref(),
        None,
    );
    let short_id = allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_TIMER)?;
    let record = TimerRecordStored {
        timer_id: short_id.to_string(),
        conversation_id,
        message,
        dispatch_mode,
        enabled: next_run_at_utc.is_some(),
        run_at_utc,
        cron_expression: cron,
        end_at_utc,
        next_run_at_utc,
        last_triggered_at_utc: None,
        created_at_utc: normalize_time_for_utc_storage(now_utc())
            .map_err(|err| format!("生成定时器创建时间失败：{err}"))?,
    };
    timer_store_write_records(&state.data_path, &record)?;
    Ok(timer_record_view_from_stored(&record))
}

fn timer_store_update(
    state: &AppState,
    input: &TimerUpdateInput,
) -> Result<TimerRecord, String> {
    let existing = timer_store_get_record(&state.data_path, &input.timer_id)?;
    let message = match input.message.as_deref() {
        Some(message) => timer_normalized_message(message)?,
        None => existing.message.clone(),
    };
    let conversation_id = match input.conversation_id.as_deref() {
        Some(value) => timer_validate_conversation_id(state, Some(value))?,
        None => existing.conversation_id.clone(),
    };
    let requested_mode = input.dispatch_mode.unwrap_or(existing.dispatch_mode);
    let dispatch_mode = timer_normalize_dispatch_mode(conversation_id.as_deref(), requested_mode);
    let run_at_utc = match input.run_at.as_deref().map(str::trim).filter(|item| !item.is_empty()) {
        Some(run_at) => timer_normalize_run_at(run_at)?,
        None => existing.run_at_utc.clone(),
    };
    let cron = match input.cron.as_deref() {
        Some(value) => timer_normalized_cron(Some(value))?,
        None => existing.cron_expression.clone(),
    };
    let end_at_utc = match input.end_at.as_deref() {
        Some(value) => timer_normalized_end_at(Some(value))?,
        None => existing.end_at_utc.clone(),
    };
    let enabled = input.enabled.unwrap_or(existing.enabled);
    let next_run_at_utc = if enabled {
        timer_compute_next_run_at_utc(
            &run_at_utc,
            cron.as_deref(),
            end_at_utc.as_deref(),
            existing.last_triggered_at_utc.as_deref(),
        )
    } else {
        None
    };
    let record = TimerRecordStored {
        timer_id: existing.timer_id,
        conversation_id,
        message,
        dispatch_mode,
        enabled: enabled && next_run_at_utc.is_some(),
        run_at_utc,
        cron_expression: cron,
        end_at_utc,
        next_run_at_utc,
        last_triggered_at_utc: existing.last_triggered_at_utc,
        created_at_utc: existing.created_at_utc,
    };
    timer_store_write_records(&state.data_path, &record)?;
    Ok(timer_record_view_from_stored(&record))
}

/// 触发之后推进下一次：记下本次触发时间，重算下一次；算不出来就停用。
fn timer_store_mark_triggered(
    data_path: &PathBuf,
    record: &TimerRecordStored,
) -> Result<TimerRecordStored, String> {
    let triggered_at_utc = normalize_time_for_utc_storage(now_utc())
        .map_err(|err| format!("生成定时器触发时间失败：{err}"))?;
    let next_run_at_utc = timer_compute_next_run_at_utc(
        &record.run_at_utc,
        record.cron_expression.as_deref(),
        record.end_at_utc.as_deref(),
        Some(&triggered_at_utc),
    );
    let updated = TimerRecordStored {
        enabled: next_run_at_utc.is_some(),
        next_run_at_utc,
        last_triggered_at_utc: Some(triggered_at_utc),
        ..record.clone()
    };
    timer_store_write_records(data_path, &updated)?;
    Ok(updated)
}

/// 派发失败后的退避：把下一次触发推到一个窗口之后，让这一行离开「到点」集合。
///
/// 失败时若原地不动，`next_run_at` 仍停在过去，调度器下一拍又会把它判成到点——
/// 一次失败就变成零间隔重试（旧实现会拿 SQLite schema 初始化把调度线程锁死）。
fn timer_store_mark_dispatch_failure(
    data_path: &PathBuf,
    record: &TimerRecordStored,
) -> Result<TimerRecordStored, String> {
    let retry_at_utc = normalize_time_for_utc_storage(
        now_utc() + time::Duration::seconds(TIMER_DISPATCH_RETRY_BACKOFF_SECONDS),
    )
    .map_err(|err| format!("生成定时器退避时间失败：{err}"))?;
    let updated = TimerRecordStored {
        next_run_at_utc: Some(retry_at_utc),
        ..record.clone()
    };
    timer_store_write_records(data_path, &updated)?;
    Ok(updated)
}

/// 补算缺失的下一次触发时间：迁移来的老数据没有 next_run_at，交给这里补齐。
///
/// 补算口径是「从当前时刻往后」，不是「按老时间补课」：迁移来的历史行如果算出来已经是过去，
/// 一次性任务直接停用（升级后不该翻出陈年待办），cron 任务则以当前时刻为基准取下一个周期，
/// 否则升级第一拍会把一批历史行同时判成到点、一起触发。
fn timer_store_recompute_next_run(
    data_path: &PathBuf,
    record: &TimerRecordStored,
) -> Result<TimerRecordStored, String> {
    let now_utc_text = normalize_time_for_utc_storage(now_utc())
        .map_err(|err| format!("生成定时器补算基准时间失败：{err}"))?;
    let mut next_run_at_utc = timer_compute_next_run_at_utc(
        &record.run_at_utc,
        record.cron_expression.as_deref(),
        record.end_at_utc.as_deref(),
        record.last_triggered_at_utc.as_deref(),
    );
    if timer_next_run_is_stale(next_run_at_utc.as_deref(), &now_utc_text) {
        let has_cron = record
            .cron_expression
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
        next_run_at_utc = if has_cron {
            timer_compute_next_run_at_utc(
                &record.run_at_utc,
                record.cron_expression.as_deref(),
                record.end_at_utc.as_deref(),
                Some(&now_utc_text),
            )
        } else {
            None
        };
        runtime_log_debug(format!(
            "[定时器] 补算下一次触发 定时器={} 结果={} 原因=历史行已过期",
            record.timer_id,
            next_run_at_utc.as_deref().unwrap_or("<停用>")
        ));
    }
    let updated = TimerRecordStored {
        enabled: record.enabled && next_run_at_utc.is_some(),
        next_run_at_utc,
        ..record.clone()
    };
    timer_store_write_records(data_path, &updated)?;
    Ok(updated)
}

/// 下一次触发是否已经落在过去（含刚好等于基准时刻）。
fn timer_next_run_is_stale(next_run_at_utc: Option<&str>, now_utc_text: &str) -> bool {
    let Some(next_run_at) = next_run_at_utc.and_then(parse_rfc3339_time) else {
        return false;
    };
    let Some(now) = parse_rfc3339_time(now_utc_text) else {
        return false;
    };
    next_run_at <= now
}
