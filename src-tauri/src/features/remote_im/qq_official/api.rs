// ==================== QQ 官方机器人：协议请求 ====================

fn qq_official_build_http_client(
    credentials: &QqOfficialCredentials,
) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(
            credentials.normalized_api_timeout_ms(),
        ))
        .build()
        .map_err(|err| format!("构建 QQ 官方机器人 HTTP 客户端失败: {err}"))
}

/// 请求调用凭证：`POST https://bots.qq.com/app/getAppAccessToken`。
/// 返回 `(access_token, 有效期秒数)`。
async fn qq_official_request_access_token(
    credentials: &QqOfficialCredentials,
) -> Result<(String, i64), String> {
    let app_id = credentials.normalized_app_id();
    let app_secret = credentials.normalized_app_secret();
    if app_id.is_empty() || app_secret.is_empty() {
        return Err("QQ 官方机器人渠道缺少 appId/appSecret".to_string());
    }
    let client = qq_official_build_http_client(credentials)?;
    let response = client
        .post(QQ_OFFICIAL_TOKEN_URL)
        .json(&serde_json::json!({
            "appId": app_id,
            "clientSecret": app_secret,
        }))
        .send()
        .await
        .map_err(|err| format!("请求 QQ 调用凭证失败: {err}"))?;
    let status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("解析 QQ 调用凭证响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!("请求 QQ 调用凭证失败: HTTP {status}, body={body}"));
    }
    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("QQ 调用凭证响应缺少 access_token: {body}"))?
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(qq_official_json_i64)
        .filter(|value| *value > 0)
        .unwrap_or(QQ_OFFICIAL_DEFAULT_TOKEN_TTL_SECS);
    Ok((access_token, expires_in))
}

/// 获取通用 WSS 接入点：`GET {base}/gateway`。
async fn qq_official_request_gateway_url(
    credentials: &QqOfficialCredentials,
    access_token: &str,
) -> Result<String, String> {
    let client = qq_official_build_http_client(credentials)?;
    let url = format!("{}/gateway", credentials.api_base_url());
    let response = client
        .get(url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("QQBot {access_token}"),
        )
        .send()
        .await
        .map_err(|err| format!("请求 QQ 网关地址失败: {err}"))?;
    let status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|err| format!("解析 QQ 网关地址响应失败: {err}"))?;
    if !status.is_success() {
        return Err(format!("请求 QQ 网关地址失败: HTTP {status}, body={body}"));
    }
    body.get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("QQ 网关地址响应缺少 url: {body}"))
}
