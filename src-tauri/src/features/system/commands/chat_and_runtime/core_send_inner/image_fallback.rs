async fn resolve_image_description_with_vision_fallback(
    state: &AppState,
    conversation_id: &str,
    vision_api: &ApiConfig,
    vision_resolved: &ResolvedApiConfig,
    image: &BinaryPart,
) -> Result<Option<String>, String> {
    let hash = compute_image_hash_hex(image)?;
    let cached = match {
        let state = state.clone();
        let hash = hash.clone();
        let api_id = vision_api.id.clone();
        tokio::task::spawn_blocking(move || {
            state_service_find_image_text_cache(&state, &hash, &api_id, "image", "")
        })
        .await
        .map_err(|err| format!("图片缓存读取任务失败：error={err}"))?
    } {
        Ok(cached) => cached,
        Err(err) => {
            runtime_log_warn(format!(
                "[图片转文] 缓存读取失败，跳过缓存继续转换，conversation_id={}，error={}",
                conversation_id, err
            ));
            None
        }
    };
    if let Some(text) = cached {
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }

    let start = std::time::Instant::now();
    let prepared = conversation_prompt_service().build_vision_description_prepared_prompt(image);
    let request_text = prepared_prompt_to_fast_request_text(&prepared);
    let converted = match describe_image_with_vision_api(state, vision_resolved, vision_api, prepared).await {
        Ok(text) => {
            let trimmed = text.trim().to_string();
            record_fast_request_turn_best_effort(
                state,
                conversation_id,
                build_fast_request_turn(
                    "vision_image_description",
                    &request_text,
                    &text,
                    !trimmed.is_empty(),
                    trimmed
                        .is_empty()
                        .then(|| "图转文返回为空".to_string()),
                    Some(vision_api.model.clone()),
                    Some(start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
                ),
            );
            text
        }
        Err(err) => {
            record_fast_request_turn_best_effort(
                state,
                conversation_id,
                build_fast_request_turn(
                    "vision_image_description",
                    &request_text,
                    "",
                    false,
                    Some(err.clone()),
                    Some(vision_api.model.clone()),
                    Some(start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
                ),
            );
            return Err(err);
        }
    };
    let trimmed = converted.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if let Err(err) = {
        let state = state.clone();
        let hash = hash.clone();
        let api_id = vision_api.id.clone();
        let trimmed = trimmed.clone();
        tokio::task::spawn_blocking(move || {
            state_service_upsert_image_text_cache(&state, &hash, &api_id, "image", "", &trimmed)
        })
        .await
        .map_err(|err| format!("图片缓存写入任务失败：error={err}"))?
    } {
        runtime_log_warn(format!(
            "[图片转文] 缓存写入失败，保留本次描述继续，conversation_id={}，error={}",
            conversation_id, err
        ));
    }

    Ok(Some(trimmed))
}

fn prepared_latest_user_present(prepared: &PreparedPrompt) -> bool {
    !prepared.latest_user_text.trim().is_empty()
        || !prepared.latest_user_meta_text.trim().is_empty()
        || !prepared.latest_user_extra_text.trim().is_empty()
        || prepared
            .latest_user_extra_blocks
            .iter()
            .any(|block| !block.trim().is_empty())
        || !prepared.latest_images.is_empty()
        || !prepared.latest_audios.is_empty()
}

fn recent_user_image_fallback_plan(prepared: &PreparedPrompt) -> (Vec<bool>, bool) {
    let latest_user_in_window = prepared_latest_user_present(prepared);
    let mut remaining = IMAGE_FALLBACK_RECENT_USER_MESSAGE_LIMIT
        .saturating_sub(usize::from(latest_user_in_window));
    let mut history_in_window = vec![false; prepared.history_messages.len()];

    // 远程联系人可能连续刷大量图片。图片转文按“最近用户消息条数”限流；
    // 附件路径由统一附件提示负责，这里只决定哪些图片需要额外转文。
    for (idx, message) in prepared.history_messages.iter().enumerate().rev() {
        if message.role.trim() != "user" {
            continue;
        }
        if remaining == 0 {
            break;
        }
        history_in_window[idx] = true;
        remaining -= 1;
    }

    (history_in_window, latest_user_in_window)
}

fn collect_payload_attachment_meta_entries(payload: &ChatInputPayload) -> Vec<Value> {
    let entries = normalize_payload_attachments(payload.attachments.as_ref());
    if !entries.is_empty() {
        return entries;
    }
    let mut entries = normalize_payload_image_attachments(payload.images.as_ref());
    entries.extend(
        payload
            .audios
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|audio| {
                let relative_path = audio
                    .saved_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.replace('\\', "/"))?;
                let file_name = std::path::Path::new(&relative_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("attachment")
                    .to_string();
                Some(serde_json::json!({
                    "fileName": file_name,
                    "relativePath": relative_path,
                    "mime": audio.mime.trim(),
                }))
            }),
    );
    entries
}

fn collect_payload_attachment_relative_paths(payload: &ChatInputPayload) -> Vec<String> {
    collect_payload_attachment_meta_entries(payload)
        .into_iter()
        .filter_map(|item| {
            item.get("relativePath")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .collect()
}

#[cfg(test)]
fn image_attachment_reference_label(
    payload: &ChatInputPayload,
    image: &BinaryPart,
    image_index: usize,
) -> String {
    let saved_path = image
        .saved_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"));
    if let Some(saved_path) = saved_path {
        for (index, item) in collect_payload_attachment_meta_entries(payload).iter().enumerate() {
            let relative_path = item
                .get("relativePath")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.replace('\\', "/"));
            if relative_path.as_deref() == Some(saved_path.as_str()) {
                return format!("附件#{}", index + 1);
            }
        }
    }
    format!("图片#{}", image_index + 1)
}

fn image_description_block(label: &str, text: &str) -> String {
    format!("[{} 图片转文]\n{}", label.trim(), text.trim())
}

fn drop_all_prepared_images(prepared: &mut PreparedPrompt) -> bool {
    let mut changed = !prepared.latest_images.is_empty();
    prepared.latest_images.clear();
    for message in &mut prepared.history_messages {
        changed |= !message.images.is_empty();
        message.images.clear();
    }
    changed
}

fn drop_unsupported_prepared_audios(
    selected_api: &ApiConfig,
    prepared: &mut PreparedPrompt,
) -> bool {
    if selected_api.enable_audio {
        return false;
    }
    let mut changed = !prepared.latest_audios.is_empty();
    prepared.latest_audios.clear();
    for message in &mut prepared.history_messages {
        changed |= !message.audios.is_empty();
        message.audios.clear();
    }
    if changed {
        runtime_log_warn(
            "[附件投影] 当前模型未启用音频输入，已跳过音频二进制并保留路径提示继续"
                .to_string(),
        );
    }
    changed
}

async fn apply_prompt_image_fallbacks_to_prepared(
    state: &AppState,
    conversation_id: &str,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    prepared: &mut PreparedPrompt,
) -> Result<bool, String> {
    if selected_api.enable_image {
        return Ok(false);
    }

    let vision_api = match resolve_vision_api_config(app_config) {
        Ok(api) => api,
        Err(err) => {
            runtime_log_warn(format!(
                "[图片转文] 跳过，原因=视觉模型不可用，conversation_id={}，error={}",
                conversation_id, err
            ));
            return Ok(drop_all_prepared_images(prepared));
        }
    };
    let vision_resolved = match resolve_api_config(app_config, Some(vision_api.id.as_str())) {
        Ok(resolved) => resolved,
        Err(err) => {
            runtime_log_warn(format!(
                "[图片转文] 跳过，原因=视觉模型配置解析失败，conversation_id={}，error={}",
                conversation_id, err
            ));
            return Ok(drop_all_prepared_images(prepared));
        }
    };
    if !vision_resolved.request_format.is_chat_text() {
        runtime_log_warn(format!(
            "[图片转文] 跳过，原因=视觉模型请求格式暂未接入，conversation_id={}，request_format={}",
            conversation_id, vision_resolved.request_format
        ));
        return Ok(drop_all_prepared_images(prepared));
    }

    let mut changed = false;
    let (history_image_fallback_window, latest_user_in_window) =
        recent_user_image_fallback_plan(prepared);

    for (message_index, message) in prepared.history_messages.iter_mut().enumerate() {
        if message.role.trim() != "user" || message.images.is_empty() {
            continue;
        }

        let original_images = std::mem::take(&mut message.images);
        let mut converted_blocks = Vec::<String>::new();
        if !history_image_fallback_window
            .get(message_index)
            .copied()
            .unwrap_or(false)
        {
            changed = true;
            continue;
        }

        for (index, image_payload) in original_images.into_iter().enumerate() {
            let image_label = if image_payload.label.trim().is_empty() {
                format!("图片#{}", index + 1)
            } else {
                image_payload.label.clone()
            };
            let image = BinaryPart {
                mime: image_payload.mime,
                bytes_base64: image_payload.content,
                saved_path: image_payload.saved_path,
            };
            match resolve_image_description_with_vision_fallback(
                state,
                conversation_id,
                &vision_api,
                &vision_resolved,
                &image,
            )
            .await
            {
                Ok(Some(text)) => converted_blocks.push(image_description_block(
                    &image_label,
                    &text,
                )),
                Ok(None) => {}
                Err(err) => runtime_log_warn(format!(
                    "[图片转文] 单图转换失败，保留路径提示继续，conversation_id={}，label={}，path={}，error={}",
                    conversation_id,
                    image_label,
                    image.saved_path.as_deref().unwrap_or("未提供"),
                    err
                )),
            }
        }
        if !converted_blocks.is_empty() {
            message.extra_text_blocks.extend(converted_blocks);
        }
        changed = true;
    }

    if !prepared.latest_images.is_empty() {
        let original_images = std::mem::take(&mut prepared.latest_images);
        let mut converted_blocks = Vec::<String>::new();
        if latest_user_in_window {
            for (index, image_payload) in original_images.into_iter().enumerate() {
                let image_label = if image_payload.label.trim().is_empty() {
                    format!("图片#{}", index + 1)
                } else {
                    image_payload.label.clone()
                };
                let image = BinaryPart {
                    mime: image_payload.mime,
                    bytes_base64: image_payload.content,
                    saved_path: image_payload.saved_path,
                };
                match resolve_image_description_with_vision_fallback(
                    state,
                    conversation_id,
                    &vision_api,
                    &vision_resolved,
                    &image,
                )
                .await
                {
                    Ok(Some(text)) => converted_blocks.push(image_description_block(
                        &image_label,
                        &text,
                    )),
                    Ok(None) => {}
                    Err(err) => runtime_log_warn(format!(
                        "[图片转文] 单图转换失败，保留路径提示继续，conversation_id={}，label={}，path={}，error={}",
                        conversation_id,
                        image_label,
                        image.saved_path.as_deref().unwrap_or("未提供"),
                        err
                    )),
                }
            }
        }
        if !converted_blocks.is_empty() {
            prepared_prompt_append_latest_user_trailing_blocks(prepared, &converted_blocks);
        }
        changed = true;
    }

    Ok(changed)
}
