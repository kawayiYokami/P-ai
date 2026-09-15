fn screenshot_artifact_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, ScreenshotArtifactEntry>> {
    static CACHE: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, ScreenshotArtifactEntry>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn next_screenshot_artifact_seq() -> u64 {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn screenshot_artifact_cache_put(payload: &ScreenshotForwardPayload) -> String {
    let artifact_id = Uuid::new_v4().to_string();
    let entry = ScreenshotArtifactEntry {
        images: payload.images.clone(),
        created_seq: next_screenshot_artifact_seq(),
    };
    let cache = screenshot_artifact_cache();
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= SCREENSHOT_ARTIFACT_MAX_ITEMS {
            if let Some(oldest_key) = guard
                .iter()
                .min_by_key(|(_, value)| value.created_seq)
                .map(|(key, _)| key.clone())
            {
                let _ = guard.remove(&oldest_key);
            }
        }
        guard.insert(artifact_id.clone(), entry);
    }
    artifact_id
}

fn screenshot_artifact_cache_get(artifact_id: &str) -> Option<ScreenshotArtifactEntry> {
    let cache = screenshot_artifact_cache();
    let guard = cache.lock().ok()?;
    guard.get(artifact_id).cloned()
}

fn clear_screenshot_artifact_cache() {
    if let Ok(mut guard) = screenshot_artifact_cache().lock() {
        guard.clear();
    }
}

fn normalize_tool_image_data(raw: &str) -> String {
    let s = raw.trim();
    if let Some(idx) = s.find("base64,") {
        return s[(idx + "base64,".len())..].to_string();
    }
    s.to_string()
}

fn extract_forward_images_from_value(value: &Value) -> Vec<ScreenshotForwardImagePayload> {
    let mut images = Vec::<ScreenshotForwardImagePayload>::new();

    if let Some(image_b64) = value
        .get("imageBase64")
        .and_then(Value::as_str)
        .or_else(|| value.get("image_base64").and_then(Value::as_str))
    {
        images.push(ScreenshotForwardImagePayload {
            mime: extract_image_mime_from_value(value).unwrap_or_else(|| "image/webp".to_string()),
            base64: normalize_tool_image_data(image_b64),
            width: value
                .get("width")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(u32::MAX as u64) as u32,
            height: value
                .get("height")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(u32::MAX as u64) as u32,
        });
        return images;
    }

    let collect_parts = |parts: &[Value]| -> Vec<ScreenshotForwardImagePayload> {
        parts
            .iter()
            .filter_map(|part| {
                let is_image = part
                    .get("type")
                    .and_then(Value::as_str)
                    .map(|t| t.eq_ignore_ascii_case("image"))
                    .unwrap_or(false);
                if !is_image {
                    return None;
                }
                let data = part.get("data").and_then(Value::as_str)?;
                Some(ScreenshotForwardImagePayload {
                    mime: part
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .filter(|m| !m.trim().is_empty())
                        .unwrap_or("image/webp")
                        .to_string(),
                    base64: normalize_tool_image_data(data),
                    width: part
                        .get("width")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .min(u32::MAX as u64) as u32,
                    height: part
                        .get("height")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .min(u32::MAX as u64) as u32,
                })
            })
            .collect()
    };

    if let Some(parts) = value.get("parts").and_then(Value::as_array) {
        images.extend(collect_parts(parts));
    }
    if images.is_empty() {
        if let Some(parts) = value.get("content").and_then(Value::as_array) {
            images.extend(collect_parts(parts));
        }
    }
    if images.is_empty() {
        if let Some(parts) = value.as_array() {
            images.extend(collect_parts(parts));
        }
    }

    images
}

fn extract_image_mime_from_value(value: &Value) -> Option<String> {
    value
        .get("imageMime")
        .and_then(Value::as_str)
        .filter(|m| !m.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("image_mime")
                .and_then(Value::as_str)
                .filter(|m| !m.trim().is_empty())
                .map(ToString::to_string)
        })
        .or_else(|| {
            value
                .get("parts")
                .and_then(Value::as_array)
                .and_then(|parts| {
                    parts.iter().find_map(|part| {
                        let is_image = part
                            .get("type")
                            .and_then(Value::as_str)
                            .map(|t| t.eq_ignore_ascii_case("image"))
                            .unwrap_or(false);
                        if !is_image {
                            return None;
                        }
                        part.get("mimeType")
                            .and_then(Value::as_str)
                            .filter(|m| !m.trim().is_empty())
                            .map(ToString::to_string)
                    })
                })
        })
        .or_else(|| {
            value
                .get("content")
                .and_then(Value::as_array)
                .and_then(|parts| {
                    parts.iter().find_map(|part| {
                        let is_image = part
                            .get("type")
                            .and_then(Value::as_str)
                            .map(|t| t.eq_ignore_ascii_case("image"))
                            .unwrap_or(false);
                        if !is_image {
                            return None;
                        }
                        part.get("mimeType")
                            .and_then(Value::as_str)
                            .filter(|m| !m.trim().is_empty())
                            .map(ToString::to_string)
                    })
                })
        })
        .or_else(|| {
            value.as_array().and_then(|parts| {
                parts.iter().find_map(|part| {
                    let is_image = part
                        .get("type")
                        .and_then(Value::as_str)
                        .map(|t| t.eq_ignore_ascii_case("image"))
                        .unwrap_or(false);
                    if !is_image {
                        return None;
                    }
                    part.get("mimeType")
                        .and_then(Value::as_str)
                        .filter(|m| !m.trim().is_empty())
                        .map(ToString::to_string)
                })
            })
        })
}

