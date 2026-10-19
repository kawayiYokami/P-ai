#[derive(Debug, Clone)]
struct TerminalShellProfile {
    kind: String,
    path: String,
    args_prefix: Vec<String>,
}

#[cfg(target_os = "windows")]
fn terminal_apply_windows_utf8_env<T>(command_builder: &mut T)
where
    T: CommandExtUtf8Env,
{
    command_builder.env("LANG", "en_US.UTF-8");
    command_builder.env("LC_ALL", "en_US.UTF-8");
    command_builder.env("PYTHONUTF8", "1");
    command_builder.env("PYTHONIOENCODING", "utf-8");
}

#[cfg(target_os = "windows")]
mod terminal_windows_command_ext {
    pub trait CommandExtUtf8Env {
        fn env(&mut self, key: &str, value: &str) -> &mut Self;
    }

    impl CommandExtUtf8Env for tokio::process::Command {
        fn env(&mut self, key: &str, value: &str) -> &mut Self {
            tokio::process::Command::env(self, key, value)
        }
    }

    impl CommandExtUtf8Env for std::process::Command {
        fn env(&mut self, key: &str, value: &str) -> &mut Self {
            std::process::Command::env(self, key, value)
        }
    }
}

#[cfg(target_os = "windows")]
use terminal_windows_command_ext::CommandExtUtf8Env;

#[derive(Debug)]
struct TerminalLiveShellSession {
    shell_kind: String,
    shell_path: String,
    created_at: String,
    last_used_at: tokio::sync::Mutex<String>,
    child: tokio::sync::Mutex<tokio::process::Child>,
}

type TerminalLiveShellSessionHandle = std::sync::Arc<TerminalLiveShellSession>;

const TERMINAL_LIVE_CLOSE_WAIT_MS: u64 = 2_000;

async fn terminal_live_kill_child_with_timeout(
    child: &mut tokio::process::Child,
    context: &str,
) {
    let _ = child.kill().await;
    match tokio::time::timeout(
        std::time::Duration::from_millis(TERMINAL_LIVE_CLOSE_WAIT_MS),
        child.wait(),
    )
    .await
    {
        Ok(_) => {}
        Err(_) => runtime_log_error(format!(
            "[终端] live shell 关闭等待超时: context={}, timeout_ms={}",
            context, TERMINAL_LIVE_CLOSE_WAIT_MS
        )),
    }
}

async fn terminal_live_close_session(state: &AppState, session_id: &str) -> Result<bool, String> {
    let normalized = normalize_terminal_tool_session_id(session_id);
    let removed = {
        let mut sessions = state.terminal_live_sessions.lock().await;
        sessions.remove(&normalized)
    };
    let Some(handle) = removed else {
        return Ok(false);
    };
    let mut child = handle.child.lock().await;
    terminal_live_kill_child_with_timeout(&mut child, "close_session").await;
    Ok(true)
}

