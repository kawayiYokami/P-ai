fn resolve_image_generation_model(
    config: &AppConfig,
    requested_model_id: Option<&str>,
) -> Result<ResolvedImageGenerationModel, String> {
    let endpoint_id = requested_model_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| config.image_generation_model_id.clone())
        .ok_or_else(|| "尚未选择默认生图模型，请先在“设置 → 生图”中配置并选择模型。".to_string())?;
    let (provider_id, model_id) = parse_image_generation_endpoint_id(&endpoint_id)
        .ok_or_else(|| format!("生图模型 ID 无效：{endpoint_id}"))?;
    let provider = config
        .image_providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| format!("找不到生图供应商：{provider_id}"))?;
    if !provider.enabled || provider.deprecated {
        return Err(format!("生图供应商当前不可用：{}", provider.name));
    }
    let model = provider
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| format!("找不到生图模型：{endpoint_id}"))?;
    if !model.enabled || model.deprecated {
        return Err(format!("生图模型当前不可用：{}", model.name));
    }
    if model.model.trim().is_empty()
        && !matches!(
            provider.provider_type,
            ImageGenerationProviderKind::Comfyui | ImageGenerationProviderKind::Codex
        )
    {
        return Err(format!("生图模型缺少供应商模型名：{}", model.name));
    }
    Ok(ResolvedImageGenerationModel {
        endpoint_id: image_generation_endpoint_id(&provider_id, &model_id),
        provider: provider.clone(),
        model: model.clone(),
    })
}

fn normalize_image_generation_request(
    mut request: ImageGenerationRequest,
) -> Result<ImageGenerationRequest, String> {
    request.prompt = request.prompt.trim().to_string();
    if request.prompt.is_empty() {
        return Err("生图提示词不能为空".to_string());
    }
    if request.prompt.chars().count() > 100_000 {
        return Err("生图提示词过长，最多 100000 个字符".to_string());
    }
    request.model_id = request
        .model_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    request.negative_prompt = request
        .negative_prompt
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    request.size = request
        .size
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    request.aspect_ratio = request
        .aspect_ratio
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    request.quality = request
        .quality
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    request.n = request.n.clamp(1, 4);
    request.steps = request.steps.map(|value| value.clamp(1, 500));
    request.images = request
        .images
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    request.mask = request
        .mask
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    // 带输入图即视为编辑；声明编辑却无图直接拒绝，避免错误降级为文生图。
    if !request.images.is_empty() {
        request.operation = ImageGenerationOperation::Edit;
    } else if matches!(request.operation, ImageGenerationOperation::Edit) {
        return Err("图像编辑至少需要一张输入图片（images）".to_string());
    }
    if request.mask.is_some() && !matches!(request.operation, ImageGenerationOperation::Edit) {
        return Err("mask 只能在图像编辑时使用，请同时提供 images".to_string());
    }
    if matches!(request.operation, ImageGenerationOperation::Edit) {
        // 编辑路径按单次结果语义运行，不支持多张重复输出。
        request.n = 1;
    }
    Ok(request)
}

async fn generate_image_with_provider_once(
    state: &AppState,
    resolved: &ResolvedImageGenerationModel,
    request: &ImageGenerationRequest,
    edit_inputs: &ImageEditInputs,
    api_key: &str,
) -> Result<ProviderImageGenerationOutput, String> {
    let editing = matches!(request.operation, ImageGenerationOperation::Edit);
    match resolved.provider.provider_type {
        ImageGenerationProviderKind::Comfyui if editing => {
            edit_comfyui_image_once(state, resolved, request, edit_inputs, api_key).await
        }
        ImageGenerationProviderKind::Comfyui => {
            generate_comfyui_image_once(state, resolved, request, api_key).await
        }
        ImageGenerationProviderKind::Codex => {
            generate_codex_image_once(state, resolved, request, edit_inputs).await
        }
        ImageGenerationProviderKind::Openai if editing => {
            edit_openai_image_once(state, resolved, request, edit_inputs, api_key).await
        }
        ImageGenerationProviderKind::Openai => {
            generate_openai_image_once(state, resolved, request, api_key).await
        }
        ImageGenerationProviderKind::Xai if editing => {
            edit_xai_image_once(state, resolved, request, edit_inputs, api_key).await
        }
        ImageGenerationProviderKind::Xai => {
            generate_xai_image_once(state, resolved, request, api_key).await
        }
        ImageGenerationProviderKind::Seedream if editing => {
            edit_seedream_image_once(state, resolved, request, edit_inputs, api_key).await
        }
        ImageGenerationProviderKind::Seedream => {
            generate_seedream_image_once(state, resolved, request, api_key).await
        }
        ImageGenerationProviderKind::Gemini if editing => {
            edit_gemini_image_once(state, resolved, request, edit_inputs, api_key).await
        }
        ImageGenerationProviderKind::Gemini => {
            generate_gemini_image_once(state, resolved, request, api_key).await
        }
        ImageGenerationProviderKind::Sensenova if editing => {
            edit_sensenova_image_once(state, resolved, request, edit_inputs, api_key).await
        }
        ImageGenerationProviderKind::Sensenova => {
            generate_sensenova_image_once(state, resolved, request, api_key).await
        }
    }
}

