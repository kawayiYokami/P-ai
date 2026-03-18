const APPLY_PATCH_APPROVAL_TIMEOUT_REASON: &str = "apply_patch_approval_timeout";

#[derive(Debug, Clone)]
enum ApplyPatchSafetyCheck {
    AutoApprove,
    AskUser { existing_paths: Vec<PathBuf> },
    Reject { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPatchToolArgs {
    input: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ApplyPatchHunk {
    lines: Vec<ApplyPatchLine>,
    end_of_file: bool,
}

#[derive(Debug, Clone)]
struct ApplyPatchUpdate {
    from: String,
    to: Option<String>,
    hunks: Vec<ApplyPatchHunk>,
}

#[derive(Debug, Clone)]
enum ApplyPatchOp {
    Add { path: String, lines: Vec<String> },
    Delete { path: String },
    Update(ApplyPatchUpdate),
}

#[derive(Debug, Clone)]
enum ApplyPatchLine {
    Context(String),
    Remove(String),
    Add(String),
}

#[derive(Debug, Clone)]
enum ApplyPatchResolvedOp {
    Add {
        path: PathBuf,
        content: String,
    },
    Delete {
        path: PathBuf,
    },
    Update {
        from: PathBuf,
        to: Option<PathBuf>,
        hunks: Vec<ApplyPatchHunk>,
    },
}

fn apply_patch_tool_description() -> String {
    "Apply structured patch to files in current workspace root.".to_string()
}

fn apply_patch_split_lines(input: &str) -> Vec<String> {
    input
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

fn apply_patch_parse(input: &str) -> Result<Vec<ApplyPatchOp>, String> {
    let lines = apply_patch_split_lines(input);
    if lines.is_empty() {
        return Err("补丁为空。".to_string());
    }
    if lines.first().map(|v| v.trim()) != Some("*** Begin Patch") {
        return Err("补丁必须以 `*** Begin Patch` 开始。".to_string());
    }
    let mut idx = 1usize;
    let mut ops = Vec::<ApplyPatchOp>::new();
    while idx < lines.len() {
        let line = lines[idx].trim_end();
        if line == "*** End Patch" {
            if idx + 1 != lines.len() && !lines[idx + 1..].iter().all(|v| v.trim().is_empty()) {
                return Err("`*** End Patch` 后不允许有额外内容。".to_string());
            }
            return Ok(ops);
        }
        if line.is_empty() {
            idx += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            let path = path.trim().to_string();
            idx += 1;
            let mut add_lines = Vec::<String>::new();
            while idx < lines.len() {
                let current = lines[idx].as_str();
                if current.starts_with("*** ") {
                    break;
                }
                let Some(payload) = current.strip_prefix('+') else {
                    return Err(format!("Add File 仅允许 `+` 行，第 {} 行非法。", idx + 1));
                };
                add_lines.push(payload.to_string());
                idx += 1;
            }
            if add_lines.is_empty() {
                return Err(format!("Add File `{path}` 至少需要一行 `+` 内容。"));
            }
            ops.push(ApplyPatchOp::Add {
                path,
                lines: add_lines,
            });
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            ops.push(ApplyPatchOp::Delete {
                path: path.trim().to_string(),
            });
            idx += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            let from = path.trim().to_string();
            idx += 1;
            let mut to = None::<String>;
            if idx < lines.len() && lines[idx].starts_with("*** Move to: ") {
                to = lines[idx]
                    .strip_prefix("*** Move to: ")
                    .map(|v| v.trim().to_string());
                idx += 1;
            }
            let mut hunks = Vec::<ApplyPatchHunk>::new();
            while idx < lines.len() {
                let current = lines[idx].as_str();
                if current == "*** End Patch"
                    || current.starts_with("*** Add File: ")
                    || current.starts_with("*** Delete File: ")
                    || current.starts_with("*** Update File: ")
                {
                    break;
                }
                if !current.starts_with("@@") {
                    return Err(format!(
                        "Update File `{from}` hunk 头非法，第 {} 行：`{}`",
                        idx + 1,
                        current
                    ));
                }
                idx += 1;
                let mut hunk_lines = Vec::<ApplyPatchLine>::new();
                let mut end_of_file = false;
                while idx < lines.len() {
                    let hunk_line = lines[idx].as_str();
                    if hunk_line == "*** End of File" {
                        end_of_file = true;
                        idx += 1;
                        break;
                    }
                    if hunk_line.starts_with("@@")
                        || hunk_line == "*** End Patch"
                        || hunk_line.starts_with("*** Add File: ")
                        || hunk_line.starts_with("*** Delete File: ")
                        || hunk_line.starts_with("*** Update File: ")
                    {
                        break;
                    }
                    let mut chars = hunk_line.chars();
                    let Some(prefix) = chars.next() else {
                        return Err(format!("空 hunk 行非法（第 {} 行）。", idx + 1));
                    };
                    let payload = chars.collect::<String>();
                    match prefix {
                        ' ' => hunk_lines.push(ApplyPatchLine::Context(payload)),
                        '-' => hunk_lines.push(ApplyPatchLine::Remove(payload)),
                        '+' => hunk_lines.push(ApplyPatchLine::Add(payload)),
                        _ => {
                            return Err(format!(
                                "hunk 行前缀必须是空格/+/-，第 {} 行：`{}`",
                                idx + 1,
                                hunk_line
                            ));
                        }
                    }
                    idx += 1;
                }
                if hunk_lines.is_empty() {
                    return Err(format!("Update File `{from}` 存在空 hunk。"));
                }
                hunks.push(ApplyPatchHunk {
                    lines: hunk_lines,
                    end_of_file,
                });
            }
            ops.push(ApplyPatchOp::Update(ApplyPatchUpdate { from, to, hunks }));
            continue;
        }
        return Err(format!("未知补丁头：`{line}`"));
    }
    Err("补丁缺少 `*** End Patch`。".to_string())
}

#[cfg(target_os = "windows")]
fn apply_patch_has_windows_drive_prefix(path: &str) -> bool {
    terminal_has_windows_drive_prefix(path)
}

#[cfg(not(target_os = "windows"))]
fn apply_patch_has_windows_drive_prefix(_path: &str) -> bool {
    false
}

fn apply_patch_resolve_path(base: &Path, raw: &str) -> Result<PathBuf, String> {
    let normalized = normalize_terminal_path_input_for_current_platform(raw.trim());
    if normalized.is_empty() {
        return Err("补丁路径为空。".to_string());
    }
    if PathBuf::from(&normalized).is_absolute() || apply_patch_has_windows_drive_prefix(&normalized)
    {
        return Err(format!("补丁路径必须是相对路径：`{raw}`"));
    }
    let joined = base.join(&normalized);
    let safe = terminal_normalize_for_access_check(&joined);
    if !path_is_within(base, &safe) {
        return Err(format!("补丁路径越界：`{raw}`"));
    }
    Ok(safe)
}

fn apply_patch_resolve_ops(base: &Path, ops: Vec<ApplyPatchOp>) -> Result<Vec<ApplyPatchResolvedOp>, String> {
    let mut out = Vec::<ApplyPatchResolvedOp>::new();
    for op in ops {
        match op {
            ApplyPatchOp::Add { path, lines } => out.push(ApplyPatchResolvedOp::Add {
                path: apply_patch_resolve_path(base, &path)?,
                content: lines.join("\n"),
            }),
            ApplyPatchOp::Delete { path } => out.push(ApplyPatchResolvedOp::Delete {
                path: apply_patch_resolve_path(base, &path)?,
            }),
            ApplyPatchOp::Update(update) => {
                let from = apply_patch_resolve_path(base, &update.from)?;
                let to = match update.to {
                    Some(raw) => Some(apply_patch_resolve_path(base, &raw)?),
                    None => None,
                };
                out.push(ApplyPatchResolvedOp::Update {
                    from,
                    to,
                    hunks: update.hunks,
                });
            }
        }
    }
    Ok(out)
}

fn apply_patch_collect_existing_paths(ops: &[ApplyPatchResolvedOp]) -> Vec<PathBuf> {
    let mut out = Vec::<PathBuf>::new();
    for op in ops {
        match op {
            ApplyPatchResolvedOp::Add { .. } => {}
            ApplyPatchResolvedOp::Delete { path } => out.push(path.clone()),
            ApplyPatchResolvedOp::Update { from, to, .. } => {
                out.push(from.clone());
                if let Some(next) = to {
                    out.push(next.clone());
                }
            }
        }
    }
    terminal_dedup_paths(out)
}

fn apply_patch_assess_safety(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
    ops: &[ApplyPatchResolvedOp],
) -> ApplyPatchSafetyCheck {
    if ops.is_empty() {
        return ApplyPatchSafetyCheck::Reject {
            reason: "empty patch".to_string(),
        };
    }
    if !terminal_cwd_in_agent_default_workspace(state, cwd) {
        let existing = apply_patch_collect_existing_paths(ops)
            .into_iter()
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        if !existing.is_empty() {
            return ApplyPatchSafetyCheck::AskUser {
                existing_paths: existing,
            };
        }
    }
    if !terminal_session_has_locked_root(state, session_id)
        && !terminal_cwd_in_agent_default_workspace(state, cwd)
    {
        return ApplyPatchSafetyCheck::AskUser {
            existing_paths: Vec::new(),
        };
    }
    ApplyPatchSafetyCheck::AutoApprove
}

fn apply_patch_split_file_lines(content: &str) -> (Vec<String>, bool) {
    let trailing_newline = content.ends_with('\n');
    let mut lines = content
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();
    if trailing_newline && lines.last().map(|v| v.is_empty()).unwrap_or(false) {
        let _ = lines.pop();
    }
    (lines, trailing_newline)
}

fn apply_patch_find_subsequence(source: &[String], needle: &[String]) -> Option<usize> {
    if needle.is_empty() {
        return Some(source.len());
    }
    source
        .windows(needle.len())
        .position(|window| window.iter().zip(needle).all(|(a, b)| a == b))
}

fn apply_patch_render_lines(lines: &[String], trailing_newline: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

fn apply_patch_apply_hunks(content: &str, hunks: &[ApplyPatchHunk]) -> Result<String, String> {
    let (mut lines, mut trailing_newline) = apply_patch_split_file_lines(content);
    for hunk in hunks {
        let mut old_seq = Vec::<String>::new();
        let mut new_seq = Vec::<String>::new();
        for line in &hunk.lines {
            match line {
                ApplyPatchLine::Context(v) => {
                    old_seq.push(v.clone());
                    new_seq.push(v.clone());
                }
                ApplyPatchLine::Remove(v) => old_seq.push(v.clone()),
                ApplyPatchLine::Add(v) => new_seq.push(v.clone()),
            }
        }
        let Some(start) = apply_patch_find_subsequence(&lines, &old_seq) else {
            return Err("hunk 上下文不匹配，无法应用补丁。".to_string());
        };
        let end = start + old_seq.len();
        lines.splice(start..end, new_seq);
        if hunk.end_of_file {
            trailing_newline = false;
        }
    }
    Ok(apply_patch_render_lines(&lines, trailing_newline))
}

async fn apply_patch_execute_ops(ops: &[ApplyPatchResolvedOp]) -> Result<Vec<Value>, String> {
    let mut changed = Vec::<Value>::new();
    for op in ops {
        match op {
            ApplyPatchResolvedOp::Add { path, content } => {
                if path.exists() {
                    return Err(format!("Add File 失败，文件已存在：{}", path.to_string_lossy()));
                }
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|err| format!("创建目录失败（{}）：{err}", parent.to_string_lossy()))?;
                }
                tokio::fs::write(path, content.as_bytes())
                    .await
                    .map_err(|err| format!("写入文件失败（{}）：{err}", path.to_string_lossy()))?;
                changed.push(serde_json::json!({
                    "op": "add",
                    "path": terminal_path_for_user(path),
                }));
            }
            ApplyPatchResolvedOp::Delete { path } => {
                let metadata = tokio::fs::metadata(path)
                    .await
                    .map_err(|_| format!("Delete File 失败，文件不存在：{}", path.to_string_lossy()))?;
                if !metadata.is_file() {
                    return Err(format!("Delete File 失败，目标不是文件：{}", path.to_string_lossy()));
                }
                tokio::fs::remove_file(path)
                    .await
                    .map_err(|err| format!("删除文件失败（{}）：{err}", path.to_string_lossy()))?;
                changed.push(serde_json::json!({
                    "op": "delete",
                    "path": terminal_path_for_user(path),
                }));
            }
            ApplyPatchResolvedOp::Update { from, to, hunks } => {
                let raw = tokio::fs::read(from)
                    .await
                    .map_err(|_| format!("Update File 失败，文件不存在：{}", from.to_string_lossy()))?;
                let old_content = String::from_utf8(raw)
                    .map_err(|_| format!("Update File 失败，文件不是 UTF-8 文本：{}", from.to_string_lossy()))?;
                let new_content = apply_patch_apply_hunks(&old_content, hunks)?;
                tokio::fs::write(from, new_content.as_bytes())
                    .await
                    .map_err(|err| format!("更新文件失败（{}）：{err}", from.to_string_lossy()))?;
                if let Some(dest) = to {
                    if let Some(parent) = dest.parent() {
                        tokio::fs::create_dir_all(parent)
                            .await
                            .map_err(|err| format!("创建重命名目录失败（{}）：{err}", parent.to_string_lossy()))?;
                    }
                    if dest.exists() && terminal_normalize_for_access_check(dest) != terminal_normalize_for_access_check(from) {
                        return Err(format!("重命名目标已存在：{}", dest.to_string_lossy()));
                    }
                    tokio::fs::rename(from, dest)
                        .await
                        .map_err(|err| format!("重命名失败（{} -> {}）：{err}", from.to_string_lossy(), dest.to_string_lossy()))?;
                    changed.push(serde_json::json!({
                        "op": "update_move",
                        "from": terminal_path_for_user(from),
                        "to": terminal_path_for_user(dest),
                    }));
                } else {
                    changed.push(serde_json::json!({
                        "op": "update",
                        "path": terminal_path_for_user(from),
                    }));
                }
            }
        }
    }
    Ok(changed)
}

fn apply_patch_timeout_blocked_result(
    session_id: &str,
    cwd: &Path,
    existing_paths: &[PathBuf],
) -> Value {
    serde_json::json!({
        "ok": false,
        "approved": false,
        "blockedReason": APPLY_PATCH_APPROVAL_TIMEOUT_REASON,
        "message": "审核超时：用户工具区修改需要本机确认。",
        "sessionId": session_id,
        "cwd": terminal_path_for_user(cwd),
        "existingPaths": existing_paths
            .iter()
            .map(|path| terminal_path_for_user(path))
            .collect::<Vec<_>>(),
    })
}

async fn builtin_apply_patch(
    state: &AppState,
    session_id: &str,
    input: &str,
) -> Result<Value, String> {
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let cwd = resolve_terminal_cwd(state, &normalized_session, None)?;
    let parsed = apply_patch_parse(input)?;
    let resolved = apply_patch_resolve_ops(&cwd, parsed)?;

    let safety = apply_patch_assess_safety(state, &normalized_session, &cwd, &resolved);
    match safety {
        ApplyPatchSafetyCheck::Reject { reason } => {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "rejected",
                "message": reason,
                "sessionId": normalized_session,
                "cwd": terminal_path_for_user(&cwd),
            }));
        }
        ApplyPatchSafetyCheck::AskUser { existing_paths } => {
            let mut lines = vec![
                "该补丁将在用户工具区执行，是否批准本次修改？".to_string(),
                format!("会话: {}", normalized_session),
                format!("工作目录: {}", cwd.to_string_lossy()),
                "命中已有文件：".to_string(),
            ];
            if existing_paths.is_empty() {
                lines.push("- 未识别到已存在文件，但该区域仍需确认。".to_string());
            } else {
                for path in existing_paths.iter().take(8) {
                    lines.push(format!("- {}", path.to_string_lossy()));
                }
            }
            let approved = match terminal_request_user_approval(
                state,
                "补丁执行审批",
                &lines.join("\n"),
                &normalized_session,
                "apply_patch_workspace_write",
                Some(&cwd),
                None,
                None,
                Some("用户工具区修改需要审批"),
                &existing_paths,
            )
            .await
            {
                Ok(v) => v,
                Err(err) if terminal_is_approval_timeout_error(&err) => {
                    return Ok(apply_patch_timeout_blocked_result(
                        &normalized_session,
                        &cwd,
                        &existing_paths,
                    ));
                }
                Err(err) => return Err(err),
            };
            if !approved {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "user_denied_apply_patch",
                    "message": "用户拒绝了本次补丁执行。",
                    "sessionId": normalized_session,
                    "cwd": terminal_path_for_user(&cwd),
                }));
            }
        }
        ApplyPatchSafetyCheck::AutoApprove => {}
    }

    let started = std::time::Instant::now();
    let changed = apply_patch_execute_ops(&resolved).await?;
    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    eprintln!(
        "[补丁执行] 完成 task=apply_patch session={} changed={} elapsed_ms={}",
        normalized_session,
        changed.len(),
        elapsed_ms
    );
    Ok(serde_json::json!({
        "ok": true,
        "approved": true,
        "sessionId": normalized_session,
        "cwd": terminal_path_for_user(&cwd),
        "changed": changed,
        "changedCount": changed.len(),
        "elapsedMs": elapsed_ms,
    }))
}

