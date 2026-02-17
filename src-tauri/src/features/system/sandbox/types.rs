#[derive(Debug, Clone)]
struct SandboxRequest {
    session_id: String,
    command: String,
    cwd: std::path::PathBuf,
    timeout_ms: u64,
}

#[derive(Debug, Clone)]
struct SandboxExecutionResult {
    ok: bool,
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    duration_ms: u64,
    shell_kind: String,
    shell_path: String,
}
