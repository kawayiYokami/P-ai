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

/// 网格参考线：把截图按份数等分，画浅灰半透线。
/// 目的是让模型看图时能说出「第几列第几行」，缩小目测比例带来的坐标误差。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenGrid {
    /// 纵向切分份数（生成 cols-1 条竖线）
    cols: u32,
    /// 横向切分份数（生成 rows-1 条横线）
    rows: u32,
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
    #[serde(default = "default_include_screenshot_base64")]
    include_base64: bool,
    /// 像素预算上限（F2）：超出时等比缩小到该预算内；None 表示保持全尺寸。
    /// operate 截图 DSL 的 max_pixels= 落到这里；其他调用方不传即历史行为。
    #[serde(default)]
    max_pixels: Option<u64>,
    /// 网格参考线：None 表示画干净图（历史行为）。operate 截图 DSL 的 grid=列x行 落到这里。
    #[serde(default)]
    grid: Option<ScreenGrid>,
}

fn default_screenshot_mode() -> ScreenshotMode {
    ScreenshotMode::Desktop
}

fn default_webp_quality() -> f32 {
    75.0
}

fn default_include_screenshot_base64() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    image_mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_base64: Option<String>,
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
