// 该模块只用于“给 LLM 组请求体前”的图片规范化。
// 它不是通用图片存储模块，也不负责 path、消息持久化或历史回放语义。

const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET: u64 = 1080 * 1080;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_WEBP_QUALITY: f32 = 80.0;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_BYTES: u64 = 50 * 1024 * 1024;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION: u32 = 10_000;

#[derive(Debug, Clone, Copy)]
struct LlmRequestImageNormalizeOptions {
    target_pixel_budget: u64,
    webp_quality: f32,
    max_source_bytes: u64,
    max_dimension: u32,
}

impl Default for LlmRequestImageNormalizeOptions {
    fn default() -> Self {
        Self {
            target_pixel_budget: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET,
            webp_quality: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_WEBP_QUALITY,
            max_source_bytes: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_BYTES,
            max_dimension: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_DIMENSION,
        }
    }
}

#[derive(Debug, Clone)]
struct LlmRequestNormalizedImage {
    mime: String,
    bytes: Vec<u8>,
    output_width: u32,
    output_height: u32,
}

#[derive(Debug, Clone, Copy)]
enum LlmRequestImageInputFormat {
    Jpeg,
    Png,
    Gif,
    Webp,
    Bmp,
}

fn llm_request_image_normalize_options_default() -> LlmRequestImageNormalizeOptions {
    LlmRequestImageNormalizeOptions::default()
}

fn llm_request_image_detect_format(
    bytes: &[u8],
    declared_mime: Option<&str>,
) -> Result<(LlmRequestImageInputFormat, image::ImageFormat), String> {
    let guessed = image::guess_format(bytes).map_err(|err| {
        format!("识别图片格式失败，文件可能已损坏或扩展名与内容不匹配: {err}")
    })?;
    let input = match guessed {
        image::ImageFormat::Jpeg => LlmRequestImageInputFormat::Jpeg,
        image::ImageFormat::Png => LlmRequestImageInputFormat::Png,
        image::ImageFormat::Gif => LlmRequestImageInputFormat::Gif,
        image::ImageFormat::WebP => LlmRequestImageInputFormat::Webp,
        image::ImageFormat::Bmp => LlmRequestImageInputFormat::Bmp,
        other => {
            return Err(format!(
                "图片规范化暂不支持该格式，declared_mime={}，detected_format={other:?}",
                declared_mime.unwrap_or("application/octet-stream")
            ))
        }
    };
    Ok((input, guessed))
}

fn llm_request_image_output_dimensions(
    width: u32,
    height: u32,
    target_pixel_budget: u64,
) -> (u32, u32) {
    let total_pixels = u64::from(width) * u64::from(height);
    if total_pixels == 0 || total_pixels <= target_pixel_budget {
        return (width.max(1), height.max(1));
    }
    let scale = (target_pixel_budget as f64 / total_pixels as f64).sqrt();
    let mut out_width = ((width as f64) * scale).floor().max(1.0) as u32;
    let mut out_height = ((height as f64) * scale).floor().max(1.0) as u32;
    while u64::from(out_width) * u64::from(out_height) > target_pixel_budget {
        if out_width >= out_height && out_width > 1 {
            out_width -= 1;
        } else if out_height > 1 {
            out_height -= 1;
        } else {
            break;
        }
    }
    (out_width.max(1), out_height.max(1))
}

fn llm_request_image_validate_dimensions(
    width: u32,
    height: u32,
    options: &LlmRequestImageNormalizeOptions,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("图片尺寸无效，宽高必须大于 0。".to_string());
    }
    if width > options.max_dimension || height > options.max_dimension {
        return Err(format!(
            "图片分辨率过大（{}x{}），当前上限为 {}x{}；请缩小截取范围：operate 脚本里用 region=@x,y,w,h 只截目标区域、monitor=<id> 只截单块显示器，或用 window_id=<id> 只截目标窗口",
            width, height, options.max_dimension, options.max_dimension
        ));
    }
    Ok(())
}

fn normalize_image_bytes_for_llm_request_with_options(
    bytes: &[u8],
    declared_mime: Option<&str>,
    options: LlmRequestImageNormalizeOptions,
) -> Result<LlmRequestNormalizedImage, String> {
    if bytes.len() as u64 > options.max_source_bytes {
        return Err(format!(
            "图片文件过大（{} bytes），当前上限为 {} bytes。",
            bytes.len(),
            options.max_source_bytes
        ));
    }
    let (_input_format, image_format) = llm_request_image_detect_format(bytes, declared_mime)?;
    let image = image::load_from_memory_with_format(bytes, image_format).map_err(|err| {
        format!("解码图片失败，文件可能已损坏或扩展名与内容不匹配: {err}")
    })?;
    let original_width = image.width();
    let original_height = image.height();
    llm_request_image_validate_dimensions(original_width, original_height, &options)?;

    let (target_width, target_height) = llm_request_image_output_dimensions(
        original_width,
        original_height,
        options.target_pixel_budget,
    );
    let normalized = if target_width == original_width && target_height == original_height {
        image
    } else {
        image.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    };
    let encoder = webp::Encoder::from_image(&normalized)
        .map_err(|err| format!("初始化 WebP 编码器失败: {err}"))?;
    let webp = encoder.encode(options.webp_quality);
    Ok(LlmRequestNormalizedImage {
        mime: "image/webp".to_string(),
        bytes: (&*webp).to_vec(),
        output_width: normalized.width(),
        output_height: normalized.height(),
    })
}

