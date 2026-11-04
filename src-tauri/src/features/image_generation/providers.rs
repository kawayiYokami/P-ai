fn image_provider_key_cursor_state() -> &'static Mutex<std::collections::HashMap<String, usize>> {
    static CURSORS: OnceLock<Mutex<std::collections::HashMap<String, usize>>> = OnceLock::new();
    CURSORS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn select_image_generation_api_key(provider: &ImageGenerationProviderConfig) -> String {
    let keys = provider
        .api_keys
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return String::new();
    }
    let Ok(mut guard) = image_provider_key_cursor_state().lock() else {
        return keys[(provider.key_cursor as usize) % keys.len()].to_string();
    };
    let cursor = guard
        .entry(provider.id.clone())
        .or_insert((provider.key_cursor as usize) % keys.len());
    let selected = keys[*cursor % keys.len()].to_string();
    *cursor = (*cursor + 1) % keys.len();
    selected
}

fn trimmed_image_generation_option(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn effective_image_generation_size(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Option<String> {
    trimmed_image_generation_option(&request.size)
        .or_else(|| trimmed_image_generation_option(&model.default_size))
}

fn effective_image_generation_aspect_ratio(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Option<String> {
    trimmed_image_generation_option(&request.aspect_ratio)
        .or_else(|| trimmed_image_generation_option(&model.default_aspect_ratio))
}

fn effective_image_generation_quality(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Option<String> {
    trimmed_image_generation_option(&request.quality)
        .or_else(|| trimmed_image_generation_option(&model.default_quality))
}

fn effective_image_generation_prompt(request: &ImageGenerationRequest) -> String {
    let prompt = request.prompt.trim();
    let Some(negative_prompt) = request
        .negative_prompt
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return prompt.to_string();
    };
    format!("{prompt}\n\n请避免出现以下内容：{negative_prompt}")
}

fn parse_pixel_size(value: &str) -> Option<(u32, u32)> {
    let normalized = value.trim().to_ascii_lowercase().replace('×', "x");
    let (width, height) = normalized.split_once('x')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn aspect_ratio_from_dimensions(width: u32, height: u32) -> String {
    let divisor = greatest_common_divisor(width, height);
    format!("{}:{}", width / divisor, height / divisor)
}

fn parse_aspect_ratio(value: &str) -> Option<(u32, u32)> {
    let (width, height) = value.trim().split_once(':')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

fn openai_size_from_aspect_ratio(value: &str, arbitrary_size: bool) -> Option<String> {
    let (width, height) = parse_aspect_ratio(value)?;
    let ratio = f64::from(width) / f64::from(height);
    if arbitrary_size {
        if !(1.0 / 3.0..=3.0).contains(&ratio) {
            return None;
        }
        if (0.87..=1.15).contains(&ratio) {
            return Some("1024x1024".to_string());
        }
        let rounded_to_16 = |value: f64| -> u32 {
            (((value / 16.0).round() as u32).max(1) * 16).clamp(512, 1536)
        };
        return if ratio > 1.0 {
            Some(format!("1536x{}", rounded_to_16(1536.0 / ratio)))
        } else {
            Some(format!("{}x1536", rounded_to_16(1536.0 * ratio)))
        };
    }
    Some(if ratio > 1.15 {
        "1536x1024".to_string()
    } else if ratio < 0.87 {
        "1024x1536".to_string()
    } else {
        "1024x1024".to_string()
    })
}

fn append_image_generation_endpoint(base_url: &str, suffix: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let suffix = suffix.trim();
    let normalized_suffix = if suffix.starts_with('/') {
        suffix.to_string()
    } else {
        format!("/{suffix}")
    };
    if base
        .to_ascii_lowercase()
        .ends_with(&normalized_suffix.to_ascii_lowercase())
    {
        base.to_string()
    } else {
        format!("{base}{normalized_suffix}")
    }
}

fn openai_image_generation_payload(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Value {
    let mut payload = serde_json::json!({
        "model": model.model,
        "prompt": effective_image_generation_prompt(request),
        "n": 1
    });
    let is_gpt_image = model.model.trim().to_ascii_lowercase().starts_with("gpt-image");
    let supports_arbitrary_size = model
        .model
        .trim()
        .to_ascii_lowercase()
        .starts_with("gpt-image-2");
    if let Some(object) = payload.as_object_mut() {
        if is_gpt_image {
            object.insert("output_format".to_string(), Value::String("png".to_string()));
        } else {
            object.insert(
                "response_format".to_string(),
                Value::String("b64_json".to_string()),
            );
        }
        let size = trimmed_image_generation_option(&request.size)
            .or_else(|| {
                trimmed_image_generation_option(&request.aspect_ratio)
                    .as_deref()
                    .and_then(|value| openai_size_from_aspect_ratio(value, supports_arbitrary_size))
            })
            .or_else(|| trimmed_image_generation_option(&model.default_size))
            .or_else(|| {
                trimmed_image_generation_option(&model.default_aspect_ratio)
                    .as_deref()
                    .and_then(|value| openai_size_from_aspect_ratio(value, supports_arbitrary_size))
            });
        if let Some(size) = size {
            object.insert("size".to_string(), Value::String(size));
        }
        if let Some(quality) = effective_image_generation_quality(request, model) {
            object.insert("quality".to_string(), Value::String(quality));
        }
    }
    payload
}

fn xai_resolution_from_request(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Option<String> {
    for candidate in [
        effective_image_generation_size(request, model),
        effective_image_generation_quality(request, model),
    ]
    .into_iter()
    .flatten()
    {
        let normalized = candidate.trim().to_ascii_lowercase();
        if matches!(normalized.as_str(), "1k" | "2k") {
            return Some(normalized);
        }
        if let Some((width, height)) = parse_pixel_size(&normalized) {
            return Some(if width.max(height) > 1536 { "2k" } else { "1k" }.to_string());
        }
    }
    None
}

fn xai_image_generation_payload(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Value {
    let mut payload = serde_json::json!({
        "model": model.model,
        "prompt": effective_image_generation_prompt(request),
        "n": 1,
        "response_format": "b64_json"
    });
    if let Some(object) = payload.as_object_mut() {
        let aspect_ratio = effective_image_generation_aspect_ratio(request, model).or_else(|| {
            effective_image_generation_size(request, model)
                .as_deref()
                .and_then(parse_pixel_size)
                .map(|(width, height)| aspect_ratio_from_dimensions(width, height))
        });
        if let Some(aspect_ratio) = aspect_ratio {
            object.insert("aspect_ratio".to_string(), Value::String(aspect_ratio));
        }
        if let Some(resolution) = xai_resolution_from_request(request, model) {
            object.insert("resolution".to_string(), Value::String(resolution));
        }
    }
    payload
}

fn seedream_image_generation_payload(
    request: &ImageGenerationRequest,
    provider: &ImageGenerationProviderConfig,
    model: &ImageGenerationModelConfig,
) -> Value {
    let model_name = model.model.trim().to_ascii_lowercase();
    let aspect_ratio = effective_image_generation_aspect_ratio(request, model);
    let size = trimmed_image_generation_option(&request.size).or_else(|| {
        let default_size = trimmed_image_generation_option(&model.default_size)?;
        if aspect_ratio.is_none() || parse_pixel_size(&default_size).is_none() {
            return Some(default_size);
        }
        if model_name.contains("seedream-5-0-pro") {
            return Some("2K".to_string());
        }
        let longest_edge = parse_pixel_size(&default_size)
            .map(|(width, height)| width.max(height))
            .unwrap_or(2048);
        Some(if longest_edge > 3072 {
            "4K"
        } else if longest_edge > 2048 {
            "3K"
        } else {
            "2K"
        }
        .to_string())
    });
    let mut prompt = effective_image_generation_prompt(request);
    if size.as_deref().and_then(parse_pixel_size).is_none() {
        if let Some(aspect_ratio) = aspect_ratio {
            prompt.push_str(&format!("\n\n画面宽高比：{aspect_ratio}"));
        }
    }
    let mut payload = serde_json::json!({
        "model": model.model,
        "prompt": prompt,
        "response_format": "b64_json",
        "watermark": provider.watermark
    });
    if let Some(object) = payload.as_object_mut() {
        if model_name.contains("seedream-5-0") {
            object.insert(
                "output_format".to_string(),
                Value::String("png".to_string()),
            );
        }
        if let Some(size) = size {
            object.insert("size".to_string(), Value::String(size));
        }
        if let Some(quality) = effective_image_generation_quality(request, model) {
            let mode = quality.trim().to_ascii_lowercase();
            let supports_prompt_optimization = model_name.contains("seedream-5-0")
                || model_name.contains("seedream-4-5")
                || model_name.contains("seedream-4-0");
            let supports_fast = model_name.contains("seedream-5-0-pro")
                || model_name.contains("seedream-4-0");
            let supports_mode = supports_prompt_optimization
                && (mode == "standard" || (mode == "fast" && supports_fast));
            if supports_mode {
                object.insert(
                    "optimize_prompt_options".to_string(),
                    serde_json::json!({ "mode": mode }),
                );
            }
        }
    }
    payload
}

fn gemini_image_generation_payload(
    request: &ImageGenerationRequest,
    model: &ImageGenerationModelConfig,
) -> Value {
    let mut response_format = serde_json::Map::<String, Value>::new();
    response_format.insert("type".to_string(), Value::String("image".to_string()));
    response_format.insert(
        "mime_type".to_string(),
        Value::String("image/png".to_string()),
    );
    let size = effective_image_generation_size(request, model);
    let aspect_ratio = effective_image_generation_aspect_ratio(request, model).or_else(|| {
        size.as_deref()
            .and_then(parse_pixel_size)
            .map(|(width, height)| aspect_ratio_from_dimensions(width, height))
    });
    if let Some(aspect_ratio) = aspect_ratio {
        response_format.insert("aspect_ratio".to_string(), Value::String(aspect_ratio));
    }
    if let Some(size) = size {
        let normalized = size.trim().to_ascii_uppercase();
        if matches!(normalized.as_str(), "512" | "1K" | "2K" | "4K") {
            response_format.insert("image_size".to_string(), Value::String(normalized));
        }
    }
    serde_json::json!({
        "model": model.model,
        "input": [{
            "type": "text",
            "text": effective_image_generation_prompt(request)
        }],
        "response_format": Value::Object(response_format)
    })
}

fn image_generation_value_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn parse_openai_style_image_response(value: &Value) -> Result<ProviderImageGenerationOutput, String> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "供应商响应缺少 data 图片数组".to_string())?;
    let mut images = Vec::<PendingGeneratedImage>::new();
    let mut item_errors = Vec::<String>::new();
    for item in data {
        if let Some(error) = item.get("error") {
            let message = image_generation_value_string(error, &["message", "code"])
                .unwrap_or("图片生成失败")
                .to_string();
            item_errors.push(message);
            continue;
        }
        let revised_prompt = image_generation_value_string(item, &["revised_prompt", "revisedPrompt"])
            .map(ToOwned::to_owned);
        if let Some(encoded) = image_generation_value_string(item, &["b64_json", "b64Json"]) {
            images.push(PendingGeneratedImage {
                source: PendingImageSource::Bytes(decode_generated_image_base64(encoded)?),
                mime_hint: None,
                remote_url: None,
                revised_prompt,
            });
            continue;
        }
        if let Some(url) = item.get("url").and_then(Value::as_str) {
            let url = url.trim().to_string();
            if !url.is_empty() {
                images.push(PendingGeneratedImage {
                    source: PendingImageSource::RemoteUrl(url.clone()),
                    mime_hint: None,
                    remote_url: Some(url),
                    revised_prompt,
                });
            }
        }
    }
    if images.is_empty() {
        return Err(if item_errors.is_empty() {
            "供应商响应中没有可用图片".to_string()
        } else {
            format!("供应商未返回可用图片：{}", item_errors.join("；"))
        });
    }
    Ok(ProviderImageGenerationOutput { images, text: None })
}

fn parse_gemini_image_response(value: &Value) -> Result<ProviderImageGenerationOutput, String> {
    let mut images = Vec::<PendingGeneratedImage>::new();
    let mut text_parts = Vec::<String>::new();
    if let Some(steps) = value.get("steps").and_then(Value::as_array) {
        for step in steps {
            let Some(contents) = step.get("content").and_then(Value::as_array) else {
                continue;
            };
            for content in contents {
                if let Some(text) = content.get("text").and_then(Value::as_str) {
                    let text = text.trim();
                    if !text.is_empty() {
                        text_parts.push(text.to_string());
                    }
                }
                if content.get("type").and_then(Value::as_str) != Some("image") {
                    continue;
                }
                let Some(encoded) = content.get("data").and_then(Value::as_str) else {
                    continue;
                };
                let mime_hint = image_generation_value_string(content, &["mime_type", "mimeType"])
                    .map(ToOwned::to_owned);
                images.push(PendingGeneratedImage {
                    source: PendingImageSource::Bytes(decode_generated_image_base64(encoded)?),
                    mime_hint,
                    remote_url: None,
                    revised_prompt: None,
                });
            }
        }
        if !images.is_empty() {
            return Ok(ProviderImageGenerationOutput {
                images,
                text: (!text_parts.is_empty()).then(|| text_parts.join("\n")),
            });
        }
    }

    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "Gemini 响应缺少 steps 或 candidates".to_string())?;
    for candidate in candidates {
        let Some(parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                let text = text.trim();
                if !text.is_empty() {
                    text_parts.push(text.to_string());
                }
            }
            let inline_data = part.get("inlineData").or_else(|| part.get("inline_data"));
            let Some(inline_data) = inline_data else {
                continue;
            };
            let Some(encoded) = inline_data.get("data").and_then(Value::as_str) else {
                continue;
            };
            let mime_hint = image_generation_value_string(inline_data, &["mimeType", "mime_type"])
                .map(ToOwned::to_owned);
            images.push(PendingGeneratedImage {
                source: PendingImageSource::Bytes(decode_generated_image_base64(encoded)?),
                mime_hint,
                remote_url: None,
                revised_prompt: None,
            });
        }
    }
    if images.is_empty() {
        let provider_text = text_parts.join("\n");
        return Err(if provider_text.is_empty() {
            "Gemini 未返回图片数据".to_string()
        } else {
            format!("Gemini 未返回图片数据：{provider_text}")
        });
    }
    Ok(ProviderImageGenerationOutput {
        images,
        text: (!text_parts.is_empty()).then(|| text_parts.join("\n")),
    })
}

async fn post_bearer_image_generation_json(
    state: &AppState,
    provider: &ImageGenerationProviderConfig,
    api_key: &str,
    endpoint: &str,
    payload: &Value,
) -> Result<Value, String> {
    let response = state
        .shared_http_client
        .post(endpoint)
        .bearer_auth(api_key)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(payload)
        .timeout(std::time::Duration::from_secs(u64::from(provider.timeout_seconds)))
        .send()
        .await
        .map_err(|err| format!("{} 请求失败：{err}", provider.name))?;
    parse_image_generation_json_response(response, &provider.name).await
}

async fn generate_openai_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    api_key: &str,
) -> Result<ProviderImageGenerationOutput, String> {
    let endpoint = append_image_generation_endpoint(&resolved.provider.base_url, "/images/generations");
    let payload = openai_image_generation_payload(request, &resolved.model);
    let value = post_bearer_image_generation_json(
        state,
        &resolved.provider,
        api_key,
        &endpoint,
        &payload,
    )
    .await?;
    parse_openai_style_image_response(&value)
}

// ==================== Grok 视频首帧体积收敛 ====================
// cli-chat-proxy 网关对请求体有 1 MiB 长度上限，实测（2026-10-03）1048195 字节通过，
// 1052415 字节返回 {"error":"Failed to read request body: length limit exceeded"}，
// 再大则上游断连、被 Cloudflare 记为 502。首帧 base64 后体积约为原图的 1.37 倍，
// 原图直出极易触顶，因此发送前按预算收敛：限制长边后编码 JPEG，保持原比例不裁切。
const GROK_PROXY_BODY_BUDGET_BYTES: usize = 1_000_000;
const GROK_FIRST_FRAME_JPEG_QUALITIES: [u8; 6] = [90, 85, 80, 75, 70, 60];
// 成片最高 1080p，长边再大也进不了画面，只白占体积预算。
const GROK_FIRST_FRAME_MAX_EDGE: u32 = 1920;
const GROK_FIRST_FRAME_MIN_EDGE: u32 = 640;
const GROK_FIRST_FRAME_JPEG_MIME: &str = "image/jpeg";
// 轮询 deadline 只在循环顶部检查，请求自身挂起时检查不到，因此单次请求必须自带超时。
const GROK_VIDEO_POLL_TIMEOUT_SECONDS: u64 = 30;
const GROK_VIDEO_DOWNLOAD_TIMEOUT_SECONDS: u64 = 300;

// 只缩不放，且保持原始比例：上游接受任意比例的输入图，裁切会白丢画面。
fn downscale_grok_first_frame(
    image: &image::DynamicImage,
    max_edge: u32,
) -> image::DynamicImage {
    let edge = image.width().max(image.height());
    if edge == 0 || edge <= max_edge {
        return image.clone();
    }
    let scale = max_edge as f64 / edge as f64;
    let width = ((image.width() as f64 * scale).round() as u32).max(1);
    let height = ((image.height() as f64 * scale).round() as u32).max(1);
    image.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
}

// JPEG 没有透明通道，带 alpha 的图必须先合成到白底，否则透明区域会变成黑块。
fn flatten_grok_first_frame(image: &image::DynamicImage) -> image::RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(width, height);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = u32::from(pixel[3]);
        let blend = |channel: u8| -> u8 {
            ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8
        };
        rgb.put_pixel(x, y, image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]));
    }
    rgb
}

