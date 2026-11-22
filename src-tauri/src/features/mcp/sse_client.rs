// ========== MCP over SSE（legacy HTTP+SSE transport） ==========
//
// 协议流程（知乎等 SSE MCP 服务）：
// 1. GET sse_url（携带鉴权头），服务端通过 `endpoint` 事件返回 message 地址
// 2. 后续 JSON-RPC（initialize / tools/list / tools/call）POST 到 message 地址
// 3. message 端点通常返回 202 Accepted，实际响应经已建立的 SSE 通道异步返回

use std::task::{Context, Poll};

use futures_util::{Sink, Stream};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};

const SSE_ENDPOINT_WAIT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
struct SseClientError(String);

impl std::fmt::Display for SseClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SseClientError {}

fn sse_client_error(text: impl Into<String>) -> SseClientError {
    SseClientError(text.into())
}

fn build_sse_http_headers(parsed: &ParsedMcpServerDefinition) -> Result<reqwest::header::HeaderMap, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    for (k, v) in &parsed.http_headers {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|err| format!("Invalid MCP http header name '{k}': {err}"))?;
        let value = reqwest::header::HeaderValue::from_str(v)
            .map_err(|err| format!("Invalid MCP http header value for '{k}': {err}"))?;
        headers.insert(name, value);
    }
    for (k, env_var) in &parsed.env_http_headers {
        let env_name = env_var.trim();
        if env_name.is_empty() {
            continue;
        }
        if let Ok(value_text) = std::env::var(env_name) {
            let value_text = value_text.trim();
            if value_text.is_empty() {
                continue;
            }
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                .map_err(|err| format!("Invalid MCP env_http_headers name '{k}': {err}"))?;
            let value = reqwest::header::HeaderValue::from_str(value_text).map_err(|err| {
                format!("Invalid MCP env_http_headers value for '{k}': {err}")
            })?;
            headers.insert(name, value);
        }
    }
    if let Some(token_env) = &parsed.bearer_token_env_var {
        let env_name = token_env.trim();
        if !env_name.is_empty() {
            if let Ok(token_value) = std::env::var(env_name) {
                let token = token_value.trim();
                if !token.is_empty() {
                    if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
                        headers.insert(reqwest::header::AUTHORIZATION, value);
                    }
                }
            }
        }
    }
    Ok(headers)
}

/// 将 endpoint 事件返回的 message 地址解析为绝对 URL
fn resolve_message_url(sse_url: &str, endpoint: &str) -> Result<String, String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err("endpoint 事件返回空 message 地址".to_string());
    }
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return Ok(endpoint.to_string());
    }
    let base = reqwest::Url::parse(sse_url)
        .map_err(|err| format!("解析 SSE url 失败: {err}"))?;
    let resolved = base
        .join(endpoint)
        .map_err(|err| format!("拼接 message url 失败: {err}"))?;
    Ok(resolved.to_string())
}

/// Sink 侧：JSON-RPC 消息经后台任务 POST 到 message 地址
struct SsePostSink {
    tx: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
}

