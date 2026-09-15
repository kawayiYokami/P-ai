const READ_FILE_TEXT_LIMIT_CHARS: usize = 30_000;
const READ_TOOL_NAME: &str = "read";
const READ_MEDIA_TOOL_NAME: &str = "read_media";
const READ_MEDIA_IMAGE_HTTP_TIMEOUT_SECS: u64 = 60;
const READ_MEDIA_AUDIO_HTTP_TIMEOUT_SECS: u64 = 3 * 60;
const READ_MEDIA_VIDEO_HTTP_TIMEOUT_SECS: u64 = 8 * 60;
const GEMINI_INLINE_AUDIO_LIMIT_BYTES: usize = 20 * 1024 * 1024;
const GEMINI_INLINE_VIDEO_LIMIT_BYTES: usize = 100 * 1024 * 1024;
const OPENAI_FAMILY_VIDEO_DATA_URL_LIMIT_BYTES: usize = 50 * 1024 * 1024;
const QWEN_MEDIA_DATA_URL_LIMIT_BYTES: usize = 50 * 1024 * 1024;
const MIMO_VIDEO_BASE64_LIMIT_BYTES: usize = 50 * 1024 * 1024;
const MINIMAX_VIDEO_BASE64_LIMIT_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadMediaDetectedType {
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadMediaRouteFamily {
    Qwen,
    OpenAI,
    Gemini,
    Anthropic,
    MiniMax,
    Mimo,
}

impl ReadMediaRouteFamily {
    fn as_str(self) -> &'static str {
        match self {
            Self::Qwen => "Qwen",
            Self::OpenAI => "OpenAI",
            Self::Gemini => "Gemini",
            Self::Anthropic => "Anthropic",
            Self::MiniMax => "MiniMax",
            Self::Mimo => "Mimo",
        }
    }
}

impl ReadMediaDetectedType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

fn resolve_read_media_route_family(
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
) -> ReadMediaRouteFamily {
    let resolved_adapter = resolve_model_protocol(
        request_format,
        base_url,
        model_name,
        genai::adapter::AdapterKind::OpenAI,
    )
    .adapter_kind;
    if should_use_opencode_qwen_read_media_compat(base_url, model_name) {
        return ReadMediaRouteFamily::Anthropic;
    }
    if is_qwen_model_name(model_name)
        && resolved_adapter != genai::adapter::AdapterKind::Anthropic
    {
        return ReadMediaRouteFamily::Qwen;
    }
    match resolved_adapter {
        genai::adapter::AdapterKind::Gemini => ReadMediaRouteFamily::Gemini,
        genai::adapter::AdapterKind::Anthropic => ReadMediaRouteFamily::Anthropic,
        genai::adapter::AdapterKind::MiniMax => ReadMediaRouteFamily::MiniMax,
        genai::adapter::AdapterKind::Mimo => ReadMediaRouteFamily::Mimo,
        _ => ReadMediaRouteFamily::OpenAI,
    }
}

fn should_use_opencode_qwen_read_media_compat(base_url: &str, model_name: &str) -> bool {
    is_qwen_model_name(model_name)
        && resolve_adapter_kind_from_base_url(base_url)
            == Some(genai::adapter::AdapterKind::OpenCodeGo)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFileDetectedType {
    Text,
    Image,
    Pdf,
    Doc,
    Docx,
    Xls,
    Xlsx,
    Xlsb,
    Ppt,
    Pptx,
    Ods,
    Odp,
    Rtf,
    Numbers,
    Pages,
    Keynote,
    Unknown,
}

impl ReadFileDetectedType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Pdf => "pdf",
            Self::Doc => "doc",
            Self::Docx => "docx",
            Self::Xls => "xls",
            Self::Xlsx => "xlsx",
            Self::Xlsb => "xlsb",
            Self::Ppt => "ppt",
            Self::Pptx => "pptx",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Rtf => "rtf",
            Self::Numbers => "numbers",
            Self::Pages => "pages",
            Self::Keynote => "keynote",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReadFileRequest {
    #[serde(alias = "absolute_path", alias = "absolutePath")]
    path: String,
    #[serde(default)]
    #[serde(alias = "start")]
    offset: Option<usize>,
    #[serde(default)]
    #[serde(alias = "count")]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReadMediaRequest {
    #[serde(alias = "absolute_path", alias = "absolutePath")]
    path: String,
    #[serde(default)]
    description: Option<String>,
}

trait ReadFileReader {
    fn reader_kind(&self) -> &'static str;
    fn supports(&self, detected: ReadFileDetectedType) -> bool;
    fn read(
        &self,
        state: &AppState,
        session_id: &str,
        api_config_id: &str,
        request: &ReadFileRequest,
        detected: ReadFileDetectedType,
    ) -> Result<Value, String>;
}

fn read_file_ext(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|v| v.to_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn detect_read_file_type(path: &std::path::Path) -> ReadFileDetectedType {
    match read_file_ext(path).as_str() {
        "txt" | "md" | "rs" | "ts" | "tsx" | "js" | "jsx" | "json" | "toml" | "yaml" | "yml"
        | "vue" | "html" | "css" | "scss" | "less" | "xml" | "csv" | "log" | "ini" | "conf"
        | "bat" | "cmd" | "ps1" | "sh" | "sql" | "py" | "java" | "kt" | "go" | "c" | "cpp"
        | "h" | "hpp" | "cs" | "swift" | "rb" | "php" | "svg" => ReadFileDetectedType::Text,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => ReadFileDetectedType::Image,
        "pdf" => ReadFileDetectedType::Pdf,
        "doc" => ReadFileDetectedType::Doc,
        "docx" => ReadFileDetectedType::Docx,
        "xls" => ReadFileDetectedType::Xls,
        "xlsx" => ReadFileDetectedType::Xlsx,
        "xlsb" => ReadFileDetectedType::Xlsb,
        "ppt" => ReadFileDetectedType::Ppt,
        "pptx" => ReadFileDetectedType::Pptx,
        "ods" => ReadFileDetectedType::Ods,
        "odp" => ReadFileDetectedType::Odp,
        "rtf" => ReadFileDetectedType::Rtf,
        "numbers" => ReadFileDetectedType::Numbers,
        "pages" => ReadFileDetectedType::Pages,
        "key" => ReadFileDetectedType::Keynote,
        _ => ReadFileDetectedType::Unknown,
    }
}

fn detect_read_media_type(path: &std::path::Path) -> Option<ReadMediaDetectedType> {
    match read_file_ext(path).as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => Some(ReadMediaDetectedType::Image),
        "mp3" | "wav" | "wave" | "m4a" | "aac" | "ogg" | "flac" | "webm" => {
            Some(ReadMediaDetectedType::Audio)
        }
        "mp4" | "mov" | "avi" | "mkv" | "m4v" => Some(ReadMediaDetectedType::Video),
        _ => media_mime_from_path(path).and_then(|mime| {
            let lower = mime.to_ascii_lowercase();
            if lower.starts_with("image/") {
                Some(ReadMediaDetectedType::Image)
            } else if lower.starts_with("audio/") {
                Some(ReadMediaDetectedType::Audio)
            } else if lower.starts_with("video/") {
                Some(ReadMediaDetectedType::Video)
            } else {
                None
            }
        }),
    }
}

fn read_file_conversation_cache_key(session_id: &str) -> String {
    delegate_session_conversation_id(session_id)
        .unwrap_or_else(|| session_id.trim().to_string())
}

fn read_file_log_target(path: &std::path::Path) -> String {
    let file_name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default();
    let ext = path.extension().and_then(|v| v.to_str()).unwrap_or_default();
    if !file_name.is_empty() {
        if ext.is_empty() {
            return format!("file_name={}", file_name);
        }
        return format!("file_name={}，ext={}", file_name, ext);
    }
    if ext.is_empty() {
        "file_name=(unknown)".to_string()
    } else {
        format!("file_name=(unknown)，ext={}", ext)
    }
}

fn ensure_absolute_file_path(request: &ReadFileRequest) -> Result<std::path::PathBuf, String> {
    let trimmed = request.path.trim();
    if trimmed.is_empty() {
        return Err("path 不能为空".to_string());
    }
    if matches!(request.limit, Some(0)) {
        return Err("limit 必须大于等于 1".to_string());
    }
    let path = std::path::PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("path 必须是绝对路径".to_string());
    }
    // 不用 Path::exists()：TCC 拒绝时它会把访问错误吞成 false，误报"文件不存在"。
    // 直接 metadata，NotFound 报不存在，其他错误（如 EPERM）保留原始 I/O 错误文本。
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("文件不存在：{}", path.display()));
        }
        Err(err) => return Err(format!("读取文件信息失败: {err}")),
    };
    if !metadata.is_file() {
        return Err(format!("目标不是文件：{}", path.display()));
    }
    Ok(path)
}

