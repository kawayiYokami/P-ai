#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
async fn sandbox_run_with_process_backend(
    shell: &TerminalShellProfile,
    request: &SandboxRequest,
) -> Result<SandboxExecutionResult, String> {
    let mut command_builder = tokio::process::Command::new(&shell.path);
    command_builder.kill_on_drop(true);
    command_builder.current_dir(&request.cwd);
    command_builder.stdout(std::process::Stdio::piped());
    command_builder.stderr(std::process::Stdio::piped());
    command_builder.stdin(std::process::Stdio::null());
    for arg in &shell.args_prefix {
        command_builder.arg(arg);
    }
    command_builder.arg(&request.command);

    let timeout_ms = request.timeout_ms.max(1);
    let started = std::time::Instant::now();
    let output = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        command_builder.output(),
    )
    .await
    .map_err(|_| format!("terminal_exec timed out after {}ms", timeout_ms))?
    .map_err(|err| format!("terminal_exec spawn failed: {err}"))?;

    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(SandboxExecutionResult {
        ok: output.status.success(),
        exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
        duration_ms,
        shell_kind: shell.kind.clone(),
        shell_path: shell.path.clone(),
    })
}
