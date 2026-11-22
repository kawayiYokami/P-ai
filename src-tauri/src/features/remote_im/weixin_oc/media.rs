fn build_weixin_oc_http_client(timeout_ms: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| format!("创建个人微信 HTTP 客户端失败: {err}"))
}

fn weixin_oc_cdn_download_url(cdn_base_url: &str, encrypted_query_param: &str) -> String {
    format!(
        "{}/download?encrypted_query_param={}",
        cdn_base_url.trim_end_matches('/'),
        urlencoding::encode(encrypted_query_param.trim())
    )
}

fn weixin_oc_cdn_upload_url(cdn_base_url: &str, upload_param: &str, file_key: &str) -> String {
    format!(
        "{}/upload?encrypted_query_param={}&filekey={}",
        cdn_base_url.trim_end_matches('/'),
        urlencoding::encode(upload_param.trim()),
        urlencoding::encode(file_key.trim())
    )
}

fn weixin_oc_pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let pad_len = 16 - (data.len() % 16);
    let pad_len = if pad_len == 0 { 16 } else { pad_len };
    let mut out = Vec::with_capacity(data.len() + pad_len);
    out.extend_from_slice(data);
    out.extend(std::iter::repeat_n(pad_len as u8, pad_len));
    out
}

fn weixin_oc_encrypt_media_ecb(raw: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{BlockCipherEncrypt, KeyInit};

    if key.len() != 16 {
        return Err(format!("媒体 AES 密钥长度不正确: {}", key.len()));
    }
    let cipher = aes::Aes128::new_from_slice(key)
        .map_err(|err| format!("初始化媒体 AES 加密器失败: {err}"))?;
    let mut encrypted = weixin_oc_pkcs7_pad(raw);
    for chunk in encrypted.chunks_exact_mut(16) {
        let block =
            <&mut aes::Block>::try_from(chunk).map_err(|_| "媒体 AES 分组长度不正确".to_string())?;
        cipher.encrypt_block(block);
    }
    Ok(encrypted)
}

fn weixin_oc_aes_padded_size(size: usize) -> usize {
    let remainder = size % 16;
    if remainder == 0 {
        size + 16
    } else {
        size + (16 - remainder)
    }
}

fn weixin_oc_encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn weixin_oc_pkcs7_unpad(data: &[u8]) -> Vec<u8> {
    let Some(&pad_len) = data.last() else {
        return Vec::new();
    };
    let pad_len = pad_len as usize;
    if pad_len == 0 || pad_len > 16 || pad_len > data.len() {
        return data.to_vec();
    }
    if data[data.len() - pad_len..]
        .iter()
        .all(|value| *value as usize == pad_len)
    {
        data[..data.len() - pad_len].to_vec()
    } else {
        data.to_vec()
    }
}

fn weixin_oc_decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let normalized = input.trim();
    if normalized.is_empty() {
        return Err("十六进制密钥为空".to_string());
    }
    if normalized.len() % 2 != 0 {
        return Err("十六进制密钥长度不正确".to_string());
    }
    let mut out = Vec::with_capacity(normalized.len() / 2);
    let bytes = normalized.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let hi = (bytes[idx] as char)
            .to_digit(16)
            .ok_or_else(|| "十六进制密钥包含非法字符".to_string())?;
        let lo = (bytes[idx + 1] as char)
            .to_digit(16)
            .ok_or_else(|| "十六进制密钥包含非法字符".to_string())?;
        out.push(((hi << 4) | lo) as u8);
        idx += 2;
    }
    Ok(out)
}