fn ensure_absolute_media_path(request: &ReadMediaRequest) -> Result<std::path::PathBuf, String> {
    let trimmed = request.path.trim();
    if trimmed.is_empty() {
        return Err("path 不能为空".to_string());
    }
    let path = std::path::PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("path 必须是绝对路径".to_string());
    }
    if !path.exists() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    let metadata = std::fs::metadata(&path).map_err(|err| format!("读取文件信息失败: {err}"))?;
    if !metadata.is_file() {
        return Err(format!("目标不是文件：{}", path.display()));
    }
    Ok(path)
}

fn paginate_lines(lines: &[String], start: usize, count: Option<usize>) -> (Vec<String>, Option<usize>) {
    if start >= lines.len() {
        return (Vec::new(), None);
    }
    let end = count
        .map(|size| start.saturating_add(size).min(lines.len()))
        .unwrap_or(lines.len());
    let chunk = lines[start..end].to_vec();
    let next_start = if end < lines.len() { Some(end) } else { None };
    (chunk, next_start)
}

fn paginate_window(total: usize, start: usize, count: Option<usize>) -> (usize, usize, Option<usize>) {
    if start >= total {
        return (start, start, None);
    }
    let end = count
        .map(|size| start.saturating_add(size).min(total))
        .unwrap_or(total);
    let next_start = if end < total { Some(end) } else { None };
    (start, end, next_start)
}

fn truncate_text_for_read_file(text: &str) -> (String, bool) {
    let total = text.chars().count();
    if total <= READ_FILE_TEXT_LIMIT_CHARS {
        return (text.to_string(), false);
    }
    (
        text.chars().take(READ_FILE_TEXT_LIMIT_CHARS).collect::<String>(),
        true,
    )
}

fn detect_read_file_line_ending(text: &str) -> &'static str {
    let has_crlf = text.contains("\r\n");
    let without_crlf = text.replace("\r\n", "");
    let has_cr = without_crlf.contains('\r');
    let has_lf = without_crlf.contains('\n') || has_crlf;
    match (has_crlf, has_cr, has_lf) {
        (true, false, true) if !without_crlf.contains('\n') => "crlf",
        (false, false, true) => "lf",
        (false, true, false) => "cr",
        (false, false, false) => "none",
        _ => "mixed",
    }
}

