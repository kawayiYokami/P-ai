const WEIXIN_OC_DEFAULT_BASE_URL: &str = "https://ilinkai.weixin.qq.com";
const WEIXIN_OC_DEFAULT_CDN_BASE_URL: &str = "https://novac2c.cdn.weixin.qq.com/c2c";
const WEIXIN_OC_DEFAULT_LONG_POLL_TIMEOUT_MS: u64 = 35_000;
const WEIXIN_OC_DEFAULT_API_TIMEOUT_MS: u64 = 15_000;
const WEIXIN_OC_DEFAULT_BOT_TYPE: &str = "3";
const WEIXIN_OC_LOGIN_TTL_SECS: i64 = 5 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeixinOcLoginSession {
    session_key: String,
    qrcode: String,
    qrcode_img_content: String,
    started_at: String,
    status: String,
    #[serde(default)]
    error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeixinOcLoginStartInput {
    channel_id: String,
    #[serde(default)]
    force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeixinOcLoginStatusInput {
    channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeixinOcLoginStartResult {
    channel_id: String,
    session_key: String,
    qrcode: String,
    qrcode_img_content: String,
    status: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeixinOcLoginStatusResult {
    channel_id: String,
    connected: bool,
    status: String,
    message: String,
    #[serde(default)]
    session_key: String,
    #[serde(default)]
    qrcode: String,
    #[serde(default)]
    qrcode_img_content: String,
    #[serde(default)]
    account_id: String,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    last_error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeixinOcSyncContactsResult {
    channel_id: String,
    synced_count: usize,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WeixinOcCredentials {
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    bot_type: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    qr_poll_interval: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    long_poll_timeout_ms: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    api_timeout_ms: Option<u64>,
    #[serde(default)]
    token: String,
    #[serde(default)]
    account_id: String,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    sync_buf: String,
}

impl WeixinOcCredentials {
    fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    fn normalized_base_url(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            WEIXIN_OC_DEFAULT_BASE_URL.to_string()
        } else {
            base.to_string()
        }
    }

    fn normalized_bot_type(&self) -> String {
        let out = self.bot_type.trim();
        if out.is_empty() {
            WEIXIN_OC_DEFAULT_BOT_TYPE.to_string()
        } else {
            out.to_string()
        }
    }

    fn normalized_long_poll_timeout_ms(&self) -> u64 {
        self.long_poll_timeout_ms
            .unwrap_or(WEIXIN_OC_DEFAULT_LONG_POLL_TIMEOUT_MS)
            .clamp(5_000, 60_000)
    }

    fn normalized_api_timeout_ms(&self) -> u64 {
        self.api_timeout_ms
            .unwrap_or(WEIXIN_OC_DEFAULT_API_TIMEOUT_MS)
            .clamp(5_000, 60_000)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcGetBotQrCodeResp {
    qrcode: Option<String>,
    qrcode_img_content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcQrStatusResp {
    ret: Option<i64>,
    errcode: Option<i64>,
    errmsg: Option<String>,
    status: Option<String>,
    #[serde(alias = "botToken")]
    bot_token: Option<String>,
    #[serde(alias = "ilinkBotId")]
    ilink_bot_id: Option<String>,
    #[serde(alias = "ilinkUserId")]
    ilink_user_id: Option<String>,
    #[serde(alias = "baseUrl")]
    baseurl: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcGetUpdatesResp {
    ret: Option<i64>,
    errcode: Option<i64>,
    errmsg: Option<String>,
    msgs: Option<Vec<WeixinOcInboundMessage>>,
    get_updates_buf: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcInboundMessage {
    message_id: Option<Value>,
    msg_id: Option<Value>,
    from_user_id: Option<String>,
    context_token: Option<String>,
    item_list: Option<Vec<WeixinOcMessageItem>>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcMessageItem {
    #[serde(rename = "type")]
    item_type: Option<i64>,
    text_item: Option<WeixinOcTextItem>,
    image_item: Option<WeixinOcImageItem>,
    voice_item: Option<WeixinOcVoiceItem>,
    file_item: Option<WeixinOcFileItem>,
    video_item: Option<WeixinOcVideoItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcTextItem {
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcImageItem {
    media: Option<WeixinOcMediaPayload>,
    aeskey: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcMediaPayload {
    encrypt_query_param: Option<String>,
    aes_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcVoiceItem {
    media: Option<WeixinOcMediaPayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcFileItem {
    media: Option<WeixinOcMediaPayload>,
    file_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WeixinOcVideoItem {
    media: Option<WeixinOcMediaPayload>,
}

#[derive(Debug, Clone)]
struct WeixinOcCollectedMedia {
    images: Vec<BinaryPart>,
    audios: Vec<BinaryPart>,
    attachments: Vec<AttachmentMetaInput>,
}

#[derive(Debug, Clone)]
struct WeixinOcRuntimeState {
    connected: bool,
    connected_at: Option<chrono::DateTime<chrono::Utc>>,
    base_url: String,
    account_id: String,
    user_id: String,
    session_key: String,
    qrcode: String,
    qrcode_img_content: String,
    login_status: String,
    last_error: String,
}

impl Default for WeixinOcRuntimeState {
    fn default() -> Self {
        Self {
            connected: false,
            connected_at: None,
            base_url: WEIXIN_OC_DEFAULT_BASE_URL.to_string(),
            account_id: String::new(),
            user_id: String::new(),
            session_key: String::new(),
            qrcode: String::new(),
            qrcode_img_content: String::new(),
            login_status: "idle".to_string(),
            last_error: String::new(),
        }
    }
}

pub struct WeixinOcManager {
    states: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, WeixinOcRuntimeState>>,
    >,
    login_sessions: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, WeixinOcLoginSession>>,
    >,
    stop_senders: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
    >,
    tasks: std::sync::Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<String, tauri::async_runtime::JoinHandle<()>>,
        >,
    >,
    context_tokens: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, String>>,
    >,
}

impl WeixinOcManager {
    pub fn new() -> Self {
        Self {
            states: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            login_sessions: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            stop_senders: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            context_tokens: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for WeixinOcManager {
    fn default() -> Self {
        Self::new()
    }
}

static WEIXIN_OC_MANAGER: once_cell::sync::Lazy<std::sync::Arc<WeixinOcManager>> =
    once_cell::sync::Lazy::new(|| std::sync::Arc::new(WeixinOcManager::new()));

fn weixin_oc_manager() -> std::sync::Arc<WeixinOcManager> {
    WEIXIN_OC_MANAGER.clone()
}

fn login_session_is_fresh(login: &WeixinOcLoginSession) -> bool {
    chrono::DateTime::parse_from_rfc3339(login.started_at.trim())
        .map(|ts| {
            chrono::Utc::now()
                .signed_duration_since(ts.with_timezone(&chrono::Utc))
                .num_seconds()
                < WEIXIN_OC_LOGIN_TTL_SECS
        })
        .unwrap_or(false)
}

fn build_weixin_oc_http_client(timeout_ms: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| format!("创建个人微信 HTTP 客户端失败: {err}"))
}

fn weixin_oc_cdn_download_url(encrypted_query_param: &str) -> String {
    format!(
        "{}/download?encrypted_query_param={}",
        WEIXIN_OC_DEFAULT_CDN_BASE_URL.trim_end_matches('/'),
        urlencoding::encode(encrypted_query_param.trim())
    )
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
    let padded = format!(
        "{}{}",
        normalized,
        "=".repeat((4usize.wrapping_sub(normalized.len() % 4)) % 4)
    );
    let decoded = B64
        .decode(padded.as_bytes())
        .map_err(|err| format!("解析媒体 AES 密钥失败: {err}"))?;
    if decoded.len() == 16 {
        return Ok(decoded);
    }
    if decoded.len() == 32
        && decoded
            .iter()
            .all(|byte| (*byte as char).is_ascii_hexdigit())
    {
        let hex_text =
            std::str::from_utf8(&decoded).map_err(|err| format!("解析媒体 AES 十六进制失败: {err}"))?;
        return weixin_oc_decode_hex(hex_text);
    }
    Err("媒体 AES 密钥格式不支持".to_string())
}

fn weixin_oc_decrypt_media_ecb(encrypted: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};

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
        let block = GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }
    Ok(weixin_oc_pkcs7_unpad(&decrypted))
}

async fn weixin_oc_download_image_bytes(
    client: &reqwest::Client,
    encrypted_query_param: &str,
    aes_key_value: Option<&str>,
) -> Result<Vec<u8>, String> {
    let resp = client
        .get(weixin_oc_cdn_download_url(encrypted_query_param))
        .send()
        .await
        .map_err(|err| format!("下载个人微信图片失败: {err}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("下载个人微信图片失败: status={} body={}", status, body));
    }
    let encrypted = resp
        .bytes()
        .await
        .map_err(|err| format!("读取个人微信图片响应失败: {err}"))?;
    if let Some(value) = aes_key_value.map(str::trim).filter(|value| !value.is_empty()) {
        let key = weixin_oc_parse_media_aes_key(value)?;
        return weixin_oc_decrypt_media_ecb(encrypted.as_ref(), &key);
    }
    Ok(encrypted.to_vec())
}

fn weixin_oc_normalize_image_mime(raw: &[u8]) -> String {
    match image::guess_format(raw) {
        Ok(image::ImageFormat::Png) => "image/png".to_string(),
        Ok(image::ImageFormat::Jpeg) => "image/jpeg".to_string(),
        Ok(image::ImageFormat::Gif) => "image/gif".to_string(),
        Ok(image::ImageFormat::WebP) => "image/webp".to_string(),
        _ => "image/jpeg".to_string(),
    }
}

fn weixin_oc_guess_attachment_mime(file_name: &str, fallback: &str) -> String {
    media_mime_from_path(std::path::Path::new(file_name))
        .unwrap_or(fallback)
        .to_string()
}

fn weixin_oc_build_attachment_meta(
    state: &AppState,
    file_name: &str,
    mime: &str,
    raw: &[u8],
) -> Result<(AttachmentMetaInput, String), String> {
    let saved = persist_raw_attachment_to_downloads(state, file_name, mime, raw)?;
    let relative_path = workspace_relative_path(state, &saved);
    let final_file_name = saved
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(file_name)
        .to_string();
    Ok((
        AttachmentMetaInput {
            file_name: final_file_name,
            relative_path: relative_path.clone(),
            mime: mime.to_string(),
        },
        relative_path,
    ))
}

async fn weixin_oc_collect_media(
    state: &AppState,
    client: &reqwest::Client,
    item_list: &[WeixinOcMessageItem],
) -> Result<WeixinOcCollectedMedia, String> {
    let mut images = Vec::<BinaryPart>::new();
    let mut audios = Vec::<BinaryPart>::new();
    let mut attachments = Vec::<AttachmentMetaInput>::new();
    for item in item_list {
        let item_type = item.item_type.unwrap_or(0);
        let (media, file_name, fallback_mime, aes_key_override) = match item_type {
            2 => {
                let Some(image_item) = item.image_item.as_ref() else {
                    continue;
                };
                let Some(media) = image_item.media.as_ref() else {
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
                        .map(|value| B64.encode(value)),
                )
            }
            3 => {
                let Some(voice_item) = item.voice_item.as_ref() else {
                    continue;
                };
                let Some(media) = voice_item.media.as_ref() else {
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
                    continue;
                };
                let Some(media) = file_item.media.as_ref() else {
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
                    continue;
                };
                let Some(media) = video_item.media.as_ref() else {
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
        let Some(encrypted_query_param) = media
            .encrypt_query_param
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let aes_key_value = aes_key_override.or_else(|| {
            media.aes_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
        let raw = weixin_oc_download_image_bytes(
            client,
            encrypted_query_param,
            aes_key_value.as_deref(),
        )
        .await?;
        let mime = if item_type == 2 {
            weixin_oc_normalize_image_mime(&raw)
        } else {
            fallback_mime
        };
        let (attachment, relative_path) =
            weixin_oc_build_attachment_meta(state, &file_name, &mime, &raw)?;
        let bytes_base64 = B64.encode(&raw);
        attachments.push(attachment);
        match item_type {
            2 => images.push(BinaryPart {
                mime,
                bytes_base64,
                saved_path: Some(relative_path),
            }),
            3 => audios.push(BinaryPart {
                mime,
                bytes_base64,
                saved_path: Some(relative_path),
            }),
            4 | 5 => {}
            _ => {}
        }
    }
    Ok(WeixinOcCollectedMedia {
        images,
        audios,
        attachments,
    })
}

fn weixin_oc_random_wechat_uin() -> String {
    let bytes = *Uuid::new_v4().as_bytes();
    let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    B64.encode(value.to_string())
}

fn weixin_oc_is_login_confirmed(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "confirmed" | "confirm" | "success" | "logged_in" | "login_success"
    )
}

fn weixin_oc_request_headers(
    body: &str,
    token: Option<&str>,
) -> Result<reqwest::header::HeaderMap, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "AuthorizationType",
        reqwest::header::HeaderValue::from_static("ilink_bot_token"),
    );
    headers.insert(
        "X-WECHAT-UIN",
        reqwest::header::HeaderValue::from_str(weixin_oc_random_wechat_uin().as_str())
            .map_err(|err| format!("构造 X-WECHAT-UIN 失败: {err}"))?,
    );
    headers.insert(
        reqwest::header::CONTENT_LENGTH,
        reqwest::header::HeaderValue::from_str(body.len().to_string().as_str())
            .map_err(|err| format!("构造 Content-Length 失败: {err}"))?,
    );
    if let Some(value) = token.map(str::trim).filter(|value| !value.is_empty()) {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(format!("Bearer {value}").as_str())
                .map_err(|err| format!("构造 Authorization 失败: {err}"))?,
        );
    }
    Ok(headers)
}

impl WeixinOcManager {
    async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        napcat_ws_manager().add_log(channel_id, level, message).await;
    }

    async fn set_state<F>(&self, channel_id: &str, update: F)
    where
        F: FnOnce(&mut WeixinOcRuntimeState),
    {
        let mut states = self.states.write().await;
        let state = states
            .entry(channel_id.to_string())
            .or_insert_with(WeixinOcRuntimeState::default);
        let was_connected = state.connected;
        update(state);
        if state.connected && !was_connected {
            state.connected_at = Some(chrono::Utc::now());
        }
        if !state.connected {
            state.connected_at = None;
        }
    }

    async fn load_state_from_channel(&self, channel: &RemoteImChannelConfig) {
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        self.set_state(&channel.id, |state| {
            state.base_url = creds.normalized_base_url();
            state.account_id = creds.account_id.trim().to_string();
            state.user_id = creds.user_id.trim().to_string();
            if state.login_status == "idle" && !creds.token.trim().is_empty() {
                state.login_status = "logged_in".to_string();
            }
        })
        .await;
    }

    async fn build_status(&self, channel_id: &str) -> ChannelConnectionStatus {
        let state = self
            .states
            .read()
            .await
            .get(channel_id)
            .cloned()
            .unwrap_or_default();
        ChannelConnectionStatus {
            channel_id: channel_id.to_string(),
            connected: state.connected,
            peer_addr: if state.account_id.trim().is_empty() {
                None
            } else {
                Some(state.account_id.clone())
            },
            connected_at: state.connected_at,
            listen_addr: String::new(),
            status_text: Some(state.login_status),
            last_error: if state.last_error.trim().is_empty() {
                None
            } else {
                Some(state.last_error)
            },
            account_id: if state.account_id.trim().is_empty() {
                None
            } else {
                Some(state.account_id)
            },
            base_url: Some(state.base_url),
            login_session_key: if state.session_key.trim().is_empty() {
                None
            } else {
                Some(state.session_key)
            },
            qrcode_url: if state.qrcode_img_content.trim().is_empty() {
                None
            } else {
                Some(state.qrcode_img_content)
            },
        }
    }

    async fn set_context_token(&self, channel_id: &str, user_id: &str, token: &str) {
        if user_id.trim().is_empty() || token.trim().is_empty() {
            return;
        }
        self.context_tokens.write().await.insert(
            format!("{}:{}", channel_id.trim(), user_id.trim()),
            token.trim().to_string(),
        );
    }

    async fn get_context_token(&self, channel_id: &str, user_id: &str) -> Option<String> {
        self.context_tokens
            .read()
            .await
            .get(&format!("{}:{}", channel_id.trim(), user_id.trim()))
            .cloned()
    }

    async fn stop_channel(&self, channel_id: &str) {
        if let Some(tx) = self.stop_senders.write().await.remove(channel_id) {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.tasks.write().await.remove(channel_id) {
            let _ = handle.await;
        }
        self.set_state(channel_id, |state| {
            state.connected = false;
        })
        .await;
    }

    async fn reconcile_channel_runtime(
        &self,
        channel: &RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        eprintln!(
            "[个人微信] reconcile_channel_runtime: channel_id={}, enabled={}, platform={:?}",
            channel.id, channel.enabled, channel.platform
        );
        self.load_state_from_channel(channel).await;
        self.stop_channel(&channel.id).await;
        if channel.platform != RemoteImPlatform::WeixinOc || !channel.enabled {
            eprintln!("[个人微信] 渠道已停用: channel_id={}", channel.id);
            self.add_log(&channel.id, "info", "[个人微信] 渠道已停用").await;
            return Ok(());
        }
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        eprintln!(
            "[个人微信] 当前凭证: channel_id={}, base_url={}, token_len={}, account_id={}, user_id={}",
            channel.id,
            creds.normalized_base_url(),
            creds.token.trim().len(),
            creds.account_id.trim(),
            creds.user_id.trim()
        );
        if creds.token.trim().is_empty() {
            eprintln!("[个人微信] 渠道已启用，但尚未登录（缺少 token）: channel_id={}", channel.id);
            self.set_state(&channel.id, |runtime| {
                runtime.connected = false;
                runtime.login_status = "need_login".to_string();
            })
            .await;
            self.add_log(&channel.id, "info", "[个人微信] 渠道已启用，但尚未登录（缺少 token）")
                .await;
            return Ok(());
        }
        eprintln!("[个人微信] 渠道已启用，正在启动轮询: channel_id={}", channel.id);
        self.add_log(&channel.id, "info", "[个人微信] 渠道已启用，正在启动轮询")
            .await;
        self.start_channel(channel.clone(), state).await
    }

    async fn start_channel(
        &self,
        channel: RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        let channel_id = channel.id.clone();
        eprintln!("[个人微信] start_channel: channel_id={}", channel_id);
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        self.stop_senders
            .write()
            .await
            .insert(channel_id.clone(), stop_tx);
        let manager = weixin_oc_manager();
        let task_channel_id = channel_id.clone();
        let handle = tauri::async_runtime::spawn(async move {
            manager
                .add_log(&task_channel_id, "info", "[个人微信] 轮询任务开始")
                .await;
            let mut stop_rx = stop_rx;
            loop {
                if *stop_rx.borrow() {
                    break;
                }
                let result = tokio::select! {
                    changed = stop_rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if *stop_rx.borrow() {
                                    break;
                                }
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    ret = run_single_weixin_oc_poll_cycle(&channel.id, &state) => ret,
                };
                if let Err(err) = result {
                    manager
                        .set_state(&task_channel_id, |runtime| {
                            runtime.connected = false;
                            runtime.last_error = err.clone();
                        })
                        .await;
                    manager
                        .add_log(
                            &task_channel_id,
                            "warn",
                            &format!("[个人微信] 拉取消息失败: {}", err),
                        )
                        .await;
                    tokio::select! {
                        changed = stop_rx.changed() => {
                            match changed {
                                Ok(()) => {
                                    if *stop_rx.borrow() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                    }
                } else {
                    manager
                        .set_state(&task_channel_id, |runtime| {
                            runtime.connected = true;
                            if runtime.login_status.trim().is_empty() || runtime.login_status == "need_login" {
                                runtime.login_status = "logged_in".to_string();
                            }
                        })
                        .await;
                }
            }
            manager
                .set_state(&task_channel_id, |runtime| {
                    runtime.connected = false;
                })
                .await;
            manager
                .add_log(&task_channel_id, "info", "[个人微信] 轮询任务结束")
                .await;
        });
        self.tasks.write().await.insert(channel_id, handle);
        Ok(())
    }

    async fn start_login(
        &self,
        state: &AppState,
        input: WeixinOcLoginStartInput,
    ) -> Result<WeixinOcLoginStartResult, String> {
        let config = state_read_config_cached(state)?;
        let channel = remote_im_channel_by_id(&config, &input.channel_id)
            .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
        if channel.platform != RemoteImPlatform::WeixinOc {
            return Err("该渠道不是个人微信渠道".to_string());
        }
        self.load_state_from_channel(channel).await;
        if !input.force_refresh {
            if let Some(existing) = self
                .login_sessions
                .read()
                .await
                .get(&input.channel_id)
                .cloned()
            {
                if login_session_is_fresh(&existing) {
                    return Ok(WeixinOcLoginStartResult {
                        channel_id: input.channel_id,
                        session_key: existing.session_key,
                        qrcode: existing.qrcode,
                        qrcode_img_content: existing.qrcode_img_content,
                        status: existing.status,
                        message: "二维码已就绪，请使用微信扫码。".to_string(),
                    });
                }
            }
        }
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        let client = build_weixin_oc_http_client(creds.normalized_api_timeout_ms())?;
        let url = format!(
            "{}/ilink/bot/get_bot_qrcode",
            creds.normalized_base_url().trim_end_matches('/')
        );
        let resp = client
            .get(url)
            .query(&[("bot_type", creds.normalized_bot_type())])
            .send()
            .await
            .map_err(|err| format!("请求二维码失败: {err}"))?;
        let status_code = resp.status();
        if !status_code.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("请求二维码失败: status={} body={}", status_code, text));
        }
        let data = resp
            .json::<WeixinOcGetBotQrCodeResp>()
            .await
            .map_err(|err| format!("解析二维码响应失败: {err}"))?;
        let qrcode = data.qrcode.unwrap_or_default().trim().to_string();
        let qrcode_img_content = data
            .qrcode_img_content
            .unwrap_or_default()
            .trim()
            .to_string();
        if qrcode.is_empty() || qrcode_img_content.is_empty() {
            return Err("二维码响应缺少 qrcode 或 qrcode_img_content".to_string());
        }
        let session = WeixinOcLoginSession {
            session_key: Uuid::new_v4().to_string(),
            qrcode: qrcode.clone(),
            qrcode_img_content: qrcode_img_content.clone(),
            started_at: now_iso(),
            status: "wait".to_string(),
            error: String::new(),
        };
        self.login_sessions
            .write()
            .await
            .insert(input.channel_id.clone(), session.clone());
        self.set_state(&input.channel_id, |runtime| {
            runtime.session_key = session.session_key.clone();
            runtime.qrcode = session.qrcode.clone();
            runtime.qrcode_img_content = session.qrcode_img_content.clone();
            runtime.login_status = session.status.clone();
            runtime.last_error.clear();
        })
        .await;
        self.add_log(
            &input.channel_id,
            "info",
            &format!("[个人微信] 已生成扫码二维码: {}", qrcode_img_content),
        )
        .await;
        Ok(WeixinOcLoginStartResult {
            channel_id: input.channel_id,
            session_key: session.session_key,
            qrcode,
            qrcode_img_content,
            status: "wait".to_string(),
            message: "请使用微信扫码登录。".to_string(),
        })
    }

    async fn poll_login_status(
        &self,
        state: &AppState,
        input: WeixinOcLoginStatusInput,
    ) -> Result<WeixinOcLoginStatusResult, String> {
        let mut login = {
            let sessions = self.login_sessions.read().await;
            sessions
                .get(&input.channel_id)
                .cloned()
                .ok_or_else(|| "当前没有进行中的扫码登录".to_string())?
        };
        if !login_session_is_fresh(&login) {
            self.login_sessions.write().await.remove(&input.channel_id);
            self.set_state(&input.channel_id, |runtime| {
                runtime.login_status = "expired".to_string();
                runtime.last_error = "二维码已过期，请重新生成".to_string();
            })
            .await;
            return Ok(WeixinOcLoginStatusResult {
                channel_id: input.channel_id,
                connected: false,
                status: "expired".to_string(),
                message: "二维码已过期，请重新生成。".to_string(),
                session_key: login.session_key,
                qrcode: login.qrcode,
                qrcode_img_content: login.qrcode_img_content,
                account_id: String::new(),
                user_id: String::new(),
                base_url: String::new(),
                last_error: "二维码已过期，请重新生成".to_string(),
            });
        }
        let config = state_read_config_cached(state)?;
        let channel = remote_im_channel_by_id(&config, &input.channel_id)
            .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        let client = build_weixin_oc_http_client(creds.normalized_long_poll_timeout_ms())?;
        let url = format!(
            "{}/ilink/bot/get_qrcode_status",
            creds.normalized_base_url().trim_end_matches('/')
        );
        let resp = client
            .get(url)
            .query(&[("qrcode", login.qrcode.clone())])
            .header("iLink-App-ClientVersion", "1")
            .send()
            .await
            .map_err(|err| format!("查询二维码状态失败: {err}"))?;
        let status_code = resp.status();
        if !status_code.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("查询二维码状态失败: status={} body={}", status_code, text));
        }
        let data = resp
            .json::<WeixinOcQrStatusResp>()
            .await
            .map_err(|err| format!("解析二维码状态失败: {err}"))?;
        if data.ret.unwrap_or(0) != 0 || data.errcode.unwrap_or(0) != 0 {
            return Err(format!(
                "查询二维码状态失败: ret={} errcode={} errmsg={}",
                data.ret.unwrap_or(0),
                data.errcode.unwrap_or(0),
                data.errmsg.unwrap_or_default()
            ));
        }
        let status = data.status.unwrap_or_else(|| "wait".to_string());
        login.status = status.clone();
        if weixin_oc_is_login_confirmed(&status) {
            let bot_token = data.bot_token.unwrap_or_default().trim().to_string();
            let account_id = data.ilink_bot_id.unwrap_or_default().trim().to_string();
            let user_id = data.ilink_user_id.unwrap_or_default().trim().to_string();
            let base_url = data
                .baseurl
                .unwrap_or_else(|| creds.normalized_base_url())
                .trim()
                .trim_end_matches('/')
                .to_string();
            if bot_token.is_empty() || account_id.is_empty() {
                login.error = "扫码已确认，等待凭证返回".to_string();
                self.login_sessions
                    .write()
                    .await
                    .insert(input.channel_id.clone(), login.clone());
                self.set_state(&input.channel_id, |runtime| {
                    runtime.session_key = login.session_key.clone();
                    runtime.qrcode = login.qrcode.clone();
                    runtime.qrcode_img_content = login.qrcode_img_content.clone();
                    runtime.login_status = status.clone();
                    runtime.last_error = login.error.clone();
                })
                .await;
                return Ok(WeixinOcLoginStatusResult {
                    channel_id: input.channel_id,
                    connected: false,
                    status,
                    message: "扫码已确认，等待微信返回凭证。".to_string(),
                    session_key: login.session_key,
                    qrcode: login.qrcode,
                    qrcode_img_content: login.qrcode_img_content,
                    account_id: String::new(),
                    user_id: String::new(),
                    base_url: creds.normalized_base_url(),
                    last_error: login.error,
                });
            }
            let updated_channel = {
                let guard = state
                    .state_lock
                    .lock()
                    .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
                let mut writable = state_read_config_cached(state)?;
                let writable_channel = writable
                    .remote_im_channels
                    .iter_mut()
                    .find(|item| item.id == input.channel_id)
                    .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
                let mut writable_creds = WeixinOcCredentials::from_value(&writable_channel.credentials);
                writable_creds.token = bot_token.clone();
                writable_creds.account_id = account_id.clone();
                writable_creds.user_id = user_id.clone();
                writable_creds.base_url = base_url.clone();
                writable_channel.credentials = serde_json::to_value(&writable_creds)
                    .map_err(|err| format!("序列化个人微信凭证失败: {err}"))?;
                let updated_channel = writable_channel.clone();
                state_write_config_cached(state, &writable)?;
                drop(guard);
                updated_channel
            };
            self.login_sessions.write().await.remove(&input.channel_id);
            self.set_state(&input.channel_id, |runtime| {
                runtime.connected = false;
                runtime.account_id = account_id.clone();
                runtime.user_id = user_id.clone();
                runtime.base_url = base_url.clone();
                runtime.login_status = "confirmed".to_string();
                runtime.last_error.clear();
                runtime.session_key.clear();
                runtime.qrcode.clear();
                runtime.qrcode_img_content.clear();
            })
            .await;
            self.add_log(
                &input.channel_id,
                "info",
                &format!(
                    "[个人微信] 扫码登录成功: account_id={}, user_id={}",
                    account_id, user_id
                ),
            )
            .await;
            if !user_id.is_empty() {
                let (_, created) = sync_weixin_oc_contact_from_user_id(state, channel, &user_id)?;
                let log_message = if created {
                    format!("[个人微信] 已自动补录联系人: {}", user_id)
                } else {
                    format!("[个人微信] 联系人已存在，跳过补录: {}", user_id)
                };
                self.add_log(&input.channel_id, "info", &log_message).await;
            }
            if updated_channel.enabled {
                self.reconcile_channel_runtime(&updated_channel, state.clone()).await?;
            }
            return Ok(WeixinOcLoginStatusResult {
                channel_id: input.channel_id,
                connected: true,
                status: "confirmed".to_string(),
                message: "扫码登录成功。".to_string(),
                session_key: String::new(),
                qrcode: String::new(),
                qrcode_img_content: String::new(),
                account_id,
                user_id,
                base_url,
                last_error: String::new(),
            });
        }
        if status == "expired" {
            self.login_sessions.write().await.remove(&input.channel_id);
            self.set_state(&input.channel_id, |runtime| {
                runtime.login_status = "expired".to_string();
                runtime.last_error = "二维码已过期，请重新生成".to_string();
            })
            .await;
            return Ok(WeixinOcLoginStatusResult {
                channel_id: input.channel_id,
                connected: false,
                status,
                message: "二维码已过期，请重新生成。".to_string(),
                session_key: login.session_key,
                qrcode: login.qrcode,
                qrcode_img_content: login.qrcode_img_content,
                account_id: String::new(),
                user_id: String::new(),
                base_url: creds.normalized_base_url(),
                last_error: "二维码已过期，请重新生成".to_string(),
            });
        }
        self.login_sessions
            .write()
            .await
            .insert(input.channel_id.clone(), login.clone());
        self.set_state(&input.channel_id, |runtime| {
            runtime.session_key = login.session_key.clone();
            runtime.qrcode = login.qrcode.clone();
            runtime.qrcode_img_content = login.qrcode_img_content.clone();
            runtime.login_status = status.clone();
            runtime.last_error = login.error.clone();
        })
        .await;
        Ok(WeixinOcLoginStatusResult {
            channel_id: input.channel_id,
            connected: false,
            status,
            message: "等待扫码确认。".to_string(),
            session_key: login.session_key,
            qrcode: login.qrcode,
            qrcode_img_content: login.qrcode_img_content,
            account_id: String::new(),
            user_id: String::new(),
            base_url: creds.normalized_base_url(),
            last_error: login.error,
        })
    }

    async fn logout(&self, state: &AppState, channel_id: &str) -> Result<(), String> {
        self.stop_channel(channel_id).await;
        self.login_sessions.write().await.remove(channel_id);
        {
            let guard = state
                .state_lock
                .lock()
                .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
            let mut writable = state_read_config_cached(state)?;
            let channel = writable
                .remote_im_channels
                .iter_mut()
                .find(|item| item.id == channel_id)
                .ok_or_else(|| format!("渠道不存在: {}", channel_id))?;
            let mut creds = WeixinOcCredentials::from_value(&channel.credentials);
            creds.token.clear();
            creds.account_id.clear();
            creds.user_id.clear();
            creds.sync_buf.clear();
            channel.credentials = serde_json::to_value(&creds)
                .map_err(|err| format!("序列化个人微信凭证失败: {err}"))?;
            state_write_config_cached(state, &writable)?;
            drop(guard);
        }
        self.set_state(channel_id, |runtime| {
            *runtime = WeixinOcRuntimeState::default();
            runtime.login_status = "logged_out".to_string();
        })
        .await;
        self.add_log(channel_id, "info", "[个人微信] 已退出登录").await;
        Ok(())
    }
}

fn weixin_oc_message_text(item_list: &[WeixinOcMessageItem]) -> String {
    let mut parts = Vec::<String>::new();
    for item in item_list {
        match item.item_type.unwrap_or(0) {
            1 => {
                let text = item
                    .text_item
                    .as_ref()
                    .and_then(|value| value.text.as_deref())
                    .map(str::trim)
                    .unwrap_or("");
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
            _ => {}
        }
    }
    parts.join("\n").trim().to_string()
}

fn weixin_oc_contact_display_name(channel: &RemoteImChannelConfig, user_id: &str) -> String {
    let channel_name = channel.name.trim();
    if !channel_name.is_empty() {
        return channel_name.to_string();
    }
    let normalized_user_id = user_id.trim();
    if !normalized_user_id.is_empty() {
        return normalized_user_id.to_string();
    }
    "个人微信".to_string()
}

async fn handle_weixin_oc_inbound_message(
    channel: &RemoteImChannelConfig,
    state: &AppState,
    msg: WeixinOcInboundMessage,
) -> Result<(), String> {
    let from_user_id = msg
        .from_user_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if from_user_id.is_empty() {
        return Ok(());
    }
    if let Some(token) = msg
        .context_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        weixin_oc_manager()
            .set_context_token(&channel.id, from_user_id, token)
            .await;
    }
    let item_list = msg.item_list.unwrap_or_default();
    let text = weixin_oc_message_text(&item_list);
    let creds = WeixinOcCredentials::from_value(&channel.credentials);
    let client = build_weixin_oc_http_client(creds.normalized_api_timeout_ms())?;
    let media = weixin_oc_collect_media(state, &client, &item_list).await?;
    let display_name = weixin_oc_contact_display_name(channel, from_user_id);
    let message_id = msg
        .message_id
        .or(msg.msg_id)
        .map(|value| value.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    remote_im_enqueue_message_internal(
        RemoteImEnqueueInput {
            channel_id: channel.id.clone(),
            platform: RemoteImPlatform::WeixinOc,
            im_name: "weixin".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: from_user_id.to_string(),
            remote_contact_name: Some(display_name.clone()),
            sender_id: from_user_id.to_string(),
            sender_name: display_name,
            sender_avatar_url: None,
            platform_message_id: Some(message_id),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            activate_assistant: Some(channel.activate_assistant),
            session: SessionSelector {
                api_config_id: None,
                conversation_id: None,
                department_id: None,
                agent_id: String::new(),
            },
            payload: ChatInputPayload {
                text: if text.is_empty() { None } else { Some(text.clone()) },
                display_text: if text.is_empty() { None } else { Some(text) },
                images: if media.images.is_empty() {
                    None
                } else {
                    Some(media.images)
                },
                audios: if media.audios.is_empty() {
                    None
                } else {
                    Some(media.audios)
                },
                attachments: if media.attachments.is_empty() {
                    None
                } else {
                    Some(media.attachments)
                },
                model: None,
                extra_text_blocks: None,
                provider_meta: msg.context_token.map(|token| {
                    serde_json::json!({
                        "contextToken": token,
                    })
                }),
            },
        },
        state,
    )?;
    Ok(())
}

async fn run_single_weixin_oc_poll_cycle(
    channel_id: &str,
    state: &AppState,
) -> Result<(), String> {
    let channel = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let config = state_read_config_cached(state)?;
        let channel = config
            .remote_im_channels
            .iter()
            .find(|item| item.id == channel_id)
            .cloned()
            .ok_or_else(|| format!("个人微信渠道不存在: {channel_id}"))?;
        drop(guard);
        channel
    };
    let creds = WeixinOcCredentials::from_value(&channel.credentials);
    let token = creds.token.trim().to_string();
    if token.is_empty() {
        return Err("缺少 token，请先扫码登录".to_string());
    }
    let body = serde_json::json!({
        "base_info": {
            "channel_version": "easy_call_ai"
        },
        "get_updates_buf": creds.sync_buf,
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 getupdates 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(&token))?;
    let client = build_weixin_oc_http_client(creds.normalized_long_poll_timeout_ms())?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/getupdates",
            creds.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 getupdates 失败: {err}"))?;
    let status_code = resp.status();
    if !status_code.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("请求 getupdates 失败: status={} body={}", status_code, text));
    }
    let data = resp
        .json::<WeixinOcGetUpdatesResp>()
        .await
        .map_err(|err| format!("解析 getupdates 响应失败: {err}"))?;
    if data.ret.unwrap_or(0) != 0 || data.errcode.unwrap_or(0) != 0 {
        return Err(format!(
            "getupdates 返回错误: ret={} errcode={} errmsg={}",
            data.ret.unwrap_or(0),
            data.errcode.unwrap_or(0),
            data.errmsg.unwrap_or_default()
        ));
    }
    if let Some(next_sync_buf) = data
        .get_updates_buf
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let mut writable = state_read_config_cached(state)?;
        if let Some(writable_channel) = writable
            .remote_im_channels
            .iter_mut()
            .find(|item| item.id == channel.id)
        {
            let mut next_creds = WeixinOcCredentials::from_value(&writable_channel.credentials);
            if next_creds.sync_buf.trim() != next_sync_buf {
                next_creds.sync_buf = next_sync_buf.to_string();
                writable_channel.credentials = serde_json::to_value(&next_creds)
                    .map_err(|err| format!("序列化个人微信凭证失败: {err}"))?;
                state_write_config_cached(state, &writable)?;
            }
        }
        drop(guard);
    }
    for msg in data.msgs.unwrap_or_default() {
        handle_weixin_oc_inbound_message(&channel, state, msg).await?;
    }
    Ok(())
}

fn upsert_weixin_oc_contact(
    data: &mut AppData,
    channel: &RemoteImChannelConfig,
    user_id: &str,
) -> (String, bool) {
    let normalized_user_id = user_id.trim();
    let display_name = weixin_oc_contact_display_name(channel, normalized_user_id);
    if let Some(contact) = data.remote_im_contacts.iter_mut().find(|item| {
        item.channel_id == channel.id
            && item.remote_contact_type == "private"
            && item.remote_contact_id == normalized_user_id
    }) {
        let current_name = contact.remote_contact_name.trim();
        if current_name.is_empty() || current_name == normalized_user_id {
            contact.remote_contact_name = display_name;
        }
        return (contact.id.clone(), false);
    }

    let contact_id = Uuid::new_v4().to_string();
    data.remote_im_contacts.push(RemoteImContact {
        id: contact_id.clone(),
        channel_id: channel.id.clone(),
        platform: RemoteImPlatform::WeixinOc,
        remote_contact_type: "private".to_string(),
        remote_contact_id: normalized_user_id.to_string(),
        remote_contact_name: display_name,
        remark_name: String::new(),
        allow_send: true,
        allow_send_files: false,
        allow_receive: channel.activate_assistant,
        activation_mode: "never".to_string(),
        activation_keywords: Vec::new(),
        activation_cooldown_seconds: 0,
        route_mode: "main_session".to_string(),
        bound_department_id: None,
        bound_conversation_id: None,
        processing_mode: "continuous".to_string(),
        last_activated_at: None,
        last_message_at: None,
        dingtalk_session_webhook: None,
        dingtalk_session_webhook_expired_time: None,
    });
    (contact_id, true)
}

fn sync_weixin_oc_contact_from_user_id(
    state: &AppState,
    channel: &RemoteImChannelConfig,
    user_id: &str,
) -> Result<(String, bool), String> {
    let normalized_user_id = user_id.trim();
    if normalized_user_id.is_empty() {
        return Err("当前登录状态没有返回联系人 user_id，暂时无法补录联系人".to_string());
    }
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(state)?;
    let result = upsert_weixin_oc_contact(&mut data, channel, normalized_user_id);
    state_write_app_data_cached(state, &data)?;
    drop(guard);
    Ok(result)
}

pub(crate) async fn weixin_oc_send_text_message(
    credentials: WeixinOcCredentials,
    to_user_id: &str,
    text: &str,
    context_token: Option<&str>,
) -> Result<String, String> {
    let client = build_weixin_oc_http_client(credentials.normalized_api_timeout_ms())?;
    let client_id = Uuid::new_v4().to_string();
    let body = serde_json::json!({
        "base_info": {
            "channel_version": "easy_call_ai"
        },
        "msg": {
            "from_user_id": "",
            "to_user_id": to_user_id,
            "client_id": client_id,
            "message_type": 2,
            "message_state": 2,
            "context_token": context_token.map(str::trim).filter(|value| !value.is_empty()),
            "item_list": [
                {
                    "type": 1,
                    "text_item": {
                        "text": text
                    }
                }
            ]
        }
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 sendmessage 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(credentials.token.as_str()))?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/sendmessage",
            credentials.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 sendmessage 失败: {err}"))?;
    let status_code = resp.status();
    if !status_code.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("请求 sendmessage 失败: status={} body={}", status_code, body));
    }
    Ok(client_id)
}

#[tauri::command]
async fn remote_im_weixin_oc_start_login(
    input: WeixinOcLoginStartInput,
    state: State<'_, AppState>,
) -> Result<WeixinOcLoginStartResult, String> {
    weixin_oc_manager().start_login(state.inner(), input).await
}

#[tauri::command]
async fn remote_im_weixin_oc_get_login_status(
    input: WeixinOcLoginStatusInput,
    state: State<'_, AppState>,
) -> Result<WeixinOcLoginStatusResult, String> {
    weixin_oc_manager()
        .poll_login_status(state.inner(), input)
        .await
}

#[tauri::command]
async fn remote_im_weixin_oc_logout(
    input: WeixinOcLoginStatusInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    weixin_oc_manager()
        .logout(state.inner(), input.channel_id.as_str())
        .await?;
    Ok(true)
}

#[tauri::command]
async fn remote_im_weixin_oc_sync_contacts(
    input: WeixinOcLoginStatusInput,
    state: State<'_, AppState>,
) -> Result<WeixinOcSyncContactsResult, String> {
    let config = state_read_config_cached(state.inner())?;
    let channel = remote_im_channel_by_id(&config, &input.channel_id)
        .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
    if channel.platform != RemoteImPlatform::WeixinOc {
        return Err("该渠道不是个人微信渠道".to_string());
    }
    let creds = WeixinOcCredentials::from_value(&channel.credentials);
    if creds.account_id.trim().is_empty() || creds.token.trim().is_empty() {
        return Ok(WeixinOcSyncContactsResult {
            channel_id: input.channel_id,
            synced_count: 0,
            message: "当前还没有完成扫码登录，请先登录后再同步联系人。".to_string(),
        });
    }
    let user_id = creds.user_id.trim().to_string();
    let (_, created) = sync_weixin_oc_contact_from_user_id(state.inner(), channel, &user_id)?;
    Ok(WeixinOcSyncContactsResult {
        channel_id: input.channel_id,
        synced_count: 1,
        message: if created {
            format!("已同步个人微信联系人：{}", user_id)
        } else {
            format!("联系人已存在，无需重复同步：{}", user_id)
        },
    })
}
