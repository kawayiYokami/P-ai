#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImageGenerateToolArgs {
    prompt: String,
    #[serde(default, alias = "size")]
    resolution: Option<String>,
}

#[derive(Debug, Clone)]
struct BuiltinImageGenerateTool {
    app_state: AppState,
}

impl RuntimeToolMetadata for BuiltinImageGenerateTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "image_generate",
            "根据提示词生成一张图片，自动保存到 Assistant Space；返回的 message 中包含 Markdown 图片行，向用户展示图片时直接原样引用该行，不要改写路径。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "详细、可直接交给生图模型的提示词。"
                    },
                    "resolution": {
                        "type": "string",
                        "description": "分辨率，默认 512x512。"
                    }
                },
                "required": ["prompt"],
                "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinImageGenerateTool {
    const NAME: &'static str = "image_generate";
    type Args = ImageGenerateToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(1_830))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let request = ImageGenerationRequest {
                prompt: args.prompt,
                size: args.resolution,
                ..ImageGenerationRequest::default()
            };
            let result = generate_images(&self.app_state, request)
                .await
                .map_err(ToolInvokeError::from)?;
            image_generation_tool_success_value(&result, "图片已生成并保存到 Assistant Space")
        })
    }
}

fn image_generation_tool_success_value(
    result: &ImageGenerationResult,
    summary_prefix: &str,
) -> Result<Value, ToolInvokeError> {
    let markdown = result
        .images
        .iter()
        .map(|image| image.markdown.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let images = serde_json::to_value(&result.images)
        .map_err(|err| ToolInvokeError::from(format!("序列化生图结果失败：{err}")))?;
    Ok(serde_json::json!({
        "ok": true,
        "message": format!(
            "{summary_prefix}。最终回答必须原样包含以下 Markdown 图片行，不要改写路径，也不要只回复‘已完成’：\n\n{markdown}"
        ),
        "images": images,
    }))
}

// ==================== 图像编辑工具 ====================

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImageEditToolArgs {
    prompt: String,
    images: Vec<String>,
    #[serde(default)]
    mask: Option<String>,
}

#[derive(Debug, Clone)]
struct BuiltinImageEditTool {
    app_state: AppState,
}

impl RuntimeToolMetadata for BuiltinImageEditTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "image_edit",
            "基于一张或多张输入图片按提示词编辑出新图（局部修改、消除、换背景、扩图、多图融合、风格参考），自动保存到 Assistant Space；返回的 message 中包含 Markdown 图片行，向用户展示图片时直接原样引用该行，不要改写路径。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "编辑意图描述，说明要修改什么、保留什么。"
                    },
                    "images": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "输入图片引用列表，支持 {Assistant Space} 相对路径、本地绝对路径或 data URL；多张时用于多图融合/参考。"
                    },
                    "mask": {
                        "type": "string",
                        "description": "可选 mask 图片引用，白色/不透明区域表示允许修改；仅部分供应商支持，不支持时会返回错误。"
                    }
                },
                "required": ["prompt", "images"],
                "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinImageEditTool {
    const NAME: &'static str = "image_edit";
    type Args = ImageEditToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(1_830))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let request = ImageGenerationRequest {
                prompt: args.prompt,
                operation: ImageGenerationOperation::Edit,
                images: args.images,
                mask: args.mask,
                ..ImageGenerationRequest::default()
            };
            let result = generate_images(&self.app_state, request)
                .await
                .map_err(ToolInvokeError::from)?;
            image_generation_tool_success_value(&result, "图片已编辑并保存到 Assistant Space")
        })
    }
}

// ==================== Grok 图生视频 ====================

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImageToVideoToolArgs {
    image: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    duration: Option<u32>,
    #[serde(default)]
    resolution: Option<String>,
}

#[derive(Debug, Clone)]
struct BuiltinImageToVideoTool {
    app_state: AppState,
}

