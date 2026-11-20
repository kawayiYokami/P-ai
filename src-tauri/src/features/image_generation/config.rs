fn image_generation_endpoint_id(provider_id: &str, model_id: &str) -> String {
    format!("{}::{}", provider_id.trim(), model_id.trim())
}

fn parse_image_generation_endpoint_id(value: &str) -> Option<(String, String)> {
    let (provider_id, model_id) = value.trim().split_once("::")?;
    let provider_id = provider_id.trim();
    let model_id = model_id.trim();
    if provider_id.is_empty() || model_id.is_empty() {
        return None;
    }
    Some((provider_id.to_string(), model_id.to_string()))
}

fn default_image_generation_provider_name(kind: ImageGenerationProviderKind) -> &'static str {
    match kind {
        ImageGenerationProviderKind::Comfyui => "Local ComfyUI",
        ImageGenerationProviderKind::Codex => "OpenAI Codex Image Generation",
        ImageGenerationProviderKind::Openai => "OpenAI Images",
        ImageGenerationProviderKind::Xai => "xAI Grok Imagine",
        ImageGenerationProviderKind::Seedream => "Seedance / Seedream",
        ImageGenerationProviderKind::Gemini => "Gemini Nano Banana",
        ImageGenerationProviderKind::Sensenova => "商汤科技 · SenseNova",
    }
}

fn default_image_generation_base_url(kind: ImageGenerationProviderKind) -> &'static str {
    match kind {
        ImageGenerationProviderKind::Comfyui => "http://127.0.0.1:8188",
        ImageGenerationProviderKind::Codex => "https://chatgpt.com/backend-api/codex",
        ImageGenerationProviderKind::Openai => "https://api.openai.com/v1",
        ImageGenerationProviderKind::Xai => "https://api.x.ai/v1",
        ImageGenerationProviderKind::Seedream => "https://ark.cn-beijing.volces.com/api/v3",
        ImageGenerationProviderKind::Gemini => {
            "https://generativelanguage.googleapis.com/v1beta"
        }
        ImageGenerationProviderKind::Sensenova => "https://token.sensenova.cn/v1",
    }
}

fn normalize_image_generation_optional_text(value: &mut Option<String>) {
    *value = value
        .as_ref()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty());
}

fn normalize_comfyui_node_mapping(
    mapping: &mut ComfyUiNodeInputMapping,
    default_input_key: &str,
) {
    let mut seen = std::collections::HashSet::<String>::new();
    mapping.node_ids = mapping
        .node_ids
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert(item.to_ascii_lowercase()))
        .collect();
    mapping.input_key = mapping.input_key.trim().to_string();
    if mapping.input_key.is_empty() {
        mapping.input_key = default_input_key.to_string();
    }
}

fn normalize_comfyui_workflow_mapping(mapping: &mut ComfyUiWorkflowMapping) {
    normalize_comfyui_node_mapping(&mut mapping.prompt, "text");
    normalize_comfyui_node_mapping(&mut mapping.negative_prompt, "text");
    normalize_comfyui_node_mapping(&mut mapping.model, "ckpt_name");
    normalize_comfyui_node_mapping(&mut mapping.width, "width");
    normalize_comfyui_node_mapping(&mut mapping.height, "height");
    normalize_comfyui_node_mapping(&mut mapping.seed, "seed");
    normalize_comfyui_node_mapping(&mut mapping.steps, "steps");
    let mut seen = std::collections::HashSet::<String>::new();
    mapping.output_node_ids = mapping
        .output_node_ids
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert(item.to_ascii_lowercase()))
        .collect();
}

fn normalize_image_generation_models(provider: &mut ImageGenerationProviderConfig) {
    let mut seen = std::collections::HashSet::<String>::new();
    provider.models.retain_mut(|model| {
        model.id = model.id.trim().to_string();
        if model.id.is_empty()
            || model.id.contains("::")
            || !seen.insert(model.id.to_ascii_lowercase())
        {
            return false;
        }
        model.name = model.name.trim().to_string();
        model.model = model.model.trim().to_string();
        if provider.provider_type == ImageGenerationProviderKind::Codex {
            // 生图模型不可选：存量 gpt-5.6-luna / gpt-image-* 一律归到 gpt-image-2，幂等。
            model.model = CODEX_IMAGE_MODEL.to_string();
        }
        if model.name.is_empty() {
            model.name = if model.model.is_empty() {
                model.id.clone()
            } else {
                model.model.clone()
            };
        }
        if model.model.is_empty()
            && !matches!(
                provider.provider_type,
                ImageGenerationProviderKind::Comfyui | ImageGenerationProviderKind::Codex
            )
        {
            model.model = model.id.clone();
        }
        normalize_image_generation_optional_text(&mut model.default_size);
        normalize_image_generation_optional_text(&mut model.default_aspect_ratio);
        normalize_image_generation_optional_text(&mut model.default_quality);
        true
    });
}

