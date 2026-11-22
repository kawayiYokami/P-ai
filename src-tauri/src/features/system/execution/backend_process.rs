#[cfg(not(target_os = "windows"))]
async fn exec_run_with_process_backend(
    shell: &TerminalShellProfile,
    request: &ExecutionRequest,
) -> Result<ExecutionResult, String> {
    use tokio::io::AsyncReadExt as _;

    let mut command_builder = tokio::process::Command::new(&shell.path);
    command_builder.kill_on_drop(true);
    // Unix: 让 shell 自成进程组，超时/取消时可以对整组发信号，
    // 避免后台派生进程脱离清理（与 Windows Job Object 整树清理对齐）。
    #[cfg(unix)]
    {
        command_builder.process_group(0);
    }
    command_builder.current_dir(&request.cwd);
    command_builder.stdout(std::process::Stdio::piped());
    command_builder.stderr(std::process::Stdio::piped());
    command_builder.stdin(std::process::Stdio::null());
    for arg in &shell.args_prefix {
        command_builder.arg(arg);
    }
    command_builder.arg(&request.command);

    let mut child = command_builder
        .spawn()
        .map_err(|err| format!("terminal_exec spawn failed: {err}"))?;
    let child_pid = child.id();
    // RAII：Future 被 drop（含取消/abort 及绕过超时的路径）时向进程组发信号，
    // 正常退出后 disarm，避免对已完成的进程组补刀。
    #[cfg(unix)]
    let mut process_group_guard = ProcessGroupGuard::new(child_pid);
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "Capture child stdout failed.".to_string())?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "Capture child stderr failed.".to_string())?;

    // 读取管道失败时返回错误而不是静默丢弃，避免调用方拿到不完整输出
    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::<u8>::new();
        stdout_pipe.read_to_end(&mut buf).await.map(|_| buf)
    });
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::<u8>::new();
        stderr_pipe.read_to_end(&mut buf).await.map(|_| buf)
    });

    let timeout_ms = request.timeout_ms.max(1);
    let started = std::time::Instant::now();
    let status = tokio::select! {
        status = child.wait() => {
            status.map_err(|err| format!("terminal_exec wait failed: {err}"))?
        }
        _ = tokio::time::sleep(std::time::Duration::from_millis(timeout_ms)) => {
            kill_process_group(child_pid);
            // kill_on_drop 会在 drop 时 SIGKILL 直接子进程，这里显式回收状态
            let _ = child.kill().await;
            let _ = child.wait().await;
            #[cfg(unix)]
            process_group_guard.disarm();
            // 进程组已终止，输出管道理论上必然 EOF；但若仍有派生进程持有管道写端，
            // reader 可能永不结束——必须有界回收，否则超时路径本身会挂起。
            // 读取失败/join 失败/回收超时都显式收集进错误信息，不静默丢弃。
            let mut read_errors = Vec::<String>::new();
            if let Err(err) = bounded_join_reader(stdout_task, "stdout").await {
                read_errors.push(err);
            }
            if let Err(err) = bounded_join_reader(stderr_task, "stderr").await {
                read_errors.push(err);
            }
            let mut message = format!("terminal_exec timed out after {}ms", timeout_ms);
            if !read_errors.is_empty() {
                message.push_str(&format!("（{}）", read_errors.join("; ")));
            }
            return Err(message);
        }
    };

    // 耗时包含两个 reader 的收尾等待：命令"完成"以输出收齐为准
    let stdout = match join_reader_task(stdout_task, "stdout").await {
        Ok(buf) => buf,
        Err(err) => return Err(err),
    };
    let stderr = match join_reader_task(stderr_task, "stderr").await {
        Ok(buf) => buf,
        Err(err) => return Err(err),
    };
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    #[cfg(unix)]
    process_group_guard.disarm();
    let exit_code = status.code().unwrap_or(-1);
    Ok(ExecutionResult {
        ok: status.success(),
        exit_code,
        stdout,
        stderr,
        duration_ms,
        shell_kind: shell.kind.clone(),
        shell_path: shell.path.clone(),
    })
}

/// 等待 reader task 结束并取回输出；读取失败或任务 join 失败都返回终端执行错误。
#[cfg(not(target_os = "windows"))]
async fn join_reader_task(
    task: tokio::task::JoinHandle<Result<Vec<u8>, std::io::Error>>,
    name: &str,
) -> Result<Vec<u8>, String> {
    match task.await {
        Ok(Ok(buf)) => Ok(buf),
        Ok(Err(err)) => Err(format!("terminal_exec read {name} failed: {err}")),
        Err(err) => Err(format!("Join {name} reader task failed: {err}")),
    }
}