#[cfg(target_os = "windows")]
fn terminal_powershell_escape_literal(input: &str) -> String {
    input.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn terminal_strip_windows_verbatim_prefix(input: &str) -> String {
    let text = input.trim();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", rest);
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    text.to_string()
}

fn terminal_path_for_user(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        terminal_strip_windows_verbatim_prefix(&text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        text
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalBackgroundShellStatus {
    Running,
    Completed,
    Failed,
    Killed,
    TimedOut,
}

#[derive(Debug)]
struct TerminalBackgroundShellTask {
    id: String,
    _session_id: String,
    conversation_id: String,
    command: String,
    cwd: String,
    description: String,
    started_at: String,
    /// 后台任务不受前台 timeout 约束：None 表示跑到自然结束或被 kill。
    timeout_ms: Option<u64>,
    log_path: std::path::PathBuf,
    status: std::sync::Mutex<TerminalBackgroundShellStatus>,
    exit_code: std::sync::Mutex<Option<i32>>,
    kill_requested: std::sync::atomic::AtomicBool,
    /// 谁发起的 kill：user / assistant / system，None 表示尚未 kill。
    kill_initiator: std::sync::Mutex<Option<String>>,
    /// kill 信号；monitor 通过 watch 感知，发出早于监听也不会丢（watch 保留最新值）。
    kill_signal_tx: tokio::sync::watch::Sender<bool>,
}

type TerminalBackgroundShellTaskHandle = std::sync::Arc<TerminalBackgroundShellTask>;

/// 输出全部落盘，不做内存缓冲；读取时只取文件尾部。
fn terminal_background_shell_log_path(id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{id}.log"))
}

fn terminal_background_shell_log_tail(log_path: &std::path::Path, max_chars: usize) -> String {
    use std::io::{Read as _, Seek as _, SeekFrom};

    let Ok(mut file) = std::fs::File::open(log_path) else {
        return String::new();
    };
    let file_len = file
        .metadata()
        .map(|meta| meta.len())
        .unwrap_or(0) as u64;
    // 多预留一些字节，避免截在多字节字符中间导致首字符乱码。
    let tail_bytes = (max_chars.saturating_mul(4)) as u64;
    let start = file_len.saturating_sub(tail_bytes);
    if file
        .seek(SeekFrom::Start(start))
        .is_err()
    {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let skip = text.chars().count().saturating_sub(max_chars);
    text.chars().skip(skip).collect()
}

fn terminal_background_shell_display_description(task: &TerminalBackgroundShellTask) -> &str {
    let description = task.description.trim();
    if description.is_empty() {
        task.command.trim()
    } else {
        description
    }
}

fn terminal_background_shell_status_text(task: &TerminalBackgroundShellTask) -> String {
    let status = *task.status.lock().expect("terminal background status poisoned");
    format!(
        "id={}\ndescription={}\nstatus={:?}\nexitCode={:?}\ncommand={}\ncwd={}\nstartedAt={}\ntimeoutMs={}\nlog={}\noutputTail={}",
        task.id,
        terminal_background_shell_display_description(task),
        status,
        *task.exit_code.lock().expect("terminal background exit code poisoned"),
        task.command,
        task.cwd,
        task.started_at,
        task.timeout_ms.map(|value| value.to_string()).unwrap_or_else(|| "none".to_string()),
        terminal_path_for_user(&task.log_path),
        terminal_background_shell_log_tail(&task.log_path, 600),
    )
}

// ==================== 监控面板摘要与终止（Tauri command / builtin 共用） ====================

/// 监控面板用任务摘要：在 builtin list 字段之上追加 outputTail，供前端直接展示运行进展。
async fn terminal_background_shell_monitor_summaries(
    state: &AppState,
    conversation_id: &str,
) -> Vec<Value> {
    let handles = {
        let tasks = state.terminal_background_shell_tasks.lock().await;
        tasks.values().cloned().collect::<Vec<_>>()
    };
    let mut out = Vec::<Value>::new();
    for task in handles {
        if task.conversation_id.trim() != conversation_id.trim() {
            continue;
        }
        let status = *task.status.lock().expect("terminal background status poisoned");
        let exit_code = *task.exit_code.lock().expect("terminal background exit code poisoned");
        out.push(serde_json::json!({
            "id": task.id,
            "kind": "shell",
            "status": terminal_background_shell_status_label(status),
            "exitCode": exit_code,
            "description": terminal_background_shell_display_description(&task),
            "command": task.command,
            "cwd": task.cwd,
            "startedAt": task.started_at,
            "timeoutMs": task.timeout_ms,
            "log": terminal_path_for_user(&task.log_path),
            "outputTail": terminal_background_shell_log_tail(&task.log_path, 800),
        }));
    }
    out
}

/// 请求终止后台任务：kill 信号 + 等待 monitor 确认终态；重复终止或已终态时幂等返回。
/// 返回 (killed, confirmed, status_label, log_path)。
async fn terminal_background_shell_request_kill(
    state: &AppState,
    conversation_id: &str,
    task_id: &str,
    initiator: &str,
) -> Result<(bool, bool, String, String), String> {
    let maybe_task = terminal_background_shell_find(state, conversation_id, task_id).await;
    let Some(task) = maybe_task else {
        return Err(format!("background id not found: {task_id}"));
    };
    let current_status = *task.status.lock().expect("terminal background status poisoned");
    let log_path = terminal_path_for_user(&task.log_path);
    if terminal_background_shell_is_terminal(current_status) {
        let label = terminal_background_shell_status_label(current_status).to_string();
        return Ok((false, true, label, log_path));
    }
    task.kill_requested.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut guard) = task.kill_initiator.lock() {
        *guard = Some(initiator.trim().to_string());
    }
    let _ = task.kill_signal_tx.send(true);
    // 等待 monitor 确认终态（含写回与登记表清理）；确认失败时只报告请求已受理。
    let mut confirmed = false;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let status = *task.status.lock().expect("terminal background status poisoned");
        if terminal_background_shell_is_terminal(status) {
            confirmed = true;
            break;
        }
    }
    let final_status = *task.status.lock().expect("terminal background status poisoned");
    Ok((true, confirmed, terminal_background_shell_status_label(final_status).to_string(), log_path))
}

async fn terminal_background_shell_register(state: &AppState, task: TerminalBackgroundShellTaskHandle) {
    let conversation_id = task.conversation_id.clone();
    let task_id = task.id.clone();
    {
        let mut tasks = state.terminal_background_shell_tasks.lock().await;
        tasks.insert(task.id.clone(), task);
    }
    terminal_background_shell_prune_terminal_records(state, &conversation_id).await;
    // 登记即发布：运行时写路径统一广播，前端收到后拉快照，监控 bar 计数即时可见
    monitor_publish_changed(state, MonitorDomain::BackgroundShell, "started", &conversation_id, &task_id);
}

/// 终态任务保留在登记表供 AI 对账（完成通知可能被压缩或丢失，list 是唯一找回通道）；
/// 超出上限时按 startedAt 淘汰最老的终态条目。
const MAX_BACKGROUND_SHELL_RECORDS_PER_CONVERSATION: usize = 32;

async fn terminal_background_shell_prune_terminal_records(state: &AppState, conversation_id: &str) {
    let mut tasks = state.terminal_background_shell_tasks.lock().await;
    let mut conversation_tasks = tasks
        .values()
        .filter(|task| task.conversation_id.trim() == conversation_id.trim())
        .cloned()
        .collect::<Vec<_>>();
    conversation_tasks.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    let mut terminal_surplus = conversation_tasks
        .len()
        .saturating_sub(MAX_BACKGROUND_SHELL_RECORDS_PER_CONVERSATION);
    for task in conversation_tasks {
        if terminal_surplus == 0 {
            break;
        }
        let status = *task.status.lock().expect("terminal background status poisoned");
        if terminal_background_shell_is_terminal(status) {
            tasks.remove(task.id.as_str());
            terminal_surplus -= 1;
        }
    }
}

async fn terminal_background_shell_list(state: &AppState, conversation_id: &str) -> Vec<Value> {
    let handles = {
        let tasks = state.terminal_background_shell_tasks.lock().await;
        tasks.values().cloned().collect::<Vec<_>>()
    };
    let mut out = Vec::<Value>::new();
    for task in handles {
        if task.conversation_id.trim() != conversation_id.trim() {
            continue;
        }
        let status = *task.status.lock().expect("terminal background status poisoned");
        out.push(serde_json::json!({
            "id": task.id,
            "kind": "shell",
            "status": format!("{:?}", status),
            "description": terminal_background_shell_display_description(&task),
            "command": task.command,
            "cwd": task.cwd,
            "startedAt": task.started_at,
            "timeoutMs": task.timeout_ms,
            "log": terminal_path_for_user(&task.log_path),
        }));
    }
    out
}

async fn terminal_background_shell_find(
    state: &AppState,
    session_id: &str,
    task_id: &str,
) -> Option<TerminalBackgroundShellTaskHandle> {
    let tasks = state.terminal_background_shell_tasks.lock().await;
    tasks
        .get(task_id)
        .cloned()
        .filter(|task| task.conversation_id.trim() == session_id.trim())
}

#[cfg(target_os = "windows")]
fn terminal_background_shell_spawn_windows(
    shell: &TerminalShellProfile,
    command: &str,
    cwd: &std::path::Path,
    log_path: &std::path::Path,
) -> Result<(std::process::Child, WindowsJobGuard), String> {
    use std::os::windows::io::AsRawHandle as _;
    use std::os::windows::process::CommandExt as _;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    let log_file = terminal_background_shell_open_log(log_path)?;
    let log_stderr = log_file
        .try_clone()
        .map_err(|err| format!("background shell log clone failed: {err}"))?;

    let mut command_builder = std::process::Command::new(&shell.path);
    command_builder.current_dir(cwd);
    command_builder.stdout(std::process::Stdio::from(log_file));
    command_builder.stderr(std::process::Stdio::from(log_stderr));
    command_builder.stdin(std::process::Stdio::null());
    command_builder.creation_flags(CREATE_NO_WINDOW);
    terminal_apply_windows_utf8_env(&mut command_builder);
    for arg in &shell.args_prefix {
        command_builder.arg(arg);
    }
    command_builder.arg(&terminal_background_shell_wrap_command_for_shell(shell, command));
    let child = command_builder
        .spawn()
        .map_err(|err| format!("background shell spawn failed: {err}"))?;
    let job = WindowsJobGuard::create_kill_on_close()?;
    job.assign_raw_process_handle(child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE)
        .map_err(|err| format!("{}: pid={}", err, child.id()))?;
    Ok((child, job))
}

#[cfg(target_os = "windows")]
fn terminal_background_shell_wrap_command_for_shell(
    shell: &TerminalShellProfile,
    command: &str,
) -> String {
    if matches!(shell.kind.as_str(), "powershell7" | "powershell5") {
        return format!(
            "$ErrorActionPreference='Continue'; try {{ [Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false); [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding; chcp.com 65001 > $null; $env:PYTHONUTF8='1'; $env:PYTHONIOENCODING='utf-8'; {command} }} catch {{ Write-Error $_; $global:LASTEXITCODE = 1 }}; exit $(if ($null -eq $LASTEXITCODE) {{ 0 }} else {{ $LASTEXITCODE }})"
        );
    }
    if shell.kind == "git-bash" {
        return format!("chcp.com 65001 > /dev/null 2>&1; export LANG=en_US.UTF-8; export LC_ALL=en_US.UTF-8; export PYTHONUTF8=1; export PYTHONIOENCODING=utf-8; {command}");
    }
    command.to_string()
}

fn terminal_background_shell_open_log(log_path: &std::path::Path) -> Result<std::fs::File, String> {
    // 注意：不能用 append 模式——FILE_APPEND_DATA-only 的句柄 MSYS2 子进程不识别，
    // 会导致 bash 静默退出（exit 1、无任何输出）。每个任务独占一个新日志文件，
    // create+write 即可。
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(log_path)
        .map_err(|err| format!("background shell log create failed: {err}"))
}

#[cfg(not(target_os = "windows"))]
fn terminal_background_shell_spawn_non_windows(
    shell: &TerminalShellProfile,
    command: &str,
    cwd: &std::path::Path,
    log_path: &std::path::Path,
) -> Result<tokio::process::Child, String> {
    let log_file = terminal_background_shell_open_log(log_path)?;
    let log_stderr = log_file
        .try_clone()
        .map_err(|err| format!("background shell log clone failed: {err}"))?;
    let mut command_builder = tokio::process::Command::new(&shell.path);
    command_builder.kill_on_drop(true);
    command_builder.current_dir(cwd);
    command_builder.stdout(std::process::Stdio::from(log_file));
    command_builder.stderr(std::process::Stdio::from(log_stderr));
    command_builder.stdin(std::process::Stdio::null());
    for arg in &shell.args_prefix {
        command_builder.arg(arg);
    }
    command_builder.arg(command);
    command_builder
        .spawn()
        .map_err(|err| format!("background shell spawn failed: {err}"))
}

#[cfg(target_os = "windows")]
fn terminal_background_shell_spawn(
    shell: &TerminalShellProfile,
    command: &str,
    cwd: &std::path::Path,
    log_path: &std::path::Path,
) -> Result<(std::process::Child, WindowsJobGuard), String> {
    terminal_background_shell_spawn_windows(shell, command, cwd, log_path)
}

#[cfg(not(target_os = "windows"))]
fn terminal_background_shell_spawn(
    shell: &TerminalShellProfile,
    command: &str,
    cwd: &std::path::Path,
    log_path: &std::path::Path,
) -> Result<tokio::process::Child, String> {
    terminal_background_shell_spawn_non_windows(shell, command, cwd, log_path)
}

/// 压缩摘要注入用：列出本会话仍在运行的后台 shell，保证压缩后 AI 仍知道自己启动过什么。
fn terminal_background_shell_running_summary_lines(
    state: &AppState,
    conversation_id: &str,
) -> Vec<String> {
    let Ok(tasks) = state.terminal_background_shell_tasks.try_lock() else {
        // 登记表被 monitor 短暂持有时跳过注入；压缩比后台任务数量级低频，可接受。
        return Vec::new();
    };
    tasks
        .values()
        .filter(|task| task.conversation_id.trim() == conversation_id.trim())
        .filter(|task| {
            matches!(
                *task.status.lock().expect("terminal background status poisoned"),
                TerminalBackgroundShellStatus::Running
            )
        })
        .map(|task| {
            format!(
                "- {}：description={}，command={}（未完成，完成后系统会推送通知）",
                task.id,
                terminal_background_shell_display_description(task),
                task.command
            )
        })
        .collect()
}

fn terminal_background_shell_status_label(status: TerminalBackgroundShellStatus) -> &'static str {
    match status {
        TerminalBackgroundShellStatus::Running => "running",
        TerminalBackgroundShellStatus::Completed => "completed",
        TerminalBackgroundShellStatus::Failed => "failed",
        TerminalBackgroundShellStatus::Killed => "killed",
        TerminalBackgroundShellStatus::TimedOut => "timed_out",
    }
}

fn terminal_background_shell_is_terminal(status: TerminalBackgroundShellStatus) -> bool {
    !matches!(status, TerminalBackgroundShellStatus::Running)
}

fn terminal_background_shell_writeback(
    state: &AppState,
    task: &TerminalBackgroundShellTask,
    status: TerminalBackgroundShellStatus,
    exit_code: Option<i32>,
) {
    // 完成即通知：附带输出尾部，省去 LLM 一次 status 往返；完整输出在日志文件里。
    let status_label = terminal_background_shell_status_label(status);
    let title = match status {
        TerminalBackgroundShellStatus::Completed => "后台 shell 已完成",
        TerminalBackgroundShellStatus::Failed => "后台 shell 执行失败",
        TerminalBackgroundShellStatus::Killed => {
            let initiator = task
                .kill_initiator
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
                .unwrap_or_default();
            match initiator.as_str() {
                "assistant" => "后台 shell 已被助手终止",
                "user" => "后台 shell 已被用户打断",
                _ => "后台 shell 已终止",
            }
        }
        TerminalBackgroundShellStatus::TimedOut => "后台 shell 已超时",
        TerminalBackgroundShellStatus::Running => "后台 shell 状态更新",
    };
    let exit_code_text = exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "-".to_string());
    let output_tail = terminal_background_shell_log_tail(&task.log_path, 600);
    let detail = format!(
        "id={}\ndescription={}\nstatus={status_label}\nexitCode={exit_code_text}\ncommand={}\ncwd={}\nlog={}\noutputTail=\n{output_tail}\n完整输出请直接读取日志文件",
        task.id,
        terminal_background_shell_display_description(task),
        task.command,
        task.cwd,
        terminal_path_for_user(&task.log_path),
    );
    let body = format_system_user_fold(title, &detail);
    if let Err(err) = enqueue_system_user_forward(
        state,
        &task.conversation_id,
        &body,
        "terminal_background_shell",
    ) {
        runtime_log_error(format!(
            "[终端后台] 写回失败，conversation_id={}，task_id={}，status={}，error={}",
            task.conversation_id,
            task.id,
            status_label,
            err
        ));
    }
    // 监控事件由运行时写路径统一发布；LLM 会话通知（带输出尾部）是独立业务语义，保持不变
    monitor_publish_changed(state, MonitorDomain::BackgroundShell, status_label, &task.conversation_id, &task.id);
}