fn normalize_text_line_endings_for_read_file(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_office_text_for_read_file(input: &str) -> String {
    let normalized = normalize_text_line_endings_for_read_file(input);
    let mut out = String::with_capacity(normalized.len());
    let mut last_was_newline = false;
    for ch in normalized.chars() {
        if ch == '\n' {
            if !last_was_newline {
                out.push('\n');
            }
            last_was_newline = true;
            continue;
        }
        if ch == '\t' {
            out.push('\t');
            last_was_newline = false;
            continue;
        }
        if ch.is_control() {
            continue;
        }
        out.push(ch);
        last_was_newline = false;
    }
    out.trim().to_string()
}

fn build_text_read_result(
    path: &std::path::Path,
    detected: ReadFileDetectedType,
    reader_kind: &str,
    text: &str,
    offset: Option<usize>,
    limit: Option<usize>,
    extra_metadata: Value,
) -> Value {
    let source_line_ending = detect_read_file_line_ending(text);
    let normalized_text = normalize_text_line_endings_for_read_file(text);
    let lines = normalized_text.split('\n').map(|v| v.to_string()).collect::<Vec<_>>();
    let applied_offset = offset.unwrap_or(0);
    let (selected_lines, next_offset_by_lines) = paginate_lines(&lines, applied_offset, limit);
    let joined = selected_lines.join("\n");
    let (truncated_text, char_truncated) = truncate_text_for_read_file(&joined);
    let next_offset = if char_truncated {
        next_offset_by_lines.or(Some(applied_offset + selected_lines.len()))
    } else {
        next_offset_by_lines
    };
    let mut output = String::new();
    if char_truncated {
        let continue_offset = next_offset.unwrap_or(applied_offset + selected_lines.len());
        output.push_str("Content was truncated to fit within 30000 character limit.\n");
        output.push_str(&format!(
            "To continue reading, use offset={} in the next read call.\n\n",
            continue_offset
        ));
    }
    output.push_str(&truncated_text);
    serde_json::json!({
        "ok": true,
        "path": path.to_string_lossy().to_string(),
        "detectedType": detected.as_str(),
        "readerKind": reader_kind,
        "truncated": char_truncated,
        "nextOffset": next_offset,
        "content": output,
        "metadata": {
            "kind": "text",
            "offset": applied_offset,
            "limit": limit,
            "returnedCount": selected_lines.len(),
            "totalCount": lines.len(),
            "returnedCharCount": joined.chars().count().min(READ_FILE_TEXT_LIMIT_CHARS),
            "charLimit": READ_FILE_TEXT_LIMIT_CHARS,
            "sourceLineEnding": source_line_ending,
            "contentLineEnding": "lf",
            "lineEndingNote": "content 已统一使用 LF(\\n) 返回；apply_patch 可用该内容作为 old_string，工具会兼容目标文件的 CRLF/LF。",
            "fileName": path.file_name().and_then(|v| v.to_str()).unwrap_or_default(),
            "extra": extra_metadata
        }
    })
}

fn resolve_pdf_image_mode(state: &AppState, api_config_id: &str) -> Result<bool, String> {
    let app_config = state_read_config_cached(state)?;
    let selected_api = resolve_selected_api_config(&app_config, Some(api_config_id))
        .or_else(|| resolve_selected_api_config(&app_config, None))
        .ok_or_else(|| "当前未找到可用聊天模型配置。".to_string())?;
    // PDF 阅读方式固定为图片模式：忽略持久化的 pdf_read_mode，仅受模型图片能力约束
    Ok(selected_api.enable_image)
}

fn build_pdf_image_read_result(
    path: &std::path::Path,
    detected: ReadFileDetectedType,
    structured: &PdfExtractStructuredResult,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Value {
    let applied_offset = offset.unwrap_or(0);
    let total_pages = structured.total_pages as usize;
    let (window_start, end, next_offset) = paginate_window(total_pages, applied_offset, limit);
    let selected_pages = if window_start >= total_pages {
        Vec::new()
    } else {
        structured.pages[window_start..end].to_vec()
    };
    let parts = selected_pages
        .iter()
        .flat_map(|page| {
            page.images.iter().map(move |image| {
                serde_json::json!({
                    "type": "image",
                    "mimeType": image.mime,
                    "data": image.bytes_base64,
                    "pageIndex": page.page_index,
                    "pageNumber": page.page_index + 1,
                    "width": image.width,
                    "height": image.height
                })
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "ok": true,
        "path": path.to_string_lossy().to_string(),
        "detectedType": detected.as_str(),
        "readerKind": "pdf_image_direct",
        "truncated": false,
        "nextOffset": next_offset,
        "parts": parts,
        "response": {
            "ok": true,
            "path": path.to_string_lossy().to_string(),
            "detectedType": detected.as_str(),
            "readerKind": "pdf_image_direct",
            "fileName": structured.file_name,
            "offset": applied_offset,
            "limit": limit,
            "returnedPageCount": selected_pages.len(),
            "returnedImageCount": selected_pages.iter().map(|page| page.images.len()).sum::<usize>(),
            "totalPages": structured.total_pages,
            "nextOffset": next_offset
        },
        "metadata": {
            "kind": "image",
            "fileName": structured.file_name,
            "offset": applied_offset,
            "limit": limit,
            "returnedPageCount": selected_pages.len(),
            "returnedImageCount": selected_pages.iter().map(|page| page.images.len()).sum::<usize>(),
            "totalPages": structured.total_pages,
            "includeImages": true
        }
    })
}

fn build_read_media_prepared_prompt(
    media_type: ReadMediaDetectedType,
    mime: &str,
    content_base64: &str,
    saved_path: Option<String>,
    description: &str,
) -> PreparedPrompt {
    let description = description.trim();
    let user_text = build_read_media_user_text(media_type, description);
    let payload = PreparedBinaryPayload {
        mime: mime.to_string(),
        content: content_base64.to_string(),
        saved_path,
        label: "图片#1".to_string(),
    };
    PreparedPrompt {
        preamble: format!(
            "[SYSTEM PROMPT]\n你是多媒体理解助手。请阅读用户提供的{}，优先完成用户要求，输出简洁、结构清楚的中文结果。",
            media_type.as_str()
        ),
        history_messages: Vec::new(),
        latest_user_text: user_text,
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_images: if matches!(media_type, ReadMediaDetectedType::Image | ReadMediaDetectedType::Video) {
            vec![payload.clone()]
        } else {
            Vec::new()
        },
        latest_audios: if matches!(media_type, ReadMediaDetectedType::Audio) {
            vec![payload]
        } else {
            Vec::new()
        },
    }
}

fn build_read_media_user_text(
    media_type: ReadMediaDetectedType,
    description: &str,
) -> String {
    let description = description.trim();
    if description.is_empty() {
        match media_type {
            ReadMediaDetectedType::Image => "请理解这张图片并输出可复用的中文分析结果。".to_string(),
            ReadMediaDetectedType::Audio => "请理解这段音频并输出可复用的中文分析结果。".to_string(),
            ReadMediaDetectedType::Video => "请理解这个视频并输出可复用的中文分析结果。".to_string(),
        }
    } else {
        format!("请按以下要求解析这份{}：{}", media_type.as_str(), description)
    }
}

fn mimo_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn openai_family_chat_completions_url(base_url: &str) -> String {
    let normalized = normalize_openai_genai_base_url(base_url);
    format!("{}chat/completions", normalized)
}

fn gemini_generate_content_url(base_url: &str, model_name: &str) -> String {
    let normalized = normalize_gemini_genai_base_url(base_url);
    format!("{normalized}models/{model_name}:generateContent")
}

fn minimax_messages_url(base_url: &str) -> String {
    let normalized = normalize_minimax_genai_base_url(base_url);
    format!("{normalized}messages")
}

fn openai_input_audio_format_from_mime_for_read_media(mime: &str) -> String {
    let normalized = mime.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "audio/wav" | "audio/wave" | "audio/x-wav" => "wav".to_string(),
        "audio/mp3" | "audio/mpeg" => "mp3".to_string(),
        _ => normalized
            .split('/')
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("wav")
            .to_string(),
    }
}

fn apply_extra_headers(
    mut request_builder: reqwest::RequestBuilder,
    headers: &[(String, String)],
) -> reqwest::RequestBuilder {
    for (key, value) in headers {
        request_builder = request_builder.header(key, value);
    }
    request_builder
}

fn read_media_http_timeout(media_type: ReadMediaDetectedType) -> std::time::Duration {
    let timeout_secs = match media_type {
        ReadMediaDetectedType::Image => READ_MEDIA_IMAGE_HTTP_TIMEOUT_SECS,
        ReadMediaDetectedType::Audio => READ_MEDIA_AUDIO_HTTP_TIMEOUT_SECS,
        ReadMediaDetectedType::Video => READ_MEDIA_VIDEO_HTTP_TIMEOUT_SECS,
    };
    std::time::Duration::from_secs(timeout_secs)
}

fn apply_read_media_timeout(
    request_builder: reqwest::RequestBuilder,
    media_type: ReadMediaDetectedType,
) -> reqwest::RequestBuilder {
    request_builder.timeout(read_media_http_timeout(media_type))
}

fn read_media_request_error(context: &str, err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "解析超时".to_string()
    } else {
        format!("{context}: {}", format_reqwest_error_diagnostics(err))
    }
}

fn read_media_response_error(context: &str, err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "解析超时".to_string()
    } else {
        format!("{context}: {err}")
    }
}

fn extract_text_from_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.trim().to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(text) => Some(text.trim().to_string()),
                serde_json::Value::Object(map) => map
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .map(|text| text.trim().to_string()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::Object(map) => map
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

fn extract_openai_family_message_text(payload: &serde_json::Value) -> String {
    payload
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .map(extract_text_from_json_value)
        .unwrap_or_default()
}

fn extract_gemini_text(payload: &serde_json::Value) -> String {
    payload
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn extract_anthropic_text(payload: &serde_json::Value) -> String {
    payload
        .get("content")
        .and_then(serde_json::Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter(|part| part.get("type").and_then(serde_json::Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn format_reqwest_error_diagnostics(err: &reqwest::Error) -> String {
    let mut parts = Vec::<String>::new();
    parts.push(err.to_string());
    if let Some(url) = err.url() {
        parts.push(format!("url={url}"));
    }
    let kind = if err.is_timeout() {
        Some("timeout")
    } else if err.is_connect() {
        Some("connect")
    } else if err.is_request() {
        Some("request")
    } else if err.is_body() {
        Some("body")
    } else if err.is_decode() {
        Some("decode")
    } else if err.is_status() {
        Some("status")
    } else if err.is_redirect() {
        Some("redirect")
    } else {
        None
    };
    if let Some(kind) = kind {
        parts.push(format!("kind={kind}"));
    }
    let mut source = std::error::Error::source(err);
    let mut source_chain = Vec::<String>::new();
    while let Some(item) = source {
        source_chain.push(item.to_string());
        source = std::error::Error::source(item);
    }
    if !source_chain.is_empty() {
        parts.push(format!("sources={}", source_chain.join(" -> ")));
    }
    parts.join(" | ")
}

async fn describe_openai_family_media_with_multimodal_api(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    media_type: ReadMediaDetectedType,
    mime: &str,
    content_base64: &str,
    description: &str,
) -> Result<String, String> {
    let request_api = resolve_request_api_config(resolved_api).await?;
    let api_key = consume_api_key_for_request(&request_api);
    let url = openai_family_chat_completions_url(&request_api.base_url);
    let user_text = build_read_media_user_text(media_type, description);
    let system_text = "[SYSTEM PROMPT]\n你是多媒体理解助手。请优先完成用户要求，输出简洁、结构清楚的中文结果。";
    let media_block = match media_type {
        ReadMediaDetectedType::Audio => serde_json::json!({
            "type": "input_audio",
            "input_audio": {
                "data": content_base64,
                "format": openai_input_audio_format_from_mime_for_read_media(mime)
            }
        }),
        ReadMediaDetectedType::Video => {
            let data_url = format!("data:{mime};base64,{content_base64}");
            if data_url.len() > OPENAI_FAMILY_VIDEO_DATA_URL_LIMIT_BYTES {
                return Err(format!(
                    "当前视频的 Base64 Data URL 超过 50MB，无法按 OpenAI 兼容视频协议发送。当前大小={} bytes，上限={} bytes。",
                    data_url.len(),
                    OPENAI_FAMILY_VIDEO_DATA_URL_LIMIT_BYTES
                ));
            }
            serde_json::json!({
                "type": "video_url",
                "video_url": {
                    "url": data_url
                },
                "fps": 2.0
            })
        }
        ReadMediaDetectedType::Image => {
            return Err("OpenAI 兼容多媒体手工适配不处理图片分支".to_string());
        }
    };
    let max_tokens = request_api
        .max_output_tokens
        .unwrap_or(selected_api.max_output_tokens);
    let body = serde_json::json!({
        "model": selected_api.model,
        "messages": [
            {
                "role": "system",
                "content": system_text
            },
            {
                "role": "user",
                "content": [
                    media_block,
                    {
                        "type": "text",
                        "text": user_text
                    }
                ]
            }
        ],
        "max_tokens": max_tokens
    });
    let resolved_protocol = resolve_model_protocol(
        request_api.request_format,
        &request_api.base_url,
        &selected_api.model,
        genai::adapter::AdapterKind::OpenAI,
    );
    let request_builder = state
        .shared_http_client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    let request_builder = apply_provider_auth_scheme(
        request_builder,
        resolved_protocol.auth_scheme,
        api_key.trim(),
    )?;
    let response = apply_read_media_timeout(
        apply_extra_headers(request_builder, &request_api.extra_headers),
        media_type,
    )
        .json(&body)
        .send()
        .await
        .map_err(|err| read_media_request_error("请求 OpenAI 兼容多媒体接口失败", &err))?;
    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        let snippet = raw.chars().take(1000).collect::<String>();
        return Err(format!(
            "OpenAI 兼容多媒体请求失败：{} | {}",
            status,
            snippet
        ));
    }
    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| read_media_response_error("解析 OpenAI 兼容多媒体响应失败", &err))?;
    let text = extract_openai_family_message_text(&payload);
    if text.is_empty() {
        return Err(format!("OpenAI 兼容多媒体响应为空：{payload}"));
    }
    Ok(text)
}

include!("read_media_qwen.rs");

async fn describe_gemini_media_with_multimodal_api(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    media_type: ReadMediaDetectedType,
    mime: &str,
    content_base64: &str,
    description: &str,
) -> Result<String, String> {
    let inline_size = content_base64.len();
    match media_type {
        ReadMediaDetectedType::Audio if inline_size > GEMINI_INLINE_AUDIO_LIMIT_BYTES => {
            return Err(format!(
                "Gemini 音频内联请求体超过 20MB，请改用更小的音频文件。当前大小={} bytes，上限={} bytes。",
                inline_size,
                GEMINI_INLINE_AUDIO_LIMIT_BYTES
            ));
        }
        ReadMediaDetectedType::Video if inline_size > GEMINI_INLINE_VIDEO_LIMIT_BYTES => {
            return Err(format!(
                "Gemini 视频内联请求体超过 100MB，请改用更小的视频文件。当前大小={} bytes，上限={} bytes。",
                inline_size,
                GEMINI_INLINE_VIDEO_LIMIT_BYTES
            ));
        }
        _ => {}
    }
    let request_api = resolve_request_api_config(resolved_api).await?;
    let api_key = consume_api_key_for_request(&request_api);
    let url = gemini_generate_content_url(&request_api.base_url, &selected_api.model);
    let user_text = build_read_media_user_text(media_type, description);
    let body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [
                {
                    "inlineData": {
                        "mimeType": mime,
                        "data": content_base64
                    }
                },
                {
                    "text": user_text
                }
            ]
        }],
        "systemInstruction": {
            "parts": [{
                "text": "你是多媒体理解助手。请优先完成用户要求，输出简洁、结构清楚的中文结果。"
            }]
        }
    });
    let api_key_header = reqwest::header::HeaderValue::from_str(api_key.trim())
        .map_err(|err| format!("构建 Gemini x-goog-api-key 请求头失败: {err}"))?;
    let request_builder = state
        .shared_http_client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-goog-api-key", api_key_header);
    let response = apply_read_media_timeout(
        apply_extra_headers(request_builder, &request_api.extra_headers),
        media_type,
    )
        .json(&body)
        .send()
        .await
        .map_err(|err| read_media_request_error("请求 Gemini 多媒体接口失败", &err))?;
    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        let snippet = raw.chars().take(1000).collect::<String>();
        return Err(format!("Gemini 多媒体请求失败：{} | {}", status, snippet));
    }
    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| read_media_response_error("解析 Gemini 多媒体响应失败", &err))?;
    let text = extract_gemini_text(&payload);
    if text.is_empty() {
        return Err(format!("Gemini 多媒体响应为空：{payload}"));
    }
    Ok(text)
}

