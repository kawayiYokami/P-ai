// ==================== QQ 官方机器人：扫码绑定 ====================
//
// 协议来自官方 connector（纯 HTTP，不使用 QQ 连接层）：
//   ① POST /lite/create_bind_task {key}                 → data.task_id
//   ② 二维码内容 = https://q.qq.com/qqbot/openclaw/connect.html?task_id=<id>&source=&_wv=2
//   ③ POST /lite/poll_bind_result {task_id}             → data.status(0/1/2/3) + bot_appid + bot_encrypt_secret
//   ④ status=2 时用 ① 的 32 字节 key 做 AES-256-GCM 解密
//      （base64 解后：前 12 字节 IV + 中间密文 + 后 16 字节 GCM tag）得到 appSecret
//
// 这里只负责「拿到 appId / appSecret 两个字符串」，**不查配置、不校验渠道、不写配置、不重连**：
// 新建的渠道还没保存过，配置里根本没有它，扫码不该先要求「渠道已落盘」。
// 凭据由前端回填表单，用户保存渠道时走既有落盘流程，启用连接也走既有启用路径。
//
// key 只在内存里持有、不落盘；轮询由前端驱动（每 2s 一次），后端不留常驻任务。

const QQ_OFFICIAL_BIND_CREATE_TASK_URL: &str = "https://q.qq.com/lite/create_bind_task";
const QQ_OFFICIAL_BIND_POLL_RESULT_URL: &str = "https://q.qq.com/lite/poll_bind_result";
const QQ_OFFICIAL_BIND_QR_BASE_URL: &str = "https://q.qq.com/qqbot/openclaw/connect.html";
/// 二维码展示有效期（秒）。真实过期以平台返回的 status=3 为准，这里只给前端倒计时。
const QQ_OFFICIAL_BIND_EXPIRE_SECONDS: i64 = 180;
/// 轮询返回的状态位（对齐官方 connector 的 BindStatus）。
/// 只有 COMPLETED（2）是成功、EXPIRED（3）是过期；
/// NONE（0）与 PENDING（1）都是“还没完成、继续等”，官方实现里不区分，
/// 所以这里也不设“已扫码”中间态。
const QQ_OFFICIAL_BIND_STATUS_COMPLETED: i64 = 2;
const QQ_OFFICIAL_BIND_STATUS_EXPIRED: i64 = 3;
/// AES-256-GCM 的 IV 与 tag 固定长度。
const QQ_OFFICIAL_BIND_GCM_IV_BYTES: usize = 12;
const QQ_OFFICIAL_BIND_GCM_TAG_BYTES: usize = 16;

/// 一次扫码任务的会话态：只有解密用的 key，与渠道无关。
#[derive(Debug, Clone)]
struct QqOfficialBindSession {
    key: [u8; 32],
}

