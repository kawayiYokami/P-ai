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

/// 在截图上绘制网格参考线：把图面按 cols/rows 等分，画浅灰半透线（不含外框）。
/// 画在缩放后的图面上，线宽固定 1px，不会被缩放抹糊；刻意避开红/蓝等语义色，避免被误认成元素矩形。
/// 份数由调用方限制在合理区间（见 operate 解析），这里只做防御性收敛。
fn draw_screen_grid(image: &mut image::RgbaImage, grid: ScreenGrid) {
    let cols = grid.cols.max(1);
    let rows = grid.rows.max(1);
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return;
    }
    const LINE_COLOR: image::Rgba<u8> = image::Rgba([200, 200, 200, 160]);
    for i in 1..cols {
        let x = (u64::from(width) * u64::from(i) / u64::from(cols)) as u32;
        if x == 0 || x >= width {
            continue;
        }
        for y in 0..height {
            image.put_pixel(x, y, LINE_COLOR);
        }
    }
    for i in 1..rows {
        let y = (u64::from(height) * u64::from(i) / u64::from(rows)) as u32;
        if y == 0 || y >= height {
            continue;
        }
        for x in 0..width {
            image.put_pixel(x, y, LINE_COLOR);
        }
    }
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
    if width > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION
        || height > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION
    {
        return Err(DesktopToolError::invalid_params(format!(
            "截图分辨率过大（{}x{}），当前最多支持 {}x{}；请缩小截取范围：只截目标区域（region）、单块显示器（monitor）或目标窗口（window_id/window）",
            width,
            height,
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION,
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION
        )));
    }
    let encode_started = Instant::now();
    // 调用方给像素预算时等比缩小到预算内（默认保持全尺寸，历史行为不变）。
    // 单边长度已由本函数上方校验，这里只用像素预算缩放，单边不再额外设限。
    let full_pixels = u64::from(width) * u64::from(height);
    let target_pixel_budget = input.max_pixels.unwrap_or(full_pixels).min(full_pixels).max(1);
    let grid = input.grid;
    let normalized = normalize_rgba_image_for_llm_request_with_overlay(
        rgba,
        width,
        height,
        LlmRequestImageNormalizeOptions {
            pixel_budget: target_pixel_budget,
            resize_dimension: width.max(height),
            webp_quality: input.webp_quality,
        },
        |image| {
            if let Some(grid) = grid {
                // 输入恒为 RGBA，resize_exact 保持变体，as_mut_rgba8 必然有值
                if let Some(canvas) = image.as_mut_rgba8() {
                    draw_screen_grid(canvas, grid);
                }
            }
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
        grid: None,
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
        grid: None,
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

#[test]
fn encode_screenshot_response_should_accept_2k_capture_and_respect_max_pixels() {
    // 源尺寸上限与缩放后单边上限是两件事：2560x1440 属于合法源尺寸，
    // 不能因为缩放后单边上限是 2048 就被当成非法截图拒掉。
    let input = ScreenshotRequest {
        mode: ScreenshotMode::Desktop,
        monitor_id: None,
        region: None,
        save_path: None,
        webp_quality: 75.0,
        include_base64: true,
        max_pixels: None,
        grid: None,
    };
    let width = 2560u32;
    let height = 1440u32;
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
    .expect("2k screenshot should encode");

    assert!(result.image_base64.is_some());
    assert_eq!(result.width, width);
    assert_eq!(result.height, height);
}

#[test]
fn encode_screenshot_response_should_scale_to_max_pixels() {
    let input = ScreenshotRequest {
        mode: ScreenshotMode::Desktop,
        monitor_id: None,
        region: None,
        save_path: None,
        webp_quality: 75.0,
        include_base64: true,
        max_pixels: Some(40_000),
        grid: None,
    };
    let width = 1920u32;
    let height = 1080u32;
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
    .expect("screenshot should encode");

    assert!(
        u64::from(result.width) * u64::from(result.height) <= 40_000,
        "输出 {}x{} 超出 max_pixels 预算",
        result.width,
        result.height
    );
}

#[test]
fn draw_screen_grid_should_stroke_equal_parts_without_touching_border() {
    let mut image = image::RgbaImage::from_pixel(80, 60, image::Rgba([0, 0, 0, 255]));
    draw_screen_grid(&mut image, ScreenGrid { cols: 4, rows: 3 });
    // 4 列 3 行各产生 3 条与 2 条等分线：x=20/40/60，y=20/40
    for x in [20u32, 40, 60] {
        assert_ne!(image.get_pixel(x, 0).0[0], 0, "竖线应出现在 x={x}");
    }
    for y in [20u32, 40] {
        assert_ne!(image.get_pixel(0, y).0[0], 0, "横线应出现在 y={y}");
    }
    // 边界与被非等分位置保持原样（不画外框，避免被误读成元素矩形）
    assert_eq!(image.get_pixel(0, 0).0[0], 0);
    assert_eq!(image.get_pixel(10, 10).0[0], 0);
    assert_eq!(image.get_pixel(79, 59).0[0], 0);
}

#[test]
fn draw_screen_grid_should_be_noop_on_single_part() {
    // 1x1 没有等分点，调用方已在解析阶段拦截；这里确认绘制本身不会碰图面
    let mut image = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 255]));
    draw_screen_grid(&mut image, ScreenGrid { cols: 1, rows: 1 });
    assert!(image.pixels().all(|p| p.0[0] == 0));
}
