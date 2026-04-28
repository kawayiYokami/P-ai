fn terminal_decode_live_line(bytes: &[u8]) -> String {
    terminal_decode_output_bytes(bytes)
}

async fn terminal_live_exec_command(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
    command: &str,
    timeout_ms: u64,
) -> Result<SandboxExecutionResult, String> {
    let session = terminal_live_session_for(state, session_id, cwd).await?;
    let runtime_shell = terminal_shell_for_state(state);
    let _session_guard = session.exec_lock.lock().await;
    let marker = format!("__ECA_DONE__{}", Uuid::new_v4());
    let wrapped = terminal_live_compose_command(&runtime_shell, cwd, command, &marker);
    {
        let mut stdin = session.stdin.lock().await;
        stdin
            .write_all(wrapped.as_bytes())
            .await
            .map_err(|err| format!("write live shell stdin failed: {err}"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|err| format!("write live shell stdin failed: {err}"))?;
        stdin
            .flush()
            .await
            .map_err(|err| format!("flush live shell stdin failed: {err}"))?;
    }

    let started = std::time::Instant::now();
    let mut stdout_reader = session.stdout.lock().await;
    let mut stderr_reader = session.stderr.lock().await;
    let mut stdout_text = String::new();
    let mut stderr_text = String::new();
    let mut exit_code = 0i32;

    loop {
        let elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        if elapsed >= timeout_ms {
            let _ = terminal_live_close_session(state, session_id).await;
            return Err(format!("terminal_exec timed out after {}ms", timeout_ms));
        }
        let remain = timeout_ms.saturating_sub(elapsed).max(1);
        let mut out_line = Vec::<u8>::new();
        let mut err_line = Vec::<u8>::new();
        let selected = tokio::time::timeout(
            std::time::Duration::from_millis(remain),
            async {
                tokio::select! {
                    out = stdout_reader.read_until(b'\n', &mut out_line) => ("stdout", out.map(|n| n as i64), out_line),
                    err = stderr_reader.read_until(b'\n', &mut err_line) => ("stderr", err.map(|n| n as i64), err_line),
                }
            },
        )
        .await;
        let (stream, read_res, line) = match selected {
            Ok(value) => value,
            Err(_) => {
                let _ = terminal_live_close_session(state, session_id).await;
                return Err(format!("terminal_exec timed out after {}ms", timeout_ms));
            }
        };
        let n = read_res.map_err(|err| format!("read live shell output failed: {err}"))?;
        if n == 0 {
            let _ = terminal_live_close_session(state, session_id).await;
            return Err("live shell closed unexpectedly".to_string());
        }
        let line = terminal_decode_live_line(&line);
        let trimmed = line.trim_end_matches(['\r', '\n']);
        // Some commands (for example `cat`/`head` on files without trailing newline)
        // may print payload and marker in the same line. Detect marker anywhere in stdout.
        if stream == "stdout" && trimmed.contains(&marker) {
            if let Some(marker_pos) = trimmed.find(&marker) {
                let prefix = &trimmed[..marker_pos];
                if !prefix.is_empty() {
                    stdout_text.push_str(prefix);
                }
                let suffix = &trimmed[marker_pos + marker.len()..];
                let suffix = suffix.strip_prefix(':').unwrap_or(suffix).trim();
                exit_code = suffix.parse::<i32>().unwrap_or(0);
            }
            loop {
                let drain_elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                if drain_elapsed >= timeout_ms {
                    break;
                }
                let drain_remain = timeout_ms.saturating_sub(drain_elapsed).max(1).min(50);
                let mut drain_err_line = Vec::<u8>::new();
                let drained = tokio::time::timeout(
                    std::time::Duration::from_millis(drain_remain),
                    stderr_reader.read_until(b'\n', &mut drain_err_line),
                )
                .await;
                let drain_n = match drained {
                    Ok(result) => result.map_err(|err| format!("read live shell output failed: {err}"))?,
                    Err(_) => break,
                };
                if drain_n == 0 {
                    break;
                }
                stderr_text.push_str(&terminal_decode_live_line(&drain_err_line));
            }
            break;
        }
        if stream == "stdout" {
            stdout_text.push_str(&line);
        } else {
            stderr_text.push_str(&line);
        }
    }

    *session.last_used_at.lock().await = now_iso();

    Ok(SandboxExecutionResult {
        ok: exit_code == 0,
        exit_code,
        stdout: stdout_text.into_bytes(),
        stderr: stderr_text.into_bytes(),
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        shell_kind: session.shell_kind.clone(),
        shell_path: session.shell_path.clone(),
    })
}

fn terminal_workspace_access_rank(access: &str) -> i32 {
    match access {
        SHELL_WORKSPACE_ACCESS_READ_ONLY => 3,
        SHELL_WORKSPACE_ACCESS_APPROVAL => 2,
        _ => 1,
    }
}

fn terminal_output_only_command_is_read_only(base_cmd: &str) -> bool {
    matches!(
        base_cmd,
        "echo"
            | "printf"
            | "true"
            | "false"
            | "date"
            | "whoami"
            | "hostname"
            | "uname"
            | "clear"
            | "cls"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalReadWhitelistSimpleCommand {
    command: String,
    allowed: bool,
    reason: String,
}

fn terminal_simple_command_text(simple: &TerminalSimpleCommand) -> String {
    let mut parts = simple.argv.clone();
    for redirection in &simple.output_redirections {
        let operator = if redirection.append { ">>" } else { ">" };
        parts.push(format!("{operator}{}", redirection.target));
    }
    parts.join(" ")
}

fn terminal_read_whitelist_base_command(simple: &TerminalSimpleCommand, family: TerminalShellFamily) -> Option<(String, usize)> {
    match family {
        TerminalShellFamily::PowerShell => {
            let first = simple.argv.first()?;
            Some((
                terminal_powershell_alias_base(&terminal_unquote_token(first).to_ascii_lowercase())
                    .to_string(),
                1usize,
            ))
        }
        TerminalShellFamily::Posix | TerminalShellFamily::Other => {
            let start_idx = terminal_skip_bash_wrappers(&simple.argv);
            let first = simple.argv.get(start_idx)?;
            Some((terminal_unquote_token(first).to_ascii_lowercase(), start_idx + 1))
        }
    }
}

fn terminal_simple_command_read_whitelist_allowed(
    simple: &TerminalSimpleCommand,
    family: TerminalShellFamily,
) -> bool {
    let Some((base_cmd, arg_start_idx)) = terminal_read_whitelist_base_command(simple, family) else {
        return false;
    };
    let second = simple
        .argv
        .get(arg_start_idx)
        .map(|item| terminal_unquote_token(item).to_ascii_lowercase())
        .unwrap_or_default();

    match family {
        TerminalShellFamily::PowerShell => {
            matches!(
                base_cmd.as_str(),
                "set-location"
                    | "get-childitem"
                    | "get-content"
                    | "select-string"
                    | "select-object"
                    | "where-object"
                    | "sort-object"
                    | "measure-object"
                    | "get-item"
                    | "test-path"
                    | "resolve-path"
                    | "get-location"
                    | "pwd"
                    | "rg"
                    | "findstr"
                    | "where"
            ) || (base_cmd == "git" && terminal_git_read_whitelist_allowed(&second, &simple.argv, arg_start_idx))
                || terminal_output_only_command_is_read_only(base_cmd.as_str())
                || terminal_check_command_is_read_only(base_cmd.as_str(), &simple.argv[arg_start_idx..])
        }
        TerminalShellFamily::Posix | TerminalShellFamily::Other => {
            matches!(
                base_cmd.as_str(),
                "pwd"
                    | "cd"
                    | "chdir"
                    | "ls"
                    | "dir"
                    | "cat"
                    | "type"
                    | "head"
                    | "tail"
                    | "sort"
                    | "uniq"
                    | "wc"
                    | "find"
                    | "rg"
                    | "grep"
                    | "findstr"
                    | "stat"
                    | "which"
                    | "where"
            ) || (base_cmd == "git" && terminal_git_read_whitelist_allowed(&second, &simple.argv, arg_start_idx))
                || terminal_output_only_command_is_read_only(base_cmd.as_str())
                || terminal_check_command_is_read_only(base_cmd.as_str(), &simple.argv[arg_start_idx..])
        }
    }
}

fn terminal_read_whitelist_simple_commands(
    command: &str,
    shell_kind: &str,
    analysis: &TerminalCommandAnalysis,
) -> Vec<TerminalReadWhitelistSimpleCommand> {
    let global_reason = if terminal_is_python_like_command(command) {
        Some("python/py 命令不属于只读白名单".to_string())
    } else if !matches!(analysis.write_risk, TerminalWriteRisk::None) {
        Some("命令存在写入风险，不属于只读白名单".to_string())
    } else if analysis.unresolved_write_targets {
        Some("命令存在无法解析的写入目标，不属于只读白名单".to_string())
    } else if analysis.accesses.iter().any(|item| {
        !matches!(
            item.intent,
            TerminalPathIntent::Read | TerminalPathIntent::ChangeDirectory
        )
    }) {
        Some("命令包含非读取路径操作，不属于只读白名单".to_string())
    } else {
        None
    };

    let family = terminal_shell_family(shell_kind);
    terminal_split_simple_commands(command)
        .into_iter()
        .map(|simple| {
            let command_text = terminal_simple_command_text(&simple);
            if let Some(reason) = &global_reason {
                return TerminalReadWhitelistSimpleCommand {
                    command: command_text,
                    allowed: false,
                    reason: reason.clone(),
                };
            }
            let allowed = terminal_simple_command_read_whitelist_allowed(&simple, family);
            TerminalReadWhitelistSimpleCommand {
                command: command_text,
                allowed,
                reason: if allowed {
                    "命中只读白名单".to_string()
                } else {
                    "未命中只读白名单".to_string()
                },
            }
        })
        .collect()
}

fn terminal_read_whitelist_diagnostics_value(
    command: &str,
    shell_kind: &str,
    analysis: &TerminalCommandAnalysis,
) -> Value {
    let commands = terminal_read_whitelist_simple_commands(command, shell_kind, analysis);
    let whitelisted = commands
        .iter()
        .filter(|item| item.allowed)
        .map(|item| item.command.clone())
        .collect::<Vec<_>>();
    let non_whitelisted = commands
        .iter()
        .filter(|item| !item.allowed)
        .map(|item| serde_json::json!({
            "command": item.command,
            "reason": item.reason,
        }))
        .collect::<Vec<_>>();
    serde_json::json!({
        "whitelistedCommands": whitelisted,
        "nonWhitelistedCommands": non_whitelisted,
    })
}

fn terminal_read_whitelist_diagnostics_message(
    command: &str,
    shell_kind: &str,
    analysis: &TerminalCommandAnalysis,
) -> String {
    let commands = terminal_read_whitelist_simple_commands(command, shell_kind, analysis);
    let whitelisted = commands
        .iter()
        .filter(|item| item.allowed)
        .map(|item| format!("- {}", item.command))
        .collect::<Vec<_>>();
    let non_whitelisted = commands
        .iter()
        .filter(|item| !item.allowed)
        .map(|item| format!("- {}：{}", item.command, item.reason))
        .collect::<Vec<_>>();
    let mut lines = vec!["当前路径权限为只读权限，你使用了白名单以外的指令。".to_string()];
    lines.push("下面是非白名单指令：".to_string());
    if non_whitelisted.is_empty() {
        lines.push("- 无".to_string());
    } else {
        lines.extend(non_whitelisted);
    }
    lines.push("下面是白名单指令：".to_string());
    if whitelisted.is_empty() {
        lines.push("- 无".to_string());
    } else {
        lines.extend(whitelisted);
    }
    lines.join("\n")
}

fn terminal_read_whitelist_usage_hint() -> &'static str {
    "只读命令白名单不受目录权限影响。若只是读取、查询、检查，请优先使用常见只读命令，例如 git status、git diff、git log、git show、git branch、git grep、rg、grep、ls、cat。"
}

fn terminal_strictest_workspace_access(accesses: &[String]) -> String {
    accesses
        .iter()
        .max_by_key(|access| terminal_workspace_access_rank(access))
        .cloned()
        .unwrap_or_else(|| SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string())
}

fn terminal_is_python_like_command(command: &str) -> bool {
    let tokens = terminal_tokenize(command);
    let Some(first) = tokens.first() else {
        return false;
    };
    let token = terminal_unquote_token(first);
    let exe = token
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(token.as_str())
        .to_ascii_lowercase();
    matches!(exe.as_str(), "python" | "python.exe" | "py" | "py.exe")
}

fn terminal_git_read_only_subcommand(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "log"
            | "show"
            | "reflog"
            | "diff"
            | "status"
            | "ls-remote"
            | "blame"
            | "ls-files"
            | "grep"
            | "cat-file"
            | "ls-tree"
            | "merge-base"
            | "rev-parse"
    )
}

fn terminal_git_read_whitelist_allowed(subcommand: &str, argv: &[String], arg_start_idx: usize) -> bool {
    let args = argv.get(arg_start_idx.saturating_add(1)..).unwrap_or(&[]);
    terminal_git_subcommand_is_read_only_with_args(subcommand, args)
}

fn terminal_git_arg_lower(arg: &str) -> String {
    terminal_unquote_token(arg).to_ascii_lowercase()
}

fn terminal_git_subcommand_is_read_only_with_args(subcommand: &str, args: &[String]) -> bool {
    match subcommand {
        "branch" => terminal_git_branch_is_read_only(args),
        "tag" => terminal_git_tag_is_read_only(args),
        "config" => terminal_git_config_is_read_only(args),
        _ => terminal_git_read_only_subcommand(subcommand),
    }
}

fn terminal_git_branch_is_read_only(args: &[String]) -> bool {
    let mut positional_count = 0usize;
    for raw in args {
        let arg = terminal_git_arg_lower(raw);
        match arg.as_str() {
            "-d" | "-m" | "-c" | "--delete" | "--move" | "--copy" => return false,
            value if value.starts_with('-') => {}
            _ => positional_count += 1,
        }
    }
    positional_count == 0 || args.iter().map(|arg| terminal_git_arg_lower(arg)).any(|arg| {
        matches!(arg.as_str(), "--list" | "-l" | "--contains" | "--merged" | "--no-merged" | "--points-at")
    })
}

fn terminal_git_tag_is_read_only(args: &[String]) -> bool {
    let mut positional_count = 0usize;
    for raw in args {
        let arg = terminal_git_arg_lower(raw);
        match arg.as_str() {
            "-d" | "--delete" | "-a" | "-s" | "-u" | "-f" | "--force" => return false,
            value if value.starts_with('-') => {}
            _ => positional_count += 1,
        }
    }
    positional_count == 0 || args.iter().map(|arg| terminal_git_arg_lower(arg)).any(|arg| {
        matches!(arg.as_str(), "--list" | "-l" | "--contains" | "--merged" | "--no-merged" | "--points-at")
    })
}

fn terminal_git_config_is_read_only(args: &[String]) -> bool {
    let mut has_read_mode = false;
    let mut pending_read_option_value = false;
    for raw in args {
        let arg = terminal_git_arg_lower(raw);
        if pending_read_option_value {
            pending_read_option_value = false;
            continue;
        }
        match arg.as_str() {
            "--get" | "--get-all" | "--get-regexp" | "--list" | "-l" | "--show-origin" | "--show-scope" => {
                has_read_mode = true;
            }
            "--name-only" | "--null" | "-z" => {}
            "--file" | "-f" | "--blob" => {
                pending_read_option_value = true;
            }
            _ if arg.starts_with("--file=") || arg.starts_with("--blob=") => {}
            _ if arg.starts_with('-') => return false,
            _ => {}
        }
    }
    has_read_mode
}

fn terminal_check_command_is_read_only(base_cmd: &str, args: &[String]) -> bool {
    match base_cmd {
        "pnpm" | "pnpm.cmd" | "npm" | "npm.cmd" | "yarn" | "yarn.cmd" => args
            .iter()
            .map(|arg| terminal_unquote_token(arg).to_ascii_lowercase())
            .any(|arg| matches!(arg.as_str(), "typecheck" | "check" | "test")),
        "cargo" | "cargo.exe" => args
            .first()
            .map(|arg| terminal_unquote_token(arg).to_ascii_lowercase())
            .map(|arg| matches!(arg.as_str(), "check" | "test"))
            .unwrap_or(false),
        "vue-tsc" | "vue-tsc.cmd" | "tsc" | "tsc.cmd" => true,
        _ => false,
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TerminalSmartReviewReply {
    #[serde(default)]
    allow: bool,
    #[serde(default)]
    review_opinion: String,
}

#[derive(Debug, Clone)]
struct TerminalSmartReviewDecision {
    allow: bool,
    review_opinion: String,
    model_name: String,
}

#[derive(Debug, Clone)]
enum TerminalSmartReviewOutcome {
    Decision(TerminalSmartReviewDecision),
    RawJson {
        raw_json: String,
        model_name: String,
    },
}

fn terminal_localized_text(ui_language: &str, zh_cn: &str, zh_tw: &str, en: &str) -> String {
    match ui_language.trim() {
        "en-US" => en.to_string(),
        "zh-TW" => zh_tw.to_string(),
        _ => zh_cn.to_string(),
    }
}

fn terminal_local_rule_model_name(ui_language: &str) -> String {
    terminal_localized_text(ui_language, "本地规则", "本地規則", "Local rules")
}

fn terminal_local_review_value(ui_language: &str, review_opinion: impl Into<String>) -> Value {
    serde_json::json!({
        "kind": "local_rule",
        "allow": false,
        "reviewOpinion": review_opinion.into(),
        "modelName": terminal_local_rule_model_name(ui_language),
    })
}

fn terminal_local_rule_reason_message(ui_language: &str, reason: &str) -> String {
    match reason {
        "encoded command is blocked" => terminal_localized_text(
            ui_language,
            "命令使用了编码执行参数，本地规则已直接拦截。这类命令难以直接确认真实执行内容，风险过高，不进入 AI 审查。",
            "命令使用了編碼執行參數，本地規則已直接攔截。這類命令難以直接確認真實執行內容，風險過高，不進入 AI 審查。",
            "This command uses an encoded execution argument and was blocked by local rules. The real behavior cannot be inspected directly, so the risk is too high to send to AI review.",
        ),
        "Invoke-Expression/iex is blocked" => terminal_localized_text(
            ui_language,
            "命令使用了 Invoke-Expression/iex 动态执行，本地规则已直接拦截。这类命令会放大真实执行内容的不确定性，风险过高，不进入 AI 审查。",
            "命令使用了 Invoke-Expression/iex 動態執行，本地規則已直接攔截。這類命令會放大真實執行內容的不確定性，風險過高，不進入 AI 審查。",
            "This command uses Invoke-Expression/iex and was blocked by local rules. Dynamic execution makes the real behavior too uncertain, so it is too risky for AI review.",
        ),
        "spawning nested shells is blocked" => terminal_localized_text(
            ui_language,
            "命令尝试再启动一层 shell，本地规则已直接拦截。这会绕开当前终端约束并放大风险，不进入 AI 审查。",
            "命令嘗試再啟動一層 shell，本地規則已直接攔截。這會繞開當前終端約束並放大風險，不進入 AI 審查。",
            "This command tries to spawn another shell and was blocked by local rules. That can bypass current terminal constraints, so it is too risky for AI review.",
        ),
        "git push --force/-f is especially dangerous and is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git push --force/-f，本地规则已按特别高危直接拦截。它可能覆盖远端历史并影响他人协作，不进入 AI 审查。",
            "命令包含 git push --force/-f，本地規則已按特別高危直接攔截。它可能覆蓋遠端歷史並影響他人協作，不進入 AI 審查。",
            "This command includes git push --force/-f and was blocked as especially high risk. It can overwrite remote history and affect collaborators, so it does not go to AI review.",
        ),
        "git pull --force/-f is especially dangerous and is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git pull --force/-f，本地规则已按特别高危直接拦截。它会强行改动本地工作区和历史状态，不进入 AI 审查。",
            "命令包含 git pull --force/-f，本地規則已按特別高危直接攔截。它會強行改動本地工作區和歷史狀態，不進入 AI 審查。",
            "This command includes git pull --force/-f and was blocked as especially high risk. It can forcibly rewrite the local workspace and history, so it does not go to AI review.",
        ),
        "git push is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git push，本地规则已直接拦截。它会改动远端仓库状态，不属于可自动审查后直接执行的范围。",
            "命令包含 git push，本地規則已直接攔截。它會改動遠端倉庫狀態，不屬於可自動審查後直接執行的範圍。",
            "This command includes git push and was blocked by local rules. It changes remote repository state and is not eligible for automatic review and execution.",
        ),
        "git pull is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git pull，本地规则已直接拦截。它会拉取并改动本地仓库状态，不属于可自动审查后直接执行的范围。",
            "命令包含 git pull，本地規則已直接攔截。它會拉取並改動本地倉庫狀態，不屬於可自動審查後直接執行的範圍。",
            "This command includes git pull and was blocked by local rules. It changes local repository state after fetching remote updates, so it is not eligible for automatic review and execution.",
        ),
        "git fetch is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git fetch，本地规则已直接拦截。它会改动本地仓库引用状态，不进入 AI 审查。",
            "命令包含 git fetch，本地規則已直接攔截。它會改動本地倉庫引用狀態，不進入 AI 審查。",
            "This command includes git fetch and was blocked by local rules. It changes local repository references, so it does not go to AI review.",
        ),
        "git commit is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git commit，本地规则已直接拦截。它会生成新的仓库历史记录，不进入 AI 审查。",
            "命令包含 git commit，本地規則已直接攔截。它會生成新的倉庫歷史記錄，不進入 AI 審查。",
            "This command includes git commit and was blocked by local rules. It creates new repository history and does not go to AI review.",
        ),
        "git merge is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git merge，本地规则已直接拦截。它会改动分支历史和工作区状态，不进入 AI 审查。",
            "命令包含 git merge，本地規則已直接攔截。它會改動分支歷史和工作區狀態，不進入 AI 審查。",
            "This command includes git merge and was blocked by local rules. It changes branch history and workspace state, so it does not go to AI review.",
        ),
        "git rebase is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git rebase，本地规则已直接拦截。它会重写历史并可能影响后续协作，不进入 AI 审查。",
            "命令包含 git rebase，本地規則已直接攔截。它會重寫歷史並可能影響後續協作，不進入 AI 審查。",
            "This command includes git rebase and was blocked by local rules. It rewrites history and can affect later collaboration, so it does not go to AI review.",
        ),
        "git reset is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git reset，本地规则已直接拦截。它会改动仓库历史或工作区状态，不进入 AI 审查。",
            "命令包含 git reset，本地規則已直接攔截。它會改動倉庫歷史或工作區狀態，不進入 AI 審查。",
            "This command includes git reset and was blocked by local rules. It changes repository history or workspace state, so it does not go to AI review.",
        ),
        "git checkout is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git checkout，本地规则已直接拦截。它可能切换分支或覆盖文件状态，不进入 AI 审查。",
            "命令包含 git checkout，本地規則已直接攔截。它可能切換分支或覆蓋文件狀態，不進入 AI 審查。",
            "This command includes git checkout and was blocked by local rules. It can switch branches or overwrite file state, so it does not go to AI review.",
        ),
        "git switch is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git switch，本地规则已直接拦截。它可能切换分支并改动工作区状态，不进入 AI 审查。",
            "命令包含 git switch，本地規則已直接攔截。它可能切換分支並改動工作區狀態，不進入 AI 審查。",
            "This command includes git switch and was blocked by local rules. It can switch branches and change workspace state, so it does not go to AI review.",
        ),
        "git restore is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git restore，本地规则已直接拦截。它会覆盖本地文件状态，不进入 AI 审查。",
            "命令包含 git restore，本地規則已直接攔截。它會覆蓋本地文件狀態，不進入 AI 審查。",
            "This command includes git restore and was blocked by local rules. It overwrites local file state, so it does not go to AI review.",
        ),
        "git clean is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git clean，本地规则已直接拦截。它会删除未跟踪文件，风险过高，不进入 AI 审查。",
            "命令包含 git clean，本地規則已直接攔截。它會刪除未跟蹤文件，風險過高，不進入 AI 審查。",
            "This command includes git clean and was blocked by local rules. It deletes untracked files and is too risky for AI review.",
        ),
        "git stash is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git stash，本地规则已直接拦截。它会改动工作区与暂存状态，不进入 AI 审查。",
            "命令包含 git stash，本地規則已直接攔截。它會改動工作區與暫存狀態，不進入 AI 審查。",
            "This command includes git stash and was blocked by local rules. It changes workspace and stash state, so it does not go to AI review.",
        ),
        "git apply is blocked" => terminal_localized_text(
            ui_language,
            "命令包含 git apply，本地规则已直接拦截。它会直接把补丁写入工作区，不进入 AI 审查。",
            "命令包含 git apply，本地規則已直接攔截。它會直接把補丁寫入工作區，不進入 AI 審查。",
            "This command includes git apply and was blocked by local rules. It writes a patch directly into the workspace, so it does not go to AI review.",
        ),
        _ => terminal_localized_text(
            ui_language,
            "本地规则已直接拦截此命令。这类命令存在明确风险，不进入 AI 审查。",
            "本地規則已直接攔截此命令。這類命令存在明確風險，不進入 AI 審查。",
            "This command was blocked by local rules. It has a clearly identified risk and does not go to AI review.",
        ),
    }
}