async fn describe_minimax_video_with_multimodal_api(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    mime: &str,
    content_base64: &str,
    description: &str,
) -> Result<String, String> {
    if content_base64.len() > MINIMAX_VIDEO_BASE64_LIMIT_BYTES {
        return Err(format!(
            "MiniMax 视频 Base64 超过 50MB，无法按 Anthropic 兼容视频协议发送。当前大小={} bytes，上限={} bytes。",
            content_base64.len(),
            MINIMAX_VIDEO_BASE64_LIMIT_BYTES
        ));
    }
    let request_api = resolve_request_api_config(resolved_api).await?;
    let api_key = consume_api_key_for_request(&request_api);
    let url = minimax_messages_url(&request_api.base_url);
    let user_text = build_read_media_user_text(ReadMediaDetectedType::Video, description);
    let max_tokens = request_api
        .max_output_tokens
        .unwrap_or(selected_api.max_output_tokens);
    let body = serde_json::json!({
        "model": selected_api.model,
        "max_tokens": max_tokens,
        "system": "你是多媒体理解助手。请优先完成用户要求，输出简洁、结构清楚的中文结果。",
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "video",
                    "source": {
                        "type": "base64",
                        "media_type": mime,
                        "data": content_base64
                    }
                },
                {
                    "type": "text",
                    "text": user_text
                }
            ]
        }]
    });
    let api_key_header = reqwest::header::HeaderValue::from_str(api_key.trim())
        .map_err(|err| format!("构建 MiniMax x-api-key 请求头失败: {err}"))?;
    let request_builder = state
        .shared_http_client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-api-key", api_key_header)
        .header("anthropic-version", "2023-06-01");
    let response = apply_read_media_timeout(
        apply_extra_headers(request_builder, &request_api.extra_headers),
        ReadMediaDetectedType::Video,
    )
        .json(&body)
        .send()
        .await
        .map_err(|err| read_media_request_error("请求 MiniMax 视频理解接口失败", &err))?;
    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        let snippet = raw.chars().take(1000).collect::<String>();
        return Err(format!("MiniMax 视频理解请求失败：{} | {}", status, snippet));
    }
    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| read_media_response_error("解析 MiniMax 视频理解响应失败", &err))?;
    let text = extract_anthropic_text(&payload);
    if text.is_empty() {
        return Err(format!("MiniMax 视频理解响应为空：{payload}"));
    }
    Ok(text)
}

async fn describe_mimo_video_with_multimodal_api(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    mime: &str,
    content_base64: &str,
    description: &str,
) -> Result<String, String> {
    let data_url = format!("data:{mime};base64,{content_base64}");
    if data_url.len() > MIMO_VIDEO_BASE64_LIMIT_BYTES {
        return Err(format!(
            "当前视频的 Base64 编码结果超过 50MB，无法按 Mimo 视频协议发送。当前大小={} bytes，上限={} bytes。",
            data_url.len(),
            MIMO_VIDEO_BASE64_LIMIT_BYTES
        ));
    }
    let request_api = resolve_request_api_config(resolved_api).await?;
    let api_key = consume_api_key_for_request(&request_api);
    let url = mimo_chat_completions_url(&request_api.base_url);
    let system_text = "[SYSTEM PROMPT]\n你是多媒体理解助手。请阅读用户提供的视频，优先完成用户要求，输出简洁、结构清楚的中文结果。";
    let user_text = build_read_media_user_text(ReadMediaDetectedType::Video, description);
    let max_completion_tokens = request_api
        .max_output_tokens
        .unwrap_or(selected_api.max_output_tokens);
    let body = serde_json::json!({
        "model": selected_api.model,
        "messages": [
            {
                "role": "system",
                "content": system_text
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "video_url",
                        "video_url": {
                            "url": data_url
                        },
                        "fps": 2,
                        "media_resolution": "default"
                    },
                    {
                        "type": "text",
                        "text": user_text
                    }
                ]
            }
        ],
        "max_completion_tokens": max_completion_tokens
    });
    let resolved_protocol = resolve_model_protocol(
        request_api.request_format,
        &request_api.base_url,
        &selected_api.model,
        genai::adapter::AdapterKind::Mimo,
    );
    let request_builder = state
        .shared_http_client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json");
    let request_builder = apply_provider_auth_scheme(
        request_builder,
        resolved_protocol.auth_scheme,
        api_key.trim(),
    )?;
    let request_builder = apply_extra_headers(request_builder, &request_api.extra_headers);
    let response = apply_read_media_timeout(request_builder, ReadMediaDetectedType::Video)
        .json(&body)
        .send()
        .await
        .map_err(|err| read_media_request_error("请求 Mimo 视频理解接口失败", &err))?;
    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        let snippet = raw.chars().take(1000).collect::<String>();
        return Err(format!(
            "Mimo 视频理解请求失败：{} | {}",
            status,
            snippet
        ));
    }
    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| read_media_response_error("解析 Mimo 视频理解响应失败", &err))?;
    let text = payload
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if text.is_empty() {
        return Err(format!(
            "Mimo 视频理解响应为空：{}",
            payload
        ));
    }
    Ok(text)
}

async fn describe_media_with_multimodal_api(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    media_type: ReadMediaDetectedType,
    mime: &str,
    content_base64: &str,
    saved_path: Option<String>,
    description: &str,
) -> Result<String, String> {
    let prepared = build_read_media_prepared_prompt(
        media_type,
        mime,
        content_base64,
        saved_path,
        description,
    );
    let supports_non_stream_fallback =
        request_format_supports_non_stream_fallback(resolved_api.request_format);
    let prefer_non_stream = supports_non_stream_fallback
        && provider_streaming_disabled(
            Some(state),
            resolved_api.request_format,
            &resolved_api.base_url,
            &selected_api.model,
        );
    let reply = if resolved_api.request_format.is_genai_chat() || resolved_api.request_format.is_auto() {
        if prefer_non_stream {
            call_model_genai_non_stream(
                resolved_api,
                &selected_api.model,
                prepared,
                Some(state),
                None,
            )
            .await?
        } else {
            match call_model_genai_stream(
                resolved_api,
                &selected_api.model,
                prepared.clone(),
                Some(state),
                None,
            )
            .await
            {
                Ok(reply) => reply,
                Err(err)
                    if supports_non_stream_fallback
                        && is_streaming_request_payload_format_error(&err) =>
                {
                    call_model_genai_non_stream(
                        resolved_api,
                        &selected_api.model,
                        prepared,
                        Some(state),
                        None,
                    )
                    .await?
                }
                Err(err) => return Err(err),
            }
        }
    } else {
        return Err(format!(
            "多模态分析模型请求格式 '{}' 暂未接入 read_media。",
            resolved_api.request_format
        ));
    };
    Ok(reply.assistant_text.trim().to_string())
}

