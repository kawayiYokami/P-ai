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

fn terminal_tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::<String>::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else if ch == '\\' {
                if let Some(next) = chars.peek().copied() {
                    if next == q || next == '\\' {
                        current.push(next);
                        chars.next();
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            } else {
                current.push(ch);
            }
            continue;
        }

        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            continue;
        }
        if matches!(ch, '>' | '<' | '|' | ';') {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            if ch == '>' && chars.peek().copied() == Some('>') {
                chars.next();
                tokens.push(">>".to_string());
            } else {
                tokens.push(ch.to_string());
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn terminal_unquote_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        if (bytes[0] == b'\'' && bytes[trimmed.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[trimmed.len() - 1] == b'"')
        {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(target_os = "windows")]
fn terminal_has_windows_drive_prefix(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    if bytes.len() == 2 {
        return true;
    }
    matches!(bytes[2], b'\\' | b'/')
}

#[cfg(not(target_os = "windows"))]
fn terminal_has_windows_drive_prefix(_token: &str) -> bool {
    false
}

fn terminal_is_explicit_path_token(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('-') {
        return false;
    }
    if trimmed.contains("://") {
        return false;
    }
    if matches!(
        trimmed,
        "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<" | "2>" | "1>"
    ) {
        return false;
    }
    if PathBuf::from(trimmed).is_absolute() {
        return true;
    }
    if terminal_has_windows_drive_prefix(trimmed) {
        return true;
    }
    trimmed.starts_with("./")
        || trimmed.starts_with(".\\")
        || trimmed.starts_with("../")
        || trimmed.starts_with("..\\")
        || trimmed.starts_with("~/")
        || trimmed.starts_with("~\\")
        || trimmed.contains('\\')
        || trimmed.contains('/')
}

fn terminal_command_contains_absolute_path_token(command: &str) -> bool {
    let tokens = terminal_tokenize(command);
    for token in tokens {
        let unquoted = terminal_unquote_token(&token);
        let trimmed = unquoted.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains("://") {
            continue;
        }
        if PathBuf::from(trimmed).is_absolute() || terminal_has_windows_drive_prefix(trimmed) {
            return true;
        }
    }
    false
}

fn terminal_collect_command_path_candidates(cwd: &Path, command: &str) -> Vec<PathBuf> {
    let tokens = terminal_tokenize(command);
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut raw_paths = Vec::<String>::new();
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
            raw_paths.push(tokens[idx + 1].clone());
            idx += 2;
            continue;
        }

        if matches!(
            lower.as_str(),
            "-path"
                | "-literalpath"
                | "--path"
                | "-file"
                | "--file"
                | "-output"
                | "--output"
        ) && tokens
            .get(idx + 1)
            .map(|next| !next.trim().is_empty())
            .unwrap_or(false)
        {
            raw_paths.push(tokens[idx + 1].clone());
            idx += 2;
            continue;
        }

        if terminal_is_explicit_path_token(&token) {
            raw_paths.push(token);
        }

        idx += 1;
    }

    let mut out = Vec::<PathBuf>::new();
    for raw in raw_paths {
        if let Some(path) = terminal_resolve_candidate_path(cwd, &raw) {
            out.push(path);
        }
    }
    terminal_dedup_paths(out)
}

fn terminal_collect_ungranted_command_paths(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
    command: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut blocked = Vec::<PathBuf>::new();
    for path in terminal_collect_command_path_candidates(cwd, command) {
        let normalized = terminal_normalize_for_access_check(&path);
        if !terminal_path_allowed(state, session_id, &normalized)? {
            blocked.push(normalized);
        }
    }
    Ok(terminal_dedup_paths(blocked))
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

fn terminal_resolve_candidate_path(cwd: &Path, raw: &str) -> Option<PathBuf> {
    let token = terminal_unquote_token(raw);
    if token.is_empty() {
        return None;
    }
    if token.starts_with('-') {
        return None;
    }
    if token.contains('*') || token.contains('?') {
        return None;
    }
    if token.contains("://") {
        return None;
    }
    if matches!(token.as_str(), "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<")
    {
        return None;
    }
    let normalized = normalize_terminal_path_input_for_current_platform(&token);
    if normalized.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(&normalized);
    let joined = if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    };
    Some(joined)
}

fn terminal_dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::<PathBuf>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for path in paths {
        let key = normalize_terminal_path_for_compare(&path);
        if seen.insert(key) {
            out.push(path);
        }
    }
    out
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
        eprintln!(
            "[TOOL-DEBUG] terminal approval request not found: {}",
            trimmed
        );
        return Ok(false);
    };

    if sender.send(approved).is_err() {
        eprintln!(
            "[TOOL-DEBUG] terminal approval receiver dropped: {}",
            trimmed
        );
        return Ok(false);
    }
    Ok(true)
}