impl RuntimeToolMetadata for BuiltinImageToVideoTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "image_to_video",
            "把一张图作为第一帧生成视频，自动保存到 Assistant Space。没有文生视频，需要新画面时先用 image_generate 画第一帧，再用本工具描述动作。返回的 message 中包含本地视频路径，向用户展示时直接原样引用，不要改写路径。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "image": {
                        "type": "string",
                        "description": "第一帧图片。支持 {Assistant Space} 相对路径、本地绝对路径或 data URL。"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "可选，只描述动作或镜头，一两句现在时。不填则按画面自然运动。"
                    },
                    "duration": {
                        "type": "integer",
                        "enum": [6, 10],
                        "description": "时长，只能是 6 或 10 秒，默认 6。"
                    },
                    "resolution": {
                        "type": "string",
                        "enum": ["480p", "720p"],
                        "description": "分辨率，只能是 480p 或 720p，默认 480p。"
                    }
                },
                "required": ["image"],
                "additionalProperties": false
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinImageToVideoTool {
    const NAME: &'static str = "image_to_video";
    type Args = ImageToVideoToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(360))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let image = load_image_edit_reference(&self.app_state, &args.image, "首帧").await?;
            let path = generate_grok_image_to_video(
                &self.app_state,
                GrokImageToVideoRequest {
                    image_url: image_edit_data_url(&image),
                    model_id: None,
                    prompt: args.prompt.unwrap_or_default(),
                    duration: args.duration.unwrap_or(6),
                    resolution: args.resolution.unwrap_or_else(|| "480p".to_string()),
                },
            )
            .await?;
            Ok(serde_json::json!({
                "ok": true,
                "message": format!("视频已生成并保存到 Assistant Space。最终回答必须原样包含这个路径，不要改写：\n\n{path}"),
                "path": path
            }))
        })
    }
}

#[cfg(test)]
mod image_generate_tool_tests {
    use super::*;

    #[test]
    fn image_generate_definition_should_require_prompt_and_default_model() {
        let state = AppState::new().ok();
        let Some(state) = state else {
            return;
        };
        let definition = BuiltinImageGenerateTool { app_state: state }.provider_tool_definition();
        assert_eq!(definition.name, "image_generate");
        assert_eq!(
            definition.parameters["required"].as_array().and_then(|items| items.first()).and_then(Value::as_str),
            Some("prompt")
        );
        assert!(definition.description.contains("Markdown 图片行"));
        assert!(!definition.description.contains("设置页"));
        let properties = definition.parameters["properties"].as_object().cloned().unwrap_or_default();
        assert!(properties.contains_key("prompt"));
        assert!(properties.contains_key("resolution"));
        assert!(!properties.contains_key("model_id"));
        assert!(!properties.contains_key("negative_prompt"));
        assert!(!properties.contains_key("quality"));
    }

    #[test]
    fn image_edit_definition_should_require_prompt_and_images_only() {
        let state = AppState::new().ok();
        let Some(state) = state else {
            return;
        };
        let definition = BuiltinImageEditTool { app_state: state }.provider_tool_definition();
        assert_eq!(definition.name, "image_edit");
        let required = definition.parameters["required"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(required.len(), 2);
        assert!(required.iter().any(|value| value.as_str() == Some("prompt")));
        assert!(required.iter().any(|value| value.as_str() == Some("images")));
        assert!(definition.description.contains("Markdown 图片行"));
        let properties = definition.parameters["properties"].as_object().cloned().unwrap_or_default();
        assert!(properties.contains_key("mask"));
        assert!(!properties.contains_key("aspect_ratio"));
        assert!(!properties.contains_key("model_id"));
        assert!(!properties.contains_key("quality"));
        assert!(!properties.contains_key("seed"));
    }

    #[test]
    fn image_to_video_definition_requires_only_the_first_frame() {
        let state = AppState::new().ok();
        let Some(state) = state else {
            return;
        };
        let definition = BuiltinImageToVideoTool { app_state: state }.provider_tool_definition();
        assert_eq!(definition.name, "image_to_video");
        assert_eq!(
            definition.parameters["required"].as_array().and_then(|items| items.first()).and_then(Value::as_str),
            Some("image")
        );
        let properties = definition.parameters["properties"].as_object().cloned().unwrap_or_default();
        assert!(properties.contains_key("prompt"));
        assert_eq!(properties["duration"]["enum"], serde_json::json!([6, 10]));
        assert_eq!(properties["resolution"]["enum"], serde_json::json!(["480p", "720p"]));
        assert!(definition.description.contains("image_generate"));
    }
}
