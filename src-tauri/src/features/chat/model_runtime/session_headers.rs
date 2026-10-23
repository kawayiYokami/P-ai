// ==================== 供应商会话头注入 ====================
// 各供应商把会话标识放在自己的专属请求头里，命名与层级都不一样。这里统一收口：
// 输入会话标识、上游地址、请求协议，返回需要注入的头列表。
// 纯函数：不读全局状态、不发请求、不依赖调用顺序。

/// 无会话上下文时（标题生成、摘要等）使用的稳定标识，保证同一进程内的请求可归组。
const PROVIDER_SESSION_FALLBACK_ID: &str = "pai-standalone";

/// 通用会话头：所有对话协议都带，用于上游按会话归并请求。
const GENERIC_SESSION_HEADER: &str = "x-pai-session";

/// 计算注入给某次请求的会话头。
pub(crate) fn provider_session_headers(
    session_id: Option<&str>,
    base_url: &str,
    request_format: RequestFormat,
) -> Vec<(String, String)> {
    let key = session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(PROVIDER_SESSION_FALLBACK_ID)
        .to_string();

    let host = provider_session_host(base_url);
    let mut headers = Vec::new();

    // 常用供应商：协议就能判定
    match request_format {
        // Codex 用 session-id 标记进程会话，thread-id 标记对话线程，request-id 复用线程标识
        RequestFormat::Codex => {
            headers.push(("session-id".to_string(), key.clone()));
            headers.push(("thread-id".to_string(), key.clone()));
            headers.push(("x-client-request-id".to_string(), key.clone()));
        }
        RequestFormat::OpenCodeGo => {
            headers.push(("x-opencode-session".to_string(), key.clone()));
        }
        _ => {}
    }

    // 地址才能判定的供应商：同协议被大量兼容端点复用，只能按宿主区分，
    // 否则会把专属头泼到第三方网关，严格代理会直接拒绝未知头。
    if is_opencode_host(&host) && request_format != RequestFormat::OpenCodeGo {
        headers.push(("x-opencode-session".to_string(), key.clone()));
    }
    if is_claude_code_host(&host) {
        headers.push(("x-claude-code-session-id".to_string(), key.clone()));
    }
    if is_grok_host(&host) {
        headers.push(("x-grok-conv-id".to_string(), key.clone()));
        headers.push(("x-grok-session-id".to_string(), key.clone()));
    }

    // 通用标识放最后，专属头优先，二者取值一致，抓包对照时无需换算
    headers.push((GENERIC_SESSION_HEADER.to_string(), key));

    headers
}

/// 提取用于判定的宿主；地址不合法时返回空串。
fn provider_session_host(base_url: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(base_url.trim()) else {
        return String::new();
    };
    parsed
        .host_str()
        .map(|host| host.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_opencode_host(host: &str) -> bool {
    host == "opencode.ai" || host.ends_with(".opencode.ai")
}

fn is_claude_code_host(host: &str) -> bool {
    host == "api.anthropic.com" || host == "claude.ai" || host.ends_with(".anthropic.com")
}

fn is_grok_host(host: &str) -> bool {
    host == "x.ai"
        || host.ends_with(".x.ai")
        || host == "grok.com"
        || host.ends_with(".grok.com")
        || host.contains("cli-chat-proxy")
}

#[cfg(test)]
mod provider_session_header_tests {
    use super::*;

    fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn codex_gets_session_thread_and_request_id() {
        let headers = provider_session_headers(
            Some("agent-a::conv-1"),
            "https://chatgpt.com/backend-api/codex",
            RequestFormat::Codex,
        );

        assert_eq!(
            header_value(&headers, "session-id").as_deref(),
            Some("agent-a::conv-1")
        );
        assert_eq!(
            header_value(&headers, "thread-id").as_deref(),
            Some("agent-a::conv-1")
        );
        assert_eq!(
            header_value(&headers, "x-client-request-id").as_deref(),
            Some("agent-a::conv-1")
        );
        assert_eq!(
            header_value(&headers, "x-pai-session").as_deref(),
            Some("agent-a::conv-1")
        );
    }

    #[test]
    fn opencode_gets_its_own_session_header() {
        let headers = provider_session_headers(
            Some("conv-9"),
            "https://opencode.ai/zen/go/v1",
            RequestFormat::OpenCodeGo,
        );

        assert_eq!(header_value(&headers, "x-opencode-session").as_deref(), Some("conv-9"));
        assert_eq!(header_value(&headers, "x-pai-session").as_deref(), Some("conv-9"));
    }

    #[test]
    fn third_party_anthropic_endpoint_only_gets_generic_header() {
        let headers = provider_session_headers(
            Some("conv-2"),
            "https://dashscope.aliyuncs.com/apps/anthropic",
            RequestFormat::Anthropic,
        );

        assert!(header_value(&headers, "x-claude-code-session-id").is_none());
        assert_eq!(header_value(&headers, "x-pai-session").as_deref(), Some("conv-2"));
    }

    #[test]
    fn first_party_anthropic_gets_claude_code_header() {
        let headers = provider_session_headers(
            Some("conv-3"),
            "https://api.anthropic.com/v1",
            RequestFormat::Anthropic,
        );

        assert_eq!(
            header_value(&headers, "x-claude-code-session-id").as_deref(),
            Some("conv-3")
        );
    }

    #[test]
    fn grok_endpoint_gets_conv_and_session_headers() {
        let headers = provider_session_headers(
            Some("conv-4"),
            "https://api.x.ai/v1",
            RequestFormat::OpenAI,
        );

        assert_eq!(header_value(&headers, "x-grok-conv-id").as_deref(), Some("conv-4"));
        assert_eq!(header_value(&headers, "x-grok-session-id").as_deref(), Some("conv-4"));
    }

    #[test]
    fn missing_session_falls_back_to_stable_id() {
        let headers = provider_session_headers(None, "https://api.openai.com/v1", RequestFormat::OpenAI);
        assert_eq!(
            header_value(&headers, "x-pai-session").as_deref(),
            Some(PROVIDER_SESSION_FALLBACK_ID)
        );

        let blank = provider_session_headers(Some("   "), "https://api.openai.com/v1", RequestFormat::OpenAI);
        assert_eq!(
            header_value(&blank, "x-pai-session").as_deref(),
            Some(PROVIDER_SESSION_FALLBACK_ID)
        );
    }
}
