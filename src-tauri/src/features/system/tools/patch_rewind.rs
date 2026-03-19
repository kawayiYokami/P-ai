// ========== apply_patch rewind ==========

#[derive(Debug, Clone)]
struct RewindApplyPatchRecord {
    tool_call_id: String,
    input: String,
    session_id: Option<String>,
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
            session_id: None,
        });
    }
    None
}

fn apply_patch_tool_result_is_success(content: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return false;
    };
    value
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && value
            .get("approved")
            .and_then(Value::as_bool)
            .unwrap_or(true)
}

fn collect_rewind_apply_patch_records(removed_messages: &[ChatMessage]) -> Vec<RewindApplyPatchRecord> {
    let mut pending = std::collections::HashMap::<String, (String, Option<String>)>::new();
    let mut out = Vec::<RewindApplyPatchRecord>::new();
    for message in removed_messages {
        let Some(events) = message.tool_call.as_ref() else {
            continue;
        };
        for event in events {
            let role = event
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or_default();
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
                    pending.insert(call_id, (args.input, args.session_id));
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
                let Some((input, session_id)) = pending.remove(&call_id) else {
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
                    session_id,
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
                if let Some(dest) = to {
                    inverse.push(ApplyPatchUndoResolvedOp::Update {
                        from: dest.clone(),
                        to: Some(from.clone()),
                        hunks: inverse_hunks,
                    });
                } else {
                    inverse.push(ApplyPatchUndoResolvedOp::Update {
                        from: from.clone(),
                        to: None,
                        hunks: inverse_hunks,
                    });
                }
            }
        }
    }
    Ok(inverse)
}

