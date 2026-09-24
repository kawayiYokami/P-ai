use once_cell::sync::OnceCell;
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationRequest, CredentialStore, OAuthState,
    StoredCredentials,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MCP_OAUTH_TIMEOUT_SECS: u64 = 300;

const MCP_OAUTH_HTML_SUCCESS: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>PAI - 授权成功</title>
</head>
<body style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;padding:48px;text-align:center;background:#0d1117;color:#e6edf3;">
  <div style="display:inline-block;padding:32px 48px;border-radius:12px;background:#161b22;border:1px solid #30363d;box-shadow:0 8px 24px rgba(0,0,0,0.4);">
    <h2 style="color:#3fb950;margin-top:0;font-size:22px;">授权成功</h2>
    <p style="color:#8b949e;margin-bottom:0;font-size:14px;">可以关闭此页面，返回 P-ai 继续使用。</p>
  </div>
  <script>setTimeout(()=>window.close(),2500)</script>
</body>
</html>"#;

const MCP_OAUTH_HTML_ERROR: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>PAI - 授权失败</title>
</head>
<body style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;padding:48px;text-align:center;background:#0d1117;color:#e6edf3;">
  <div style="display:inline-block;padding:32px 48px;border-radius:12px;background:#161b22;border:1px solid #30363d;box-shadow:0 8px 24px rgba(0,0,0,0.4);">
    <h2 style="color:#f85149;margin-top:0;font-size:22px;">授权失败</h2>
    <p style="color:#8b949e;margin-bottom:0;font-size:14px;">请返回 P-ai 查看详情并重试。</p>
  </div>
</body>
</html>"#;

// ==================== 凭据文件持久化 ====================

#[derive(Clone, Debug)]
pub struct McpFileCredentialStore {
    path: PathBuf,
}

impl McpFileCredentialStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait::async_trait]
impl CredentialStore for McpFileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| AuthError::InternalError(format!("读取 MCP OAuth 凭据文件失败: {e}")))?;
        let creds: StoredCredentials = serde_json::from_str(&content)
            .map_err(|e| AuthError::InternalError(format!("解析 MCP OAuth 凭据文件失败: {e}")))?;
        Ok(Some(creds))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        if let Some(parent) = self.path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let content = serde_json::to_string_pretty(&credentials)
            .map_err(|e| AuthError::InternalError(format!("序列化 MCP OAuth 凭据失败: {e}")))?;
        tokio::fs::write(&self.path, content)
            .await
            .map_err(|e| AuthError::InternalError(format!("写入 MCP OAuth 凭据文件失败: {e}")))?;
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        if self.path.exists() {
            let _ = tokio::fs::remove_file(&self.path).await;
        }
        Ok(())
    }
}

// ==================== 401 挑战头内存缓存 ====================

fn mcp_oauth_challenge_store() -> &'static std::sync::Mutex<HashMap<String, String>> {
    static STORE: OnceCell<std::sync::Mutex<HashMap<String, String>>> = OnceCell::new();
    STORE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn mcp_oauth_set_cached_challenge(server_id: &str, challenge: &str) {
    if let Ok(mut guard) = mcp_oauth_challenge_store().lock() {
        guard.insert(server_id.to_string(), challenge.to_string());
    }
}

fn mcp_oauth_get_cached_challenge(server_id: &str) -> Option<String> {
    let Ok(guard) = mcp_oauth_challenge_store().lock() else {
        return None;
    };
    guard.get(server_id).cloned()
}

fn mcp_oauth_clear_cached_challenge(server_id: &str) {
    if let Ok(mut guard) = mcp_oauth_challenge_store().lock() {
        guard.remove(server_id);
    }
}

// ==================== 授权会话状态管理 ====================

struct ActiveOAuthSession {
    _server_id: String,
    status: String, // "authorizing" | "success" | "error" | "cancelled" | "expired"
    message: String,
    auth_url: String,
    cancel_flag: Arc<AtomicBool>,
}

fn mcp_oauth_sessions() -> &'static std::sync::Mutex<HashMap<String, ActiveOAuthSession>> {
    static SESSIONS: OnceCell<std::sync::Mutex<HashMap<String, ActiveOAuthSession>>> = OnceCell::new();
    SESSIONS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn mcp_oauth_get_session_status(server_id: &str, has_credentials: bool) -> McpOAuthStatusResult {
    let Ok(guard) = mcp_oauth_sessions().lock() else {
        return McpOAuthStatusResult {
            server_id: server_id.to_string(),
            status: "idle".to_string(),
            message: String::new(),
            auth_url: String::new(),
            has_credentials,
        };
    };
    if let Some(session) = guard.get(server_id) {
        McpOAuthStatusResult {
            server_id: server_id.to_string(),
            status: session.status.clone(),
            message: session.message.clone(),
            auth_url: session.auth_url.clone(),
            has_credentials,
        }
    } else {
        McpOAuthStatusResult {
            server_id: server_id.to_string(),
            status: if has_credentials {
                "authenticated".to_string()
            } else {
                "idle".to_string()
            },
            message: String::new(),
            auth_url: String::new(),
            has_credentials,
        }
    }
}

