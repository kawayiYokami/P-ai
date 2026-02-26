use std::path::Path;

const TERMINAL_MAX_OUTPUT_BYTES: usize = 256 * 1024;
const TERMINAL_DEFAULT_TIMEOUT_MS: u64 = 20_000;
const TERMINAL_MAX_TIMEOUT_MS: u64 = 120_000;
const TERMINAL_APPROVAL_TIMEOUT_MS: u64 = 180_000;

#[derive(Debug, Clone)]
struct TerminalShellProfile {
    kind: String,
    path: String,
    args_prefix: Vec<String>,
}

fn detect_default_terminal_shell() -> TerminalShellProfile {
    #[cfg(target_os = "windows")]
    {
        fn with_args(kind: &str, path: String, args_prefix: &[&str]) -> TerminalShellProfile {
            TerminalShellProfile {
                kind: kind.to_string(),
                path,
                args_prefix: args_prefix.iter().map(|v| (*v).to_string()).collect(),
            }
        }

        fn first_existing_path(candidates: &[String]) -> Option<String> {
            candidates
                .iter()
                .find(|candidate| Path::new(candidate.as_str()).is_file())
                .cloned()
        }

        fn where_first(name: &str) -> Option<String> {
            let output = std::process::Command::new("where").arg(name).output().ok()?;
            if !output.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&output.stdout);
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && Path::new(line).is_file())
                .map(ToString::to_string)
        }

        fn derive_bash_candidates_from_git(git_exe: &str) -> Vec<String> {
            let mut out = Vec::<String>::new();
            let git_path = PathBuf::from(git_exe);
            let Some(cmd_dir) = git_path.parent() else {
                return out;
            };
            let Some(git_root) = cmd_dir.parent() else {
                return out;
            };
            out.push(git_root.join("bin").join("bash.exe").to_string_lossy().to_string());
            out.push(
                git_root
                    .join("usr")
                    .join("bin")
                    .join("bash.exe")
                    .to_string_lossy()
                    .to_string(),
            );
            out
        }

        let mut git_bash_candidates = vec![
            r"C:\Program Files\Git\bin\bash.exe".to_string(),
            r"C:\Program Files\Git\usr\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\usr\bin\bash.exe".to_string(),
        ];
        if let Some(git_path) = where_first("git") {
            git_bash_candidates.extend(derive_bash_candidates_from_git(&git_path));
        }
        if let Some(path) = where_first("bash") {
            git_bash_candidates.push(path);
        }

        if let Some(path) = first_existing_path(&git_bash_candidates) {
            return with_args("git-bash", path, &["-lc"]);
        }

        return TerminalShellProfile {
            kind: "missing-git-bash".to_string(),
            path: String::new(),
            args_prefix: Vec::new(),
        };
    }

    #[cfg(target_os = "macos")]
    {
        let zsh = Path::new("/bin/zsh");
        if zsh.is_file() {
            return TerminalShellProfile {
                kind: "zsh".to_string(),
                path: zsh.to_string_lossy().to_string(),
                args_prefix: vec!["-lc".to_string()],
            };
        }
        let bash = Path::new("/bin/bash");
        if bash.is_file() {
            return TerminalShellProfile {
                kind: "bash".to_string(),
                path: bash.to_string_lossy().to_string(),
                args_prefix: vec!["-lc".to_string()],
            };
        }
        return TerminalShellProfile {
            kind: "sh".to_string(),
            path: "/bin/sh".to_string(),
            args_prefix: vec!["-lc".to_string()],
        };
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for candidate in ["/bin/bash", "/usr/bin/bash", "/bin/sh"] {
            if Path::new(candidate).is_file() {
                let kind = Path::new(candidate)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("sh")
                    .to_string();
                return TerminalShellProfile {
                    kind,
                    path: candidate.to_string(),
                    args_prefix: vec!["-lc".to_string()],
                };
            }
        }
        return TerminalShellProfile {
            kind: "sh".to_string(),
            path: "/bin/sh".to_string(),
            args_prefix: vec!["-lc".to_string()],
        };
    }

    #[allow(unreachable_code)]
    TerminalShellProfile {
        kind: "shell".to_string(),
        path: "sh".to_string(),
        args_prefix: vec!["-lc".to_string()],
    }
}