fn execute_inverse_apply_patch_ops(ops: &[ApplyPatchUndoResolvedOp]) -> Result<usize, String> {
    let mut applied = 0usize;
    for op in ops {
        match op {
            ApplyPatchUndoResolvedOp::DeleteIfMatches {
                path,
                expected_content,
            } => {
                let metadata = std::fs::metadata(path).map_err(|_| {
                    format!("撤回失败：目标文件不存在 {}", terminal_path_for_user(path))
                })?;
                if !metadata.is_file() {
                    return Err(format!(
                        "撤回失败：目标不是文件 {}",
                        terminal_path_for_user(path)
                    ));
                }
                let raw = std::fs::read(path).map_err(|err| {
                    format!("撤回失败：读取文件失败 {}: {err}", terminal_path_for_user(path))
                })?;
                let current_content = String::from_utf8(raw).map_err(|_| {
                    format!("撤回失败：目标不是 UTF-8 文本 {}", terminal_path_for_user(path))
                })?;
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
                let raw = std::fs::read(from).map_err(|err| {
                    format!(
                        "撤回失败：读取文件失败 {}: {err}",
                        terminal_path_for_user(from)
                    )
                })?;
                let old_content = String::from_utf8(raw).map_err(|err| {
                    format!(
                        "撤回失败：文件不是 UTF-8 文本 {}: {err}",
                        terminal_path_for_user(from)
                    )
                })?;
                let new_content = apply_patch_apply_hunks(&old_content, hunks).map_err(|err| {
                    format!("撤回失败：反向补丁应用失败 {}: {err}", terminal_path_for_user(from))
                })?;
                std::fs::write(from, new_content.as_bytes()).map_err(|err| {
                    format!("撤回失败：写入文件失败 {}: {err}", terminal_path_for_user(from))
                })?;
                if let Some(dest) = to {
                    let from_norm = terminal_normalize_for_access_check(from);
                    let dest_norm = terminal_normalize_for_access_check(dest);
                    if dest.exists() && from_norm != dest_norm {
                        return Err(format!(
                            "撤回失败：重命名目标已存在 {}",
                            terminal_path_for_user(dest)
                        ));
                    }
                    if let Some(parent) = dest.parent() {
                        std::fs::create_dir_all(parent).map_err(|err| {
                            format!(
                                "撤回失败：创建目录失败 {}: {err}",
                                terminal_path_for_user(parent)
                            )
                        })?;
                    }
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
        let session = normalize_terminal_tool_session_id(record.session_id.as_deref().unwrap_or(""));
        let cwd = resolve_terminal_cwd(state, &session, None).map_err(|err| {
            format!(
                "撤回失败：解析补丁工作目录失败（tool_call_id={}）: {err}",
                record.tool_call_id
            )
        })?;
        let parsed = apply_patch_parse(&record.input).map_err(|err| {
            format!(
                "撤回失败：解析原始补丁失败（tool_call_id={}）: {err}",
                record.tool_call_id
            )
        })?;
        let resolved = apply_patch_resolve_ops(&cwd, parsed).map_err(|err| {
            format!(
                "撤回失败：补丁路径解析失败（tool_call_id={}）: {err}",
                record.tool_call_id
            )
        })?;
        let inverse_ops = build_inverse_apply_patch_ops(&resolved).map_err(|err| {
            format!(
                "撤回失败：生成反向补丁失败（tool_call_id={}）: {err}",
                record.tool_call_id
            )
        })?;
        let applied = execute_inverse_apply_patch_ops(&inverse_ops).map_err(|err| {
            format!(
                "撤回失败：执行反向补丁失败（tool_call_id={}）: {err}",
                record.tool_call_id
            )
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

    #[test]
    fn collect_records_should_only_pick_success_apply_patch() {
        let args = json!({
            "input": "*** Begin Patch\n*** Add File: a.txt\n+hello\n*** End Patch",
            "sessionId": "s1"
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
            json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_2",
                    "function": { "name": "apply_patch", "arguments": "*** Begin Patch\n*** Add File: b.txt\n+no\n*** End Patch" }
                }]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_2",
                "content": json!({"ok": false}).to_string()
            }),
            json!({
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_3",
                    "function": { "name": "exec", "arguments": "{\"command\":\"echo hi\"}" }
                }]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_3",
                "content": json!({"ok": true}).to_string()
            }),
        ];
        let records = collect_rewind_apply_patch_records(&[make_message_with_tool_events(events)]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool_call_id, "call_1");
        assert!(records[0].input.contains("*** Add File: a.txt"));
    }

    #[test]
    fn build_inverse_should_reject_delete_op() {
        let base = make_temp_dir("rewind-delete");
        let patch = "*** Begin Patch\n*** Delete File: a.txt\n*** End Patch";
        let parsed = apply_patch_parse(patch).expect("parse");
        let resolved = apply_patch_resolve_ops(&base, parsed).expect("resolve");
        let err = build_inverse_apply_patch_ops(&resolved).expect_err("should fail");
        assert!(err.contains("Delete File"));
    }

    #[test]
    fn inverse_should_restore_updated_file_content() {
        let base = make_temp_dir("rewind-update");
        let file = base.join("a.txt");
        std::fs::write(&file, "line1\nold\nline3\n").expect("seed file");
        let patch = "*** Begin Patch\n*** Update File: a.txt\n@@\n line1\n-old\n+new\n line3\n*** End Patch";
        let parsed = apply_patch_parse(patch).expect("parse");
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
        let restored = std::fs::read_to_string(&file).expect("read restored");
        assert_eq!(restored, "line1\nold\nline3\n");
    }

    #[test]
    fn inverse_should_restore_move_and_content() {
        let base = make_temp_dir("rewind-move");
        let src = base.join("a.txt");
        let dst = base.join("b.txt");
        std::fs::write(&src, "before\nold\n").expect("seed file");
        let patch = "*** Begin Patch\n*** Update File: a.txt\n*** Move to: b.txt\n@@\n before\n-old\n+new\n*** End Patch";
        let parsed = apply_patch_parse(patch).expect("parse");
        let resolved = apply_patch_resolve_ops(&base, parsed).expect("resolve");
        let ApplyPatchResolvedOp::Update { from, to, hunks } = &resolved[0] else {
            panic!("expected update");
        };
        let old = std::fs::read_to_string(from).expect("read old");
        let updated = apply_patch_apply_hunks(&old, hunks).expect("apply forward");
        std::fs::write(from, updated).expect("write forward");
        std::fs::rename(from, to.as_ref().expect("to")).expect("move forward");
        assert!(!src.exists());
        assert!(dst.exists());

        let inverse = build_inverse_apply_patch_ops(&resolved).expect("inverse");
        let changed = execute_inverse_apply_patch_ops(&inverse).expect("execute inverse");
        assert_eq!(changed, 1);
        assert!(src.exists());
        assert!(!dst.exists());
        let restored = std::fs::read_to_string(&src).expect("read restored");
        assert_eq!(restored, "before\nold\n");
    }

    #[test]
    fn inverse_should_fail_on_add_file_drift() {
        let base = make_temp_dir("rewind-add-drift");
        let file = base.join("a.txt");
        std::fs::write(&file, "drift\n").expect("seed drift file");
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+hello\n*** End Patch";
        let parsed = apply_patch_parse(patch).expect("parse");
        let resolved = apply_patch_resolve_ops(&base, parsed).expect("resolve");
        let inverse = build_inverse_apply_patch_ops(&resolved).expect("inverse");
        let err = execute_inverse_apply_patch_ops(&inverse).expect_err("should fail");
        assert!(err.contains("文件已变更") || err.contains("无法安全删除"));
    }
}
