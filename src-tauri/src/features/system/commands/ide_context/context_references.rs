fn ide_context_normalize_time_or_now(field_name: &str, raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return now_iso();
    }
    match normalize_rfc3339_to_utc_storage(field_name, trimmed) {
        Ok(value) => value,
        Err(err) => {
            runtime_log_info(format!(
                "[IDE 上下文桥] 时间字段非法，回退当前时间: field={}, value={}, error={}",
                field_name, trimmed, err
            ));
            now_iso()
        }
    }
}

fn ide_context_timestamp_compare_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (parse_iso(left), parse_iso(right)) {
        (Some(left_time), Some(right_time)) => right_time.cmp(&left_time),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => right.cmp(left),
    }
}

fn ide_context_timestamp_is_newer(candidate: &str, current: &str) -> bool {
    if current.trim().is_empty() {
        return !candidate.trim().is_empty();
    }
    ide_context_timestamp_compare_desc(candidate, current) == std::cmp::Ordering::Less
}

fn ide_context_reference_dedup_key(item: &IdeContextReferenceItemOutput) -> String {
    let file_key = ide_context_compare_key(&item.file_path);
    let source_key = item.source.trim();
    if file_key.is_empty() && source_key.is_empty() {
        item.id.clone()
    } else if file_key.is_empty() {
        format!("{}|{}", item.id, source_key)
    } else if source_key.is_empty() {
        file_key
    } else {
        format!("{}|{}", file_key, source_key)
    }
}

fn ide_context_reference_source_priority(source: &str) -> u8 {
    match source.trim() {
        "selection" => 3,
        "visible_range" => 2,
        "active_file" => 1,
        _ => 0,
    }
}

fn ide_context_should_replace_reference(
    candidate: &IdeContextReferenceItemOutput,
    existing: &IdeContextReferenceItemOutput,
) -> bool {
    if ide_context_timestamp_is_newer(&candidate.captured_at, &existing.captured_at) {
        return true;
    }
    if ide_context_timestamp_is_newer(&existing.captured_at, &candidate.captured_at) {
        return false;
    }

    let candidate_priority = ide_context_reference_source_priority(&candidate.source);
    let existing_priority = ide_context_reference_source_priority(&existing.source);
    if candidate_priority != existing_priority {
        return candidate_priority > existing_priority;
    }

    let candidate_content_len = candidate.content.trim().chars().count();
    let existing_content_len = existing.content.trim().chars().count();
    if candidate_content_len != existing_content_len {
        return candidate_content_len > existing_content_len;
    }

    candidate.display_label < existing.display_label
}

fn ide_context_snapshot_is_expired(snapshot: &IdeContextSnapshot, now: &OffsetDateTime) -> bool {
    match parse_iso(&snapshot.updated_at) {
        Some(updated_at) => updated_at < (*now - time::Duration::seconds(IDE_CONTEXT_SNAPSHOT_TTL_SECS)),
        None => true,
    }
}

fn ide_context_prune_expired_snapshots(
    snapshots: &mut std::collections::HashMap<String, IdeContextSnapshot>,
) {
    let now = now_utc();
    snapshots.retain(|client_id, snapshot| {
        if ide_context_snapshot_is_expired(snapshot, &now) {
            runtime_log_debug(format!(
                "[IDE 上下文桥] 快照过期已清理: client_id={}, updated_at={}",
                client_id, snapshot.updated_at
            ));
            false
        } else {
            true
        }
    });
}

fn emit_ide_context_updated(state: &AppState, client_id: &str, updated_at: &str) {
    let app_handle = match state.app_handle.lock() {
        Ok(slot) => slot.clone(),
        Err(_) => None,
    };
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(
            "ide-context-updated",
            IdeContextUpdatedEvent {
                client_id: client_id.to_string(),
                updated_at: updated_at.to_string(),
            },
        );
    }
    ide_chat_broadcast_notification(
        "ideContext.updated",
        serde_json::json!({
            "clientId": client_id,
            "updatedAt": updated_at,
        }),
    );
}

