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

pub(crate) async fn weixin_oc_get_typing_config(
    credentials: &WeixinOcCredentials,
    ilink_user_id: &str,
    context_token: Option<&str>,
) -> Result<String, String> {
    let client = build_weixin_oc_http_client(credentials.normalized_api_timeout_ms())?;
    let body = serde_json::json!({
        "ilink_user_id": ilink_user_id,
        "context_token": context_token.map(str::trim).filter(|value| !value.is_empty()),
        "base_info": {
            "channel_version": "easy_call_ai"
        }
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 getconfig 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(credentials.token.as_str()))?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/getconfig",
            credentials.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 getconfig 失败: {err}"))?;
    let status_code = resp.status();
    if !status_code.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "请求 getconfig 结果=失败, 状态码={}, 响应={}",
            status_code, body
        ));
    }
    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|err| format!("解析 getconfig 响应失败: {err}"))?;
    let ret = resp_body
        .get("ret")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    if ret != 0 {
        let errmsg = resp_body
            .get("errmsg")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        return Err(format!(
            "请求 getconfig 结果=失败, ret={}, errmsg={}",
            ret, errmsg
        ));
    }
    let ticket = resp_body
        .get("typing_ticket")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            format!(
                "请求 getconfig 结果=失败, 响应中缺少 typing_ticket 字段: resp={}",
                resp_body
            )
        })?;
    Ok(ticket)
}

/// status: 1 = 开始输入, 2 = 停止输入
pub(crate) async fn weixin_oc_send_typing(
    credentials: &WeixinOcCredentials,
    ilink_user_id: &str,
    typing_ticket: &str,
    status: i64,
) -> Result<(), String> {
    let client = build_weixin_oc_http_client(credentials.normalized_api_timeout_ms())?;
    let body = serde_json::json!({
        "ilink_user_id": ilink_user_id,
        "typing_ticket": typing_ticket,
        "status": status,
        "base_info": {
            "channel_version": "easy_call_ai"
        }
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("序列化 sendtyping 请求失败: {err}"))?;
    let headers = weixin_oc_request_headers(&body_text, Some(credentials.token.as_str()))?;
    let resp = client
        .post(format!(
            "{}/ilink/bot/sendtyping",
            credentials.normalized_base_url().trim_end_matches('/')
        ))
        .headers(headers)
        .body(body_text)
        .send()
        .await
        .map_err(|err| format!("请求 sendtyping 失败: {err}"))?;
    let status_code = resp.status();
    if !status_code.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "请求 sendtyping 结果=失败, 状态码={}, 响应={}",
            status_code, body
        ));
    }
    let resp_body: serde_json::Value = resp
        .json()
        .await
        .map_err(|err| format!("解析 sendtyping 响应失败: {err}"))?;
    let ret = resp_body
        .get("ret")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-1);
    if ret != 0 {
        let errmsg = resp_body
            .get("errmsg")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        return Err(format!(
            "请求 sendtyping 结果=失败, ret={}, errmsg={}",
            ret, errmsg
        ));
    }
    Ok(())
}
