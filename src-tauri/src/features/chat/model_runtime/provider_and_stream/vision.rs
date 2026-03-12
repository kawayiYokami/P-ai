async fn describe_image_with_vision_api(
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

    let reply = match vision_resolved.request_format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
            call_model_openai_rig_style(vision_resolved, &vision_api.model, prepared).await?
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