fn terminal_command_is_read_whitelist(
    command: &str,
    shell_kind: &str,
    analysis: &TerminalCommandAnalysis,
) -> bool {
    if terminal_is_python_like_command(command)
        || !matches!(analysis.write_risk, TerminalWriteRisk::None)
        || analysis.unresolved_write_targets
    {
        return false;
    }

    if analysis.accesses.iter().any(|item| {
        !matches!(
            item.intent,
            TerminalPathIntent::Read | TerminalPathIntent::ChangeDirectory
        )
    }) {
        return false;
    }

    let family = terminal_shell_family(shell_kind);
    for simple in terminal_split_simple_commands(command) {
        if !terminal_simple_command_read_whitelist_allowed(&simple, family) {
            return false;
        }
    }

    true
}

fn terminal_read_whitelist_cwd_from_command(
    command: &str,
    shell_kind: &str,
    fallback_cwd: &Path,
) -> PathBuf {
    let family = terminal_shell_family(shell_kind);
    let mut cwd = fallback_cwd.to_path_buf();
    for simple in terminal_split_simple_commands(command) {
        let (base_cmd, args_start_idx) = match family {
            TerminalShellFamily::PowerShell => {
                let Some(first) = simple.argv.first() else {
                    continue;
                };
                (
                    terminal_powershell_alias_base(
                        &terminal_unquote_token(first).to_ascii_lowercase(),
                    )
                    .to_string(),
                    1usize,
                )
            }
            TerminalShellFamily::Posix | TerminalShellFamily::Other => {
                let start_idx = terminal_skip_bash_wrappers(&simple.argv);
                let Some(first) = simple.argv.get(start_idx) else {
                    continue;
                };
                (terminal_unquote_token(first).to_ascii_lowercase(), start_idx + 1)
            }
        };
        if !matches!(base_cmd.as_str(), "cd" | "chdir" | "set-location") {
            continue;
        }
        let Some(target_raw) = simple.argv.get(args_start_idx) else {
            continue;
        };
        if let Some(next_cwd) = terminal_resolve_candidate_path(&cwd, target_raw) {
            cwd = next_cwd;
        }
    }
    cwd
}

