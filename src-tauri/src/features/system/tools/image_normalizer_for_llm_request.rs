// 该模块只用于“给 LLM 组请求体前”的图片规范化。
// 它不是通用图片存储模块，也不负责 path、消息持久化或历史回放语义。

// 缩放预算：总像素上限与单边上限。总像素限制整体分量，单边避免长图把长边拉得过长。
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET: u64 = 2_560_000;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_RESIZE_DIMENSION: u32 = 2048;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_WEBP_QUALITY: f32 = 80.0;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_BYTES: u64 = 50 * 1024 * 1024;
const IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION: u32 = 10_000;

#[derive(Debug, Clone, Copy)]
struct LlmRequestImageNormalizeOptions {
    /// 缩放后的总像素上限
    pixel_budget: u64,
    /// 缩放后的单边上限
    resize_dimension: u32,
    webp_quality: f32,
}

impl Default for LlmRequestImageNormalizeOptions {
    fn default() -> Self {
        Self {
            pixel_budget: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET,
            resize_dimension: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_RESIZE_DIMENSION,
            webp_quality: IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_WEBP_QUALITY,
        }
    }
}

#[derive(Debug, Clone)]
struct LlmRequestNormalizedImage {
    mime: String,
    bytes: Vec<u8>,
    source_width: u32,
    source_height: u32,
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

/// 判断这组尺寸是否已经落在预算内：单边与总像素都要满足。
fn llm_request_image_dimensions_fit(
    width: u32,
    height: u32,
    options: &LlmRequestImageNormalizeOptions,
) -> bool {
    u64::from(width) * u64::from(height) <= options.pixel_budget
        && width <= options.resize_dimension
        && height <= options.resize_dimension
}

/// 计算缩放目标尺寸：先按单边上限等比压，再按总像素等比压。
fn llm_request_image_output_dimensions(
    width: u32,
    height: u32,
    options: &LlmRequestImageNormalizeOptions,
) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    if llm_request_image_dimensions_fit(width, height, options) {
        return (width, height);
    }
    let dimension_scale =
        (f64::from(options.resize_dimension) / f64::from(width.max(height))).min(1.0);
    let width = ((f64::from(width) * dimension_scale).round() as u32).max(1);
    let height = ((f64::from(height) * dimension_scale).round() as u32).max(1);
    let pixel_scale = (options.pixel_budget as f64 / (f64::from(width) * f64::from(height)))
        .sqrt()
        .min(1.0);
    // 向下取整：四舍五入会让整数尺寸略微越过预算，floor 才能保证输出不超
    let out_width = ((f64::from(width) * pixel_scale).floor() as u32).max(1);
    let out_height = ((f64::from(height) * pixel_scale).floor() as u32).max(1);
    (out_width, out_height)
}

/// 尺寸检查后统一收口：缩放、可选叠加绘制、编码成 WebP。
/// 字节输入与 RGBA 输入共用这一条路径，差别只在进来之前如何解码。
fn llm_request_image_finish<F>(
    image: image::DynamicImage,
    options: LlmRequestImageNormalizeOptions,
    overlay: F,
) -> Result<LlmRequestNormalizedImage, String>
where
    F: FnOnce(&mut image::DynamicImage),
{
    let source_width = image.width();
    let source_height = image.height();
    let (target_width, target_height) =
        llm_request_image_output_dimensions(source_width, source_height, &options);
    let mut normalized = if target_width == source_width && target_height == source_height {
        image
    } else {
        image.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Triangle,
        )
    };
    // 叠加绘制排在缩放之后执行，绘制的线宽不会被缩放抹成半透明糊边（截图网格线依赖这一点）。
    overlay(&mut normalized);
    let encoder = webp::Encoder::from_image(&normalized)
        .map_err(|err| format!("初始化 WebP 编码器失败: {err}"))?;
    let webp = encoder.encode(options.webp_quality);
    Ok(LlmRequestNormalizedImage {
        mime: "image/webp".to_string(),
        bytes: (&*webp).to_vec(),
        source_width,
        source_height,
        output_width: normalized.width(),
        output_height: normalized.height(),
    })
}

fn llm_request_image_validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("图片尺寸无效，宽高必须大于 0。".to_string());
    }
    if width > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION
        || height > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION
    {
        return Err(format!(
            "图片分辨率过大（{}x{}），当前上限为 {}x{}；请缩小截取范围：operate 脚本里用 region=@x,y,w,h 只截目标区域、monitor=<id> 只截单块显示器，或用 window_id=<id> 只截目标窗口",
            width, height,
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION,
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_DIMENSION
        ));
    }
    Ok(())
}

