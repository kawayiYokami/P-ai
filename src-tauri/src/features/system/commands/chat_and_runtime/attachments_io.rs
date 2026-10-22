#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SttTranscribeInput {
    mime: String,
    bytes_base64: String,
    #[serde(default)]
    stt_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SttTranscribeOutput {
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalBinaryFileInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalBinaryFileOutput {
    mime: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueLocalFileAttachmentInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueInlineFileAttachmentInput {
    file_name: String,
    #[serde(default)]
    mime: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueLocalFileAttachmentOutput {
    mime: String,
    file_name: String,
    saved_path: String,
    attach_as_media: bool,
    #[serde(default)]
    bytes_base64: Option<String>,
    text_notice: String,
}

fn media_mime_from_path(path: &std::path::Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => Some("application/pdf"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "heic" => Some("image/heic"),
        "heif" => Some("image/heif"),
        "svg" => Some("image/svg+xml"),
        "wav" | "wave" => Some("audio/wav"),
        "mp3" => Some("audio/mpeg"),
        "m4a" => Some("audio/mp4"),
        "aac" => Some("audio/aac"),
        "aiff" | "aif" => Some("audio/aiff"),
        "ogg" | "oga" => Some("audio/ogg"),
        "opus" => Some("audio/opus"),
        "flac" => Some("audio/flac"),
        "webm" => Some("audio/webm"),
        _ => None,
    }
}

fn image_mime_from_bytes(raw: &[u8]) -> Option<&'static str> {
    infer::get(raw)
        .map(|kind| kind.mime_type())
        .filter(|mime| mime.starts_with("image/"))
}

fn workspace_downloads_dir(state: &AppState) -> PathBuf {
    // downloads 是用户与 LLM 共用的附件落地区；允许 LLM 后续自行清理和管理空间占用。
    configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone())
        .join("downloads")
}

fn media_extension_from_mime_for_download(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/heic" => "heic",
        "image/heif" => "heif",
        "image/svg+xml" => "svg",
        "audio/wav" => "wav",
        "audio/x-wav" => "wav",
        "audio/mpeg" => "mp3",
        "audio/mp3" => "mp3",
        "audio/mp4" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "audio/flac" => "flac",
        _ => "bin",
    }
}

fn is_dangerous_executable_extension(ext: &str) -> bool {
    matches!(
        ext.trim().to_ascii_lowercase().as_str(),
        "bat"
            | "cmd"
            | "ps1"
            | "psm1"
            | "psd1"
            | "vbs"
            | "js"
            | "jse"
            | "wsf"
            | "wsh"
            | "hta"
            | "msi"
            | "com"
            | "exe"
            | "scr"
            | "pif"
    )
}

fn should_force_bin_by_file_name(file_name: &str) -> bool {
    std::path::Path::new(file_name.trim())
        .extension()
        .and_then(|v| v.to_str())
        .map(is_dangerous_executable_extension)
        .unwrap_or(false)
}

