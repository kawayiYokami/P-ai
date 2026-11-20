const IMAGE_GENERATION_MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const IMAGE_GENERATION_MAX_JSON_BYTES: usize = 96 * 1024 * 1024;
const IMAGE_GENERATION_MAX_ERROR_BYTES: usize = 8 * 1024;
const IMAGE_GENERATION_MAX_PIXELS: u64 = 100_000_000;

async fn read_limited_response_bytes(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::<u8>::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| format!("读取响应失败：{err}"))?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("响应超过大小限制（最大 {} MiB）", max_bytes / 1024 / 1024));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn truncate_image_generation_error_body(bytes: &[u8]) -> String {
    let limit = bytes.len().min(IMAGE_GENERATION_MAX_ERROR_BYTES);
    String::from_utf8_lossy(&bytes[..limit]).trim().to_string()
}

async fn parse_image_generation_json_response(
    response: reqwest::Response,
    provider_name: &str,
) -> Result<Value, String> {
    let status = response.status();
    let bytes = read_limited_response_bytes(response, IMAGE_GENERATION_MAX_JSON_BYTES).await?;
    if !status.is_success() {
        let body = truncate_image_generation_error_body(&bytes);
        return Err(if body.is_empty() {
            format!("{provider_name} 请求失败：HTTP {status}")
        } else {
            format!("{provider_name} 请求失败：HTTP {status}，{body}")
        });
    }
    serde_json::from_slice::<Value>(&bytes)
        .map_err(|err| format!("{provider_name} 响应不是有效 JSON：{err}"))
}

fn decode_generated_image_base64(value: &str) -> Result<Vec<u8>, String> {
    let encoded = value
        .split_once(',')
        .filter(|(prefix, _)| prefix.trim_start().starts_with("data:"))
        .map(|(_, payload)| payload)
        .unwrap_or(value);
    if encoded.len() > IMAGE_GENERATION_MAX_JSON_BYTES {
        return Err("图片 Base64 数据超过大小限制".to_string());
    }
    let compact = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let bytes = B64
        .decode(compact.as_bytes())
        .map_err(|err| format!("解析图片 Base64 失败：{err}"))?;
    if bytes.len() > IMAGE_GENERATION_MAX_IMAGE_BYTES {
        return Err("图片超过 64 MiB 大小限制".to_string());
    }
    Ok(bytes)
}

fn generated_image_format_info(
    bytes: &[u8],
) -> Result<(ImageFormat, &'static str, &'static str, u32, u32), String> {
    if bytes.is_empty() {
        return Err("供应商返回了空图片".to_string());
    }
    if bytes.len() > IMAGE_GENERATION_MAX_IMAGE_BYTES {
        return Err("图片超过 64 MiB 大小限制".to_string());
    }
    let format = image::guess_format(bytes).map_err(|_| "无法识别真实图片格式".to_string())?;
    let (mime, extension) = match format {
        ImageFormat::Png => ("image/png", "png"),
        ImageFormat::Jpeg => ("image/jpeg", "jpg"),
        ImageFormat::Gif => ("image/gif", "gif"),
        ImageFormat::WebP => ("image/webp", "webp"),
        ImageFormat::Bmp => ("image/bmp", "bmp"),
        _ => return Err("当前仅支持 PNG、JPEG、GIF、WebP 和 BMP 图片".to_string()),
    };
    let reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|err| format!("读取图片尺寸失败：{err}"))?;
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0 || height == 0 || pixels > IMAGE_GENERATION_MAX_PIXELS {
        return Err(format!("图片尺寸不安全：{width}x{height}"));
    }
    Ok((format, mime, extension, width, height))
}

fn same_origin_url(base_url: &str, target_url: &str) -> bool {
    let Ok(base) = reqwest::Url::parse(base_url) else {
        return false;
    };
    let Ok(target) = reqwest::Url::parse(target_url) else {
        return false;
    };
    base.scheme() == target.scheme()
        && base.host_str() == target.host_str()
        && base.port_or_known_default() == target.port_or_known_default()
}

