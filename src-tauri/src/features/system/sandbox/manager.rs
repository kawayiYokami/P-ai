#[derive(Debug, Clone, Copy)]
enum SandboxBackendKind {
    ProcessBackend,
}

#[derive(Debug, Clone, Copy)]
struct SandboxManager {
    backend: SandboxBackendKind,
}

impl SandboxManager {
    fn from_state(_state: &AppState) -> Self {
        Self {
            backend: SandboxBackendKind::ProcessBackend,
        }
    }

    async fn run(
        &self,
        state: &AppState,
        request: SandboxRequest,
    ) -> Result<SandboxExecutionResult, String> {
        // Defense in depth: backend entrance re-checks cwd policy.
        sandbox_assert_cwd_allowed(state, &request.session_id, &request.cwd)?;
        match self.backend {
            SandboxBackendKind::ProcessBackend => {
                sandbox_run_with_process_backend(&state.terminal_shell, &request).await
            }
        }
    }
}

async fn sandbox_execute_command(
    state: &AppState,
    session_id: &str,
    command: &str,
    cwd: &std::path::Path,
    timeout_ms: u64,
) -> Result<SandboxExecutionResult, String> {
    let manager = SandboxManager::from_state(state);
    let request = SandboxRequest {
        session_id: session_id.to_string(),
        command: command.to_string(),
        cwd: cwd.to_path_buf(),
        timeout_ms,
    };
    manager.run(state, request).await
}
