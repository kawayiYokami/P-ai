const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_OAUTH_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const CODEX_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_OAUTH_CALLBACK_PORT: u16 = 1455;
const CODEX_OAUTH_CALLBACK_PATH: &str = "/auth/callback";
const CODEX_OAUTH_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const CODEX_OAUTH_SCOPE: &str = "openid profile email offline_access";
const CODEX_OAUTH_ORIGINATOR: &str = "cline";
const CODEX_REFRESH_LEAD_MS: i64 = 5 * 24 * 60 * 60 * 1000;
const CODEX_OAUTH_HTML_SUCCESS: &str = r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Codex 登录成功</title></head><body style="font-family:Segoe UI,Arial,sans-serif;padding:32px;"><h2>Codex 登录成功</h2><p>可以关闭这个窗口，返回应用继续使用。</p><script>setTimeout(()=>window.close(),2000)</script></body></html>"#;
const CODEX_OAUTH_HTML_ERROR: &str = r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Codex 登录失败</title></head><body style="font-family:Segoe UI,Arial,sans-serif;padding:32px;"><h2>Codex 登录失败</h2><p>请回到应用查看错误信息后重试。</p></body></html>"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexStoredCredential {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    account_id: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    expires_at_ms: i64,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct CodexJwtApiClaims {
    #[serde(default)]
    chatgpt_account_id: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct CodexJwtOrganization {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct CodexJwtClaims {
    #[serde(default)]
    exp: i64,
    #[serde(default)]
    email: String,
    #[serde(default)]
    chatgpt_account_id: String,
    #[serde(default)]
    organizations: Vec<CodexJwtOrganization>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    api_auth: CodexJwtApiClaims,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexLocalAuthTokens {
    #[serde(default, alias = "accessToken")]
    access_token: String,
    #[serde(default, alias = "refreshToken")]
    refresh_token: String,
    #[serde(default, alias = "idToken")]
    id_token: String,
    #[serde(default, alias = "accountId")]
    account_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexLocalAuthFile {
    #[serde(default)]
    tokens: Option<CodexLocalAuthTokens>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexOAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    id_token: String,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAuthStatusInput {
    provider_id: String,
    #[serde(default = "default_codex_auth_mode")]
    auth_mode: String,
    #[serde(default = "default_codex_local_auth_path")]
    local_auth_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexStartOAuthLoginInput {
    provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexLogoutInput {
    provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAuthStatus {
    provider_id: String,
    auth_mode: String,
    authenticated: bool,
    status: String,
    message: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    account_id: String,
    #[serde(default)]
    access_token_preview: String,
    #[serde(default)]
    local_auth_path: String,
    #[serde(default)]
    managed_auth_path: String,
    #[serde(default)]
    expires_at: String,
}

#[derive(Debug, Clone)]
struct CodexOAuthSession {
    status: String,
    message: String,
    error: String,
    email: String,
    account_id: String,
}

fn codex_oauth_sessions() -> &'static Mutex<std::collections::HashMap<String, CodexOAuthSession>> {
    static SESSIONS: OnceLock<Mutex<std::collections::HashMap<String, CodexOAuthSession>>> =
        OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn codex_refresh_locks(
) -> &'static Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>> {
    static LOCKS: OnceLock<
        Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>,
    > = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn codex_runtime_auth_cache() -> &'static Mutex<std::collections::HashMap<String, CodexStoredCredential>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, CodexStoredCredential>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn codex_runtime_auth_cache_key(provider_id: &str, auth_mode: &str, local_auth_path: &str) -> String {
    format!(
        "{}\n{}\n{}",
        provider_id.trim(),
        normalize_codex_auth_mode(auth_mode),
        normalize_terminal_path_input_for_current_platform(local_auth_path)
    )
}

fn codex_refresh_lock_for_provider(provider_id: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let key = provider_id.trim().to_string();
    let mut guard = codex_refresh_locks().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(key)
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

fn codex_runtime_auth_cache_get(
    provider_id: &str,
    auth_mode: &str,
    local_auth_path: &str,
) -> Option<CodexStoredCredential> {
    let key = codex_runtime_auth_cache_key(provider_id, auth_mode, local_auth_path);
    codex_runtime_auth_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&key).cloned())
}

fn codex_runtime_auth_cache_set(
    provider_id: &str,
    auth_mode: &str,
    local_auth_path: &str,
    credential: &CodexStoredCredential,
) {
    let key = codex_runtime_auth_cache_key(provider_id, auth_mode, local_auth_path);
    if let Ok(mut guard) = codex_runtime_auth_cache().lock() {
        guard.insert(key, credential.clone());
    }
}

fn codex_pick_newer_credential(
    current: CodexStoredCredential,
    candidate: CodexStoredCredential,
) -> CodexStoredCredential {
    if candidate.expires_at_ms > current.expires_at_ms {
        return candidate;
    }
    if candidate.expires_at_ms == current.expires_at_ms && candidate.updated_at > current.updated_at {
        return candidate;
    }
    current
}

fn codex_session_get(provider_id: &str) -> Option<CodexOAuthSession> {
    codex_oauth_sessions()
        .lock()
        .ok()
        .and_then(|guard| guard.get(provider_id.trim()).cloned())
}

fn codex_session_set(provider_id: &str, session: CodexOAuthSession) {
    if let Ok(mut guard) = codex_oauth_sessions().lock() {
        guard.insert(provider_id.trim().to_string(), session);
    }
}

fn codex_session_remove(provider_id: &str) {
    if let Ok(mut guard) = codex_oauth_sessions().lock() {
        guard.remove(provider_id.trim());
    }
}

fn codex_auth_storage_root() -> Result<PathBuf, String> {
    if let Some(portable_root) = detect_portable_runtime_root() {
        return Ok(portable_root.join("auth").join("codex"));
    }
    let (config_dir, _legacy_dir) = resolve_standard_config_dir()?;
    Ok(app_root_from_data_path(&config_dir.join("app_data.json"))
        .join("auth")
        .join("codex"))
}

fn managed_codex_auth_path(provider_id: &str) -> Result<PathBuf, String> {
    let safe = provider_id
        .trim()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') { ch } else { '_' })
        .collect::<String>();
    Ok(codex_auth_storage_root()?.join(format!("{safe}.json")))
}

fn codex_parse_jwt_claims(token: &str) -> Option<CodexJwtClaims> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let mut parts = token.trim().split('.');
    let _ = parts.next()?;
    let payload = parts.next()?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice::<CodexJwtClaims>(&decoded).ok()
}

fn codex_extract_account_id(claims: &CodexJwtClaims) -> String {
    if !claims.chatgpt_account_id.trim().is_empty() {
        return claims.chatgpt_account_id.trim().to_string();
    }
    if !claims.api_auth.chatgpt_account_id.trim().is_empty() {
        return claims.api_auth.chatgpt_account_id.trim().to_string();
    }
    claims
        .organizations
        .iter()
        .find_map(|item| (!item.id.trim().is_empty()).then(|| item.id.trim().to_string()))
        .unwrap_or_default()
}

fn codex_token_preview(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.len() <= 12 {
        return trimmed.to_string();
    }
    format!("{}...{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
}

fn codex_pkce_code_verifier() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let mut seed = [0u8; 32];
    seed[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    seed[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    URL_SAFE_NO_PAD.encode(seed)
}

fn codex_pkce_code_challenge(verifier: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use sha2::{Digest, Sha256};
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn codex_build_authorize_url(code_challenge: &str, state: &str) -> String {
    let params = [
        ("client_id", CODEX_OAUTH_CLIENT_ID),
        ("redirect_uri", CODEX_OAUTH_REDIRECT_URI),
        ("scope", CODEX_OAUTH_SCOPE),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("response_type", "code"),
        ("state", state),
        ("codex_cli_simplified_flow", "true"),
        ("originator", CODEX_OAUTH_ORIGINATOR),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", urlencoding::encode(key), urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{CODEX_OAUTH_AUTHORIZE_URL}?{query}")
}

fn codex_expiry_iso(expires_at_ms: i64) -> String {
    if expires_at_ms <= 0 {
        return String::new();
    }
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(expires_at_ms) * 1_000_000)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn codex_is_token_expired(expires_at_ms: i64) -> bool {
    if expires_at_ms <= 0 {
        return false;
    }
    let now_ms = now_utc().unix_timestamp_nanos() / 1_000_000;
    now_ms >= i128::from(expires_at_ms.saturating_sub(CODEX_REFRESH_LEAD_MS))
}

fn codex_credential_from_token_response(
    response: CodexOAuthTokenResponse,
    fallback: Option<&CodexStoredCredential>,
) -> Result<CodexStoredCredential, String> {
    if response.access_token.trim().is_empty() {
        return Err("Codex OAuth 响应缺少 access_token".to_string());
    }
    let access_claims = codex_parse_jwt_claims(&response.access_token).unwrap_or_default();
    let id_claims = if response.id_token.trim().is_empty() {
        CodexJwtClaims::default()
    } else {
        codex_parse_jwt_claims(&response.id_token).unwrap_or_default()
    };
    let mut account_id = codex_extract_account_id(&id_claims);
    if account_id.is_empty() {
        account_id = codex_extract_account_id(&access_claims);
    }
    if account_id.is_empty() {
        if let Some(fallback) = fallback {
            account_id = fallback.account_id.trim().to_string();
        }
    }
    let mut email = response.email.trim().to_string();
    if email.is_empty() {
        email = id_claims.email.trim().to_string();
    }
    if email.is_empty() {
        email = access_claims.email.trim().to_string();
    }
    if email.is_empty() {
        if let Some(fallback) = fallback {
            email = fallback.email.trim().to_string();
        }
    }
    let expires_at_ms = if response.expires_in > 0 {
        let now_ms = now_utc().unix_timestamp_nanos() / 1_000_000;
        (now_ms + i128::from(response.expires_in) * 1000)
            .min(i128::from(i64::MAX)) as i64
    } else {
        access_claims.exp.saturating_mul(1000)
    };
    let refresh_token = if response.refresh_token.trim().is_empty() {
        fallback
            .map(|item| item.refresh_token.trim().to_string())
            .unwrap_or_default()
    } else {
        response.refresh_token.trim().to_string()
    };
    Ok(CodexStoredCredential {
        access_token: response.access_token.trim().to_string(),
        refresh_token,
        account_id,
        email,
        expires_at_ms,
        updated_at: now_iso(),
    })
}

async fn codex_exchange_code_for_tokens(
    code: &str,
    code_verifier: &str,
) -> Result<CodexStoredCredential, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| format!("构建 Codex OAuth 客户端失败: {err}"))?;
    let response = client
        .post(CODEX_OAUTH_TOKEN_URL)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CODEX_OAUTH_CLIENT_ID),
            ("code", code),
            ("redirect_uri", CODEX_OAUTH_REDIRECT_URI),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|err| format!("Codex OAuth 换取 token 失败: {err}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Codex OAuth 换取 token 失败: {status} | {body}"));
    }
    let payload = response
        .json::<CodexOAuthTokenResponse>()
        .await
        .map_err(|err| format!("解析 Codex OAuth token 响应失败: {err}"))?;
    codex_credential_from_token_response(payload, None)
}

async fn codex_refresh_credential(
    credential: &CodexStoredCredential,
) -> Result<CodexStoredCredential, String> {
    if credential.refresh_token.trim().is_empty() {
        return Err("Codex 凭证缺少 refresh_token，无法刷新".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| format!("构建 Codex OAuth 客户端失败: {err}"))?;
    let response = client
        .post(CODEX_OAUTH_TOKEN_URL)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CODEX_OAUTH_CLIENT_ID),
            ("refresh_token", credential.refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(|err| format!("刷新 Codex token 失败: {err}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("刷新 Codex token 失败: {status} | {body}"));
    }
    let payload = response
        .json::<CodexOAuthTokenResponse>()
        .await
        .map_err(|err| format!("解析刷新后的 Codex token 响应失败: {err}"))?;
    codex_credential_from_token_response(payload, Some(credential))
}

fn codex_parse_local_auth_file(path: &str) -> Result<CodexStoredCredential, String> {
    let normalized = normalize_terminal_path_input_for_current_platform(path);
    if normalized.trim().is_empty() {
        return Err("本地 Codex 凭证路径为空".to_string());
    }
    let content = fs::read_to_string(&normalized)
        .map_err(|err| format!("读取本地 Codex 凭证失败 ({}): {err}", normalized))?;
    let payload = serde_json::from_str::<CodexLocalAuthFile>(&content)
        .map_err(|err| format!("解析本地 Codex 凭证失败 ({}): {err}", normalized))?;
    let tokens = payload
        .tokens
        .ok_or_else(|| "本地 Codex 凭证缺少 tokens 字段".to_string())?;
    if tokens.access_token.trim().is_empty() {
        return Err("本地 Codex 凭证缺少 access_token".to_string());
    }
    let access_claims = codex_parse_jwt_claims(&tokens.access_token).unwrap_or_default();
    let id_claims = if tokens.id_token.trim().is_empty() {
        CodexJwtClaims::default()
    } else {
        codex_parse_jwt_claims(&tokens.id_token).unwrap_or_default()
    };
    let mut account_id = tokens.account_id.trim().to_string();
    if account_id.is_empty() {
        account_id = codex_extract_account_id(&id_claims);
    }
    if account_id.is_empty() {
        account_id = codex_extract_account_id(&access_claims);
    }
    let mut email = id_claims.email.trim().to_string();
    if email.is_empty() {
        email = access_claims.email.trim().to_string();
    }
    Ok(CodexStoredCredential {
        access_token: tokens.access_token.trim().to_string(),
        refresh_token: tokens.refresh_token.trim().to_string(),
        account_id,
        email,
        expires_at_ms: access_claims.exp.saturating_mul(1000),
        updated_at: now_iso(),
    })
}

fn read_managed_codex_auth(provider_id: &str) -> Result<CodexStoredCredential, String> {
    let path = managed_codex_auth_path(provider_id)?;
    let body = fs::read_to_string(&path)
        .map_err(|err| format!("读取托管 Codex 凭证失败 ({}): {err}", path.display()))?;
    serde_json::from_str::<CodexStoredCredential>(&body)
        .map_err(|err| format!("解析托管 Codex 凭证失败 ({}): {err}", path.display()))
}

fn write_managed_codex_auth(
    provider_id: &str,
    credential: &CodexStoredCredential,
) -> Result<(), String> {
    let path = managed_codex_auth_path(provider_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建 Codex 托管凭证目录失败 ({}): {err}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(credential)
        .map_err(|err| format!("序列化 Codex 托管凭证失败: {err}"))?;
    fs::write(&path, body)
        .map_err(|err| format!("写入 Codex 托管凭证失败 ({}): {err}", path.display()))
}

fn delete_managed_codex_auth(provider_id: &str) -> Result<(), String> {
    let path = managed_codex_auth_path(provider_id)?;
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path)
        .map_err(|err| format!("删除 Codex 托管凭证失败 ({}): {err}", path.display()))
}

fn build_codex_runtime_auth(
    provider_id: &str,
    auth_mode: &str,
    local_auth_path: &str,
    credential: CodexStoredCredential,
) -> CodexRuntimeAuth {
    CodexRuntimeAuth {
        provider_id: provider_id.trim().to_string(),
        auth_mode: normalize_codex_auth_mode(auth_mode),
        local_auth_path: normalize_terminal_path_input_for_current_platform(local_auth_path),
        access_token: credential.access_token.clone(),
        refresh_token: (!credential.refresh_token.trim().is_empty())
            .then(|| credential.refresh_token.clone()),
        account_id: (!credential.account_id.trim().is_empty()).then(|| credential.account_id.clone()),
        email: (!credential.email.trim().is_empty()).then(|| credential.email.clone()),
        expires_at_ms: (credential.expires_at_ms > 0).then_some(credential.expires_at_ms),
    }
}

fn read_codex_runtime_auth_snapshot(
    provider_id: &str,
    auth_mode: &str,
    local_auth_path: &str,
) -> Result<CodexRuntimeAuth, String> {
    let normalized_mode = normalize_codex_auth_mode(auth_mode);
    let mut credential = if normalized_mode == CODEX_AUTH_MODE_MANAGED_OAUTH {
        read_managed_codex_auth(provider_id)?
    } else {
        codex_parse_local_auth_file(local_auth_path)?
    };
    if let Some(cached) = codex_runtime_auth_cache_get(provider_id, &normalized_mode, local_auth_path) {
        credential = codex_pick_newer_credential(credential, cached);
    }
    Ok(build_codex_runtime_auth(
        provider_id,
        &normalized_mode,
        local_auth_path,
        credential,
    ))
}

async fn ensure_codex_runtime_auth_fresh(
    auth: &CodexRuntimeAuth,
) -> Result<CodexRuntimeAuth, String> {
    let expires_at_ms = auth.expires_at_ms.unwrap_or_default();
    if !codex_is_token_expired(expires_at_ms) {
        return Ok(auth.clone());
    }
    codex_refresh_runtime_auth_with_trigger(auth, "token_expired").await
}

async fn codex_refresh_runtime_auth_with_trigger(
    auth: &CodexRuntimeAuth,
    trigger: &str,
) -> Result<CodexRuntimeAuth, String> {
    let lock = codex_refresh_lock_for_provider(&auth.provider_id);
    let _guard = lock.lock().await;

    let latest_auth = read_codex_runtime_auth_snapshot(
        &auth.provider_id,
        &auth.auth_mode,
        &auth.local_auth_path,
    )?;
    let expires_at_ms = latest_auth.expires_at_ms.unwrap_or_default();
    if !codex_is_token_expired(expires_at_ms) {
        return Ok(latest_auth);
    }
    let Some(refresh_token) = latest_auth
        .refresh_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(latest_auth);
    };
    let refresh_started = std::time::Instant::now();
    let refreshed = codex_refresh_credential(&CodexStoredCredential {
        access_token: latest_auth.access_token.clone(),
        refresh_token: refresh_token.to_string(),
        account_id: latest_auth.account_id.clone().unwrap_or_default(),
        email: latest_auth.email.clone().unwrap_or_default(),
        expires_at_ms,
        updated_at: now_iso(),
    })
    .await
    .map_err(|err| {
        codex_auth_log_info(
            "token_refresh",
            trigger,
            &latest_auth.provider_id,
            "error",
            refresh_started.elapsed().as_millis(),
            &[
                ("message", err.clone()),
                ("token_count", "1".to_string()),
            ],
        );
        err
    })?;
    if latest_auth.auth_mode == CODEX_AUTH_MODE_MANAGED_OAUTH {
        write_managed_codex_auth(&latest_auth.provider_id, &refreshed)?;
    }
    codex_runtime_auth_cache_set(
        &latest_auth.provider_id,
        &latest_auth.auth_mode,
        &latest_auth.local_auth_path,
        &refreshed,
    );
    codex_auth_log_info(
        "token_refresh",
        trigger,
        &latest_auth.provider_id,
        "authenticated",
        refresh_started.elapsed().as_millis(),
        &[
            (
                "token_count",
                (1 + usize::from(!refreshed.refresh_token.trim().is_empty())).to_string(),
            ),
            ("email", refreshed.email.clone()),
        ],
    );
    Ok(build_codex_runtime_auth(
        &latest_auth.provider_id,
        &latest_auth.auth_mode,
        &latest_auth.local_auth_path,
        refreshed,
    ))
}

fn codex_status_from_runtime_auth(auth: &CodexRuntimeAuth) -> CodexAuthStatus {
    let managed_auth_path = managed_codex_auth_path(&auth.provider_id)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let expired = auth
        .expires_at_ms
        .map(codex_is_token_expired)
        .unwrap_or(false);
    CodexAuthStatus {
        provider_id: auth.provider_id.clone(),
        auth_mode: auth.auth_mode.clone(),
        authenticated: !auth.access_token.trim().is_empty() && !expired,
        status: if expired {
            "expired".to_string()
        } else {
            "authenticated".to_string()
        },
        message: if expired {
            "Codex 凭证已过期，运行时会尝试刷新。".to_string()
        } else {
            "Codex 凭证可用。".to_string()
        },
        email: auth.email.clone().unwrap_or_default(),
        account_id: auth.account_id.clone().unwrap_or_default(),
        access_token_preview: codex_token_preview(&auth.access_token),
        local_auth_path: auth.local_auth_path.clone(),
        managed_auth_path,
        expires_at: auth.expires_at_ms.map(codex_expiry_iso).unwrap_or_default(),
    }
}

fn codex_write_callback_html(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    let response = format!(
        "{status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = std::io::Write::write_all(stream, response.as_bytes());
    let _ = std::io::Write::flush(stream);
}

fn codex_auth_log_info(
    task: &str,
    trigger: &str,
    provider_id: &str,
    status: &str,
    duration_ms: u128,
    extra_fields: &[(&str, String)],
) {
    let extras = extra_fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ");
    let suffix = if extras.is_empty() {
        String::new()
    } else {
        format!(" {extras}")
    };
    runtime_log_info(format!(
        "[Codex认证] task={} trigger={} provider_id={} status={} duration_ms={} timestamp={}{}",
        task,
        trigger,
        provider_id.trim(),
        status,
        duration_ms,
        now_iso(),
        suffix
    ));
}

// ========== OAuth 回调辅助 ==========

fn handle_oauth_callback_request(
    request: &str,
    expected_state: &str,
) -> Result<(String, reqwest::Url), (&'static str, String)> {
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or_default()
        .to_string();
    let url = reqwest::Url::parse(&format!(
        "http://localhost:{}{}",
        CODEX_OAUTH_CALLBACK_PORT, target
    ))
    .map_err(|_| ("HTTP/1.1 400 Bad Request", "Codex OAuth 回调格式无效。".to_string()))?;
    if url.path() != CODEX_OAUTH_CALLBACK_PATH {
        return Err(("HTTP/1.1 404 Not Found", "Codex OAuth 回调路径无效。".to_string()));
    }
    let code = url
        .query_pairs()
        .find_map(|(key, value)| (key == "code").then(|| value.to_string()))
        .unwrap_or_default();
    let callback_state = url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.to_string()))
        .unwrap_or_default();
    let oauth_error = url
        .query_pairs()
        .find_map(|(key, value)| (key == "error").then(|| value.to_string()))
        .unwrap_or_default();
    if !oauth_error.trim().is_empty() {
        return Err((
            "HTTP/1.1 400 Bad Request",
            format!("Codex OAuth 登录失败: {oauth_error}"),
        ));
    }
    if code.trim().is_empty() || callback_state.trim() != expected_state.trim() {
        return Err((
            "HTTP/1.1 400 Bad Request",
            "Codex OAuth 回调校验失败，请重新登录。".to_string(),
        ));
    }
    Ok((code, url))
}

fn process_oauth_code_exchange(
    provider_id: &str,
    code: &str,
    code_verifier: &str,
) -> Result<CodexStoredCredential, String> {
    let credential =
        tauri::async_runtime::block_on(codex_exchange_code_for_tokens(code, code_verifier))?;
    write_managed_codex_auth(provider_id, &credential)?;
    Ok(credential)
}

fn send_callback_response(
    provider_id: &str,
    stream: &mut std::net::TcpStream,
    status_line: &str,
    html: &str,
    session: Option<CodexOAuthSession>,
) {
    codex_write_callback_html(stream, status_line, html);
    if let Some(session) = session {
        codex_session_set(provider_id, session);
    }
}

fn handle_oauth_callback_connection(
    provider_id: &str,
    code_verifier: &str,
    state: &str,
    started: &std::time::Instant,
    stream: &mut std::net::TcpStream,
) -> bool {
    let mut buffer = [0u8; 4096];
    let read = std::io::Read::read(stream, &mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
    let code = match handle_oauth_callback_request(&request, state) {
        Ok((code, _url)) => code,
        Err((status_line, error_message)) => {
            send_callback_response(
                provider_id,
                stream,
                status_line,
                CODEX_OAUTH_HTML_ERROR,
                (status_line != "HTTP/1.1 404 Not Found").then_some(CodexOAuthSession {
                    status: "error".to_string(),
                    message: error_message.clone(),
                    error: error_message.clone(),
                    email: String::new(),
                    account_id: String::new(),
                }),
            );
            if status_line == "HTTP/1.1 404 Not Found" {
                return true;
            }
            codex_auth_log_info(
                "oauth_login",
                "oauth_callback",
                provider_id,
                "error",
                started.elapsed().as_millis(),
                &[("message", error_message)],
            );
            return false;
        }
    };

    match process_oauth_code_exchange(provider_id, &code, code_verifier) {
        Ok(credential) => {
            send_callback_response(
                provider_id,
                stream,
                "HTTP/1.1 200 OK",
                CODEX_OAUTH_HTML_SUCCESS,
                Some(CodexOAuthSession {
                    status: "authenticated".to_string(),
                    message: "Codex OAuth 登录成功。".to_string(),
                    error: String::new(),
                    email: credential.email.clone(),
                    account_id: credential.account_id.clone(),
                }),
            );
            codex_auth_log_info(
                "oauth_login",
                "oauth_callback",
                provider_id,
                "authenticated",
                started.elapsed().as_millis(),
                &[
                    ("email", credential.email.clone()),
                    ("account_id", credential.account_id.clone()),
                ],
            );
        }
        Err(err) => {
            send_callback_response(
                provider_id,
                stream,
                "HTTP/1.1 500 Internal Server Error",
                CODEX_OAUTH_HTML_ERROR,
                Some(CodexOAuthSession {
                    status: "error".to_string(),
                    message: format!("Codex OAuth 换取 token 失败: {err}"),
                    error: err.clone(),
                    email: String::new(),
                    account_id: String::new(),
                }),
            );
            codex_auth_log_info(
                "oauth_login",
                "oauth_callback",
                provider_id,
                "error",
                started.elapsed().as_millis(),
                &[("message", err)],
            );
        }
    }

    false
}

fn codex_start_listener_thread(provider_id: String, code_verifier: String, state: String) {
    std::thread::spawn(move || {
        // ========== 初始化 ==========
        let listener = match std::net::TcpListener::bind(("127.0.0.1", CODEX_OAUTH_CALLBACK_PORT)) {
            Ok(listener) => listener,
            Err(err) => {
                codex_session_set(
                    &provider_id,
                    CodexOAuthSession {
                        status: "error".to_string(),
                        message: format!("Codex OAuth 回调端口被占用: {err}"),
                        error: err.to_string(),
                        email: String::new(),
                        account_id: String::new(),
                    },
                );
                codex_auth_log_info(
                    "oauth_login",
                    "listener_bind",
                    &provider_id,
                    "error",
                    0,
                    &[("message", format!("Codex OAuth 回调端口被占用: {err}"))],
                );
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        let started = std::time::Instant::now();

        loop {
            // ========== 超时判断 ==========
            if started.elapsed() > std::time::Duration::from_secs(300) {
                codex_session_set(
                    &provider_id,
                    CodexOAuthSession {
                        status: "expired".to_string(),
                        message: "Codex OAuth 登录已超时，请重新发起。".to_string(),
                        error: "expired".to_string(),
                        email: String::new(),
                        account_id: String::new(),
                    },
                );
                codex_auth_log_info(
                    "oauth_login",
                    "listener_timeout",
                    &provider_id,
                    "expired",
                    started.elapsed().as_millis(),
                    &[],
                );
                return;
            }

            // ========== 处理回调 ==========
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    if handle_oauth_callback_connection(
                        &provider_id,
                        &code_verifier,
                        &state,
                        &started,
                        &mut stream,
                    ) {
                        continue;
                    }
                    return;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(err) => {
                    codex_session_set(
                        &provider_id,
                        CodexOAuthSession {
                            status: "error".to_string(),
                            message: format!("Codex OAuth 回调监听失败: {err}"),
                            error: err.to_string(),
                            email: String::new(),
                            account_id: String::new(),
                        },
                    );
                    codex_auth_log_info(
                        "oauth_login",
                        "listener_accept",
                        &provider_id,
                        "error",
                        started.elapsed().as_millis(),
                        &[("message", format!("Codex OAuth 回调监听失败: {err}"))],
                    );
                    return;
                }
            }
        }
    });
}

#[tauri::command]
async fn codex_get_auth_status(input: CodexAuthStatusInput) -> Result<CodexAuthStatus, String> {
    let auth_mode = normalize_codex_auth_mode(&input.auth_mode);
    let managed_auth_path = managed_codex_auth_path(&input.provider_id)?
        .to_string_lossy()
        .to_string();
    if auth_mode == CODEX_AUTH_MODE_MANAGED_OAUTH {
        if let Some(session) = codex_session_get(&input.provider_id) {
            return Ok(CodexAuthStatus {
                provider_id: input.provider_id,
                auth_mode,
                authenticated: session.status == "authenticated",
                status: session.status,
                message: if session.error.trim().is_empty() {
                    session.message
                } else {
                    format!("{} ({})", session.message, session.error)
                },
                email: session.email,
                account_id: session.account_id,
                access_token_preview: String::new(),
                local_auth_path: String::new(),
                managed_auth_path,
                expires_at: String::new(),
            });
        }
    }
    let auth = read_codex_runtime_auth_snapshot(
        &input.provider_id,
        &auth_mode,
        &input.local_auth_path,
    )?;
    let mut status = codex_status_from_runtime_auth(&auth);
    status.managed_auth_path = managed_auth_path;
    Ok(status)
}

#[tauri::command]
async fn codex_start_oauth_login(input: CodexStartOAuthLoginInput) -> Result<CodexAuthStatus, String> {
    let provider_id = input.provider_id.trim();
    if provider_id.is_empty() {
        return Err("providerId 不能为空".to_string());
    }
    codex_auth_log_info("oauth_login", "user_action", provider_id, "start", 0, &[]);
    let code_verifier = codex_pkce_code_verifier();
    let code_challenge = codex_pkce_code_challenge(&code_verifier);
    let state = format!("codex-{}", Uuid::new_v4().simple());
    let auth_url = codex_build_authorize_url(&code_challenge, &state);
    codex_session_set(
        provider_id,
        CodexOAuthSession {
            status: "pending".to_string(),
            message: "浏览器已打开，请完成 Codex OAuth 登录。".to_string(),
            error: String::new(),
            email: String::new(),
            account_id: String::new(),
        },
    );
    codex_start_listener_thread(provider_id.to_string(), code_verifier, state);
    webbrowser::open(&auth_url).map_err(|err| format!("打开 Codex OAuth 登录页失败: {err}"))?;
    Ok(CodexAuthStatus {
        provider_id: provider_id.to_string(),
        auth_mode: CODEX_AUTH_MODE_MANAGED_OAUTH.to_string(),
        authenticated: false,
        status: "pending".to_string(),
        message: "浏览器已打开，请完成 Codex OAuth 登录。".to_string(),
        email: String::new(),
        account_id: String::new(),
        access_token_preview: String::new(),
        local_auth_path: String::new(),
        managed_auth_path: managed_codex_auth_path(provider_id)?.to_string_lossy().to_string(),
        expires_at: String::new(),
    })
}

#[tauri::command]
fn codex_logout(input: CodexLogoutInput) -> Result<bool, String> {
    let provider_id = input.provider_id.trim();
    if provider_id.is_empty() {
        return Err("providerId 不能为空".to_string());
    }
    delete_managed_codex_auth(provider_id)?;
    codex_session_remove(provider_id);
    codex_auth_log_info("logout", "user_action", provider_id, "done", 0, &[]);
    Ok(true)
}