async fn builtin_read_media(
    state: &AppState,
    request: ReadMediaRequest,
    model_supports_image: bool,
) -> Result<Value, String> {
    // 路径校验（metadata）是同步文件 I/O，移入 blocking 线程池，避免阻塞 Tokio 工作线程
    let request_for_check = request.clone();
    let path = tokio::task::spawn_blocking(move || ensure_absolute_media_path(&request_for_check))
        .await
        .map_err(|err| format!("read_media 工具路径校验后台执行失败：{err}"))??;
    let detected = detect_read_media_type(&path).ok_or_else(|| "read_media 仅支持图片、音频或视频文件".to_string())?;
    let description_is_empty = request
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty();
    // 当前对话模型本身支持图片输入，且未指定解析侧重时，直接返回原图 base64 交给模型自行观察，
    // 不再调用独立多模态模型生成文字描述。音频、视频不适用，仍走下面的多模态解析链路。
    if model_supports_image && detected == ReadMediaDetectedType::Image && description_is_empty {
        let raw = tokio::fs::read(&path)
            .await
            .map_err(|err| format!("读取媒体文件失败: {err}"))?;
        let mime = media_mime_from_path(&path)
            .unwrap_or("image/png")
            .to_string();
        let content_base64 = B64.encode(&raw);
        runtime_log_debug(format!(
            "[read_media] 直接返回原图：模型支持图片输入且未指定解析侧重，mime={}，字节数={}",
            mime,
            raw.len()
        ));
        return Ok(serde_json::json!({
            "ok": true,
            "mediaType": detected.as_str(),
            "path": path.to_string_lossy().to_string(),
            "text": "已直接返回原图（未生成文字描述），请查看图片内容。",
            "imageMime": mime,
            "imageBase64": content_base64,
            "directImage": true
        }));
    }
    let app_config = state_read_config_cached(state)?;
    let selected_api = resolve_vision_api_config(&app_config)?;
    match detected {
        ReadMediaDetectedType::Image if !selected_api.enable_image => {
            return Err("当前多模态模型未启用图片输入".to_string());
        }
        ReadMediaDetectedType::Audio if !selected_api.enable_audio => {
            runtime_log_debug(format!(
                "[read_media] 跳过，媒体类型=音频，原因=模型未启用音频输入，api_id={}，api_name={}，api_url={}，模型={}",
                selected_api.id, selected_api.name, selected_api.base_url, selected_api.model
            ));
            return Err("当前多模态模型未启用音频输入".to_string());
        }
        ReadMediaDetectedType::Video if !selected_api.enable_video => {
            return Err("当前多模态模型未启用视频输入".to_string());
        }
        _ => {}
    }
    let mut resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
    let configured_request_format = resolved_api.request_format;
    let use_opencode_qwen_compat = should_use_opencode_qwen_read_media_compat(
        &resolved_api.base_url,
        &selected_api.model,
    );
    if use_opencode_qwen_compat {
        resolved_api.request_format = RequestFormat::Anthropic;
    }
    let route_family = resolve_read_media_route_family(
        configured_request_format,
        &resolved_api.base_url,
        &selected_api.model,
    );
    let resolved_protocol = resolve_model_protocol(
        resolved_api.request_format,
        &resolved_api.base_url,
        &selected_api.model,
        genai::adapter::AdapterKind::OpenAI,
    );
    runtime_log_debug(format!(
        "[read_media 路由] configured_request_format={:?}，effective_request_format={:?}，base_url={}，model={}，adapter={:?}，adapter_source={:?}，route_family={}",
        configured_request_format,
        resolved_api.request_format,
        resolved_api.base_url,
        selected_api.model,
        resolved_protocol.adapter_kind,
        resolved_protocol.source,
        route_family.as_str(),
    ));
    if detected == ReadMediaDetectedType::Audio && use_opencode_qwen_compat {
        return Err(
            "OpenCode Go 的 Qwen 模型官方使用 Anthropic /messages 端点，该端点不支持 audio 模态；请改用支持音频输入的 Qwen-Omni/Audio 直连配置。"
                .to_string(),
        );
    }
    let raw = tokio::fs::read(&path)
        .await
        .map_err(|err| format!("读取媒体文件失败: {err}"))?;
    let mime = media_mime_from_path(&path)
        .unwrap_or(match detected {
            ReadMediaDetectedType::Image => "image/png",
            ReadMediaDetectedType::Audio => "audio/mpeg",
            ReadMediaDetectedType::Video => "video/mp4",
        })
        .to_string();
    let content_base64 = B64.encode(&raw);
    let description = request.description.unwrap_or_default().trim().to_string();
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&raw);
    let hash = bytes_to_lower_hex(hasher.finalize());
    if let Some(cached) =
        state_service_find_image_text_cache(state, &hash, &selected_api.id, detected.as_str(), &description)?
    {
        return Ok(serde_json::json!({
            "ok": true,
            "mediaType": detected.as_str(),
            "modelId": selected_api.id,
            "path": path.to_string_lossy().to_string(),
            "description": description,
            "text": cached,
            "cached": true
        }));
    }
    let text = match detected {
        ReadMediaDetectedType::Image => {
            match route_family {
                ReadMediaRouteFamily::Qwen => {
                    describe_qwen_media_with_multimodal_api(
                        state,
                        &resolved_api,
                        &selected_api,
                        detected,
                        &mime,
                        &content_base64,
                        &description,
                    )
                    .await?
                }
                _ => describe_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    Some(path.to_string_lossy().to_string()),
                    &description,
                )
                .await?,
            }
        }
        ReadMediaDetectedType::Audio => match route_family {
            ReadMediaRouteFamily::Qwen => {
                describe_qwen_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::OpenAI => {
                describe_openai_family_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::Gemini => {
                describe_gemini_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::Mimo => {
                describe_openai_family_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::Anthropic | ReadMediaRouteFamily::MiniMax => {
                return Err("当前 Anthropic 兼容多模态链路暂不支持音频解析，请改用支持音频的 OpenAI 或 Gemini 多模态模型。".to_string());
            }
        },
        ReadMediaDetectedType::Video => match route_family {
            ReadMediaRouteFamily::Qwen => {
                describe_qwen_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::Mimo => {
                describe_mimo_video_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::OpenAI => {
                describe_openai_family_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::Gemini => {
                describe_gemini_media_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    detected,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::MiniMax => {
                describe_minimax_video_with_multimodal_api(
                    state,
                    &resolved_api,
                    &selected_api,
                    &mime,
                    &content_base64,
                    &description,
                )
                .await?
            }
            ReadMediaRouteFamily::Anthropic => {
                return Err("当前标准 Anthropic 多模态协议不支持直接解析视频；仅 MiniMax 的 Anthropic 兼容扩展支持该视频载荷。".to_string());
            }
        },
    };
    if text.is_empty() {
        return Err("多模态分析模型返回了空结果".to_string());
    }
    state_service_upsert_image_text_cache(
        state,
        &hash,
        &selected_api.id,
        detected.as_str(),
        &description,
        &text,
    )?;
    Ok(serde_json::json!({
        "ok": true,
        "mediaType": detected.as_str(),
        "modelId": selected_api.id,
        "path": path.to_string_lossy().to_string(),
        "description": description,
        "text": text,
        "cached": false
    }))
}

struct TextFileReader;

impl ReadFileReader for TextFileReader {
    fn reader_kind(&self) -> &'static str {
        "text"
    }

    fn supports(&self, detected: ReadFileDetectedType) -> bool {
        matches!(detected, ReadFileDetectedType::Text)
    }

    fn read(
        &self,
        _state: &AppState,
        _session_id: &str,
        _api_config_id: &str,
        request: &ReadFileRequest,
        detected: ReadFileDetectedType,
    ) -> Result<Value, String> {
        let path = ensure_absolute_file_path(request)?;
        let decoded = decode_text_file_from_path(&path)
            .map_err(|err| format!("读取文本文件失败：{err}"))?;
        Ok(build_text_read_result(
            &path,
            detected,
            self.reader_kind(),
            &decoded.text,
            request.offset,
            request.limit,
            serde_json::json!({}),
        ))
    }
}

struct PdfFileReader;

impl ReadFileReader for PdfFileReader {
    fn reader_kind(&self) -> &'static str {
        "pdf_builtin"
    }

    fn supports(&self, detected: ReadFileDetectedType) -> bool {
        matches!(detected, ReadFileDetectedType::Pdf)
    }

    fn read(
        &self,
        state: &AppState,
        session_id: &str,
        api_config_id: &str,
        request: &ReadFileRequest,
        detected: ReadFileDetectedType,
    ) -> Result<Value, String> {
        let path = ensure_absolute_file_path(request)?;
        let conversation_id = read_file_conversation_cache_key(session_id);
        let include_images = resolve_pdf_image_mode(state, api_config_id)?;
        let structured = match get_or_extract_pdf_structured(
            state,
            &conversation_id,
            &path.to_string_lossy(),
            include_images,
        ) {
            Ok(value) => value,
            Err(err) if include_images && !is_pdf_page_limit_exceeded_error(&err) => {
                runtime_log_warn(format!(
                    "[read] PDF 页图提取失败，降级为文本读取，file={}，err={}",
                    path.display(),
                    err
                ));
                let mut fallback = get_or_extract_pdf_structured(
                    state,
                    &conversation_id,
                    &path.to_string_lossy(),
                    false,
                )?;
                if let Some(first_page) = fallback.pages.first_mut() {
                    if first_page.text.trim().is_empty() {
                        first_page.text = format!(
                            "[系统提示] PDF 页图未能成功提供给模型，已自动回退为文本读取。\n原因：{}",
                            err.trim()
                        );
                    } else {
                        first_page.text = format!(
                            "[系统提示] PDF 页图未能成功提供给模型，已自动回退为文本读取。\n原因：{}\n\n{}",
                            err.trim(),
                            first_page.text
                        );
                    }
                } else {
                    fallback.pages.push(PdfPageExtractBlock {
                        page_index: 0,
                        text: format!(
                            "[系统提示] PDF 页图未能成功提供给模型，已自动回退为文本读取。\n原因：{}",
                            err.trim()
                        ),
                        images: Vec::new(),
                    });
                }
                fallback
            }
            Err(err) => return Err(err),
        };
        if include_images {
            return Ok(build_pdf_image_read_result(
                &path,
                detected,
                &structured,
                request.offset,
                request.limit,
            ));
        }
        let text = structured
            .pages
            .iter()
            .map(|page| format!("[第 {} 页]\n{}", page.page_index + 1, page.text))
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(build_text_read_result(
            &path,
            detected,
            self.reader_kind(),
            &text,
            request.offset,
            request.limit,
            serde_json::json!({
                "totalPages": structured.total_pages,
                "includeImages": structured.include_images
            }),
        ))
    }
}

struct OfficeLitchiReader;

impl ReadFileReader for OfficeLitchiReader {
    fn reader_kind(&self) -> &'static str {
        "litchi"
    }

    fn supports(&self, detected: ReadFileDetectedType) -> bool {
        matches!(
            detected,
            ReadFileDetectedType::Doc
                | ReadFileDetectedType::Docx
                | ReadFileDetectedType::Xls
                | ReadFileDetectedType::Xlsx
                | ReadFileDetectedType::Xlsb
                | ReadFileDetectedType::Ppt
                | ReadFileDetectedType::Pptx
                | ReadFileDetectedType::Ods
                | ReadFileDetectedType::Odp
                | ReadFileDetectedType::Rtf
                | ReadFileDetectedType::Numbers
                | ReadFileDetectedType::Pages
                | ReadFileDetectedType::Keynote
        )
    }

    fn read(
        &self,
        _state: &AppState,
        _session_id: &str,
        _api_config_id: &str,
        request: &ReadFileRequest,
        detected: ReadFileDetectedType,
    ) -> Result<Value, String> {
        let path = ensure_absolute_file_path(request)?;
        let path_for_read = path.clone();
        let detected_for_read = detected;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || -> Result<String, String> {
            match detected_for_read {
                ReadFileDetectedType::Doc
                | ReadFileDetectedType::Docx
                | ReadFileDetectedType::Rtf
                | ReadFileDetectedType::Pages => {
                    let document = litchi::Document::open(&path_for_read)
                        .map_err(|err| format!("litchi 打开文档失败: {err}"))?;
                    document
                        .text()
                        .map_err(|err| format!("litchi 提取文档文本失败: {err}"))
                }
                ReadFileDetectedType::Ppt
                | ReadFileDetectedType::Pptx
                | ReadFileDetectedType::Odp
                | ReadFileDetectedType::Keynote => {
                    let presentation = litchi::Presentation::open(&path_for_read)
                        .map_err(|err| format!("litchi 打开演示文稿失败: {err}"))?;
                    presentation
                        .text()
                        .map_err(|err| format!("litchi 提取演示文稿文本失败: {err}"))
                }
                ReadFileDetectedType::Xls
                | ReadFileDetectedType::Xlsx
                | ReadFileDetectedType::Xlsb
                | ReadFileDetectedType::Ods
                | ReadFileDetectedType::Numbers => {
                    let workbook = litchi::sheet::Workbook::open(&path_for_read)
                        .map_err(|err| format!("litchi 打开表格失败: {err}"))?;
                    workbook
                        .text()
                        .map_err(|err| format!("litchi 提取表格文本失败: {err}"))
                }
                _ => Err("当前 Office 类型尚未接入 litchi reader".to_string()),
            }
        }));
        let text = match caught {
            Ok(result) => result?,
            Err(_) => {
                return Err(format!(
                    "实验性 Office reader 解析失败并触发 panic：{}",
                    path.display()
                ))
            }
        };
        let text = normalize_office_text_for_read_file(&text);
        Ok(build_text_read_result(
            &path,
            detected,
            self.reader_kind(),
            &text,
            request.offset,
            request.limit,
            serde_json::json!({
                "experimental": true
            }),
        ))
    }
}

async fn builtin_read_file(
    state: &AppState,
    session_id: &str,
    api_config_id: &str,
    request: ReadFileRequest,
) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let ui_language = state_read_config_cached(&state)
        .map(|config| config.ui_language)
        .unwrap_or_else(|_| "zh-CN".to_string());
    // 路径校验（metadata）是同步文件 I/O，可能被 TCC 拒绝、慢盘或网络卷拖慢；
    // 移入 blocking 线程池，避免阻塞 Tokio 工作线程。
    let request_for_check = request.clone();
    let (path, detected) = match tokio::task::spawn_blocking(move || {
        let path = ensure_absolute_file_path(&request_for_check)?;
        let detected = detect_read_file_type(&path);
        Ok::<_, String>((path, detected))
    })
    .await
    .map_err(|err| format!("read 工具路径校验后台执行失败：{err}"))?
    {
        Ok(result) => result,
        Err(err) => {
            // 前置校验（metadata）也可能被 TCC 拦截，失败时同样附加授权建议
            let hint_path = std::path::Path::new(request.path.trim());
            return match macos_tcc_permission_hint(&ui_language, &err, Some(hint_path)) {
                Some(hint) => Err(format!("{err}\n\n{hint}")),
                None => Err(err),
            };
        }
    };
    runtime_log_info(format!(
        "[read] 开始，任务=read，session_id={}，api_config_id={}，{}，detected_type={}",
        session_id,
        api_config_id,
        read_file_log_target(&path),
        detected.as_str()
    ));
    if matches!(detected, ReadFileDetectedType::Unknown) {
        return Err(format!(
            "暂不支持该文件类型：{}",
            path.extension().and_then(|v| v.to_str()).unwrap_or_default()
        ));
    }
    if matches!(detected, ReadFileDetectedType::Image) {
        return Ok(build_text_read_result(
            &path,
            detected,
            "media_redirect_notice",
            "该文件被识别为图片。`read_file` 现在只负责文本、PDF 和 Office 等文档读取；请改用 `read_media` 解析图片、音频或视频。",
            request.offset,
            request.limit,
            serde_json::json!({
                "redirectTool": READ_MEDIA_TOOL_NAME
            }),
        ));
    }
    // 文件读取与 PDF 渲染是同步 IO + CPU 密集操作，移到 blocking 线程池，
    // 避免每次 read 工具调用占住 tokio 工作线程阻塞其他并发任务。
    let state = state.clone();
    let session_id_owned = session_id.to_string();
    let api_config_id_owned = api_config_id.to_string();
    let (reader_kind, result) = tokio::task::spawn_blocking(
        move || -> Result<(String, Result<Value, String>), String> {
            let readers: [&dyn ReadFileReader; 3] = [
                &TextFileReader,
                &PdfFileReader,
                &OfficeLitchiReader,
            ];
            let reader = readers
                .into_iter()
                .find(|item| item.supports(detected))
                .ok_or_else(|| format!("未找到可用读取器：{}", detected.as_str()))?;
            let kind = reader.reader_kind().to_string();
            let result = reader.read(
                &state,
                &session_id_owned,
                &api_config_id_owned,
                &request,
                detected,
            );
            Ok((kind, result))
        },
    )
    .await
    .map_err(|err| format!("read 工具后台执行失败：{err}"))??;
    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    match &result {
        Ok(_) => runtime_log_info(format!(
            "[read] 完成，任务=read，session_id={}，api_config_id={}，reader={}，detected_type={}，elapsed_ms={}",
            session_id,
            api_config_id,
            reader_kind,
            detected.as_str(),
            elapsed_ms
        )),
        Err(err) => runtime_log_error(format!(
            "[read] 失败，任务=read，session_id={}，api_config_id={}，reader={}，detected_type={}，elapsed_ms={}，error={}",
            session_id,
            api_config_id,
            reader_kind,
            detected.as_str(),
            elapsed_ms,
            err
        )),
    }
    let result = match result {
        Ok(value) => Ok(value),
        Err(err) => {
            match macos_tcc_permission_hint(&ui_language, &err, Some(&path)) {
                Some(hint) => Err(format!("{err}\n\n{hint}")),
                None => Err(err),
            }
        }
    };
    result
}

