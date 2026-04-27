// ========== apply_patch rewind ==========

#[derive(Debug, Clone)]
struct RewindApplyPatchRecord {
    tool_call_id: String,
    input: String,
}

#[derive(Debug, Clone)]
enum ApplyPatchUndoResolvedOp {
    DeleteIfMatches {
        path: PathBuf,
        expected_content: String,
    },
    Update {
        from: PathBuf,
        to: Option<PathBuf>,
        hunks: Vec<ApplyPatchHunk>,
    },
}

fn parse_apply_patch_tool_args(raw_args: &str) -> Option<ApplyPatchToolArgs> {
    let trimmed = raw_args.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{') {
        return serde_json::from_str::<ApplyPatchToolArgs>(trimmed).ok();
    }
    if trimmed.starts_with("*** Begin Patch") {
        return Some(ApplyPatchToolArgs {
            input: trimmed.to_string(),
        });
    }
    None
}

fn apply_patch_tool_result_is_success(content: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return false;
    };
    value.get("ok").and_then(Value::as_bool).unwrap_or(false)
        && value.get("approved").and_then(Value::as_bool).unwrap_or(true)
}

fn collect_rewind_apply_patch_records(removed_messages: &[ChatMessage]) -> Vec<RewindApplyPatchRecord> {
    let mut pending = std::collections::HashMap::<String, String>::new();
    let mut out = Vec::<RewindApplyPatchRecord>::new();
    for message in removed_messages {
        let Some(events) = message.tool_call.as_ref() else {
            continue;
        };
        for event in events {
            let role = event.get("role").and_then(Value::as_str).unwrap_or_default();
            if role == "assistant" {
                let Some(calls) = event.get("tool_calls").and_then(Value::as_array) else {
                    continue;
                };
                for call in calls {
                    let name = call
                        .get("function")
                        .and_then(|value| value.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if name != "apply_patch" {
                        continue;
                    }
                    let call_id = call
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or_default()
                        .to_string();
                    if call_id.is_empty() {
                        continue;
                    }
                    let raw_args = call
                        .get("function")
                        .and_then(|value| value.get("arguments"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default();
                    let Some(args) = parse_apply_patch_tool_args(raw_args) else {
                        continue;
                    };
                    if args.input.trim().is_empty() {
                        continue;
                    }
                    pending.insert(call_id, args.input);
                }
                continue;
            }
            if role == "tool" {
                let call_id = event
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_default()
                    .to_string();
                if call_id.is_empty() {
                    continue;
                }
                let Some(input) = pending.remove(&call_id) else {
                    continue;
                };
                let content = event
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or_default();
                if !apply_patch_tool_result_is_success(content) {
                    continue;
                }
                out.push(RewindApplyPatchRecord {
                    tool_call_id: call_id,
                    input,
                });
            }
        }
    }
    out
}

fn invert_apply_patch_hunks(hunks: &[ApplyPatchHunk]) -> Vec<ApplyPatchHunk> {
    let mut out = Vec::<ApplyPatchHunk>::new();
    for hunk in hunks {
        let mut lines = Vec::<ApplyPatchLine>::new();
        for line in &hunk.lines {
            match line {
                ApplyPatchLine::Context(value) => lines.push(ApplyPatchLine::Context(value.clone())),
                ApplyPatchLine::Remove(value) => lines.push(ApplyPatchLine::Add(value.clone())),
                ApplyPatchLine::Add(value) => lines.push(ApplyPatchLine::Remove(value.clone())),
            }
        }
        out.push(ApplyPatchHunk {
            lines,
            end_of_file: hunk.end_of_file,
        });
    }
    out
}

fn build_inverse_apply_patch_ops(
    resolved: &[ApplyPatchResolvedOp],
) -> Result<Vec<ApplyPatchUndoResolvedOp>, String> {
    let mut inverse = Vec::<ApplyPatchUndoResolvedOp>::new();
    for op in resolved.iter().rev() {
        match op {
            ApplyPatchResolvedOp::Add { path, content } => {
                inverse.push(ApplyPatchUndoResolvedOp::DeleteIfMatches {
                    path: path.clone(),
                    expected_content: content.clone(),
                });
            }
            ApplyPatchResolvedOp::Delete { path } => {
                return Err(format!(
                    "补丁包含 Delete File，缺少原始内容快照，无法安全撤回：{}",
                    terminal_path_for_user(path)
                ));
            }
            ApplyPatchResolvedOp::Update { from, to, hunks } => {
                let inverse_hunks = invert_apply_patch_hunks(hunks);
                inverse.push(ApplyPatchUndoResolvedOp::Update {
                    from: to.clone().unwrap_or_else(|| from.clone()),
                    to: to.as_ref().map(|_| from.clone()),
                    hunks: inverse_hunks,
                });
            }
        }
    }
    Ok(inverse)
}

fn execute_inverse_apply_patch_ops(ops: &[ApplyPatchUndoResolvedOp]) -> Result<usize, String> {
    let mut applied = 0usize;
    for op in ops {
        match op {
            ApplyPatchUndoResolvedOp::DeleteIfMatches { path, expected_content } => {
                let metadata = std::fs::metadata(path)
                    .map_err(|_| format!("撤回失败：目标文件不存在 {}", terminal_path_for_user(path)))?;
                if !metadata.is_file() {
                    return Err(format!("撤回失败：目标不是文件 {}", terminal_path_for_user(path)));
                }
                let current_content = apply_patch_read_utf8_file(path)?;
                if &current_content != expected_content {
                    return Err(format!(
                        "撤回失败：文件已变更，无法安全删除 {}",
                        terminal_path_for_user(path)
                    ));
                }
                std::fs::remove_file(path).map_err(|err| {
                    format!("撤回失败：删除文件失败 {}: {err}", terminal_path_for_user(path))
                })?;
                applied = applied.saturating_add(1);
            }
            ApplyPatchUndoResolvedOp::Update { from, to, hunks } => {
                let old_content = apply_patch_read_utf8_file(from)?;
                let new_content = apply_patch_apply_hunks(&old_content, hunks).map_err(|err| {
                    format!("撤回失败：反向补丁应用失败 {}: {err}", terminal_path_for_user(from))
                })?;
                std::fs::write(from, new_content.as_bytes()).map_err(|err| {
                    format!("撤回失败：写入文件失败 {}: {err}", terminal_path_for_user(from))
                })?;
                if let Some(dest) = to {
                    if dest.exists()
                        && terminal_normalize_for_access_check(dest)
                            != terminal_normalize_for_access_check(from)
                    {
                        return Err(format!(
                            "撤回失败：重命名目标已存在 {}",
                            terminal_path_for_user(dest)
                        ));
                    }
                    apply_patch_write_parent_dir(dest)?;
                    std::fs::rename(from, dest).map_err(|err| {
                        format!(
                            "撤回失败：重命名失败 {} -> {}: {err}",
                            terminal_path_for_user(from),
                            terminal_path_for_user(dest)
                        )
                    })?;
                }
                applied = applied.saturating_add(1);
            }
        }
    }
    Ok(applied)
}

fn try_restore_apply_patch_record(
    state: &AppState,
    session: &str,
    cwd: &Path,
    input: &str,
) -> Result<Option<usize>, String> {
    let fingerprint = apply_patch_fingerprint(session, cwd, input);
    let Some((_record_path, record)) = apply_patch_take_latest_backup_record(&state.data_path, &fingerprint)? else {
        return Ok(None);
    };
    let restored = apply_patch_restore_backup_record(&state.data_path, &record)?;
    apply_patch_cleanup_backup_record_by_value(&state.data_path, &record)?;
    Ok(Some(restored))
}

fn try_undo_apply_patch_from_removed_messages(
    state: &AppState,
    removed_messages: &[ChatMessage],
) -> Result<usize, String> {
    let records = collect_rewind_apply_patch_records(removed_messages);
    if records.is_empty() {
        return Ok(0);
    }
    let mut undone_count = 0usize;
    for record in records.iter().rev() {
        let session = normalize_terminal_tool_session_id("");
        let cwd = resolve_terminal_cwd(state, &session, None).map_err(|err| {
            format!("撤回失败：解析补丁工作目录失败（tool_call_id={}）: {err}", record.tool_call_id)
        })?;
        if let Some(restored) = try_restore_apply_patch_record(state, &session, &cwd, &record.input)
            .map_err(|err| format!("撤回失败：恢复备份记录失败（tool_call_id={}）: {err}", record.tool_call_id))?
        {
            undone_count = undone_count.saturating_add(restored);
            continue;
        }
        let parsed = apply_patch_parse(&record.input).map_err(|err| {
            format!("撤回失败：解析原始补丁失败（tool_call_id={}）: {err}", record.tool_call_id)
        })?;
        let resolved = apply_patch_resolve_ops(&cwd, parsed).map_err(|err| {
            format!("撤回失败：补丁路径解析失败（tool_call_id={}）: {err}", record.tool_call_id)
        })?;
        let inverse_ops = build_inverse_apply_patch_ops(&resolved).map_err(|err| {
            format!("撤回失败：生成反向补丁失败（tool_call_id={}）: {err}", record.tool_call_id)
        })?;
        let applied = execute_inverse_apply_patch_ops(&inverse_ops).map_err(|err| {
            format!("撤回失败：执行反向补丁失败（tool_call_id={}）: {err}", record.tool_call_id)
        })?;
        undone_count = undone_count.saturating_add(applied);
    }
    Ok(undone_count)
}

#[cfg(test)]
mod rewind_apply_patch_tests {
    use super::*;
    use serde_json::json;

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir.canonicalize().expect("canonical temp dir")
    }

    fn make_message_with_tool_events(events: Vec<Value>) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![],
            extra_text_blocks: vec![],
            provider_meta: None,
            tool_call: Some(events),
            mcp_call: None,
        }
    }

    fn absolute_user_path(path: &Path) -> String {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn collect_records_should_only_pick_success_apply_patch() {
        let base = make_temp_dir("rewind-collect");
        let add_path = base.join("a.txt");
        let args = json!({
            "input": format!("*** Begin Patch\n*** Add File: {}\n+hello\n*** End Patch", add_path.to_string_lossy())
        })
        .to_string();
        let events = vec![
            json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "function": { "name": "apply_patch", "arguments": args }
                }]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": json!({"ok": true, "approved": true}).to_string()
            }),
        ];
        let records = collect_rewind_apply_patch_records(&[make_message_with_tool_events(events)]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool_call_id, "call_1");
    }

    #[test]
    fn backup_record_should_restore_deleted_file() {
        let root = make_temp_dir("rewind-delete");
        let data_path = root.join("config").join("app_data.json");
        std::fs::create_dir_all(root.join("config")).expect("create config");
        let file = root.join("a.txt");
        std::fs::write(&file, b"\x00\x01hello").expect("seed file");
        let record = ApplyPatchBackupRecord {
            record_id: Uuid::new_v4().to_string(),
            session_id: "s1".to_string(),
            cwd: root.to_string_lossy().to_string(),
            fingerprint: "fp".to_string(),
            created_at: now_iso(),
            entries: vec![ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::Delete,
                path: file.to_string_lossy().to_string(),
                from_path: None,
                to_path: None,
                expected_current_content: None,
                backup_blob_file: Some("delete.bin".to_string()),
            }],
        };
        std::fs::create_dir_all(apply_patch_temp_blobs_dir(&data_path)).expect("create blobs");
        std::fs::write(apply_patch_blob_path(&data_path, "delete.bin"), b"\x00\x01hello")
            .expect("write blob");
        std::fs::remove_file(&file).expect("delete file");
        let changed = apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(changed, 1);
        assert_eq!(std::fs::read(&file).expect("read restored"), b"\x00\x01hello");
    }

    #[test]
    fn backup_record_should_restore_move_and_content() {
        let root = make_temp_dir("rewind-move");
        let data_path = root.join("config").join("app_data.json");
        std::fs::create_dir_all(root.join("config")).expect("create config");
        let src = root.join("a.txt");
        let dst = root.join("b.txt");
        std::fs::write(&dst, "before\nnew\n").expect("seed moved");
        let record = ApplyPatchBackupRecord {
            record_id: Uuid::new_v4().to_string(),
            session_id: "s1".to_string(),
            cwd: root.to_string_lossy().to_string(),
            fingerprint: "fp".to_string(),
            created_at: now_iso(),
            entries: vec![ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::MoveUpdate,
                path: dst.to_string_lossy().to_string(),
                from_path: Some(src.to_string_lossy().to_string()),
                to_path: Some(dst.to_string_lossy().to_string()),
                expected_current_content: Some("before\nnew\n".to_string()),
                backup_blob_file: Some("move.bin".to_string()),
            }],
        };
        std::fs::create_dir_all(apply_patch_temp_blobs_dir(&data_path)).expect("create blobs");
        std::fs::write(apply_patch_blob_path(&data_path, "move.bin"), "before\nold\n")
            .expect("write blob");
        let changed = apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(changed, 1);
        assert!(src.exists());
        assert!(!dst.exists());
        assert_eq!(std::fs::read_to_string(&src).expect("read restored"), "before\nold\n");
    }

    #[test]
    fn inverse_should_restore_updated_file_content_for_legacy_record() {
        let base = make_temp_dir("rewind-legacy-update");
        let file = base.join("a.txt");
        std::fs::write(&file, "line1\nold\nline3\n").expect("seed file");
        let patch = format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n line1\n-old\n+new\n line3\n*** End Patch",
            absolute_user_path(&file)
        );
        let parsed = apply_patch_parse(&patch).expect("parse");
        let resolved = apply_patch_resolve_ops(&base, parsed).expect("resolve");
        let ApplyPatchResolvedOp::Update { hunks, .. } = &resolved[0] else {
            panic!("expected update");
        };
        let old = std::fs::read_to_string(&file).expect("read old");
        let updated = apply_patch_apply_hunks(&old, hunks).expect("apply forward");
        std::fs::write(&file, updated).expect("write forward");

        let inverse = build_inverse_apply_patch_ops(&resolved).expect("inverse");
        let changed = execute_inverse_apply_patch_ops(&inverse).expect("execute inverse");
        assert_eq!(changed, 1);
        assert_eq!(std::fs::read_to_string(&file).expect("read restored"), "line1\nold\nline3\n");
    }

    #[test]
    fn inverse_should_fail_on_add_file_drift_for_legacy_record() {
        let base = make_temp_dir("rewind-legacy-add-drift");
        let file = base.join("a.txt");
        std::fs::write(&file, "drift\n").expect("seed drift");
        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+hello\n*** End Patch",
            absolute_user_path(&file)
        );
        let parsed = apply_patch_parse(&patch).expect("parse");
        let resolved = apply_patch_resolve_ops(&base, parsed).expect("resolve");
        let inverse = build_inverse_apply_patch_ops(&resolved).expect("inverse");
        let err = execute_inverse_apply_patch_ops(&inverse).expect_err("should fail");
        assert!(err.contains("文件已变更") || err.contains("无法安全删除"));
    }
}
