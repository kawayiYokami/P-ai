// ========== 时间语义统一 ==========
// 当地时间（local）：用户/LLM/UI 可见时间。
// 真实时间（UTC）：数据层存储、调度比较、跨时区稳定时间。

fn now_utc_rfc3339() -> String {
    now_utc()
        .replace_nanosecond(0)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn parse_rfc3339_time(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value.trim(), &Rfc3339).ok()
}

fn normalize_time_for_utc_storage(dt: OffsetDateTime) -> Result<String, String> {
    dt.to_offset(UtcOffset::UTC)
        .replace_nanosecond(0)
        .map_err(|err| format!("Normalize UTC time failed: {err}"))?
        .format(&Rfc3339)
        .map_err(|err| format!("Format UTC time failed: {err}"))
}

fn normalize_rfc3339_to_utc_storage(field_name: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} must not be empty"));
    }
    let parsed = parse_rfc3339_time(trimmed).ok_or_else(|| {
        format!(
            "{field_name} must be RFC3339 with timezone offset, for example 2026-03-10T09:30:00+08:00"
        )
    })?;
    normalize_time_for_utc_storage(parsed)
}

fn local_utc_offset() -> Option<UtcOffset> {
    match UtcOffset::current_local_offset() {
        Ok(offset) => Some(offset),
        Err(err) => {
            runtime_log_info(format!(
                "[时间语义] 获取本地 UTC 偏移失败，回退为 UTC 显示: {err}"
            ));
            None
        }
    }
}

fn to_local_datetime(dt: OffsetDateTime) -> OffsetDateTime {
    if let Some(offset) = local_utc_offset() {
        dt.to_offset(offset)
    } else {
        dt
    }
}

fn format_offset_datetime_to_local_text(dt: OffsetDateTime) -> String {
    let local = to_local_datetime(dt);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.year(),
        local.month() as u8,
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    )
}

fn format_offset_datetime_to_local_rfc3339(dt: OffsetDateTime) -> String {
    to_local_datetime(dt)
        .replace_nanosecond(0)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| format_offset_datetime_to_local_text(dt))
}

fn now_local_rfc3339() -> String {
    format_offset_datetime_to_local_rfc3339(now_utc())
}

fn now_local_text_seconds() -> String {
    format_offset_datetime_to_local_text(now_utc())
}

fn format_utc_storage_time_to_local_rfc3339(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(dt) = parse_rfc3339_time(trimmed) {
        return format_offset_datetime_to_local_rfc3339(dt);
    }
    trimmed.to_string()
}

fn format_utc_storage_time_to_local_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(dt) = parse_rfc3339_time(trimmed) {
        return format_offset_datetime_to_local_text(dt);
    }
    let mut normalized = trimmed.replace('T', " ");
    if let Some((head, _)) = normalized.split_once('.') {
        normalized = head.to_string();
    }
    if normalized.ends_with('Z') {
        normalized.pop();
    }
    if normalized.chars().count() > 19 {
        normalized.chars().take(19).collect::<String>()
    } else {
        normalized
    }
}
