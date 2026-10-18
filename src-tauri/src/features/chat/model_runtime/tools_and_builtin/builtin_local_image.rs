const LOCAL_IMAGE_THUMBNAIL_MAX_EDGE: u32 = 1080;
const LOCAL_IMAGE_REMOTE_MAX_EDGE: u32 = 2160;
const LOCAL_IMAGE_WEBP_QUALITY: f32 = 82.0;
const LOCAL_IMAGE_FALLBACK_MIME: &str = "image/webp";
const LOCAL_IMAGE_MAX_SOURCE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum PersistedInlineMessageSegment {
    Text { text: String },
    Meme {
        name: String,
        mime: String,
        relative_path: String,
        bytes_base64: String,
    },
    LocalImage {
        path: String,
        file_name: String,
        mime: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
    },
}

#[derive(Debug, Clone)]
struct LocalImageRenderData {
    mime: String,
    bytes: Vec<u8>,
    original_width: u32,
    original_height: u32,
    output_width: u32,
    output_height: u32,
}

#[derive(Debug, Clone)]
struct LocalImageFileInfo {
    mime: String,
    width: u32,
    height: u32,
}

fn local_image_workspace_root(state: &AppState) -> PathBuf {
    configured_workspace_root_path(state).unwrap_or_else(|_| state.llm_workspace_path.clone())
}

fn local_image_resolve_path(state: &AppState, raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    let direct = PathBuf::from(trimmed);
    if direct.is_absolute() {
        direct
    } else {
        local_image_workspace_root(state).join(direct)
    }
}

fn local_image_file_name(path: &std::path::Path, alt: Option<&str>) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            alt.map(str::trim)
                .filter(|value| !value.is_empty())
                .map(sanitize_download_file_name)
        })
        .unwrap_or_else(|| "image.webp".to_string())
}

fn local_image_mime_from_format(format: image::ImageFormat) -> Option<&'static str> {
    match format {
        image::ImageFormat::Png => Some("image/png"),
        image::ImageFormat::Jpeg => Some("image/jpeg"),
        image::ImageFormat::Gif => Some("image/gif"),
        image::ImageFormat::WebP => Some("image/webp"),
        image::ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

fn local_image_format_from_mime(mime: &str) -> Option<image::ImageFormat> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => Some(image::ImageFormat::Png),
        "image/jpeg" | "image/jpg" => Some(image::ImageFormat::Jpeg),
        "image/gif" => Some(image::ImageFormat::Gif),
        "image/webp" => Some(image::ImageFormat::WebP),
        "image/bmp" => Some(image::ImageFormat::Bmp),
        _ => None,
    }
}

fn local_image_guess_mime_from_path(path: &std::path::Path) -> String {
    media_mime_from_path(path)
        .and_then(local_image_format_from_mime)
        .and_then(local_image_mime_from_format)
        .unwrap_or(LOCAL_IMAGE_FALLBACK_MIME)
        .to_string()
}

fn local_image_detect_format(raw: &[u8], path: &std::path::Path) -> Result<(image::ImageFormat, String), String> {
    let format = image::guess_format(raw).map_err(|err| {
        format!(
            "识别本地图片格式失败: path={}, err={err}",
            path.to_string_lossy()
        )
    })?;
    let Some(mime) = local_image_mime_from_format(format) else {
        return Err(format!(
            "本地图片格式不支持: path={}, format={format:?}",
            path.to_string_lossy()
        ));
    };
    Ok((format, mime.to_string()))
}

fn local_image_read_raw(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|err| format!("本地图片不存在或无法读取元数据: path={}, err={err}", path.to_string_lossy()))?;
    if !metadata.is_file() {
        return Err(format!("本地图片路径不是文件: {}", path.to_string_lossy()));
    }
    if metadata.len() > LOCAL_IMAGE_MAX_SOURCE_BYTES {
        return Err(format!(
            "本地图片过大: path={}, bytes={}, max_bytes={}",
            path.to_string_lossy(),
            metadata.len(),
            LOCAL_IMAGE_MAX_SOURCE_BYTES
        ));
    }
    std::fs::read(path)
        .map_err(|err| format!("读取本地图片失败: path={}, err={err}", path.to_string_lossy()))
}

