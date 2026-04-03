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
        "heic" => Some("image/heic"),
        "heif" => Some("image/heif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
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
    let dir = workspace_downloads_dir(state);
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

fn build_attachment_notice_text(file_name: &str, relative_path: &str) -> String {
    format!(
        "用户本次上传了一个附件：{file_name}\n保存到了你工作区的downloads文件夹内（路径：{relative_path}）\n如果需要，请使用 shell 工具读取该文件内容。"
    )
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
    ) && raw.len() <= MAX_MULTIMODAL_BYTES;

    // 入队即落盘：附件进入队列后立刻可在 downloads 查看与复查。
    let saved_path = persist_raw_attachment_to_downloads(state, &file_name, &mime, raw)?;
    let final_saved_path = workspace_relative_path(state, &saved_path);
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
    let text_notice = build_attachment_notice_text(&final_file_name, &final_saved_path);
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
    for item in items {
        let file_name = String::from(item.file_name.trim());
        let relative_path = String::from(item.relative_path.trim()).replace('\\', "/");
        let mime = String::from(item.mime.trim());
        if file_name.is_empty() || relative_path.is_empty() {
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

fn merge_provider_meta_with_attachments(
    provider_meta: Option<Value>,
    attachments: &[Value],
) -> Option<Value> {
    let mut merged = provider_meta.unwrap_or_else(|| serde_json::json!({}));
    if !merged.is_object() {
        merged = serde_json::json!({});
    }
    if attachments.is_empty() {
        return if merged.as_object().map(|v| v.is_empty()).unwrap_or(true) {
            None
        } else {
            Some(merged)
        };
    }
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("attachments".to_string(), Value::Array(attachments.to_vec()));
    }
    Some(merged)
}

fn persist_payload_images_to_workspace_downloads(
    state: &AppState,
    images: &[BinaryPart],
) -> Vec<String> {
    let mut notices = Vec::<String>::new();
    for (idx, image) in images.iter().enumerate() {
        if let Some(saved_path) = image
            .saved_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let file_name = std::path::Path::new(saved_path)
                .file_name()
                .and_then(|v| v.to_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("attachment")
                .to_string();
            notices.push(build_attachment_notice_text(&file_name, saved_path));
            continue;
        }
        let mime = image.mime.trim();
        if mime.is_empty() {
            continue;
        }
        let Ok(raw) = B64.decode(image.bytes_base64.trim()) else {
            eprintln!("[CHAT] skip persist image to downloads: invalid base64, index={idx}");
            continue;
        };
        let suggested = format!("queued-image-{}", idx + 1);
        match persist_raw_attachment_to_downloads(state, &suggested, mime, &raw) {
            Ok(saved_path) => {
                let relative = workspace_relative_path(state, &saved_path);
                let file_name = saved_path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or(&suggested)
                    .to_string();
                notices.push(build_attachment_notice_text(&file_name, &relative));
            }
            Err(err) => {
                eprintln!("[CHAT] persist queued image to downloads failed: index={}, err={}", idx, err);
            }
        }
    }
    notices
}

#[tauri::command]
fn read_local_binary_file(
    input: ReadLocalBinaryFileInput,
) -> Result<ReadLocalBinaryFileOutput, String> {
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
}

#[tauri::command]
fn queue_local_file_attachment(
    input: QueueLocalFileAttachmentInput,
    state: State<'_, AppState>,
) -> Result<QueueLocalFileAttachmentOutput, String> {
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
    queue_attachment_from_raw(state.inner(), &file_name, &mime, &raw)
}

#[tauri::command]
fn queue_inline_file_attachment(
    input: QueueInlineFileAttachmentInput,
    state: State<'_, AppState>,
) -> Result<QueueLocalFileAttachmentOutput, String> {
    if input.bytes_base64.trim().is_empty() {
        return Err("Attachment payload is empty.".to_string());
    }
    let raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode attachment base64 failed: {err}"))?;
    queue_attachment_from_raw(
        state.inner(),
        input.file_name.trim(),
        input.mime.trim(),
        &raw,
    )
}