fn weixin_oc_parse_media_aes_key(aes_key_value: &str) -> Result<Vec<u8>, String> {
    let normalized = aes_key_value.trim();
    if normalized.is_empty() {
        return Err("媒体 AES 密钥为空".to_string());
    }
    // 1. 如果原始字符串是 32 字符十六进制，直接 Hex 解码成 16 字节
    if normalized.len() == 32 && normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        return weixin_oc_decode_hex(normalized);
    }
    // 2. 如果原始字符串的 bytes 长度正好是 16 字节
    if normalized.as_bytes().len() == 16 {
        return Ok(normalized.as_bytes().to_vec());
    }
    // 3. 尝试 Base64 解码
    let padded = format!(
        "{}{}",
        normalized,
        "=".repeat((4usize.wrapping_sub(normalized.len() % 4)) % 4)
    );
    if let Ok(decoded) = B64.decode(padded.as_bytes()) {
        if decoded.len() == 16 {
            return Ok(decoded);
        }
        if decoded.len() == 32
            && decoded.iter().all(|byte| (*byte as char).is_ascii_hexdigit())
        {
            if let Ok(hex_text) = std::str::from_utf8(&decoded) {
                return weixin_oc_decode_hex(hex_text);
            }
        }
    }
    Err(format!(
        "媒体 AES 密钥格式不支持: len={}, preview={}",
        normalized.len(),
        if normalized.len() > 10 {
            &normalized[..10]
        } else {
            normalized
        }
    ))
}

fn weixin_oc_decrypt_media_ecb(encrypted: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{BlockCipherDecrypt, KeyInit};

    if key.len() != 16 {
        return Err(format!("媒体 AES 密钥长度不正确: {}", key.len()));
    }
    if encrypted.is_empty() {
        return Ok(Vec::new());
    }
    if encrypted.len() % 16 != 0 {
        return Err(format!("媒体密文长度不是 16 的倍数: {}", encrypted.len()));
    }
    let cipher = aes::Aes128::new_from_slice(key)
        .map_err(|err| format!("初始化媒体 AES 解密器失败: {err}"))?;
    let mut decrypted = encrypted.to_vec();
    for chunk in decrypted.chunks_exact_mut(16) {
        let block =
            <&mut aes::Block>::try_from(chunk).map_err(|_| "媒体 AES 分组长度不正确".to_string())?;
        cipher.decrypt_block(block);
    }
    Ok(weixin_oc_pkcs7_unpad(&decrypted))
}

async fn weixin_oc_download_image_bytes(
    client: &reqwest::Client,
    cdn_base_url: &str,
    full_url: Option<&str>,
    encrypted_query_param: &str,
    aes_key_value: Option<&str>,
) -> Result<Vec<u8>, String> {
    let started_at = std::time::Instant::now();
    let download_url = full_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| weixin_oc_cdn_download_url(cdn_base_url, encrypted_query_param));
    runtime_log_info(format!(
        "[远程IM] 开始下载图片 地址={} 完整地址={} 参数长度={} 密钥={}",
        download_url,
        full_url.is_some(),
        encrypted_query_param.len(),
        aes_key_value.is_some()
    ));
    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|err| format!("下载个人微信图片网络请求失败: {err}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("下载个人微信图片失败: status={} body={}", status, body));
    }
    let encrypted = resp
        .bytes()
        .await
        .map_err(|err| format!("读取个人微信图片响应流失败: {err}"))?;
    let download_ms = started_at.elapsed().as_millis();
    runtime_log_info(format!(
        "[远程IM] 图片密文下载完成 状态={} 密文字节={} 耗时={}ms",
        status,
        encrypted.len(),
        download_ms
    ));
    if let Some(value) = aes_key_value.map(str::trim).filter(|value| !value.is_empty()) {
        let decrypt_start = std::time::Instant::now();
        let key = weixin_oc_parse_media_aes_key(value)
            .map_err(|err| format!("解析图片 AES 密钥失败: {err}"))?;
        let decrypted = weixin_oc_decrypt_media_ecb(encrypted.as_ref(), &key)
            .map_err(|err| format!("解密个人微信图片失败: {err}"))?;
        runtime_log_info(format!(
            "[远程IM] 图片解密完成 明文字节={} 耗时={}ms",
            decrypted.len(),
            decrypt_start.elapsed().as_millis()
        ));
        return Ok(decrypted);
    }
    Ok(encrypted.to_vec())
}