fn local_image_decode_dynamic(raw: &[u8], path: &std::path::Path) -> Result<(image::DynamicImage, String), String> {
    let (format, mime) = local_image_detect_format(raw, path)?;
    let image = image::load_from_memory_with_format(raw, format).map_err(|err| {
        format!(
            "解码本地图片失败: path={}, format={format:?}, err={err}",
            path.to_string_lossy()
        )
    })?;
    Ok((image, mime))
}

fn local_image_file_info(path: &std::path::Path) -> Result<LocalImageFileInfo, String> {
    let raw = local_image_read_raw(path)?;
    let (image, mime) = local_image_decode_dynamic(&raw, path)?;
    Ok(LocalImageFileInfo {
        mime,
        width: image.width(),
        height: image.height(),
    })
}

fn local_image_resized_dimensions(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_edge || longest == 0 {
        return (width.max(1), height.max(1));
    }
    let new_width = ((width as u64 * max_edge as u64 + longest as u64 / 2) / longest as u64)
        .max(1) as u32;
    let new_height = ((height as u64 * max_edge as u64 + longest as u64 / 2) / longest as u64)
        .max(1) as u32;
    (new_width, new_height)
}

fn local_image_encode_webp(image: image::DynamicImage, max_edge: u32) -> Result<LocalImageRenderData, String> {
    let original_width = image.width();
    let original_height = image.height();
    let (target_width, target_height) =
        local_image_resized_dimensions(original_width, original_height, max_edge);
    let resized = if (target_width, target_height) == (original_width, original_height) {
        image
    } else {
        image.resize_exact(target_width, target_height, image::imageops::FilterType::Lanczos3)
    };
    let encoder = webp::Encoder::from_image(&resized)
        .map_err(|err| format!("初始化本地图片 WebP 编码器失败: {err}"))?;
    let encoded = encoder.encode(LOCAL_IMAGE_WEBP_QUALITY);
    Ok(LocalImageRenderData {
        mime: LOCAL_IMAGE_FALLBACK_MIME.to_string(),
        bytes: (&*encoded).to_vec(),
        original_width,
        original_height,
        output_width: resized.width(),
        output_height: resized.height(),
    })
}

fn local_image_read_for_display(path: &std::path::Path, max_edge: u32) -> Result<LocalImageRenderData, String> {
    let raw = local_image_read_raw(path)?;
    let (image, mime) = local_image_decode_dynamic(&raw, path)?;
    let original_width = image.width();
    let original_height = image.height();
    if original_width.max(original_height) <= max_edge {
        return Ok(LocalImageRenderData {
            mime,
            bytes: raw,
            original_width,
            original_height,
            output_width: original_width,
            output_height: original_height,
        });
    }
    local_image_encode_webp(image, max_edge)
}

fn local_image_read_original(path: &std::path::Path) -> Result<LocalImageRenderData, String> {
    let raw = local_image_read_raw(path)?;
    let (image, mime) = local_image_decode_dynamic(&raw, path)?;
    Ok(LocalImageRenderData {
        mime,
        bytes: raw,
        original_width: image.width(),
        original_height: image.height(),
        output_width: image.width(),
        output_height: image.height(),
    })
}

#[derive(Debug, Clone)]
struct LocalImageReference {
    start: usize,
    end: usize,
    raw_path: String,
    alt: Option<String>,
}

fn local_image_markdown_is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    let mut backslash_count = 0usize;
    while cursor > 0 {
        cursor -= 1;
        if bytes.get(cursor) != Some(&b'\\') {
            break;
        }
        backslash_count += 1;
    }
    backslash_count % 2 == 1
}

fn local_image_skip_ascii_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while let Some(value) = bytes.get(cursor) {
        if !matches!(value, b' ' | b'\t' | b'\r' | b'\n') {
            break;
        }
        cursor += 1;
    }
    cursor
}

fn local_image_unescape_markdown_text(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.peek().copied() {
                if matches!(
                    next,
                    '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#'
                        | '+' | '-' | '.' | '!' | '<' | '>' | '|'
                ) {
                    out.push(next);
                    let _ = chars.next();
                    continue;
                }
            }
        }
        out.push(ch);
    }
    out
}

