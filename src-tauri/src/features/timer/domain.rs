// ==================== 定时器域模型 ====================
// 定时器只承载一句话：什么时候、在哪个会话、以什么方式把这句话再递进去。
// 不设标题、进度、待办、结论文档——那些都是任务式账本，已经随旧 task 体系一起退场。

/// 调度模式：直接激活目标会话，或在该会话名下起一场独立委托。
const TIMER_DISPATCH_MODE_DIRECT: &str = "direct";
const TIMER_DISPATCH_MODE_DELEGATE: &str = "delegate";

/// 调度轮询兜底间隔：没有更近的到点时按这个节奏复查（与旧任务调度一致）。
const TIMER_SCHEDULER_FALLBACK_SECONDS: u64 = 60;

/// 调度等待的硬下限：任何分支都不许零间隔连转。
/// 到点集合里可能残留刚派发失败、还没来得及退避的行，等 0 会把这拍变成单核自旋。
const TIMER_SCHEDULER_MIN_WAIT_MILLIS: u64 = 500;

/// 派发失败后的退避窗口：失败的行必须离开「到点」集合，否则会被每拍重试一次。
const TIMER_DISPATCH_RETRY_BACKOFF_SECONDS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TimerDispatchMode {
    Direct,
    Delegate,
}

impl TimerDispatchMode {
    fn as_str(&self) -> &'static str {
        match self {
            TimerDispatchMode::Direct => TIMER_DISPATCH_MODE_DIRECT,
            TimerDispatchMode::Delegate => TIMER_DISPATCH_MODE_DELEGATE,
        }
    }

    fn from_storage(value: &str) -> Self {
        match value.trim() {
            TIMER_DISPATCH_MODE_DIRECT => TimerDispatchMode::Direct,
            _ => TimerDispatchMode::Delegate,
        }
    }
}

/// 定时器记录：`state/state.sqlite` 的 `timers` 表一行。
///
/// 序列化给前端与模型工具时用 camelCase；时间字段都是本地 RFC3339（入库转 UTC）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimerRecord {
    id: String,
    /// 触发时要递进去的那一句话。
    message: String,
    /// 目标会话；`None` 表示全局定时器（在系统通知会话里起委托）。
    conversation_id: Option<String>,
    dispatch_mode: TimerDispatchMode,
    enabled: bool,
    /// 起始时间 / 单次触发时间（本地 RFC3339）。
    run_at: String,
    /// 标准 5 段 cron；为空表示只触发一次。
    cron: Option<String>,
    /// 截止时间：过了它就不再调度。
    end_at: Option<String>,
    last_triggered_at: Option<String>,
    next_run_at: Option<String>,
    created_at: String,
}