fn encode_grok_first_frame_jpeg(
    image: &image::DynamicImage,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let rgb = flatten_grok_first_frame(image);
    let mut buffer = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
    rgb.write_with_encoder(encoder)
        .map_err(|err| format!("编码图生视频首帧失败：{err}"))?;
    Ok(buffer)
}

// 除首帧 data URL 以外，请求体其余字段的固定长度，用于反推首帧可用预算。
fn grok_video_payload_overhead(
    model: &str,
    prompt: &str,
    duration: u32,
    resolution: &str,
) -> usize {
    serde_json::json!({
        "model": model,
        "prompt": prompt,
        "image": { "url": "" },
        "duration": duration,
        "resolution": resolution,
    })
    .to_string()
    .len()
}

fn grok_first_frame_data_url(image_url: &str, budget_bytes: usize) -> Result<String, String> {
    let trimmed = image_url.trim();
    // 公网 URL 与已在预算内的 data URL 原样透传，不做有损重编码。
    if !trimmed.starts_with("data:") || trimmed.len() <= budget_bytes {
        return Ok(trimmed.to_string());
    }
    let original_bytes = decode_generated_image_base64(trimmed)?;
    let decoded = image::load_from_memory(&original_bytes)
        .map_err(|err| format!("解码图生视频首帧失败：{err}"))?;
    let mut max_edge = GROK_FIRST_FRAME_MAX_EDGE;
    loop {
        let fitted = downscale_grok_first_frame(&decoded, max_edge);
        let mut smallest_len = 0;
        for quality in GROK_FIRST_FRAME_JPEG_QUALITIES {
            let encoded = encode_grok_first_frame_jpeg(&fitted, quality)?;
            let data_url = format!(
                "data:{GROK_FIRST_FRAME_JPEG_MIME};base64,{}",
                B64.encode(&encoded)
            );
            if data_url.len() <= budget_bytes {
                runtime_log_info(format!(
                    "[图生视频] 首帧体积收敛 原始={}KB 输出={}KB 长边上限={max_edge} 尺寸={}x{} 质量={quality}",
                    original_bytes.len() / 1024,
                    encoded.len() / 1024,
                    fitted.width(),
                    fitted.height(),
                ));
                return Ok(data_url);
            }
            smallest_len = data_url.len();
        }
        let next_edge = (max_edge as f64 * 0.8).round() as u32;
        if next_edge < GROK_FIRST_FRAME_MIN_EDGE {
            return Err(format!(
                "图生视频首帧压缩到 {}KB 仍超过请求体积上限 {}KB，请换一张更小的图片",
                smallest_len / 1024,
                budget_bytes / 1024,
            ));
        }
        max_edge = next_edge;
    }
}

