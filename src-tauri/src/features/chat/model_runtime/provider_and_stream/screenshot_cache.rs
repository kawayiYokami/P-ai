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

fn screenshot_payload_value<'a>(value: &'a Value) -> &'a Value {
    value.get("data").unwrap_or(value)
}

fn compact_screenshot_tool_result(
    tool_result: &str,
    artifact_id: &str,
) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(tool_result) else {
        return tool_result.to_string();
    };
    if let Some(obj) = value.as_object_mut() {
        if obj.get("imageBase64").is_some() {
            obj.insert(
                "imageBase64".to_string(),
                Value::String(format!("<cached:{}:0>", artifact_id)),
            );
        }
        if obj.get("image_base64").is_some() {
            obj.insert(
                "image_base64".to_string(),
                Value::String(format!("<cached:{}:0>", artifact_id)),
            );
        }
        if let Some(parts) = obj.get_mut("parts").and_then(Value::as_array_mut) {
            let mut image_idx = 0usize;
            for part in parts {
                if let Some(map) = part.as_object_mut() {
                    let is_image = map
                        .get("type")
                        .and_then(Value::as_str)
                        .map(|t| t.eq_ignore_ascii_case("image"))
                        .unwrap_or(false);
                        if is_image && map.get("data").is_some() {
                            map.insert(
                                "data".to_string(),
                                Value::String(format!("<cached:{}:{}>", artifact_id, image_idx)),
                            );
                            image_idx += 1;
                        }
                    }
                }
        }
        if let Some(data_obj) = obj.get_mut("data").and_then(Value::as_object_mut) {
            if data_obj.get("imageBase64").is_some() {
                data_obj.insert(
                    "imageBase64".to_string(),
                    Value::String(format!("<cached:{}:0>", artifact_id)),
                );
            }
            if data_obj.get("image_base64").is_some() {
                data_obj.insert(
                    "image_base64".to_string(),
                    Value::String(format!("<cached:{}:0>", artifact_id)),
                );
            }
            if let Some(parts) = data_obj.get_mut("parts").and_then(Value::as_array_mut) {
                let mut image_idx = 0usize;
                for part in parts {
                    if let Some(map) = part.as_object_mut() {
                        let is_image = map
                            .get("type")
                            .and_then(Value::as_str)
                            .map(|t| t.eq_ignore_ascii_case("image"))
                            .unwrap_or(false);
                        if is_image && map.get("data").is_some() {
                            map.insert(
                                "data".to_string(),
                                Value::String(format!("<cached:{}:{}>", artifact_id, image_idx)),
                            );
                            image_idx += 1;
                        }
                    }
                }
            }
        }
        obj.insert(
            "screenshotArtifact".to_string(),
            serde_json::json!({
                "id": artifact_id,
                "maxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS
            }),
        );
        if let Some(response_obj) = obj.get_mut("response").and_then(Value::as_object_mut) {
            response_obj.insert(
                "screenshotArtifactId".to_string(),
                Value::String(artifact_id.to_string()),
            );
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| tool_result.to_string())
}

fn enrich_screenshot_tool_result_with_cache(
    _tool_name: &str,
    tool_result: &str,
) -> (String, Option<(ScreenshotForwardPayload, String)>) {
    let Some(payload) = screenshot_forward_payload_from_tool_result(tool_result) else {
        return (tool_result.to_string(), None);
    };
    let artifact_id = screenshot_artifact_cache_put(&payload);
    let compacted = compact_screenshot_tool_result(tool_result, &artifact_id);
    (compacted, Some((payload, artifact_id)))
}

fn screenshot_forward_payload_from_tool_result(
    tool_result: &str,
) -> Option<ScreenshotForwardPayload> {
    let value = serde_json::from_str::<Value>(tool_result).ok()?;
    let payload_value = screenshot_payload_value(&value);
    let images = extract_forward_images_from_value(payload_value);
    if images.is_empty() {
        return None;
    }
    Some(ScreenshotForwardPayload { images })
}

fn screenshot_forward_notice(payload: &ScreenshotForwardPayload) -> String {
    if payload.images.len() > 1 {
        format!(
            "工具已执行，以下 {} 张图片来自工具结果，将作为用户消息转发，请注意鉴别。",
            payload.images.len()
        )
    } else if let Some(image) = payload.images.first() {
        if image.width > 0 && image.height > 0 {
            format!(
                "截图工具已执行，以下图片来自工具结果（{}x{}），将作为用户消息转发，请注意鉴别。",
                image.width, image.height
            )
        } else {
            "截图工具已执行，以下图片来自工具结果，将作为用户消息转发，请注意鉴别。".to_string()
        }
    } else {
        "截图工具已执行，以下图片来自工具结果，将作为用户消息转发，请注意鉴别。".to_string()
    }
}

fn sanitize_tool_result_for_history(tool_name: &str, tool_result: &str) -> String {
    let _ = tool_name;
    let Ok(mut value) = serde_json::from_str::<Value>(tool_result) else {
        return tool_result.to_string();
    };
    if let Some(obj) = value.as_object_mut() {
        if let Some(image_b64) = obj.get("imageBase64").and_then(Value::as_str) {
            obj.insert(
                "imageBase64".to_string(),
                Value::String(format!("<omitted:{} chars>", image_b64.len())),
            );
        }
        if let Some(parts) = obj.get_mut("parts").and_then(Value::as_array_mut) {
            for part in parts {
                let data_len = part
                    .get("data")
                    .and_then(Value::as_str)
                    .map(|data| data.len());
                if let Some(len) = data_len {
                    if let Some(map) = part.as_object_mut() {
                        map.insert(
                            "data".to_string(),
                            Value::String(format!("<omitted:{} chars>", len)),
                        );
                    }
                }
            }
        }
        if let Some(data_obj) = obj.get_mut("data").and_then(Value::as_object_mut) {
            if let Some(image_b64) = data_obj.get("imageBase64").and_then(Value::as_str) {
                data_obj.insert(
                    "imageBase64".to_string(),
                    Value::String(format!("<omitted:{} chars>", image_b64.len())),
                );
            }
            if let Some(image_b64) = data_obj.get("image_base64").and_then(Value::as_str) {
                data_obj.insert(
                    "image_base64".to_string(),
                    Value::String(format!("<omitted:{} chars>", image_b64.len())),
                );
            }
            if let Some(parts) = data_obj.get_mut("parts").and_then(Value::as_array_mut) {
                for part in parts {
                    let data_len = part
                        .get("data")
                        .and_then(Value::as_str)
                        .map(|data| data.len());
                    if let Some(len) = data_len {
                        if let Some(map) = part.as_object_mut() {
                            map.insert(
                                "data".to_string(),
                                Value::String(format!("<omitted:{} chars>", len)),
                            );
                        }
                    }
                }
            }
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| tool_result.to_string())
}

#[cfg(test)]
#[test]
fn screenshot_forward_payload_from_tool_result_should_support_multiple_images() {
    let tool_result = serde_json::json!({
        "parts": [
            {"type": "image", "mimeType": "image/webp", "data": "aaa", "width": 100, "height": 80},
            {"type": "image", "mimeType": "image/png", "data": "bbb", "width": 50, "height": 40}
        ]
    })
    .to_string();

    let payload = screenshot_forward_payload_from_tool_result(&tool_result).expect("payload");
    assert_eq!(payload.images.len(), 2);
    assert_eq!(payload.images[0].mime, "image/webp");
    assert_eq!(payload.images[1].mime, "image/png");
}