fn mcp_oauth_cancel_login_session(server_id: &str) -> bool {
    let Ok(mut guard) = mcp_oauth_sessions().lock() else {
        return false;
    };
    if let Some(session) = guard.get_mut(server_id) {
        session.cancel_flag.store(true, Ordering::SeqCst);
        session.status = "cancelled".to_string();
        session.message = "用户取消了授权。".to_string();
        true
    } else {
        false
    }
}

enum OAuthCallbackQuery {
    Code {
        code: String,
        state: String,
        issuer: Option<String>,
    },
    Error {
        error: String,
        error_description: Option<String>,
    },
}

fn parse_oauth_callback_query(request_line: &str) -> Option<OAuthCallbackQuery> {
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    let mut code = None;
    let mut state = None;
    let mut issuer = None;
    let mut error = None;
    let mut error_description = None;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        let val = parts.next().unwrap_or("");
        let decoded = urlencoding::decode(val)
            .unwrap_or(std::borrow::Cow::Borrowed(val))
            .to_string();
        match key {
            "code" => code = Some(decoded),
            "state" => state = Some(decoded),
            "iss" => issuer = Some(decoded),
            "error" => error = Some(decoded),
            "error_description" => error_description = Some(decoded),
            _ => {}
        }
    }
    if let (Some(code), Some(state)) = (code, state) {
        Some(OAuthCallbackQuery::Code {
            code,
            state,
            issuer,
        })
    } else if let Some(error) = error {
        Some(OAuthCallbackQuery::Error {
            error,
            error_description,
        })
    } else {
        None
    }
}

// ==================== 发起 OAuth 授权流程 ====================