fn terminal_read_whitelist_cwd_for_execution(
    command: &str,
    shell_kind: &str,
    fallback_cwd: &Path,
) -> PathBuf {
    terminal_read_whitelist_cwd_from_command(command, shell_kind, fallback_cwd)
        .canonicalize()
        .unwrap_or_else(|_| fallback_cwd.to_path_buf())
}

fn terminal_smart_review_language(ui_language: &str) -> &'static str {
    match ui_language.trim() {
        "en-US" => "English",
        "zh-TW" => "繁體中文",
        _ => "简体中文",
    }
}

fn terminal_smart_review_extract_json(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    trimmed
}

fn terminal_smart_review_local_risk_label(write_risk: &TerminalWriteRisk) -> &'static str {
    match write_risk {
        TerminalWriteRisk::None => "none",
        TerminalWriteRisk::NewOnly { .. } => "new_write",
        TerminalWriteRisk::Existing { .. } => "existing_write",
        TerminalWriteRisk::Unknown => "unknown_write",
    }
}

fn terminal_smart_review_local_risk_summary(write_risk: &TerminalWriteRisk) -> String {
    match write_risk {
        TerminalWriteRisk::None => "No local write risk was detected.".to_string(),
        TerminalWriteRisk::NewOnly { count } => format!(
            "The command appears to create or overwrite {count} new path(s)."
        ),
        TerminalWriteRisk::Existing { paths } => format!(
            "The command appears to modify or delete {} existing path(s).",
            paths.len()
        ),
        TerminalWriteRisk::Unknown => {
            "The command may write, but the local parser could not identify the exact target."
                .to_string()
        }
    }
}

