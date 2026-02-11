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
        last_user_at: None,
        last_assistant_at: None,
        last_context_usage_ratio: 0.0,
        status: "active".to_string(),
        messages: Vec::new(),
    };

    data.conversations.push(conversation);
    data.conversations.len() - 1
}

#[derive(Debug, Clone)]
struct ArchiveDecision {
    should_archive: bool,
    forced: bool,
    reason: String,
    usage_ratio: f64,
}

fn estimated_tokens_for_text(text: &str) -> f64 {
    let mut zh_chars = 0usize;
    let mut other_chars = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{f900}'..='\u{faff}').contains(&ch)
        {
            zh_chars += 1;
        } else {
            other_chars += 1;
        }
    }
    zh_chars as f64 * 0.6 + other_chars as f64 * 0.3
}

fn estimated_tokens_for_message(message: &ChatMessage) -> f64 {
    let mut tokens = 12.0;
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => {
                tokens += estimated_tokens_for_text(text);
            }
            MessagePart::Image { .. } => {
                tokens += 280.0;
            }
            MessagePart::Audio { .. } => {
                tokens += 320.0;
            }
        }
    }
    tokens
}

fn estimate_conversation_tokens(conversation: &Conversation) -> u32 {
    let mut sum = 0.0f64;
    for msg in &conversation.messages {
        sum += estimated_tokens_for_message(msg);
    }
    sum.ceil().max(0.0) as u32
}

fn compute_context_usage_ratio(conversation: &Conversation, context_window_tokens: u32) -> f64 {
    let max_tokens = context_window_tokens.max(1) as f64;
    (estimate_conversation_tokens(conversation) as f64 / max_tokens).max(0.0)
}

fn decide_archive_before_user_message(
    conversation: &Conversation,
    context_window_tokens: u32,
) -> ArchiveDecision {
    let usage_ratio = compute_context_usage_ratio(conversation, context_window_tokens);
    if usage_ratio >= 0.82 {
        return ArchiveDecision {
            should_archive: true,
            forced: true,
            reason: "force_context_usage_82".to_string(),
            usage_ratio,
        };
    }

    let Some(last_user_at) = conversation.last_user_at.as_deref().and_then(parse_iso) else {
        return ArchiveDecision {
            should_archive: false,
            forced: false,
            reason: "no_last_user_timestamp".to_string(),
            usage_ratio,
        };
    };

    let now = now_utc();
    let idle_seconds = now.unix_timestamp() - last_user_at.unix_timestamp();
    if idle_seconds < ARCHIVE_IDLE_SECONDS {
        return ArchiveDecision {
            should_archive: false,
            forced: false,
            reason: "idle_not_reached_30m".to_string(),
            usage_ratio,
        };
    }

    if usage_ratio >= 0.30 {
        return ArchiveDecision {
            should_archive: true,
            forced: false,
            reason: "idle_30m_and_usage_30pct".to_string(),
            usage_ratio,
        };
    }

    ArchiveDecision {
        should_archive: false,
        forced: false,
        reason: "usage_below_30pct".to_string(),
        usage_ratio,
    }
}

fn archive_conversation_now(
    data: &mut AppData,
    conversation_id: &str,
    reason: &str,
    summary: &str,
) -> Option<String> {
    let idx = data
        .conversations
        .iter()
        .position(|c| c.id == conversation_id && c.status == "active")?;
    let mut source = data.conversations.remove(idx);
    source.status = "archived".to_string();
    source.updated_at = now_iso();
    let archive_id = Uuid::new_v4().to_string();
    data.archived_conversations.push(ConversationArchive {
        archive_id: archive_id.clone(),
        archived_at: now_iso(),
        reason: reason.to_string(),
        summary: summary.to_string(),
        source_conversation: source,
    });
    Some(archive_id)
}