async fn terminal_background_shell_finish(
    state: &AppState,
    task: &TerminalBackgroundShellTaskHandle,
    status: TerminalBackgroundShellStatus,
    exit_code: Option<i32>,
) {
    {
        let mut status_guard = task.status.lock().expect("terminal background status poisoned");
        // 只允许 Running -> 终态，防止 kill 信号与自然退出双写。
        if terminal_background_shell_is_terminal(*status_guard) {
            return;
        }
        if !terminal_background_shell_is_terminal(status) {
            return;
        }
        *status_guard = status;
    }
    *task.exit_code.lock().expect("terminal background exit code poisoned") = exit_code;
    terminal_background_shell_writeback(state, task, status, exit_code);
    // 终态任务保留在登记表供对账；数量由注册时的裁剪逻辑控制。
}

#[cfg(target_os = "windows")]
async fn terminal_background_shell_monitor(
    state: AppState,
    task: TerminalBackgroundShellTaskHandle,
    mut exit_rx: tokio::sync::oneshot::Receiver<i32>,
    mut kill_rx: tokio::sync::watch::Receiver<bool>,
    _job: WindowsJobGuard,
) {
    // kill 请求可能早于 monitor 启动：先消费 watch 当前值，再进入等待。
    let _ = kill_rx.borrow_and_update();
    enum WindowsMonitorOutcome {
        Exit(i32),
        KillRequested,
    }
    let waited = {
        let wait_exit_or_kill = async {
            tokio::select! {
                code = &mut exit_rx => match code {
                    Ok(code) => WindowsMonitorOutcome::Exit(code),
                    Err(_) => WindowsMonitorOutcome::Exit(-1),
                },
                _ = kill_rx.changed() => WindowsMonitorOutcome::KillRequested,
            }
        };
        match task.timeout_ms {
            Some(timeout_ms) => {
                tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), wait_exit_or_kill).await
            }
            None => Ok(wait_exit_or_kill.await),
        }
    };
    let (final_status, exit_code) = match waited {
        Err(_elapsed) => {
            let _ = _job.terminate_job();
            let exit_code = exit_rx.await.unwrap_or(-1);
            (TerminalBackgroundShellStatus::TimedOut, Some(exit_code))
        }
        Ok(WindowsMonitorOutcome::KillRequested) => {
            let _ = _job.terminate_job();
            let exit_code = exit_rx.await.unwrap_or(-1);
            (TerminalBackgroundShellStatus::Killed, Some(exit_code))
        }
        Ok(WindowsMonitorOutcome::Exit(code)) => {
            if task.kill_requested.load(std::sync::atomic::Ordering::Relaxed) {
                (TerminalBackgroundShellStatus::Killed, Some(code))
            } else if code == 0 {
                (TerminalBackgroundShellStatus::Completed, Some(0))
            } else {
                (TerminalBackgroundShellStatus::Failed, Some(code))
            }
        }
    };
    terminal_background_shell_finish(&state, &task, final_status, exit_code).await;
    // _job 在此 drop；若 shell 又拉起了子进程，kill-on-close 兜底清树。
}

