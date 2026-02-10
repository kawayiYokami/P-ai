fn ensure_active_conversation_index(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    if let Some((idx, _)) = data.conversations.iter().enumerate().find(|(_, c)| {
        c.status == "active" && c.api_config_id == api_config_id && c.agent_id == agent_id
    }) {
        return idx;
    }

    let now = now_iso();
    let conversation = Conversation {
        id: Uuid::new_v4().to_string(),
        title: format!("Chat {}", &now.chars().take(16).collect::<String>()),
        api_config_id: api_config_id.to_string(),
        agent_id: agent_id.to_string(),
        created_at: now.clone(),
        updated_at: now,
        last_assistant_at: None,
        status: "active".to_string(),
        messages: Vec::new(),
    };

    data.conversations.push(conversation);
    data.conversations.len() - 1
}

fn archive_if_idle(data: &mut AppData, api_config_id: &str, agent_id: &str) -> bool {
    let Some((idx, _)) = data.conversations.iter().enumerate().find(|(_, c)| {
        c.status == "active" && c.api_config_id == api_config_id && c.agent_id == agent_id
    }) else {
        return false;
    };

    let Some(last_assistant_at) = data.conversations[idx]
        .last_assistant_at
        .as_deref()
        .and_then(parse_iso)
    else {
        return false;
    };

    let now = now_utc();
    if now.unix_timestamp() - last_assistant_at.unix_timestamp() < ARCHIVE_IDLE_SECONDS {
        return false;
    }

    let mut source = data.conversations.remove(idx);
    source.status = "archived".to_string();
    source.updated_at = now_iso();
    data.archived_conversations.push(ConversationArchive {
        archive_id: Uuid::new_v4().to_string(),
        archived_at: now_iso(),
        reason: "idle_timeout_30m".to_string(),
        source_conversation: source,
    });

    true
}

fn compress_image_to_webp(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image =
        image::load_from_memory(bytes).map_err(|err| format!("Decode image failed: {err}"))?;
    let mut cursor = Cursor::new(Vec::<u8>::new());
    image
        .write_to(&mut cursor, ImageFormat::WebP)
        .map_err(|err| format!("Encode image to WebP failed: {err}"))?;
    Ok(cursor.into_inner())
}

fn build_user_parts(
    payload: &ChatInputPayload,
    api_config: &ApiConfig,
) -> Result<Vec<MessagePart>, String> {
    let mut parts = Vec::<MessagePart>::new();
    let mut total_binary = 0usize;

    if let Some(text) = payload
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if !api_config.enable_text {
            return Err("Current API config has text disabled.".to_string());
        }
        parts.push(MessagePart::Text {
            text: text.to_string(),
        });
    }

    if let Some(images) = &payload.images {
        if !images.is_empty() && !api_config.enable_image {
            return Err("Current API config has image disabled.".to_string());
        }

        for image in images {
            let raw = B64
                .decode(image.bytes_base64.trim())
                .map_err(|err| format!("Decode image base64 failed: {err}"))?;
            let webp = compress_image_to_webp(&raw)?;
            total_binary += webp.len();
            parts.push(MessagePart::Image {
                mime: "image/webp".to_string(),
                bytes_base64: B64.encode(webp),
                name: None,
                compressed: true,
            });
        }
    }

    if let Some(audios) = &payload.audios {
        if !audios.is_empty() && !api_config.enable_audio {
            return Err("Current API config has audio disabled.".to_string());
        }

        for audio in audios {
            let raw = B64
                .decode(audio.bytes_base64.trim())
                .map_err(|err| format!("Decode audio base64 failed: {err}"))?;
            total_binary += raw.len();
            parts.push(MessagePart::Audio {
                mime: audio.mime.trim().to_string(),
                bytes_base64: B64.encode(raw),
                name: None,
                compressed: false,
            });
        }
    }

    if total_binary > MAX_MULTIMODAL_BYTES {
        return Err(format!(
            "Multimodal payload exceeds 10MB limit ({} bytes).",
            total_binary
        ));
    }

    if parts.is_empty() {
        return Err("Request payload is empty. Provide text, image, or audio.".to_string());
    }

    Ok(parts)
}

fn render_message_for_context(message: &ChatMessage) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => chunks.push(text.clone()),
            MessagePart::Image { .. } => chunks.push("[image attached]".to_string()),
            MessagePart::Audio { .. } => chunks.push("[audio attached]".to_string()),
        }
    }
    format!("{}: {}", message.role.to_uppercase(), chunks.join(" | "))
}
fn build_prompt(
    conversation: &Conversation,
    agent: &AgentProfile,
    user_alias: &str,
    current_time: &str,
) -> PreparedPrompt {
    let mut history_lines = Vec::<String>::new();
    for message in &conversation.messages {
        history_lines.push(render_message_for_context(message));
    }

    let preamble = format!(
    "[SYSTEM PROMPT]\n{}\n\n[ROLE MAPPING]\nAssistant name: {}\nUser name: {}\nRules:\n- You are the assistant named '{}'.\n- The human user is named '{}'.\n- Never treat yourself as the user.\n\n[TIME]\nCurrent UTC time: {}\n\n[CONVERSATION HISTORY]\n{}\n",
    agent.system_prompt,
    agent.name,
    user_alias,
    agent.name,
    user_alias,
    current_time,
    history_lines.join("\n")
  );

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();

    let mut latest_user_text = String::new();
    let mut latest_images = Vec::<(String, String)>::new();
    let mut latest_audios = Vec::<(String, String)>::new();

    if let Some(msg) = latest_user {
        for part in msg.parts {
            match part {
                MessagePart::Text { text } => {
                    if !latest_user_text.is_empty() {
                        latest_user_text.push('\n');
                    }
                    latest_user_text.push_str(&text);
                }
                MessagePart::Image {
                    mime, bytes_base64, ..
                } => latest_images.push((mime, bytes_base64)),
                MessagePart::Audio {
                    mime, bytes_base64, ..
                } => latest_audios.push((mime, bytes_base64)),
            }
        }
    }

    PreparedPrompt {
        preamble,
        latest_user_text,
        latest_images,
        latest_audios,
    }
}

fn image_media_type_from_mime(mime: &str) -> Option<ImageMediaType> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some(ImageMediaType::JPEG),
        "image/png" => Some(ImageMediaType::PNG),
        "image/gif" => Some(ImageMediaType::GIF),
        "image/webp" => Some(ImageMediaType::WEBP),
        "image/heic" => Some(ImageMediaType::HEIC),
        "image/heif" => Some(ImageMediaType::HEIF),
        "image/svg+xml" => Some(ImageMediaType::SVG),
        _ => None,
    }
}

fn audio_media_type_from_mime(mime: &str) -> Option<AudioMediaType> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "audio/wav" | "audio/wave" => Some(AudioMediaType::WAV),
        "audio/mp3" | "audio/mpeg" => Some(AudioMediaType::MP3),
        "audio/aiff" => Some(AudioMediaType::AIFF),
        "audio/aac" => Some(AudioMediaType::AAC),
        "audio/ogg" => Some(AudioMediaType::OGG),
        "audio/flac" => Some(AudioMediaType::FLAC),
        _ => None,
    }
}