fn keep_recent_turns(messages: &[ChatMessage], turn_count: usize) -> Vec<ChatMessage> {
    let mut turns: Vec<Vec<ChatMessage>> = Vec::new();
    let mut i = 0usize;
    while i < messages.len() {
        if messages[i].role != "user" {
            i += 1;
            continue;
        }
        let mut turn = vec![messages[i].clone()];
        if i + 1 < messages.len() && messages[i + 1].role == "assistant" {
            turn.push(messages[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
        turns.push(turn);
    }

    let start = turns.len().saturating_sub(turn_count);
    turns[start..].iter().flat_map(|t| t.clone()).collect::<Vec<_>>()
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

fn render_message_content_for_model(message: &ChatMessage) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => chunks.push(text.clone()),
            MessagePart::Image { .. } => chunks.push("[image attached]".to_string()),
            MessagePart::Audio { .. } => chunks.push("[audio attached]".to_string()),
        }
    }
    chunks.join(" | ")
}

fn sanitize_memory_block_xml(raw: &str) -> String {
    if !raw.contains("<memory_board") {
        return raw.to_string();
    }
    raw.lines()
        .filter(|line| {
            let t = line.trim();
            !(t.starts_with("<keywords>")
                || t.starts_with("</keywords>")
                || t.starts_with("<reason>")
                || t.starts_with("</reason>"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn xml_escape_prompt(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_prompt(
    conversation: &Conversation,
    agent: &AgentProfile,
    user_name: &str,
    user_intro: &str,
    response_style_id: &str,
) -> PreparedPrompt {
    let latest_user_index = conversation.messages.iter().rposition(|m| m.role == "user");
    let mut history_messages = Vec::<PreparedHistoryMessage>::new();
    for (idx, message) in conversation.messages.iter().enumerate() {
        if Some(idx) == latest_user_index {
            continue;
        }
        if let Some(events) = &message.tool_call {
            for event in events {
                let event_role = event
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                if event_role == "assistant" {
                    let text = event
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let tool_calls = event
                        .get("tool_calls")
                        .and_then(Value::as_array)
                        .cloned();
                    let reasoning_content = event
                        .get("reasoning_content")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    history_messages.push(PreparedHistoryMessage {
                        role: "assistant".to_string(),
                        text,
                        tool_calls,
                        tool_call_id: None,
                        reasoning_content,
                    });
                } else if event_role == "tool" {
                    let text = event
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let tool_call_id = event
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    if !text.trim().is_empty() || tool_call_id.is_some() {
                        history_messages.push(PreparedHistoryMessage {
                            role: "tool".to_string(),
                            text,
                            tool_calls: None,
                            tool_call_id,
                            reasoning_content: None,
                        });
                    }
                }
            }
        }
        let role = message.role.trim().to_lowercase();
        if role != "user" && role != "assistant" {
            continue;
        }
        let text = render_message_content_for_model(message);
        if text.trim().is_empty() {
            continue;
        }
        history_messages.push(PreparedHistoryMessage {
            role,
            text,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
    }
    let user_intro_display = if user_intro.trim().is_empty() {
        "未提供".to_string()
    } else {
        user_intro.trim().to_string()
    };
    let response_style = response_style_preset(response_style_id);
    let highest_instruction_md = highest_instruction_markdown();

    let preamble = format!(
        "{}\n\
## 助理设定\n\
{}\n\
\n\
## 用户设定\n\
- 用户昵称：{}\n\
- 用户自我介绍：{}\n\
\n\
## 角色约束\n\
- 你是“{}”，用户是“{}”。\n\
- 不要把自己当作用户，不要混淆双方身份。\n\
\n\
## 对话风格\n\
- 当前风格：{}\n\
{}\n\
\n\
## 语言设定\n\
- 优先使用用户当前界面语言回答，保持简洁、自然、直接。\n\
- 若用户明确指定回答语言，以用户指定为准；未指定时保持会话语言一致。\n\
\n",
        highest_instruction_md,
        agent.system_prompt,
        xml_escape_prompt(user_name),
        xml_escape_prompt(&user_intro_display),
        agent.name,
        user_name,
        response_style.name,
        response_style.prompt
    );

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();

    let mut latest_user_text = String::new();
    let mut latest_user_system_text = String::new();
    let mut latest_images = Vec::<(String, String)>::new();
    let mut latest_audios = Vec::<(String, String)>::new();

    if let Some(msg) = latest_user {
        let ChatMessage {
            parts,
            extra_text_blocks,
            ..
        } = msg;
        for part in parts {
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
        for extra in extra_text_blocks {
            if extra.trim().is_empty() {
                continue;
            }
            let extra = sanitize_memory_block_xml(&extra);
            if extra.trim().is_empty() {
                continue;
            }
            if !latest_user_system_text.is_empty() {
                latest_user_system_text.push('\n');
            }
            latest_user_system_text.push_str(&extra);
        }
    }

    PreparedPrompt {
        preamble,
        history_messages,
        latest_user_text,
        latest_user_system_text,
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

