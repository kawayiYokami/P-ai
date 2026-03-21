async fn describe_image_with_vision_api(
    state: &AppState,
    vision_resolved: &ResolvedApiConfig,
    vision_api: &ApiConfig,
    image: &BinaryPart,
) -> Result<String, String> {
    let mime = image.mime.trim();
    let prepared = PreparedPrompt {
        preamble: "[SYSTEM PROMPT]\n你是图像理解助手。请读取图片中的关键信息并输出简洁中文描述，保留有价值的文本、数字、UI元素与上下文。".to_string(),
        history_messages: Vec::new(),
        latest_user_text: "请识别这张图片并给出可用于后续对话的文本描述。".to_string(),
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_images: vec![(
            if mime.is_empty() {
                "image/png".to_string()
            } else {
                mime.to_string()
            },
            image.bytes_base64.clone(),
        )],
        latest_audios: Vec::new(),
    };

    let supports_non_stream_fallback =
        request_format_supports_non_stream_fallback(vision_resolved.request_format);
    let prefer_non_stream = supports_non_stream_fallback
        && provider_streaming_disabled(Some(state), &vision_resolved.base_url);
    let reply = match vision_resolved.request_format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
            if prefer_non_stream {
                call_model_openai_non_stream_rig_style(vision_resolved, &vision_api.model, prepared).await?
            } else {
                match call_model_openai_rig_style(vision_resolved, &vision_api.model, prepared.clone()).await {
                    Ok(reply) => reply,
                    Err(err) if supports_non_stream_fallback => {
                        if let Err(mark_err) =
                            provider_mark_streaming_disabled(Some(state), &vision_resolved.base_url)
                        {
                            runtime_log_warn(format!(
                                "[视觉] 持久化非流式 base_url 失败: base_url={}, err={}",
                                vision_resolved.base_url, mark_err
                            ));
                        }
                        runtime_log_info(format!(
                            "[视觉] 流式失败，切换非流式重试: base_url={}, model={}, err={}",
                            vision_resolved.base_url, vision_api.model, err
                        ));
                        call_model_openai_non_stream_rig_style(
                            vision_resolved,
                            &vision_api.model,
                            prepared,
                        )
                        .await?
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        RequestFormat::OpenAIResponses => {
            call_model_openai_responses_rig_style(
                vision_resolved,
                &vision_api.model,
                prepared,
                None,
            )
            .await?
        }
        RequestFormat::Gemini => {
            call_model_gemini_rig_style(vision_resolved, &vision_api.model, prepared).await?
        }
        RequestFormat::Anthropic => {
            call_model_anthropic_rig_style(vision_resolved, &vision_api.model, prepared).await?
        }
        RequestFormat::OpenAITts
        | RequestFormat::OpenAIStt
        | RequestFormat::GeminiEmbedding
        | RequestFormat::OpenAIEmbedding
        | RequestFormat::OpenAIRerank => {
            return Err(
                format!(
                    "Vision request format '{}' is not supported.",
                    vision_resolved.request_format
                ),
            )
        }
    };
    Ok(reply.assistant_text)
}
