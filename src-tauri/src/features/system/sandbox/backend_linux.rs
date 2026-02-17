#[cfg(target_os = "linux")]
fn sandbox_find_bwrap_binary() -> Option<std::path::PathBuf> {
    for candidate in ["/usr/bin/bwrap", "/bin/bwrap"] {
        let path = std::path::PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("bwrap"))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(target_os = "linux")]
fn sandbox_linux_ro_bind_dirs(shell: &TerminalShellProfile) -> Vec<std::path::PathBuf> {
    let mut out = Vec::<std::path::PathBuf>::new();
    let mut seen = std::collections::HashSet::<String>::new();

    let mut push = |path: std::path::PathBuf| {
        if !path.is_dir() {
            return;
        }
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            out.push(path);
        }
    };

    for candidate in [
        "/usr",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/etc",
        "/opt",
        "/run",
        "/nix",
    ] {
        push(std::path::PathBuf::from(candidate));
    }

    if let Some(parent) = std::path::Path::new(&shell.path).parent() {
        push(parent.to_path_buf());
    }

    out
}

#[cfg(target_os = "linux")]
async fn sandbox_run_with_linux_bwrap_backend(
    shell: &TerminalShellProfile,
    request: &SandboxRequest,
) -> Result<SandboxExecutionResult, String> {
    // NOTE: Cross-platform backend parity implementation. Not yet validated on real Linux hosts.
    let bwrap = sandbox_find_bwrap_binary().ok_or_else(|| {
        "Linux sandbox backend requires bubblewrap (bwrap), but bwrap was not found in PATH."
            .to_string()
    })?;

    let mut command_builder = tokio::process::Command::new(&bwrap);
    command_builder.kill_on_drop(true);
    command_builder.stdout(std::process::Stdio::piped());
    command_builder.stderr(std::process::Stdio::piped());
    command_builder.stdin(std::process::Stdio::null());

    command_builder.arg("--die-with-parent");
    command_builder.arg("--new-session");
    command_builder.arg("--unshare-pid");
    command_builder.arg("--proc").arg("/proc");
    command_builder.arg("--dev").arg("/dev");
    command_builder.arg("--tmpfs").arg("/tmp");

    for ro_dir in sandbox_linux_ro_bind_dirs(shell) {
        command_builder.arg("--ro-bind");
        command_builder.arg(&ro_dir);
        command_builder.arg(&ro_dir);
    }

    command_builder.arg("--bind");
    command_builder.arg(&request.cwd);
    command_builder.arg("/workspace");
    command_builder.arg("--chdir");
    command_builder.arg("/workspace");

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
        shell_kind: format!("linux-bwrap+{}", shell.kind),
        shell_path: format!("{} via {}", shell.path, bwrap.to_string_lossy()),
    })
}