#[cfg(test)]
fn test_read_file_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-read-file-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("config_mark"),
            llm_workspace_path: root.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

#[cfg(test)]
#[test]
fn read_file_request_should_accept_new_and_legacy_argument_names() {
        let current: ReadFileRequest = serde_json::from_str(
            r#"{"path":"E:\\docs\\a.md","offset":2,"limit":5}"#,
        )
        .expect("parse current read args");
        assert_eq!(current.path, "E:\\docs\\a.md");
        assert_eq!(current.offset, Some(2));
        assert_eq!(current.limit, Some(5));

        let legacy: ReadFileRequest = serde_json::from_str(
            r#"{"absolute_path":"E:\\docs\\b.md","start":3,"count":7}"#,
        )
        .expect("parse legacy read_file args");
        assert_eq!(legacy.path, "E:\\docs\\b.md");
        assert_eq!(legacy.offset, Some(3));
        assert_eq!(legacy.limit, Some(7));
}

#[cfg(test)]
#[test]
fn read_file_conversation_cache_key_should_strip_remote_reply_delegate_runtime_tag() {
        let session_id = "agent-a::conversation-sub::remote_reply_delegate:delegate-a";

        assert_eq!(
            read_file_conversation_cache_key(session_id),
            "conversation-sub"
        );
}