pub(crate) struct GrokImageToVideoRequest {
    pub(crate) image_url: String,
    pub(crate) model_id: Option<String>,
    pub(crate) prompt: String,
    pub(crate) duration: u32,
    pub(crate) resolution: String,
}

pub(crate) fn generate_grok_image_to_video_request(
    image_url: &str,
    prompt: &str,
    duration: u32,
    resolution: &str,
) -> Result<GrokImageToVideoRequest, String> {
    if image_url.trim().is_empty() {
        return Err("Grok 图生视频缺少首帧".to_string());
    }
    if !matches!(duration, 6 | 10) {
        return Err("Grok 图生视频时长只能是 6 秒或 10 秒".to_string());
    }
    if !matches!(resolution, "480p" | "720p") {
        return Err("Grok 图生视频分辨率只能是 480p 或 720p".to_string());
    }
    Ok(GrokImageToVideoRequest {
        image_url: image_url.to_string(),
        model_id: None,
        prompt: prompt.to_string(),
        duration,
        resolution: resolution.to_string(),
    })
}

pub(crate) async fn generate_grok_image_to_video(
    state: &AppState,
    request: GrokImageToVideoRequest,
) -> Result<String, String> {
    let requested_model_id = request.model_id.clone();
    let request = generate_grok_image_to_video_request(
        &request.image_url,
        &request.prompt,
        request.duration,
        &request.resolution,
    )?;
    let config = state_read_config_cached(state).map_err(|error| format!("读取视觉模型配置失败: {error}"))?;
    let target_model_id = requested_model_id
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            config
                .image_to_video_model_id
                .clone()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| {
            "尚未选择默认生视频模型，请在“设置 → 常用 → 模型分工”中选择。".to_string()
        })?;
    let resolved = resolve_image_generation_model(&config, Some(target_model_id.as_str()))?;
    let model_name = resolved.model.model.trim();
    if model_name.is_empty() {
        return Err(format!("视觉模型“{}”缺少模型名", resolved.model.name));
    }
    let (api_key, base_url) = resolve_xai_image_auth(state, &resolved.provider, "").await?;
    let endpoint = append_image_generation_endpoint(&base_url, "/videos/generations");
    let first_frame_budget = GROK_PROXY_BODY_BUDGET_BYTES.saturating_sub(
        grok_video_payload_overhead(
            model_name,
            &request.prompt,
            request.duration,
            &request.resolution,
        ),
    );
    let first_frame_url = grok_first_frame_data_url(&request.image_url, first_frame_budget)?;
    let payload = serde_json::json!({
        "model": model_name,
        "prompt": request.prompt,
        "image": { "url": first_frame_url },
        "duration": request.duration,
        "resolution": request.resolution,
    });
    let provider = resolved.provider.clone();
    let created = post_bearer_image_generation_json(state, &provider, &api_key, &endpoint, &payload).await?;
    let request_id = grok_video_request_id(&created).ok_or_else(|| "Grok 视频生成没有返回 request_id".to_string())?;
    let poll_endpoint = append_image_generation_endpoint(&base_url, &format!("/videos/{request_id}"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        if std::time::Instant::now() >= deadline {
            return Err("Grok 视频生成超时".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let response = state
            .shared_http_client
            .get(&poll_endpoint)
            .bearer_auth(&api_key)
            .timeout(std::time::Duration::from_secs(GROK_VIDEO_POLL_TIMEOUT_SECONDS))
            .send()
            .await
            .map_err(|error| format!("Grok 视频轮询失败: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Grok 视频轮询失败: HTTP {}", response.status()));
        }
        let value: Value = response.json().await.map_err(|error| format!("解析 Grok 视频状态失败: {error}"))?;
        match grok_video_poll_decision(&value) {
            GrokVideoPollDecision::Pending => {}
            GrokVideoPollDecision::Ready(url) => {
                let bytes = download_grok_video(state, &url).await?;
                return persist_generated_video_bytes(state, bytes).await;
            }
            GrokVideoPollDecision::Failed(error) => return Err(error),
        }
    }
}

async fn download_grok_video(state: &AppState, url: &str) -> Result<Vec<u8>, String> {
    if url.trim().is_empty() {
        return Err("Grok 视频完成但没有下载地址".to_string());
    }
    let response = state
        .shared_http_client
        .get(url)
        .timeout(std::time::Duration::from_secs(GROK_VIDEO_DOWNLOAD_TIMEOUT_SECONDS))
        .send()
        .await
        .map_err(|error| format!("下载 Grok 视频失败: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("下载 Grok 视频失败: HTTP {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|error| format!("读取 Grok 视频失败: {error}"))?;
    if bytes.is_empty() {
        return Err("Grok 视频内容为空".to_string());
    }
    Ok(bytes.to_vec())
}

async fn persist_generated_video_bytes(state: &AppState, bytes: Vec<u8>) -> Result<String, String> {
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    let date_dir = chrono::Local::now().format("%Y%m%d").to_string();
    let blocking_root = workspace_root.clone();
    let absolute_path = tokio::task::spawn_blocking(move || {
        let output_dir = blocking_root.join("generated-videos").join(date_dir);
        fs::create_dir_all(&output_dir).map_err(|error| format!("创建生成视频目录失败: {error}"))?;
        let output_path = output_dir.join(format!("{}.mp4", Uuid::new_v4()));
        fs::write(&output_path, bytes).map_err(|error| format!("保存生成视频失败: {error}"))?;
        Ok::<PathBuf, String>(output_path)
    })
    .await
    .map_err(|error| format!("生成视频落盘失败: {error}"))??;
    absolute_path
        .strip_prefix(&workspace_root)
        .map(|path| assistant_space_display_path(&path.to_string_lossy().replace('\\', "/")))
        .map_err(|_| "生成视频路径不在 Assistant Space 内".to_string())
}

async fn resolve_xai_image_auth(
    state: &AppState,
    provider: &ImageGenerationProviderConfig,
    api_key: &str,
) -> Result<(String, String), String> {
    // 凭证引用：按 codex_api_provider_id 指向的 Grok 登录供应商取票据与地址，不再读视觉供应商自己的 baseUrl。
    let referenced_id = provider
        .codex_api_provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(referenced_id) = referenced_id {
        let mut config = state_read_config_cached(state)?;
        normalize_app_config(&mut config);
        let api_provider = config
            .api_providers
            .iter()
            .find(|item| item.id == referenced_id)
            .ok_or_else(|| {
                format!(
                    "视觉供应商“{}”引用的 API 供应商不存在，请重新选择 Grok 登录。",
                    provider.name
                )
            })?;
        if api_provider.login_provider.trim() != "grok" {
            return Err(format!(
                "视觉供应商“{}”引用的 API 供应商不是 Grok 登录供应商：{}",
                provider.name, api_provider.name
            ));
        }
        return resolve_grok_access_token(&state.data_path, &api_provider.id)
            .await
            .map_err(|error| format!("视觉供应商“{}”的 Grok 登录不可用: {error}", provider.name));
    }
    if !api_key.trim().is_empty() {
        return Ok((api_key.trim().to_string(), provider.base_url.clone()));
    }
    Err(format!(
        "视觉供应商“{}”尚未配置 API Key，也没有引用 Grok 登录",
        provider.name
    ))
}

async fn generate_xai_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    api_key: &str,
) -> Result<ProviderImageGenerationOutput, String> {
    let (api_key, base_url) = resolve_xai_image_auth(state, &resolved.provider, api_key).await?;
    let endpoint = append_image_generation_endpoint(&base_url, "/images/generations");
    let payload = xai_image_generation_payload(request, &resolved.model);
    let value = post_bearer_image_generation_json(
        state,
        &resolved.provider,
        &api_key,
        &endpoint,
        &payload,
    )
    .await?;
    parse_openai_style_image_response(&value)
}

async fn generate_seedream_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    api_key: &str,
) -> Result<ProviderImageGenerationOutput, String> {
    let endpoint = append_image_generation_endpoint(&resolved.provider.base_url, "/images/generations");
    let payload = seedream_image_generation_payload(request, &resolved.provider, &resolved.model);
    let value = post_bearer_image_generation_json(
        state,
        &resolved.provider,
        api_key,
        &endpoint,
        &payload,
    )
    .await?;
    parse_openai_style_image_response(&value)
}

async fn post_gemini_image_interactions(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    api_key: &str,
    payload: &Value,
) -> Result<Value, String> {
    let endpoint = append_image_generation_endpoint(
        &resolved.provider.base_url,
        "/interactions",
    );
    let response = state
        .shared_http_client
        .post(endpoint)
        .header("x-goog-api-key", api_key)
        .header("Api-Revision", "2026-05-20")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(payload)
        .timeout(std::time::Duration::from_secs(u64::from(
            resolved.provider.timeout_seconds,
        )))
        .send()
        .await
        .map_err(|err| format!("{} 请求失败：{err}", resolved.provider.name))?;
    parse_image_generation_json_response(response, &resolved.provider.name).await
}

async fn generate_gemini_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    api_key: &str,
) -> Result<ProviderImageGenerationOutput, String> {
    let payload = gemini_image_generation_payload(request, &resolved.model);
    let value = post_gemini_image_interactions(state, resolved, api_key, &payload).await?;
    parse_gemini_image_response(&value)
}

