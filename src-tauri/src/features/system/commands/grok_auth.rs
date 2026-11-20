const GROK_ISSUER: &str = "https://auth.x.ai";
const GROK_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const GROK_SCOPE: &str = "openid profile email offline_access grok-cli:access api:access conversations:read conversations:write workspaces:read workspaces:write";
const GROK_OAUTH_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";
const GROK_OAUTH_CLIENT_VERSION: &str = "1.0.45";
const GROK_LOGIN_TIMEOUT_SECONDS: u64 = 300;
const GROK_REFRESH_SKEW_SECONDS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GrokAuthFile {
    #[serde(rename = "type")]
    pub(crate) token_type_name: String,
    pub(crate) access_token: String,
    #[serde(default)]
    pub(crate) refresh_token: String,
    #[serde(default)]
    pub(crate) id_token: String,
    #[serde(default)]
    pub(crate) token_type: String,
    #[serde(default)]
    pub(crate) expires_in: u64,
    #[serde(default)]
    pub(crate) expired: String,
    #[serde(default)]
    pub(crate) last_refresh: String,
    #[serde(default)]
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) sub: String,
    #[serde(default)]
    pub(crate) base_url: String,
    #[serde(default)]
    pub(crate) redirect_uri: String,
    #[serde(default)]
    pub(crate) token_endpoint: String,
    #[serde(default)]
    pub(crate) auth_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GrokAuthStatus {
    pub(crate) provider_id: String,
    pub(crate) authenticated: bool,
    pub(crate) status: String,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) account_id: String,
    #[serde(default)]
    pub(crate) file_name: String,
    #[serde(default)]
    pub(crate) file_path: String,
    #[serde(default)]
    pub(crate) expires_at: String,
    #[serde(default)]
    pub(crate) base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokProviderInput {
    provider_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokImportInput {
    provider_id: String,
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct GrokTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    id_token: String,
    #[serde(default)]
    token_type: String,
    #[serde(default)]
    expires_in: u64,
}

struct GrokOidcMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
}