fn qq_official_bind_sessions(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, QqOfficialBindSession>> {
    static SESSIONS: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, QqOfficialBindSession>>,
    > = OnceLock::new();
    SESSIONS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QqOfficialBindStartInput {}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QqOfficialBindPollInput {
    task_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QqOfficialBindCancelInput {
    task_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QqOfficialBindStartResult {
    task_id: String,
    qr_svg: String,
    /// 与 `qr_svg` 同源的 data URL，供前端用 `<img>` 直接显示（与微信扫码登录同一渲染方式）。
    qr_image: String,
    expire_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QqOfficialBindPollResult {
    /// `waiting`（平台 0/1 还没完成，继续等）/ `confirmed`（平台 2）/ `expired`（平台 3）。
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_id: Option<String>,
    /// 只在 `confirmed` 时返回：前端拿它回填表单。
    #[serde(skip_serializing_if = "Option::is_none")]
    app_secret: Option<String>,
}

/// 32 字节随机 key：用两个 v4 UUID 拼出来（uuid 走系统 CSPRNG），不额外引入随机数依赖。
fn qq_official_bind_random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    key
}

fn qq_official_bind_qr_payload(task_id: &str) -> String {
    format!("{QQ_OFFICIAL_BIND_QR_BASE_URL}?task_id={task_id}&source=&_wv=2")
}

/// 二维码直接产出 SVG 字符串，不落文件。
fn qq_official_bind_qr_svg(content: &str) -> Result<String, String> {
    use qrcode::render::svg;
    let code =
        qrcode::QrCode::new(content.as_bytes()).map_err(|err| format!("生成二维码失败: {err}"))?;
    let image = code
        .render::<svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .build();
    // qrcode crate 会在 SVG 前加 `<?xml version="1.0" standalone="yes"?>` 声明；
    // 前端只接受以 `<svg` 开头的字符串并内联渲染，这里剥掉声明，返回可直接嵌入的 `<svg>`。
    let svg = match image.find("<svg") {
        Some(index) => image[index..].to_string(),
        None => image,
    };
    Ok(svg)
}

/// AES-256-GCM 解密：入参是「12B IV + 密文 + 16B tag」的 base64。
fn qq_official_bind_decrypt_secret(key: &[u8; 32], encrypted_base64: &str) -> Result<String, String> {
    use aes_gcm::aead::Aead as _;
    use aes_gcm::KeyInit as _;

    let raw = B64
        .decode(encrypted_base64.trim())
        .map_err(|err| format!("解密凭据失败：base64 解码失败, error={err}"))?;
    if raw.len() <= QQ_OFFICIAL_BIND_GCM_IV_BYTES + QQ_OFFICIAL_BIND_GCM_TAG_BYTES {
        return Err(format!("解密凭据失败：密文长度不足, len={}", raw.len()));
    }
    let (iv, ciphertext) = raw.split_at(QQ_OFFICIAL_BIND_GCM_IV_BYTES);
    let nonce = aes_gcm::Nonce::try_from(iv)
        .map_err(|_| "解密凭据失败：IV 长度不是 12 字节".to_string())?;
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(key)
        .map_err(|err| format!("解密凭据失败：初始化 AES-256-GCM 失败, error={err}"))?;
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "解密凭据失败：GCM 校验未通过（key 不匹配或密文被截断）".to_string())?;
    String::from_utf8(plaintext).map_err(|err| format!("解密凭据失败：明文不是合法 UTF-8, error={err}"))
}

fn qq_official_bind_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(
            QQ_OFFICIAL_DEFAULT_HTTP_TIMEOUT_MS,
        ))
        .build()
        .map_err(|err| format!("构建扫码绑定 HTTP 客户端失败: {err}"))
}

async fn qq_official_bind_start_inner() -> Result<QqOfficialBindStartResult, String> {
    let key = qq_official_bind_random_key();
    let client = qq_official_bind_http_client()?;
    let response = client
        .post(QQ_OFFICIAL_BIND_CREATE_TASK_URL)
        .json(&serde_json::json!({ "key": B64.encode(key) }))
        .send()
        .await
        .map_err(|err| format!("创建扫码绑定任务失败: {err}"))?;
    let status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("解析扫码绑定任务响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!("创建扫码绑定任务失败: HTTP {status}, body={body}"));
    }
    if body.get("retcode").and_then(qq_official_json_i64).unwrap_or(-1) != 0 {
        return Err(format!("创建扫码绑定任务失败: body={body}"));
    }
    let task_id = body
        .get("data")
        .and_then(|data| data.get("task_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("扫码绑定任务响应缺少 data.task_id: {body}"))?
        .to_string();

    let qr_svg = qq_official_bind_qr_svg(&qq_official_bind_qr_payload(&task_id))?;
    let qr_image = format!(
        "data:image/svg+xml;base64,{}",
        B64.encode(qr_svg.as_bytes())
    );
    if let Ok(mut sessions) = qq_official_bind_sessions().lock() {
        sessions.insert(task_id.clone(), QqOfficialBindSession { key });
    }
    runtime_log_info(format!(
        "[远程IM] 扫码绑定任务已创建 任务={task_id}"
    ));
    Ok(QqOfficialBindStartResult {
        task_id,
        qr_svg,
        qr_image,
        expire_seconds: QQ_OFFICIAL_BIND_EXPIRE_SECONDS,
    })
}

fn qq_official_bind_session(task_id: &str) -> Option<QqOfficialBindSession> {
    let sessions = qq_official_bind_sessions().lock().ok()?;
    sessions.get(task_id).cloned()
}

fn qq_official_bind_remove_session(task_id: &str) {
    if let Ok(mut sessions) = qq_official_bind_sessions().lock() {
        sessions.remove(task_id);
    }
}

async fn qq_official_bind_poll_inner(task_id: &str) -> Result<QqOfficialBindPollResult, String> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return Err("taskId 为必填项。".to_string());
    }
    let session = qq_official_bind_session(task_id).ok_or_else(|| {
        "扫码任务不存在或已结束，请重新发起扫码".to_string()
    })?;

    let client = qq_official_bind_http_client()?;
    let response = client
        .post(QQ_OFFICIAL_BIND_POLL_RESULT_URL)
        .json(&serde_json::json!({ "task_id": task_id }))
        .send()
        .await
        .map_err(|err| format!("轮询扫码绑定结果失败: {err}"))?;
    let http_status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("解析扫码绑定结果响应失败: {err}"))?;
    if !http_status.is_success() {
        return Err(format!(
            "轮询扫码绑定结果失败: HTTP {http_status}, body={body}"
        ));
    }
    if body.get("retcode").and_then(qq_official_json_i64).unwrap_or(-1) != 0 {
        return Err(format!("轮询扫码绑定结果失败: body={body}"));
    }
    let data = body.get("data").cloned().unwrap_or(Value::Null);
    let bind_status = data
        .get("status")
        .and_then(qq_official_json_i64)
        .unwrap_or_default();

    if bind_status == QQ_OFFICIAL_BIND_STATUS_EXPIRED {
        qq_official_bind_remove_session(task_id);
        runtime_log_info(format!(
            "[远程IM] 扫码绑定任务已过期 任务={task_id}"
        ));
        return Ok(QqOfficialBindPollResult {
            status: "expired".to_string(),
            app_id: None,
            app_secret: None,
        });
    }
    if bind_status != QQ_OFFICIAL_BIND_STATUS_COMPLETED {
        // 对齐官方 connector：0（NONE）与 1（PENDING）都是“还没完成、继续等”，
        // 不设“已扫码”中间态；只有 2 是成功、3 是过期。
        return Ok(QqOfficialBindPollResult {
            status: "waiting".to_string(),
            app_id: None,
            app_secret: None,
        });
    }

    let app_id = data
        .get("bot_appid")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("扫码绑定结果缺少 bot_appid: {body}"))?
        .to_string();
    let encrypted_secret = data
        .get("bot_encrypt_secret")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("扫码绑定结果缺少 bot_encrypt_secret: {body}"))?;
    let app_secret = qq_official_bind_decrypt_secret(&session.key, encrypted_secret)?;

    // 一次扫码只发一次凭据：拿到就清掉会话，避免重复 poll 反复取同一把 secret。
    qq_official_bind_remove_session(task_id);
    runtime_log_info(format!(
        "[远程IM] 扫码绑定完成 任务={task_id} 应用={app_id}"
    ));

    Ok(QqOfficialBindPollResult {
        status: "confirmed".to_string(),
        app_id: Some(app_id),
        app_secret: Some(app_secret),
    })
}

