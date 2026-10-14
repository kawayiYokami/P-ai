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

fn maybe_save_screenshot_bytes(
    input: &ScreenshotRequest,
    image_bytes: &[u8],
) -> DesktopToolResult<(Option<String>, Option<u64>)> {
    if input.save_path.is_none() {
        return Ok((None, None));
    }
    let started = Instant::now();
    let output_path = build_output_path(input)?;
    std::fs::write(&output_path, image_bytes).map_err(|err| {
        DesktopToolError::internal_error(format!("write screenshot file failed: {err}"))
    })?;
    Ok((
        Some(output_path.to_string_lossy().to_string()),
        Some(started.elapsed().as_millis() as u64),
    ))
}

/// region 是虚拟桌面绝对像素矩形：找一块完整包含它的显示器。
/// xcap 的 capture_region 只接受显示器内偏移，跨显示器矩形无法一次裁剪，直接报错让模型分次截图。
fn resolve_monitor_containing_region(
    monitors: &[xcap::Monitor],
    region: &ScreenBounds,
) -> DesktopToolResult<xcap::Monitor> {
    let x0 = region.x;
    let y0 = region.y;
    let x1 = region.x.saturating_add(region.width as i32);
    let y1 = region.y.saturating_add(region.height as i32);
    for monitor in monitors {
        let mx = monitor.x().unwrap_or(0);
        let my = monitor.y().unwrap_or(0);
        let (Ok(mw), Ok(mh)) = (monitor.width(), monitor.height()) else {
            continue;
        };
        if x0 >= mx && y0 >= my && x1 <= mx.saturating_add(mw as i32) && y1 <= my.saturating_add(mh as i32) {
            return Ok(monitor.clone());
        }
    }
    Err(DesktopToolError::invalid_params(format!(
        "region 必须完整落在一块显示器内：@({}, {}), {}x{} 超出单块显示器范围；跨显示器请分次截图",
        region.x, region.y, region.width, region.height
    )))
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

    let started = Instant::now();
    let (rgba, width, height, bounds) = match input.mode {
        ScreenshotMode::Region => {
            let region = input
                .region
                .as_ref()
                .ok_or_else(|| DesktopToolError::invalid_params("region is required"))?;
            // region 是虚拟桌面绝对像素：先找完整包含它的显示器，再换算成该显示器内偏移
            // （xcap 的 capture_region 只接受显示器内偏移，跨显示器矩形无法一次裁剪）
            let monitor = resolve_monitor_containing_region(&monitors, region)?;
            let region_x = monitor.x().unwrap_or(0);
            let region_y = monitor.y().unwrap_or(0);
            let sx = (region.x - region_x) as u32;
            let sy = (region.y - region_y) as u32;
            let crop_width = region.width;
            let crop_height = region.height;
            let image = monitor.capture_region(sx, sy, crop_width, crop_height).map_err(|err| {
                DesktopToolError::internal_error(format!("capture region failed: {err}"))
            })?;
            (
                image.into_raw(),
                crop_width,
                crop_height,
                ScreenBounds {
                    x: region.x,
                    y: region.y,
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
    if width > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION
        || height > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION
    {
        return Err(DesktopToolError::invalid_params(format!(
            "截图分辨率过大（{}x{}），当前最多支持 {}x{}；请缩小截取范围：只截目标区域（region）、单块显示器（monitor）或目标窗口（window_id/window）",
            width,
            height,
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION,
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION
        )));
    }
    let encode_started = Instant::now();
    // F2：调用方给像素预算时等比缩小到预算内（默认保持全尺寸，历史行为不变）
    let full_pixels = u64::from(width) * u64::from(height);
    let target_pixel_budget = input.max_pixels.unwrap_or(full_pixels).min(full_pixels).max(1);
    let normalized = normalize_rgba_image_for_llm_request_with_options(
        rgba,
        width,
        height,
        LlmRequestImageNormalizeOptions {
            target_pixel_budget,
            webp_quality: input.webp_quality,
            max_source_bytes: u64::MAX,
            max_dimension: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION,
        },
    )
    .map_err(DesktopToolError::internal_error)?;
    let image_base64 = if input.include_base64 {
        Some(B64.encode(&normalized.bytes))
    } else {
        None
    };
    let encode_ms = encode_started.elapsed().as_millis() as u64;

    let (path, save_ms) = maybe_save_screenshot_bytes(input, &normalized.bytes)?;

    Ok(ScreenshotResponse {
        ok: true,
        path,
        image_mime: normalized.mime,
        image_base64,
        width: normalized.output_width,
        height: normalized.output_height,
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

fn resolve_focused_window(windows: &[xcap::Window]) -> DesktopToolResult<xcap::Window> {
    // 前台窗口若属于 PAI 自身（xcap 枚举会过滤当前进程窗口，直接找不到），给出明确提示而非模糊的 not found。
    // 这样模型知道要先切换到目标应用再重试。
    #[cfg(target_os = "windows")]
    unsafe {
        let fg = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if !fg.is_null() {
            let mut pid = 0u32;
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(fg, &mut pid);
            if pid == std::process::id() {
                return Err(DesktopToolError {
                    code: DesktopToolErrorCode::TargetNotFound,
                    message: "当前前台窗口是 PAI 自身，请先切换到目标应用窗口后再重试 focused_window".to_string(),
                    details: None,
                });
            }
        }
    }
    windows
        .iter()
        .find(|w| w.is_focused().unwrap_or(false))
        .cloned()
        .ok_or_else(|| DesktopToolError {
            code: DesktopToolErrorCode::TargetNotFound,
            message: "focused window not found".to_string(),
            details: None,
        })
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
        resolve_focused_window(&windows)?
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
    // 截图捕获、WebP 编码与写盘都是同步 IO + CPU 密集操作，移到 blocking 线程池，
    // 避免每次截图占住 tokio 工作线程。
    tokio::task::spawn_blocking(move || {
        validate_screenshot_request(&input)?;
        let started = Instant::now();
        let (rgba, width, height, bounds, capture_ms) = capture_once_xcap(&input)?;
        encode_screenshot_response(&input, &rgba, width, height, bounds, capture_ms, started)
    })
    .await
    .map_err(|err| {
        DesktopToolError::internal_error(format!("screenshot task failed: {err}"))
    })?
}

#[cfg(test)]
#[test]
fn encode_screenshot_response_should_reject_over_10k_capture() {
    let input = ScreenshotRequest {
        mode: ScreenshotMode::Desktop,
        monitor_id: None,
        region: None,
        save_path: None,
        webp_quality: 75.0,
        include_base64: true,
        max_pixels: None,
    };
    let width = 10_001u32;
    let height = 8u32;
    let rgba = vec![0u8; (width as usize) * (height as usize) * 4];
    let err = encode_screenshot_response(
        &input,
        &rgba,
        width,
        height,
        ScreenBounds {
            x: 0,
            y: 0,
            width,
            height,
        },
        0,
        Instant::now(),
    )
    .expect_err("oversized screenshot should be rejected");

    assert!(matches!(err.code, DesktopToolErrorCode::InvalidParams));
    assert!(err.message.contains("10000x10000"));
}

#[test]
fn encode_screenshot_response_should_skip_base64_and_save_file_when_disabled() {
    let save_path = std::env::temp_dir()
        .join(format!("easy-call-ai-encode-test-{}.webp", Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let input = ScreenshotRequest {
        mode: ScreenshotMode::Desktop,
        monitor_id: None,
        region: None,
        save_path: Some(save_path.clone()),
        webp_quality: 75.0,
        include_base64: false,
        max_pixels: None,
    };
    let width = 64u32;
    let height = 64u32;
    let rgba = vec![0u8; (width as usize) * (height as usize) * 4];
    let result = encode_screenshot_response(
        &input,
        &rgba,
        width,
        height,
        ScreenBounds {
            x: 0,
            y: 0,
            width,
            height,
        },
        0,
        Instant::now(),
    )
    .expect("small screenshot should encode");

    assert!(
        result.image_base64.is_none(),
        "base64 should be skipped when include_base64=false"
    );
    let saved = result.path.expect("file should be saved when save_path given");
    assert!(
        std::path::Path::new(&saved).exists(),
        "screenshot file should exist on disk: {saved}"
    );
    let _ = std::fs::remove_file(&saved);
}