fn grok_refresh_locks() -> &'static Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn grok_refresh_lock(file_name: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut locks = grok_refresh_locks().lock().unwrap_or_else(|error| error.into_inner());
    locks
        .entry(file_name.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

pub(crate) fn grok_auth_dir() -> Result<PathBuf, String> {
    if let Some(portable_root) = detect_portable_runtime_root() {
        return Ok(portable_root.join("auth").join("grok"));
    }
    let (config_dir, _legacy_dir) = resolve_standard_config_dir()?;
    Ok(app_root_from_data_path(&config_dir.join("config_mark"))
        .join("auth")
        .join("grok"))
}

fn normalize_grok_auth_file_name(file_name: &str) -> Result<String, String> {
    let trimmed = file_name.trim();
    let candidate = trimmed.strip_prefix("xai-").unwrap_or(trimmed);
    let candidate = candidate.strip_suffix(".json").unwrap_or(candidate);
    if candidate.is_empty()
        || candidate.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || candidate == "."
        || candidate == ".."
    {
        return Err("Grok 凭证文件名无效".to_string());
    }
    Ok(format!("xai-{candidate}.json"))
}

fn grok_auth_file_path(_data_path: &Path, file_name: &str) -> Result<PathBuf, String> {
    Ok(grok_auth_dir()?.join(normalize_grok_auth_file_name(file_name)?))
}

fn grok_selection_path(_data_path: &Path) -> Result<PathBuf, String> {
    Ok(grok_auth_dir()?.join("selected.json"))
}

fn read_grok_selection(data_path: &Path) -> Result<HashMap<String, String>, String> {
    let path = grok_selection_path(data_path)?;
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(HashMap::new());
    };
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn write_grok_selection(data_path: &Path, selection: &HashMap<String, String>) -> Result<(), String> {
    let path = grok_selection_path(data_path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 Grok 凭证目录失败: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(selection).map_err(|error| format!("序列化 Grok 账号选择失败: {error}"))?;
    fs::write(path, format!("{raw}\n")).map_err(|error| format!("写入 Grok 账号选择失败: {error}"))
}

fn read_selected_grok_account(state: &AppState, provider_id: &str) -> Result<String, String> {
    Ok(read_grok_selection(&state.data_path)?
        .get(provider_id.trim())
        .cloned()
        .unwrap_or_default())
}

fn write_selected_grok_account(state: &AppState, provider_id: &str, file_name: &str) -> Result<(), String> {
    let mut selection = read_grok_selection(&state.data_path)?;
    if file_name.trim().is_empty() {
        selection.remove(provider_id.trim());
    } else {
        selection.insert(provider_id.trim().to_string(), normalize_grok_auth_file_name(file_name)?);
    }
    write_grok_selection(&state.data_path, &selection)
}

fn parse_grok_auth_file(raw: &str) -> Result<GrokAuthFile, String> {
    let parsed: GrokAuthFile =
        serde_json::from_str(raw).map_err(|error| format!("Grok 凭证解析失败: {error}"))?;
    if parsed.token_type_name.trim() != "xai" {
        return Err("Grok 凭证类型不是 xai".to_string());
    }
    if parsed.access_token.trim().is_empty() {
        return Err("Grok 凭证缺少 access_token".to_string());
    }
    Ok(parsed)
}

fn load_grok_auth_from_path(path: &Path) -> Result<GrokAuthFile, String> {
    let raw = fs::read_to_string(path).map_err(|error| format!("读取 Grok 凭证失败: {error}"))?;
    parse_grok_auth_file(&raw)
}

fn write_grok_auth_file(path: &Path, auth: &GrokAuthFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 Grok 凭证目录失败: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(auth).map_err(|error| format!("序列化 Grok 凭证失败: {error}"))?;
    fs::write(path, format!("{raw}\n")).map_err(|error| format!("写入 Grok 凭证失败: {error}"))
}

fn grok_expired_epoch_seconds(auth: &GrokAuthFile) -> Option<u64> {
    let expired = auth.expired.trim();
    if !expired.is_empty() {
        return chrono::DateTime::parse_from_rfc3339(expired)
            .ok()
            .map(|value| value.timestamp().max(0) as u64);
    }
    // 外部导入的凭证可能没有 expired（本应用写出的文件总会带上），
    // 这时用 last_refresh + expires_in 推断过期时刻，避免每次都判成已过期而强制刷新。
    let refreshed_at = chrono::DateTime::parse_from_rfc3339(auth.last_refresh.trim())
        .ok()
        .map(|value| value.timestamp().max(0) as u64)?;
    if auth.expires_in == 0 {
        return None;
    }
    Some(refreshed_at.saturating_add(auth.expires_in))
}

fn grok_token_needs_refresh(auth: &GrokAuthFile) -> bool {
    let Some(expires_at) = grok_expired_epoch_seconds(auth) else {
        return !auth.refresh_token.trim().is_empty();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    expires_at <= now.saturating_add(GROK_REFRESH_SKEW_SECONDS)
}

fn grok_status_from_file(provider_id: &str, path: &Path, auth: &GrokAuthFile) -> GrokAuthStatus {
    let expired = grok_token_needs_refresh(auth);
    GrokAuthStatus {
        provider_id: provider_id.to_string(),
        authenticated: !auth.access_token.trim().is_empty(),
        status: if expired { "expired".to_string() } else { "authenticated".to_string() },
        message: if expired { "Grok 登录已过期".to_string() } else { "Grok 已登录".to_string() },
        email: auth.email.clone(),
        account_id: auth.sub.clone(),
        file_name: path.file_name().and_then(|value| value.to_str()).unwrap_or_default().to_string(),
        file_path: path.display().to_string(),
        expires_at: auth.expired.clone(),
        base_url: auth.base_url.clone(),
    }
}

fn grok_status_missing(provider_id: &str) -> GrokAuthStatus {
    GrokAuthStatus {
        provider_id: provider_id.to_string(),
        authenticated: false,
        status: "missing".to_string(),
        message: "尚未登录 Grok".to_string(),
        email: String::new(),
        account_id: String::new(),
        file_name: String::new(),
        file_path: String::new(),
        expires_at: String::new(),
        base_url: String::new(),
    }
}

pub(crate) fn grok_auth_status_for_provider(state: &AppState, provider_id: &str) -> Result<GrokAuthStatus, String> {
    let file_name = read_selected_grok_account(state, provider_id)?;
    if file_name.trim().is_empty() {
        return Ok(grok_status_missing(provider_id));
    }
    let path = grok_auth_file_path(&state.data_path, &file_name)?;
    if !path.is_file() {
        return Ok(GrokAuthStatus {
            status: "error".to_string(),
            message: format!("Grok 凭证文件不存在: {}", path.display()),
            file_name,
            ..grok_status_missing(provider_id)
        });
    }
    let auth = load_grok_auth_from_path(&path)?;
    Ok(grok_status_from_file(provider_id, &path, &auth))
}

async fn fetch_grok_oidc_metadata() -> Result<GrokOidcMetadata, String> {
    let response = reqwest::Client::new()
        .get(format!("{GROK_ISSUER}/.well-known/openid-configuration"))
        .send()
        .await
        .map_err(|error| format!("读取 Grok OIDC 发现文档失败: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("读取 Grok OIDC 发现文档失败: HTTP {}", response.status()));
    }
    let body: Value = response.json().await.map_err(|error| format!("解析 Grok OIDC 发现文档失败: {error}"))?;
    let authorization_endpoint = body.get("authorization_endpoint").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let token_endpoint = body.get("token_endpoint").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if authorization_endpoint.is_empty() || token_endpoint.is_empty() {
        return Err("Grok OIDC 发现文档缺少授权或令牌端点".to_string());
    }
    Ok(GrokOidcMetadata { authorization_endpoint, token_endpoint })
}

fn pkce_challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

struct GrokLoopback {
    redirect_uri: String,
    listener: std::net::TcpListener,
}

fn start_grok_loopback() -> Result<GrokLoopback, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|error| format!("绑定 Grok 回调端口失败: {error}"))?;
    let port = listener.local_addr().map_err(|error| format!("读取 Grok 回调端口失败: {error}"))?.port();
    Ok(GrokLoopback {
        redirect_uri: format!("http://127.0.0.1:{port}/callback"),
        listener,
    })
}

fn build_grok_authorize_url(metadata: &GrokOidcMetadata, redirect_uri: &str, state: &str, nonce: &str, challenge: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(&metadata.authorization_endpoint).map_err(|error| format!("Grok 授权端点无效: {error}"))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", GROK_CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", GROK_SCOPE)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("referrer", "grok-build");
    Ok(url.to_string())
}

const GROK_ACCOUNTS_ORIGIN: &str = "https://accounts.x.ai";

enum GrokCallbackDecision {
    Continue,
    Code(String),
}

fn grok_callback_cors_headers() -> String {
    format!(
        "Access-Control-Allow-Origin: {GROK_ACCOUNTS_ORIGIN}\r\nAccess-Control-Allow-Methods: GET\r\nAccess-Control-Allow-Private-Network: true\r\nVary: Origin\r\n"
    )
}

fn write_grok_http_response(stream: &mut std::net::TcpStream, status_line: &str, content_type: Option<&str>, body: &str) {
    let type_header = content_type
        .map(|value| format!("Content-Type: {value}\r\n"))
        .unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {status_line}\r\n{type_header}Content-Length: {}\r\nConnection: close\r\n{}\r\n{body}",
        body.len(),
        grok_callback_cors_headers()
    );
    let _ = stream.write_all(header.as_bytes());
}

