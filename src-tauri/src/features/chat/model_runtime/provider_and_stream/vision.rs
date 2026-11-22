async fn describe_image_with_vision_api(
    state: &AppState,
    vision_resolved: &ResolvedApiConfig,
    vision_api: &ApiConfig,
    prepared: PreparedPrompt,
) -> Result<String, String> {
    let supports_non_stream_fallback =
        request_format_supports_non_stream_fallback(vision_resolved.request_format);
    let prefer_non_stream = supports_non_stream_fallback
        && provider_streaming_disabled(
            Some(state),
            vision_resolved.request_format,
            &vision_resolved.base_url,
            &vision_api.model,
        );
    let reply = if vision_resolved.request_format.is_genai_chat() || vision_resolved.request_format.is_auto() {
        if prefer_non_stream {
            call_model_genai_non_stream(vision_resolved, &vision_api.model, prepared, Some(state), None, 1).await?
        } else {
            match call_model_genai_stream(vision_resolved, &vision_api.model, prepared.clone(), Some(state), None, 1).await {
                Ok(reply) => {
                    if let Err(clear_err) = provider_clear_streaming_disabled(
                        Some(state),
                        vision_resolved.request_format,
                        &vision_resolved.base_url,
                        &vision_api.model,
                    ) {
                        runtime_log_warn(format!(
                            "[聊天] 清理流式降级缓存失败 地址={} 模型={} 原因={}",
                            vision_resolved.base_url, vision_api.model, clear_err
                        ));
                    }
                    reply
                }
                Err(err)
                    if supports_non_stream_fallback
                        && is_streaming_request_payload_format_error(&err) =>
                {
                    if let Err(mark_err) = provider_mark_streaming_disabled(
                        Some(state),
                        vision_resolved.request_format,
                        &vision_resolved.base_url,
                        &vision_api.model,
                    ) {
                        runtime_log_warn(format!(
                            "[聊天] 标记本次运行内非流式 base_url 失败 地址={} 原因={}",
                            vision_resolved.base_url, mark_err
                        ));
                    }
                    runtime_log_warn(format!(
                        "[聊天] 流式失败，已在本次运行内切换非流式重试 地址={} 模型={} 原因={}",
                        vision_resolved.base_url, vision_api.model, err
                    ));
                    call_model_genai_non_stream(vision_resolved, &vision_api.model, prepared, Some(state), None, 1).await?
                }
                Err(err) => return Err(err),
            }
        }
    } else {
        return Err(format!(
            "Vision request format '{}' is not supported.",
            vision_resolved.request_format
        ));
    };
    Ok(reply.assistant_text)
}