#[cfg(test)]
#[test]
fn detect_read_file_type_should_classify_common_formats() {
        assert_eq!(
            detect_read_file_type(std::path::Path::new("a.txt")),
            ReadFileDetectedType::Text
        );
        assert_eq!(
            detect_read_file_type(std::path::Path::new("a.svg")),
            ReadFileDetectedType::Text
        );
        assert_eq!(
            detect_read_file_type(std::path::Path::new("a.pdf")),
            ReadFileDetectedType::Pdf
        );
        assert_eq!(
            detect_read_file_type(std::path::Path::new("a.doc")),
            ReadFileDetectedType::Doc
        );
        assert_eq!(
            detect_read_file_type(std::path::Path::new("a.xlsx")),
            ReadFileDetectedType::Xlsx
        );
        assert_eq!(
            detect_read_file_type(std::path::Path::new("a.ppt")),
            ReadFileDetectedType::Ppt
        );
    }

#[cfg(test)]
#[test]
fn detect_read_media_type_should_classify_common_formats() {
        assert_eq!(
            detect_read_media_type(std::path::Path::new("a.png")),
            Some(ReadMediaDetectedType::Image)
        );
        assert_eq!(
            detect_read_media_type(std::path::Path::new("a.mp3")),
            Some(ReadMediaDetectedType::Audio)
        );
        assert_eq!(
            detect_read_media_type(std::path::Path::new("a.mp4")),
            Some(ReadMediaDetectedType::Video)
        );
        assert_eq!(detect_read_media_type(std::path::Path::new("a.txt")), None);
}

#[cfg(test)]
#[test]
fn read_media_timeout_should_follow_detected_type() {
        assert_eq!(
            read_media_http_timeout(ReadMediaDetectedType::Image),
            std::time::Duration::from_secs(READ_MEDIA_IMAGE_HTTP_TIMEOUT_SECS)
        );
        assert_eq!(
            read_media_http_timeout(ReadMediaDetectedType::Audio),
            std::time::Duration::from_secs(READ_MEDIA_AUDIO_HTTP_TIMEOUT_SECS)
        );
        assert_eq!(
            read_media_http_timeout(ReadMediaDetectedType::Video),
            std::time::Duration::from_secs(READ_MEDIA_VIDEO_HTTP_TIMEOUT_SECS)
        );
}

#[cfg(test)]
#[test]
fn resolve_read_media_route_family_should_follow_request_format_or_auto_fallback() {
        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Gemini,
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini-2.5-pro",
            ),
            ReadMediaRouteFamily::Gemini
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::OpenAI,
                "https://example.com/v1",
                "mimo-v2.5",
            ),
            ReadMediaRouteFamily::OpenAI
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Anthropic,
                "https://api.anthropic.com/v1",
                "MiniMax-M3",
            ),
            ReadMediaRouteFamily::Anthropic
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::MiniMax,
                "https://api.minimax.io/anthropic/v1",
                "MiniMax-M3",
            ),
            ReadMediaRouteFamily::MiniMax
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Auto,
                "https://opencode.ai/zen/go/v1",
                "qwen3.7-plus",
            ),
            ReadMediaRouteFamily::Anthropic
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Auto,
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "qwen3.7-plus",
            ),
            ReadMediaRouteFamily::Qwen
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Auto,
                "https://dashscope.aliyuncs.com/apps/anthropic",
                "qwen3.7-plus",
            ),
            ReadMediaRouteFamily::Anthropic
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Aliyun,
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "qwen3.7-plus",
            ),
            ReadMediaRouteFamily::Qwen
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::OpenCodeGo,
                "https://opencode.ai/zen/go/v1",
                "qwen3.7-plus",
            ),
            ReadMediaRouteFamily::Anthropic
        );

        assert_eq!(
            resolve_read_media_route_family(
                RequestFormat::Mimo,
                "https://api.xiaomimimo.com/v1",
                "mimo-v2.5",
            ),
            ReadMediaRouteFamily::Mimo
        );
}

#[cfg(test)]
#[test]
fn build_qwen_media_block_should_use_qwen_media_payloads_for_all_media_types() {
    let image = build_qwen_media_block(ReadMediaDetectedType::Image, "image/png", "AAAA")
        .expect("build qwen image block");
    assert_eq!(
        image.pointer("/image_url/url").and_then(Value::as_str),
        Some("data:image/png;base64,AAAA")
    );

    let audio = build_qwen_media_block(ReadMediaDetectedType::Audio, "audio/wav", "BBBB")
        .expect("build qwen audio block");
    assert_eq!(
        audio.pointer("/input_audio/data").and_then(Value::as_str),
        Some("BBBB")
    );
    assert_eq!(
        audio.pointer("/input_audio/format").and_then(Value::as_str),
        Some("wav")
    );

    let video = build_qwen_media_block(ReadMediaDetectedType::Video, "video/mp4", "CCCC")
        .expect("build qwen video block");
    assert_eq!(
        video.pointer("/video_url/url").and_then(Value::as_str),
        Some("data:video/mp4;base64,CCCC")
    );
}