fn weixin_oc_normalize_image_mime(raw: &[u8]) -> String {
    image_mime_from_bytes(raw).unwrap_or("image/jpeg").to_string()
}

fn weixin_oc_item_is_media(item: &WeixinOcMessageItem) -> bool {
    matches!(
        item.item_type,
        Some(WEIXIN_OC_IMAGE_ITEM_TYPE)
            | Some(WEIXIN_OC_VOICE_ITEM_TYPE)
            | Some(WEIXIN_OC_FILE_ITEM_TYPE)
            | Some(WEIXIN_OC_VIDEO_ITEM_TYPE)
    )
}

/// 取引用消息内容（文本或语音转文字），与官方 bodyFromItemList 一致
fn weixin_oc_ref_message_body(item: &WeixinOcMessageItem) -> Option<String> {
    if item.item_type == Some(WEIXIN_OC_TEXT_ITEM_TYPE) {
        if let Some(text) = item
            .text_item
            .as_ref()
            .and_then(|value| value.text.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }
    if item.item_type == Some(WEIXIN_OC_VOICE_ITEM_TYPE) {
        if let Some(text) = item
            .voice_item
            .as_ref()
            .and_then(|value| value.text.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }
    None
}

/// 引用消息格式化：官方行为为 `[引用: title | body]\n当前文本`；引用媒体时只保留当前文本
fn weixin_oc_format_quoted_text(item: &WeixinOcMessageItem) -> Option<String> {
    let text = item
        .text_item
        .as_ref()
        .and_then(|value| value.text.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let ref_msg = item.ref_msg.as_ref()?;
    if let Some(ref_item) = ref_msg.message_item.as_deref() {
        if weixin_oc_item_is_media(ref_item) {
            return Some(text);
        }
    }
    let mut parts = Vec::<String>::new();
    if let Some(title) = ref_msg
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(title.to_string());
    }
    if let Some(ref_item) = ref_msg.message_item.as_deref() {
        if let Some(body) = weixin_oc_ref_message_body(ref_item) {
            parts.push(body);
        }
    }
    if parts.is_empty() {
        return Some(text);
    }
    Some(format!("[引用: {}]\n{}", parts.join(" | "), text))
}

fn weixin_oc_guess_attachment_mime(file_name: &str, fallback: &str) -> String {
    media_mime_from_path(std::path::Path::new(file_name))
        .unwrap_or(fallback)
        .to_string()
}

async fn weixin_oc_collect_media(
    client: &reqwest::Client,
    credentials: &WeixinOcCredentials,
    item_list: &[WeixinOcMessageItem],
) -> WeixinOcCollectedMedia {
    let mut parts = Vec::<ChatIngressPart>::new();
    let cdn_base_url = credentials.normalized_cdn_base_url();
    runtime_log_info(format!(
        "[远程IM] 开始收集媒体 条目数={}",
        item_list.len()
    ));
    for (idx, item) in item_list.iter().enumerate() {
        let item_type = item.item_type.unwrap_or(0);
        runtime_log_info(format!(
            "[远程IM] 处理条目 序号={} 类型={}",
            idx, item_type
        ));
        if item_type == 1 {
            if let Some(text) = weixin_oc_format_quoted_text(item).or_else(|| {
                item.text_item
                    .as_ref()
                    .and_then(|value| value.text.as_deref())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            }) {
                parts.push(ChatIngressPart::Text { text });
            }
            continue;
        }
        let (media, file_name, fallback_mime, aes_key_override) = match item_type {
            2 => {
                let Some(image_item) = item.image_item.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信图片元数据缺失，已跳过并继续]".to_string() });
                    continue;
                };
                let Some(media) = image_item.media.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信图片下载信息缺失，已跳过并继续]".to_string() });
                    continue;
                };
                (
                    media,
                    "image.jpg".to_string(),
                    "image/jpeg".to_string(),
                    image_item
                        .aeskey
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string),
                )
            }
            WEIXIN_OC_VOICE_ITEM_TYPE => {
                let Some(voice_item) = item.voice_item.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信语音元数据缺失，已跳过并继续]".to_string() });
                    continue;
                };
                // 官方 2.x：服务端直接返回语音转文字，优先直取文本，省本地转写链路
                if let Some(voice_text) = voice_item
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    parts.push(ChatIngressPart::Text {
                        text: format!("[语音转文字] {voice_text}"),
                    });
                    continue;
                }
                let Some(media) = voice_item.media.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信语音下载信息缺失，已跳过并继续]".to_string() });
                    continue;
                };
                (
                    media,
                    "voice.silk".to_string(),
                    "audio/x-silk".to_string(),
                    None,
                )
            }
            4 => {
                let Some(file_item) = item.file_item.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信文件元数据缺失，已跳过并继续]".to_string() });
                    continue;
                };
                let Some(media) = file_item.media.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信文件下载信息缺失，已跳过并继续]".to_string() });
                    continue;
                };
                let file_name = file_item
                    .file_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("file.bin")
                    .to_string();
                let mime = weixin_oc_guess_attachment_mime(&file_name, "application/octet-stream");
                (
                    media,
                    file_name.clone(),
                    mime,
                    None,
                )
            }
            5 => {
                let Some(video_item) = item.video_item.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信视频元数据缺失，已跳过并继续]".to_string() });
                    continue;
                };
                let Some(media) = video_item.media.as_ref() else {
                    parts.push(ChatIngressPart::Text { text: "[附件不可用：微信视频下载信息缺失，已跳过并继续]".to_string() });
                    continue;
                };
                (
                    media,
                    "video.mp4".to_string(),
                    "video/mp4".to_string(),
                    None,
                )
            }
            _ => continue,
        };
        // 官方 2.x：服务端可能只下发 full_url（encrypt_query_param 为空），两者任一存在即可下载
        let full_url = media
            .full_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let encrypted_query_param = media
            .encrypt_query_param
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if full_url.is_none() && encrypted_query_param.is_none() {
            parts.push(ChatIngressPart::Text {
                text: format!("[附件不可用：{} 缺少下载参数，已跳过并继续]", file_name),
            });
            continue;
        }
        let aes_key_value = aes_key_override.or_else(|| {
            media.aes_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
        let raw = match weixin_oc_download_image_bytes(
            client,
            &cdn_base_url,
            full_url,
            encrypted_query_param.unwrap_or(""),
            aes_key_value.as_deref(),
        )
        .await {
            Ok(raw) => raw,
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程IM] 单个附件下载失败，已跳过并继续 文件={} 原因={}",
                    file_name, err
                ));
                parts.push(ChatIngressPart::Text {
                    text: format!("[附件不可用：{} 下载失败，已跳过并继续]", file_name),
                });
                continue;
            }
        };
        let mime = if item_type == 2 {
            weixin_oc_normalize_image_mime(&raw)
        } else {
            fallback_mime
        };
        runtime_log_info(format!(
            "[远程IM] 附件收集成功 类型={} 名称={} 格式={} 字节={}",
            item_type, file_name, mime, raw.len()
        ));
        parts.push(ChatIngressPart::Attachment {
            path: None,
            bytes_base64: Some(B64.encode(raw)),
            mime,
            name: file_name,
        });
    }
    runtime_log_info(format!(
        "[远程IM] 媒体收集完成 数量={}",
        parts.len()
    ));
    WeixinOcCollectedMedia { parts }
}