async fn generate_images(
    state: &AppState,
    request: ImageGenerationRequest,
) -> Result<ImageGenerationResult, String> {
    let request = normalize_image_generation_request(request)?;
    let mut config = state_read_config_cached(state)?;
    normalize_image_generation_config(&mut config);
    let resolved = resolve_image_generation_model(&config, request.model_id.as_deref())?;
    let api_key = select_image_generation_api_key(&resolved.provider);
    if !matches!(
        resolved.provider.provider_type,
        ImageGenerationProviderKind::Comfyui | ImageGenerationProviderKind::Codex | ImageGenerationProviderKind::Xai
    )
        && api_key.trim().is_empty()
    {
        return Err(format!("生图供应商“{}”尚未配置 API Key", resolved.provider.name));
    }
    let started = std::time::Instant::now();
    let edit_inputs = load_image_edit_inputs(state, &request).await?;
    let operation_label = if matches!(request.operation, ImageGenerationOperation::Edit) {
        "编辑"
    } else {
        "生成"
    };
    runtime_log_info(format!(
        "[工具] 图像生成 开始 操作={} 供应商={} 类型={} 模型={} 数量={} 输入图={}",
        operation_label,
        resolved.provider.name,
        resolved.provider.provider_type.as_str(),
        resolved.model.model,
        request.n,
        edit_inputs.images.len()
    ));
    let mut assets = Vec::<GeneratedImageAsset>::new();
    let mut provider_texts = Vec::<String>::new();
    for index in 0..request.n {
        let mut single_request = request.clone();
        single_request.n = 1;
        if let Some(seed) = request.seed {
            single_request.seed = Some(seed.saturating_add(i64::from(index)));
        }
        let output = generate_image_with_provider_once(
            state,
            &resolved,
            &single_request,
            &edit_inputs,
            &api_key,
        )
        .await?;
        if let Some(text) = output.text.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) {
            if !provider_texts.contains(&text) {
                provider_texts.push(text);
            }
        }
        for pending in output.images {
            let asset = materialize_pending_generated_image(
                state,
                &resolved.provider,
                &api_key,
                pending,
            )
            .await?;
            assets.push(asset);
            if assets.len() >= request.n as usize {
                break;
            }
        }
        if assets.len() >= request.n as usize {
            break;
        }
    }
    if assets.is_empty() {
        return Err("供应商未返回可保存的图片".to_string());
    }
    runtime_log_info(format!(
        "[工具] 图像生成 完成 操作={} 供应商={} 模型={} 图片数={} 耗时={}ms",
        operation_label,
        resolved.provider.name,
        resolved.model.model,
        assets.len(),
        started.elapsed().as_millis()
    ));
    Ok(ImageGenerationResult {
        provider_id: resolved.provider.id.clone(),
        provider_name: resolved.provider.name.clone(),
        provider_type: resolved.provider.provider_type,
        model_id: resolved.endpoint_id,
        model: if resolved.model.model.trim().is_empty() {
            resolved.model.id.clone()
        } else {
            resolved.model.model.clone()
        },
        images: assets,
        provider_text: (!provider_texts.is_empty()).then(|| provider_texts.join("\n")),
    })
}

#[cfg(test)]
mod image_generation_service_tests {
    use super::*;

    #[test]
    fn resolver_should_use_default_model_and_allow_explicit_override() {
        let mut config = AppConfig::default();
        let mut first = ImageGenerationProviderConfig::default();
        first.id = "first".to_string();
        let mut second = ImageGenerationProviderConfig::default();
        second.id = "second".to_string();
        second.models[0].id = "other".to_string();
        config.image_generation_model_id = Some(image_generation_endpoint_id(
            &first.id,
            &first.models[0].id,
        ));
        config.image_providers = vec![first, second];

        let default = resolve_image_generation_model(&config, None).ok();
        let explicit = resolve_image_generation_model(&config, Some("second::other")).ok();

        assert_eq!(default.map(|value| value.provider.id), Some("first".to_string()));
        assert_eq!(explicit.map(|value| value.provider.id), Some("second".to_string()));
    }
}
