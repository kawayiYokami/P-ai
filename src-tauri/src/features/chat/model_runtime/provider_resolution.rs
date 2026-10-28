#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelProtocolResolutionSource {
    Explicit,
    BaseUrl,
    Model,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderAuthScheme {
    Bearer,
    ApiKey,
    GoogleApiKey,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedModelProtocol {
    adapter_kind: genai::adapter::AdapterKind,
    auth_scheme: ProviderAuthScheme,
    source: ModelProtocolResolutionSource,
}

fn provider_auth_scheme_for_adapter(
    adapter_kind: genai::adapter::AdapterKind,
) -> ProviderAuthScheme {
    match adapter_kind {
        genai::adapter::AdapterKind::Gemini => ProviderAuthScheme::GoogleApiKey,
        genai::adapter::AdapterKind::Anthropic | genai::adapter::AdapterKind::MiniMax => {
            ProviderAuthScheme::ApiKey
        }
        genai::adapter::AdapterKind::Ollama => ProviderAuthScheme::None,
        _ => ProviderAuthScheme::Bearer,
    }
}

fn apply_provider_auth_scheme(
    request_builder: reqwest::RequestBuilder,
    auth_scheme: ProviderAuthScheme,
    api_key: &str,
) -> Result<reqwest::RequestBuilder, String> {
    let api_key = api_key.trim();
    match auth_scheme {
        ProviderAuthScheme::Bearer => Ok(request_builder.bearer_auth(api_key)),
        ProviderAuthScheme::ApiKey => {
            let value = reqwest::header::HeaderValue::from_str(api_key)
                .map_err(|err| format!("构建 x-api-key 请求头失败: {err}"))?;
            Ok(request_builder.header("x-api-key", value))
        }
        ProviderAuthScheme::GoogleApiKey => {
            let value = reqwest::header::HeaderValue::from_str(api_key)
                .map_err(|err| format!("构建 x-goog-api-key 请求头失败: {err}"))?;
            Ok(request_builder.header("x-goog-api-key", value))
        }
        ProviderAuthScheme::None => Ok(request_builder),
    }
}

fn resolve_adapter_kind_from_base_url(base_url: &str) -> Option<genai::adapter::AdapterKind> {
    let parsed = reqwest::Url::parse(base_url.trim()).ok()?;
    let host = parsed.host_str()?.trim().to_ascii_lowercase();
    let path = parsed
        .path()
        .trim()
        .trim_end_matches('/')
        .to_ascii_lowercase();

    let host_matches = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    let path_is_any = |candidates: &[&str]| candidates.contains(&path.as_str());
    let is_aliyun_host = host == "dashscope.aliyuncs.com"
        || host.ends_with(".dashscope.aliyuncs.com")
        || host == "dashscope-intl.aliyuncs.com"
        || host == "dashscope-us.aliyuncs.com"
        || host.ends_with(".maas.aliyuncs.com");
    let is_mimo_host = host == "api.xiaomimimo.com"
        || matches!(
            host.as_str(),
            "token-plan-cn.xiaomimimo.com"
                | "token-plan-sgp.xiaomimimo.com"
                | "token-plan-ams.xiaomimimo.com"
        );

    // 同一供应商可能同时提供 OpenAI 与 Anthropic 兼容端点，必须先看路径，
    // 否则供应商主域名会把 Anthropic 端点提前吞成默认的 OpenAI 兼容协议。
    if (is_aliyun_host && path_is_any(&["/apps/anthropic", "/apps/anthropic/v1"]))
        || (host == "api.deepseek.com" && path_is_any(&["/anthropic", "/anthropic/v1"]))
        || (host == "ark.cn-beijing.volces.com" && path == "/api/coding")
        || (is_mimo_host && path_is_any(&["/anthropic", "/anthropic/v1"]))
        || (host == "api.kimi.com" && path == "/coding")
    {
        return Some(genai::adapter::AdapterKind::Anthropic);
    }

    if host == "opencode.ai" && path_is_any(&["/zen/go", "/zen/go/v1"]) {
        return Some(genai::adapter::AdapterKind::OpenCodeGo);
    }
    if is_aliyun_host {
        return Some(genai::adapter::AdapterKind::Aliyun);
    }
    if is_mimo_host {
        return Some(genai::adapter::AdapterKind::Mimo);
    }
    if host == "api.kimi.com" && path == "/coding/v1" {
        return Some(genai::adapter::AdapterKind::Kimi);
    }
    if host == "api.openai.com" {
        return Some(genai::adapter::AdapterKind::OpenAI);
    }
    if host == "chatgpt.com" && path == "/backend-api/codex" {
        return Some(genai::adapter::AdapterKind::OpenAIResp);
    }
    if host_matches("generativelanguage.googleapis.com") || host_matches("aistudio.google.com") {
        return Some(genai::adapter::AdapterKind::Gemini);
    }
    if host == "api.anthropic.com" {
        return Some(genai::adapter::AdapterKind::Anthropic);
    }
    if host_matches("minimax.io") {
        return Some(genai::adapter::AdapterKind::MiniMax);
    }
    if host == "api.deepseek.com" {
        return Some(genai::adapter::AdapterKind::DeepSeek);
    }
    if host == "api.moonshot.cn" {
        return Some(genai::adapter::AdapterKind::Moonshot);
    }
    if host == "ark.cn-beijing.volces.com" {
        return Some(genai::adapter::AdapterKind::OpenAI);
    }
    if host_matches("openrouter.ai") {
        return Some(genai::adapter::AdapterKind::OpenRouter);
    }
    if host_matches("requesty.ai") {
        return Some(genai::adapter::AdapterKind::OpenAI);
    }
    if host_matches("groq.com") {
        return Some(genai::adapter::AdapterKind::Groq);
    }
    if host_matches("together.xyz") {
        return Some(genai::adapter::AdapterKind::Together);
    }
    if host_matches("fireworks.ai") {
        return Some(genai::adapter::AdapterKind::Fireworks);
    }
    if host_matches("nebius.ai") {
        return Some(genai::adapter::AdapterKind::Nebius);
    }
    if host_matches("bigmodel.cn") {
        return Some(genai::adapter::AdapterKind::BigModel);
    }
    if host_matches("qianfan.baidubce.com") {
        return Some(genai::adapter::AdapterKind::Baidu);
    }
    if host_matches("cohere.ai") {
        return Some(genai::adapter::AdapterKind::Cohere);
    }
    None
}

fn is_qwen_model_name(model_name: &str) -> bool {
    model_name.trim().to_ascii_lowercase().contains("qwen")
}

fn resolve_adapter_kind_from_model_name(
    model_name: &str,
) -> Option<genai::adapter::AdapterKind> {
    let normalized = model_name.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized.contains("::") {
        if let Ok(adapter_kind) = genai::adapter::AdapterKind::from_model(&normalized) {
            return Some(adapter_kind);
        }
    }
    if is_qwen_model_name(&normalized) {
        return Some(genai::adapter::AdapterKind::Aliyun);
    }
    match genai::adapter::AdapterKind::from_model(&normalized) {
        // genai 将所有未知模型兜底成 Ollama，这不是可用于动态协议判断的有效命中。
        // 显式 ollama:: 命名空间已在上方保留；无命名空间时交给调用方协议回退。
        Ok(genai::adapter::AdapterKind::Ollama) => None,
        Ok(adapter_kind) => Some(adapter_kind),
        Err(_) => None,
    }
}

fn resolve_model_adapter_for_auto(model_name: &str) -> genai::adapter::AdapterKind {
    resolve_adapter_kind_from_model_name(model_name)
        .unwrap_or(genai::adapter::AdapterKind::OpenAI)
}

fn resolve_model_protocol(
    request_format: RequestFormat,
    base_url: &str,
    model_name: &str,
    fallback_adapter_kind: genai::adapter::AdapterKind,
) -> ResolvedModelProtocol {
    let (adapter_kind, source) = if !request_format.is_auto() {
        (
            request_format
                .genai_adapter_kind()
                .unwrap_or(fallback_adapter_kind),
            ModelProtocolResolutionSource::Explicit,
        )
    } else if let Some(adapter_kind) = resolve_adapter_kind_from_base_url(base_url) {
        (adapter_kind, ModelProtocolResolutionSource::BaseUrl)
    } else if let Some(adapter_kind) = resolve_adapter_kind_from_model_name(model_name) {
        (adapter_kind, ModelProtocolResolutionSource::Model)
    } else {
        (fallback_adapter_kind, ModelProtocolResolutionSource::Fallback)
    };

    ResolvedModelProtocol {
        adapter_kind,
        auth_scheme: provider_auth_scheme_for_adapter(adapter_kind),
        source,
    }
}

#[cfg(test)]
mod provider_resolution_tests {
    use super::*;

    #[test]
    fn explicit_protocol_should_override_url_and_model() {
        let resolved = resolve_model_protocol(
            RequestFormat::OpenCodeGo,
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen3.7-plus",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::OpenCodeGo);
        assert_eq!(resolved.source, ModelProtocolResolutionSource::Explicit);
    }

    #[test]
    fn auto_protocol_should_resolve_url_before_model() {
        let resolved = resolve_model_protocol(
            RequestFormat::Auto,
            "https://opencode.ai/zen/go/v1",
            "qwen3.7-plus",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::OpenCodeGo);
        assert_eq!(resolved.source, ModelProtocolResolutionSource::BaseUrl);
        assert_eq!(resolved.auth_scheme, ProviderAuthScheme::Bearer);

        let non_qwen = resolve_model_protocol(
            RequestFormat::Auto,
            "https://opencode.ai/zen/go/v1",
            "mimo-v2.5-pro",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(non_qwen.adapter_kind, genai::adapter::AdapterKind::OpenCodeGo);
        assert_eq!(non_qwen.auth_scheme, ProviderAuthScheme::Bearer);
    }

    #[test]
    fn auto_protocol_should_resolve_aliyun_openai_and_anthropic_endpoint_families() {
        let openai_urls = [
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            "https://dashscope-us.aliyuncs.com/compatible-mode/v1",
            "https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            "https://trial.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            "https://coding.dashscope.aliyuncs.com/v1",
        ];
        for base_url in openai_urls {
            assert_eq!(
                resolve_adapter_kind_from_base_url(base_url),
                Some(genai::adapter::AdapterKind::Aliyun),
                "URL should use Aliyun OpenAI-compatible protocol: {base_url}"
            );
        }

        let anthropic_urls = [
            "https://dashscope.aliyuncs.com/apps/anthropic",
            "https://dashscope.aliyuncs.com/apps/anthropic/v1",
            "https://dashscope-intl.aliyuncs.com/apps/anthropic",
            "https://dashscope-us.aliyuncs.com/apps/anthropic",
            "https://workspace.cn-beijing.maas.aliyuncs.com/apps/anthropic",
            "https://trial.cn-beijing.maas.aliyuncs.com/apps/anthropic",
            "https://token-plan.cn-beijing.maas.aliyuncs.com/apps/anthropic",
            "https://coding.dashscope.aliyuncs.com/apps/anthropic/",
        ];
        for base_url in anthropic_urls {
            assert_eq!(
                resolve_adapter_kind_from_base_url(base_url),
                Some(genai::adapter::AdapterKind::Anthropic),
                "URL should use Anthropic protocol: {base_url}"
            );
        }
    }

    #[test]
    fn auto_protocol_should_resolve_multi_protocol_provider_urls_by_path_first() {
        let cases = [
            (
                "https://api.deepseek.com/v1",
                genai::adapter::AdapterKind::DeepSeek,
            ),
            (
                "https://api.deepseek.com/anthropic",
                genai::adapter::AdapterKind::Anthropic,
            ),
            (
                "https://api.deepseek.com/anthropic/v1",
                genai::adapter::AdapterKind::Anthropic,
            ),
            (
                "https://ark.cn-beijing.volces.com/api/v3",
                genai::adapter::AdapterKind::OpenAI,
            ),
            (
                "https://ark.cn-beijing.volces.com/api/coding/v3",
                genai::adapter::AdapterKind::OpenAI,
            ),
            (
                "https://ark.cn-beijing.volces.com/api/coding",
                genai::adapter::AdapterKind::Anthropic,
            ),
            (
                "https://api.xiaomimimo.com/v1",
                genai::adapter::AdapterKind::Mimo,
            ),
            (
                "https://api.xiaomimimo.com/anthropic",
                genai::adapter::AdapterKind::Anthropic,
            ),
            (
                "https://api.xiaomimimo.com/anthropic/v1",
                genai::adapter::AdapterKind::Anthropic,
            ),
            (
                "https://api.moonshot.cn/v1",
                genai::adapter::AdapterKind::Moonshot,
            ),
            (
                "https://api.kimi.com/coding/",
                genai::adapter::AdapterKind::Anthropic,
            ),
            (
                "https://api.kimi.com/coding/v1",
                genai::adapter::AdapterKind::Kimi,
            ),
        ];
        for (base_url, expected) in cases {
            assert_eq!(
                resolve_adapter_kind_from_base_url(base_url),
                Some(expected),
                "unexpected protocol for URL: {base_url}"
            );
        }

        for region in ["cn", "sgp", "ams"] {
            let host = format!("token-plan-{region}.xiaomimimo.com");
            assert_eq!(
                resolve_adapter_kind_from_base_url(&format!("https://{host}/v1")),
                Some(genai::adapter::AdapterKind::Mimo)
            );
            assert_eq!(
                resolve_adapter_kind_from_base_url(&format!("https://{host}/anthropic")),
                Some(genai::adapter::AdapterKind::Anthropic)
            );
        }

        assert_eq!(
            resolve_adapter_kind_from_base_url(
                "https://dashscope.aliyuncs.com/apps/anthropics"
            ),
            Some(genai::adapter::AdapterKind::Aliyun)
        );
    }

    #[test]
    fn auto_protocol_should_resolve_requesty_urls_as_openai_compatible() {
        for base_url in [
            "https://router.requesty.ai/v1",
            "https://router.eu.requesty.ai/v1",
        ] {
            let resolved = resolve_model_protocol(
                RequestFormat::Auto,
                base_url,
                "claude-sonnet-4-5",
                genai::adapter::AdapterKind::OpenAI,
            );
            assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::OpenAI);
            assert_eq!(resolved.source, ModelProtocolResolutionSource::BaseUrl);
            assert_eq!(resolved.auth_scheme, ProviderAuthScheme::Bearer);
        }
    }

    #[test]
    fn auto_protocol_should_resolve_qwen_from_model_for_unknown_url() {
        let resolved = resolve_model_protocol(
            RequestFormat::Auto,
            "https://example.com/v1",
            "Qwen3.7-Plus",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::Aliyun);
        assert_eq!(resolved.source, ModelProtocolResolutionSource::Model);
    }

    #[test]
    fn auto_protocol_should_keep_genai_namespace_resolution() {
        let resolved = resolve_model_protocol(
            RequestFormat::Auto,
            "https://example.com/v1",
            "opencode_go::some-model",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::OpenCodeGo);
        assert_eq!(resolved.source, ModelProtocolResolutionSource::Model);
    }

    #[test]
    fn explicit_model_namespace_should_override_qwen_name_fallback() {
        let resolved = resolve_model_protocol(
            RequestFormat::Auto,
            "https://example.com/v1",
            "opencode_go::qwen3.7-plus",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::OpenCodeGo);
    }

    #[test]
    fn unknown_model_should_not_treat_genai_ollama_fallback_as_detection() {
        let resolved = resolve_model_protocol(
            RequestFormat::Auto,
            "https://example.com/v1",
            "provider-new-model",
            genai::adapter::AdapterKind::OpenAI,
        );
        assert_eq!(resolved.adapter_kind, genai::adapter::AdapterKind::OpenAI);
        assert_eq!(resolved.source, ModelProtocolResolutionSource::Fallback);

        assert_eq!(
            resolve_adapter_kind_from_model_name("ollama::qwen3:8b"),
            Some(genai::adapter::AdapterKind::Ollama)
        );
    }

    #[test]
    fn mimo_and_opencode_go_should_use_bearer_auth() {
        assert_eq!(
            provider_auth_scheme_for_adapter(genai::adapter::AdapterKind::Mimo),
            ProviderAuthScheme::Bearer
        );
        assert_eq!(
            provider_auth_scheme_for_adapter(genai::adapter::AdapterKind::OpenCodeGo),
            ProviderAuthScheme::Bearer
        );
    }

    #[test]
    fn bearer_auth_should_not_emit_model_specific_api_key_header() {
        let request = apply_provider_auth_scheme(
            reqwest::Client::new().post("https://example.com/v1/chat/completions"),
            provider_auth_scheme_for_adapter(genai::adapter::AdapterKind::Mimo),
            "secret-key",
        )
        .expect("apply bearer auth")
        .build()
        .expect("build request");
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer secret-key")
        );
        assert!(request.headers().get("api-key").is_none());
    }
}