#[derive(Debug, Deserialize)]
struct WeixinOcGetUploadUrlResp {
    #[serde(default)]
    ret: i64,
    #[serde(default)]
    errcode: i64,
    #[serde(default)]
    errmsg: String,
    #[serde(default)]
    #[serde(alias = "uploadParam")]
    upload_param: String,
    #[serde(default)]
    #[serde(alias = "uploadFullUrl")]
    upload_full_url: String,
}

fn weixin_oc_media_aes_key_hex() -> String {
    weixin_oc_encode_hex(Uuid::new_v4().as_bytes())
}

fn weixin_oc_random_hex_id() -> String {
    Uuid::new_v4().simple().to_string()
}

async fn weixin_oc_request_upload_url(
    client: &reqwest::Client,
    credentials: &WeixinOcCredentials,
    to_user_id: &str,
    file_key: &str,
    raw: &[u8],
    upload_media_type: i64,
    aes_key_hex: &str,
) -> Result<WeixinOcGetUploadUrlResp, String> {
    let ciphertext_size = weixin_oc_aes_padded_size(raw.len());
    let body = serde_json::json!({
        "filekey": file_key,
        "media_type": upload_media_type,
        "to_user_id": to_user_id,
        "rawsize": raw.len(),
        "rawfilemd5": format!("{:x}", md5::compute(raw)),
        "filesize": ciphertext_size,
        "no_need_thumb": true,
        "aeskey": aes_key_hex,
        "base_info": weixin_oc_build_base_info()
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 getuploadurl 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(credentials.token.as_str()))?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/getuploadurl",
            credentials.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 getuploadurl 失败: {err}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|err| format!("读取 getuploadurl 响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "请求 getuploadurl 失败: status={} body={}",
            status, text
        ));
    }
    let parsed = serde_json::from_str::<WeixinOcGetUploadUrlResp>(&text)
        .map_err(|err| format!("解析 getuploadurl 响应失败: {err}, body={text}"))?;
    if parsed.ret != 0 || parsed.errcode != 0 {
        return Err(format!(
            "请求 getuploadurl 失败: ret={} errcode={} errmsg={}",
            parsed.ret, parsed.errcode, parsed.errmsg
        ));
    }
    if parsed.upload_param.trim().is_empty() && parsed.upload_full_url.trim().is_empty() {
        return Err("请求 getuploadurl 失败: 返回中缺少 upload_param / upload_full_url".to_string());
    }
    runtime_log_info(format!(
        "[远程IM] 获取上传地址完成 接收者={} 媒体类型={} 字节={} 参数长度={} 完整地址={}",
        to_user_id.trim(),
        upload_media_type,
        raw.len(),
        parsed.upload_param.len(),
        !parsed.upload_full_url.trim().is_empty()
    ));
    Ok(parsed)
}