fn classify_grok_callback(request: &str, expected_state: &str) -> Result<GrokCallbackDecision, String> {
    let request_line = request.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method.eq_ignore_ascii_case("OPTIONS") || !method.eq_ignore_ascii_case("GET") {
        return Ok(GrokCallbackDecision::Continue);
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|error| format!("解析 Grok 回调失败: {error}"))?;
    if url.path() != "/callback" {
        return Ok(GrokCallbackDecision::Continue);
    }
    let query: HashMap<String, String> = url.query_pairs().map(|(key, value)| (key.to_string(), value.to_string())).collect();
    if query.get("state").map(String::as_str).unwrap_or_default() != expected_state {
        return Err("Grok 登录回调 state 不匹配".to_string());
    }
    if let Some(error) = query.get("error").filter(|value| !value.trim().is_empty()) {
        return Err(format!("Grok 登录被拒绝: {error}"));
    }
    let code = query.get("code").cloned().unwrap_or_default();
    if code.trim().is_empty() {
        return Err("Grok 登录回调缺少 code".to_string());
    }
    Ok(GrokCallbackDecision::Code(code))
}

fn wait_for_grok_callback(listener: std::net::TcpListener, expected_state: &str) -> Result<String, String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("设置 Grok 回调超时失败: {error}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(GROK_LOGIN_TIMEOUT_SECONDS);
    loop {
        if std::time::Instant::now() >= deadline {
            return Err("等待 Grok 登录回调超时".to_string());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                if request.trim().is_empty() {
                    continue;
                }
                match classify_grok_callback(&request, expected_state) {
                    Ok(GrokCallbackDecision::Continue) => {
                        write_grok_http_response(&mut stream, "204 No Content", None, "");
                    }
                    Ok(GrokCallbackDecision::Code(code)) => {
                        write_grok_http_response(
                            &mut stream,
                            "200 OK",
                            Some("text/plain; charset=utf-8"),
                            "Grok login complete. You can return to PAI.",
                        );
                        return Ok(code);
                    }
                    Err(error) => {
                        write_grok_http_response(&mut stream, "400 Bad Request", Some("text/plain; charset=utf-8"), "");
                        return Err(error);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => return Err(format!("等待 Grok 登录回调失败: {error}")),
        }
    }
}

async fn post_grok_token(token_endpoint: &str, form: &[(&str, &str)]) -> Result<GrokTokenResponse, String> {
    let response = reqwest::Client::new()
        .post(token_endpoint)
        .form(form)
        .send()
        .await
        .map_err(|error| format!("Grok 令牌请求失败: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Grok 令牌请求失败: HTTP {status} {body}"));
    }
    response.json::<GrokTokenResponse>().await.map_err(|error| format!("解析 Grok 令牌失败: {error}"))
}

fn jwt_claim(token: &str, claim: &str) -> String {
    let Some(payload) = token.split('.').nth(1) else {
        return String::new();
    };
    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload) else {
        return String::new();
    };
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|value| value.get(claim).and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

fn grok_expired_rfc3339(expires_in: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0);
    chrono::DateTime::<chrono::Utc>::from_timestamp(now.saturating_add(expires_in.max(1)) as i64, 0)
        .map(|value| value.to_rfc3339())
        .unwrap_or_default()
}