async fn download_generated_image(
    state: &AppState,
    provider: &ImageGenerationProviderConfig,
    api_key: &str,
    url: &str,
) -> Result<(Vec<u8>, Option<String>), String> {
    if url.trim_start().starts_with("data:image/") {
        return decode_generated_image_base64(url).map(|bytes| (bytes, None));
    }
    let mut request = state
        .shared_http_client
        .get(url)
        .timeout(std::time::Duration::from_secs(u64::from(provider.timeout_seconds)));
    if matches!(provider.provider_type, ImageGenerationProviderKind::Comfyui)
        && !api_key.trim().is_empty()
        && same_origin_url(&provider.base_url, url)
    {
        request = request.bearer_auth(api_key.trim());
    }
    // 原生端点可能回 url 且需鉴权（magpie 录制样本是 b64，但 readImagesAnswer 两分支都认）。
    // 同源才带 Codex token；解析失败就按无鉴权继续，原有报错行为不变。
    if matches!(provider.provider_type, ImageGenerationProviderKind::Codex)
        && same_origin_url(&provider.base_url, url)
    {
        if let Ok(auth) = resolve_codex_image_auth(state, provider).await {
            if !auth.access_token.trim().is_empty() {
                request = request.bearer_auth(auth.access_token.trim());
            }
        }
    }
    let response = request
        .send()
        .await
        .map_err(|err| format!("下载生成图片失败：{err}"))?;
    let status = response.status();
    let mime_hint = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string());
    let bytes = read_limited_response_bytes(response, IMAGE_GENERATION_MAX_IMAGE_BYTES).await?;
    if !status.is_success() {
        let body = truncate_image_generation_error_body(&bytes);
        return Err(if body.is_empty() {
            format!("下载生成图片失败：HTTP {status}")
        } else {
            format!("下载生成图片失败：HTTP {status}，{body}")
        });
    }
    Ok((bytes, mime_hint))
}

async fn persist_generated_image_bytes(
    state: &AppState,
    bytes: Vec<u8>,
    remote_url: Option<String>,
    revised_prompt: Option<String>,
) -> Result<GeneratedImageAsset, String> {
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    let date_dir = chrono::Local::now().format("%Y%m%d").to_string();
    let blocking_root = workspace_root.clone();
    let persisted = tokio::task::spawn_blocking(move || {
        let (_format, mime, extension, width, height) = generated_image_format_info(&bytes)?;
        let output_dir = blocking_root.join("generated-images").join(date_dir);
        fs::create_dir_all(&output_dir)
            .map_err(|err| format!("创建生成图片目录失败：{err}"))?;
        let output_path = output_dir.join(format!("{}.{}", Uuid::new_v4(), extension));
        fs::write(&output_path, &bytes).map_err(|err| format!("保存生成图片失败：{err}"))?;
        Ok::<(PathBuf, String, u32, u32), String>((
            output_path,
            mime.to_string(),
            width,
            height,
        ))
    })
    .await
    .map_err(|err| format!("生成图片落盘任务失败：{err}"))??;
    let (absolute_path, mime, width, height) = persisted;
    let relative_path = absolute_path
        .strip_prefix(&workspace_root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "生成图片路径不在 Assistant Space 内".to_string())?;
    let markdown = format!(
        "![生成图片]({})",
        assistant_space_display_path(&relative_path)
    );
    Ok(GeneratedImageAsset {
        relative_path,
        remote_url,
        markdown,
        mime,
        width,
        height,
        revised_prompt,
    })
}

async fn materialize_pending_generated_image(
    state: &AppState,
    provider: &ImageGenerationProviderConfig,
    api_key: &str,
    pending: PendingGeneratedImage,
) -> Result<GeneratedImageAsset, String> {
    let (bytes, _download_mime_hint) = match pending.source {
        PendingImageSource::Bytes(bytes) => (bytes, pending.mime_hint.clone()),
        PendingImageSource::RemoteUrl(url) => {
            download_generated_image(state, provider, api_key, &url).await?
        }
    };
    persist_generated_image_bytes(state, bytes, pending.remote_url, pending.revised_prompt).await
}

#[cfg(test)]
mod image_generation_storage_tests {
    use super::*;

    #[test]
    fn generated_image_format_should_reject_html_error_page() {
        let error = generated_image_format_info(b"<html>bad gateway</html>")
            .err()
            .unwrap_or_default();
        assert!(error.contains("图片格式"));
    }

    #[test]
    fn generated_image_base64_should_accept_data_url() {
        let bytes = decode_generated_image_base64("data:image/png;base64,aGVsbG8=")
            .unwrap_or_default();
        assert_eq!(bytes, b"hello");
    }
}