fn normalize_image_generation_provider(provider: &mut ImageGenerationProviderConfig) {
    provider.id = provider.id.trim().to_string();
    provider.name = provider.name.trim().to_string();
    if provider.name.is_empty() {
        provider.name = default_image_generation_provider_name(provider.provider_type).to_string();
    }
    provider.base_url = provider.base_url.trim().trim_end_matches('/').to_string();
    if provider.base_url.is_empty() {
        provider.base_url = default_image_generation_base_url(provider.provider_type).to_string();
    }
    let mut seen_keys = std::collections::HashSet::<String>::new();
    provider.api_keys = provider
        .api_keys
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| seen_keys.insert(item.clone()))
        .collect();
    provider.key_cursor = provider.key_cursor.min(1_000_000);
    provider.timeout_seconds = if provider.timeout_seconds == 0 {
        default_image_generation_timeout_seconds()
    } else {
        provider.timeout_seconds.clamp(10, 600)
    };
    provider.comfyui_workflow_json = provider.comfyui_workflow_json.trim().to_string();
    provider.codex_api_provider_id = provider
        .codex_api_provider_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    normalize_comfyui_workflow_mapping(&mut provider.comfyui_mapping);
    normalize_image_generation_models(provider);
}

fn image_generation_model_exists(config: &AppConfig, endpoint_id: &str) -> bool {
    let Some((provider_id, model_id)) = parse_image_generation_endpoint_id(endpoint_id) else {
        return false;
    };
    config.image_providers.iter().any(|provider| {
        provider.id == provider_id
            && provider.enabled
            && !provider.deprecated
            && provider.models.iter().any(|model| {
                model.id == model_id && model.enabled && !model.deprecated
            })
    })
}

fn normalize_image_generation_config(config: &mut AppConfig) {
    let mut seen = std::collections::HashSet::<String>::new();
    config.image_providers.retain_mut(|provider| {
        normalize_image_generation_provider(provider);
        !provider.id.is_empty()
            && !provider.id.contains("::")
            && seen.insert(provider.id.to_ascii_lowercase())
    });
    config.image_generation_model_id = config
        .image_generation_model_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| image_generation_model_exists(config, value));
    config.image_to_video_model_id = config
        .image_to_video_model_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| image_generation_model_exists(config, value));
}

#[cfg(test)]
mod image_generation_config_tests {
    use super::*;

    #[test]
    fn image_generation_config_should_drop_duplicates_and_invalid_default() {
        let mut config = AppConfig::default();
        let mut provider = ImageGenerationProviderConfig::default();
        provider.id = " images ".to_string();
        provider.api_keys = vec![" key ".to_string(), "key".to_string()];
        provider.timeout_seconds = 1;
        provider.models.push(provider.models[0].clone());
        config.image_providers = vec![provider.clone(), provider];
        config.image_generation_model_id = Some("missing::model".to_string());
        config.image_to_video_model_id = Some("missing::video".to_string());

        normalize_image_generation_config(&mut config);

        assert_eq!(config.image_providers.len(), 1);
        assert_eq!(config.image_providers[0].id, "images");
        assert_eq!(config.image_providers[0].api_keys, vec!["key"]);
        assert_eq!(config.image_providers[0].timeout_seconds, 10);
        assert_eq!(config.image_providers[0].models.len(), 1);
        assert!(config.image_generation_model_id.is_none());
        assert!(config.image_to_video_model_id.is_none());
    }

    #[test]
    fn image_generation_config_should_keep_enabled_default_endpoint() {
        let mut config = AppConfig::default();
        let provider = ImageGenerationProviderConfig::default();
        let endpoint_id = image_generation_endpoint_id(&provider.id, &provider.models[0].id);
        config.image_providers = vec![provider];
        config.image_generation_model_id = Some(endpoint_id.clone());

        normalize_image_generation_config(&mut config);

        assert_eq!(config.image_generation_model_id.as_deref(), Some(endpoint_id.as_str()));
    }
}
