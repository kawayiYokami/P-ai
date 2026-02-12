use std::time::Instant;

#[cfg(target_os = "windows")]
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
#[cfg(target_os = "windows")]
use windows_capture::frame::Frame;
#[cfg(target_os = "windows")]
use windows_capture::graphics_capture_api::InternalCaptureControl;
#[cfg(target_os = "windows")]
use windows_capture::monitor::Monitor;
#[cfg(target_os = "windows")]
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

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
    let started = std::time::Instant::now();
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

fn bgra_to_rgb(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity((bytes.len() / 4) * 3);
    for px in bytes.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0]]);
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

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct CaptureFlags {
    output: Arc<Mutex<Option<CaptureFrame>>>,
    crop_region: Option<ScreenBounds>,
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct CaptureFrame {
    width: u32,
    height: u32,
    bgra: Vec<u8>,
    bounds: ScreenBounds,
}

#[cfg(target_os = "windows")]
fn capture_once_windows(input: &ScreenshotRequest) -> DesktopToolResult<(Vec<u8>, u32, u32, ScreenBounds, u64)> {
    let monitor = if matches!(input.mode, ScreenshotMode::Monitor) {
        let monitor_id = input
            .monitor_id
            .ok_or_else(|| DesktopToolError::invalid_params("monitor_id is required"))?;
        let by_zero_based = Monitor::from_index(monitor_id as usize + 1);
        match by_zero_based {
            Ok(v) => v,
            Err(_) if monitor_id > 0 => Monitor::from_index(monitor_id as usize).map_err(|err| {
                DesktopToolError::invalid_params(format!("monitor_id not found: {err}"))
            })?,
            Err(err) => {
                return Err(DesktopToolError::invalid_params(format!(
                    "monitor_id not found: {err}"
                )))
            }
        }
    } else {
        Monitor::primary().map_err(|err| {
            DesktopToolError::internal_error(format!("resolve primary monitor failed: {err}"))
        })?
    };

    let output = Arc::new(Mutex::new(None::<CaptureFrame>));
    let flags = CaptureFlags {
        output: output.clone(),
        crop_region: if matches!(input.mode, ScreenshotMode::Region) {
            input.region.clone()
        } else {
            None
        },
    };

    struct Handler {
        flags: CaptureFlags,
    }

    impl GraphicsCaptureApiHandler for Handler {
        type Flags = CaptureFlags;
        type Error = String;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self { flags: ctx.flags })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let frame_w = frame.width();
            let frame_h = frame.height();

            let mut fb = if let Some(region) = &self.flags.crop_region {
                let (sx, sy, ex, ey) = normalize_region_crop(region, frame_w, frame_h)
                    .map_err(|err| to_tool_err_string(&err))?;
                frame
                    .buffer_crop(sx, sy, ex, ey)
                    .map_err(|err| format!("crop frame failed: {err}"))?
            } else {
                frame
                    .buffer()
                    .map_err(|err| format!("read frame buffer failed: {err}"))?
            };
            let w = fb.width();
            let h = fb.height();
            let bytes = fb
                .as_nopadding_buffer()
                .map_err(|err| format!("normalize frame buffer failed: {err}"))?
                .to_vec();

            let bounds = if let Some(region) = &self.flags.crop_region {
                ScreenBounds {
                    x: region.x.max(0),
                    y: region.y.max(0),
                    width: w,
                    height: h,
                }
            } else {
                ScreenBounds {
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                }
            };

            let mut guard = self
                .flags
                .output
                .lock()
                .map_err(|_| "capture output mutex poisoned".to_string())?;
            *guard = Some(CaptureFrame {
                width: w,
                height: h,
                bgra: bytes,
                bounds,
            });
            drop(guard);

            capture_control.stop();
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    let started = Instant::now();
    Handler::start(settings).map_err(|err| {
        DesktopToolError::internal_error(format!("windows-capture start failed: {err}"))
    })?;
    let capture_ms = started.elapsed().as_millis() as u64;

    let frame = output
        .lock()
        .map_err(|_| DesktopToolError::internal_error("capture output mutex poisoned"))?
        .clone()
        .ok_or_else(|| DesktopToolError::internal_error("no frame captured"))?;

    Ok((frame.bgra, frame.width, frame.height, frame.bounds, capture_ms))
}

#[cfg(not(target_os = "windows"))]
fn capture_once_windows(
    _input: &ScreenshotRequest,
) -> DesktopToolResult<(Vec<u8>, u32, u32, ScreenBounds, u64)> {
    Err(DesktopToolError::internal_error(
        "windows-capture is only available on Windows",
    ))
}

async fn run_screenshot_tool(input: ScreenshotRequest) -> DesktopToolResult<ScreenshotResponse> {
    validate_screenshot_request(&input)?;
    let started = Instant::now();

    let (bgra, width, height, bounds, capture_ms) = capture_once_windows(&input)?;
    let encode_started = Instant::now();
    let rgb = bgra_to_rgb(&bgra);
    let webp = webp::Encoder::from_rgb(&rgb, width, height).encode(input.webp_quality);
    let webp_bytes: &[u8] = webp.as_ref();
    let image_base64 = B64.encode(webp_bytes);
    let encode_ms = encode_started.elapsed().as_millis() as u64;

    let (path, save_ms) = maybe_save_from_base64(&input, &image_base64)?;

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
