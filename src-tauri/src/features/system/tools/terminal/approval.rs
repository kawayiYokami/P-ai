const TERMINAL_APPROVAL_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone)]
enum TerminalWriteRisk {
    None,
    NewOnly { count: usize },
    Existing { paths: Vec<PathBuf> },
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalApprovalRequestPayload {
    request_id: String,
    title: String,
    message: String,
    approval_kind: String,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    existing_paths: Vec<String>,
    timeout_ms: u64,
}

fn terminal_has_output_redirection(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut prev = '\0';
    for ch in command.chars() {
        if ch == '\'' && !in_double && prev != '\\' {
            in_single = !in_single;
        } else if ch == '"' && !in_single && prev != '\\' {
            in_double = !in_double;
        } else if ch == '>' && !in_single && !in_double {
            return true;
        }
        prev = ch;
    }
    false
}

fn terminal_has_write_intent(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    terminal_has_output_redirection(command)
        || lower.contains("set-content")
        || lower.contains("add-content")
        || lower.contains("out-file")
        || lower.contains("remove-item")
        || lower.contains("copy-item")
        || lower.contains("move-item")
        || lower.contains("rename-item")
        || lower.contains("new-item")
        || lower.contains("clear-content")
        || lower.contains("git checkout")
        || lower.contains("git restore")
        || lower.contains("git apply")
        || lower.contains("git clean")
        || lower.contains("git reset")
        || lower.contains("git add")
        || lower.contains("git rm")
        || lower.contains("git mv")
        || lower.contains(" rm ")
        || lower.starts_with("rm ")
        || lower.contains(" del ")
        || lower.starts_with("del ")
        || lower.contains(" erase ")
        || lower.starts_with("erase ")
        || lower.contains(" mv ")
        || lower.starts_with("mv ")
        || lower.contains(" cp ")
        || lower.starts_with("cp ")
        || lower.contains(" touch ")
        || lower.starts_with("touch ")
        || lower.contains(" sed -i")
        || lower.contains(" perl -pi")
}

fn classify_terminal_write_risk(cwd: &Path, command: &str) -> TerminalWriteRisk {
    if !terminal_has_write_intent(command) {
        return TerminalWriteRisk::None;
    }
    let tokens = terminal_tokenize(command);
    if tokens.is_empty() {
        return TerminalWriteRisk::Unknown;
    }

    let mut raw_targets = Vec::<String>::new();
    let mut unknown = false;

    let mut idx = 0usize;
    while idx < tokens.len() {
        let token = terminal_unquote_token(&tokens[idx]);
        let lower = token.to_ascii_lowercase();

        if (lower == ">" || lower == ">>")
            && tokens
                .get(idx + 1)
                .map(|next| !next.trim().is_empty())
                .unwrap_or(false)
        {
            raw_targets.push(tokens[idx + 1].clone());
            idx += 2;
            continue;
        }

        if matches!(lower.as_str(), "-path" | "-literalpath")
            && tokens
                .get(idx + 1)
                .map(|next| !next.trim().is_empty())
                .unwrap_or(false)
        {
            raw_targets.push(tokens[idx + 1].clone());
            idx += 2;
            continue;
        }

        if matches!(
            lower.as_str(),
            "set-content"
                | "add-content"
                | "out-file"
                | "remove-item"
                | "copy-item"
                | "move-item"
                | "rename-item"
                | "new-item"
                | "rm"
                | "del"
                | "erase"
                | "mv"
                | "cp"
                | "touch"
                | "truncate"
        ) {
            let mut found = false;
            for next in tokens.iter().skip(idx + 1) {
                let next_trim = next.trim();
                if next_trim.is_empty() || next_trim.starts_with('-') {
                    continue;
                }
                if matches!(next_trim, "|" | "||" | "&" | "&&" | ";") {
                    break;
                }
                raw_targets.push(next.clone());
                found = true;
                break;
            }
            if !found {
                unknown = true;
            }
        }

        if lower == "git" {
            if let Some(sub) = tokens.get(idx + 1).map(|v| terminal_unquote_token(v)) {
                let sub_lower = sub.to_ascii_lowercase();
                if matches!(
                    sub_lower.as_str(),
                    "checkout" | "restore" | "apply" | "clean" | "reset" | "add" | "rm" | "mv"
                ) {
                    let mut found = false;
                    for next in tokens.iter().skip(idx + 2) {
                        let next_trim = next.trim();
                        if next_trim.is_empty() || next_trim.starts_with('-') {
                            continue;
                        }
                        if matches!(next_trim, "|" | "||" | "&" | "&&" | ";") {
                            break;
                        }
                        raw_targets.push(next.clone());
                        found = true;
                        break;
                    }
                    if !found {
                        unknown = true;
                    }
                }
            }
        }

        idx += 1;
    }

    let mut existing = Vec::<PathBuf>::new();
    let mut new_paths = Vec::<PathBuf>::new();
    for raw in raw_targets {
        let Some(path) = terminal_resolve_candidate_path(cwd, &raw) else {
            unknown = true;
            continue;
        };
        if path.exists() {
            existing.push(path);
        } else {
            new_paths.push(path);
        }
    }
    existing = terminal_dedup_paths(existing);
    new_paths = terminal_dedup_paths(new_paths);

    if !existing.is_empty() {
        return TerminalWriteRisk::Existing { paths: existing };
    }
    if !new_paths.is_empty() && !unknown {
        return TerminalWriteRisk::NewOnly {
            count: new_paths.len(),
        };
    }
    TerminalWriteRisk::Unknown
}

async fn terminal_request_user_approval(
    state: &AppState,
    title: &str,
    message: &str,
    session_id: &str,
    approval_kind: &str,
    cwd: Option<&Path>,
    command: Option<&str>,
    requested_path: Option<&Path>,
    reason: Option<&str>,
    existing_paths: &[PathBuf],
) -> Result<bool, String> {
    let request_id = Uuid::new_v4().to_string();
    let app_handle = {
        let guard = state
            .app_handle
            .lock()
            .map_err(|_| "Failed to lock app handle".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "App handle is not ready".to_string())?
    };

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    {
        let mut pending = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "Failed to lock terminal pending approvals".to_string())?;
        pending.insert(request_id.clone(), tx);
    }

