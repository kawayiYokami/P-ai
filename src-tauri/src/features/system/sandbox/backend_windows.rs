#[cfg(target_os = "windows")]
fn sandbox_windows_process_compatible_path(path: &std::path::Path) -> std::path::PathBuf {
    let raw = path.as_os_str().to_string_lossy();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return std::path::PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        return std::path::PathBuf::from(rest);
    }
    path.to_path_buf()
}

#[cfg(target_os = "windows")]
fn sandbox_run_with_windows_job_backend_blocking(
    shell: &TerminalShellProfile,
    request: &SandboxRequest,
) -> Result<SandboxExecutionResult, String> {
    use std::io::Read as _;
    use std::os::windows::io::AsRawHandle as _;
    use std::os::windows::process::CommandExt as _;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    struct JobGuard(HANDLE);
    impl Drop for JobGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    let mut command_builder = std::process::Command::new(&shell.path);
    let cwd = sandbox_windows_process_compatible_path(&request.cwd);
    command_builder.current_dir(&cwd);
    command_builder.stdout(std::process::Stdio::piped());
    command_builder.stderr(std::process::Stdio::piped());
    command_builder.stdin(std::process::Stdio::null());
    command_builder.creation_flags(CREATE_NO_WINDOW);
    for arg in &shell.args_prefix {
        command_builder.arg(arg);
    }
    command_builder.arg(&request.command);

    let mut child = command_builder
        .spawn()
        .map_err(|err| format!("terminal_exec spawn failed: {err}"))?;

    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err("CreateJobObjectW failed.".to_string());
    }
    let job = JobGuard(job);

    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    // Keep process tree cleanup on timeout/exit, but do not cap child process count:
    // Git Bash bootstrap may spawn helper processes during startup.
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let set_ok = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if set_ok == 0 {
        return Err("SetInformationJobObject failed.".to_string());
    }

    let assign_ok = unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) };
    if assign_ok == 0 {
        return Err("AssignProcessToJobObject failed.".to_string());
    }

    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "Capture child stdout failed.".to_string())?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "Capture child stderr failed.".to_string())?;

    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let timeout_ms = request.timeout_ms.max(1);
    let started = std::time::Instant::now();
    loop {
        if let Some(_status) = child
            .try_wait()
            .map_err(|err| format!("terminal_exec try_wait failed: {err}"))?
        {
            break;
        }
        if started.elapsed().as_millis() >= timeout_ms as u128 {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "terminal_exec timed out after {}ms",
                timeout_ms
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    let status = child
        .wait()
        .map_err(|err| format!("terminal_exec wait failed: {err}"))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "Join stdout reader thread failed.".to_string())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Join stderr reader thread failed.".to_string())?;
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    Ok(SandboxExecutionResult {
        ok: status.success(),
        exit_code: status.code().unwrap_or(-1),
        stdout,
        stderr,
        duration_ms,
        shell_kind: shell.kind.clone(),
        shell_path: shell.path.clone(),
    })
}

#[cfg(target_os = "windows")]
async fn sandbox_run_with_windows_job_backend(
    shell: &TerminalShellProfile,
    request: &SandboxRequest,
) -> Result<SandboxExecutionResult, String> {
    let shell = shell.clone();
    let request = request.clone();
    tokio::task::spawn_blocking(move || {
        sandbox_run_with_windows_job_backend_blocking(&shell, &request)
    })
    .await
    .map_err(|err| format!("Join windows sandbox worker failed: {err}"))?
}