fn apply_download_extension_policy(file_name: &str, mime: &str) -> String {
    let normalized = sanitize_download_file_name(file_name);
    if should_force_bin_by_file_name(&normalized) {
        let stem = std::path::Path::new(&normalized)
            .file_stem()
            .and_then(|v| v.to_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("attachment");
        return format!("{stem}.bin");
    }
    let ext = media_extension_from_mime_for_download(mime);
    if should_append_download_extension(&normalized, ext) {
        format!("{normalized}.{ext}")
    } else {
        normalized
    }
}

fn should_append_download_extension(file_name: &str, ext: &str) -> bool {
    let file_name = file_name.trim();
    if file_name.is_empty() || ext.trim().is_empty() {
        return false;
    }
    if ext.eq_ignore_ascii_case("bin") {
        let has_existing_ext = std::path::Path::new(file_name)
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if has_existing_ext {
            return false;
        }
    }
    !file_name
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", ext.to_ascii_lowercase()))
}

fn sanitize_download_file_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return format!("attachment-{}", Uuid::new_v4());
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        let blocked = matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
        if blocked || ch.is_control() {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let normalized = out.trim().trim_matches('.').trim().to_string();
    if normalized.is_empty() {
        format!("attachment-{}", Uuid::new_v4())
    } else {
        normalized
    }
}

fn persist_raw_attachment_to_downloads(
    state: &AppState,
    suggested_name: &str,
    mime: &str,
    raw: &[u8],
) -> Result<PathBuf, String> {
    persist_raw_attachment_to_downloads_subdir(state, None, suggested_name, mime, raw)
}

fn persist_raw_attachment_to_downloads_subdir(
    state: &AppState,
    subdir: Option<&str>,
    suggested_name: &str,
    mime: &str,
    raw: &[u8],
) -> Result<PathBuf, String> {
    if raw.is_empty() {
        return Err("Attachment payload is empty".to_string());
    }
    let dir = if let Some(subdir) = subdir
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        workspace_downloads_dir(state).join(sanitize_storage_subdir(subdir)?)
    } else {
        workspace_downloads_dir(state)
    };
    fs::create_dir_all(&dir).map_err(|err| format!("Create downloads dir failed: {err}"))?;

    let file_name = apply_download_extension_policy(suggested_name, mime);
    let target = dir.join(file_name);
    let final_target = if target.exists() {
        if existing_file_content_equals_raw(&target, raw)? {
            target
        } else {
            let stem = target
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("attachment");
            let ext = target.extension().and_then(|v| v.to_str()).unwrap_or("bin");
            dir.join(format!("{stem}-{}.{}", Uuid::new_v4(), ext))
        }
    } else {
        target
    };
    if final_target.exists() {
        return Ok(final_target);
    }
    fs::write(&final_target, raw).map_err(|err| format!("Write attachment failed: {err}"))?;
    Ok(final_target)
}

fn existing_file_content_equals_raw(path: &std::path::Path, raw: &[u8]) -> Result<bool, String> {
    let meta = fs::metadata(path).map_err(|err| format!("Read existing attachment metadata failed: {err}"))?;
    if meta.len() != raw.len() as u64 {
        return Ok(false);
    }
    let mut file = fs::File::open(path).map_err(|err| format!("Open existing attachment failed: {err}"))?;
    let mut offset = 0usize;
    let mut buf = [0u8; 8192];
    while offset < raw.len() {
        let read = std::io::Read::read(&mut file, &mut buf)
            .map_err(|err| format!("Read existing attachment failed: {err}"))?;
        if read == 0 {
            return Ok(false);
        }
        let end = offset + read;
        if end > raw.len() || buf[..read] != raw[offset..end] {
            return Ok(false);
        }
        offset = end;
    }
    Ok(true)
}

fn workspace_relative_path(state: &AppState, absolute: &std::path::Path) -> String {
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    absolute
        .strip_prefix(&workspace_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| absolute.to_string_lossy().replace('\\', "/"))
}

fn assistant_space_display_path(relative_path: &str) -> String {
    let trimmed = relative_path.trim().trim_start_matches(['\\', '/']);
    if trimmed.is_empty() {
        "{Assistant Space}".to_string()
    } else {
        format!("{{Assistant Space}}/{}", trimmed.replace('\\', "/"))
    }
}

fn build_attachment_notice_text(index: usize, relative_path: &str) -> String {
    format!(
        "<attachment index=\"{}\">path: {}</attachment>",
        index + 1,
        assistant_space_display_path(relative_path)
    )
}

// 附件属于用户发来的内容，整组跟在用户正文之后，不进入前置提醒包裹。
fn build_attachments_block(relative_paths: &[String]) -> Option<String> {
    let mut children = Vec::<String>::new();
    for (index, relative_path) in relative_paths.iter().enumerate() {
        let trimmed = relative_path.trim();
        if trimmed.is_empty() {
            continue;
        }
        children.push(build_attachment_notice_text(index, trimmed));
    }
    if children.is_empty() {
        return None;
    }
    Some(format!(
        "<attachments>\n{}\n</attachments>",
        children.join("\n")
    ))
}