fn normalize_terminal_tool_session_id(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        "default-session".to_string()
    } else {
        trimmed.to_string()
    }
}

fn terminal_session_has_locked_root(state: &AppState, session_id: &str) -> bool {
    let normalized = normalize_terminal_tool_session_id(session_id);
    let root_text = {
        let guard = match state.terminal_session_roots.lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        guard.get(&normalized).cloned()
    };
    let Some(root_text) = root_text else {
        return false;
    };
    PathBuf::from(root_text).is_dir()
}

fn normalize_terminal_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    let value = timeout_ms.unwrap_or(TERMINAL_DEFAULT_TIMEOUT_MS);
    value.clamp(1, TERMINAL_MAX_TIMEOUT_MS)
}

fn normalize_terminal_path_for_compare(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        text.to_ascii_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        text
    }
}

fn path_is_within(base: &Path, target: &Path) -> bool {
    let base_norm = normalize_terminal_path_for_compare(base);
    let target_norm = normalize_terminal_path_for_compare(target);
    let separator = std::path::MAIN_SEPARATOR.to_string();
    target_norm == base_norm
        || target_norm
            .strip_prefix(&(base_norm.clone() + &separator))
            .is_some()
}

fn resolve_terminal_path(base_dir: &Path, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Path is empty.".to_string());
    }
    let normalized = normalize_terminal_path_input_for_current_platform(trimmed);
    if normalized.is_empty() {
        return Err("Path is empty.".to_string());
    }
    let raw_path = PathBuf::from(&normalized);
    let joined = if raw_path.is_absolute() {
        raw_path
    } else {
        base_dir.join(raw_path)
    };

    let canonical = joined
        .canonicalize()
        .map_err(|_| format!("Path does not exist: {}", joined.to_string_lossy()))?;
    if !canonical.is_dir() {
        return Err(format!(
            "Path is not a directory: {}",
            canonical.to_string_lossy()
        ));
    }
    Ok(canonical)
}

fn terminal_workspace_canonical(state: &AppState) -> Result<PathBuf, String> {
    state
        .llm_workspace_path
        .canonicalize()
        .map_err(|err| format!("Resolve llm workspace failed: {err}"))
}

#[derive(Debug, Clone)]
struct TerminalWorkspaceResolved {
    name: String,
    path: PathBuf,
}

fn ensure_default_shell_workspace_in_config(config: &mut AppConfig, state: &AppState) {
    let default_path = state.llm_workspace_path.to_string_lossy().to_string();
    if config.shell_workspaces.iter().any(|w| w.built_in) {
        return;
    }
    config.shell_workspaces.insert(
        0,
        ShellWorkspaceConfig {
            name: "默认工作空间".to_string(),
            path: default_path,
            built_in: true,
        },
    );
}

fn terminal_allowed_workspaces_canonical(
    state: &AppState,
) -> Result<Vec<TerminalWorkspaceResolved>, String> {
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);
    ensure_default_shell_workspace_in_config(&mut config, state);
    let mut out = Vec::<TerminalWorkspaceResolved>::new();
    let mut seen_names = std::collections::HashSet::<String>::new();
    for raw in &config.shell_workspaces {
        let name = raw.name.trim();
        let path = raw.path.trim();
        if name.is_empty() || path.is_empty() {
            continue;
        }
        let canonical = match PathBuf::from(path).canonicalize() {
            Ok(v) if v.is_dir() => v,
            _ => continue,
        };
        let key = name.to_ascii_lowercase();
        if !seen_names.insert(key) {
            continue;
        }
        out.push(TerminalWorkspaceResolved {
            name: name.to_string(),
            path: canonical,
        });
    }
    if out.is_empty() {
        out.push(TerminalWorkspaceResolved {
            name: "默认工作空间".to_string(),
            path: terminal_workspace_canonical(state)?,
        });
    }
    Ok(out)
}

