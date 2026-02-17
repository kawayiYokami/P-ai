#[cfg(target_os = "macos")]
fn sandbox_find_sandbox_exec_binary() -> Option<std::path::PathBuf> {
    let builtin = std::path::PathBuf::from("/usr/bin/sandbox-exec");
    if builtin.is_file() {
        return Some(builtin);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("sandbox-exec"))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(target_os = "macos")]
fn sandbox_escape_profile_path(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push('_'),
            c => out.push(c),
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn sandbox_build_macos_profile(root: &std::path::Path) -> String {
    let root = sandbox_escape_profile_path(root);
    format!(
        r#"(version 1)
(deny default)
(allow process*)
(allow signal (target self))
(allow sysctl-read)
(allow network*)
(allow file-read-metadata)
(allow file-read-data
    (subpath "/System")
    (subpath "/usr")
    (subpath "/bin")
    (subpath "/sbin")
    (subpath "/dev")
    (subpath "/private/etc")
    (subpath "/private/preboot")
    (subpath "/private/var/db/dyld")
)
(allow file-read* file-write* file-ioctl (subpath "{root}"))
"#
    )
}

#[cfg(target_os = "macos")]
async fn sandbox_run_with_macos_seatbelt_backend(
    shell: &TerminalShellProfile,
    request: &SandboxRequest,
) -> Result<SandboxExecutionResult, String> {
    // NOTE: Cross-platform backend parity implementation. Not yet validated on real macOS hosts.
    let sandbox_exec = sandbox_find_sandbox_exec_binary().ok_or_else(|| {
        "macOS sandbox backend requires sandbox-exec, but sandbox-exec was not found.".to_string()
    })?;

    let profile = sandbox_build_macos_profile(&request.cwd);

    let mut command_builder = tokio::process::Command::new(&sandbox_exec);
    command_builder.kill_on_drop(true);
    command_builder.current_dir(&request.cwd);
    command_builder.stdout(std::process::Stdio::piped());
    command_builder.stderr(std::process::Stdio::piped());
    command_builder.stdin(std::process::Stdio::null());

    command_builder.arg("-p");
    command_builder.arg(profile);
    command_builder.arg(&shell.path);
    for arg in &shell.args_prefix {
        if arg == "-lc" {
            command_builder.arg("-c");
        } else {
            command_builder.arg(arg);
        }
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
        shell_kind: format!("macos-seatbelt+{}", shell.kind),
        shell_path: format!("{} via {}", shell.path, sandbox_exec.to_string_lossy()),
    })
}