impl Sink<ClientJsonRpcMessage> for SsePostSink {
    type Error = SseClientError;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: ClientJsonRpcMessage) -> Result<(), Self::Error> {
        let value = serde_json::to_value(&item)
            .map_err(|err| sse_client_error(format!("序列化 MCP JSON-RPC 消息失败: {err}")))?;
        let _ = self.tx.send(value);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Stream 侧：从 SSE 通道读取 `message` 事件并解析为 JSON-RPC 响应
struct SseMessageStream {
    inner: Pin<
        Box<dyn Stream<Item = Result<sse_stream::Sse, sse_stream::Error>> + Send>,
    >,
}

impl Stream for SseMessageStream {
    type Item = ServerJsonRpcMessage;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    if event.event.as_deref() == Some("message") {
                        let Some(data) = event.data.as_deref() else {
                            continue;
                        };
                        match serde_json::from_str::<ServerJsonRpcMessage>(data) {
                            Ok(msg) => return Poll::Ready(Some(msg)),
                            Err(err) => {
                                runtime_log_warn(format!(
                                    "[工具] MCP 流式 message 事件 JSON-RPC 解析失败: {err}"
                                ));
                                continue;
                            }
                        }
                    }
                    // endpoint / ping / 其他事件忽略
                }
                Poll::Ready(Some(Err(err))) => {
                    runtime_log_warn(format!("[工具] MCP 流式 SSE 流错误: {err}"));
                    continue;
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// 连接 SSE 端点并等待 endpoint 事件，返回 (sink, stream) 供 rmcp serve
async fn connect_sse_transport(
    parsed: &ParsedMcpServerDefinition,
) -> Result<(SsePostSink, SseMessageStream), String> {
    let sse_url = parsed
        .url
        .as_deref()
        .ok_or_else(|| "SSE MCP url is missing".to_string())?;
    let headers = build_sse_http_headers(parsed)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(MCP_REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|err| format!("Build MCP SSE HTTP client failed: {err}"))?;

    let response = client
        .get(sse_url)
        .header(
            reqwest::header::ACCEPT,
            rmcp::transport::common::http_header::EVENT_STREAM_MIME_TYPE,
        )
        .headers(headers.clone())
        .send()
        .await
        .map_err(|err| format!("Connect MCP SSE endpoint failed: {err}"))?;
    let response = response
        .error_for_status()
        .map_err(|err| format!("MCP SSE endpoint 响应异常: {err}"))?;

    let sse_stream = sse_stream::SseStream::from_bytes_stream(response.bytes_stream());
    let mut stream = Box::pin(sse_stream);

    // 等待 endpoint 事件获取 message 地址
    let endpoint_future = async {
        while let Some(event) = stream.next().await {
            match event {
                Ok(ev) => {
                    if ev.event.as_deref() == Some("endpoint") {
                        return ev
                            .data
                            .ok_or_else(|| "endpoint 事件缺少 data".to_string());
                    }
                }
                Err(err) => return Err(format!("读取 SSE endpoint 事件失败: {err}")),
            }
        }
        Err("SSE 流提前结束，未收到 endpoint 事件".to_string())
    };
    let endpoint = tokio::time::timeout(
        std::time::Duration::from_secs(SSE_ENDPOINT_WAIT_TIMEOUT_SECS),
        endpoint_future,
    )
    .await
    .map_err(|_| "等待 SSE endpoint 事件超时".to_string())??;
    let message_url = resolve_message_url(sse_url, &endpoint)?;

    runtime_log_info(format!(
        "[工具] MCP 流式 连接成功 流式地址={sse_url} 消息地址={message_url}"
    ));

    // 后台 POST 任务：SSE 的 message 地址只接受 POST，响应走 SSE 通道
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
    let post_client = client.clone();
    let post_url = message_url.clone();
    let post_headers = headers.clone();
    tokio::spawn(async move {
        while let Some(value) = rx.recv().await {
            let body = match serde_json::to_vec(&value) {
                Ok(body) => body,
                Err(err) => {
                    runtime_log_warn(format!("[工具] MCP 流式 序列化 POST 消息失败: {err}"));
                    continue;
                }
            };
            match post_client
                .post(&post_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .headers(post_headers.clone())
                .body(body)
                .send()
                .await
            {
                Ok(resp) => {
                    // 202 Accepted 属正常（响应经 SSE 通道异步返回）
                    if !resp.status().is_success() {
                        runtime_log_warn(format!(
                            "[工具] MCP 流式 message POST 返回状态码 {}",
                            resp.status()
                        ));
                    }
                }
                Err(err) => {
                    runtime_log_error(format!("[工具] MCP 流式 message POST 失败: {err}"));
                }
            }
        }
    });

    Ok((
        SsePostSink { tx },
        SseMessageStream { inner: stream },
    ))
}