async fn weixin_oc_upload_to_cdn(
    client: &reqwest::Client,
    credentials: &WeixinOcCredentials,
    upload_param: &str,
    upload_full_url: &str,
    file_key: &str,
    aes_key_hex: &str,
    raw: &[u8],
) -> Result<String, String> {
    let key = weixin_oc_decode_hex(aes_key_hex)?;
    let encrypted = weixin_oc_encrypt_media_ecb(raw, &key)?;
    let upload_url = if !upload_full_url.trim().is_empty() {
        upload_full_url.trim().to_string()
    } else {
        weixin_oc_cdn_upload_url(
            credentials.normalized_cdn_base_url().as_str(),
            upload_param,
            file_key,
        )
    };
    let resp = client
        .post(upload_url)
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(encrypted)
        .send()
        .await
        .map_err(|err| format!("上传个人微信媒体到 CDN 失败: {err}"))?;
    let status = resp.status();
    let encrypted_query_param = resp
        .headers()
        .get("x-encrypted-param")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "上传个人微信媒体到 CDN 失败: status={} body={}",
            status, body
        ));
    }
    if encrypted_query_param.is_empty() {
        return Err(format!(
            "上传个人微信媒体到 CDN 失败: 响应缺少 x-encrypted-param, body={body}"
        ));
    }
    runtime_log_info(format!(
        "[远程IM] CDN 上传完成 文件={} 密文字节={} 参数长度={}",
        file_key.trim(),
        weixin_oc_aes_padded_size(raw.len()),
        encrypted_query_param.len()
    ));
    Ok(encrypted_query_param)
}