fn queue_attachment_from_raw(
    state: &AppState,
    file_name_input: &str,
    mime_input: &str,
    raw: &[u8],
) -> Result<QueueLocalFileAttachmentOutput, String> {
    let file_name = file_name_input
        .trim()
        .trim_matches(['\\', '/'])
        .trim()
        .to_string();
    let file_name = if file_name.is_empty() {
        "attachment".to_string()
    } else {
        file_name
    };
    let mime = if mime_input.trim().is_empty() {
        media_mime_from_path(std::path::Path::new(&file_name))
            .unwrap_or("application/octet-stream")
            .to_string()
    } else {
        mime_input.trim().to_ascii_lowercase()
    };
    let attach_as_media = matches!(
        mime.as_str(),
        "application/pdf"
            | "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
    ) && raw.len() <= MAX_MULTIMODAL_BYTES;

    // 入队即落盘：附件进入队列后立刻可在 downloads 查看与复查。
    let saved_path = persist_raw_attachment_to_downloads(state, &file_name, &mime, raw)?;
    let final_saved_path = message_attachment_display_path(&saved_path.to_string_lossy());
    let final_file_name = saved_path
        .file_name()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(file_name.as_str())
        .to_string();

    let bytes_base64 = if attach_as_media {
        Some(B64.encode(raw))
    } else {
        None
    };
    let label = if mime.starts_with("image/") {
        "图片#1"
    } else {
        "附件#1"
    };
    let text_notice = message_attachment_notice_text(label, &final_saved_path);
    Ok(QueueLocalFileAttachmentOutput {
        mime,
        file_name: final_file_name,
        saved_path: final_saved_path,
        attach_as_media,
        bytes_base64,
        text_notice,
    })
}

fn normalize_payload_attachments(
    raw: Option<&Vec<AttachmentMetaInput>>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::<serde_json::Value>::new();
    let Some(items) = raw else {
        return out;
    };
    let mut seen = std::collections::HashSet::<String>::new();
    for item in items {
        let file_name = String::from(item.file_name.trim());
        let relative_path = String::from(item.path.trim()).replace('\\', "/");
        let mime = String::from(item.mime.trim());
        if file_name.is_empty() || relative_path.is_empty() {
            continue;
        }
        let dedup_key = format!(
            "{}::{}",
            relative_path.to_ascii_lowercase(),
            mime.to_ascii_lowercase()
        );
        if !seen.insert(dedup_key) {
            continue;
        }
        out.push(serde_json::json!({
            "fileName": file_name,
            "relativePath": relative_path,
            "mime": mime,
        }));
    }
    out
}

fn provider_meta_without_legacy_attachments(provider_meta: Option<Value>) -> Option<Value> {
    let mut meta = provider_meta?;
    if let Some(object) = meta.as_object_mut() {
        object.remove("attachments");
        if object.is_empty() {
            return None;
        }
    }
    Some(meta)
}

fn provider_meta_attachment_relative_paths(meta: &Value) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let Some(attachments) = meta.get("attachments").and_then(Value::as_array) else {
        return out;
    };
    let mut seen = std::collections::HashSet::<String>::new();
    for item in attachments {
        let relative_path = item
            .get("relativePath")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.replace('\\', "/"));
        let Some(relative_path) = relative_path else {
            continue;
        };
        let mime = item
            .get("mime")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let dedup_key = format!(
            "{}::{}",
            relative_path.to_ascii_lowercase(),
            mime.to_ascii_lowercase()
        );
        if !seen.insert(dedup_key) {
            continue;
        }
        out.push(relative_path);
    }
    out
}

#[tauri::command]
async fn read_local_binary_file(
    input: ReadLocalBinaryFileInput,
) -> Result<ReadLocalBinaryFileOutput, String> {
    tokio::task::spawn_blocking(move || {
        let path_text = input.path.trim();
        if path_text.is_empty() {
            return Err("File path is empty.".to_string());
        }
        let path = std::path::PathBuf::from(path_text);
        let mime = media_mime_from_path(&path)
            .ok_or_else(|| format!("Unsupported file type: '{}'.", path_text))?
            .to_string();
        let raw = fs::read(&path).map_err(|err| format!("Read file failed: {err}"))?;
        if raw.len() > MAX_MULTIMODAL_BYTES {
            return Err(format!(
                "File is too large ({} bytes). Max allowed is {} bytes.",
                raw.len(),
                MAX_MULTIMODAL_BYTES
            ));
        }
        Ok(ReadLocalBinaryFileOutput {
            mime,
            bytes_base64: B64.encode(raw),
        })
    })
    .await
    .map_err(|err| format!("读取本地二进制文件任务异常：{err}"))?
}