fn local_image_strip_markdown_destination_title(value: &str, quote: char) -> Option<String> {
    if !value.ends_with(quote) {
        return None;
    }
    let marker = format!(" {quote}");
    value
        .rsplit_once(marker.as_str())
        .map(|(path, _title)| path.trim().to_string())
        .filter(|path| !path.is_empty())
}

fn local_image_clean_markdown_destination_path(value: &str) -> String {
    let trimmed = value.trim();
    if local_image_is_windows_drive_path(trimmed) || trimmed.starts_with("\\\\") {
        return trimmed.to_string();
    }
    local_image_unescape_markdown_text(trimmed)
}

fn local_image_extract_markdown_destination_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('<') {
        if let Some(end) = rest.find('>') {
            return local_image_clean_markdown_destination_path(&rest[..end]);
        }
    }
    let without_title = local_image_strip_markdown_destination_title(trimmed, '"')
        .or_else(|| local_image_strip_markdown_destination_title(trimmed, '\''))
        .unwrap_or_else(|| trimmed.to_string());
    local_image_clean_markdown_destination_path(&without_title)
}

fn local_image_is_windows_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn local_image_uri_scheme(value: &str) -> Option<&str> {
    let colon = value.find(':')?;
    let scheme = &value[..colon];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')) {
        return None;
    }
    Some(scheme)
}

fn local_image_path_from_file_url(raw: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(raw).ok()?;
    if parsed.scheme() != "file" {
        return None;
    }
    let decoded_path = urlencoding::decode(parsed.path()).ok()?.to_string();
    let host = parsed.host_str().map(str::trim).unwrap_or_default();
    if !host.is_empty() && host != "localhost" {
        if cfg!(windows) {
            return Some(format!(
                "\\\\{}{}",
                host,
                decoded_path.replace('/', "\\")
            ));
        }
        return Some(format!("//{}{}", host, decoded_path));
    }
    if decoded_path.len() >= 3 {
        let bytes = decoded_path.as_bytes();
        if bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
            return Some(decoded_path[1..].to_string());
        }
    }
    Some(decoded_path)
}

