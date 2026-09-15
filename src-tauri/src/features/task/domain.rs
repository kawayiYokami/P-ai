const TASK_DB_FILE_NAME: &str = "task_store.db";
const TASK_STATE_ACTIVE: &str = "active";
const TASK_STATE_COMPLETED: &str = "completed";
const TASK_STATE_FAILED_COMPLETED: &str = "failed_completed";
const TASK_SCHEDULER_FALLBACK_SECONDS: u64 = 60;
const TASK_MAX_BOARD_ITEMS: usize = 4;
const TASK_TARGET_SCOPE_DESKTOP: &str = "desktop";
const TASK_TARGET_SCOPE_CONTACT: &str = "contact";
const TASK_BOUND_CONVERSATION_MISSING_CONCLUSION: &str = "绑定会话不存在，任务已终止。";
const TASK_BOUND_OWNER_UNAVAILABLE_CONCLUSION: &str = "任务负责人不可用，任务已终止。";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskTriggerInputLocal {
    #[serde(
        default,
        alias = "runAt",
        alias = "runAtLocal",
        alias = "run_at_local"
    )]
    run_at: Option<String>,
    #[serde(default, alias = "cronExpression")]
    cron_expression: Option<String>,
    #[serde(
        default,
        alias = "endAt",
        alias = "endAtLocal",
        alias = "end_at_local"
    )]
    end_at: Option<String>,
    #[serde(default, alias = "everyMinutes", alias = "every_minutes", skip_serializing)]
    legacy_every_minutes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskTriggerView {
    #[serde(default, alias = "runAt", alias = "runAtLocal")]
    run_at: Option<String>,
    #[serde(default, alias = "cronExpression")]
    cron_expression: Option<String>,
    #[serde(default, alias = "everyMinutes")]
    every_minutes: Option<f64>,
    #[serde(default, alias = "endAt", alias = "endAtLocal")]
    end_at: Option<String>,
    #[serde(default, alias = "nextRunAt", alias = "nextRunAtLocal")]
    next_run_at: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskTriggerStored {
    run_at_utc: Option<String>,
    cron_expression: Option<String>,
    legacy_every_minutes: Option<f64>,
    end_at_utc: Option<String>,
    next_run_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskProgressNoteView {
    #[serde(default)]
    at_local: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskProgressNoteStored {
    #[serde(alias = "at", alias = "atUtc")]
    at_utc: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskEntry {
    task_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    order_index: i64,
    goal: String,
    why: String,
    todo: String,
    completion_state: String,
    #[serde(default)]
    completion_conclusion: String,
    progress_notes: Vec<TaskProgressNoteView>,
    trigger: TaskTriggerView,
    created_at_local: String,
    updated_at_local: String,
    #[serde(default)]
    last_triggered_at_local: Option<String>,
    #[serde(default)]
    completed_at_local: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskRecordStored {
    task_id: String,
    conversation_id: Option<String>,
    agent_id: Option<String>,
    target_scope: String,
    order_index: i64,
    title: String,
    cause: String,
    goal: String,
    flow: String,
    todos: Vec<String>,
    status_summary: String,
    completion_state: String,
    completion_conclusion: String,
    progress_notes: Vec<TaskProgressNoteStored>,
    stage_key: String,
    stage_updated_at_utc: Option<String>,
    trigger: TaskTriggerStored,
    created_at_utc: String,
    updated_at_utc: String,
    last_triggered_at_utc: Option<String>,
    completed_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskBoardSnapshot {
    tasks: Vec<TaskEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRunLogEntry {
    id: i64,
    task_id: String,
    triggered_at_local: String,
    outcome: String,
    note: String,
}

#[derive(Debug, Clone)]
struct TaskRunLogStored {
    id: i64,
    task_id: String,
    triggered_at_utc: String,
    outcome: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRunLogListInput {
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskCreateInput {
    goal: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    target_scope: Option<String>,
    #[serde(default)]
    why: String,
    #[serde(default)]
    todo: String,
    trigger: TaskTriggerInputLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdateInput {
    task_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    target_scope: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    todo: Option<String>,
    #[serde(default)]
    trigger: Option<TaskTriggerInputLocal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskCompleteInput {
    task_id: String,
    completion_state: String,
    #[serde(default)]
    completion_conclusion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskDeleteInput {
    task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskOptimizeDraftInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    title: String,
    content: String,
    #[serde(default)]
    schedule_mode: String,
    #[serde(default)]
    run_at: String,
    #[serde(default)]
    repeat_every: String,
    #[serde(default)]
    repeat_unit: String,
    #[serde(default)]
    end_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskOptimizeDraftOutput {
    title: String,
    content: String,
    schedule_mode: String,
    run_at: String,
    repeat_every: String,
    repeat_unit: String,
    end_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskGetInput {
    task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskDispatchNowInput {
    task_id: String,
}

fn task_trigger_view_from_stored(trigger: &TaskTriggerStored) -> TaskTriggerView {
    TaskTriggerView {
        run_at: trigger
            .run_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
        cron_expression: trigger.cron_expression.clone(),
        every_minutes: trigger.legacy_every_minutes,
        end_at: trigger
            .end_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
        next_run_at: trigger
            .next_run_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
    }
}

fn task_progress_note_view_from_stored(note: &TaskProgressNoteStored) -> TaskProgressNoteView {
    TaskProgressNoteView {
        at_local: format_utc_storage_time_to_local_rfc3339(&note.at_utc),
        note: note.note.clone(),
    }
}

fn task_weekday_number_from_local(dt: OffsetDateTime) -> u8 {
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

fn task_month_name_to_number(value: &str) -> Option<u8> {
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

fn task_weekday_name_to_number(value: &str) -> Option<u8> {
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

fn task_cron_field_value_from_token(
    token: &str,
    min: u8,
    max: u8,
    field_name: &str,
    allow_names: bool,
    is_weekday: bool,
) -> Result<u8, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(format!("task.trigger.{field_name} contains an empty cron token"));
    }
    let parsed = if allow_names {
        if is_weekday {
            task_weekday_name_to_number(trimmed)
        } else {
            task_month_name_to_number(trimmed)
        }
        .or_else(|| trimmed.parse::<u8>().ok())
    } else {
        trimmed.parse::<u8>().ok()
    }
    .ok_or_else(|| format!("task.trigger.{field_name} has invalid cron token: {trimmed}"))?;
    let normalized = if is_weekday && parsed == 7 { 0 } else { parsed };
    if normalized < min || normalized > max {
        return Err(format!(
            "task.trigger.{field_name} cron token out of range: {trimmed}"
        ));
    }
    Ok(normalized)
}

fn task_cron_expand_part(
    part: &str,
    min: u8,
    max: u8,
    field_name: &str,
    allow_names: bool,
    is_weekday: bool,
) -> Result<Vec<u8>, String> {
    let trimmed = part.trim();
    if trimmed.is_empty() {
        return Err(format!("task.trigger.{field_name} contains an empty cron part"));
    }
    let (base_part, step) = if let Some((base, raw_step)) = trimmed.split_once('/') {
        let parsed = raw_step
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("task.trigger.{field_name} has invalid cron step: {trimmed}"))?;
        (base.trim(), parsed)
    } else {
        (trimmed, 1)
    };
    let (start, end) = if base_part == "*" {
        (min, max)
    } else if let Some((raw_start, raw_end)) = base_part.split_once('-') {
        let start = task_cron_field_value_from_token(
            raw_start,
            min,
            max,
            field_name,
            allow_names,
            is_weekday,
        )?;
        let end = task_cron_field_value_from_token(
            raw_end,
            min,
            max,
            field_name,
            allow_names,
            is_weekday,
        )?;
        if start > end {
            return Err(format!(
                "task.trigger.{field_name} has invalid cron range: {trimmed}"
            ));
        }
        (start, end)
    } else {
        let start = task_cron_field_value_from_token(
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
        return Err(format!(
            "task.trigger.{field_name} has no effective cron values: {trimmed}"
        ));
    }
    Ok(values)
}

fn task_cron_parse_field(
    field: &str,
    min: u8,
    max: u8,
    field_name: &str,
    allow_names: bool,
    is_weekday: bool,
) -> Result<(Vec<bool>, bool), String> {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return Err(format!("task.trigger.{field_name} must not be empty"));
    }
    let mut allowed = vec![false; max as usize + 1];
    for part in trimmed.split(',') {
        for value in task_cron_expand_part(
            part,
            min,
            max,
            field_name,
            allow_names,
            is_weekday,
        )? {
            allowed[value as usize] = true;
        }
    }
    if !allowed.iter().skip(min as usize).any(|value| *value) {
        return Err(format!(
            "task.trigger.{field_name} has no usable cron values"
        ));
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
struct TaskCronSchedule {
    minutes: Vec<bool>,
    hours: Vec<bool>,
    days_of_month: Vec<bool>,
    months: Vec<bool>,
    days_of_week: Vec<bool>,
    dom_unrestricted: bool,
    dow_unrestricted: bool,
    normalized: String,
}

fn task_parse_cron_expression(expression: &str) -> Result<TaskCronSchedule, String> {
    let normalized = expression.split_whitespace().collect::<Vec<_>>().join(" ");
    let parts = normalized.split(' ').collect::<Vec<_>>();
    if parts.len() != 5 {
        return Err(
            "task.trigger.cron_expression must be a standard 5-field Linux/Unix cron expression"
                .to_string(),
        );
    }
    let (minutes, minutes_unrestricted) =
        task_cron_parse_field(parts[0], 0, 59, "cron_expression.minute", false, false)?;
    let (hours, hours_unrestricted) =
        task_cron_parse_field(parts[1], 0, 23, "cron_expression.hour", false, false)?;
    let (days_of_month, dom_unrestricted) = task_cron_parse_field(
        parts[2],
        1,
        31,
        "cron_expression.day_of_month",
        false,
        false,
    )?;
    let (months, months_unrestricted) =
        task_cron_parse_field(parts[3], 1, 12, "cron_expression.month", true, false)?;
    let (days_of_week, dow_unrestricted) = task_cron_parse_field(
        parts[4],
        0,
        6,
        "cron_expression.day_of_week",
        true,
        true,
    )?;
    let _ = minutes_unrestricted;
    let _ = hours_unrestricted;
    let _ = months_unrestricted;
    Ok(TaskCronSchedule {
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

fn task_cron_matches_local(schedule: &TaskCronSchedule, local: OffsetDateTime) -> bool {
    let minute = local.minute() as usize;
    let hour = local.hour() as usize;
    let day = local.day() as usize;
    let month = local.month() as u8 as usize;
    let weekday = task_weekday_number_from_local(local) as usize;
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

fn task_cron_next_after_local(
    base_local: OffsetDateTime,
    schedule: &TaskCronSchedule,
) -> Option<OffsetDateTime> {
    let base = base_local
        .replace_second(0)
        .ok()?
        .replace_nanosecond(0)
        .ok()?;
    let mut candidate = base + time::Duration::minutes(1);
    let horizon = base + time::Duration::days(366 * 5);
    while candidate <= horizon {
        if task_cron_matches_local(schedule, candidate) {
            return Some(candidate);
        }
        candidate += time::Duration::minutes(1);
    }
    None
}

fn task_normalize_cron_expression(value: &str) -> Result<String, String> {
    Ok(task_parse_cron_expression(value)?.normalized)
}

fn task_legacy_every_minutes_normalized(value: f64) -> Option<f64> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    if value < 1.0 {
        return Some(1.0);
    }
    Some(value)
}

fn task_exact_cron_expression_from_legacy_every_minutes(
    run_at_utc: Option<&str>,
    every_minutes: f64,
) -> Option<String> {
    let normalized_minutes = task_legacy_every_minutes_normalized(every_minutes)?.ceil() as u32;
    let run_at_local = run_at_utc
        .and_then(parse_rfc3339_time)
        .map(to_local_datetime)
        .unwrap_or_else(now_utc);
    let start_minute = run_at_local.minute() as u32;
    let start_hour = run_at_local.hour() as u32;
    if normalized_minutes == 1 {
        return Some("* * * * *".to_string());
    }
    if normalized_minutes < 60 && 60 % normalized_minutes == 0 {
        let mut minutes = Vec::<String>::new();
        let mut current = start_minute % normalized_minutes;
        while current < 60 {
            minutes.push(current.to_string());
            current += normalized_minutes;
        }
        return Some(format!("{} * * * *", minutes.join(",")));
    }
    if normalized_minutes == 60 {
        return Some(format!("{} * * * *", start_minute));
    }
    if normalized_minutes < 1440 && normalized_minutes % 60 == 0 {
        let hours_step = normalized_minutes / 60;
        if hours_step > 0 && 24 % hours_step == 0 {
            let mut hours = Vec::<String>::new();
            let mut current = start_hour % hours_step;
            while current < 24 {
                hours.push(current.to_string());
                current += hours_step;
            }
            return Some(format!("{} {} * * *", start_minute, hours.join(",")));
        }
        return None;
    }
    if normalized_minutes == 1440 {
        return Some(format!("{} {} * * *", start_minute, start_hour));
    }
    None
}

fn task_compute_next_run_at_from_legacy_every_minutes_raw(
    run_at_utc: Option<&str>,
    legacy_every_minutes: f64,
    end_at_utc: Option<&str>,
    last_triggered_at_utc: Option<&str>,
    completion_state: &str,
) -> Option<String> {
    if completion_state != TASK_STATE_ACTIVE {
        return None;
    }
    let run_at = run_at_utc.and_then(parse_rfc3339_time)?;
    let end_at = end_at_utc.and_then(parse_rfc3339_time);
    if let Some(end_at) = end_at {
        if end_at <= run_at {
            return None;
        }
    }
    let normalized_minutes = task_legacy_every_minutes_normalized(legacy_every_minutes)?;
    let next_dt = if let Some(last_triggered_at_utc) = last_triggered_at_utc {
        let last_triggered = parse_rfc3339_time(last_triggered_at_utc)?;
        let interval_ms = (normalized_minutes * 60_000.0).round() as i64;
        if interval_ms <= 0 {
            return None;
        }
        let candidate = last_triggered + time::Duration::milliseconds(interval_ms);
        if candidate < run_at {
            run_at
        } else {
            candidate
        }
    } else {
        run_at
    };
    if let Some(end_at) = end_at {
        if next_dt > end_at {
            return None;
        }
    }
    normalize_time_for_utc_storage(next_dt).ok()
}

fn task_compute_next_run_at_utc_raw(
    run_at_utc: Option<&str>,
    cron_expression: Option<&str>,
    legacy_every_minutes: Option<f64>,
    end_at_utc: Option<&str>,
    last_triggered_at_utc: Option<&str>,
    completion_state: &str,
) -> Option<String> {
    if completion_state != TASK_STATE_ACTIVE {
        return None;
    }
    let run_at = run_at_utc.and_then(parse_rfc3339_time)?;
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
        if let Some(last_triggered_at_utc) = last_triggered_at_utc {
            let last_triggered = parse_rfc3339_time(last_triggered_at_utc)?;
            let schedule = task_parse_cron_expression(cron_expression).ok()?;
            let next_local = task_cron_next_after_local(to_local_datetime(last_triggered), &schedule)?;
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
    if let Some(legacy_every_minutes) = legacy_every_minutes {
        return task_compute_next_run_at_from_legacy_every_minutes_raw(
            run_at_utc,
            legacy_every_minutes,
            end_at_utc,
            last_triggered_at_utc,
            completion_state,
        );
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

fn task_goal_from_legacy_fields(title: &str, goal: &str) -> String {
    let normalized_goal = goal.trim();
    if !normalized_goal.is_empty() {
        return normalized_goal.to_string();
    }
    title.trim().to_string()
}

fn task_why_from_legacy_record(record: &TaskRecordStored) -> String {
    let normalized_goal = task_goal_from_legacy_fields(&record.title, &record.goal);
    let normalized_title = record.title.trim();
    let mut parts = Vec::<String>::new();
    if !record.cause.trim().is_empty() {
        parts.push(record.cause.trim().to_string());
    }
    if !normalized_title.is_empty() && normalized_title != normalized_goal {
        parts.push(format!("原标题：{}", normalized_title));
    }
    if !record.flow.trim().is_empty() {
        parts.push(format!("原流程：{}", record.flow.trim()));
    }
    if !record.stage_key.trim().is_empty() {
        parts.push(format!("原阶段：{}", record.stage_key.trim()));
    }
    parts.join("\n")
}

fn task_todo_from_legacy_fields(status_summary: &str, todos: &[String]) -> String {
    let normalized_status = status_summary.trim();
    let normalized_todos = todos
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let mut parts = Vec::<String>::new();
    if !normalized_status.is_empty() {
        parts.push(normalized_status.to_string());
    }
    if !normalized_todos.is_empty() {
        let joined = normalized_todos.join("；");
        if normalized_status.is_empty() {
            parts.push(joined);
        } else if joined == normalized_status || format!("待办：{}", joined) == normalized_status {
            // New task records persist the simplified todo into both legacy fields.
            // Keep the user-facing text single-line instead of echoing the same todo twice.
        } else {
            parts.push(format!("待办：{}", joined));
        }
    }
    parts.join("\n")
}

fn task_legacy_title_from_goal(goal: &str) -> String {
    goal.trim().to_string()
}

fn task_legacy_goal_from_goal(goal: &str) -> String {
    goal.trim().to_string()
}

fn task_legacy_cause_from_why(why: &str) -> String {
    why.trim().to_string()
}

fn task_legacy_flow_from_why(_why: &str) -> String {
    String::new()
}

fn task_legacy_todos_from_todo(todo: &str) -> Vec<String> {
    let normalized = todo.trim();
    if normalized.is_empty() {
        return Vec::new();
    }
    vec![normalized.to_string()]
}

fn task_legacy_status_summary_from_todo(todo: &str) -> String {
    todo.trim().to_string()
}

fn task_target_scope_normalized(value: &str) -> &'static str {
    match value.trim() {
        TASK_TARGET_SCOPE_CONTACT => TASK_TARGET_SCOPE_CONTACT,
        _ => TASK_TARGET_SCOPE_DESKTOP,
    }
}

fn task_conversation_id_is_system_notification(conversation_id: &str) -> bool {
    conversation_id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID
}

fn task_trigger_is_one_time(trigger: &TaskTriggerStored) -> bool {
    trigger
        .cron_expression
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
        && trigger.legacy_every_minutes.is_none()
}

fn task_record_is_one_time(task: &TaskRecordStored) -> bool {
    task_trigger_is_one_time(&task.trigger)
}

fn task_normalize_bound_conversation_id(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .unwrap_or(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        .to_string()
}

fn task_entry_view_from_stored(record: &TaskRecordStored) -> TaskEntry {
    TaskEntry {
        task_id: record.task_id.clone(),
        conversation_id: Some(task_normalize_bound_conversation_id(
            record.conversation_id.as_deref(),
        )),
        agent_id: record.agent_id.clone(),
        order_index: record.order_index,
        goal: task_goal_from_legacy_fields(&record.title, &record.goal),
        why: task_why_from_legacy_record(record),
        todo: task_todo_from_legacy_fields(&record.status_summary, &record.todos),
        completion_state: record.completion_state.clone(),
        completion_conclusion: record.completion_conclusion.clone(),
        progress_notes: record
            .progress_notes
            .iter()
            .map(task_progress_note_view_from_stored)
            .collect(),
        trigger: task_trigger_view_from_stored(&record.trigger),
        created_at_local: format_utc_storage_time_to_local_rfc3339(&record.created_at_utc),
        updated_at_local: format_utc_storage_time_to_local_rfc3339(&record.updated_at_utc),
        last_triggered_at_local: record
            .last_triggered_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
        completed_at_local: record
            .completed_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
    }
}

fn task_run_log_view_from_stored(record: &TaskRunLogStored) -> TaskRunLogEntry {
    TaskRunLogEntry {
        id: record.id,
        task_id: record.task_id.clone(),
        triggered_at_local: format_utc_storage_time_to_local_rfc3339(&record.triggered_at_utc),
        outcome: record.outcome.clone(),
        note: record.note.clone(),
    }
}