#[tauri::command]
async fn queue_local_file_attachment(
    input: QueueLocalFileAttachmentInput,
    state: State<'_, AppState>,
) -> Result<QueueLocalFileAttachmentOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let path_text = input.path.trim();
        if path_text.is_empty() {
            return Err("File path is empty.".to_string());
        }
        let path = std::path::PathBuf::from(path_text);
        let file_name = path
            .file_name()
            .and_then(|v| v.to_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("attachment")
            .to_string();
        let raw = fs::read(&path).map_err(|err| format!("Read file failed: {err}"))?;
        let mime = media_mime_from_path(&path)
            .unwrap_or("application/octet-stream")
            .to_string();
        queue_attachment_from_raw(&app_state, &file_name, &mime, &raw)
    })
    .await
    .map_err(|err| format!("本地附件兼容摄取任务异常：{err}"))?
}

#[tauri::command]
async fn queue_inline_file_attachment(
    input: QueueInlineFileAttachmentInput,
    state: State<'_, AppState>,
) -> Result<QueueLocalFileAttachmentOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || queue_inline_file_attachment_inner(input, &app_state))
        .await
        .map_err(|err| format!("内联附件兼容摄取任务异常：{err}"))?
}

fn queue_inline_file_attachment_inner(
    input: QueueInlineFileAttachmentInput,
    state: &AppState,
) -> Result<QueueLocalFileAttachmentOutput, String> {
    if input.bytes_base64.trim().is_empty() {
        return Err("Attachment payload is empty.".to_string());
    }
    let raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode attachment base64 failed: {err}"))?;
    queue_attachment_from_raw(
        state,
        input.file_name.trim(),
        input.mime.trim(),
        &raw,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalChatImageThumbnailInput {
    path: String,
    #[serde(default)]
    max_edge: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalChatImageOutput {
    data_url: String,
    mime: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLocalChatImageThumbnailOutput {
    data_url: String,
    mime: String,
    width: u32,
    height: u32,
    original_width: u32,
    original_height: u32,
}

fn assistant_space_relative_image_path(path: &str) -> Result<Option<PathBuf>, String> {
    const ASSISTANT_SPACE_PREFIX: &str = "{Assistant Space}";

    let trimmed = path.trim();
    let Some(suffix) = trimmed.strip_prefix(ASSISTANT_SPACE_PREFIX) else {
        return Ok(None);
    };
    if !suffix.is_empty() && !suffix.starts_with(['/', '\\']) {
        return Ok(None);
    }
    let normalized = suffix
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/");
    if normalized.is_empty() {
        return Err("Assistant Space 图片路径为空".to_string());
    }
    let relative = PathBuf::from(&normalized);
    if relative.is_absolute()
        || relative.components().any(|component| match component {
            std::path::Component::Normal(value) => value.to_string_lossy().contains(':'),
            _ => true,
        })
    {
        return Err("Assistant Space 图片路径不安全".to_string());
    }
    Ok(Some(relative))
}

fn resolve_local_chat_image_path(state: &AppState, path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("图片路径为空".to_string());
    }
    let Some(relative) = assistant_space_relative_image_path(trimmed)? else {
        return Ok(PathBuf::from(trimmed));
    };
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|err| format!("解析 Assistant Space 目录失败：{err}"))?;
    let target = workspace_root.join(relative);
    let canonical_target = target
        .canonicalize()
        .map_err(|err| format!("解析 Assistant Space 图片路径失败：{err}"))?;
    if !path_is_within(&canonical_root, &canonical_target) {
        return Err("Assistant Space 图片路径越界".to_string());
    }
    Ok(canonical_target)
}

#[tauri::command]
async fn read_local_chat_image_thumbnail(
    input: ReadLocalChatImageThumbnailInput,
    state: State<'_, AppState>,
) -> Result<ReadLocalChatImageThumbnailOutput, String> {
    read_local_chat_image_thumbnail_inner(input, state.inner()).await
}

async fn read_local_chat_image_thumbnail_inner(
    input: ReadLocalChatImageThumbnailInput,
    state: &AppState,
) -> Result<ReadLocalChatImageThumbnailOutput, String> {
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        let path = resolve_local_chat_image_path(&app_state, &input.path)?;
        let max_edge = input.max_edge.unwrap_or(LOCAL_IMAGE_THUMBNAIL_MAX_EDGE);
        let render = local_image_read_for_display(&path, max_edge)?;
        let data_url = format!("data:{};base64,{}", render.mime, B64.encode(&render.bytes));
        Ok(ReadLocalChatImageThumbnailOutput {
            data_url,
            mime: render.mime,
            width: render.output_width,
            height: render.output_height,
            original_width: render.original_width,
            original_height: render.original_height,
        })
    })
    .await
    .map_err(|err| format!("读取本地图片缩略图任务异常：{err}"))?
}