#[cfg(test)]
mod apply_patch_tool_tests {
    use super::*;

    #[test]
    fn parse_should_support_add_delete_update_and_move() {
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+hello\n*** Delete File: b.txt\n*** Update File: c.txt\n*** Move to: d.txt\n@@\n-old\n+new\n*** End Patch";
        let ops = apply_patch_parse(patch).expect("parse");
        assert_eq!(ops.len(), 3);
        match &ops[2] {
            ApplyPatchOp::Update(update) => {
                assert_eq!(update.from, "c.txt");
                assert_eq!(update.to.as_deref(), Some("d.txt"));
                assert_eq!(update.hunks.len(), 1);
            }
            _ => panic!("expected update op"),
        }
    }

    #[test]
    fn apply_hunks_should_replace_lines() {
        let content = "a\nb\nc\n";
        let hunks = vec![ApplyPatchHunk {
            lines: vec![
                ApplyPatchLine::Context("a".to_string()),
                ApplyPatchLine::Remove("b".to_string()),
                ApplyPatchLine::Add("B".to_string()),
                ApplyPatchLine::Context("c".to_string()),
            ],
            end_of_file: false,
        }];
        let updated = apply_patch_apply_hunks(content, &hunks).expect("apply");
        assert_eq!(updated, "a\nB\nc\n");
    }

    #[test]
    fn resolve_path_should_reject_escape() {
        let base = std::env::temp_dir().join("eca-apply-patch-tests");
        let _ = std::fs::create_dir_all(&base);
        let result = apply_patch_resolve_path(&base, "../escape.txt");
        assert!(result.is_err());
    }
}