#[cfg(test)]
#[test]
fn image_mime_from_bytes_should_detect_common_images_without_extension() {
        assert_eq!(
            image_mime_from_bytes(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            Some("image/png")
        );
        assert_eq!(
            image_mime_from_bytes(&[0xff, 0xd8, 0xff, 0xe0, 0, 0x10, b'J', b'F', b'I', b'F', 0]),
            Some("image/jpeg")
        );
        assert_eq!(image_mime_from_bytes(b"hello world"), None);
    }

#[cfg(test)]
#[test]
fn normalize_office_text_should_drop_control_chars() {
        let input = "a\u{0001}b\r\n\r\nc\t\u{0004}d";
        let output = normalize_office_text_for_read_file(input);
        assert_eq!(output, "ab\nc\td");
    }

#[cfg(test)]
#[test]
fn build_text_read_result_should_normalize_crlf_to_lf_and_report_source_line_ending() {
        let path = std::path::Path::new("sample.txt");
        let value = build_text_read_result(
            path,
            ReadFileDetectedType::Text,
            "text",
            "line1\r\nline2\r\n",
            None,
            None,
            serde_json::json!({}),
        );
        assert_eq!(
            value.get("content").and_then(Value::as_str),
            Some("line1\nline2\n")
        );
        let metadata = value.get("metadata").expect("metadata");
        assert_eq!(
            metadata.get("sourceLineEnding").and_then(Value::as_str),
            Some("crlf")
        );
        assert_eq!(
            metadata.get("contentLineEnding").and_then(Value::as_str),
            Some("lf")
        );
    }

#[cfg(test)]
#[test]
fn build_text_read_result_should_normalize_lone_cr_to_lf() {
        let path = std::path::Path::new("sample.txt");
        let value = build_text_read_result(
            path,
            ReadFileDetectedType::Text,
            "text",
            "line1\rline2\r",
            None,
            None,
            serde_json::json!({}),
        );
        assert_eq!(
            value.get("content").and_then(Value::as_str),
            Some("line1\nline2\n")
        );
        let metadata = value.get("metadata").expect("metadata");
        assert_eq!(
            metadata.get("sourceLineEnding").and_then(Value::as_str),
            Some("cr")
        );
    }

#[cfg(test)]
#[test]
fn builtin_read_file_should_paginate_text_file() {
        let root = std::env::temp_dir().join(format!("eca-read-file-page-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("sample.txt");
        std::fs::write(&file, "line1\nline2\nline3\nline4\n").expect("write sample text");
        let state = test_read_file_state();
        let value = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(builtin_read_file(
            &state,
            "chat::conv-1",
            "__frontend_tool_preview__",
            ReadFileRequest {
                path: file.to_string_lossy().to_string(),
                offset: Some(1),
                limit: Some(2),
            },
        ))
        .expect("read text");
        assert_eq!(value.get("detectedType").and_then(Value::as_str), Some("text"));
        assert_eq!(
            value.get("content").and_then(Value::as_str),
            Some("line2\nline3")
        );
        assert_eq!(value.get("nextOffset").and_then(Value::as_u64), Some(3));
}

#[cfg(test)]
#[test]
fn builtin_read_file_should_decode_gbk_text_file() {
        let root = std::env::temp_dir().join(format!("eca-read-file-gbk-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("sample.txt");
        std::fs::write(&file, [0xd6, 0xd0, 0xce, 0xc4, b'\n']).expect("write gbk text");
        let state = test_read_file_state();
        let value = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(builtin_read_file(
            &state,
            "chat::conv-1",
            "__frontend_tool_preview__",
            ReadFileRequest {
                path: file.to_string_lossy().to_string(),
                offset: None,
                limit: None,
            },
        ))
        .expect("read gbk text");
        assert_eq!(value.get("content").and_then(Value::as_str), Some("中文\n"));
}

#[cfg(test)]
#[test]
fn builtin_read_file_should_redirect_image_input_to_read_media() {
        let root = std::env::temp_dir().join(format!("eca-read-file-image-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("sample.png");
        std::fs::write(&file, b"fake-png").expect("write sample image");
        let state = test_read_file_state();
        let config = AppConfig {
            selected_api_config_id: "vision-a".to_string(),
            expert_api_config_id: "vision-a".to_string(),
            api_configs: vec![ApiConfig {
                id: "vision-a".to_string(),
                name: "vision-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: true,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://example.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                model: "gpt-image".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 0.7,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let value = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
            .block_on(async {
                builtin_read_file(
                    &state,
                    "assistant::conversation-a",
                    "vision-a",
                    ReadFileRequest {
                        path: file.to_string_lossy().to_string(),
                        offset: None,
                        limit: None,
                    },
                )
                .await
            })
            .expect("read image");

        assert_eq!(
            value.get("readerKind").and_then(Value::as_str),
            Some("media_redirect_notice")
        );
        assert_eq!(
            value.get("metadata")
                .and_then(|item| item.get("extra"))
                .and_then(|item| item.get("redirectTool"))
                .and_then(Value::as_str),
            Some(READ_MEDIA_TOOL_NAME)
        );
        assert!(
            value.get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("请改用 `read_media`")
        );
    }

#[cfg(test)]
#[test]
fn builtin_read_media_should_reject_non_media_file() {
        let root = std::env::temp_dir().join(format!("eca-read-media-text-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("sample.txt");
        std::fs::write(&file, b"hello").expect("write text");
        let state = test_read_file_state();
        let err = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(builtin_read_media(
            &state,
            ReadMediaRequest {
                path: file.to_string_lossy().to_string(),
                description: None,
            },
            false,
        ))
        .expect_err("reject non-media");

        assert!(err.contains("read_media 仅支持图片、音频或视频文件"));
    }

#[cfg(test)]
#[test]
fn builtin_read_media_should_fail_when_audio_capability_is_disabled() {
        let root = std::env::temp_dir().join(format!("eca-read-media-audio-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("sample.mp3");
        std::fs::write(&file, b"fake-mp3").expect("write audio");
        let state = test_read_file_state();
        let config = AppConfig {
            selected_api_config_id: "vision-a".to_string(),
            expert_api_config_id: "vision-a".to_string(),
            vision_api_config_id: Some("vision-a".to_string()),
            api_configs: vec![ApiConfig {
                id: "vision-a".to_string(),
                name: "vision-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: true,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://example.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                model: "gpt-image".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 0.7,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let err = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(builtin_read_media(
            &state,
            ReadMediaRequest {
                path: file.to_string_lossy().to_string(),
                description: Some("只关注语音内容".to_string()),
            },
            false,
        ))
        .expect_err("audio capability should be rejected");

        assert_eq!(err, "当前多模态模型未启用音频输入");
    }

#[cfg(test)]
#[test]
fn builtin_read_media_should_return_original_image_when_model_supports_image_and_description_empty() {
        let root = std::env::temp_dir().join(format!("eca-read-media-direct-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("sample.png");
        std::fs::write(&file, b"\x89PNG\r\n\x1a\nfake-image-bytes").expect("write image");
        let state = test_read_file_state();
        let value = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(builtin_read_media(
            &state,
            ReadMediaRequest {
                path: file.to_string_lossy().to_string(),
                description: None,
            },
            true,
        ))
        .expect("direct image should succeed");

        assert_eq!(value.get("mediaType").and_then(Value::as_str), Some("image"));
        assert_eq!(value.get("directImage").and_then(Value::as_bool), Some(true));
        assert_eq!(value.get("imageMime").and_then(Value::as_str), Some("image/png"));
        assert!(value
            .get("imageBase64")
            .and_then(Value::as_str)
            .map(|text| !text.is_empty())
            .unwrap_or(false));
        // 直返路径不经过描述缓存，也不产生文字描述
        assert!(value.get("cached").is_none());
        assert!(value.get("text").and_then(Value::as_str).is_some());
    }

#[cfg(test)]
#[test]
fn builtin_read_media_should_not_return_original_audio_or_video_when_description_empty() {
        // 模型支持图片，但音频、视频不适用直返路径；两者能力关闭时即便 description 留空也应报错，
        // 证明它们没有走「返回原媒体」的分支。
        let root = std::env::temp_dir().join(format!("eca-read-media-av-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let audio = root.join("sample.mp3");
        std::fs::write(&audio, b"fake-mp3").expect("write audio");
        let video = root.join("sample.mp4");
        std::fs::write(&video, b"fake-mp4").expect("write video");
        let state = test_read_file_state();
        let config = AppConfig {
            selected_api_config_id: "vision-a".to_string(),
            assistant_department_api_config_id: "vision-a".to_string(),
            vision_api_config_id: Some("vision-a".to_string()),
            api_configs: vec![ApiConfig {
                id: "vision-a".to_string(),
                name: "vision-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: true,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://example.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                model: "gpt-image".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 0.7,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        for (file, expected) in [
            (&audio, "当前多模态模型未启用音频输入"),
            (&video, "当前多模态模型未启用视频输入"),
        ] {
            let err = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime")
                .block_on(builtin_read_media(
                    &state,
                    ReadMediaRequest {
                        path: file.to_string_lossy().to_string(),
                        description: None,
                    },
                    true,
                ))
                .expect_err("audio/video should not be returned as original media");

            assert_eq!(err, expected);
        }
    }

#[cfg(test)]
#[test]
fn builtin_read_file_should_prefix_truncation_notice_only_when_truncated() {
        let root = std::env::temp_dir().join(format!("eca-read-file-trunc-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let file = root.join("big.txt");
        let long_line = "a".repeat(31_000);
        std::fs::write(&file, long_line).expect("write big text");
        let state = test_read_file_state();
        let value = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
        .block_on(builtin_read_file(
            &state,
            "chat::conv-1",
            "__frontend_tool_preview__",
            ReadFileRequest {
                path: file.to_string_lossy().to_string(),
                offset: None,
                limit: None,
            },
        ))
        .expect("read truncated text");
        let text = value.get("content").and_then(Value::as_str).unwrap_or_default();
        assert!(text.starts_with("Content was truncated to fit within 30000 character limit.\nTo continue reading, use offset="));
    }

#[cfg(test)]
#[test]
fn build_pdf_image_read_result_should_paginate_by_page_start() {
        let path = std::path::PathBuf::from("E:\\docs\\sample.pdf");
        let structured = PdfExtractStructuredResult {
            file_name: "sample.pdf".to_string(),
            total_pages: 3,
            include_images: true,
            pages: vec![
                PdfPageExtractBlock {
                    page_index: 0,
                    text: String::new(),
                    images: vec![PdfRenderedImage {
                        page_index: 0,
                        width: 10,
                        height: 20,
                        bytes_base64: "img0".to_string(),
                        mime: "image/webp".to_string(),
                    }],
                },
                PdfPageExtractBlock {
                    page_index: 1,
                    text: String::new(),
                    images: vec![PdfRenderedImage {
                        page_index: 1,
                        width: 11,
                        height: 21,
                        bytes_base64: "img1".to_string(),
                        mime: "image/webp".to_string(),
                    }],
                },
                PdfPageExtractBlock {
                    page_index: 2,
                    text: String::new(),
                    images: vec![PdfRenderedImage {
                        page_index: 2,
                        width: 12,
                        height: 22,
                        bytes_base64: "img2".to_string(),
                        mime: "image/webp".to_string(),
                    }],
                },
            ],
        };

        let value = build_pdf_image_read_result(
            &path,
            ReadFileDetectedType::Pdf,
            &structured,
            Some(1),
            Some(1),
        );

        assert_eq!(value.get("readerKind").and_then(Value::as_str), Some("pdf_image_direct"));
        assert_eq!(value.get("nextOffset").and_then(Value::as_u64), Some(2));
        let parts = value.get("parts").and_then(Value::as_array).expect("parts");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].get("pageIndex").and_then(Value::as_u64), Some(1));
}