#[tauri::command]
async fn read_local_chat_image_original(
    input: ReadLocalChatImageThumbnailInput,
    state: State<'_, AppState>,
) -> Result<ReadLocalChatImageOutput, String> {
    read_local_chat_image_original_inner(input, state.inner()).await
}

async fn read_local_chat_image_original_inner(
    input: ReadLocalChatImageThumbnailInput,
    state: &AppState,
) -> Result<ReadLocalChatImageOutput, String> {
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        let path = resolve_local_chat_image_path(&app_state, &input.path)?;
        let render = local_image_read_original(&path)?;
        let data_url = format!("data:{};base64,{}", render.mime, B64.encode(&render.bytes));
        Ok(ReadLocalChatImageOutput {
            data_url,
            mime: render.mime,
            width: render.output_width,
            height: render.output_height,
        })
    })
    .await
    .map_err(|err| format!("读取本地图片原图任务异常：{err}"))?
}

#[tauri::command]
async fn copy_local_chat_image_to_clipboard(
    input: ReadLocalChatImageThumbnailInput,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let path = resolve_local_chat_image_path(&app_state, &input.path)?;
        let raw = local_image_read_raw(&path)?;
        let (_, mime) = local_image_detect_format(&raw, &path)?;
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|err| format!("初始化剪贴板失败: {err}"))?;

        if matches!(mime.as_str(), "image/gif" | "image/webp") {
            clipboard
                .set()
                .file_list(&[path.as_path()])
                .map_err(|err| format!("复制图片文件到剪贴板失败: {err}"))?;
            return Ok(serde_json::json!({ "ok": true, "mode": "file" }));
        }

        let (dynamic, _) = local_image_decode_dynamic(&raw, &path)?;
        let rgba = dynamic.to_rgba8();
        let image_data = arboard::ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: rgba.as_raw().clone().into(),
        };
        clipboard.set_image(image_data)
            .map_err(|err| format!("复制图片到剪贴板失败: {err}"))?;
        Ok(serde_json::json!({ "ok": true, "mode": "bitmap" }))
    })
    .await
    .map_err(|err| format!("复制本地图片任务异常：{err}"))?
}

#[tauri::command]
async fn save_local_chat_image_as(
    input: ReadLocalChatImageThumbnailInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let source_path = resolve_local_chat_image_path(state.inner(), &input.path)?;
    if !source_path.exists() {
        return Err(format!("源文件不存在: {}", source_path.to_string_lossy()));
    }
    let file_name = source_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("image.webp");
    let (dialog_tx, dialog_rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(file_name)
        .save_file(move |file| {
            let _ = dialog_tx.send(file);
        });
    let dest = dialog_rx
        .await
        .map_err(|err| format!("等待保存对话框结果失败：{err}"))?;
    let dest_path = dest
        .and_then(|fp| fp.as_path().map(ToOwned::to_owned))
        .ok_or_else(|| "用户取消了保存".to_string())?;
    tokio::task::spawn_blocking(move || std::fs::copy(&source_path, &dest_path))
        .await
        .map_err(|err| format!("复制图片文件任务异常：{err}"))?
        .map_err(|err| format!("复制文件失败: {err}"))?;
    Ok(serde_json::json!({ "ok": true }))
}

#[cfg(test)]
mod assistant_space_image_path_tests {
    use super::*;

    #[test]
    fn assistant_space_image_path_should_parse_safe_relative_path() {
        let path = assistant_space_relative_image_path(
            "{Assistant Space}/generated-images/20260727/image.png",
        )
        .ok()
        .flatten()
        .unwrap_or_default();
        assert_eq!(
            path.to_string_lossy().replace('\\', "/"),
            "generated-images/20260727/image.png"
        );
    }

    #[test]
    fn assistant_space_image_path_should_reject_parent_traversal() {
        let error = assistant_space_relative_image_path(
            "{Assistant Space}/generated-images/../../outside.png",
        )
        .err()
        .unwrap_or_default();
        assert!(error.contains("不安全"));
    }
}