async fn weixin_oc_prepare_outbound_media_item(
    client: &reqwest::Client,
    credentials: &WeixinOcCredentials,
    to_user_id: &str,
    upload_media_type: i64,
    item_type: i64,
    file_name: &str,
    raw: &[u8],
) -> Result<Value, String> {
    runtime_log_info(format!(
        "[远程IM] 开始准备媒体 接收者={} 类型={} 上传类型={} 文件={} 字节={}",
        to_user_id.trim(),
        item_type,
        upload_media_type,
        file_name.trim(),
        raw.len()
    ));
    let file_key = weixin_oc_random_hex_id();
    let aes_key_hex = weixin_oc_media_aes_key_hex();
    let upload = weixin_oc_request_upload_url(
        client,
        credentials,
        to_user_id,
        &file_key,
        raw,
        upload_media_type,
        &aes_key_hex,
    )
    .await?;
    let encrypted_query_param = weixin_oc_upload_to_cdn(
        client,
        credentials,
        upload.upload_param.as_str(),
        upload.upload_full_url.as_str(),
        &file_key,
        &aes_key_hex,
        raw,
    )
    .await?;
    let media_payload = serde_json::json!({
        "encrypt_query_param": encrypted_query_param,
        "aes_key": B64.encode(aes_key_hex.as_bytes()),
        "encrypt_type": 1,
    });
    let ciphertext_size = weixin_oc_aes_padded_size(raw.len());
    Ok(match item_type {
        WEIXIN_OC_IMAGE_ITEM_TYPE => serde_json::json!({
            "type": WEIXIN_OC_IMAGE_ITEM_TYPE,
            "image_item": {
                "media": media_payload,
                "mid_size": ciphertext_size,
            }
        }),
        WEIXIN_OC_FILE_ITEM_TYPE => serde_json::json!({
            "type": WEIXIN_OC_FILE_ITEM_TYPE,
            "file_item": {
                "media": media_payload,
                "file_name": file_name,
                "len": raw.len().to_string(),
            }
        }),
        WEIXIN_OC_VIDEO_ITEM_TYPE => serde_json::json!({
            "type": WEIXIN_OC_VIDEO_ITEM_TYPE,
            "video_item": {
                "media": media_payload,
                "video_size": ciphertext_size,
            }
        }),
        _ => {
            return Err(format!("个人微信媒体类型不支持: item_type={item_type}"));
        }
    })
}


#[cfg(test)]
mod weixin_oc_media_tests {
    use super::*;