/// 定时器在库里的原始形态：时间都是 UTC 存储串，直接对应表列。
#[derive(Debug, Clone)]
struct TimerRecordStored {
    timer_id: String,
    conversation_id: Option<String>,
    message: String,
    dispatch_mode: TimerDispatchMode,
    enabled: bool,
    run_at_utc: String,
    cron_expression: Option<String>,
    end_at_utc: Option<String>,
    next_run_at_utc: Option<String>,
    last_triggered_at_utc: Option<String>,
    created_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimerCreateInput {
    message: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    dispatch_mode: Option<TimerDispatchMode>,
    #[serde(default)]
    run_at: Option<String>,
    #[serde(default)]
    cron: Option<String>,
    #[serde(default)]
    end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimerUpdateInput {
    timer_id: String,
    #[serde(default)]
    message: Option<String>,
    /// 三态：字段不出现＝不改；出现且为空串＝改成全局定时器。
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    dispatch_mode: Option<TimerDispatchMode>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    run_at: Option<String>,
    #[serde(default)]
    cron: Option<String>,
    #[serde(default)]
    end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimerDeleteInput {
    timer_id: String,
}

// ==================== 时间与 cron 计算 ====================

fn timer_normalize_run_at(value: &str) -> Result<String, String> {
    normalize_rfc3339_to_utc_storage("timer.runAt", value)
}

fn timer_normalize_end_at(value: &str) -> Result<String, String> {
    normalize_rfc3339_to_utc_storage("timer.endAt", value)
}

fn timer_weekday_number_from_local(dt: OffsetDateTime) -> u8 {
    match dt.weekday() {
        time::Weekday::Sunday => 0,
        time::Weekday::Monday => 1,
        time::Weekday::Tuesday => 2,
        time::Weekday::Wednesday => 3,
        time::Weekday::Thursday => 4,
        time::Weekday::Friday => 5,
        time::Weekday::Saturday => 6,
    }
}

fn timer_month_name_to_number(value: &str) -> Option<u8> {
    match value.trim().to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

fn timer_weekday_name_to_number(value: &str) -> Option<u8> {
    match value.trim().to_ascii_lowercase().as_str() {
        "sun" => Some(0),
        "mon" => Some(1),
        "tue" => Some(2),
        "wed" => Some(3),
        "thu" => Some(4),
        "fri" => Some(5),
        "sat" => Some(6),
        _ => None,
    }
}

fn timer_cron_field_value_from_token(
    token: &str,
    min: u8,
    max: u8,
    field_name: &str,
    allow_names: bool,
    is_weekday: bool,
) -> Result<u8, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(format!("timer.cron.{field_name} 存在空的 cron 片段"));
    }
    let parsed = if allow_names {
        if is_weekday {
            timer_weekday_name_to_number(trimmed)
        } else {
            timer_month_name_to_number(trimmed)
        }
        .or_else(|| trimmed.parse::<u8>().ok())
    } else {
        trimmed.parse::<u8>().ok()
    }
    .ok_or_else(|| format!("timer.cron.{field_name} 存在非法 cron 片段：{trimmed}"))?;
    let normalized = if is_weekday && parsed == 7 { 0 } else { parsed };
    if normalized < min || normalized > max {
        return Err(format!(
            "timer.cron.{field_name} cron 片段超出范围：{trimmed}"
        ));
    }
    Ok(normalized)
}

fn timer_cron_expand_part(
    part: &str,
    min: u8,
    max: u8,
    field_name: &str,
    allow_names: bool,
    is_weekday: bool,
) -> Result<Vec<u8>, String> {
    let trimmed = part.trim();
    if trimmed.is_empty() {
        return Err(format!("timer.cron.{field_name} 存在空的 cron 部分"));
    }
    let (base_part, step) = if let Some((base, raw_step)) = trimmed.split_once('/') {
        let parsed = raw_step
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("timer.cron.{field_name} 步长非法：{trimmed}"))?;
        (base.trim(), parsed)
    } else {
        (trimmed, 1)
    };
    let (start, end) = if base_part == "*" {
        (min, max)
    } else if let Some((raw_start, raw_end)) = base_part.split_once('-') {
        let start = timer_cron_field_value_from_token(
            raw_start,
            min,
            max,
            field_name,
            allow_names,
            is_weekday,
        )?;
        let end = timer_cron_field_value_from_token(
            raw_end,
            min,
            max,
            field_name,
            allow_names,
            is_weekday,
        )?;
        if start > end {
            return Err(format!("timer.cron.{field_name} 区间非法：{trimmed}"));
        }
        (start, end)
    } else {
        let start = timer_cron_field_value_from_token(
            base_part,
            min,
            max,
            field_name,
            allow_names,
            is_weekday,
        )?;
        (start, if trimmed.contains('/') { max } else { start })
    };
    let mut values = Vec::new();
    let mut current = start;
    while current <= end {
        values.push(current);
        let next = current as u16 + step as u16;
        if next > u8::MAX as u16 {
            break;
        }
        current = next as u8;
    }
    if values.is_empty() {
        return Err(format!("timer.cron.{field_name} 没有可用的 cron 值：{trimmed}"));
    }
    Ok(values)
}

fn timer_cron_parse_field(
    field: &str,
    min: u8,
    max: u8,
    field_name: &str,
    allow_names: bool,
    is_weekday: bool,
) -> Result<(Vec<bool>, bool), String> {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return Err(format!("timer.cron.{field_name} 不能为空"));
    }
    let mut allowed = vec![false; max as usize + 1];
    for part in trimmed.split(',') {
        for value in
            timer_cron_expand_part(part, min, max, field_name, allow_names, is_weekday)?
        {
            allowed[value as usize] = true;
        }
    }
    if !allowed.iter().skip(min as usize).any(|value| *value) {
        return Err(format!("timer.cron.{field_name} 没有可用的 cron 值"));
    }
    let unrestricted = allowed
        .iter()
        .enumerate()
        .skip(min as usize)
        .take((max - min + 1) as usize)
        .all(|(_, value)| *value);
    Ok((allowed, unrestricted))
}

