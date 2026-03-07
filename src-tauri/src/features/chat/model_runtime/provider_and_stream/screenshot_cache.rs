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
        mime: payload.mime.clone(),
        base64: payload.base64.clone(),
        width: payload.width,
        height: payload.height,
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

fn extract_image_base64_from_value(value: &Value) -> Option<String> {
    fn normalize_data_uri(raw: &str) -> String {
        let s = raw.trim();
        if let Some(idx) = s.find("base64,") {
            return s[(idx + "base64,".len())..].to_string();
        }
        s.to_string()
    }

    value
        .get("imageBase64")
        .and_then(Value::as_str)
        .map(normalize_data_uri)
        .or_else(|| {
            value
                .get("image_base64")
                .and_then(Value::as_str)
                .map(normalize_data_uri)
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
                        part.get("data")
                            .and_then(Value::as_str)
                            .map(normalize_data_uri)
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
                        part.get("data")
                            .and_then(Value::as_str)
                            .map(normalize_data_uri)
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
                    part.get("data")
                        .and_then(Value::as_str)
                        .map(normalize_data_uri)
                })
            })
        })
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
                Value::String(format!("<cached:{}>", artifact_id)),
            );
        }
        if obj.get("image_base64").is_some() {
            obj.insert(
                "image_base64".to_string(),
                Value::String(format!("<cached:{}>", artifact_id)),
            );
        }
        if let Some(parts) = obj.get_mut("parts").and_then(Value::as_array_mut) {
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
                            Value::String(format!("<cached:{}>", artifact_id)),
                        );
                    }
                }
            }
        }
        if let Some(data_obj) = obj.get_mut("data").and_then(Value::as_object_mut) {
            if data_obj.get("imageBase64").is_some() {
                data_obj.insert(
                    "imageBase64".to_string(),
                    Value::String(format!("<cached:{}>", artifact_id)),
                );
            }
            if data_obj.get("image_base64").is_some() {
                data_obj.insert(
                    "image_base64".to_string(),
                    Value::String(format!("<cached:{}>", artifact_id)),
                );
            }
            if let Some(parts) = data_obj.get_mut("parts").and_then(Value::as_array_mut) {
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
                                Value::String(format!("<cached:{}>", artifact_id)),
                            );
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
    let image_base64 = extract_image_base64_from_value(payload_value)?;
    if image_base64.is_empty() {
        return None;
    }
    let mime = extract_image_mime_from_value(payload_value)
        .unwrap_or_else(|| "image/webp".to_string());
    let width = payload_value
        .get("width")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let height = payload_value
        .get("height")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    Some(ScreenshotForwardPayload {
        mime,
        base64: image_base64,
        width,
        height,
    })
}

fn screenshot_forward_notice(payload: &ScreenshotForwardPayload) -> String {
    if payload.width > 0 && payload.height > 0 {
        format!(
            "截图工具已执行，以下图片来自工具结果（{}x{}），将作为用户消息转发，请注意鉴别。",
            payload.width, payload.height
        )
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
