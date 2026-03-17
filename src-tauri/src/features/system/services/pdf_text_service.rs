use rayon::prelude::*;

const MAX_PDF_CONVERT_PAGES: usize = 100;
const MAX_PDF_TEXT_TOKENS: usize = 30_000;
const PDF_PAGE_LIMIT_ERR_PREFIX: &str = "pdf_page_limit_exceeded";

static PDF_SESSION_MEMORY_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::collections::HashMap<String, PdfExtractStructuredResult>>>,
> = std::sync::OnceLock::new();

fn pdf_session_memory_cache(
) -> &'static std::sync::Mutex<
    std::collections::HashMap<String, std::collections::HashMap<String, PdfExtractStructuredResult>>,
> {
    PDF_SESSION_MEMORY_CACHE.get_or_init(|| {
        std::sync::Mutex::new(std::collections::HashMap::new())
    })
}

fn build_pdf_session_cache_key(file_path: &str, include_images: bool) -> Result<String, String> {
    let meta = std::fs::metadata(file_path)
        .map_err(|err| format!("stat pdf failed: {err}"))?;
    let size = meta.len();
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok(format!(
        "v2|path={}|size={}|mtime_nanos={}|mode={}",
        file_path,
        size,
        modified,
        if include_images { "image" } else { "text" }
    ))
}

fn get_pdf_session_cached_result(
    conversation_id: &str,
    cache_key: &str,
) -> Option<PdfExtractStructuredResult> {
    let guard = pdf_session_memory_cache().lock().ok()?;
    guard
        .get(conversation_id)
        .and_then(|items| items.get(cache_key))
        .cloned()
}

fn put_pdf_session_cached_result(
    conversation_id: &str,
    cache_key: String,
    result: PdfExtractStructuredResult,
) {
    if let Ok(mut guard) = pdf_session_memory_cache().lock() {
        guard
            .entry(conversation_id.to_string())
            .or_insert_with(std::collections::HashMap::new)
            .insert(cache_key, result);
    }
}

pub(crate) fn cleanup_pdf_session_memory_cache_for_conversation(conversation_id: &str) {
    if let Ok(mut guard) = pdf_session_memory_cache().lock() {
        guard.remove(conversation_id);
    }
}

pub(crate) fn is_pdf_page_limit_exceeded_error(err: &str) -> bool {
    err.starts_with(PDF_PAGE_LIMIT_ERR_PREFIX)
}

fn pdf_page_limit_error(total_pages: usize) -> String {
    format!(
        "{}: total_pages={}, limit={}",
        PDF_PAGE_LIMIT_ERR_PREFIX, total_pages, MAX_PDF_CONVERT_PAGES
    )
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPageExtractBlock {
    pub page_index: usize,
    pub text: String,
    pub images: Vec<PdfRenderedImage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfExtractStructuredResult {
    pub file_name: String,
    pub total_pages: u32,
    pub include_images: bool,
    pub pages: Vec<PdfPageExtractBlock>,
}

fn extract_pdf_page_text(doc: &mut pdf_oxide::api::Pdf, page_index: usize) -> Result<String, String> {
    let lines = doc
        .extract_text_lines(page_index)
        .map_err(|err| format!("pdf_oxide extract_text_lines(page={}) failed: {}", page_index, err))?;
    let raw = lines
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(normalize_pdf_page_text_once(&raw))
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
    )
}

fn normalize_pdf_page_text_once(input: &str) -> String {
    let chars = input
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n'))
        .collect::<Vec<_>>();
    let mut out = String::with_capacity(chars.len());
    for (idx, ch) in chars.iter().enumerate() {
        if ch.is_whitespace() {
            let mut left = idx;
            while left > 0 && chars[left - 1].is_whitespace() {
                left -= 1;
            }
            let mut right = idx + 1;
            while right < chars.len() && chars[right].is_whitespace() {
                right += 1;
            }
            if left > 0
                && right < chars.len()
                && is_cjk_char(chars[left - 1])
                && is_cjk_char(chars[right])
            {
                continue;
            }
        }
        out.push(*ch);
    }
    out
}

fn truncate_pdf_pages_to_token_limit(
    pages: &mut Vec<PdfPageExtractBlock>,
    token_limit: usize,
) -> Result<(), String> {
    static TOKEN_BPE: std::sync::OnceLock<Option<tiktoken_rs::CoreBPE>> = std::sync::OnceLock::new();
    let Some(bpe) = TOKEN_BPE
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
    else {
        eprintln!("[PDF提取] tiktoken 初始化失败，跳过 30K token 截断。");
        return Ok(());
    };

    if token_limit == 0 {
        pages.clear();
        return Ok(());
    }

    let mut remaining = token_limit;
    let mut kept = Vec::<PdfPageExtractBlock>::with_capacity(pages.len());
    for mut page in pages.drain(..) {
        if remaining == 0 {
            break;
        }
        if page.text.trim().is_empty() {
            kept.push(page);
            continue;
        }
        let tokens = bpe.encode_with_special_tokens(&page.text);
        if tokens.len() <= remaining {
            remaining -= tokens.len();
            kept.push(page);
            continue;
        }
        let truncated = bpe
            .decode(tokens[..remaining].to_vec())
            .map_err(|err| format!("tiktoken decode failed: {err}"))?;
        page.text = truncated;
        kept.push(page);
        break;
    }
    *pages = kept;
    Ok(())
}

fn encode_rgba_page_to_webp(
    page_index: usize,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<PdfRenderedImage, String> {
    let rgba_image = image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
        format!(
            "build rgba image failed(page={}): width={}, height={}",
            page_index, width, height
        )
    })?;
    let dyn_img = image::DynamicImage::ImageRgba8(rgba_image);
    let encoder = webp::Encoder::from_image(&dyn_img)
        .map_err(|err| format!("webp encoder init failed(page={}): {}", page_index, err))?;
    let webp = encoder.encode(50.0);
    let webp_bytes: &[u8] = webp.as_ref();
    Ok(PdfRenderedImage {
        page_index,
        width,
        height,
        bytes_base64: B64.encode(webp_bytes),
        mime: "image/webp".to_string(),
    })
}