fn grok_auth_from_token_response(
    tokens: GrokTokenResponse,
    redirect_uri: &str,
    token_endpoint: &str,
    previous: Option<&GrokAuthFile>,
) -> Result<GrokAuthFile, String> {
    let id_token = if tokens.id_token.trim().is_empty() {
        previous.map(|value| value.id_token.clone()).unwrap_or_default()
    } else {
        tokens.id_token
    };
    let email = {
        let claimed = jwt_claim(&id_token, "email");
        if claimed.trim().is_empty() { previous.map(|value| value.email.clone()).unwrap_or_default() } else { claimed }
    };
    if email.trim().is_empty() {
        return Err("Grok 登录结果缺少邮箱".to_string());
    }
    let sub = {
        let claimed = jwt_claim(&id_token, "sub");
        if claimed.trim().is_empty() { previous.map(|value| value.sub.clone()).unwrap_or_default() } else { claimed }
    };
    let refresh_token = if tokens.refresh_token.trim().is_empty() {
        previous.map(|value| value.refresh_token.clone()).unwrap_or_default()
    } else {
        tokens.refresh_token
    };
    Ok(GrokAuthFile {
        token_type_name: "xai".to_string(),
        access_token: tokens.access_token,
        refresh_token,
        id_token,
        token_type: if tokens.token_type.trim().is_empty() { "Bearer".to_string() } else { tokens.token_type },
        expires_in: tokens.expires_in,
        expired: grok_expired_rfc3339(tokens.expires_in),
        last_refresh: chrono::Utc::now().to_rfc3339(),
        email,
        sub,
        base_url: GROK_OAUTH_BASE_URL.to_string(),
        redirect_uri: redirect_uri.to_string(),
        token_endpoint: token_endpoint.to_string(),
        auth_kind: "oauth".to_string(),
    })
}