fn terminal_allowed_project_roots_canonical(state: &AppState) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::<PathBuf>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for ws in terminal_allowed_workspaces_canonical(state)? {
        let canonical = ws.path;
        let key = normalize_terminal_path_for_compare(&canonical);
        if seen.insert(key) {
            roots.push(canonical);
        }
    }

    if roots.is_empty() {
        roots.push(terminal_workspace_canonical(state)?);
    }

    Ok(roots)
}

fn terminal_prompt_trusted_roots_block(state: &AppState, selected_api: &ApiConfig) -> Option<String> {
    let terminal_enabled = selected_api.enable_tools
        && selected_api
            .tools
            .iter()
            .any(|tool| {
                tool.enabled
                    && matches!(
                        tool.id.as_str(),
                        "shell-exec" | "shell-switch-workspace"
                    )
            });
    if !terminal_enabled {
        return None;
    }

    let workspaces = terminal_allowed_workspaces_canonical(state).ok()?;
    if workspaces.is_empty() {
        return None;
    }

    let mut lines = Vec::<String>::new();
    lines.push("[SHELL WORKSPACE 约束]".to_string());
    lines.push("你只能基于“工作空间名称”进行切换，不要使用路径。".to_string());
    lines.push("禁止在命令中使用绝对路径。".to_string());
    lines.push("禁止在命令中使用绝对路径。".to_string());
    lines.push("禁止在命令中使用绝对路径。".to_string());
    lines.push("可用工作空间：".to_string());
    for (index, ws) in workspaces.iter().enumerate() {
        lines.push(format!("{}. {}", index + 1, ws.name));
    }
    lines.push(
        "切换请调用 shell_switch_workspace(workspaceName)；执行请调用 shell_exec(command)。"
            .to_string(),
    );
    Some(lines.join("\n"))
}

fn terminal_default_session_root_canonical(state: &AppState) -> Result<PathBuf, String> {
    let allowed = terminal_allowed_project_roots_canonical(state)?;
    allowed
        .into_iter()
        .next()
        .ok_or_else(|| "No terminal project root available".to_string())
}

fn terminal_session_root_canonical(state: &AppState, session_id: &str) -> Result<PathBuf, String> {
    let default_root = terminal_default_session_root_canonical(state)?;
    let root_text = {
        let guard = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        guard.get(session_id).cloned()
    };
    let Some(root_text) = root_text else {
        return Ok(default_root);
    };

    let root = PathBuf::from(root_text);
    match root.canonicalize() {
        Ok(path) if path.is_dir() => {
            Ok(path)
        }
        _ => Ok(default_root),
    }
}

fn ensure_terminal_workdir_allowed(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
) -> Result<(), String> {
    let session_root = terminal_session_root_canonical(state, session_id)?;
    if path_is_within(&session_root, cwd) {
        return Ok(());
    }
    Err(format!(
        "Working directory is outside current shell root: {}. Call shell_switch_workspace first.",
        cwd.to_string_lossy()
    ))
}

fn resolve_terminal_cwd(
    state: &AppState,
    session_id: &str,
    requested_cwd: Option<&str>,
) -> Result<PathBuf, String> {
    let session_root = terminal_session_root_canonical(state, session_id)?;
    let resolved = if let Some(raw) = requested_cwd {
        if raw.trim().is_empty() {
            session_root.clone()
        } else {
            resolve_terminal_path(&session_root, raw)?
        }
    } else {
        session_root.clone()
    };
    ensure_terminal_workdir_allowed(state, session_id, &resolved)?;
    Ok(resolved)
}

fn terminal_path_allowed(state: &AppState, session_id: &str, target: &Path) -> Result<bool, String> {
    let session_root = terminal_session_root_canonical(state, session_id)?;
    if path_is_within(&session_root, target) {
        return Ok(true);
    }
    Ok(false)
}