fn terminal_smart_review_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .take(8)
        .map(|path| terminal_path_for_user(path))
        .collect()
}

fn tool_safety_review_system_prompt(language: &str) -> String {
    format!(
        "请使用{language}完成工具执行审查。\n\
你负责判断当前工具执行结果是否可以直接放行，还是必须先交给用户确认。\n\
你的目标是让不会编程的普通人也能看明白这次工具执行大概要做什么、可能影响什么、为什么建议直接执行或先确认。\n\
请优先使用简单人话，而不是技术术语；如果看不清影响范围，就直接说明无法确认。\n\
对于 shell_exec，若命令虽不在白名单中，但可以明确判断为只读取、只查询、只检查、只测试、只输出结果，且不会写入或修改本地文件、不会修改 Git 状态、不会修改系统配置、也不会把网络内容保存到本地文件，则应返回 allow=true。\n\
对于 shell_exec，各类测试、检查、编译校验命令只要只是运行并输出结果、不修改本地项目文件，应返回 allow=true；curl、wget、Invoke-WebRequest 等命令只要只是获取内容并输出到终端、不写入本地文件，也应返回 allow=true。\n\
对于 shell_exec，如果命令会新增、覆盖、删除、重命名本地文件，修改 Git 工作区、索引、提交历史、分支指向或 stash 状态，修改系统配置、环境变量或其他持久化状态，下载内容到本地文件，使用输出重定向写文件，或通过管道直接执行脚本，则应返回 allow=false。\n\
对于 shell_exec，如果无法确认命令是否存在副作用，也应返回 allow=false，不要猜测放行。\n\
只返回一个 JSON 对象，不要输出 Markdown、代码块或额外解释。\n\
JSON 只能包含这些字段：allow, review_opinion。\n\
其中：allow 表示是否放行，review_opinion 表示给普通用户看的审查意见。"
    )
}

fn build_tool_safety_review_user_prompt(tool_name: &str, context: &Value) -> String {
    format!("当前待审查工具：{tool_name}\n请审查以下内容：\n{context}")
}

fn current_tool_review_api_config_id(state: &AppState) -> Result<Option<String>, String> {
    let app_config = state_read_config_cached(state)?;
    Ok(app_config
        .tool_review_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string))
}

async fn run_tool_smart_review(
    state: &AppState,
    review_api_config_id: &str,
    tool_name: &str,
    scene: &'static str,
    context: Value,
) -> Result<TerminalSmartReviewOutcome, String> {
    let app_config = state_read_config_cached(state)?;
    let selected_api = resolve_selected_api_config(&app_config, Some(review_api_config_id))
        .ok_or_else(|| format!("Tool review API config '{}' not found.", review_api_config_id))?;
    if !selected_api.enable_text || !selected_api.request_format.is_chat_text() {
        return Err(format!(
            "Tool review API config '{}' does not support chat text.",
            review_api_config_id
        ));
    }
    let resolved_api = resolve_api_config(&app_config, Some(review_api_config_id))?;
    let language = terminal_smart_review_language(&app_config.ui_language);
    let prepared = conversation_prompt_service()
        .build_tool_safety_review_prepared_prompt(language, tool_name, &context);
    let review_execution = invoke_model_with_policy(
        &resolved_api,
        &selected_api.model,
        prepared,
        CallPolicy {
            scene,
            timeout_secs: Some(12),
            json_only: true,
        },
        Some(state),
    )
    .await;
    push_model_call_log_parts(Some(state), &review_execution);
    let reply = review_execution.result?;
    let raw_json = terminal_smart_review_extract_json(&reply.assistant_text);
    let parsed_value = match serde_json::from_str::<Value>(raw_json) {
        Ok(value) => value,
        Err(_) => {
            return Ok(TerminalSmartReviewOutcome::RawJson {
                raw_json: raw_json.trim().to_string(),
                model_name: selected_api.name.trim().to_string(),
            });
        }
    };
    let pretty_json = serde_json::to_string_pretty(&parsed_value)
        .unwrap_or_else(|_| raw_json.trim().to_string());
    let parsed = match serde_json::from_value::<TerminalSmartReviewReply>(parsed_value) {
        Ok(value) => value,
        Err(_) => {
            return Ok(TerminalSmartReviewOutcome::RawJson {
                raw_json: pretty_json,
                model_name: selected_api.name.trim().to_string(),
            });
        }
    };
    Ok(TerminalSmartReviewOutcome::Decision(TerminalSmartReviewDecision {
        allow: parsed.allow,
        review_opinion: parsed.review_opinion.trim().to_string(),
        model_name: selected_api.name.trim().to_string(),
    }))
}

async fn terminal_run_smart_review(
    state: &AppState,
    review_api_config_id: &str,
    cwd: &Path,
    command: &str,
    effective_access: &str,
    write_risk: &TerminalWriteRisk,
    target_paths: &[PathBuf],
    existing_paths: &[PathBuf],
) -> Result<TerminalSmartReviewOutcome, String> {
    let context = serde_json::json!({
        "cwd": terminal_path_for_user(cwd),
        "command": command,
        "workspace_access": effective_access,
        "local_risk": terminal_smart_review_local_risk_label(write_risk),
        "local_risk_summary": terminal_smart_review_local_risk_summary(write_risk),
        "target_paths": terminal_smart_review_paths(target_paths),
        "existing_paths": terminal_smart_review_paths(existing_paths),
    });
    run_tool_smart_review(
        state,
        review_api_config_id,
        "shell_exec",
        "Tool safety review",
        context,
    )
    .await
}