fn open_system_browser(url: &str) -> Result<(), String> {
    webbrowser::open(url).map_err(|error| format!("打开浏览器失败: {error}"))
}

fn persist_grok_login(state: &AppState, provider_id: &str, auth: &GrokAuthFile) -> Result<GrokAuthStatus, String> {
    let path = grok_auth_file_path(&state.data_path, &auth.email)?;
    write_grok_auth_file(&path, auth)?;
    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
    write_selected_grok_account(state, provider_id, file_name)?;
    runtime_log_info(format!("[Grok登录] 已写入凭证 {}", path.display()));
    Ok(grok_status_from_file(provider_id, &path, auth))
}

pub(crate) async fn grok_auth_login(app: AppHandle, input: GrokProviderInput) -> Result<GrokAuthStatus, String> {
    let provider_id = input.provider_id.trim().to_string();
    if provider_id.is_empty() {
        return Err("缺少 Grok 供应商".to_string());
    }
    let metadata = fetch_grok_oidc_metadata().await?;
    let loopback = start_grok_loopback()?;
    let verifier = random_token();
    let state_value = random_token();
    let authorize_url = build_grok_authorize_url(&metadata, &loopback.redirect_uri, &state_value, &random_token(), &pkce_challenge(&verifier))?;
    open_system_browser(&authorize_url)?;
    let redirect_uri = loopback.redirect_uri.clone();
    let token_endpoint = metadata.token_endpoint.clone();
    let code = tokio::task::spawn_blocking(move || wait_for_grok_callback(loopback.listener, &state_value))
        .await
        .map_err(|error| format!("等待 Grok 登录回调失败: {error}"))??;
    let tokens = post_grok_token(
        &token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", GROK_CLIENT_ID),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", verifier.as_str()),
        ],
    )
    .await?;
    let auth = grok_auth_from_token_response(tokens, &redirect_uri, &token_endpoint, None)?;
    let state = app.state::<AppState>();
    persist_grok_login(state.inner(), &provider_id, &auth)
}

pub(crate) async fn refresh_grok_auth_file(path: &Path) -> Result<GrokAuthFile, String> {
    let current = load_grok_auth_from_path(path)?;
    if current.refresh_token.trim().is_empty() {
        return Err("Grok 凭证缺少 refresh_token，无法刷新".to_string());
    }
    let token_endpoint = if current.token_endpoint.trim().is_empty() {
        format!("{GROK_ISSUER}/oauth2/token")
    } else {
        current.token_endpoint.clone()
    };
    let file_name = normalize_grok_auth_file_name(
        path.file_name().and_then(|value| value.to_str()).unwrap_or_default(),
    )?;
    let lock = grok_refresh_lock(&file_name);
    let _guard = lock.lock().await;
    let latest = load_grok_auth_from_path(path)?;
    if !grok_token_needs_refresh(&latest) {
        return Ok(latest);
    }
    let tokens = post_grok_token(
        &token_endpoint,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", GROK_CLIENT_ID),
            ("refresh_token", latest.refresh_token.as_str()),
        ],
    )
    .await?;
    let refreshed = grok_auth_from_token_response(tokens, &latest.redirect_uri, &token_endpoint, Some(&latest))?;
    write_grok_auth_file(path, &refreshed)?;
    Ok(refreshed)
}

pub(crate) async fn grok_auth_refresh(app: AppHandle, input: GrokProviderInput) -> Result<GrokAuthStatus, String> {
    let state = app.state::<AppState>();
    let provider_id = input.provider_id.trim();
    let file_name = read_selected_grok_account(state.inner(), provider_id)?;
    if file_name.trim().is_empty() {
        return Err("尚未选择 Grok 凭证".to_string());
    }
    let path = grok_auth_file_path(&state.data_path, &file_name)?;
    let auth = refresh_grok_auth_file(&path).await?;
    Ok(grok_status_from_file(provider_id, &path, &auth))
}

