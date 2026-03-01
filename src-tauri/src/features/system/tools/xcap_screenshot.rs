use std::time::Instant;

fn validate_screenshot_request(input: &ScreenshotRequest) -> DesktopToolResult<()> {
    if let Some(region) = &input.region {
        if region.width == 0 || region.height == 0 {
            return Err(DesktopToolError::invalid_params(
                "region.width and region.height must be > 0",
            ));
        }
    }
    if matches!(input.mode, ScreenshotMode::Monitor) && input.monitor_id.is_none() {
        return Err(DesktopToolError::invalid_params(
            "monitor_id is required when mode=monitor",
        ));
    }
    if matches!(input.mode, ScreenshotMode::Region) && input.region.is_none() {
        return Err(DesktopToolError::invalid_params(
            "region is required when mode=region",
        ));
    }
    if !(1.0..=100.0).contains(&input.webp_quality) {
        return Err(DesktopToolError::invalid_params(
            "webp_quality must be between 1 and 100",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_linux_display_backend_supported() -> DesktopToolResult<()> {
    // Product requirement: we currently do not support Wayland runtime.
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Err(DesktopToolError::internal_error(
            "xcap screenshot does not support Wayland runtime on Linux builds",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn ensure_linux_display_backend_supported() -> DesktopToolResult<()> {
    Ok(())
}

fn default_screenshot_path() -> DesktopToolResult<std::path::PathBuf> {
    let mut dir = std::env::temp_dir();
    dir.push("easy-call-ai");
    dir.push("screenshots");
    std::fs::create_dir_all(&dir).map_err(|err| {
        DesktopToolError::internal_error(format!("create screenshot directory failed: {err}"))
    })?;
    let filename = format!(
        "shot_{}_{}.webp",
        OffsetDateTime::now_utc().unix_timestamp(),
        Uuid::new_v4().simple()
    );
    dir.push(filename);
    Ok(dir)
}

fn build_output_path(input: &ScreenshotRequest) -> DesktopToolResult<std::path::PathBuf> {
    if let Some(path) = &input.save_path {
        let output = std::path::PathBuf::from(path);
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    DesktopToolError::internal_error(format!(
                        "create screenshot parent directory failed: {err}"
                    ))
                })?;
            }
        }
        return Ok(output);
    }
    default_screenshot_path()
}

fn maybe_save_from_base64(
    input: &ScreenshotRequest,
    image_base64: &str,
) -> DesktopToolResult<(Option<String>, Option<u64>)> {
    if input.save_path.is_none() {
        return Ok((None, None));
    }
    let started = Instant::now();
    let output_path = build_output_path(input)?;
    let bytes = B64
        .decode(image_base64)
        .map_err(|err| DesktopToolError::internal_error(format!("decode base64 failed: {err}")))?;
    std::fs::write(&output_path, bytes).map_err(|err| {
        DesktopToolError::internal_error(format!("write screenshot file failed: {err}"))
    })?;
    Ok((
        Some(output_path.to_string_lossy().to_string()),
        Some(started.elapsed().as_millis() as u64),
    ))
}

fn rgba_to_rgb(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity((bytes.len() / 4) * 3);
    for px in bytes.chunks_exact(4) {
        out.extend_from_slice(&[px[0], px[1], px[2]]);
    }
    out
}

fn normalize_region_crop(
    region: &ScreenBounds,
    frame_width: u32,
    frame_height: u32,
) -> DesktopToolResult<(u32, u32, u32, u32)> {
    let x0 = region.x.max(0) as i64;
    let y0 = region.y.max(0) as i64;
    let x1 = (region.x as i64 + region.width as i64).max(0);
    let y1 = (region.y as i64 + region.height as i64).max(0);
    let max_x = frame_width as i64;
    let max_y = frame_height as i64;
    let sx = x0.min(max_x);
    let sy = y0.min(max_y);
    let ex = x1.min(max_x);
    let ey = y1.min(max_y);
    if ex <= sx || ey <= sy {
        return Err(DesktopToolError::invalid_params(
            "region is outside captured monitor bounds",
        ));
    }
    Ok((sx as u32, sy as u32, ex as u32, ey as u32))
}

fn monitor_list() -> DesktopToolResult<Vec<xcap::Monitor>> {
    ensure_linux_display_backend_supported()?;
    let monitors = xcap::Monitor::all()
        .map_err(|err| DesktopToolError::internal_error(format!("list monitors failed: {err}")))?;
    if monitors.is_empty() {
        return Err(DesktopToolError::internal_error("no monitor detected"));
    }
    Ok(monitors)
}

fn window_list() -> DesktopToolResult<Vec<xcap::Window>> {
    ensure_linux_display_backend_supported()?;
    xcap::Window::all()
        .map_err(|err| DesktopToolError::internal_error(format!("list windows failed: {err}")))
}

fn resolve_monitor_by_id(monitors: &[xcap::Monitor], monitor_id: u32) -> Option<xcap::Monitor> {
    monitors.get(monitor_id as usize).cloned().or_else(|| {
        if monitor_id > 0 {
            monitors.get((monitor_id - 1) as usize).cloned()
        } else {
            None
        }
    }).or_else(|| {
        monitors
            .iter()
            .find(|m| m.id().ok() == Some(monitor_id))
            .cloned()
    })
}

fn resolve_primary_monitor(monitors: &[xcap::Monitor]) -> xcap::Monitor {
    monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .cloned()
        .unwrap_or_else(|| monitors[0].clone())
}

fn capture_once_xcap(
    input: &ScreenshotRequest,
) -> DesktopToolResult<(Vec<u8>, u32, u32, ScreenBounds, u64)> {
    let monitors = monitor_list()?;
    let monitor = match input.mode {
        ScreenshotMode::Monitor => {
            let id = input
                .monitor_id
                .ok_or_else(|| DesktopToolError::invalid_params("monitor_id is required"))?;
            resolve_monitor_by_id(&monitors, id).ok_or_else(|| {
                DesktopToolError::invalid_params(format!("monitor_id not found: {id}"))
            })?
        }
        ScreenshotMode::Desktop => resolve_primary_monitor(&monitors),
        ScreenshotMode::Region => {
            if let Some(id) = input.monitor_id {
                resolve_monitor_by_id(&monitors, id).ok_or_else(|| {
                    DesktopToolError::invalid_params(format!("monitor_id not found: {id}"))
                })?
            } else {
                resolve_primary_monitor(&monitors)
            }
        }
    };

    let mx = monitor.x().unwrap_or(0);
    let my = monitor.y().unwrap_or(0);
    let mw = monitor.width().map_err(|err| {
        DesktopToolError::internal_error(format!("read monitor width failed: {err}"))
    })?;
    let mh = monitor.height().map_err(|err| {
        DesktopToolError::internal_error(format!("read monitor height failed: {err}"))
    })?;

    let started = Instant::now();
    let (rgba, width, height, bounds) = match input.mode {
        ScreenshotMode::Region => {
            let region = input
                .region
                .as_ref()
                .ok_or_else(|| DesktopToolError::invalid_params("region is required"))?;
            let (sx, sy, ex, ey) = normalize_region_crop(region, mw, mh)?;
            let crop_width = ex - sx;
            let crop_height = ey - sy;
            let image = monitor.capture_region(sx, sy, crop_width, crop_height).map_err(|err| {
                DesktopToolError::internal_error(format!("capture region failed: {err}"))
            })?;
            (
                image.into_raw(),
                crop_width,
                crop_height,
                ScreenBounds {
                    x: mx.saturating_add(sx as i32),
                    y: my.saturating_add(sy as i32),
                    width: crop_width,
                    height: crop_height,
                },
            )
        }
        ScreenshotMode::Desktop | ScreenshotMode::Monitor => {
            let image = monitor.capture_image().map_err(|err| {
                DesktopToolError::internal_error(format!("capture desktop failed: {err}"))
            })?;
            let w = image.width();
            let h = image.height();
            (
                image.into_raw(),
                w,
                h,
                ScreenBounds {
                    x: mx,
                    y: my,
                    width: w,
                    height: h,
                },
            )
        }
    };
    let capture_ms = started.elapsed().as_millis() as u64;

    Ok((rgba, width, height, bounds, capture_ms))
}

fn encode_screenshot_response(
    input: &ScreenshotRequest,
    rgba: &[u8],
    width: u32,
    height: u32,
    bounds: ScreenBounds,
    capture_ms: u64,
    started: Instant,
) -> DesktopToolResult<ScreenshotResponse> {
    let encode_started = Instant::now();
    let rgb = rgba_to_rgb(rgba);
    let webp = webp::Encoder::from_rgb(&rgb, width, height).encode(input.webp_quality);
    let webp_bytes: &[u8] = webp.as_ref();
    let image_base64 = B64.encode(webp_bytes);
    let encode_ms = encode_started.elapsed().as_millis() as u64;

    let (path, save_ms) = maybe_save_from_base64(input, &image_base64)?;

    Ok(ScreenshotResponse {
        ok: true,
        path,
        image_mime: "image/webp".to_string(),
        image_base64,
        width,
        height,
        bounds,
        elapsed_ms: started.elapsed().as_millis() as u64,
        capture_ms,
        encode_ms,
        save_ms,
        timestamp: now_iso(),
    })
}

fn resolve_window_by_id(windows: &[xcap::Window], window_id: u32) -> Option<xcap::Window> {
    windows
        .iter()
        .find(|w| w.id().ok() == Some(window_id))
        .cloned()
}

fn resolve_focused_window(windows: &[xcap::Window]) -> Option<xcap::Window> {
    windows
        .iter()
        .find(|w| w.is_focused().unwrap_or(false))
        .cloned()
}

fn capture_window_once_xcap(window_id: Option<u32>) -> DesktopToolResult<(Vec<u8>, u32, u32, ScreenBounds, u64)> {
    let windows = window_list()?;
    if windows.is_empty() {
        return Err(DesktopToolError {
            code: DesktopToolErrorCode::TargetNotFound,
            message: "no capturable windows found".to_string(),
            details: None,
        });
    }
    let window = if let Some(id) = window_id {
        resolve_window_by_id(&windows, id).ok_or_else(|| DesktopToolError {
            code: DesktopToolErrorCode::TargetNotFound,
            message: format!("window_id not found: {id}"),
            details: None,
        })?
    } else {
        resolve_focused_window(&windows).ok_or_else(|| DesktopToolError {
            code: DesktopToolErrorCode::TargetNotFound,
            message: "focused window not found".to_string(),
            details: None,
        })?
    };

    let x = window.x().unwrap_or(0);
    let y = window.y().unwrap_or(0);
    let started = Instant::now();
    let image = window
        .capture_image()
        .map_err(|err| DesktopToolError::internal_error(format!("capture window failed: {err}")))?;
    let capture_ms = started.elapsed().as_millis() as u64;
    let width = image.width();
    let height = image.height();
    let rgba = image.into_raw();
    Ok((
        rgba,
        width,
        height,
        ScreenBounds {
            x,
            y,
            width,
            height,
        },
        capture_ms,
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct XcapWindowInfo {
    id: u32,
    pid: u32,
    app_name: String,
    title: String,
    x: i32,
    y: i32,
    z: i32,
    width: u32,
    height: u32,
    is_focused: bool,
    is_minimized: bool,
    is_maximized: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct XcapMonitorInfo {
    id: u32,
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    rotation: f32,
    scale_factor: f32,
    frequency: f32,
    is_primary: bool,
    is_builtin: bool,
}

fn xcap_list_windows_infos() -> DesktopToolResult<Vec<XcapWindowInfo>> {
    let windows = window_list()?;
    let mut out = Vec::with_capacity(windows.len());
    for w in windows {
        let id = w.id().unwrap_or(0);
        let pid = w.pid().unwrap_or(0);
        let app_name = w.app_name().unwrap_or_default();
        let title = w.title().unwrap_or_default();
        let x = w.x().unwrap_or(0);
        let y = w.y().unwrap_or(0);
        let z = w.z().unwrap_or(0);
        let width = w.width().unwrap_or(0);
        let height = w.height().unwrap_or(0);
        let is_focused = w.is_focused().unwrap_or(false);
        let is_minimized = w.is_minimized().unwrap_or(false);
        let is_maximized = w.is_maximized().unwrap_or(false);
        out.push(XcapWindowInfo {
            id,
            pid,
            app_name,
            title,
            x,
            y,
            z,
            width,
            height,
            is_focused,
            is_minimized,
            is_maximized,
        });
    }
    Ok(out)
}

fn xcap_list_monitors_infos() -> DesktopToolResult<Vec<XcapMonitorInfo>> {
    let monitors = monitor_list()?;
    let mut out = Vec::with_capacity(monitors.len());
    for m in monitors {
        out.push(XcapMonitorInfo {
            id: m.id().unwrap_or(0),
            name: m.name().unwrap_or_default(),
            x: m.x().unwrap_or(0),
            y: m.y().unwrap_or(0),
            width: m.width().unwrap_or(0),
            height: m.height().unwrap_or(0),
            rotation: m.rotation().unwrap_or(0.0),
            scale_factor: m.scale_factor().unwrap_or(1.0),
            frequency: m.frequency().unwrap_or(0.0),
            is_primary: m.is_primary().unwrap_or(false),
            is_builtin: m.is_builtin().unwrap_or(false),
        });
    }
    Ok(out)
}

fn run_capture_window_tool(input: ScreenshotRequest, window_id: Option<u32>) -> DesktopToolResult<ScreenshotResponse> {
    validate_screenshot_request(&input)?;
    ensure_linux_display_backend_supported()?;
    let started = Instant::now();
    let (rgba, width, height, bounds, capture_ms) = capture_window_once_xcap(window_id)?;
    encode_screenshot_response(&input, &rgba, width, height, bounds, capture_ms, started)
}

async fn run_screenshot_tool(input: ScreenshotRequest) -> DesktopToolResult<ScreenshotResponse> {
    validate_screenshot_request(&input)?;
    let started = Instant::now();

    let (rgba, width, height, bounds, capture_ms) = capture_once_xcap(&input)?;
    encode_screenshot_response(&input, &rgba, width, height, bounds, capture_ms, started)
}
