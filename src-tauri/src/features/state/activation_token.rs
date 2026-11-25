// ==================== 会话激活令牌 ====================
//
// 外部脚本 / Webhook 用 `Authorization: Bearer <token>` 调
// `POST /api/v1/sessions/activate`，把一句话送进令牌绑定的那个会话。
//
// 令牌绑定单个会话：粒度到会话，写进脚本里也不必再分层保管，所以明文只在创建
// 这一次存在——回执直接交给模型，库里只留 SHA-256 哈希，没有「从数据库捞回明文」
// 这条路径。
//
// 令牌号走全局短 ID 序列（域 `activation_token`）：号只增不减，对外一律用短号指代。

/// 令牌短 ID 域。
const SHORT_ID_DOMAIN_ACTIVATION_TOKEN: &str = "activation_token";

/// 令牌明文前缀：便于在脚本与日志里一眼认出这是激活令牌，不是模型密钥。
const ACTIVATION_TOKEN_PREFIX: &str = "pai_act_";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActivationTokenRecord {
    /// 对外短号，也是 `activation_tokens` 的主键。
    token_id: u64,
    /// 绑定的会话主键。
    conversation_id: String,
    created_at: String,
}

/// 令牌明文的 SHA-256 十六进制摘要。库内只存它，校验时同法比对。
fn activation_token_hash_hex(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

/// 生成高熵明文：两枚 UUIDv4 的 128 位随机数拼成 64 个十六进制字符。
fn activation_token_new_secret() -> String {
    format!(
        "{}{}{}",
        ACTIVATION_TOKEN_PREFIX,
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

const ACTIVATION_TOKEN_SELECT_COLUMNS: &str = "token_id, conversation_id, created_at";

fn activation_token_row_to_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ActivationTokenRecord> {
    let token_id: i64 = row.get(0)?;
    Ok(ActivationTokenRecord {
        token_id: token_id.max(0) as u64,
        conversation_id: row.get(1)?,
        created_at: row.get(2)?,
    })
}

/// 新建令牌：返回落库记录与明文（明文只在这一次调用里存在）。
fn state_db_activation_token_create(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<(ActivationTokenRecord, String), String> {
    let conversation_id = conversation_id.trim();
    let secret = activation_token_new_secret();
    let token_hash = activation_token_hash_hex(&secret);
    let token_id = allocate_next_short_id(data_path, SHORT_ID_DOMAIN_ACTIVATION_TOKEN)?;
    let created_at = now_iso();
    let conn = state_db_open(data_path)?;
    conn.execute(
        "INSERT INTO activation_tokens(token_id, token_hash, conversation_id, created_at)
         VALUES(?1, ?2, ?3, ?4)",
        rusqlite::params![token_id as i64, token_hash, conversation_id, created_at],
    )
    .map_err(|err| format!("写入激活令牌失败，error={err}"))?;
    Ok((
        ActivationTokenRecord {
            token_id,
            conversation_id: conversation_id.to_string(),
            created_at,
        },
        secret,
    ))
}

/// 按明文校验：查不到就是不认。
fn state_db_activation_token_verify(
    data_path: &PathBuf,
    secret: &str,
) -> Result<ActivationTokenRecord, String> {
    let conn = state_db_open(data_path)?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {ACTIVATION_TOKEN_SELECT_COLUMNS} FROM activation_tokens WHERE token_hash=?1"
        ))
        .map_err(|err| format!("准备激活令牌查询失败，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![activation_token_hash_hex(secret.trim())])
        .map_err(|err| format!("查询激活令牌失败，error={err}"))?;
    match rows
        .next()
        .map_err(|err| format!("读取激活令牌行失败，error={err}"))?
    {
        Some(row) => activation_token_row_to_record(row)
            .map_err(|err| format!("解析激活令牌行失败，error={err}")),
        None => Err("激活令牌无效".to_string()),
    }
}

// ========== service 层入口 ==========

fn activation_token_create(
    state: &AppState,
    conversation_id: &str,
) -> Result<(ActivationTokenRecord, String), String> {
    state_db_activation_token_create(&state.data_path, conversation_id)
}

fn activation_token_verify(
    state: &AppState,
    secret: &str,
) -> Result<ActivationTokenRecord, String> {
    state_db_activation_token_verify(&state.data_path, secret)
}

// ========== 激活端点地址 ==========

/// Web Access 实际监听的端口。服务启动成功后写入，停机时清空。
/// 端口随配置变动，不能让调用方各自猜一个默认值。
static ACTIVATION_API_PORT: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(0);

fn activation_api_set_port(port: u16) {
    ACTIVATION_API_PORT.store(port, std::sync::atomic::Ordering::SeqCst);
}

fn activation_api_clear_port() {
    ACTIVATION_API_PORT.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// 激活端点的完整 URL；服务没在跑时返回 `None`，让回执直说哪一个开关没开。
fn activation_api_endpoint_url() -> Option<String> {
    let port = ACTIVATION_API_PORT.load(std::sync::atomic::Ordering::SeqCst);
    if port == 0 {
        return None;
    }
    Some(format!(
        "http://{}:{}/api/v1/sessions/activate",
        IDE_CONTEXT_BRIDGE_HOST, port
    ))
}