fn render_pdf_pages_as_webp_with_hayro(file_path: &str) -> Result<(usize, Vec<PdfRenderedImage>), String> {
    let file_bytes: hayro::hayro_syntax::PdfData = std::sync::Arc::new(
        std::fs::read(file_path).map_err(|err| format!("read pdf failed: {err}"))?,
    );
    let pdf = hayro::hayro_syntax::Pdf::new(file_bytes)
        .map_err(|err| format!("hayro open failed: {err:?}"))?;
    let pages = pdf.pages();
    let total_pages = pages.len();
    if total_pages > MAX_PDF_CONVERT_PAGES {
        return Err(pdf_page_limit_error(total_pages));
    }

    let interpreter_settings = hayro::hayro_interpret::InterpreterSettings::default();
    let render_settings = hayro::RenderSettings {
        x_scale: 1.0,
        y_scale: 1.0,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        ..Default::default()
    };

    let rendered_images = pages
        .par_iter()
        .enumerate()
        .map(|(page_index, page)| {
            let pixmap = hayro::render(page, &interpreter_settings, &render_settings);
            let width = pixmap.width() as u32;
            let height = pixmap.height() as u32;
            let rgba = pixmap.data_as_u8_slice().to_vec();
            encode_rgba_page_to_webp(page_index, width, height, rgba)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((total_pages, rendered_images))
}

/// 按页提取 PDF 内容，返回结构化结果：
/// 每页一个 text block，并可选追加该页 image blocks。
pub(crate) fn get_or_extract_pdf_structured(
    _state: &AppState,
    conversation_id: &str,
    file_path: &str,
    include_images: bool,
) -> Result<PdfExtractStructuredResult, String> {
    let cache_key = build_pdf_session_cache_key(file_path, include_images)?;
    if let Some(cached) = get_pdf_session_cached_result(conversation_id, &cache_key) {
        return Ok(cached);
    }

    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    if include_images {
        let (total_pages, rendered_images) = render_pdf_pages_as_webp_with_hayro(file_path)?;
        let pages = rendered_images
            .into_iter()
            .map(|img| PdfPageExtractBlock {
                page_index: img.page_index,
                text: String::new(),
                images: vec![img],
            })
            .collect::<Vec<_>>();
        let result = PdfExtractStructuredResult {
            file_name,
            total_pages: total_pages as u32,
            include_images: true,
            pages,
        };
        put_pdf_session_cached_result(conversation_id, cache_key, result.clone());
        return Ok(result);
    }

    let mut doc = pdf_oxide::api::Pdf::open(Path::new(file_path))
        .map_err(|err| format!("pdf_oxide open failed: {}", err))?;
    let page_count = doc
        .page_count()
        .map_err(|err| format!("pdf_oxide page_count failed: {}", err))?;
    if page_count > MAX_PDF_CONVERT_PAGES {
        return Err(pdf_page_limit_error(page_count));
    }

    let mut pages = Vec::<PdfPageExtractBlock>::new();
    for page_index in 0..page_count {
        let text = extract_pdf_page_text(&mut doc, page_index)?;
        pages.push(PdfPageExtractBlock {
            page_index,
            text,
            images: Vec::new(),
        });
    }
    truncate_pdf_pages_to_token_limit(&mut pages, MAX_PDF_TEXT_TOKENS)?;

    let result = PdfExtractStructuredResult {
        file_name,
        total_pages: page_count as u32,
        include_images: false,
        pages,
    };
    put_pdf_session_cached_result(conversation_id, cache_key, result.clone());
    Ok(result)
}

/// 清理对话关联的PDF缓存（兼容旧缓存结构，文本/图片都清）
pub(crate) fn cleanup_pdf_cache_for_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    cleanup_pdf_session_memory_cache_for_conversation(conversation_id);
    let mut data = state_read_app_data_cached(state)?;
    let original_text_len = data.pdf_text_cache.len();
    let original_image_len = data.pdf_image_cache.len();

    data.pdf_text_cache.retain_mut(|entry| {
        entry.conversation_ids.retain(|id| id != conversation_id);
        if entry.conversation_ids.is_empty() {
            eprintln!("[PDF缓存清理] 删除文本缓存条目 file={}, hash={}", entry.file_name, entry.file_hash);
            false
        } else {
            entry.updated_at = chrono::Utc::now().to_rfc3339();
            true
        }
    });

    data.pdf_image_cache.retain_mut(|entry| {
        entry.conversation_ids.retain(|id| id != conversation_id);
        if entry.conversation_ids.is_empty() {
            eprintln!("[PDF缓存清理] 删除图片缓存条目 file={}, hash={}", entry.file_name, entry.file_hash);
            false
        } else {
            entry.updated_at = chrono::Utc::now().to_rfc3339();
            true
        }
    });

    let removed_text = original_text_len - data.pdf_text_cache.len();
    let removed_image = original_image_len - data.pdf_image_cache.len();
    if removed_text > 0 || removed_image > 0 {
        eprintln!(
            "[PDF缓存清理] 完成 conversation_id={}, removed_text={}, removed_image={}",
            conversation_id, removed_text, removed_image
        );
    }

    state_write_app_data_cached(state, &data)?;
    Ok(())
}