#[cfg(target_os = "windows")]
fn terminal_background_shell_wait_and_report_exit(
    mut child: std::process::Child,
    exit_tx: tokio::sync::oneshot::Sender<i32>,
) {
    std::thread::spawn(move || {
        let waited = child.wait();
        let exit_code = match &waited {
            Ok(status) => status.code().unwrap_or(-1),
            Err(err) => {
                runtime_log_error(format!("[终端后台] 等待进程退出失败，error={err}"));
                -1
            }
        };
        runtime_log_info(format!("[终端后台] 进程退出，exit_code={exit_code}"));
        let _ = exit_tx.send(exit_code);
    });
}

#[cfg(not(target_os = "windows"))]
async fn terminal_background_shell_monitor(
    state: AppState,
    task: TerminalBackgroundShellTaskHandle,
    mut child: tokio::process::Child,
    mut kill_rx: tokio::sync::watch::Receiver<bool>,
) {
    // kill 请求可能早于 monitor 启动：先消费 watch 当前值，再进入等待。
    let _ = kill_rx.borrow_and_update();
    enum UnixMonitorOutcome {
        Exit(Option<i32>),
        KillRequested,
    }
    let waited = {
        let wait_exit_or_kill = async {
            tokio::select! {
                status = child.wait() => UnixMonitorOutcome::Exit(status.ok().and_then(|item| item.code())),
                _ = kill_rx.changed() => UnixMonitorOutcome::KillRequested,
            }
        };
        match task.timeout_ms {
            Some(timeout_ms) => {
                tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), wait_exit_or_kill).await
            }
            None => Ok(wait_exit_or_kill.await),
        }
    };
    let (final_status, exit_code) = match waited {
        Err(_elapsed) => {
            let _ = child.start_kill();
            let exit_code = child.wait().await.ok().and_then(|item| item.code());
            (TerminalBackgroundShellStatus::TimedOut, exit_code)
        }
        Ok(UnixMonitorOutcome::KillRequested) => {
            let _ = child.start_kill();
            let exit_code = child.wait().await.ok().and_then(|item| item.code());
            (TerminalBackgroundShellStatus::Killed, exit_code)
        }
        Ok(UnixMonitorOutcome::Exit(exit_code)) => {
            if task.kill_requested.load(std::sync::atomic::Ordering::Relaxed) {
                (TerminalBackgroundShellStatus::Killed, exit_code)
            } else if exit_code == Some(0) {
                (TerminalBackgroundShellStatus::Completed, Some(0))
            } else {
                (TerminalBackgroundShellStatus::Failed, exit_code)
            }
        }
    };
    terminal_background_shell_finish(&state, &task, final_status, exit_code).await;
    // child 在此 drop；kill_on_drop 兜底清理仍在运行的进程。
}