    #[tokio::test]
    async fn collect_media_uses_voice_text_without_download() {
        let client = reqwest::Client::new();
        let credentials = WeixinOcCredentials {
            base_url: "https://example.com".to_string(),
            cdn_base_url: "https://cdn.example.com".to_string(),
            bot_type: String::new(),
            qr_poll_interval: None,
            long_poll_timeout_ms: None,
            api_timeout_ms: None,
            token: String::new(),
            account_id: String::new(),
            user_id: String::new(),
            sync_buf: String::new(),
        };
        let item = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_VOICE_ITEM_TYPE),
            text_item: None,
            image_item: None,
            voice_item: Some(WeixinOcVoiceItem {
                media: Some(WeixinOcMediaPayload {
                    encrypt_query_param: Some("enc=1".to_string()),
                    aes_key: None,
                    encrypt_type: Some(1),
                    full_url: Some("https://cdn.example.com/full/voice.silk".to_string()),
                }),
                encode_type: Some(6),
                sample_rate: Some(24000),
                playtime: Some(3000),
                text: Some("这是语音转文字".to_string()),
            }),
            file_item: None,
            video_item: None,
            ref_msg: None,
        };
        let collected = weixin_oc_collect_media(&client, &credentials, std::slice::from_ref(&item)).await;
        assert_eq!(collected.parts.len(), 1);
        match &collected.parts[0] {
            ChatIngressPart::Text { text } => {
                assert_eq!(text, "[语音转文字] 这是语音转文字");
            }
            _ => panic!("语音带 text 时应直取文本，不下载附件"),
        }
    }

    #[tokio::test]
    async fn collect_media_voice_without_text_falls_back_to_download() {
        // 语音无 text 时维持原逻辑：尝试下载附件（这里仅验证走附件分支，下载会失败并降级为文本提示）
        let client = reqwest::Client::new();
        let credentials = WeixinOcCredentials {
            base_url: "https://example.com".to_string(),
            cdn_base_url: "https://cdn.example.com".to_string(),
            bot_type: String::new(),
            qr_poll_interval: None,
            long_poll_timeout_ms: None,
            api_timeout_ms: None,
            token: String::new(),
            account_id: String::new(),
            user_id: String::new(),
            sync_buf: String::new(),
        };
        let item = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_VOICE_ITEM_TYPE),
            text_item: None,
            image_item: None,
            voice_item: Some(WeixinOcVoiceItem {
                media: Some(WeixinOcMediaPayload {
                    encrypt_query_param: Some("enc=1".to_string()),
                    aes_key: None,
                    encrypt_type: None,
                    full_url: Some("https://cdn.example.com/not-exist.silk".to_string()),
                }),
                encode_type: None,
                sample_rate: None,
                playtime: None,
                text: None,
            }),
            file_item: None,
            video_item: None,
            ref_msg: None,
        };
        let collected = weixin_oc_collect_media(&client, &credentials, std::slice::from_ref(&item)).await;
        // 下载失败时降级为文本提示，不能 panic
        assert!(!collected.parts.is_empty());
        match &collected.parts[0] {
            ChatIngressPart::Text { text } => {
                assert!(text.contains("附件不可用"));
            }
            ChatIngressPart::Attachment { .. } => {}
        }
    }

    #[tokio::test]
    async fn collect_media_formats_quoted_text_like_official() {
        let client = reqwest::Client::new();
        let credentials = WeixinOcCredentials {
            base_url: "https://example.com".to_string(),
            cdn_base_url: "https://cdn.example.com".to_string(),
            bot_type: String::new(),
            qr_poll_interval: None,
            long_poll_timeout_ms: None,
            api_timeout_ms: None,
            token: String::new(),
            account_id: String::new(),
            user_id: String::new(),
            sync_buf: String::new(),
        };
        let quoted = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_TEXT_ITEM_TYPE),
            text_item: Some(WeixinOcTextItem {
                text: Some("被引用的原文".to_string()),
            }),
            image_item: None,
            voice_item: None,
            file_item: None,
            video_item: None,
            ref_msg: None,
        };
        let item = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_TEXT_ITEM_TYPE),
            text_item: Some(WeixinOcTextItem {
                text: Some("当前回复".to_string()),
            }),
            image_item: None,
            voice_item: None,
            file_item: None,
            video_item: None,
            ref_msg: Some(WeixinOcRefMessage {
                title: Some("引用摘要".to_string()),
                message_item: Some(Box::new(quoted)),
            }),
        };
        let collected = weixin_oc_collect_media(&client, &credentials, std::slice::from_ref(&item)).await;
        assert_eq!(collected.parts.len(), 1);
        match &collected.parts[0] {
            ChatIngressPart::Text { text } => {
                assert_eq!(text, "[引用: 引用摘要 | 被引用的原文]\n当前回复");
            }
            _ => panic!("引用文本应格式化为文本块"),
        }
    }

    #[tokio::test]
    async fn collect_media_quoted_media_keeps_only_current_text() {
        let client = reqwest::Client::new();
        let credentials = WeixinOcCredentials {
            base_url: "https://example.com".to_string(),
            cdn_base_url: "https://cdn.example.com".to_string(),
            bot_type: String::new(),
            qr_poll_interval: None,
            long_poll_timeout_ms: None,
            api_timeout_ms: None,
            token: String::new(),
            account_id: String::new(),
            user_id: String::new(),
            sync_buf: String::new(),
        };
        let quoted_media = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_IMAGE_ITEM_TYPE),
            text_item: None,
            image_item: None,
            voice_item: None,
            file_item: None,
            video_item: None,
            ref_msg: None,
        };
        let item = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_TEXT_ITEM_TYPE),
            text_item: Some(WeixinOcTextItem {
                text: Some("回复图片".to_string()),
            }),
            image_item: None,
            voice_item: None,
            file_item: None,
            video_item: None,
            ref_msg: Some(WeixinOcRefMessage {
                title: None,
                message_item: Some(Box::new(quoted_media)),
            }),
        };
        let collected = weixin_oc_collect_media(&client, &credentials, std::slice::from_ref(&item)).await;
        match &collected.parts[0] {
            ChatIngressPart::Text { text } => {
                assert_eq!(text, "回复图片");
            }
            _ => panic!("引用媒体时应只保留当前文本"),
        }
    }

    #[tokio::test]
    async fn collect_media_full_url_only_attempts_download_not_skip() {
        // 官方 2.x 可能只下发 full_url（无 encrypt_query_param），此时应走下载而非「缺少下载参数」跳过
        let client = reqwest::Client::new();
        let credentials = WeixinOcCredentials {
            base_url: "https://example.com".to_string(),
            cdn_base_url: "https://cdn.example.com".to_string(),
            bot_type: String::new(),
            qr_poll_interval: None,
            long_poll_timeout_ms: None,
            api_timeout_ms: None,
            token: String::new(),
            account_id: String::new(),
            user_id: String::new(),
            sync_buf: String::new(),
        };
        let item = WeixinOcMessageItem {
            item_type: Some(WEIXIN_OC_FILE_ITEM_TYPE),
            text_item: None,
            image_item: None,
            voice_item: None,
            file_item: Some(WeixinOcFileItem {
                media: Some(WeixinOcMediaPayload {
                    encrypt_query_param: None,
                    aes_key: None,
                    encrypt_type: None,
                    full_url: Some("https://cdn.example.com/not-exist.bin".to_string()),
                }),
                file_name: Some("doc.pdf".to_string()),
            }),
            video_item: None,
            ref_msg: None,
        };
        let collected = weixin_oc_collect_media(&client, &credentials, std::slice::from_ref(&item)).await;
        assert_eq!(collected.parts.len(), 1);
        match &collected.parts[0] {
            ChatIngressPart::Text { text } => {
                // 走到下载分支（测试环境下载失败降级为提示），而不是「缺少下载参数」跳过
                assert!(text.contains("下载失败"), "应为下载失败降级: {text}");
            }
            ChatIngressPart::Attachment { .. } => {}
        }
    }

    #[test]
    fn parse_media_aes_key_supports_hex_and_base64_and_nested() {
        // 1. 32 字符 hex
        let hex_key = "0123456789abcdef0123456789abcdef";
        let parsed = weixin_oc_parse_media_aes_key(hex_key).unwrap();
        assert_eq!(parsed.len(), 16);

        // 2. 16 字节 Base64 (24 字符)
        let raw_16 = b"0123456789abcdef";
        let b64_key = B64.encode(raw_16);
        let parsed = weixin_oc_parse_media_aes_key(&b64_key).unwrap();
        assert_eq!(parsed, raw_16);

        // 3. Base64 编码的 32 字符 hex
        let nested = B64.encode(hex_key.as_bytes());
        let parsed = weixin_oc_parse_media_aes_key(&nested).unwrap();
        assert_eq!(parsed.len(), 16);
    }
}