async fn builtin_shell_exec(
    state: &AppState,
    session_id: &str,
    action: &str,
    command: &str,
    timeout_ms: Option<u64>,
) -> Result<Value, String> {
    let action = action.trim().to_ascii_lowercase();
    let cmd = command.trim();
    let ui_language = state_read_config_cached(state)
        .map(|config| config.ui_language)
        .unwrap_or_else(|_| "zh-CN".to_string());
    let runtime_shell = terminal_shell_for_state(state);
    #[cfg(target_os = "windows")]
    if runtime_shell.kind == "missing-terminal-shell" {
        let review = terminal_local_review_value(
            &ui_language,
            "当前系统未检测到可用终端，无法安全执行命令，请先安装并配置受支持的终端环境。",
        );
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "missing_terminal_shell",
            "message": terminal_localized_text(
                &ui_language,
                "当前系统未检测到受支持的终端。请先安装 Git，并使用 Git Bash： https://git-scm.com/downloads",
                "當前系統未檢測到受支持的終端。請先安裝 Git，並使用 Git Bash： https://git-scm.com/downloads",
                "No supported terminal was detected on Windows. Install Git and use Git Bash: https://git-scm.com/downloads",
            ),
            "toolReview": review,
            "command": cmd
        }));
    }
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    if action == "list" {
        let sessions = terminal_live_list_sessions(state).await;
        return Ok(serde_json::json!({
            "ok": true,
            "action": "list",
            "sessions": sessions,
            "sessionCount": sessions.len(),
        }));
    }
    if action == "close" {
        let closed = terminal_live_close_session(state, &normalized_session).await?;
        return Ok(serde_json::json!({
            "ok": true,
            "action": "close",
            "closed": closed,
        }));
    }
    if action != "run" {
        return Err(format!("shell_exec.action must be run|list|close, got: {action}"));
    }
    if cmd.is_empty() {
        return Err("shell_exec.command is empty".to_string());
    }
    if let Some(reason) = terminal_command_block_reason(cmd) {
        let review = terminal_local_review_value(&ui_language, terminal_local_rule_reason_message(&ui_language, reason));
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "local_rule_blocked",
            "message": terminal_localized_text(
                &ui_language,
                "本地规则已直接拦截此命令，存在明确风险，不进入 AI 审查。",
                "本地規則已直接攔截此命令，存在明確風險，不進入 AI 審查。",
                "This command was blocked by local rules because it has a clearly identified risk and does not go to AI review.",
            ),
            "toolReview": review,
            "command": cmd,
        }));
    }
    let allowed_project_roots = terminal_allowed_project_roots_for_session_canonical(state, &normalized_session)?
        .iter()
        .map(|v| terminal_path_for_user(v))
        .collect::<Vec<_>>();
    let session_root = terminal_session_root_canonical(state, &normalized_session)?;
    let session_root_text = terminal_path_for_user(&session_root);
    let workspace_path_text = session_root_text.clone();
    let cwd = match resolve_terminal_cwd(state, &normalized_session, None) {
        Ok(path) => path,
        Err(err) if err.contains("Call shell_switch_workspace first.") => {
            let review = terminal_local_review_value(
                &ui_language,
                "当前会话还没有切换到可执行命令的工作目录，请先切换工作目录后再执行。",
            );
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "path_not_granted",
                "message": err,
                "toolReview": review,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "readWhitelistHint": terminal_read_whitelist_usage_hint(),
                "cwd": "",
                "command": cmd,
            }));
        }
        Err(err) => return Err(err),
    };
    let timeout_ms = normalize_terminal_timeout_ms(timeout_ms);
    let command_analysis = terminal_analyze_command(&cwd, cmd, &runtime_shell.kind);
    let is_read_whitelist =
        terminal_command_is_read_whitelist(cmd, &runtime_shell.kind, &command_analysis);
    let read_whitelist_diagnostics = if is_read_whitelist {
        None
    } else {
        Some(terminal_read_whitelist_diagnostics_value(
            cmd,
            &runtime_shell.kind,
            &command_analysis,
        ))
    };
    let read_whitelist_diagnostics_message = if is_read_whitelist {
        None
    } else {
        Some(terminal_read_whitelist_diagnostics_message(
            cmd,
            &runtime_shell.kind,
            &command_analysis,
        ))
    };
    let execution_cwd = if is_read_whitelist {
        terminal_read_whitelist_cwd_for_execution(cmd, &runtime_shell.kind, &cwd)
    } else {
        cwd.clone()
    };
    let command_paths = command_analysis.path_candidates();
    let mut unmatched_paths = Vec::<TerminalCommandPathCandidate>::new();
    let mut matched_accesses = Vec::<String>::new();
    for candidate in &command_paths {
        if let Some(workspace) = terminal_match_workspace_for_session_target(state, &normalized_session, &candidate.path)? {
            matched_accesses.push(workspace.access);
        } else {
            unmatched_paths.push(candidate.clone());
        }
    }
    let write_target_paths = command_analysis.write_target_paths();
    let mut matched_write_accesses = Vec::<String>::new();
    let mut unmatched_write_targets = Vec::<PathBuf>::new();
    for path in &write_target_paths {
        if let Some(workspace) = terminal_match_workspace_for_session_target(state, &normalized_session, path)? {
            matched_write_accesses.push(workspace.access);
        } else {
            unmatched_write_targets.push(path.clone());
        }
    }
    let cwd_access = terminal_match_workspace_for_session_target(state, &normalized_session, &cwd)?
        .map(|workspace| workspace.access)
        .unwrap_or_else(|| SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string());
    let effective_access = if matched_accesses.is_empty() {
        cwd_access.clone()
    } else {
        terminal_strictest_workspace_access(&matched_accesses)
    };
    let effective_write_access = if matched_write_accesses.is_empty() {
        cwd_access.clone()
    } else {
        terminal_strictest_workspace_access(&matched_write_accesses)
    };
    let write_risk = command_analysis.write_risk.clone();
    let is_write_command = !matches!(write_risk, TerminalWriteRisk::None);
    if !is_read_whitelist {
        let relative_unmatched_paths = unmatched_paths
            .iter()
            .filter(|item| !item.is_absolute)
            .map(|item| terminal_path_for_user(&item.path))
            .collect::<Vec<_>>();
        if !relative_unmatched_paths.is_empty() {
            let review = terminal_local_review_value(
                &ui_language,
                "命令包含脱离当前工作目录的相对路径，本地规则已直接拦截，请改成当前目录内相对路径或显式绝对路径。",
            );
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "relative_path_outside_workspace",
                "message": "相对路径不能脱离当前工作目录，请改用当前目录内相对路径或显式绝对路径。",
                "toolReview": review,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "readWhitelistHint": terminal_read_whitelist_usage_hint(),
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
                "ungrantedPaths": relative_unmatched_paths,
            }));
        }

        let absolute_unmatched_paths = unmatched_paths
            .iter()
            .filter(|item| item.is_absolute)
            .map(|item| terminal_path_for_user(&item.path))
            .collect::<Vec<_>>();
        if !absolute_unmatched_paths.is_empty() {
            let message = read_whitelist_diagnostics_message.clone().unwrap_or_else(|| {
                "非读取类命令不能访问未纳管的绝对路径，请改用已授权工作目录内路径。".to_string()
            });
            let review = terminal_local_review_value(
                &ui_language,
                "命令试图访问未纳管的绝对路径，本地规则已直接拦截；非读取类操作只能作用于已授权工作目录。",
            );
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "absolute_path_not_granted",
                "message": message,
                "toolReview": review,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "readWhitelistHint": terminal_read_whitelist_usage_hint(),
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
                "ungrantedPaths": absolute_unmatched_paths,
                "readWhitelist": read_whitelist_diagnostics,
            }));
        }
    }

    if !is_read_whitelist && terminal_is_python_like_command(cmd) {
        if effective_access != SHELL_WORKSPACE_ACCESS_FULL_ACCESS {
            let review = terminal_local_review_value(
                &ui_language,
                "python/py 命令在当前目录需要完全访问权限，本地规则已直接拦截，请改用 apply_patch 或显式文件修改命令。",
            );
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "python_requires_full_access",
                "message": "python/py 命令默认不走审批；当前目录不是完全访问，请改用 apply_patch 或明确的文件修改命令。",
                "toolReview": review,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "readWhitelistHint": terminal_read_whitelist_usage_hint(),
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
            }));
        }
    } else if !is_read_whitelist && effective_access == SHELL_WORKSPACE_ACCESS_READ_ONLY {
        let message = read_whitelist_diagnostics_message.clone().unwrap_or_else(|| {
            "当前目录权限为只读，仅允许读取类白名单命令。".to_string()
        });
        let review = terminal_local_review_value(
            &ui_language,
            "当前目录权限为只读，本地规则只允许读取类白名单命令，已直接拦截本次执行。",
        );
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "read_only_workspace",
            "message": message,
            "toolReview": review,
            "rootPath": session_root_text,
            "workspacePath": workspace_path_text,
            "allowedProjectRoots": allowed_project_roots,
            "readWhitelistHint": terminal_read_whitelist_usage_hint(),
            "cwd": terminal_path_for_user(&cwd),
            "command": cmd,
            "readWhitelist": read_whitelist_diagnostics,
        }));
    } else if is_write_command {
        let unmatched_write_paths = if matches!(write_risk, TerminalWriteRisk::Unknown)
            && unmatched_write_targets.is_empty()
        {
            unmatched_paths
                .iter()
                .map(|item| terminal_path_for_user(&item.path))
                .collect::<Vec<_>>()
        } else {
            unmatched_write_targets
                .iter()
                .map(|item| terminal_path_for_user(item))
                .collect::<Vec<_>>()
        };
        if !unmatched_write_paths.is_empty() {
            let review = terminal_local_review_value(
                &ui_language,
                "写入类命令试图作用于未授权路径，本地规则已直接拦截；写入只能发生在已配置工作目录中。",
            );
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "write_path_not_granted",
                "message": "写入类命令只能作用于已配置工作目录；未纳管绝对路径仅允许读取。",
                "toolReview": review,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "readWhitelistHint": terminal_read_whitelist_usage_hint(),
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
                "ungrantedPaths": unmatched_write_paths,
            }));
        }

        if effective_write_access == SHELL_WORKSPACE_ACCESS_READ_ONLY {
            let review = terminal_local_review_value(
                &ui_language,
                "当前目录权限为只读，本地规则禁止执行写入类终端命令，已直接拦截。",
            );
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "read_only_workspace",
                "message": "当前目录权限为只读，禁止执行写入类终端命令。",
                "toolReview": review,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "readWhitelistHint": terminal_read_whitelist_usage_hint(),
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
            }));
        }
    }

    let mut smart_review_unavailable_notice = None::<String>;
    let mut smart_review_handled = false;
    let mut smart_review_history: Option<Value>;
    let effective_review_access = if is_write_command {
        effective_write_access.as_str()
    } else {
        effective_access.as_str()
    };
    let skip_smart_review =
        is_read_whitelist || effective_review_access == SHELL_WORKSPACE_ACCESS_FULL_ACCESS;
    let smart_review = if skip_smart_review {
        None
    } else {
        let review_api_config_id = current_tool_review_api_config_id(state)?;
        if let Some(review_api_config_id) = review_api_config_id {
            match terminal_run_smart_review(
                state,
                &review_api_config_id,
                &cwd,
                cmd,
                effective_review_access,
                &write_risk,
                &write_target_paths,
                match &write_risk {
                    TerminalWriteRisk::Existing { paths } => paths,
                    _ => &[],
                },
            )
            .await
            {
                Ok(TerminalSmartReviewOutcome::Decision(review)) => Some(review),
                Ok(TerminalSmartReviewOutcome::RawJson {
                    raw_json,
                    model_name,
                }) => {
                    let review_note =
                        "当前工具审查模型返回了不符合约定的结果，请直接查看原始返回内容后决定是否执行。";
                    smart_review_history = Some(serde_json::json!({
                        "kind": "raw_json",
                        "allow": false,
                        "reviewOpinion": review_note,
                        "modelName": model_name,
                        "rawContent": raw_json,
                    }));
                    if !state
                        .delegate_active_ids
                        .lock()
                        .map(|ids| ids.is_empty())
                        .unwrap_or(false)
                    {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "approved": false,
                            "blockedReason": "delegate_denied_ai_review_raw_json_command",
                            "message": "子代理工具调用被自动拒绝（智能审查返回了不符合约定的结果）。",
                            "toolReview": smart_review_history.clone(),
                            "rootPath": session_root_text,
                            "workspacePath": workspace_path_text,
                            "cwd": terminal_path_for_user(&cwd),
                        }));
                    }
                    let approved = match terminal_request_user_approval(
                        state,
                        "工具智能审查",
                        review_note,
                        &normalized_session,
                        "ai_tool_review_raw_json",
                        Some("shell_exec"),
                        Some(review_note),
                        Some(&raw_json),
                        Some(&cwd),
                        Some(cmd),
                        None,
                        None,
                        match &write_risk {
                            TerminalWriteRisk::Existing { paths } => paths,
                            _ => &[],
                        },
                        &write_target_paths,
                        Some(review_note),
                        Some(model_name.as_str()),
                    )
                    .await
                    {
                        Ok(v) => v,
                        Err(err) => return Err(err),
                    };
                    if !approved {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "approved": false,
                            "blockedReason": "user_denied_ai_review_raw_json_command",
                            "message": "用户拒绝了查看原始审查结果后的终端命令。",
                            "toolReview": smart_review_history.clone(),
                            "rootPath": session_root_text,
                            "workspacePath": workspace_path_text,
                            "cwd": terminal_path_for_user(&cwd),
                        }));
                    }
                    smart_review_handled = true;
                    None
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[工具审查] 失败 session={} command={} err={:?}",
                        normalized_session, cmd, err
                    ));
                    smart_review_unavailable_notice = Some(
                        "当前审查模型不可用，已跳过自动审查，请直接确认是否允许执行。".to_string()
                    );
                    None
                }
            }
        } else {
            None
        }
    };

    if let Some(review) = &smart_review {
        smart_review_history = Some(serde_json::json!({
            "kind": "decision",
            "allow": review.allow,
            "reviewOpinion": review.review_opinion,
            "modelName": review.model_name,
        }));
        if !review.allow {
            let mut lines = vec!["智能审查建议先由你确认后再执行。".to_string()];
            if !review.review_opinion.is_empty() {
                lines.push(format!("审查意见: {}", review.review_opinion));
            }
            if !state
                .delegate_active_ids
                .lock()
                .map(|ids| ids.is_empty())
                .unwrap_or(false)
            {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "delegate_denied_ai_reviewed_command",
                    "message": "子代理工具调用被自动拒绝（智能审查不通过）。",
                    "toolReview": smart_review_history.clone(),
                    "rootPath": session_root_text,
                    "workspacePath": workspace_path_text,
                    "cwd": terminal_path_for_user(&cwd),
                }));
            }
            let approved = match terminal_request_user_approval(
                state,
                "工具智能审查",
                &lines.join("\n"),
                &normalized_session,
                "ai_tool_review",
                Some("shell_exec"),
                None,
                None,
                Some(&cwd),
                None,
                None,
                None,
                match &write_risk {
                    TerminalWriteRisk::Existing { paths } => paths,
                    _ => &[],
                },
                &write_target_paths,
                (!review.review_opinion.is_empty()).then_some(review.review_opinion.as_str()),
                (!review.model_name.is_empty()).then_some(review.model_name.as_str()),
            )
            .await
            {
                Ok(v) => v,
                Err(err) => return Err(err),
            };
            if !approved {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "user_denied_ai_reviewed_command",
                    "message": "用户拒绝了智能审查后的终端命令。",
                    "toolReview": smart_review_history.clone(),
                    "rootPath": session_root_text,
                    "workspacePath": workspace_path_text,
                    "cwd": terminal_path_for_user(&cwd),
                }));
            }
        }
        smart_review_handled = true;
    }

    if !smart_review_handled {
        if smart_review
            .as_ref()
            .map(|review| review.allow)
            .unwrap_or(false)
            && effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL
        {
            smart_review_handled = true;
        }
    }

    if !smart_review_handled {
        match write_risk {
            TerminalWriteRisk::None => {}
            TerminalWriteRisk::NewOnly { count } => {
                runtime_log_debug(format!(
                    "[工具审查] shell_exec 写入风险=仅新建 count={} session={}",
                    count, normalized_session
                ));
                if effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL {
                    let message = format!(
                        "{}该命令将创建或改写文件，是否批准本次执行？\n会话: {normalized_session}\n工作目录: {}\n命令: {cmd}",
                        smart_review_unavailable_notice
                            .as_deref()
                            .map(|text| format!("{text}\n"))
                            .unwrap_or_default(),
                        terminal_path_for_user(&cwd)
                    );
                    let summary = format!("该命令将创建或改写 {} 个新路径。", count);
                    if !state
                        .delegate_active_ids
                        .lock()
                        .map(|ids| ids.is_empty())
                        .unwrap_or(false)
                    {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "approved": false,
                            "blockedReason": "delegate_denied_write_risk_command",
                            "message": "子代理工具调用被自动拒绝（存在写入风险且无审查模型）。",
                            "rootPath": session_root_text,
                            "workspacePath": workspace_path_text,
                            "cwd": terminal_path_for_user(&cwd),
                            "command": cmd,
                        }));
                    }
                    let approved = match terminal_request_user_approval(
                        state,
                        "终端执行审批",
                        &message,
                        &normalized_session,
                        "new_write_risk",
                        Some("shell_exec"),
                        Some(&summary),
                        Some(cmd),
                        Some(&cwd),
                        Some(cmd),
                        None,
                        smart_review_unavailable_notice.as_deref(),
                        &[],
                        &[],
                        None,
                        None,
                    )
                    .await
                    {
                        Ok(v) => v,
                        Err(err) => return Err(err),
                    };
                    if !approved {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "approved": false,
                            "blockedReason": "user_denied_new_file_change",
                            "message": "用户拒绝了本次写入类终端命令。",
                            "rootPath": session_root_text,
                            "workspacePath": workspace_path_text,
                            "cwd": terminal_path_for_user(&cwd),
                            "command": cmd,
                        }));
                    }
                }
            }
            TerminalWriteRisk::Existing { paths } => {
                if effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL {
                    let mut lines = vec![
                        "该命令将修改/删除已有文件，是否批准本次执行？".to_string(),
                        format!("会话: {normalized_session}"),
                        format!("工作目录: {}", terminal_path_for_user(&cwd)),
                        format!("命令: {cmd}"),
                        "命中已有路径：".to_string(),
                    ];
                    if let Some(notice) = &smart_review_unavailable_notice {
                        lines.insert(0, notice.clone());
                    }
                    for path in paths.iter().take(8) {
                        lines.push(format!("- {}", terminal_path_for_user(path)));
                    }
                    if paths.len() > 8 {
                        lines.push(format!("... 其余 {} 项已省略", paths.len() - 8));
                    }
                    let summary = format!("该命令将修改或删除 {} 个已有路径。", paths.len());
                    if !state
                        .delegate_active_ids
                        .lock()
                        .map(|ids| ids.is_empty())
                        .unwrap_or(false)
                    {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "approved": false,
                            "blockedReason": "delegate_denied_write_risk_command",
                            "message": "子代理工具调用被自动拒绝（存在写入风险）。",
                            "rootPath": session_root_text,
                            "workspacePath": workspace_path_text,
                            "cwd": terminal_path_for_user(&cwd),
                            "command": cmd,
                        }));
                    }
                    let approved = match terminal_request_user_approval(
                        state,
                        "终端执行审批",
                        &lines.join("\n"),
                        &normalized_session,
                        "existing_write_risk",
                        Some("shell_exec"),
                        Some(&summary),
                        Some(cmd),
                        Some(&cwd),
                        Some(cmd),
                        None,
                        smart_review_unavailable_notice.as_deref(),
                        &paths,
                        &paths,
                        None,
                        None,
                    )
                    .await
                    {
                        Ok(v) => v,
                        Err(err) => return Err(err),
                    };
                    if !approved {
                        return Ok(serde_json::json!({
                            "ok": false,
                            "approved": false,
                            "blockedReason": "user_denied_existing_file_change",
                            "message": "用户拒绝了本次写入类终端命令。",
                            "rootPath": session_root_text,
                            "workspacePath": workspace_path_text,
                            "cwd": terminal_path_for_user(&cwd),
                            "command": cmd,
                        }));
                    }
                }
            }
            TerminalWriteRisk::Unknown => {
                if effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL {
                    let review = terminal_local_review_value(
                        &ui_language,
                        "命令可能写入，但本地规则无法确认具体目标；在审批目录下，这类不明确写入会被直接拦截，请改成更明确的写入命令。",
                    );
                    return Ok(serde_json::json!({
                        "ok": false,
                        "approved": false,
                        "blockedReason": "approval_requires_explicit_write_command",
                        "message": format!(
                            "{}当前目录需要审批，但该命令无法明确识别具体写入目标，请改用 apply_patch 或更明确的文件修改命令。",
                            smart_review_unavailable_notice
                                .as_deref()
                                .map(|text| format!("{text} "))
                                .unwrap_or_default()
                        ),
                        "toolReview": review,
                        "rootPath": session_root_text,
                        "workspacePath": workspace_path_text,
                        "cwd": terminal_path_for_user(&cwd),
                        "command": cmd,
                    }));
                }
            }
        }
    }

    let execution_result = if terminal_live_session_supported(&runtime_shell) {
        terminal_live_exec_command(state, &normalized_session, &execution_cwd, cmd, timeout_ms).await
    } else {
        sandbox_execute_command(state, &normalized_session, cmd, &execution_cwd, timeout_ms).await
    };
    let execution = match execution_result {
        Ok(execution) => execution,
        Err(err) if terminal_is_timeout_error(&err) => {
            return Ok(serde_json::json!({
                "ok": false,
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

#[cfg(test)]
mod terminal_exec_tests {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::time::{timeout, Duration};

    fn build_test_state(shell: TerminalShellProfile, llm_workspace_path: PathBuf) -> AppState {
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: llm_workspace_path.join("app_config.toml"),
            data_path: llm_workspace_path.join("app_data.json"),
            llm_workspace_path,
            shared_http_client: reqwest::Client::new(),
            terminal_shell: shell.clone(),
            terminal_shell_candidates: vec![shell],
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_runtime_state: Arc::new(Mutex::new(None)),
            cached_runtime_state_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_chat_index_mtime: Arc::new(Mutex::new(None)),
            cached_conversations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_chat_index_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(HashSet::new())),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            conversation_index_repair_gates: Arc::new(Mutex::new(HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    fn shell_candidate_by_kind(kind: &str) -> Option<TerminalShellProfile> {
        detect_terminal_shell_candidates()
            .into_iter()
            .find(|item| item.kind == kind)
    }

    fn configure_test_workspaces(
        state: &AppState,
        _main_access: &str,
        _secondary_access: &str,
    ) -> Result<(PathBuf, PathBuf, PathBuf), String> {
        let system_root = state.llm_workspace_path.clone();
        let main_root = system_root.join("main-workspace");
        let secondary_root = system_root.join("secondary-workspace");
        fs::create_dir_all(&system_root).map_err(|err| format!("create system root failed: {err}"))?;
        fs::create_dir_all(&main_root).map_err(|err| format!("create main root failed: {err}"))?;
        fs::create_dir_all(&secondary_root).map_err(|err| format!("create secondary root failed: {err}"))?;
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "系统工作目录".to_string(),
            path: terminal_path_for_user(&system_root),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(state, &config).map_err(|err| format!("write config failed: {err}"))?;
        Ok((system_root, main_root, secondary_root))
    }

    fn configure_test_conversation_workspaces(
        state: &AppState,
        conversation_id: &str,
        agent_id: &str,
        locked_root: Option<&Path>,
        main_root: &Path,
        main_access: &str,
        secondary_root: &Path,
        secondary_access: &str,
    ) -> Result<String, String> {
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: conversation_id.to_string(),
            title: "Terminal Test Conversation".to_string(),
            agent_id: agent_id.to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: locked_root.map(terminal_path_for_user),
            shell_workspaces: vec![
                ShellWorkspaceConfig {
                    id: "main-workspace-1".to_string(),
                    name: "主要工作目录".to_string(),
                    path: terminal_path_for_user(main_root),
                    level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                    access: main_access.to_string(),
                    built_in: false,
                },
                ShellWorkspaceConfig {
                    id: "secondary-workspace-1".to_string(),
                    name: "次要工作目录".to_string(),
                    path: terminal_path_for_user(secondary_root),
                    level: SHELL_WORKSPACE_LEVEL_SECONDARY.to_string(),
                    access: secondary_access.to_string(),
                    built_in: false,
                },
            ],
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
        state_write_app_data_cached(state, &data)
            .map_err(|err| format!("write app data failed: {err}"))?;
        Ok(normalize_terminal_tool_session_id(&inflight_chat_key(
            agent_id,
            Some(conversation_id),
        )))
    }

    async fn verify_default_workspace_skip_for_shell(kind: &str) -> Result<(), String> {
        let Some(shell) = shell_candidate_by_kind(kind) else {
            eprintln!("[TEST] skip shell kind={kind}: not available on this machine");
            return Ok(());
        };

        let root = std::env::temp_dir().join(format!("eca-terminal-skip-{}-{}", kind, Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        let existing_file = root.join("existing.txt");
        fs::write(&existing_file, "before\n").map_err(|err| format!("seed file failed: {err}"))?;

        let state = build_test_state(shell, root.clone());
        let started = std::time::Instant::now();
        let run = builtin_shell_exec(
            &state,
            "test-session",
            "run",
            "echo changed > ./existing.txt",
            Some(8_000),
        );
        let result = timeout(Duration::from_secs(15), run)
            .await
            .map_err(|_| "builtin_shell_exec timed out (likely waiting for approval)".to_string())??;

        let elapsed = started.elapsed();
        let approvals_left = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "lock terminal_pending_approvals failed".to_string())?
            .len();
        if approvals_left != 0 {
            return Err(format!("unexpected pending approvals count: {approvals_left}"));
        }
        if elapsed > Duration::from_secs(15) {
            return Err(format!("execution took too long: {elapsed:?}"));
        }
        let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if !ok {
            return Err(format!("shell_exec returned non-ok: {}", result));
        }

        let content = fs::read_to_string(&existing_file)
            .map_err(|err| format!("read updated file failed: {err}"))?;
        if !content.contains("changed") {
            return Err(format!("existing file not updated as expected, content={content:?}"));
        }

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    async fn verify_dev_null_read_command_for_shell(kind: &str) -> Result<(), String> {
        let Some(shell) = shell_candidate_by_kind(kind) else {
            eprintln!("[TEST] skip shell kind={kind}: not available on this machine");
            return Ok(());
        };

        let root =
            std::env::temp_dir().join(format!("eca-terminal-devnull-{}-{}", kind, Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        let archive_dir = root.join("archive");
        fs::create_dir_all(&archive_dir)
            .map_err(|err| format!("create archive dir failed: {err}"))?;

        let state = build_test_state(shell, root.clone());
        let (_system_root, main_root, secondary_root) =
            configure_test_workspaces(&state, "full_access", "read_only")?;
        let session_id = configure_test_conversation_workspaces(
            &state,
            "dev-null-read-conversation",
            DEFAULT_AGENT_ID,
            Some(main_root.as_path()),
            &main_root,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            &secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )?;
        let command = if kind.starts_with("powershell") {
            "Get-ChildItem .\\archive 2>nul"
        } else {
            "ls -la ./archive 2>/dev/null || true"
        };

        let result = timeout(
            Duration::from_secs(15),
            builtin_shell_exec(&state, &session_id, "run", command, Some(8_000)),
        )
        .await
        .map_err(|_| "builtin_shell_exec timed out".to_string())??;

        let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let blocked_reason = result.get("blockedReason").and_then(Value::as_str);
        if !ok || blocked_reason.is_some() {
            return Err(format!("dev-null read command unexpectedly blocked: {}", result));
        }

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[tokio::test]
    async fn git_bash_dev_null_read_command_should_not_require_write_access() {
        verify_dev_null_read_command_for_shell("git-bash")
            .await
            .expect("git-bash dev null regression should pass");
    }

    #[tokio::test]
    async fn powershell_dev_null_read_command_should_not_require_write_access() {
        let kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        verify_dev_null_read_command_for_shell(kind)
            .await
            .expect("powershell dev null regression should pass");
    }

    #[tokio::test]
    async fn default_workspace_skip_approval_for_powershell() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        if let Err(err) = verify_default_workspace_skip_for_shell(powershell_kind).await {
            panic!("powershell default-workspace skip check failed: {err}");
        }
    }

    #[tokio::test]
    async fn default_workspace_skip_approval_for_git_bash() {
        if let Err(err) = verify_default_workspace_skip_for_shell("git-bash").await {
            panic!("git-bash default-workspace skip check failed: {err}");
        }
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn unmatched_absolute_read_should_be_allowed() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-read-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");

        let result = builtin_shell_exec(
            &state,
            "read-outside-session",
            "run",
            "Get-Content C:\\Windows\\win.ini | Select-Object -First 1",
            Some(8_000),
        )
        .await
        .expect("run read command");

        assert_eq!(result.get("blockedReason").and_then(Value::as_str), None);
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(true));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn unmatched_absolute_write_should_be_blocked() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-write-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let outside_path = std::env::temp_dir().join(format!("eca-unmanaged-write-{}.txt", Uuid::new_v4()));
        let command = format!(
            "Set-Content -Path '{}' -Value 'hi'",
            outside_path.to_string_lossy()
        );

        let result = builtin_shell_exec(&state, "write-outside-session", "run", &command, Some(8_000))
            .await
            .expect("run write command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("absolute_path_not_granted")
        );
        assert!(!outside_path.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn approval_workspace_should_reject_python_command() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-python-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        let (_, main_root, secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-python-approval",
            "agent-python-approval",
            Some(&main_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            &secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "python -c \"print('hello')\"",
            Some(8_000),
        )
        .await
        .expect("run python command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("python_requires_full_access")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tool_safety_prompt_should_describe_approval_allow_examples() {
        let prompt = tool_safety_review_system_prompt("简体中文");

        assert!(prompt.contains("只读取、只查询、只检查、只测试、只输出结果"));
        assert!(prompt.contains("应返回 allow=true"));
        assert!(prompt.contains("curl、wget、Invoke-WebRequest"));
        assert!(prompt.contains("通过管道直接执行脚本，则应返回 allow=false"));
    }

    #[test]
    fn approval_allowing_smart_review_should_skip_followup_write_prompt() {
        let mut smart_review_handled = false;
        let smart_review = Some(TerminalSmartReviewDecision {
            allow: true,
            review_opinion: "只读检查".to_string(),
            model_name: "mock".to_string(),
        });
        let effective_write_access = SHELL_WORKSPACE_ACCESS_APPROVAL;

        if !smart_review_handled {
            if smart_review
                .as_ref()
                .map(|review| review.allow)
                .unwrap_or(false)
                && effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL
            {
                smart_review_handled = true;
            }
        }

        assert!(smart_review_handled);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn read_only_workspace_should_block_write_command() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-readonly-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        let (_, main_root, secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-read-only",
            "agent-read-only",
            Some(&main_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
            &secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "Set-Content -Path .\\note.txt -Value 'hi'",
            Some(8_000),
        )
        .await
        .expect("run readonly command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("read_only_workspace")
        );
        assert!(!secondary_root.join("note.txt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn unmatched_absolute_read_should_block_non_whitelist_command() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-mixed-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        let (_system_root, main_root, _secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-mixed-read-write",
            "agent-mixed-read-write",
            Some(&main_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            &_secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "Get-Content C:\\Windows\\win.ini | Select-Object -First 1 | Set-Content -Path '.\\note.txt'",
            Some(8_000),
        )
        .await
        .expect("run mixed read/write command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("absolute_path_not_granted")
        );
        assert!(!main_root.join("note.txt").exists(), "note.txt should not be created");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn powershell_set_location_then_rg_should_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command =
            r#"Set-Location 'E:/github/easy_call_ai'; rg -n "codex|status" src src-tauri README.md"#;
        let analysis = terminal_analyze_command(&cwd, command, "powershell7");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis.has_directory_change);
        assert!(terminal_command_is_read_whitelist(
            command,
            "powershell7",
            &analysis
        ));
    }

    #[test]
    fn powershell_set_location_then_pnpm_typecheck_should_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command = r#"Set-Location 'E:/github/easy_call_ai'; pnpm typecheck"#;
        let analysis = terminal_analyze_command(&cwd, command, "powershell7");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis.has_directory_change);
        assert!(terminal_command_is_read_whitelist(
            command,
            "powershell7",
            &analysis
        ));
    }

    #[test]
    fn powershell_set_location_then_git_diff_should_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command = r#"Set-Location 'E:/github/easy_call_ai'; git diff --stat"#;
        let analysis = terminal_analyze_command(&cwd, command, "powershell7");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis.has_directory_change);
        assert!(terminal_command_is_read_whitelist(
            command,
            "powershell7",
            &analysis
        ));
    }

    #[test]
    fn git_bash_output_separators_should_keep_git_diff_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command = "cd /e/github/easy_call_ai && git status --porcelain && printf '\\n---DIFF---\\n' && git diff HEAD --stat && printf '\\n---FULLDIFF---\\n' && git diff HEAD";
        let analysis = terminal_analyze_command(&cwd, command, "git-bash");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis.has_directory_change);
        assert!(terminal_command_is_read_whitelist(
            command,
            "git-bash",
            &analysis
        ));
    }

    #[test]
    fn git_bash_code_review_git_metadata_should_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command = r#"cd /e/github/easy_call_ai && git merge-base HEAD main && git rev-parse --show-toplevel && git branch --show-current"#;
        let analysis = terminal_analyze_command(&cwd, command, "git-bash");

        assert_eq!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(analysis.has_directory_change);
        assert!(terminal_command_is_read_whitelist(
            command,
            "git-bash",
            &analysis
        ));
    }

    #[test]
    fn common_git_read_only_commands_should_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let commands = [
            "git log --oneline -5",
            "git show --stat HEAD",
            "git reflog -5",
            "git diff --stat",
            "git status --porcelain",
            "git branch --all",
            "git tag --list",
            "git ls-remote --heads origin",
            "git blame -- src/main.rs",
            "git ls-files",
            "git grep TODO",
            "git config --get remote.origin.url",
            "git cat-file -p HEAD",
            "git ls-tree HEAD",
        ];

        for command in commands {
            let analysis = terminal_analyze_command(&cwd, command, "git-bash");
            assert_eq!(analysis.write_risk, TerminalWriteRisk::None, "{command}");
            assert!(
                terminal_command_is_read_whitelist(command, "git-bash", &analysis),
                "{command}"
            );
        }
    }

    #[test]
    fn git_branch_tag_config_write_forms_should_not_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let commands = [
            "git branch -d feature",
            "git branch -D feature",
            "git branch --delete feature",
            "git branch -m old new",
            "git branch -M old new",
            "git branch -c old new",
            "git branch -C old new",
            "git branch feature",
            "git tag -d v1.0",
            "git tag --delete v1.0",
            "git tag -a v1.0 -m release",
            "git tag -f v1.0 HEAD",
            "git tag v1.0",
            "git config --global user.name xxx",
            "git config user.name xxx",
            "git config --unset user.name",
            "git config --replace-all user.name xxx",
        ];

        for command in commands {
            let analysis = terminal_analyze_command(&cwd, command, "git-bash");
            assert!(
                !terminal_command_is_read_whitelist(command, "git-bash", &analysis),
                "{command}"
            );
        }
    }

    #[test]
    fn output_only_command_with_redirection_should_not_be_read_whitelist() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command = "printf 'changed' > generated.txt";
        let analysis = terminal_analyze_command(&cwd, command, "git-bash");

        assert_ne!(analysis.write_risk, TerminalWriteRisk::None);
        assert!(!terminal_command_is_read_whitelist(
            command,
            "git-bash",
            &analysis
        ));
    }

    #[test]
    fn read_whitelist_diagnostics_should_split_allowed_and_denied_commands() {
        let cwd = PathBuf::from("E:\\github\\easy_call_ai");
        let command = "git diff --stat && foo-review-helper && git status --porcelain";
        let analysis = terminal_analyze_command(&cwd, command, "git-bash");
        let diagnostics = terminal_read_whitelist_diagnostics_value(command, "git-bash", &analysis);

        let whitelisted = diagnostics
            .get("whitelistedCommands")
            .and_then(Value::as_array)
            .expect("whitelisted commands");
        let non_whitelisted = diagnostics
            .get("nonWhitelistedCommands")
            .and_then(Value::as_array)
            .expect("non-whitelisted commands");
        assert!(whitelisted.iter().any(|item| item.as_str() == Some("git diff --stat")));
        assert!(whitelisted.iter().any(|item| item.as_str() == Some("git status --porcelain")));
        assert!(non_whitelisted.iter().any(|item| {
            item.get("command").and_then(Value::as_str) == Some("foo-review-helper")
        }));
    }

    #[test]
    fn read_whitelist_usage_hint_should_explain_directory_permission_bypass() {
        let hint = terminal_read_whitelist_usage_hint();
        assert!(hint.contains("只读命令白名单不受目录权限影响"));
        assert!(hint.contains("git status"));
        assert!(hint.contains("git diff"));
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn git_bash_read_whitelist_should_run_outside_authorized_roots() {
        let Some(shell) = shell_candidate_by_kind("git-bash") else {
            return;
        };
        let system_root = std::env::temp_dir().join(format!("eca-terminal-system-{}", Uuid::new_v4()));
        let outside_root = std::env::temp_dir().join(format!("eca-terminal-outside-{}", Uuid::new_v4()));
        fs::create_dir_all(&system_root).expect("create system root");
        fs::create_dir_all(&outside_root).expect("create outside root");
        std::process::Command::new("git")
            .arg("init")
            .current_dir(&outside_root)
            .output()
            .expect("git init outside root");

        let state = build_test_state(shell, system_root.clone());
        configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let command = format!(
            "cd '{}' && git status --porcelain && git diff --stat",
            terminal_path_for_user(&outside_root).replace('\\', "/")
        );

        let result = builtin_shell_exec(
            &state,
            "git-read-outside-roots",
            "run",
            &command,
            Some(8_000),
        )
        .await
        .expect("run git read command outside roots");

        assert_eq!(result.get("blockedReason").and_then(Value::as_str), None);
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(true));
        let _ = fs::remove_dir_all(&system_root);
        let _ = fs::remove_dir_all(&outside_root);
    }

    #[tokio::test]
    async fn blocked_local_rule_should_return_local_tool_review() {
        let root = std::env::temp_dir().join(format!("eca-terminal-blocked-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let shell = shell_candidate_by_kind("powershell7")
            .or_else(|| shell_candidate_by_kind("powershell5"))
            .expect("powershell shell");
        let state = build_test_state(shell, root.clone());
        let (_system_root, main_root, _secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-blocked-local-rule",
            "agent-blocked-local-rule",
            Some(&main_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            &_secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "powershell -EncodedCommand AAAA",
            Some(8_000),
        )
        .await
        .expect("run blocked command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("local_rule_blocked")
        );
        assert_eq!(
            result
                .get("toolReview")
                .and_then(|v| v.get("kind"))
                .and_then(Value::as_str),
            Some("local_rule")
        );
        assert_eq!(
            result
                .get("toolReview")
                .and_then(|v| v.get("allow"))
                .and_then(Value::as_bool),
            Some(false)
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn shell_exec_result_should_not_expose_session_id() -> Result<(), String> {
        let Some(shell) = shell_candidate_by_kind("git-bash") else {
            eprintln!("[TEST] skip shell kind=git-bash: not available on this machine");
            return Ok(());
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-no-session-id-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        let state = build_test_state(shell, root.clone());

        let run_result = builtin_shell_exec(&state, "no-session-id", "run", "echo ok", Some(8_000)).await?;
        assert!(run_result.get("sessionId").is_none(), "run result leaked sessionId: {run_result}");

        let list_result = builtin_shell_exec(&state, "no-session-id", "list", "", Some(8_000)).await?;
        assert!(list_result.get("sessionId").is_none(), "list result leaked sessionId: {list_result}");
        for session in list_result
            .get("sessions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            assert!(session.get("sessionId").is_none(), "listed session leaked sessionId: {session}");
        }

        let close_result = builtin_shell_exec(&state, "no-session-id", "close", "", Some(8_000)).await?;
        assert!(close_result.get("sessionId").is_none(), "close result leaked sessionId: {close_result}");

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
