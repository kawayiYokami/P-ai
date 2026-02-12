#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DesktopToolErrorCode {
    InvalidParams,
    Timeout,
    TargetNotFound,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopToolError {
    code: DesktopToolErrorCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl DesktopToolError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: DesktopToolErrorCode::InvalidParams,
            message: message.into(),
            details: None,
        }
    }

    fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: DesktopToolErrorCode::InternalError,
            message: message.into(),
            details: None,
        }
    }
}

type DesktopToolResult<T> = Result<T, DesktopToolError>;

fn to_tool_err_string(err: &DesktopToolError) -> String {
    serde_json::to_string(err).unwrap_or_else(|_| err.message.clone())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ScreenshotMode {
    Desktop,
    Monitor,
    Region,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotRequest {
    #[serde(default = "default_screenshot_mode")]
    mode: ScreenshotMode,
    #[serde(default)]
    monitor_id: Option<u32>,
    #[serde(default)]
    region: Option<ScreenBounds>,
    #[serde(default)]
    save_path: Option<String>,
    #[serde(default = "default_webp_quality")]
    webp_quality: f32,
}

fn default_screenshot_mode() -> ScreenshotMode {
    ScreenshotMode::Desktop
}

fn default_webp_quality() -> f32 {
    75.0
}

fn normalize_webp_quality(value: Option<f32>) -> f32 {
    match value {
        Some(v) if v.is_finite() => v.clamp(1.0, 100.0),
        _ => default_webp_quality(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    image_mime: String,
    image_base64: String,
    width: u32,
    height: u32,
    bounds: ScreenBounds,
    elapsed_ms: u64,
    capture_ms: u64,
    encode_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    save_ms: Option<u64>,
    timestamp: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WaitMode {
    Sleep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WaitRequest {
    #[serde(default = "default_wait_mode")]
    mode: WaitMode,
    ms: u64,
}

fn default_wait_mode() -> WaitMode {
    WaitMode::Sleep
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WaitResponse {
    ok: bool,
    waited_ms: u64,
    elapsed_ms: u64,
}