pub(crate) fn grok_auth_logout(app: AppHandle, input: GrokProviderInput) -> Result<GrokAuthStatus, String> {
    let state = app.state::<AppState>();
    let provider_id = input.provider_id.trim();
    let file_name = read_selected_grok_account(state.inner(), provider_id)?;
    if !file_name.trim().is_empty() {
        let path = grok_auth_file_path(&state.data_path, &file_name)?;
        if path.is_file() {
            fs::remove_file(&path).map_err(|error| format!("删除 Grok 凭证失败: {error}"))?;
        }
    }
    write_selected_grok_account(state.inner(), provider_id, "")?;
    Ok(grok_status_missing(provider_id))
}

pub(crate) fn grok_auth_status(app: AppHandle, input: GrokProviderInput) -> Result<GrokAuthStatus, String> {
    let state = app.state::<AppState>();
    grok_auth_status_for_provider(state.inner(), input.provider_id.trim())
}

pub(crate) fn grok_auth_import(app: AppHandle, input: GrokImportInput) -> Result<GrokAuthStatus, String> {
    let state = app.state::<AppState>();
    let provider_id = input.provider_id.trim();
    if provider_id.is_empty() {
        return Err("缺少 Grok 供应商".to_string());
    }
    let source = PathBuf::from(input.file_path.trim());
    if !source.is_file() {
        return Err("Grok 凭证文件不存在".to_string());
    }
    let auth = load_grok_auth_from_path(&source)?;
    if auth.auth_kind.trim() != "oauth" && !auth.auth_kind.trim().is_empty() {
        runtime_log_warn(format!("[Grok登录] 导入文件 auth_kind={}，仍按 OAuth 凭证处理", auth.auth_kind));
    }
    persist_grok_login(state.inner(), provider_id, &auth)
}

fn grok_video_request_id(value: &Value) -> Option<String> {
    let request_id = value.get("request_id").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if request_id.is_empty() { None } else { Some(request_id) }
}

enum GrokVideoPollDecision {
    Pending,
    Ready(String),
    Failed(String),
}

fn grok_video_poll_decision(value: &Value) -> GrokVideoPollDecision {
    let status = value.get("status").and_then(Value::as_str).unwrap_or_default().trim().to_ascii_lowercase();
    if matches!(status.as_str(), "succeeded" | "completed" | "success" | "ready" | "done") {
        let url = value.get("video").and_then(|video| video.get("url")).and_then(Value::as_str).unwrap_or_default().trim().to_string();
        return GrokVideoPollDecision::Ready(if url.is_empty() { value.to_string() } else { url });
    }
    if matches!(status.as_str(), "failed" | "error" | "cancelled" | "canceled") {
        return GrokVideoPollDecision::Failed(format!("Grok 视频生成失败: {value}"));
    }
    GrokVideoPollDecision::Pending
}