/// 有界等待 reader task：超时路径防止后代进程持有管道写端导致永久挂起。
/// 与 Windows 后端的 2 秒清理超时保持一致。
#[cfg(not(target_os = "windows"))]
async fn bounded_join_reader(
    task: tokio::task::JoinHandle<Result<Vec<u8>, std::io::Error>>,
    name: &str,
) -> Result<Vec<u8>, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(READER_CLEANUP_TIMEOUT_MS),
        task,
    )
    .await;
    match result {
        Ok(Ok(Ok(buf))) => Ok(buf),
        Ok(Ok(Err(err))) => Err(format!("terminal_exec read {name} failed: {err}")),
        Ok(Err(err)) => Err(format!("Join {name} reader task failed: {err}")),
        Err(_) => Err(format!("Join {name} reader task timed out")),
    }
}

#[cfg(unix)]
struct ProcessGroupGuard {
    pid: Option<u32>,
    armed: bool,
}

#[cfg(unix)]
impl ProcessGroupGuard {
    fn new(pid: Option<u32>) -> Self {
        Self {
            pid,
            armed: pid.is_some(),
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(unix)]
impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Some(pid) = self.pid {
            let result = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
            if result == 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            // 进程组已退出（ESRCH）视为正常，不告警
            if err.raw_os_error() != Some(libc::ESRCH) {
                runtime_log_warn(format!(
                    "[系统] 进程组清理失败 进程={}: {err}",
                    pid
                ));
            }
        }
    }
}

#[cfg(unix)]
fn kill_process_group(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    // 负 PID：向该进程组所有成员广播信号；SIGKILL 不可被忽略或捕获
    let result = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        runtime_log_warn(format!(
            "[系统] 进程组清理失败 进程={}: {err}",
            pid
        ));
    }
}

#[cfg(all(test, unix))]
mod exec_process_backend_tests {
    use super::*;

    fn test_profile() -> TerminalShellProfile {
        TerminalShellProfile {
            kind: "sh".to_string(),
            path: "/bin/sh".to_string(),
            args_prefix: vec!["-c".to_string()],
        }
    }

    #[test]
    fn normal_exit_with_background_holder_should_wait_for_pipe_eof() {
        // shell 立即退出，但后台进程延迟写入输出管道：完成条件必须等 reader 收完，
        // 不能只等 child.wait() 就返回不完整输出；duration_ms 应包含这段等待。
        let marker = "BG_MARKER_9f3a2c".to_string();
        let request = ExecutionRequest {
            session_id: "test-session".to_string(),
            command: format!("echo done; (sleep 1; echo {marker}) &"),
            cwd: std::env::temp_dir(),
            timeout_ms: 5_000,
            cwd_policy_exempt: true,
        };
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let execution = rt
            .block_on(exec_run_with_process_backend(&test_profile(), &request))
            .expect("execution should succeed");
        let stdout = String::from_utf8_lossy(&execution.stdout);
        assert!(
            stdout.contains("done"),
            "stdout should contain immediate output: {stdout:?}"
        );
        assert!(
            stdout.contains(&marker),
            "stdout should contain delayed marker: {stdout:?}"
        );
        assert!(
            execution.duration_ms >= 900,
            "duration should include reader wait: {}ms",
            execution.duration_ms
        );
    }

    #[test]
    fn timeout_should_kill_process_group_and_return_timed_out() {
        // 后台子进程延迟向临时目录写标记文件：超时后进程组整体清理，
        // 标记文件不应被写出；返回超时错误。
        struct MarkerCleanup(std::path::PathBuf);
        impl Drop for MarkerCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        let marker_path = std::env::temp_dir().join(format!(
            "pai-backend-timeout-marker-{}.txt",
            std::process::id()
        ));
        let _cleanup = MarkerCleanup(marker_path.clone());
        let request = ExecutionRequest {
            session_id: "test-session".to_string(),
            command: format!(
                "(sleep 2; echo killed > {}) & wait",
                marker_path.to_string_lossy()
            ),
            cwd: std::env::temp_dir(),
            timeout_ms: 500,
            cwd_policy_exempt: true,
        };
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result =
            rt.block_on(exec_run_with_process_backend(&test_profile(), &request));
        let err = result.expect_err("should time out");
        assert!(
            err.contains("timed out"),
            "error should mention timeout: {err}"
        );
        // 若进程组未被清理，后台子进程会在约 2 秒后写出标记文件；等待超过该时长再断言
        std::thread::sleep(std::time::Duration::from_millis(3_000));
        assert!(
            !marker_path.exists(),
            "background process should have been killed before writing marker"
        );
    }
}