#[derive(Debug, Clone)]
struct TimerCronSchedule {
    minutes: Vec<bool>,
    hours: Vec<bool>,
    days_of_month: Vec<bool>,
    months: Vec<bool>,
    days_of_week: Vec<bool>,
    dom_unrestricted: bool,
    dow_unrestricted: bool,
    normalized: String,
}

fn timer_parse_cron_expression(expression: &str) -> Result<TimerCronSchedule, String> {
    let normalized = expression.split_whitespace().collect::<Vec<_>>().join(" ");
    let parts = normalized.split(' ').collect::<Vec<_>>();
    if parts.len() != 5 {
        return Err("timer.cron 必须是标准 5 段 Linux/Unix cron 表达式".to_string());
    }
    let (minutes, _) = timer_cron_parse_field(parts[0], 0, 59, "minute", false, false)?;
    let (hours, _) = timer_cron_parse_field(parts[1], 0, 23, "hour", false, false)?;
    let (days_of_month, dom_unrestricted) =
        timer_cron_parse_field(parts[2], 1, 31, "day_of_month", false, false)?;
    let (months, _) = timer_cron_parse_field(parts[3], 1, 12, "month", true, false)?;
    let (days_of_week, dow_unrestricted) =
        timer_cron_parse_field(parts[4], 0, 6, "day_of_week", true, true)?;
    Ok(TimerCronSchedule {
        minutes,
        hours,
        days_of_month,
        months,
        days_of_week,
        dom_unrestricted,
        dow_unrestricted,
        normalized,
    })
}

fn timer_normalize_cron_expression(value: &str) -> Result<String, String> {
    Ok(timer_parse_cron_expression(value)?.normalized)
}

fn timer_cron_matches_local(schedule: &TimerCronSchedule, local: OffsetDateTime) -> bool {
    let minute = local.minute() as usize;
    let hour = local.hour() as usize;
    let day = local.day() as usize;
    let month = local.month() as u8 as usize;
    let weekday = timer_weekday_number_from_local(local) as usize;
    if !schedule.minutes.get(minute).copied().unwrap_or(false) {
        return false;
    }
    if !schedule.hours.get(hour).copied().unwrap_or(false) {
        return false;
    }
    if !schedule.months.get(month).copied().unwrap_or(false) {
        return false;
    }
    let dom_match = schedule
        .days_of_month
        .get(day)
        .copied()
        .unwrap_or(false);
    let dow_match = schedule
        .days_of_week
        .get(weekday)
        .copied()
        .unwrap_or(false);
    if schedule.dom_unrestricted && schedule.dow_unrestricted {
        return true;
    }
    if schedule.dom_unrestricted {
        return dow_match;
    }
    if schedule.dow_unrestricted {
        return dom_match;
    }
    dom_match || dow_match
}

fn timer_cron_next_after_local(
    base_local: OffsetDateTime,
    schedule: &TimerCronSchedule,
) -> Option<OffsetDateTime> {
    let base = base_local
        .replace_second(0)
        .ok()?
        .replace_nanosecond(0)
        .ok()?;
    let mut candidate = base + time::Duration::minutes(1);
    let horizon = base + time::Duration::days(366 * 5);
    while candidate <= horizon {
        if timer_cron_matches_local(schedule, candidate) {
            return Some(candidate);
        }
        candidate += time::Duration::minutes(1);
    }
    None
}