fn local_image_path_from_markdown_destination(raw: &str) -> Option<String> {
    let value = local_image_extract_markdown_destination_path(raw);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.starts_with("blob:")
        || lower.starts_with("mailto:")
        || lower.starts_with("javascript:")
    {
        return None;
    }
    if lower.starts_with("file:") {
        return local_image_path_from_file_url(trimmed);
    }
    if local_image_uri_scheme(trimmed).is_some() && !local_image_is_windows_drive_path(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
}

fn local_image_find_markdown_alt_end(text: &str, cursor: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = cursor;
    while index < bytes.len() {
        if bytes[index] == b']' && !local_image_markdown_is_escaped(bytes, index) {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn local_image_find_angle_destination_end(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut index = cursor + 1;
    while index < bytes.len() {
        if bytes[index] == b'>' && !local_image_markdown_is_escaped(bytes, index) {
            let close = local_image_skip_ascii_whitespace(bytes, index + 1);
            if bytes.get(close) == Some(&b')') {
                return Some((index + 1, close + 1));
            }
            return None;
        }
        index += 1;
    }
    None
}

fn local_image_find_plain_destination_end(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = cursor;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            index += 2;
            continue;
        }
        if bytes[index] == b'(' {
            depth += 1;
        } else if bytes[index] == b')' {
            if depth == 0 {
                return Some((index, index + 1));
            }
            depth = depth.saturating_sub(1);
        }
        index += 1;
    }
    None
}

fn local_image_find_markdown_destination_end(text: &str, open_paren: usize) -> Option<(usize, usize, usize)> {
    let bytes = text.as_bytes();
    let start = local_image_skip_ascii_whitespace(bytes, open_paren + 1);
    if bytes.get(start) == Some(&b'<') {
        let (end, token_end) = local_image_find_angle_destination_end(text, start)?;
        return Some((start, end, token_end));
    }
    let (mut end, token_end) = local_image_find_plain_destination_end(text, start)?;
    while end > start && matches!(bytes[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    Some((start, end, token_end))
}

fn local_image_find_next_markdown_reference(text: &str, cursor: usize) -> Option<LocalImageReference> {
    let bytes = text.as_bytes();
    let mut search_from = cursor;
    while search_from + 2 <= text.len() {
        let start = text.get(search_from..)?.find("![").map(|idx| search_from + idx)?;
        let alt_start = start + 2;
        let Some(alt_end) = local_image_find_markdown_alt_end(text, alt_start) else {
            return None;
        };
        let open_paren = local_image_skip_ascii_whitespace(bytes, alt_end + 1);
        if bytes.get(open_paren) != Some(&b'(') {
            search_from = start + 2;
            continue;
        }
        let Some((dest_start, dest_end, token_end)) =
            local_image_find_markdown_destination_end(text, open_paren)
        else {
            search_from = start + 2;
            continue;
        };
        let raw_destination = &text[dest_start..dest_end];
        let Some(raw_path) = local_image_path_from_markdown_destination(raw_destination) else {
            search_from = start + 2;
            continue;
        };
        let alt = local_image_unescape_markdown_text(&text[alt_start..alt_end])
            .trim()
            .to_string();
        return Some(LocalImageReference {
            start,
            end: token_end,
            raw_path,
            alt: (!alt.is_empty()).then_some(alt),
        });
    }
    None
}

fn local_image_find_next_reference(text: &str, cursor: usize) -> Option<LocalImageReference> {
    local_image_find_next_markdown_reference(text, cursor)
}

fn local_image_segment_from_reference(
    state: &AppState,
    reference: &LocalImageReference,
) -> PersistedInlineMessageSegment {
    let resolved = local_image_resolve_path(state, &reference.raw_path);
    let info = local_image_file_info(&resolved).ok();
    PersistedInlineMessageSegment::LocalImage {
        path: resolved.to_string_lossy().to_string(),
        file_name: local_image_file_name(&resolved, reference.alt.as_deref()),
        mime: info
            .as_ref()
            .map(|value| value.mime.clone())
            .unwrap_or_else(|| local_image_guess_mime_from_path(&resolved)),
        alt: reference.alt.clone(),
        width: info.as_ref().map(|value| value.width),
        height: info.as_ref().map(|value| value.height),
    }
}

fn resolve_text_to_local_image_segments(
    state: &AppState,
    text: &str,
) -> Vec<PersistedInlineMessageSegment> {
    let mut segments = Vec::<PersistedInlineMessageSegment>::new();
    let mut cursor = 0usize;
    let mut text_cursor = 0usize;
    while cursor < text.len() {
        let Some(reference) = local_image_find_next_reference(text, cursor) else {
            break;
        };
        if reference.start > text_cursor {
            segments.push(PersistedInlineMessageSegment::Text {
                text: text[text_cursor..reference.start].to_string(),
            });
        }
        segments.push(local_image_segment_from_reference(state, &reference));
        cursor = reference.end;
        text_cursor = reference.end;
    }
    if text_cursor < text.len() {
        segments.push(PersistedInlineMessageSegment::Text {
            text: text[text_cursor..].to_string(),
        });
    }
    if segments.is_empty() {
        segments.push(PersistedInlineMessageSegment::Text {
            text: text.to_string(),
        });
    }
    segments
}

fn inline_segment_from_meme_segment(segment: PersistedMemeSegment) -> PersistedInlineMessageSegment {
    match segment {
        PersistedMemeSegment::Text { text } => PersistedInlineMessageSegment::Text { text },
        PersistedMemeSegment::Meme {
            name,
            mime,
            relative_path,
            bytes_base64,
        } => PersistedInlineMessageSegment::Meme {
            name,
            mime,
            relative_path,
            bytes_base64,
        },
    }
}

#[derive(Debug, Clone)]
struct MemeAnnotationInlineReference {
    start: usize,
    end: usize,
    path: String,
}

fn collect_meme_annotation_inline_references(
    text: &str,
    annotations: &[MemeAnnotation],
) -> Vec<MemeAnnotationInlineReference> {
    let mut out = Vec::<MemeAnnotationInlineReference>::new();
    let mut cursor = 0usize;
    for annotation in annotations {
        let meme = annotation.meme.trim();
        let path = annotation.path.trim();
        if meme.is_empty() || path.is_empty() {
            continue;
        }
        let Some(start_rel) = text[cursor..].find(meme) else {
            continue;
        };
        let start = cursor + start_rel;
        let end = start + meme.len();
        out.push(MemeAnnotationInlineReference {
            start,
            end,
            path: path.to_string(),
        });
        cursor = end;
    }
    out
}

fn inline_segment_from_meme_annotation(
    state: &AppState,
    reference: &MemeAnnotationInlineReference,
) -> Result<PersistedInlineMessageSegment, String> {
    let path = PathBuf::from(reference.path.trim());
    let raw = std::fs::read(&path)
        .map_err(|err| format!("读取表情文件失败: path={}, err={err}", path.display()))?;
    let mime = media_mime_from_path(&path)
        .unwrap_or("image/webp")
        .to_string();
    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("meme")
        .to_string();
    Ok(PersistedInlineMessageSegment::Meme {
        name,
        mime,
        relative_path: workspace_relative_path(state, &path),
        bytes_base64: B64.encode(raw),
    })
}

fn resolve_text_and_meme_annotations_to_inline_segments(
    state: &AppState,
    text: &str,
    annotations: Option<&[MemeAnnotation]>,
) -> Result<Option<Vec<PersistedInlineMessageSegment>>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let meme_refs =
        annotations.map(|items| collect_meme_annotation_inline_references(text, items)).unwrap_or_default();
    let mut local_refs = Vec::<LocalImageReference>::new();
    let mut cursor = 0usize;
    while cursor < text.len() {
        let Some(reference) = local_image_find_next_reference(text, cursor) else {
            break;
        };
        cursor = reference.end;
        local_refs.push(reference);
    }

    let mut out = Vec::<PersistedInlineMessageSegment>::new();
    let mut text_cursor = 0usize;
    let mut local_idx = 0usize;
    let mut meme_idx = 0usize;

    while text_cursor < text.len() {
        let next_local = local_refs.get(local_idx);
        let next_meme = meme_refs.get(meme_idx);
        let next_start = match (next_local, next_meme) {
            (Some(local), Some(meme)) => {
                if local.start <= meme.start {
                    Some((local.start, true))
                } else {
                    Some((meme.start, false))
                }
            }
            (Some(local), None) => Some((local.start, true)),
            (None, Some(meme)) => Some((meme.start, false)),
            (None, None) => None,
        };
        let Some((start, is_local)) = next_start else {
            break;
        };
        if start < text_cursor {
            if is_local {
                local_idx += 1;
            } else {
                meme_idx += 1;
            }
            continue;
        }
        if start > text_cursor {
            out.push(PersistedInlineMessageSegment::Text {
                text: text[text_cursor..start].to_string(),
            });
        }
        if is_local {
            let reference = &local_refs[local_idx];
            out.push(local_image_segment_from_reference(state, reference));
            text_cursor = reference.end;
            local_idx += 1;
        } else {
            let reference = &meme_refs[meme_idx];
            out.push(inline_segment_from_meme_annotation(state, reference)?);
            text_cursor = reference.end;
            meme_idx += 1;
        }
    }

    if text_cursor < text.len() {
        out.push(PersistedInlineMessageSegment::Text {
            text: text[text_cursor..].to_string(),
        });
    }
    if out.is_empty() {
        return Ok(None);
    }
    let has_non_text = out.iter().any(|segment| {
        !matches!(segment, PersistedInlineMessageSegment::Text { .. })
    });
    if has_non_text {
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

fn resolve_text_to_persisted_inline_segments(
    state: &AppState,
    agent_id: &str,
    text: &str,
) -> Result<Option<Vec<PersistedInlineMessageSegment>>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let local_segments = resolve_text_to_local_image_segments(state, text);
    let has_local_image = local_segments
        .iter()
        .any(|segment| matches!(segment, PersistedInlineMessageSegment::LocalImage { .. }));
    let mut has_meme = false;
    let mut out = Vec::<PersistedInlineMessageSegment>::new();
    for segment in local_segments {
        match segment {
            PersistedInlineMessageSegment::Text { text } => {
                if let Some(meme_segments) =
                    resolve_text_to_persisted_meme_segments(state, agent_id, &text)?
                {
                    if meme_segments
                        .iter()
                        .any(|item| matches!(item, PersistedMemeSegment::Meme { .. }))
                    {
                        has_meme = true;
                    }
                    out.extend(meme_segments.into_iter().map(inline_segment_from_meme_segment));
                } else if !text.is_empty() {
                    out.push(PersistedInlineMessageSegment::Text { text });
                }
            }
            other => out.push(other),
        }
    }
    if has_local_image || has_meme {
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod local_image_reference_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn local_image_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-local-image-test-{}", Uuid::new_v4()));
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
            conversation_project_skills_cache: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn write_test_png(path: &Path) {
        let image = image::RgbImage::from_fn(32, 32, |x, y| {
            let r = ((x * 7 + y * 3) % 255) as u8;
            let g = ((x * 11 + y * 5) % 255) as u8;
            let b = ((x * 13 + y * 17) % 255) as u8;
            image::Rgb([r, g, b])
        });
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create image parent");
        }
        image.save(path).expect("save png");
    }

    #[test]
    fn markdown_image_reference_should_parse_windows_path_and_alt() {
        let input = "看这张：![结果图](E:/tmp/result.png) 好了";
        let reference = local_image_find_next_markdown_reference(input, 0)
            .expect("markdown image reference");

        assert_eq!(reference.raw_path, "E:/tmp/result.png");
        assert_eq!(reference.alt.as_deref(), Some("结果图"));
        assert_eq!(&input[reference.start..reference.end], "![结果图](E:/tmp/result.png)");
    }

    #[test]
    fn markdown_image_reference_should_skip_remote_images() {
        let input = "![badge](https://example.com/a.png) ![local](outputs/a.png)";
        let reference = local_image_find_next_markdown_reference(input, 0)
            .expect("local markdown image reference");

        assert_eq!(reference.raw_path, "outputs/a.png");
        assert_eq!(reference.alt.as_deref(), Some("local"));
    }

    #[test]
    fn custom_image_token_should_not_be_parsed() {
        assert!(local_image_find_next_reference("{{image:E:/tmp/a.png|图}}", 0).is_none());
        assert!(local_image_find_next_reference("{{img:E:/tmp/a.png|图}}", 0).is_none());
    }

    #[test]
    fn meme_annotations_should_replace_repeated_tokens_in_order() {
        let state = local_image_test_state();
        let meme_root = meme_agent_root(&state, "agent-meme-test").expect("test meme root");
        let first = meme_root.join("坏笑.webp");
        let second = meme_root.join("坏笑(2).webp");
        write_test_png(&first);
        write_test_png(&second);

        let segments = resolve_text_and_meme_annotations_to_inline_segments(
            &state,
            "前 :坏笑: 中 :坏笑: 后",
            Some(&[
                MemeAnnotation {
                    meme: ":坏笑:".to_string(),
                    path: first.to_string_lossy().to_string(),
                },
                MemeAnnotation {
                    meme: ":坏笑:".to_string(),
                    path: second.to_string_lossy().to_string(),
                },
            ]),
        )
        .expect("resolve meme annotations")
        .expect("segments");

        assert!(matches!(
            segments.first(),
            Some(PersistedInlineMessageSegment::Text { text }) if text == "前 "
        ));
        assert!(matches!(
            segments.get(1),
            Some(PersistedInlineMessageSegment::Meme { relative_path, .. })
                if relative_path.ends_with("坏笑.webp")
        ));
        assert!(matches!(
            segments.get(2),
            Some(PersistedInlineMessageSegment::Text { text }) if text == " 中 "
        ));
        assert!(matches!(
            segments.get(3),
            Some(PersistedInlineMessageSegment::Meme { relative_path, .. })
                if relative_path.ends_with("坏笑(2).webp")
        ));
        assert!(matches!(
            segments.get(4),
            Some(PersistedInlineMessageSegment::Text { text }) if text == " 后"
        ));
    }

    #[test]
    fn markdown_image_destination_should_support_file_url_and_angle_path() {
        assert_eq!(
            local_image_path_from_markdown_destination("<E:/tmp/result image.png>").as_deref(),
            Some("E:/tmp/result image.png")
        );
        assert_eq!(
            local_image_path_from_markdown_destination("file:///E:/tmp/result%20image.png").as_deref(),
            Some("E:/tmp/result image.png")
        );
    }

    #[test]
    fn markdown_image_destination_should_preserve_windows_backslash_separators() {
        assert_eq!(
            local_image_path_from_markdown_destination(r"E:\[tmp]\result.png").as_deref(),
            Some(r"E:\[tmp]\result.png")
        );
    }
}
