async fn describe_image_with_vision_api(
    state: &AppState,
    vision_resolved: &ResolvedApiConfig,
    vision_api: &ApiConfig,
    image: &BinaryPart,
) -> Result<String, String> {
    let prepared = conversation_prompt_service().build_vision_description_prepared_prompt(image);

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
            call_model_openai_non_stream(vision_resolved, &vision_api.model, prepared, Some(state)).await?
        } else {
            match call_model_openai_stream(vision_resolved, &vision_api.model, prepared.clone(), Some(state)).await {
                Ok(reply) => {
                    if let Err(clear_err) = provider_clear_streaming_disabled(
                        Some(state),
                        vision_resolved.request_format,
                        &vision_resolved.base_url,
                        &vision_api.model,
                    ) {
                        runtime_log_warn(format!(
                            "[视觉] 清理流式降级缓存失败: base_url={}, model={}, err={}",
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
                            "[视觉] 标记本次运行内非流式 base_url 失败: base_url={}, err={}",
                            vision_resolved.base_url, mark_err
                        ));
                    }
                    runtime_log_info(format!(
                        "[视觉] 流式失败，已在本次运行内切换非流式重试: base_url={}, model={}, err={}",
                        vision_resolved.base_url, vision_api.model, err
                    ));
                    call_model_openai_non_stream(vision_resolved, &vision_api.model, prepared, Some(state)).await?
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
