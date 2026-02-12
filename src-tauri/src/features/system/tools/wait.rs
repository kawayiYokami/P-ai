async fn run_wait_tool(input: WaitRequest) -> DesktopToolResult<WaitResponse> {
    if !matches!(input.mode, WaitMode::Sleep) {
        return Err(DesktopToolError::invalid_params(
            "unsupported wait mode, only 'sleep' is available in MVP",
        ));
    }

    // Cap explicit wait to avoid accidental long blocking.
    if input.ms > 120_000 {
        return Err(DesktopToolError::invalid_params(
            "ms must be <= 120000 for wait mode sleep",
        ));
    }

    let started = std::time::Instant::now();
    tokio::time::sleep(std::time::Duration::from_millis(input.ms)).await;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(WaitResponse {
        ok: true,
        waited_ms: input.ms,
        elapsed_ms,
    })
}
