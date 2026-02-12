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
