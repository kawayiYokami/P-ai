#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalWriteRisk {
    None,
    NewOnly { count: usize },
    Existing { paths: Vec<PathBuf> },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalPathIntent {
    Read,
    Write,
    Create,
    Delete,
    ChangeDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalPathAccess {
    path: PathBuf,
    is_absolute: bool,
    intent: TerminalPathIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalShellFamily {
    Posix,
    PowerShell,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalCommandAnalysis {
    accesses: Vec<TerminalPathAccess>,
    write_risk: TerminalWriteRisk,
    has_directory_change: bool,
    has_output_redirection: bool,
    unresolved_write_targets: bool,
}

impl TerminalCommandAnalysis {
    fn path_candidates(&self) -> Vec<TerminalCommandPathCandidate> {
        let mut deduped = Vec::<TerminalCommandPathCandidate>::new();
        let mut seen = std::collections::HashSet::<String>::new();
        for access in &self.accesses {
            let key = normalize_terminal_path_for_compare(&access.path);
            if seen.insert(key) {
                deduped.push(TerminalCommandPathCandidate {
                    path: access.path.clone(),
                    is_absolute: access.is_absolute,
                });
            }
        }
        deduped
    }

    fn write_target_paths(&self) -> Vec<PathBuf> {
        let mut out = Vec::<PathBuf>::new();
        let mut seen = std::collections::HashSet::<String>::new();
        for access in &self.accesses {
            if !matches!(
                access.intent,
                TerminalPathIntent::Write | TerminalPathIntent::Create | TerminalPathIntent::Delete
            ) {
                continue;
            }
            let key = normalize_terminal_path_for_compare(&access.path);
            if seen.insert(key) {
                out.push(access.path.clone());
            }
        }
        out
    }

    fn final_execution_cwd(&self, initial_cwd: &Path) -> PathBuf {
        self.accesses
            .iter()
            .rev()
            .find(|access| access.intent == TerminalPathIntent::ChangeDirectory)
            .map(|access| access.path.clone())
            .unwrap_or_else(|| initial_cwd.to_path_buf())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalSimpleCommand {
    argv: Vec<String>,
    output_redirections: Vec<TerminalRedirection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalRedirection {
    target: String,
    append: bool,
    fd: Option<u8>,
}

fn terminal_shell_family(shell_kind: &str) -> TerminalShellFamily {
    let lower = shell_kind.trim().to_ascii_lowercase();
    if matches!(lower.as_str(), "git-bash" | "bash" | "zsh" | "sh") {
        return TerminalShellFamily::Posix;
    }
    if lower.contains("powershell") || lower.contains("pwsh") {
        return TerminalShellFamily::PowerShell;
    }
    TerminalShellFamily::Other
}

fn terminal_lex_command(command: &str) -> Vec<String> {
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

        if matches!(ch, '>' | '<' | '|' | ';' | '&') {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            let pair = chars.peek().copied();
            let combined = match (ch, pair) {
                ('>', Some('>')) => Some(">>"),
                ('|', Some('|')) => Some("||"),
                ('&', Some('&')) => Some("&&"),
                _ => None,
            };
            if let Some(operator) = combined {
                chars.next();
                tokens.push(operator.to_string());
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

fn terminal_is_command_separator(token: &str) -> bool {
    matches!(token, "|" | "||" | "&&" | ";" | "&")
}

fn terminal_split_simple_commands(command: &str) -> Vec<TerminalSimpleCommand> {
    let tokens = terminal_lex_command(command);
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut commands = Vec::<TerminalSimpleCommand>::new();
    let mut current = TerminalSimpleCommand {
        argv: Vec::new(),
        output_redirections: Vec::new(),
    };

    let mut idx = 0usize;
    while idx < tokens.len() {
        let token = terminal_unquote_token(&tokens[idx]);
        let raw = tokens[idx].clone();
        if terminal_is_command_separator(token.as_str()) {
            if !current.argv.is_empty() || !current.output_redirections.is_empty() {
                commands.push(current);
            }
            current = TerminalSimpleCommand {
                argv: Vec::new(),
                output_redirections: Vec::new(),
            };
            idx += 1;
            continue;
        }

        if let Some(redirection) =
            terminal_parse_redirection_token(&tokens, idx).or_else(|| terminal_parse_embedded_redirection_token(&raw))
        {
            current.output_redirections.push(redirection.0);
            idx += redirection.1;
            continue;
        }

        current.argv.push(raw);
        idx += 1;
    }

    if !current.argv.is_empty() || !current.output_redirections.is_empty() {
        commands.push(current);
    }

    commands
}

fn terminal_parse_redirection_token(
    tokens: &[String],
    idx: usize,
) -> Option<(TerminalRedirection, usize)> {
    let token = terminal_unquote_token(tokens.get(idx)?);
    if !matches!(token.as_str(), ">" | ">>" | "<") {
        return None;
    }
    let target = tokens.get(idx + 1)?.clone();
    let target_trimmed = terminal_unquote_token(&target);
    if target_trimmed.is_empty() {
        return None;
    }
    Some((
        TerminalRedirection {
            target,
            append: token == ">>",
            fd: None,
        },
        2,
    ))
}

fn terminal_parse_embedded_redirection_token(raw: &str) -> Option<(TerminalRedirection, usize)> {
    let token = terminal_unquote_token(raw);
    if token.is_empty() {
        return None;
    }
    let bytes = token.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b'>' {
        return None;
    }
    let append = bytes.get(idx + 1).copied() == Some(b'>');
    let start = if append { idx + 2 } else { idx + 1 };
    if start >= bytes.len() {
        return None;
    }
    let fd = if idx == 0 {
        None
    } else {
        token[..idx].parse::<u8>().ok()
    };
    Some((
        TerminalRedirection {
            target: token[start..].to_string(),
            append,
            fd,
        },
        1,
    ))
}

fn terminal_redirection_target_is_sink(target: &str, shell_kind: &str) -> bool {
    let token = terminal_unquote_token(target);
    if token.is_empty() {
        return false;
    }
    terminal_is_virtual_sink_path(&token, shell_kind)
}

fn terminal_is_assignment_like(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return false;
    }
    let Some(eq_index) = trimmed.find('=') else {
        return false;
    };
    if eq_index == 0 {
        return false;
    }
    let name = &trimmed[..eq_index];
    name.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn terminal_skip_bash_wrappers(argv: &[String]) -> usize {
    let mut idx = 0usize;
    while idx < argv.len() && terminal_is_assignment_like(&terminal_unquote_token(&argv[idx])) {
        idx += 1;
    }

    loop {
        let Some(raw) = argv.get(idx) else {
            return idx;
        };
        let token = terminal_unquote_token(raw).to_ascii_lowercase();
        match token.as_str() {
            "env" => {
                idx += 1;
                while idx < argv.len()
                    && terminal_is_assignment_like(&terminal_unquote_token(&argv[idx]))
                {
                    idx += 1;
                }
            }
            "command" | "nohup" => idx += 1,
            "nice" => {
                idx += 1;
                while idx < argv.len()
                    && terminal_unquote_token(&argv[idx]).starts_with('-')
                {
                    idx += 1;
                }
            }
            "timeout" => {
                idx += 1;
                while idx < argv.len() {
                    let next = terminal_unquote_token(&argv[idx]);
                    if next.starts_with('-') {
                        idx += 1;
                        continue;
                    }
                    idx += 1;
                    break;
                }
            }
            _ => return idx,
        }
    }
}

fn terminal_push_access(
    accesses: &mut Vec<TerminalPathAccess>,
    cwd: &Path,
    raw: &str,
    intent: TerminalPathIntent,
) {
    let Some(path) = terminal_resolve_candidate_path(cwd, raw) else {
        return;
    };
    accesses.push(TerminalPathAccess {
        is_absolute: terminal_raw_token_is_absolute_path(raw),
        path,
        intent,
    });
}

fn terminal_nonflag_args(args: &[String]) -> Vec<String> {
    args.iter()
        .filter_map(|arg| {
            let token = terminal_unquote_token(arg);
            if token.is_empty() || token.starts_with('-') {
                return None;
            }
            Some(token)
        })
        .collect()
}

fn terminal_extract_bash_read_paths(base_cmd: &str, args: &[String]) -> Vec<String> {
    let nonflag = terminal_nonflag_args(args);
    match base_cmd {
        "ls" | "dir" | "cat" | "head" | "tail" | "sort" | "uniq" | "wc" | "less"
        | "more" | "stat" => nonflag,
        "find" => nonflag.into_iter().take(1).collect(),
        "rg" | "grep" => nonflag.into_iter().skip(1).collect(),
        "pwd" => Vec::new(),
        _ => Vec::new(),
    }
}

fn terminal_analyze_bash_simple_command(
    shell_kind: &str,
    cwd: &mut PathBuf,
    simple: &TerminalSimpleCommand,
    accesses: &mut Vec<TerminalPathAccess>,
    unresolved_write_targets: &mut bool,
    has_directory_change: &mut bool,
    has_output_redirection: &mut bool,
) {
    for redirect in &simple.output_redirections {
        *has_output_redirection = true;
        if terminal_redirection_target_is_sink(&redirect.target, shell_kind) {
            continue;
        }
        terminal_push_access(accesses, cwd, &redirect.target, TerminalPathIntent::Create);
    }

    let start_idx = terminal_skip_bash_wrappers(&simple.argv);
    let Some(base_raw) = simple.argv.get(start_idx) else {
        return;
    };
    let base_cmd = terminal_unquote_token(base_raw).to_ascii_lowercase();
    let args = &simple.argv[start_idx + 1..];

    match base_cmd.as_str() {
        "cd" => {
            *has_directory_change = true;
            let Some(target_raw) = args.first() else {
                return;
            };
            let target = terminal_unquote_token(target_raw);
            if target.is_empty()
                || target == "-"
                || target.contains('$')
                || target.contains('*')
                || target.contains('?')
            {
                *unresolved_write_targets = true;
                return;
            }
            terminal_push_access(accesses, cwd, target_raw, TerminalPathIntent::ChangeDirectory);
            if let Some(next_cwd) = terminal_resolve_candidate_path(cwd, target_raw) {
                *cwd = next_cwd;
            }
        }
        "cp" => {
            let nonflag = terminal_nonflag_args(args);
            if nonflag.len() < 2 {
                *unresolved_write_targets = true;
                return;
            }
            for source in nonflag.iter().take(nonflag.len() - 1) {
                terminal_push_access(accesses, cwd, source, TerminalPathIntent::Read);
            }
            if let Some(dest) = nonflag.last() {
                terminal_push_access(accesses, cwd, dest, TerminalPathIntent::Create);
            }
        }
        "mv" => {
            let nonflag = terminal_nonflag_args(args);
            if nonflag.len() < 2 {
                *unresolved_write_targets = true;
                return;
            }
            for source in nonflag.iter().take(nonflag.len() - 1) {
                terminal_push_access(accesses, cwd, source, TerminalPathIntent::Delete);
            }
            if let Some(dest) = nonflag.last() {
                terminal_push_access(accesses, cwd, dest, TerminalPathIntent::Create);
            }
        }
        "rm" | "rmdir" | "del" | "erase" => {
            let nonflag = terminal_nonflag_args(args);
            if nonflag.is_empty() {
                *unresolved_write_targets = true;
                return;
            }
            for path in nonflag {
                terminal_push_access(accesses, cwd, &path, TerminalPathIntent::Delete);
            }
        }
        "mkdir" | "touch" => {
            let nonflag = terminal_nonflag_args(args);
            if nonflag.is_empty() {
                *unresolved_write_targets = true;
                return;
            }
            for path in nonflag {
                terminal_push_access(accesses, cwd, &path, TerminalPathIntent::Create);
            }
        }
        "truncate" => {
            let nonflag = terminal_nonflag_args(args);
            if let Some(path) = nonflag.last() {
                terminal_push_access(accesses, cwd, path, TerminalPathIntent::Write);
            } else {
                *unresolved_write_targets = true;
            }
        }
        "sed" => {
            let lower_args = args
                .iter()
                .map(|item| terminal_unquote_token(item).to_ascii_lowercase())
                .collect::<Vec<_>>();
            if lower_args
                .iter()
                .any(|item| item == "-i" || item.starts_with("-i"))
            {
                let nonflag = terminal_nonflag_args(args);
                if let Some(path) = nonflag.last() {
                    terminal_push_access(accesses, cwd, path, TerminalPathIntent::Write);
                } else {
                    *unresolved_write_targets = true;
                }
            } else {
                for path in terminal_extract_bash_read_paths("sed", args) {
                    terminal_push_access(accesses, cwd, &path, TerminalPathIntent::Read);
                }
            }
        }
        "perl" => {
            let lower_args = args
                .iter()
                .map(|item| terminal_unquote_token(item).to_ascii_lowercase())
                .collect::<Vec<_>>();
            if lower_args
                .iter()
                .any(|item| item == "-pi" || item == "-p" || item == "-i" || item.starts_with("-pi"))
            {
                let nonflag = terminal_nonflag_args(args);
                if let Some(path) = nonflag.last() {
                    terminal_push_access(accesses, cwd, path, TerminalPathIntent::Write);
                } else {
                    *unresolved_write_targets = true;
                }
            }
        }
        _ => {
            for path in terminal_extract_bash_read_paths(base_cmd.as_str(), args) {
                terminal_push_access(accesses, cwd, &path, TerminalPathIntent::Read);
            }
        }
    }
}

fn terminal_powershell_alias_base<'a>(base_cmd: &'a str) -> &'a str {
    match base_cmd {
        "cd" | "chdir" | "sl" | "set-location" => "set-location",
        "ls" | "dir" | "gci" | "get-childitem" => "get-childitem",
        "cat" | "type" | "gc" | "get-content" => "get-content",
        "pwd" | "gl" | "get-location" => "get-location",
        "cp" | "copy" | "copy-item" => "copy-item",
        "mv" | "move" | "move-item" => "move-item",
        "rm" | "del" | "erase" | "ri" | "remove-item" => "remove-item",
        "ni" | "new-item" => "new-item",
        "sc" | "set-content" => "set-content",
        "ac" | "add-content" => "add-content",
        _ => base_cmd,
    }
}

fn terminal_collect_powershell_param_map(args: &[String]) -> (Vec<String>, std::collections::HashMap<String, Vec<String>>) {
    let mut positional = Vec::<String>::new();
    let mut params = std::collections::HashMap::<String, Vec<String>>::new();
    let mut idx = 0usize;
    while idx < args.len() {
        let raw = terminal_unquote_token(&args[idx]);
        let lower = raw.to_ascii_lowercase();
        if lower.starts_with('-') {
            let (name, inline_value) = if let Some((left, right)) = raw.split_once(':') {
                (left.to_ascii_lowercase(), Some(right.to_string()))
            } else {
                (lower, None)
            };
            if let Some(value) = inline_value {
                params.entry(name).or_default().push(value);
                idx += 1;
                continue;
            }
            if let Some(next) = args.get(idx + 1) {
                let next_raw = terminal_unquote_token(next);
                if !next_raw.starts_with('-') {
                    params.entry(name).or_default().push(next_raw);
                    idx += 2;
                    continue;
                }
            }
            params.entry(name).or_default();
            idx += 1;
            continue;
        }
        positional.push(raw);
        idx += 1;
    }
    (positional, params)
}

fn terminal_push_powershell_param_paths(
    accesses: &mut Vec<TerminalPathAccess>,
    cwd: &Path,
    params: &std::collections::HashMap<String, Vec<String>>,
    names: &[&str],
    intent: TerminalPathIntent,
) {
    for name in names {
        if let Some(values) = params.get(&name.to_ascii_lowercase()) {
            for value in values {
                terminal_push_access(accesses, cwd, value, intent);
            }
        }
    }
}

fn terminal_analyze_powershell_simple_command(
    shell_kind: &str,
    cwd: &mut PathBuf,
    simple: &TerminalSimpleCommand,
    accesses: &mut Vec<TerminalPathAccess>,
    unresolved_write_targets: &mut bool,
    has_directory_change: &mut bool,
    has_output_redirection: &mut bool,
) {
    for redirect in &simple.output_redirections {
        *has_output_redirection = true;
        if terminal_redirection_target_is_sink(&redirect.target, shell_kind) {
            continue;
        }
        terminal_push_access(accesses, cwd, &redirect.target, TerminalPathIntent::Create);
    }

    let Some(base_raw) = simple.argv.first() else {
        return;
    };
    let base_owned = terminal_unquote_token(base_raw).to_ascii_lowercase();
    let normalized_base = terminal_powershell_alias_base(base_owned.as_str()).to_string();
    let args = &simple.argv[1..];
    let (positional, params) = terminal_collect_powershell_param_map(args);

    match normalized_base.as_str() {
        "set-location" => {
            *has_directory_change = true;
            let target = params
                .get("-path")
                .and_then(|values| values.first())
                .or_else(|| params.get("-literalpath").and_then(|values| values.first()))
                .or_else(|| positional.first());
            let Some(target) = target else {
                return;
            };
            if target == "-" || target.contains('$') || target.contains('*') || target.contains('?') {
                *unresolved_write_targets = true;
                return;
            }
            terminal_push_access(accesses, cwd, target, TerminalPathIntent::ChangeDirectory);
            if let Some(next_cwd) = terminal_resolve_candidate_path(cwd, target) {
                *cwd = next_cwd;
            }
        }
        "get-childitem" | "get-content" | "test-path" | "select-string" => {
            terminal_push_powershell_param_paths(
                accesses,
                cwd,
                &params,
                &["-path", "-literalpath"],
                TerminalPathIntent::Read,
            );
            if let Some(first) = positional.first() {
                terminal_push_access(accesses, cwd, first, TerminalPathIntent::Read);
            }
        }
        "set-content" | "add-content" | "out-file" => {
            terminal_push_powershell_param_paths(
                accesses,
                cwd,
                &params,
                &["-path", "-literalpath", "-filepath", "-outfile"],
                TerminalPathIntent::Write,
            );
            if params.is_empty() {
                if let Some(first) = positional.first() {
                    terminal_push_access(accesses, cwd, first, TerminalPathIntent::Write);
                } else {
                    *unresolved_write_targets = true;
                }
            }
        }
        "new-item" => {
            terminal_push_powershell_param_paths(
                accesses,
                cwd,
                &params,
                &["-path", "-literalpath"],
                TerminalPathIntent::Create,
            );
            if params.is_empty() {
                if let Some(first) = positional.first() {
                    terminal_push_access(accesses, cwd, first, TerminalPathIntent::Create);
                } else {
                    *unresolved_write_targets = true;
                }
            }
        }
        "remove-item" => {
            terminal_push_powershell_param_paths(
                accesses,
                cwd,
                &params,
                &["-path", "-literalpath"],
                TerminalPathIntent::Delete,
            );
            if params.is_empty() {
                if let Some(first) = positional.first() {
                    terminal_push_access(accesses, cwd, first, TerminalPathIntent::Delete);
                } else {
                    *unresolved_write_targets = true;
                }
            }
        }
        "copy-item" | "move-item" => {
            let source = params
                .get("-path")
                .and_then(|values| values.first())
                .or_else(|| params.get("-literalpath").and_then(|values| values.first()))
                .or_else(|| positional.first());
            let destination = params
                .get("-destination")
                .and_then(|values| values.first())
                .or_else(|| positional.get(1));
            if let Some(source) = source {
                let intent = if normalized_base == "move-item" {
                    TerminalPathIntent::Delete
                } else {
                    TerminalPathIntent::Read
                };
                terminal_push_access(accesses, cwd, source, intent);
            }
            if let Some(destination) = destination {
                terminal_push_access(accesses, cwd, destination, TerminalPathIntent::Create);
            } else {
                *unresolved_write_targets = true;
            }
        }
        "rename-item" => {
            let target = params
                .get("-path")
                .and_then(|values| values.first())
                .or_else(|| positional.first());
            if let Some(target) = target {
                terminal_push_access(accesses, cwd, target, TerminalPathIntent::Write);
            } else {
                *unresolved_write_targets = true;
            }
        }
        "expand-archive" | "compress-archive" => {
            terminal_push_powershell_param_paths(
                accesses,
                cwd,
                &params,
                &["-destinationpath"],
                TerminalPathIntent::Create,
            );
            terminal_push_powershell_param_paths(
                accesses,
                cwd,
                &params,
                &["-path", "-literalpath"],
                TerminalPathIntent::Read,
            );
        }
        _ => {}
    }
}

fn terminal_derive_write_risk(
    accesses: &[TerminalPathAccess],
    unresolved_write_targets: bool,
) -> TerminalWriteRisk {
    let write_accesses = accesses
        .iter()
        .filter(|access| {
            matches!(
                access.intent,
                TerminalPathIntent::Write | TerminalPathIntent::Create | TerminalPathIntent::Delete
            )
        })
        .collect::<Vec<_>>();
    if write_accesses.is_empty() {
        return if unresolved_write_targets {
            TerminalWriteRisk::Unknown
        } else {
            TerminalWriteRisk::None
        };
    }

    let mut existing = Vec::<PathBuf>::new();
    let mut new_paths = Vec::<PathBuf>::new();
    let mut seen_existing = std::collections::HashSet::<String>::new();
    let mut seen_new = std::collections::HashSet::<String>::new();
    for access in write_accesses {
        let key = normalize_terminal_path_for_compare(&access.path);
        if access.path.exists() {
            if seen_existing.insert(key) {
                existing.push(access.path.clone());
            }
        } else if seen_new.insert(key) {
            new_paths.push(access.path.clone());
        }
    }

    if !existing.is_empty() {
        return TerminalWriteRisk::Existing { paths: existing };
    }
    if !new_paths.is_empty() && !unresolved_write_targets {
        return TerminalWriteRisk::NewOnly {
            count: new_paths.len(),
        };
    }
    TerminalWriteRisk::Unknown
}

fn terminal_analyze_command(
    cwd: &Path,
    command: &str,
    shell_kind: &str,
) -> TerminalCommandAnalysis {
    let family = terminal_shell_family(shell_kind);
    let mut analysis_cwd = cwd.to_path_buf();
    let mut accesses = Vec::<TerminalPathAccess>::new();
    let mut unresolved_write_targets = false;
    let mut has_directory_change = false;
    let mut has_output_redirection = false;

    for simple in terminal_split_simple_commands(command) {
        match family {
            TerminalShellFamily::PowerShell => terminal_analyze_powershell_simple_command(
                shell_kind,
                &mut analysis_cwd,
                &simple,
                &mut accesses,
                &mut unresolved_write_targets,
                &mut has_directory_change,
                &mut has_output_redirection,
            ),
            TerminalShellFamily::Posix | TerminalShellFamily::Other => {
                terminal_analyze_bash_simple_command(
                    shell_kind,
                    &mut analysis_cwd,
                    &simple,
                    &mut accesses,
                    &mut unresolved_write_targets,
                    &mut has_directory_change,
                    &mut has_output_redirection,
                )
            }
        }
    }

    let write_risk = terminal_derive_write_risk(&accesses, unresolved_write_targets);

    TerminalCommandAnalysis {
        accesses,
        write_risk,
        has_directory_change,
        has_output_redirection,
        unresolved_write_targets,
    }
}

#[cfg(test)]
mod terminal_command_analyzer_tests {
    use super::*;

    #[test]
    fn bash_should_ignore_dev_null_redirection_for_read_command() {
        let cwd = PathBuf::from("D:\\work\\demo-home");
        let analysis = terminal_analyze_command(
            &cwd,
            r#"pwd; ls -la; ls -la ./archive 2>/dev/null || true"#,
            "git-bash",
        );

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().contains("archive")));
        assert!(!analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().replace('\\', "/").contains("dev/null")));
    }

    #[test]
    fn bash_should_treat_output_redirection_to_file_as_write() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis =
            terminal_analyze_command(&cwd, r#"echo hello > ./out.txt"#, "git-bash");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::NewOnly { count: 1 });
        assert!(analysis.has_output_redirection);
    }

    #[test]
    fn bash_should_collect_cp_source_and_destination() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis =
            terminal_analyze_command(&cwd, r#"cp ./a.txt ./b.txt"#, "git-bash");

        let candidates = analysis.path_candidates();
        assert_eq!(analysis.write_risk, TerminalWriteRisk::NewOnly { count: 1 });
        assert!(candidates.iter().any(|item| item.path.to_string_lossy().contains("a.txt")));
        assert!(candidates.iter().any(|item| item.path.to_string_lossy().contains("b.txt")));
    }

    #[test]
    fn bash_should_track_cd_then_relative_reads() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis =
            terminal_analyze_command(&cwd, r#"cd src-tauri && ls ./src"#, "git-bash");

        assert!(analysis.has_directory_change);
        assert!(analysis.path_candidates().len() >= 1);
    }

    #[test]
    fn powershell_should_treat_nul_as_safe_sink() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis =
            terminal_analyze_command(&cwd, r#"Get-ChildItem .\archive 2>nul"#, "powershell7");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().contains("archive")));
        assert!(!analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().to_ascii_lowercase().contains("nul")));
    }

    #[test]
    fn powershell_should_detect_set_content_as_write() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis = terminal_analyze_command(
            &cwd,
            r#"Set-Content -Path .\note.txt -Value 'hi'"#,
            "powershell7",
        );

        assert_eq!(analysis.write_risk, TerminalWriteRisk::NewOnly { count: 1 });
        assert!(analysis
            .write_target_paths()
            .iter()
            .any(|path| path.to_string_lossy().contains("note.txt")));
    }

    #[test]
    fn powershell_should_detect_move_item_source_and_destination() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis = terminal_analyze_command(
            &cwd,
            r#"Move-Item -Path .\a.txt -Destination .\b.txt"#,
            "powershell7",
        );

        assert_eq!(analysis.write_risk, TerminalWriteRisk::NewOnly { count: 2 });
        assert_eq!(analysis.path_candidates().len(), 2);
    }

    #[test]
    fn bash_should_skip_env_wrappers_and_keep_read_paths() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis = terminal_analyze_command(
            &cwd,
            r#"FOO=bar env DEBUG=1 ls -la ./archive 2>/dev/null"#,
            "git-bash",
        );

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().contains("archive")));
        assert!(!analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().replace('\\', "/").contains("dev/null")));
    }

    #[test]
    fn bash_should_treat_sed_in_place_as_write() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis = terminal_analyze_command(
            &cwd,
            r#"sed -i 's/a/b/' ./note.txt"#,
            "git-bash",
        );

        assert_eq!(analysis.write_risk, TerminalWriteRisk::NewOnly { count: 1 });
        assert!(analysis
            .write_target_paths()
            .iter()
            .any(|path| path.to_string_lossy().contains("note.txt")));
    }

    #[test]
    fn powershell_should_detect_out_file_target_as_write() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let analysis = terminal_analyze_command(
            &cwd,
            r#"Get-Content .\input.txt | Out-File -FilePath .\output.txt"#,
            "powershell7",
        );

        assert_eq!(analysis.write_risk, TerminalWriteRisk::NewOnly { count: 1 });
        assert!(analysis
            .path_candidates()
            .iter()
            .any(|item| item.path.to_string_lossy().contains("input.txt")));
        assert!(analysis
            .write_target_paths()
            .iter()
            .any(|path| path.to_string_lossy().contains("output.txt")));
    }
}