async fn generate_sensenova_image_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    api_key: &str,
) -> Result<ProviderImageGenerationOutput, String> {
    // SenseNova 生图完全兼容 OpenAI images/generations 协议，直接复用
    let endpoint = append_image_generation_endpoint(&resolved.provider.base_url, "/images/generations");
    let payload = openai_image_generation_payload(request, &resolved.model);
    let value = post_bearer_image_generation_json(
        state,
        &resolved.provider,
        api_key,
        &endpoint,
        &payload,
    )
    .await?;
    parse_openai_style_image_response(&value)
}

#[cfg(test)]
mod image_generation_provider_tests {
    use super::*;

    fn request() -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "一只猫".to_string(),
            aspect_ratio: Some("16:9".to_string()),
            ..ImageGenerationRequest::default()
        }
    }

    #[test]
    fn openai_payload_should_use_current_gpt_image_fields() {
        let payload = openai_image_generation_payload(
            &request(),
            &ImageGenerationModelConfig::default(),
        );
        assert_eq!(payload.get("model").and_then(Value::as_str), Some("gpt-image-2"));
        assert_eq!(payload.get("output_format").and_then(Value::as_str), Some("png"));
        assert_eq!(payload.get("size").and_then(Value::as_str), Some("1536x864"));
    }

    #[test]
    fn openai_payload_should_prefer_explicit_size_over_aspect_ratio() {
        let mut request = request();
        request.size = Some("1024x1536".to_string());
        let payload = openai_image_generation_payload(
            &request,
            &ImageGenerationModelConfig::default(),
        );
        assert_eq!(payload.get("size").and_then(Value::as_str), Some("1024x1536"));
    }

    #[test]
    fn xai_payload_should_keep_aspect_ratio_and_resolution() {
        let mut model = ImageGenerationModelConfig::default();
        model.model = "grok-imagine-image-quality".to_string();
        model.default_size = Some("2k".to_string());
        let payload = xai_image_generation_payload(&request(), &model);
        assert_eq!(payload.get("aspect_ratio").and_then(Value::as_str), Some("16:9"));
        assert_eq!(payload.get("resolution").and_then(Value::as_str), Some("2k"));
    }

    #[test]
    fn seedream_pro_payload_should_use_current_output_format() {
        let provider = ImageGenerationProviderConfig::default();
        let mut model = ImageGenerationModelConfig::default();
        model.model = "doubao-seedream-5-0-pro-260628".to_string();
        let payload = seedream_image_generation_payload(&request(), &provider, &model);
        assert_eq!(payload.get("output_format").and_then(Value::as_str), Some("png"));
        assert_eq!(payload.get("size").and_then(Value::as_str), Some("2K"));
        assert!(payload
            .get("prompt")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("画面宽高比：16:9")));
        assert_eq!(
            payload.get("response_format").and_then(Value::as_str),
            Some("b64_json")
        );
    }

    #[test]
    fn seedream_payload_should_only_send_fast_prompt_optimization_for_supported_models() {
        let provider = ImageGenerationProviderConfig::default();
        let mut fast_request = request();
        fast_request.quality = Some("fast".to_string());

        for model_name in [
            "doubao-seedream-5-0-lite-260128",
            "doubao-seedream-5-0-260128",
            "doubao-seedream-4-5-251128",
        ] {
            let mut model = ImageGenerationModelConfig::default();
            model.model = model_name.to_string();
            let payload = seedream_image_generation_payload(&fast_request, &provider, &model);
            assert!(payload.get("optimize_prompt_options").is_none());
        }

        let mut pro_model = ImageGenerationModelConfig::default();
        pro_model.model = "doubao-seedream-5-0-pro-260628".to_string();
        let pro_payload =
            seedream_image_generation_payload(&fast_request, &provider, &pro_model);
        assert_eq!(
            pro_payload
                .get("optimize_prompt_options")
                .and_then(|value| value.get("mode"))
                .and_then(Value::as_str),
            Some("fast")
        );

        let mut legacy_model = ImageGenerationModelConfig::default();
        legacy_model.model = "doubao-seedream-4-0-250828".to_string();
        let legacy_payload =
            seedream_image_generation_payload(&fast_request, &provider, &legacy_model);
        assert_eq!(
            legacy_payload
                .get("optimize_prompt_options")
                .and_then(|value| value.get("mode"))
                .and_then(Value::as_str),
            Some("fast")
        );

        let mut standard_request = request();
        standard_request.quality = Some("standard".to_string());
        let mut lite_model = ImageGenerationModelConfig::default();
        lite_model.model = "doubao-seedream-5-0-lite-260128".to_string();
        let lite_payload =
            seedream_image_generation_payload(&standard_request, &provider, &lite_model);
        assert_eq!(
            lite_payload
                .get("optimize_prompt_options")
                .and_then(|value| value.get("mode"))
                .and_then(Value::as_str),
            Some("standard")
        );
    }

    #[test]
    fn gemini_payload_should_use_current_interactions_schema() {
        let mut model = ImageGenerationModelConfig::default();
        model.model = "gemini-3.1-flash-image".to_string();
        model.default_size = Some("2K".to_string());
        let payload = gemini_image_generation_payload(&request(), &model);
        assert_eq!(
            payload.get("model").and_then(Value::as_str),
            Some("gemini-3.1-flash-image")
        );
        assert_eq!(
            payload
                .get("response_format")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("image")
        );
        assert_eq!(
            payload
                .get("response_format")
                .and_then(|value| value.get("aspect_ratio"))
                .and_then(Value::as_str),
            Some("16:9")
        );
        assert_eq!(
            payload
                .get("response_format")
                .and_then(|value| value.get("image_size"))
                .and_then(Value::as_str),
            Some("2K")
        );
    }

    #[test]
    fn gemini_parser_should_accept_interactions_steps() {
        let value = serde_json::json!({
            "status": "completed",
            "steps": [{
                "type": "model_output",
                "content": [{
                    "type": "image",
                    "mime_type": "image/png",
                    "data": "aGVsbG8="
                }]
            }]
        });
        let parsed = parse_gemini_image_response(&value).unwrap_or_default();
        assert_eq!(parsed.images.len(), 1);
        assert_eq!(parsed.images[0].mime_hint.as_deref(), Some("image/png"));
    }

    #[test]
    fn gemini_parser_should_accept_legacy_snake_case_inline_data() {
        let value = serde_json::json!({
            "candidates": [{
                "content": { "parts": [{
                    "inline_data": {
                        "mime_type": "image/png",
                        "data": "aGVsbG8="
                    }
                }]}
            }]
        });
        let parsed = parse_gemini_image_response(&value).unwrap_or_default();
        assert_eq!(parsed.images.len(), 1);
        assert_eq!(parsed.images[0].mime_hint.as_deref(), Some("image/png"));
    }

    #[test]
    fn sensenova_payload_should_reuse_openai_protocol() {
        let payload = openai_image_generation_payload(
            &request(),
            &ImageGenerationModelConfig {
                model: "sensenova-u1-fast".to_string(),
                ..ImageGenerationModelConfig::default()
            },
        );
        assert_eq!(payload.get("model").and_then(Value::as_str), Some("sensenova-u1-fast"));
        assert_eq!(payload.get("size").and_then(Value::as_str), Some("1536x1024"));
    }

    #[test]
    fn grok_first_frame_should_only_limit_long_edge() {
        let huge = image::DynamicImage::ImageRgb8(image::RgbImage::new(4000, 3000));
        let limited = downscale_grok_first_frame(&huge, GROK_FIRST_FRAME_MAX_EDGE);
        assert_eq!((limited.width(), limited.height()), (1920, 1440));

        let moderate = image::DynamicImage::ImageRgb8(image::RgbImage::new(1600, 1200));
        let untouched = downscale_grok_first_frame(&moderate, GROK_FIRST_FRAME_MAX_EDGE);
        assert_eq!((untouched.width(), untouched.height()), (1600, 1200));

        let small = image::DynamicImage::ImageRgb8(image::RgbImage::new(320, 240));
        let kept = downscale_grok_first_frame(&small, GROK_FIRST_FRAME_MAX_EDGE);
        assert_eq!((kept.width(), kept.height()), (320, 240));
    }

    #[test]
    fn grok_first_frame_should_flatten_transparency_to_white() {
        let mut rgba = image::RgbaImage::new(800, 800);
        for (x, _y, pixel) in rgba.enumerate_pixels_mut() {
            *pixel = if x < 400 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            };
        }
        let flattened = flatten_grok_first_frame(&image::DynamicImage::ImageRgba8(rgba));
        assert_eq!(flattened.get_pixel(10, 10).0, [255, 0, 0]);
        assert_eq!(flattened.get_pixel(790, 10).0, [255, 255, 255]);
    }

    #[test]
    fn grok_first_frame_should_pass_through_payload_within_budget() {
        let small = "data:image/png;base64,aGVsbG8=".to_string();
        assert_eq!(
            grok_first_frame_data_url(&small, 1024).expect("pass through"),
            small
        );
        let public = "https://example.com/a.png";
        assert_eq!(
            grok_first_frame_data_url(public, 0).expect("public url"),
            public
        );
    }

    fn noisy_png_data_url(width: u32, height: u32) -> String {
        let mut rgb = image::RgbImage::new(width, height);
        let mut seed = 7_u32;
        for pixel in rgb.pixels_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let value = (seed >> 16) as u8;
            *pixel = image::Rgb([value, value.wrapping_add(31), value.wrapping_add(97)]);
        }
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(rgb)
            .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .expect("encode png");
        format!("data:image/png;base64,{}", B64.encode(&png_bytes))
    }

    #[test]
    fn grok_first_frame_should_shrink_oversized_image_into_budget() {
        // 高频噪声图 base64 后远超预算，用来验证质量阶梯。
        let source = noisy_png_data_url(1600, 1200);
        assert!(source.len() > 400_000);

        let output = grok_first_frame_data_url(&source, 400_000).expect("shrink");
        assert!(output.starts_with("data:image/jpeg;base64,"));
        assert!(output.len() <= 400_000);
        let decoded = image::load_from_memory(&decode_generated_image_base64(&output).expect("decode"))
            .expect("load");
        assert!(decoded.width() <= 1600 && decoded.height() <= 1200);
    }

    #[test]
    fn grok_first_frame_should_shrink_edge_when_budget_is_tight() {
        let source = noisy_png_data_url(1600, 1200);
        let output = grok_first_frame_data_url(&source, 500_000).expect("shrink");
        assert!(output.len() <= 500_000);
        let decoded = image::load_from_memory(&decode_generated_image_base64(&output).expect("decode"))
            .expect("load");
        // 质量阶梯用尽后必须真的缩了长边，而不是原地反复编码。
        assert!(decoded.width().max(decoded.height()) < 1600);
    }

    #[test]
    fn grok_first_frame_should_reject_impossible_budget_without_looping() {
        // 预算小到任何尺寸都装不下时必须直接报错，不能无限缩边。
        let source = noisy_png_data_url(400, 300);
        let error = grok_first_frame_data_url(&source, 1_000).err().unwrap_or_default();
        assert!(error.contains("请换一张更小的图片"), "unexpected error: {error}");
    }

    #[test]
    fn grok_video_overhead_should_track_prompt_length() {
        let model = "grok-imagine-video-1.5";
        let short = grok_video_payload_overhead(model, "a", 6, "480p");
        let long = grok_video_payload_overhead(model, &"a".repeat(100), 6, "480p");
        assert_eq!(long - short, 99);
    }
}