/// 下一次该在什么时候触发（UTC 存储串）；返回 `None` 表示不会再有下一次。
///
/// - 有 cron：上次触发之后的下一个匹配点；没有上次触发就是 `run_at`；
/// - 无 cron：单次触发，触发过就没有下一次；
/// - 任何情况下超过 `end_at` 都不再调度。
fn timer_compute_next_run_at_utc(
    run_at_utc: &str,
    cron_expression: Option<&str>,
    end_at_utc: Option<&str>,
    last_triggered_at_utc: Option<&str>,
) -> Option<String> {
    let run_at = parse_rfc3339_time(run_at_utc)?;
    let end_at = end_at_utc.and_then(parse_rfc3339_time);
    if let Some(end_at) = end_at {
        if end_at <= run_at {
            return None;
        }
    }
    let normalized_cron = cron_expression
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(cron_expression) = normalized_cron {
        let schedule = timer_parse_cron_expression(cron_expression).ok()?;
        if let Some(last_triggered_at_utc) = last_triggered_at_utc {
            let last_triggered = parse_rfc3339_time(last_triggered_at_utc)?;
            let next_local =
                timer_cron_next_after_local(to_local_datetime(last_triggered), &schedule)?;
            let next_utc = normalize_time_for_utc_storage(next_local).ok()?;
            let next_dt = parse_rfc3339_time(&next_utc)?;
            if next_dt < run_at {
                return normalize_time_for_utc_storage(run_at).ok();
            }
            if let Some(end_at) = end_at {
                if next_dt > end_at {
                    return None;
                }
            }
            return Some(next_utc);
        }
        if let Some(end_at) = end_at {
            if run_at > end_at {
                return None;
            }
        }
        return normalize_time_for_utc_storage(run_at).ok();
    }
    if last_triggered_at_utc.is_some() {
        return None;
    }
    if let Some(end_at) = end_at {
        if run_at > end_at {
            return None;
        }
    }
    normalize_time_for_utc_storage(run_at).ok()
}

// ==================== 视图与存储互转 ====================

fn timer_view_time(value: Option<&str>) -> Option<String> {
    value.map(format_utc_storage_time_to_local_rfc3339)
}

fn timer_record_view_from_stored(record: &TimerRecordStored) -> TimerRecord {
    TimerRecord {
        id: record.timer_id.clone(),
        message: record.message.clone(),
        conversation_id: record.conversation_id.clone(),
        dispatch_mode: record.dispatch_mode,
        enabled: record.enabled,
        run_at: format_utc_storage_time_to_local_rfc3339(&record.run_at_utc),
        cron: record
            .cron_expression
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        end_at: timer_view_time(record.end_at_utc.as_deref()),
        last_triggered_at: timer_view_time(record.last_triggered_at_utc.as_deref()),
        next_run_at: timer_view_time(record.next_run_at_utc.as_deref()),
        created_at: format_utc_storage_time_to_local_rfc3339(&record.created_at_utc),
    }
}

/// 全局定时器只能以委托方式运行：没有会话就没有可以激活的前台轮次。
fn timer_normalize_dispatch_mode(
    conversation_id: Option<&str>,
    mode: TimerDispatchMode,
) -> TimerDispatchMode {
    match timer_normalize_conversation_id(conversation_id) {
        Some(_) => mode,
        None => TimerDispatchMode::Delegate,
    }
}

fn timer_normalize_conversation_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .filter(|item| !item.eq_ignore_ascii_case(SYSTEM_NOTIFICATION_CONVERSATION_ID))
        .map(ToOwned::to_owned)
}

/// 这个会话是不是「系统通知会话」：全局定时器与全局委托都落在它身上。
fn timer_conversation_id_is_system_notification(value: &str) -> bool {
    value
        .trim()
        .eq_ignore_ascii_case(SYSTEM_NOTIFICATION_CONVERSATION_ID)
}

fn timer_is_due(record: &TimerRecordStored, now: OffsetDateTime) -> bool {
    if !record.enabled {
        return false;
    }
    record
        .next_run_at_utc
        .as_deref()
        .and_then(parse_rfc3339_time)
        .map(|next_run_at| now >= next_run_at)
        .unwrap_or(false)
}
