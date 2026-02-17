#[tauri::command]
async fn desktop_screenshot(input: ScreenshotRequest) -> Result<ScreenshotResponse, String> {
    run_screenshot_tool(input)
        .await
        .map_err(|err| to_tool_err_string(&err))
}

#[tauri::command]
async fn desktop_wait(input: WaitRequest) -> Result<WaitResponse, String> {
    run_wait_tool(input)
        .await
        .map_err(|err| to_tool_err_string(&err))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveTerminalApprovalInput {
    request_id: String,
    approved: bool,
}

#[tauri::command]
fn resolve_terminal_approval(
    input: ResolveTerminalApprovalInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = resolve_terminal_approval_request(&state, &input.request_id, input.approved)?;
    Ok(())
}