fn terminal_should_parse_command_paths_for_boundary_check() -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Linux/macOS rely on OS sandbox backend for hard path boundary.
        return false;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        true
    }
}

fn terminal_normalize_for_access_check(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    if let Some(parent) = path.parent() {
        if let Ok(parent_canonical) = parent.canonicalize() {
            if let Some(name) = path.file_name() {
                return parent_canonical.join(name);
            }
            return parent_canonical;
        }
    }
    path.to_path_buf()
}

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
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
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
            "Terminal approval timed out after {}ms.",
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

fn terminal_is_powershell_encoded_command(command: &str) -> bool {
    let tokens = terminal_tokenize(command);
    if tokens.is_empty() {
        return false;
    }

    let mut saw_powershell = false;
    let mut saw_encoded_flag = false;
    for token in tokens {
        let unquoted = terminal_unquote_token(&token);
        let exe_name = unquoted
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(unquoted.as_str());
        let lower = exe_name.to_ascii_lowercase();
        let lower_full = unquoted.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
        ) {
            saw_powershell = true;
        }
        if matches!(lower_full.as_str(), "-encodedcommand" | "-enc" | "-e")
            || lower_full.starts_with("-encodedcommand:")
            || lower_full.starts_with("-enc:")
            || lower_full.starts_with("-e:")
        {
            saw_encoded_flag = true;
        }
    }
    saw_powershell && saw_encoded_flag
}

fn terminal_command_block_reason(command: &str) -> Option<&'static str> {
    if terminal_is_powershell_encoded_command(command) {
        return Some("encoded command is blocked");
    }
    let lower = command.to_ascii_lowercase();
    if lower.contains("invoke-expression") || lower.contains("iex ") || lower.contains("iex(") {
        return Some("Invoke-Expression/iex is blocked");
    }
    if lower.contains("start-process")
        && (lower.contains("powershell")
            || lower.contains("pwsh")
            || lower.contains("cmd.exe")
            || lower.contains("/bin/sh")
            || lower.contains("/bin/bash"))
    {
        return Some("spawning nested shells is blocked");
    }
    None
}

fn truncate_terminal_output(bytes: &[u8]) -> (String, bool) {
    if bytes.len() <= TERMINAL_MAX_OUTPUT_BYTES {
        return (String::from_utf8_lossy(bytes).to_string(), false);
    }
    (
        String::from_utf8_lossy(&bytes[..TERMINAL_MAX_OUTPUT_BYTES]).to_string(),
        true,
    )
}

fn terminal_is_timeout_error(err: &str) -> bool {
    err.to_ascii_lowercase().contains("timed out after")
}