    let payload = TerminalApprovalRequestPayload {
        request_id: request_id.clone(),
        title: title.to_string(),
        message: message.to_string(),
        approval_kind: approval_kind.to_string(),
        session_id: session_id.to_string(),
        cwd: cwd.map(|v| v.to_string_lossy().to_string()),
        command: command
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        requested_path: requested_path.map(|v| v.to_string_lossy().to_string()),
        reason: reason
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        existing_paths: existing_paths
            .iter()
            .take(32)
            .map(|v| v.to_string_lossy().to_string())
            .collect(),
        timeout_ms: TERMINAL_APPROVAL_TIMEOUT_MS,
    };

    if let Err(err) = app_handle.emit("easy-call:terminal-approval-request", &payload) {
        if let Ok(mut pending) = state.terminal_pending_approvals.lock() {
            pending.remove(&request_id);
        }
        return Err(format!("Emit terminal approval request failed: {err}"));
    }

    let wait = tokio::time::timeout(
        std::time::Duration::from_millis(TERMINAL_APPROVAL_TIMEOUT_MS),
        rx,
    )
    .await;

    if let Ok(mut pending) = state.terminal_pending_approvals.lock() {
        pending.remove(&request_id);
    }

    match wait {
        Ok(Ok(approved)) => Ok(approved),
        Ok(Err(_)) => Err("Terminal approval channel closed unexpectedly.".to_string()),
        Err(_) => Err(format!(
            "terminal_approval_timeout: 审核超时（{}ms）",
            TERMINAL_APPROVAL_TIMEOUT_MS
        )),
    }
}

fn resolve_terminal_approval_request(
    state: &AppState,
    request_id: &str,
    approved: bool,
) -> Result<bool, String> {
    let trimmed = request_id.trim();
    if trimmed.is_empty() {
        return Err("requestId is empty.".to_string());
    }

    let sender = {
        let mut pending = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "Failed to lock terminal pending approvals".to_string())?;
        pending.remove(trimmed)
    };

    let Some(sender) = sender else {
        runtime_log_debug(format!(
            "[TOOL-DEBUG] terminal approval request not found: {}",
            trimmed
        ));
        return Ok(false);
    };

    if sender.send(approved).is_err() {
        runtime_log_debug(format!(
            "[TOOL-DEBUG] terminal approval receiver dropped: {}",
            trimmed
        ));
        return Ok(false);
    }
    Ok(true)
}