#[tauri::command]
async fn qq_official_bind_start(
    input: QqOfficialBindStartInput,
) -> Result<QqOfficialBindStartResult, String> {
    let _ = input;
    qq_official_bind_start_inner().await
}

#[tauri::command]
async fn qq_official_bind_poll(
    input: QqOfficialBindPollInput,
) -> Result<QqOfficialBindPollResult, String> {
    qq_official_bind_poll_inner(&input.task_id).await
}

fn qq_official_bind_cancel_inner(task_id: &str) -> Result<(), String> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return Err("taskId 为必填项。".to_string());
    }
    qq_official_bind_remove_session(task_id);
    Ok(())
}

#[tauri::command]
async fn qq_official_bind_cancel(input: QqOfficialBindCancelInput) -> Result<(), String> {
    qq_official_bind_cancel_inner(&input.task_id)
}

#[cfg(test)]
mod qq_official_bind_tests {
    use super::*;

    #[test]
    fn bind_decrypt_should_recover_secret_from_iv_ciphertext_tag() {
        use aes_gcm::aead::Aead as _;
        use aes_gcm::KeyInit as _;

        let key = [7u8; 32];
        let iv = [3u8; 12];
        let secret = "test-app-secret-0123456789";
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).expect("init cipher");
        let mut payload = iv.to_vec();
        payload.extend(
            cipher
                .encrypt(aes_gcm::Nonce::from_slice(&iv), secret.as_bytes())
                .expect("encrypt"),
        );

        let decrypted =
            qq_official_bind_decrypt_secret(&key, &B64.encode(&payload)).expect("解密应当成功");
        assert_eq!(decrypted, secret);

        // key 不匹配时必须报错而不是返回乱码
        let wrong_key = [9u8; 32];
        assert!(qq_official_bind_decrypt_secret(&wrong_key, &B64.encode(&payload)).is_err());
        // 密文被截断也必须报错
        assert!(qq_official_bind_decrypt_secret(&key, &B64.encode(&payload[..16])).is_err());
    }

    #[test]
    fn bind_qr_payload_should_carry_task_id_in_official_shape() {
        let payload = qq_official_bind_qr_payload("TASK123");
        assert_eq!(
            payload,
            "https://q.qq.com/qqbot/openclaw/connect.html?task_id=TASK123&source=&_wv=2"
        );
        let svg = qq_official_bind_qr_svg(&payload).expect("生成二维码");
        // 前端按「以 <svg 开头」做形状校验后内联渲染；带 XML 声明会被判为不安全而整块不显示。
        assert!(
            svg.starts_with("<svg"),
            "二维码必须以 <svg 开头（不能带 XML 声明）: {svg}"
        );
        assert!(!svg.contains("<?xml"), "二维码不应包含 XML 声明: {svg}");
        assert!(svg.contains("</svg>"), "二维码 SVG 必须闭合: {svg}");
    }

    #[test]
    fn bind_session_should_be_keyed_only_by_task_id() {
        // 会话只按 taskId 存，与渠道无关；清掉之后 poll 会拿到可读错误。
        let task_id = format!("task-{}", Uuid::new_v4());
        qq_official_bind_sessions()
            .lock()
            .expect("sessions")
            .insert(task_id.clone(), QqOfficialBindSession { key: [1u8; 32] });
        assert!(qq_official_bind_session(&task_id).is_some());
        qq_official_bind_cancel_inner(&task_id).expect("cancel");
        assert!(qq_official_bind_session(&task_id).is_none());
        assert!(qq_official_bind_cancel_inner("  ").is_err());
    }
}