async fn builtin_shell_exec(
    state: &AppState,
    session_id: &str,
    command: &str,
    timeout_ms: Option<u64>,
) -> Result<Value, String> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err("shell_exec.command is empty".to_string());
    }
    #[cfg(target_os = "windows")]
    if state.terminal_shell.kind == "missing-git-bash" {
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "missing_git_bash",
            "message": "Git Bash is required for terminal tools on Windows. Please install Git for Windows and ensure bash.exe is available.",
            "sessionId": normalize_terminal_tool_session_id(session_id),
            "command": cmd
        }));
    }
    if let Some(reason) = terminal_command_block_reason(cmd) {
        return Err(format!("shell_exec blocked: {reason}"));
    }
    if terminal_command_contains_absolute_path_token(cmd) {
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "absolute_path_forbidden",
            "message": "Absolute paths are forbidden in shell commands. Use relative paths and workspace switching.",
            "sessionId": normalize_terminal_tool_session_id(session_id),
            "command": cmd,
        }));
    }

    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let allowed_project_roots = terminal_allowed_project_roots_canonical(state)?
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let session_root = terminal_session_root_canonical(state, &normalized_session)?;
    let cwd = match resolve_terminal_cwd(state, &normalized_session, None) {
        Ok(path) => path,
        Err(err) if err.contains("Call shell_switch_workspace first.") => {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "path_not_granted",
                "message": err,
                "sessionId": normalized_session,
                "rootPath": session_root.to_string_lossy().to_string(),
                "workspacePath": state.llm_workspace_path.to_string_lossy().to_string(),
                "allowedProjectRoots": allowed_project_roots,
                "cwd": "",
                "command": cmd,
            }));
        }
        Err(err) => return Err(err),
    };
    let timeout_ms = normalize_terminal_timeout_ms(timeout_ms);
    if terminal_should_parse_command_paths_for_boundary_check() {
        let ungranted_paths =
            terminal_collect_ungranted_command_paths(state, &normalized_session, &cwd, cmd)?;
        if !ungranted_paths.is_empty() {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "path_not_granted_in_command",
                "message": "Command references paths outside current shell root. Call shell_switch_workspace first.",
                "sessionId": normalized_session,
                "rootPath": session_root.to_string_lossy().to_string(),
                "workspacePath": state.llm_workspace_path.to_string_lossy().to_string(),
                "allowedProjectRoots": allowed_project_roots,
                "cwd": cwd.to_string_lossy().to_string(),
                "command": cmd,
                "ungrantedPaths": ungranted_paths
                    .iter()
                    .take(24)
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>(),
            }));
        }
    }

    match classify_terminal_write_risk(&cwd, cmd) {
        TerminalWriteRisk::None => {}
        TerminalWriteRisk::NewOnly { count } => {
            eprintln!(
                "[TOOL-DEBUG] shell_exec write-risk=NewOnly new_path_count={} session={}",
                count, normalized_session
            );
        }
        TerminalWriteRisk::Existing { paths } => {
            let mut lines = vec![
                "该命令将修改/删除已有文件，是否批准本次执行？".to_string(),
                format!("会话: {normalized_session}"),
                format!("工作目录: {}", cwd.to_string_lossy()),
                format!("命令: {cmd}"),
                "命中已有路径：".to_string(),
            ];
            for path in paths.iter().take(8) {
                lines.push(format!("- {}", path.to_string_lossy()));
            }
            if paths.len() > 8 {
                lines.push(format!("... 其余 {} 项已省略", paths.len() - 8));
            }
            let approved = terminal_request_user_approval(
                state,
                "终端执行审批",
                &lines.join("\n"),
                &normalized_session,
                "existing_write_risk",
                Some(&cwd),
                Some(cmd),
                None,
                None,
                &paths,
            )
            .await?;
            if !approved {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "user_denied_existing_file_change",
                    "message": "User denied command that may modify existing files.",
                    "sessionId": normalized_session,
                    "rootPath": session_root.to_string_lossy().to_string(),
                    "workspacePath": state.llm_workspace_path.to_string_lossy().to_string(),
                    "cwd": cwd.to_string_lossy().to_string(),
                    "command": cmd,
                }));
            }
        }
        TerminalWriteRisk::Unknown => {
            let message = format!(
                "无法判定该命令是否会修改已有文件，是否批准本次执行？\n会话: {normalized_session}\n工作目录: {}\n命令: {cmd}",
                cwd.to_string_lossy()
            );
            let approved = terminal_request_user_approval(
                state,
                "终端执行审批",
                &message,
                &normalized_session,
                "unknown_write_risk",
                Some(&cwd),
                Some(cmd),
                None,
                None,
                &[],
            )
            .await?;
            if !approved {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "user_denied_unknown_write_risk",
                    "message": "User denied command with unknown write risk.",
                    "sessionId": normalized_session,
                    "rootPath": session_root.to_string_lossy().to_string(),
                    "workspacePath": state.llm_workspace_path.to_string_lossy().to_string(),
                    "cwd": cwd.to_string_lossy().to_string(),
                    "command": cmd,
                }));
            }
        }
    }

    let execution = match sandbox_execute_command(state, &normalized_session, cmd, &cwd, timeout_ms)
        .await
    {
        Ok(execution) => execution,
        Err(err) if terminal_is_timeout_error(&err) => {
            return Ok(serde_json::json!({
                "ok": false,
                "shellKind": state.terminal_shell.kind,
                "shellPath": state.terminal_shell.path,
                "sessionId": normalized_session,
                "rootPath": session_root.to_string_lossy().to_string(),
                "workspacePath": state.llm_workspace_path.to_string_lossy().to_string(),
                "allowedProjectRoots": allowed_project_roots,
                "cwd": cwd.to_string_lossy().to_string(),
                "command": cmd,
                "exitCode": -1,
                "stdout": "",
                "stderr": err,
                "durationMs": timeout_ms,
                "timedOut": true,
                "truncated": false,
                "stdoutTruncated": false,
                "stderrTruncated": false
            }));
        }
        Err(err) => return Err(err),
    };
    let (stdout, stdout_truncated) = truncate_terminal_output(&execution.stdout);
    let (stderr, stderr_truncated) = truncate_terminal_output(&execution.stderr);

    Ok(serde_json::json!({
        "ok": execution.ok,
        "shellKind": execution.shell_kind,
        "shellPath": execution.shell_path,
        "sessionId": normalized_session,
        "rootPath": session_root.to_string_lossy().to_string(),
        "workspacePath": state.llm_workspace_path.to_string_lossy().to_string(),
        "allowedProjectRoots": allowed_project_roots,
        "cwd": cwd.to_string_lossy().to_string(),
        "command": cmd,
        "exitCode": execution.exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "durationMs": execution.duration_ms,
        "timedOut": false,
        "truncated": stdout_truncated || stderr_truncated,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated
    }))
}