fn ide_context_compare_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let normalized = normalize_terminal_path_input_for_current_platform(trimmed);
    let path = std::path::PathBuf::from(if normalized.is_empty() { trimmed } else { &normalized });
    shell_workspace_display_path(&path)
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn ide_context_display_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let normalized = normalize_terminal_path_input_for_current_platform(trimmed);
    let path = std::path::PathBuf::from(if normalized.is_empty() { trimmed } else { &normalized });
    let resolved = path.canonicalize().unwrap_or(path);
    shell_workspace_display_path(&resolved).replace('\\', "/")
}

fn ide_context_workspace_name(input: &IdeContextWorkspaceInput) -> String {
    let explicit = input.name.as_deref().map(str::trim).unwrap_or("");
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    let display_path = ide_context_display_path(&input.path);
    std::path::Path::new(&display_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or(display_path)
}

fn ide_context_path_is_within_workspace(file_path: &str, workspace_path: &str) -> bool {
    let file_key = ide_context_compare_key(file_path);
    let workspace_key = ide_context_compare_key(workspace_path);
    if file_key.is_empty() || workspace_key.is_empty() {
        return false;
    }
    file_key == workspace_key || file_key.starts_with(&(workspace_key + "/"))
}

fn ide_context_relative_display_path(file_path: &str, workspace_path: &str) -> String {
    let file_display = ide_context_display_path(file_path);
    let workspace_display = ide_context_display_path(workspace_path);
    let file_key = ide_context_compare_key(&file_display);
    let workspace_key = ide_context_compare_key(&workspace_display);
    if file_key == workspace_key {
        return std::path::Path::new(&file_display)
            .file_name()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or(file_display);
    }
    let prefix = format!("{}/", workspace_key);
    if let Some(relative_key) = file_key.strip_prefix(&prefix) {
        let relative = relative_key.replace('/', std::path::MAIN_SEPARATOR_STR);
        return relative.replace('\\', "/");
    }
    file_display
}

fn ide_context_line_suffix(start_line: Option<u32>, end_line: Option<u32>) -> String {
    match (start_line, end_line) {
        (Some(start), Some(end)) if end > start => format!(":{start}-{end}"),
        (Some(start), _) => format!(":{start}"),
        _ => String::new(),
    }
}

const IDE_CONTEXT_TEXT_BLOCK_CONTENT_LIMIT: usize = 2000;

fn ide_context_text_block(file_path: &str, reference: &IdeContextReference) -> String {
    let location = format!(
        "{}{}",
        file_path,
        ide_context_line_suffix(reference.start_line, reference.end_line)
    );
    let version = reference.version.trim();
    if reference.content.chars().count() > IDE_CONTEXT_TEXT_BLOCK_CONTENT_LIMIT {
        return if version.is_empty() {
            format!("用户引用了文件片段：{location}")
        } else {
            format!("用户引用了文件片段：{location}（版本: {version}）")
        };
    }
    // 普通活动文件只给路径，模型按路径读磁盘即可；diff 里的文件带了该版本原文，走下面的完整分支
    if reference.source.trim() == "active_file" && reference.content.trim().is_empty() {
        let mut lines = vec!["[IDE 上下文引用]".to_string(), format!("文件: {file_path}")];
        if !version.is_empty() {
            lines.push(format!("版本: {version}（diff 中查看的版本，非磁盘当前内容）"));
        }
        return lines.join("\n");
    }
    let mut lines = vec!["[IDE 上下文引用]".to_string(), format!("文件: {file_path}")];
    if !version.is_empty() {
        lines.push(format!("版本: {version}（diff 中查看的版本，非磁盘当前内容）"));
    }
    if reference.start_line.is_some() || reference.end_line.is_some() {
        let line_text = match (reference.start_line, reference.end_line) {
            (Some(start), Some(end)) if end > start => format!("{start}-{end}"),
            (Some(start), _) => start.to_string(),
            (_, Some(end)) => end.to_string(),
            _ => String::new(),
        };
        if !line_text.is_empty() {
            lines.push(format!("行号: {line_text}"));
        }
    }
    if let Some(language_id) = reference
        .language_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("语言: {language_id}"));
    }
    let source = reference.source.trim();
    if !source.is_empty() {
        lines.push(format!("来源: {source}"));
    }
    let captured_at = reference.captured_at.trim();
    if !captured_at.is_empty() {
        lines.push(format!("采集时间: {captured_at}"));
    }
    lines.push("内容:".to_string());
    lines.push(reference.content.clone());
    lines.join("\n")
}