fn enrich_screenshot_tool_result_with_cache(
    _tool_name: &str,
    tool_result: &ProviderToolResult,
    projected_text: &str,
) -> (String, Option<(ScreenshotForwardPayload, String)>) {
    let images = tool_result.parts.iter().filter_map(|part| match part {
        ProviderToolResultPart::Image { mime, data_base64, width, height } => Some(ScreenshotForwardImagePayload {
            mime: mime.clone(),
            base64: normalize_tool_image_data(data_base64),
            width: *width,
            height: *height,
        }),
        _ => None,
    }).collect::<Vec<_>>();
    if images.is_empty() {
        return (projected_text.to_string(), None);
    }
    let payload = ScreenshotForwardPayload { images };
    let artifact_id = screenshot_artifact_cache_put(&payload);
    let text = if projected_text.trim().is_empty() || projected_text.contains("[image:") {
        format!("Screenshot captured. Artifact ID: {artifact_id}")
    } else {
        format!("{projected_text}\nScreenshot artifact ID: {artifact_id}")
    };
    (text, Some((payload, artifact_id)))
}

fn screenshot_forward_notice(payload: &ScreenshotForwardPayload, tool_name: &str) -> String {
    // 工具结果图片不只来自截图（如 read_media 直返原图），按来源工具区分措辞，避免误导模型。
    let source_label = if tool_name == "operate" { "截图工具" } else { "工具" };
    if payload.images.len() > 1 {
        format!(
            "工具已执行，以下 {} 张图片来自工具结果，将作为用户消息转发，请注意鉴别。",
            payload.images.len()
        )
    } else if let Some(image) = payload.images.first() {
        if image.width > 0 && image.height > 0 {
            format!(
                "{source_label}已执行，以下图片来自工具结果（{}x{}），将作为用户消息转发，请注意鉴别。",
                image.width, image.height
            )
        } else {
            format!(
                "{source_label}已执行，以下图片来自工具结果，将作为用户消息转发，请注意鉴别。"
            )
        }
    } else {
        format!("{source_label}已执行，以下图片来自工具结果，将作为用户消息转发，请注意鉴别。")
    }
}

#[cfg(test)]
#[test]
fn screenshot_value_boundary_should_extract_multiple_images() {
    let value = serde_json::json!({
        "parts": [
            {"type": "image", "mimeType": "image/webp", "data": "aaa", "width": 100, "height": 80},
            {"type": "image", "mimeType": "image/png", "data": "bbb", "width": 50, "height": 40}
        ]
    });
    let images = extract_forward_images_from_value(&value);
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].mime, "image/webp");
    assert_eq!(images[1].mime, "image/png");
}

#[cfg(test)]
#[test]
fn screenshot_forward_notice_should_label_source_tool() {
    let payload_with = |width: u32, height: u32| ScreenshotForwardPayload {
        images: vec![ScreenshotForwardImagePayload {
            mime: "image/png".to_string(),
            base64: "aaa".to_string(),
            width,
            height,
        }],
    };

    // operate 的既有文案保持不变
    let operate_notice = screenshot_forward_notice(&payload_with(10, 20), "operate");
    assert!(operate_notice.starts_with("截图工具已执行"));
    assert!(operate_notice.contains("（10x20）"));

    // 非截图工具（如 read_media 直返原图）改用中性措辞
    let read_media_notice = screenshot_forward_notice(&payload_with(0, 0), "read_media");
    assert!(read_media_notice.starts_with("工具已执行"));
    assert!(!read_media_notice.contains("截图"));
}