async fn builtin_shell_switch_workspace(
    state: &AppState,
    session_id: &str,
    workspace_name: &str,
    reason: Option<&str>,
) -> Result<Value, String> {
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let allowed_workspaces = terminal_allowed_workspaces_canonical(state)?;
    let allowed_workspace_names = allowed_workspaces
        .iter()
        .map(|v| v.name.clone())
        .collect::<Vec<_>>();
    let current_root = terminal_session_root_canonical(state, &normalized_session)?;
    let workspace = terminal_workspace_canonical(state)?;
    let target_name = workspace_name.trim();
    if target_name.is_empty() {
        return Err("workspaceName is required".to_string());
    }
    let target = allowed_workspaces
        .iter()
        .find(|ws| ws.name.eq_ignore_ascii_case(target_name))
        .map(|ws| ws.path.clone());
    let Some(target) = target else {
        return Ok(serde_json::json!({
            "ok": false,
            "granted": false,
            "blockedReason": "workspace_not_found",
            "message": "Requested workspaceName not found.",
            "sessionId": normalized_session,
            "workspaceName": target_name,
            "rootPath": current_root.to_string_lossy().to_string(),
            "workspacePath": workspace.to_string_lossy().to_string(),
            "allowedWorkspaceNames": allowed_workspace_names,
            "reason": reason.unwrap_or("").trim(),
            "note": "Session shell root remains unchanged."
        }));
    };

    let set_session_root = |state: &AppState, session_id: &str, root: &Path| -> Result<(), String> {
        let mut roots = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        roots.insert(session_id.to_string(), root.to_string_lossy().to_string());
        Ok(())
    };

    set_session_root(state, &normalized_session, &target)?;

    Ok(serde_json::json!({
        "ok": true,
        "granted": true,
        "sessionId": normalized_session,
        "workspaceName": target_name,
        "normalizedPath": target.to_string_lossy().to_string(),
        "rootPath": target.to_string_lossy().to_string(),
        "workspacePath": workspace.to_string_lossy().to_string(),
        "allowedWorkspaceNames": allowed_workspace_names,
        "reason": reason.unwrap_or("").trim(),
        "note": "Session shell root switched to requested workspace."
    }))
}