async fn mcp_oauth_start_login(
    state: &AppState,
    server_id: &str,
) -> Result<McpOAuthStatusResult, String> {
    let server = load_server_by_id(state, server_id)?;
    let members = parse_mcp_group_definitions(&server)?;

    // 寻找第一个 streamable_http 成员的 URL
    let streamable_member = members
        .into_iter()
        .find(|(_, _, def)| def.transport == McpTransportKind::StreamableHttp)
        .ok_or_else(|| "此 MCP 服务不是 streamable_http 协议，不支持 OAuth 登录".to_string())?;

    let server_url = streamable_member
        .2
        .url
        .ok_or_else(|| "MCP 服务定义中缺少 url".to_string())?;

    let cred_path = mcp_oauth_credential_path(state, server_id)?;

    // 绑定 127.0.0.1:0 随机端口
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| format!("绑定本地 OAuth 回调端口失败: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("获取本地回调端口失败: {err}"))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let store = McpFileCredentialStore::new(cred_path);
    let mut auth_manager = AuthorizationManager::new(&server_url)
        .await
        .map_err(|err| format!("初始化 MCP OAuth 管理器失败: {err}"))?;
    auth_manager.set_credential_store(store);

    let mut request = AuthorizationRequest::new(&redirect_uri).with_client_name("PAI");
    let challenge = match mcp_oauth_get_cached_challenge(server_id) {
        Some(c) if !c.is_empty() => Some(c),
        _ => {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .ok();
            if let Some(client) = client {
                if let Ok(resp) = client.get(&server_url).send().await {
                    if resp.status().as_u16() == 401 {
                        if let Some(val) = resp.headers().get("www-authenticate") {
                            if let Ok(s) = val.to_str() {
                                mcp_oauth_set_cached_challenge(server_id, s);
                                Some(s.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    };
    if let Some(c) = challenge {
        request.challenge = Some(c);
    }

    let mut oauth_state = OAuthState::Unauthorized(auth_manager);
    oauth_state
        .start_authorization(request)
        .await
        .map_err(|err| format!("MCP OAuth 启动授权失败: {err}"))?;

    let auth_url = oauth_state
        .get_authorization_url()
        .await
        .map_err(|err| format!("获取 MCP OAuth 授权 URL 失败: {err}"))?;

    let cancel_flag = Arc::new(AtomicBool::new(false));

    {
        let mut guard = mcp_oauth_sessions()
            .lock()
            .map_err(|_| "获取会话锁失败".to_string())?;
        // 若已有旧会话，通知其取消
        if let Some(old) = guard.get(server_id) {
            old.cancel_flag.store(true, Ordering::SeqCst);
        }
        guard.insert(
            server_id.to_string(),
            ActiveOAuthSession {
                _server_id: server_id.to_string(),
                status: "authorizing".to_string(),
                message: "浏览器已打开，等待授权完成...".to_string(),
                auth_url: auth_url.clone(),
                cancel_flag: cancel_flag.clone(),
            },
        );
    }

    // 打开系统浏览器
    if let Err(err) = webbrowser::open(&auth_url) {
        runtime_log_warn(format!(
            "[MCP OAuth] 尝试用 webbrowser 打开授权页失败: {}，请手动在浏览器中访问: {}",
            err, auth_url
        ));
    }

    runtime_log_info(format!(
        "[MCP OAuth] 开始授权 server_id={} redirect_uri={}",
        server_id, redirect_uri
    ));

    // 启动后台监听处理回调
    let app_state_clone = state.clone();
    let server_id_owned = server_id.to_string();
    tauri::async_runtime::spawn(async move {
        mcp_oauth_run_callback_listener(
            app_state_clone,
            server_id_owned,
            listener,
            oauth_state,
            cancel_flag,
        )
        .await;
    });

    Ok(McpOAuthStatusResult {
        server_id: server_id.to_string(),
        status: "authorizing".to_string(),
        message: "浏览器已打开，请在网页中完成授权。".to_string(),
        auth_url,
        has_credentials: false,
    })
}

async fn mcp_oauth_run_callback_listener(
    state: AppState,
    server_id: String,
    listener: TcpListener,
    mut oauth_state: OAuthState,
    cancel_flag: Arc<AtomicBool>,
) {
    let started = Instant::now();
    let timeout = Duration::from_secs(MCP_OAUTH_TIMEOUT_SECS);

    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            runtime_log_info(format!("[MCP OAuth] 用户取消授权 server_id={}", server_id));
            return;
        }

        if started.elapsed() > timeout {
            let mut guard = match mcp_oauth_sessions().lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            if let Some(session) = guard.get_mut(&server_id) {
                session.status = "expired".to_string();
                session.message = "OAuth 授权超时，请重试。".to_string();
            }
            runtime_log_warn(format!("[MCP OAuth] 授权超时 server_id={}", server_id));
            return;
        }

        let accept_result = tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(300)) => {
                continue;
            }
            res = listener.accept() => res,
        };

        match accept_result {
            Ok((mut stream, _addr)) => {
                let mut buffer = [0u8; 4096];
                let n = match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer)).await {
                    Ok(Ok(n)) => n,
                    _ => 0,
                };
                let request_text = String::from_utf8_lossy(&buffer[..n]);
                let first_line = request_text.lines().next().unwrap_or("");

                let query = match parse_oauth_callback_query(first_line) {
                    Some(query) => query,
                    None => {
                        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
                        continue;
                    }
                };

                // 授权服务器返回 error 参数（如 access_denied）：告知用户并结束本次授权，
                // 不再继续监听，否则浏览器只看到 404、会话空转到超时。
                let (code, state_param, issuer) = match query {
                    OAuthCallbackQuery::Code { code, state, issuer } => (code, state, issuer),
                    OAuthCallbackQuery::Error {
                        error,
                        error_description,
                    } => {
                        let response_body = MCP_OAUTH_HTML_ERROR;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.flush().await;

                        let detail = match error_description.as_deref() {
                            Some(desc) if !desc.is_empty() => format!("{error}: {desc}"),
                            _ => error.clone(),
                        };
                        {
                            let mut guard = match mcp_oauth_sessions().lock() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            if let Some(session) = guard.get_mut(&server_id) {
                                session.status = "error".to_string();
                                session.message = format!("OAuth 授权被拒绝: {detail}");
                            }
                        }

                        runtime_log_warn(format!(
                            "[MCP OAuth] 授权回调返回错误 server_id={} error={}",
                            server_id, detail
                        ));
                        return;
                    }
                };

                match oauth_state
                    .handle_callback_with_issuer(&code, &state_param, issuer.as_deref())
                    .await
                {
                    Ok(_) => {
                        let response_body = MCP_OAUTH_HTML_SUCCESS;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.flush().await;

                        // 标记策略 oauth_capable
                        let _ = set_workspace_mcp_policy_oauth_capable(&state, &server_id, true);
                        mcp_oauth_clear_cached_challenge(&server_id);

                        // 更新会话状态
                        {
                            let mut guard = match mcp_oauth_sessions().lock() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            if let Some(session) = guard.get_mut(&server_id) {
                                session.status = "success".to_string();
                                session.message = "授权成功！".to_string();
                            }
                        }

                        runtime_log_info(format!(
                            "[MCP OAuth] 完成授权 server_id={} duration_ms={}",
                            server_id,
                            started.elapsed().as_millis()
                        ));

                        // 授权成功后自动重新探测部署该 server
                        if let Ok(server) = load_server_by_id(&state, &server_id) {
                            mcp_start_supervisor_probe_for_server(state, server, "oauth_success");
                        }
                        return;
                    }
                    Err(err) => {
                        let response_body = MCP_OAUTH_HTML_ERROR;
                        let response = format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.flush().await;

                        {
                            let mut guard = match mcp_oauth_sessions().lock() {
                                Ok(g) => g,
                                Err(p) => p.into_inner(),
                            };
                            if let Some(session) = guard.get_mut(&server_id) {
                                session.status = "error".to_string();
                                session.message = format!("OAuth 换取凭据失败: {err}");
                            }
                        }

                        runtime_log_error(format!(
                            "[MCP OAuth] 失败 换取 token 错误 server_id={} error={}",
                            server_id, err
                        ));
                        return;
                    }
                }
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[MCP OAuth] listener.accept 发生异常 server_id={} error={}",
                    server_id, err
                ));
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}