async fn terminal_background_shell_spawn_and_register(
    state: &AppState,
    session_id: &str,
    conversation_id: &str,
    description: String,
    command: String,
    cwd: std::path::PathBuf,
    timeout_ms: Option<u64>,
    shell: TerminalShellProfile,
) -> Result<String, String> {
    let id = format!("bg-shell-{}", Uuid::new_v4());
    let log_path = terminal_background_shell_log_path(&id);
    let now = now_iso();
    let (kill_signal_tx, kill_signal_rx) = tokio::sync::watch::channel(false);
    #[cfg(target_os = "windows")]
    let (child, job) = terminal_background_shell_spawn(&shell, &command, &cwd, &log_path)?;
    #[cfg(not(target_os = "windows"))]
    let child = terminal_background_shell_spawn(&shell, &command, &cwd, &log_path)?;
    let task = std::sync::Arc::new(TerminalBackgroundShellTask {
        id: id.clone(),
        _session_id: session_id.to_string(),
        conversation_id: conversation_id.to_string(),
        command: command.clone(),
        cwd: cwd.to_string_lossy().to_string(),
        description,
        started_at: now.to_string(),
        timeout_ms,
        log_path: log_path.clone(),
        status: std::sync::Mutex::new(TerminalBackgroundShellStatus::Running),
        exit_code: std::sync::Mutex::new(None),
        kill_requested: std::sync::atomic::AtomicBool::new(false),
        kill_initiator: std::sync::Mutex::new(None),
        kill_signal_tx,
    });
    terminal_background_shell_register(state, task.clone()).await;
    #[cfg(target_os = "windows")]
    {
        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
        terminal_background_shell_wait_and_report_exit(child, exit_tx);
        tauri::async_runtime::spawn(terminal_background_shell_monitor(
            state.clone(),
            task,
            exit_rx,
            kill_signal_rx,
            job,
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        tauri::async_runtime::spawn(terminal_background_shell_monitor(
            state.clone(),
            task,
            child,
            kill_signal_rx,
        ));
    }
    runtime_log_info(format!(
        "[终端后台] 已启动，task_id={}，conversation_id={}，log={}",
        id,
        conversation_id,
        terminal_path_for_user(&log_path)
    ));
    Ok(id)
}

async fn terminal_live_list_sessions(state: &AppState) -> Vec<Value> {
    let handles = {
        let sessions = state.terminal_live_sessions.lock().await;
        sessions.values().cloned().collect::<Vec<_>>()
    };
    let mut out = Vec::<Value>::new();
    for handle in handles {
        let last_used_at = handle.last_used_at.lock().await.clone();
        out.push(serde_json::json!({
            "shellKind": handle.shell_kind,
            "shellPath": handle.shell_path,
            "createdAt": handle.created_at,
            "lastUsedAt": last_used_at
        }));
    }
    out
}

fn detect_terminal_shell_candidates() -> Vec<TerminalShellProfile> {
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
                .find(|candidate| Path::new(candidate).is_file())
                .cloned()
        }

        fn path_lookup_first(name: &str) -> Option<String> {
            let path_value = std::env::var_os("PATH")?;
            let name_path = Path::new(name);
            let has_ext = name_path.extension().is_some();
            let mut candidates = Vec::<String>::new();
            if has_ext {
                candidates.push(name.to_string());
            } else {
                candidates.push(name.to_string());
                if let Some(pathext) = std::env::var_os("PATHEXT") {
                    for ext in pathext.to_string_lossy().split(';') {
                        let trimmed = ext.trim();
                        if !trimmed.is_empty() {
                            candidates.push(format!("{name}{trimmed}"));
                        }
                    }
                } else {
                    candidates.push(format!("{name}.exe"));
                }
            }

            for dir in std::env::split_paths(&path_value) {
                for candidate in &candidates {
                    let full = dir.join(candidate);
                    if full.is_file() {
                        return Some(full.to_string_lossy().to_string());
                    }
                }
            }
            None
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

        let mut out = Vec::<TerminalShellProfile>::new();
        let mut git_bash_candidates = vec![
            r"C:\Program Files\Git\bin\bash.exe".to_string(),
            r"C:\Program Files\Git\usr\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\usr\bin\bash.exe".to_string(),
        ];
        if let Some(git_path) = path_lookup_first("git") {
            git_bash_candidates.extend(derive_bash_candidates_from_git(&git_path));
        }
        if let Some(path) = path_lookup_first("bash") {
            git_bash_candidates.push(path);
        }

        if let Some(path) = first_existing_path(&git_bash_candidates) {
            out.push(with_args("git-bash", path, &["-lc"]));
        }

        let mut pwsh7_candidates = vec![
            r"C:\Program Files\PowerShell\7\pwsh.exe".to_string(),
            r"C:\Program Files\PowerShell\7-preview\pwsh.exe".to_string(),
        ];
        if let Some(path) = path_lookup_first("pwsh.exe") {
            pwsh7_candidates.push(path);
        }
        if let Some(path) = first_existing_path(&pwsh7_candidates) {
            out.push(with_args("powershell7", path, &["-NoProfile", "-Command"]));
        }

        let mut powershell5_candidates = Vec::<String>::new();
        if let Ok(windir) = std::env::var("WINDIR") {
            powershell5_candidates.push(
                PathBuf::from(windir)
                    .join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        if let Some(path) = path_lookup_first("powershell.exe") {
            powershell5_candidates.push(path);
        }
        if let Some(path) = first_existing_path(&powershell5_candidates) {
            out.push(with_args("powershell5", path, &["-NoProfile", "-Command"]));
        }
        return out;
    }

    #[cfg(target_os = "macos")]
    {
        let mut out = Vec::<TerminalShellProfile>::new();
        let zsh = Path::new("/bin/zsh");
        if zsh.is_file() {
            out.push(TerminalShellProfile {
                kind: "zsh".to_string(),
                path: zsh.to_string_lossy().to_string(),
                args_prefix: vec!["-lc".to_string()],
            });
        }
        let bash = Path::new("/bin/bash");
        if bash.is_file() {
            out.push(TerminalShellProfile {
                kind: "bash".to_string(),
                path: bash.to_string_lossy().to_string(),
                args_prefix: vec!["-lc".to_string()],
            });
        }
        if Path::new("/bin/sh").is_file() {
            out.push(TerminalShellProfile {
                kind: "sh".to_string(),
                path: "/bin/sh".to_string(),
                args_prefix: vec!["-lc".to_string()],
            });
        }
        return out;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut out = Vec::<TerminalShellProfile>::new();
        for candidate in ["/bin/bash", "/usr/bin/bash", "/bin/zsh", "/usr/bin/zsh", "/bin/sh"] {
            if Path::new(candidate).is_file() {
                let kind = Path::new(candidate)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("sh")
                    .to_string();
                out.push(TerminalShellProfile {
                    kind,
                    path: candidate.to_string(),
                    args_prefix: vec!["-lc".to_string()],
                });
            }
        }
        return out;
    }

    #[allow(unreachable_code)]
    Vec::new()
}

fn terminal_shell_missing_profile() -> TerminalShellProfile {
    TerminalShellProfile {
        kind: "missing-terminal-shell".to_string(),
        path: String::new(),
        args_prefix: Vec::new(),
    }
}

fn detect_default_terminal_shell() -> TerminalShellProfile {
    detect_terminal_shell_candidates()
        .into_iter()
        .next()
        .unwrap_or_else(terminal_shell_missing_profile)
}

fn terminal_shell_from_candidates(
    candidates: &[TerminalShellProfile],
    preferred_kind: &str,
) -> TerminalShellProfile {
    let preferred = preferred_kind.trim().to_ascii_lowercase();
    if preferred != "auto" && !preferred.is_empty() {
        if let Some(hit) = candidates.iter().find(|item| item.kind == preferred) {
            return hit.clone();
        }
    }
    candidates
        .first()
        .cloned()
        .unwrap_or_else(terminal_shell_missing_profile)
}

fn terminal_shell_for_state(state: &AppState) -> TerminalShellProfile {
    let preferred = state_read_config_cached(state)
        .map(|cfg| cfg.terminal_shell_kind)
        .unwrap_or_else(|_| "auto".to_string());
    terminal_shell_from_candidates(&state.terminal_shell_candidates, &preferred)
}

fn terminal_shell_candidates_for_ui(
    state: &AppState,
) -> (String, TerminalShellProfile, Vec<Value>) {
    let preferred = state_read_config_cached(state)
        .map(|cfg| cfg.terminal_shell_kind)
        .unwrap_or_else(|_| "auto".to_string());
    let candidates = state.terminal_shell_candidates.clone();
    let current = terminal_shell_from_candidates(&candidates, &preferred);
    let mut items = Vec::<Value>::new();
    items.push(serde_json::json!({
        "kind": "auto",
        "label": "Auto",
        "available": true,
        "path": ""
    }));
    for item in &candidates {
        items.push(serde_json::json!({
            "kind": item.kind,
            "label": terminal_shell_runtime_label(item),
            "available": true,
            "path": item.path
        }));
    }
    (preferred, current, items)
}

fn terminal_shell_runtime_label(shell: &TerminalShellProfile) -> String {
    let title = match shell.kind.as_str() {
        "powershell7" => "PowerShell 7",
        "powershell5" => "Windows PowerShell 5.1",
        "git-bash" => "Git Bash",
        "missing-terminal-shell" => "Unavailable",
        other => other,
    };
    if shell.path.trim().is_empty() {
        return title.to_string();
    }
    format!("{title} ({})", shell.path.trim())
}

fn terminal_exec_tool_description(shell: &TerminalShellProfile) -> String {
    format!(
        "在当前 shell 工作区根目录中执行一次性命令。运行时 shell：{}。命令结束、失败或超时后，本次进程树会被回收。\n\n\
         后台执行（mode=background）：如无必要不要使用；仅适合长耗时的服务、监听或构建任务（如启动 dev server、持续 watch）。\
         启动时建议传 description 说明用途，返回 backgroundId 与 logPath；输出全部写入日志文件，\
         完成时系统会向会话推送通知（含退出码与输出尾部），不要忙轮询，等通知即可。\
         后台任务不受 timeout_ms 约束，会一直跑到自然结束或被 kill，需要限时请在命令内部自行处理。\
         用 background 工具管理：action=list 查看本会话任务，action=status id=… 查看状态与日志尾部，action=kill id=… 终止。\
         启动过的后台任务即使经历上下文压缩也不会丢：完成有通知，随时可用 background list 对账。",
        terminal_shell_runtime_label(shell)
    )
}