/// 图片字节入口：解码、校验尺寸、缩放编码，返回压缩后的字节与前后分辨率。
fn normalize_image_bytes_for_llm_request(
    bytes: &[u8],
    declared_mime: Option<&str>,
) -> Result<LlmRequestNormalizedImage, String> {
    let options = LlmRequestImageNormalizeOptions::default();
    if bytes.len() as u64 > IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_BYTES {
        return Err(format!(
            "图片文件过大（{} bytes），当前上限为 {} bytes。",
            bytes.len(),
            IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_MAX_SOURCE_BYTES
        ));
    }
    let (_input_format, image_format) = llm_request_image_detect_format(bytes, declared_mime)?;
    let image = image::load_from_memory_with_format(bytes, image_format).map_err(|err| {
        format!("解码图片失败，文件可能已损坏或扩展名与内容不匹配: {err}")
    })?;
    llm_request_image_validate_dimensions(image.width(), image.height())?;
    llm_request_image_finish(image, options, |_| {})
}

/// RGBA 入口：调用方已持有解码后的像素，缩放与编码同上。
fn normalize_rgba_image_for_llm_request(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<LlmRequestNormalizedImage, String> {
    normalize_rgba_image_for_llm_request_with_overlay(
        rgba,
        width,
        height,
        LlmRequestImageNormalizeOptions::default(),
        |_| {},
    )
}

/// 同上，但允许在缩放完成、编码之前对图面做一次绘制作业，并可指定预算。
fn normalize_rgba_image_for_llm_request_with_overlay<F>(
    rgba: &[u8],
    width: u32,
    height: u32,
    options: LlmRequestImageNormalizeOptions,
    overlay: F,
) -> Result<LlmRequestNormalizedImage, String>
where
    F: FnOnce(&mut image::DynamicImage),
{
    llm_request_image_validate_dimensions(width, height)?;
    let rgba_image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
        format!(
            "构建 RGBA 图像失败：width={}, height={}, bytes_len={}",
            width,
            height,
            rgba.len()
        )
    })?;
    llm_request_image_finish(image::DynamicImage::ImageRgba8(rgba_image), options, overlay)
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
    assert!(normalized.output_width <= IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_RESIZE_DIMENSION);
    assert!(normalized.output_height <= IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_RESIZE_DIMENSION);
    assert!(
        u64::from(normalized.output_width) * u64::from(normalized.output_height)
            <= IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET
    );
}

#[cfg(test)]
#[test]
fn normalize_image_bytes_for_llm_request_should_keep_dimensions_inside_budget() {
    // 预算内的图不改尺寸，只做编码
    let bytes = image_normalizer_test_png(1600, 900);
    let normalized =
        normalize_image_bytes_for_llm_request(&bytes, Some("image/png")).expect("normalize png");
    assert_eq!(normalized.output_width, 1600);
    assert_eq!(normalized.output_height, 900);
}

#[cfg(test)]
#[test]
fn llm_request_image_output_dimensions_should_not_collapse_thin_images() {
    // 细长图缩到短边极小也不能塌成 0 或 1，否则画面整张丢失
    let options = LlmRequestImageNormalizeOptions {
        pixel_budget: 40_000,
        resize_dimension: 2048,
        ..LlmRequestImageNormalizeOptions::default()
    };
    for (width, height) in [(1920, 32), (1920, 45), (10000, 50), (1920, 1)] {
        let (out_width, out_height) =
            llm_request_image_output_dimensions(width, height, &options);
        assert!(
            out_width >= 1 && out_height >= 1,
            "{width}x{height} 不应塌缩，实际 {out_width}x{out_height}"
        );
        assert!(
            llm_request_image_dimensions_fit(out_width, out_height, &options),
            "{width}x{height} 输出 {out_width}x{out_height} 超出预算"
        );
    }
}

#[cfg(test)]
#[test]
fn llm_request_image_output_dimensions_should_stay_within_budget() {
    let options = LlmRequestImageNormalizeOptions::default();
    for (width, height) in [
        (4000, 2000),
        (3000, 1500),
        (600, 9000),
        (2048, 2048),
        (5000, 200),
    ] {
        let (out_width, out_height) =
            llm_request_image_output_dimensions(width, height, &options);
        assert!(
            llm_request_image_dimensions_fit(out_width, out_height, &options),
            "{width}x{height} 输出 {out_width}x{out_height} 超出预算"
        );
    }
}

#[cfg(test)]
#[test]
fn llm_request_image_output_dimensions_should_keep_images_inside_budget() {
    let options = LlmRequestImageNormalizeOptions::default();
    // 预算内的图不改尺寸
    assert_eq!(
        llm_request_image_output_dimensions(1600, 900, &options),
        (1600, 900)
    );
    assert_eq!(
        llm_request_image_output_dimensions(1024, 1024, &options),
        (1024, 1024)
    );
}

#[cfg(test)]
#[test]
fn normalize_image_bytes_for_llm_request_should_reject_corrupt_input() {
    let err = normalize_image_bytes_for_llm_request(b"not-an-image", Some("image/png"))
        .expect_err("should fail");
    assert!(err.contains("识别图片格式失败") || err.contains("解码图片失败"));
}