fn normalize_image_bytes_for_llm_request(
    bytes: &[u8],
    declared_mime: Option<&str>,
) -> Result<LlmRequestNormalizedImage, String> {
    normalize_image_bytes_for_llm_request_with_options(
        bytes,
        declared_mime,
        llm_request_image_normalize_options_default(),
    )
}

fn normalize_rgba_image_for_llm_request_with_options(
    rgba: &[u8],
    width: u32,
    height: u32,
    options: LlmRequestImageNormalizeOptions,
) -> Result<LlmRequestNormalizedImage, String> {
    normalize_rgba_image_for_llm_request_with_overlay(rgba, width, height, options, |_| {})
}

/// 同上，但允许在缩放完成、编码之前对图面做一次绘制作业。
/// 叠加在缩放后执行：绘制的线宽不会被缩放抹成半透明糊边（截图网格线依赖这一点）。
fn normalize_rgba_image_for_llm_request_with_overlay<F>(
    rgba: &[u8],
    width: u32,
    height: u32,
    options: LlmRequestImageNormalizeOptions,
    overlay: F,
) -> Result<LlmRequestNormalizedImage, String>
where
    F: FnOnce(&mut image::RgbaImage),
{
    llm_request_image_validate_dimensions(width, height, &options)?;
    let rgba_image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
        format!(
            "构建 RGBA 图像失败：width={}, height={}, bytes_len={}",
            width,
            height,
            rgba.len()
        )
    })?;
    let dynamic = image::DynamicImage::ImageRgba8(rgba_image);
    let (target_width, target_height) =
        llm_request_image_output_dimensions(width, height, options.target_pixel_budget);
    let mut normalized = if target_width == width && target_height == height {
        dynamic
    } else {
        dynamic.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    };
    // 输入与缩放结果恒为 ImageRgba8，as_mut_rgba8 必然有值；用 if let 而非错误分支，
    // 避免为不可能发生的场景制造错误路径。
    if let Some(canvas) = normalized.as_mut_rgba8() {
        overlay(canvas);
    }
    let encoder = webp::Encoder::from_image(&normalized)
        .map_err(|err| format!("初始化 WebP 编码器失败: {err}"))?;
    let webp = encoder.encode(options.webp_quality);
    Ok(LlmRequestNormalizedImage {
        mime: "image/webp".to_string(),
        bytes: (&*webp).to_vec(),
        output_width: normalized.width(),
        output_height: normalized.height(),
    })
}

fn normalize_rgba_image_for_llm_request(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<LlmRequestNormalizedImage, String> {
    normalize_rgba_image_for_llm_request_with_options(
        rgba,
        width,
        height,
        llm_request_image_normalize_options_default(),
    )
}

#[cfg(test)]
fn image_normalizer_test_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([
            (x % 255) as u8,
            (y % 255) as u8,
            ((x + y) % 255) as u8,
        ])
    }));
    let mut out = Vec::<u8>::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    encoder.encode_image(&image).expect("encode jpeg");
    out
}

#[cfg(test)]
fn image_normalizer_test_noisy_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
        let seed = (u64::from(x) * 1_103_515_245)
            ^ (u64::from(y) * 12_345_679)
            ^ ((u64::from(x) + u64::from(y)) * 2_654_435_761);
        image::Rgb([
            (seed & 0xff) as u8,
            ((seed >> 8) & 0xff) as u8,
            ((seed >> 16) & 0xff) as u8,
        ])
    }));
    let mut out = Vec::<u8>::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    encoder.encode_image(&image).expect("encode noisy jpeg");
    out
}

#[cfg(test)]
fn image_normalizer_test_png(width: u32, height: u32) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([
            (x % 255) as u8,
            (y % 255) as u8,
            ((x * 2 + y) % 255) as u8,
            255,
        ])
    }));
    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .expect("encode png");
    cursor.into_inner()
}

#[cfg(test)]
#[test]
fn normalize_image_bytes_for_llm_request_should_encode_small_jpeg_as_webp() {
    let bytes = image_normalizer_test_jpeg(512, 512, 80);
    let normalized =
        normalize_image_bytes_for_llm_request(&bytes, Some("image/jpeg")).expect("normalize jpeg");
    assert_eq!(normalized.mime, "image/webp");
    assert_eq!(normalized.output_width, 512);
    assert_eq!(normalized.output_height, 512);
}

#[cfg(test)]
#[test]
fn normalize_image_bytes_for_llm_request_should_compress_large_jpeg_within_budget() {
    let bytes = image_normalizer_test_noisy_jpeg(1166, 1000, 100);
    let normalized =
        normalize_image_bytes_for_llm_request(&bytes, Some("image/jpeg")).expect("normalize jpeg");
    assert_eq!(normalized.mime, "image/webp");
}

#[cfg(test)]
#[test]
fn normalize_image_bytes_for_llm_request_should_compress_when_pixel_budget_exceeded() {
    let bytes = image_normalizer_test_png(3000, 1500);
    let normalized =
        normalize_image_bytes_for_llm_request(&bytes, Some("image/png")).expect("normalize png");
    assert_eq!(normalized.mime, "image/webp");
    assert!(
        u64::from(normalized.output_width) * u64::from(normalized.output_height)
            <= IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET
    );
}

#[cfg(test)]
#[test]
fn normalize_image_bytes_for_llm_request_should_reject_corrupt_input() {
    let err = normalize_image_bytes_for_llm_request(b"not-an-image", Some("image/png"))
        .expect_err("should fail");
    assert!(err.contains("识别图片格式失败") || err.contains("解码图片失败"));
}