pub(crate) fn grok_oauth_chat_endpoint(stored_base_url: &str) -> String {
    let trimmed = stored_base_url.trim().trim_end_matches('/');
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.is_empty() || lowered.contains("://api.x.ai") || lowered.starts_with("api.x.ai") {
        GROK_OAUTH_BASE_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn grok_oauth_chat_headers(base_url: &str) -> Vec<(String, String)> {
    let mut headers = vec![
        (
            "user-agent".to_string(),
            format!("xai-grok-workspace/{GROK_OAUTH_CLIENT_VERSION}"),
        ),
        (
            "x-grok-client-version".to_string(),
            GROK_OAUTH_CLIENT_VERSION.to_string(),
        ),
        (
            "x-grok-client-identifier".to_string(),
            "grok-shell".to_string(),
        ),
        // plugins/packages/grok/index.mjs:309-311 的 sign()：这两个头原来只在
        // 中转域名上补，真实 Grok CLI 每次都发，移到公共集合里。
        ("x-xai-token-auth".to_string(), "xai-grok-cli".to_string()),
        ("x-grok-client-mode".to_string(), "headless".to_string()),
    ];
    if base_url.contains("cli-chat-proxy") || base_url.contains("chat-proxy") {
        headers.push((
            "x-authenticateresponse".to_string(),
            "authenticate-response".to_string(),
        ));
    }
    headers
}

pub(crate) async fn resolve_grok_access_token(data_path: &Path, provider_id: &str) -> Result<(String, String), String> {
    let file_name = read_grok_selection(data_path)?
        .get(provider_id.trim())
        .cloned()
        .unwrap_or_default();
    if file_name.trim().is_empty() {
        return Err("尚未登录 Grok".to_string());
    }
    let path = grok_auth_file_path(data_path, &file_name)?;
    let mut auth = load_grok_auth_from_path(&path)?;
    if grok_token_needs_refresh(&auth) {
        auth = refresh_grok_auth_file(&path).await?;
    }
    let base_url = if auth.base_url.trim().is_empty() {
        GROK_OAUTH_BASE_URL.to_string()
    } else {
        auth.base_url.trim().trim_end_matches('/').to_string()
    };
    Ok((auth.access_token, base_url))
}

#[cfg(test)]
mod grok_auth_tests {
    use super::*;

    #[test]
    fn parses_cpa_token_file_and_normalizes_account_name() {
        let raw = r#"{
            "type": "xai",
            "access_token": "access",
            "refresh_token": "refresh",
            "email": "user@example.com",
            "auth_kind": "oauth",
            "base_url": "https://cli-chat-proxy.grok.com/v1"
        }"#;
        let parsed = parse_grok_auth_file(raw).expect("parse");
        assert_eq!(parsed.email, "user@example.com");
        assert_eq!(normalize_grok_auth_file_name("user@example.com").expect("name"), "xai-user@example.com.json");
        assert_eq!(normalize_grok_auth_file_name("xai-user@example.com.json").expect("name"), "xai-user@example.com.json");
    }

    #[test]
    fn expired_token_needs_refresh() {
        let auth = GrokAuthFile {
            token_type_name: "xai".to_string(),
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            id_token: String::new(),
            token_type: "Bearer".to_string(),
            expires_in: 1,
            expired: "2000-01-01T00:00:00+00:00".to_string(),
            last_refresh: String::new(),
            email: "user@example.com".to_string(),
            sub: String::new(),
            base_url: GROK_OAUTH_BASE_URL.to_string(),
            redirect_uri: String::new(),
            token_endpoint: String::new(),
            auth_kind: "oauth".to_string(),
        };
        assert!(grok_token_needs_refresh(&auth));
    }

    fn grok_auth_without_expired(last_refresh: &str, expires_in: u64) -> GrokAuthFile {
        GrokAuthFile {
            token_type_name: "xai".to_string(),
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            id_token: String::new(),
            token_type: "Bearer".to_string(),
            expires_in,
            expired: String::new(),
            last_refresh: last_refresh.to_string(),
            email: "user@example.com".to_string(),
            sub: String::new(),
            base_url: GROK_OAUTH_BASE_URL.to_string(),
            redirect_uri: String::new(),
            token_endpoint: String::new(),
            auth_kind: "oauth".to_string(),
        }
    }

    #[test]
    fn missing_expired_falls_back_to_last_refresh_and_expires_in() {
        let fresh = grok_auth_without_expired(&chrono::Utc::now().to_rfc3339(), 3600);
        assert!(!grok_token_needs_refresh(&fresh));
        let stale = grok_auth_without_expired("2000-01-01T00:00:00+00:00", 60);
        assert!(grok_token_needs_refresh(&stale));
    }

    #[test]
    fn missing_expired_without_usable_timing_keeps_refresh_fallback() {
        let empty_last_refresh = grok_auth_without_expired("", 3600);
        assert!(grok_token_needs_refresh(&empty_last_refresh));
        let unknown_lifetime = grok_auth_without_expired(&chrono::Utc::now().to_rfc3339(), 0);
        assert!(grok_token_needs_refresh(&unknown_lifetime));
    }

    #[test]
    fn public_xai_base_url_uses_cli_chat_proxy() {
        assert_eq!(
            grok_oauth_chat_endpoint("https://api.x.ai/v1"),
            "https://cli-chat-proxy.grok.com/v1"
        );
        assert_eq!(
            grok_oauth_chat_endpoint("https://cli-chat-proxy.grok.com/v1"),
            "https://cli-chat-proxy.grok.com/v1"
        );
        let headers = grok_oauth_chat_headers("https://cli-chat-proxy.grok.com/v1");
        assert!(headers.iter().any(|(key, value)| key == "x-grok-client-version" && !value.is_empty()));
        assert!(headers.iter().any(|(key, value)| key == "x-xai-token-auth" && value == "xai-grok-cli"));
        assert!(headers.iter().any(|(key, value)| key == "x-grok-client-mode" && value == "headless"));
    }

    #[test]
    fn image_to_video_rejects_duration_and_resolution_outside_grok_cli() {
        let image = "data:image/png;base64,aaaa";
        assert!(generate_grok_image_to_video_request(image, "", 5, "480p").is_err());
        assert!(generate_grok_image_to_video_request(image, "", 6, "1080p").is_err());
        let request = generate_grok_image_to_video_request(image, "pan", 6, "480p").expect("request");
        assert_eq!(request.duration, 6);
        assert_eq!(request.resolution, "480p");
        assert_eq!(request.image_url, image);
    }

    #[test]
    fn video_status_reads_request_id_and_video_url() {
        let created = serde_json::json!({"request_id": "req-1", "status": "queued"});
        assert_eq!(grok_video_request_id(&created).expect("id"), "req-1");
        assert!(matches!(grok_video_poll_decision(&created), GrokVideoPollDecision::Pending));

        let ready = serde_json::json!({
            "request_id": "req-1",
            "status": "done",
            "video": { "url": "https://example.test/video.mp4" }
        });
        match grok_video_poll_decision(&ready) {
            GrokVideoPollDecision::Ready(url) => assert_eq!(url, "https://example.test/video.mp4"),
            GrokVideoPollDecision::Pending | GrokVideoPollDecision::Failed(_) => panic!("expected ready"),
        }

        let failed = serde_json::json!({"status": "failed"});
        assert!(matches!(grok_video_poll_decision(&failed), GrokVideoPollDecision::Failed(_)));
    }

    #[test]
    fn authorize_url_includes_grok_cli_referrer() {
        let metadata = GrokOidcMetadata {
            authorization_endpoint: "https://auth.x.ai/oauth2/authorize".to_string(),
            token_endpoint: "https://auth.x.ai/oauth2/token".to_string(),
        };
        let url = build_grok_authorize_url(&metadata, "http://127.0.0.1:9/callback", "state", "nonce", "challenge").expect("url");
        assert!(url.contains("referrer=grok-build"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn callback_accepts_private_network_preflight_then_code() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || wait_for_grok_callback(listener, "state-1"));
        let mut client = connect_grok_callback(port);
        client
            .write_all(b"OPTIONS /callback?code=abc&state=state-1 HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: https://accounts.x.ai\r\nAccess-Control-Request-Method: GET\r\nAccess-Control-Request-Private-Network: true\r\nConnection: close\r\n\r\n")
            .expect("preflight");
        let preflight = read_grok_callback_response(&mut client);
        assert!(preflight.contains("204"));
        assert!(preflight.contains("Access-Control-Allow-Origin: https://accounts.x.ai"));
        assert!(preflight.contains("Access-Control-Allow-Private-Network: true"));
        drop(client);

        let mut client = connect_grok_callback(port);
        client
            .write_all(b"GET /callback?code=abc&state=state-1 HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: https://accounts.x.ai\r\nConnection: close\r\n\r\n")
            .expect("callback");
        let response = read_grok_callback_response(&mut client);
        assert!(response.contains("200"));
        assert!(response.contains("Access-Control-Allow-Private-Network: true"));
        let code = server.join().expect("join").expect("code");
        assert_eq!(code, "abc");
    }

    fn connect_grok_callback(port: u16) -> std::net::TcpStream {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Ok(stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).expect("timeout");
                return stream;
            }
            if std::time::Instant::now() >= deadline {
                panic!("连接 Grok 回调端口失败");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn read_grok_callback_response(stream: &mut std::net::TcpStream) -> String {
        use std::io::Read;
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }
}
