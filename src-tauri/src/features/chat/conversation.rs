fn latest_active_conversation_index(
    data: &AppData,
    _api_config_id: &str,
    agent_id: &str,
) -> Option<usize> {
    data.conversations
        .iter()
        .enumerate()
        .filter(|(_, c)| c.status == "active" && c.summary.trim().is_empty() && c.agent_id == agent_id)
        .max_by(|(idx_a, a), (idx_b, b)| {
            let a_updated = a.updated_at.trim();
            let b_updated = b.updated_at.trim();
            let a_created = a.created_at.trim();
            let b_created = b.created_at.trim();
            a_updated
                .cmp(b_updated)
                .then_with(|| a_created.cmp(b_created))
                .then_with(|| idx_a.cmp(idx_b))
        })
        .map(|(idx, _)| idx)
}

fn ensure_active_conversation_index(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    if let Some(idx) = latest_active_conversation_index(data, api_config_id, agent_id) {
        // Keep conversation metadata aligned with the currently selected chat API.
        if data.conversations[idx].api_config_id != api_config_id {
            data.conversations[idx].api_config_id = api_config_id.to_string();
            data.conversations[idx].updated_at = now_iso();
        }
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
        summary: String::new(),
        archived_at: None,
        messages: Vec::new(),
        memory_recall_table: Vec::new(),
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
    let conv = data.conversations.get_mut(idx)?;
    let now = now_iso();
    conv.status = "archived".to_string();
    conv.summary = summary.to_string();
    conv.archived_at = Some(now.clone());
    conv.updated_at = now;
    let archive_id = conv.id.clone();
    let _ = reason;
    clear_screenshot_artifact_cache();
    Some(archive_id)
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

fn is_supported_image_upload_mime(mime: &str) -> bool {
    matches!(
        mime.trim().to_ascii_lowercase().as_str(),
        "image/jpeg"
            | "image/jpg"
            | "image/png"
            | "image/gif"
            | "image/webp"
            | "image/heic"
            | "image/heif"
            | "image/svg+xml"
            | "application/pdf"
    )
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
            let mime = image.mime.trim().to_ascii_lowercase();
            if !is_supported_image_upload_mime(&mime) {
                return Err(format!(
                    "Unsupported attachment mime type: '{}'.",
                    image.mime.trim()
                ));
            }
            let bytes_base64 = image.bytes_base64.trim();
            let raw = B64
                .decode(bytes_base64)
                .map_err(|err| format!("Decode image base64 failed: {err}"))?;
            if mime == "application/pdf" {
                if !api_config.request_format.is_gemini() {
                    return Err(
                        "PDF attachment is only supported when request format is 'gemini'."
                            .to_string(),
                    );
                }
                total_binary += raw.len();
                parts.push(MessagePart::Image {
                    mime: "application/pdf".to_string(),
                    bytes_base64: bytes_base64.to_string(),
                    name: None,
                    compressed: false,
                });
            } else {
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
    }

    if let Some(audios) = &payload.audios {
        if !audios.is_empty() && !api_config.enable_audio {
            return Err("Current API config has audio disabled.".to_string());
        }

        for audio in audios {
            let bytes_base64 = audio.bytes_base64.trim();
            let raw = B64
                .decode(bytes_base64)
                .map_err(|err| format!("Decode audio base64 failed: {err}"))?;
            total_binary += raw.len();
            parts.push(MessagePart::Audio {
                mime: audio.mime.trim().to_string(),
                bytes_base64: bytes_base64.to_string(),
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
            MessagePart::Image { mime, .. } => {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    chunks.push("[pdf attached]".to_string());
                } else {
                    chunks.push("[image attached]".to_string());
                }
            }
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
            MessagePart::Image { mime, .. } => {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    chunks.push("[pdf attached]".to_string());
                } else {
                    chunks.push("[image attached]".to_string());
                }
            }
            MessagePart::Audio { .. } => chunks.push("[audio attached]".to_string()),
        }
    }
    chunks.join(" | ")
}

fn sanitize_memory_block_xml(raw: &str) -> String {
    if !raw.contains("<memory_board") && !raw.contains("[MemoryBoard]") {
        return raw.to_string();
    }
    raw.lines()
        .filter(|line| {
            let t = line.trim();
            !(t.contains("<keywords>")
                || t.contains("</keywords>")
                || t.contains("<reason>")
                || t.contains("</reason>"))
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
    ui_language: &str,
    data_path: Option<&PathBuf>,
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
                        user_time_text: None,
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
                            user_time_text: None,
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
            role: role.clone(),
            text,
            user_time_text: if role == "user" {
                let t = format_message_time_text(&message.created_at);
                if t.is_empty() { None } else { Some(t) }
            } else {
                None
            },
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
    }
    let response_style = response_style_preset(response_style_id);
    let highest_instruction_md = highest_instruction_markdown();
    let (
        not_provided_label,
        assistant_settings_label,
        user_settings_label,
        role_constraints_label,
        conversation_style_label,
        language_settings_label,
        user_nickname_label,
        user_intro_label,
        role_identity_line,
        role_confusion_line,
        language_follow_user_line,
        language_instruction,
    ) = match ui_language.trim() {
        "en-US" => (
            "Not provided",
            "Assistant settings",
            "User settings",
            "Role constraints",
            "Conversation style",
            "Language settings",
            "User nickname",
            "User self-introduction",
            "- You are \"{}\", and the user is \"{}\".",
            "- Do not treat yourself as the user, and do not confuse the two roles.",
            "- If the user explicitly requests a reply language, follow the user's request.",
            "Please respond in English by default.",
        ),
        "zh-TW" => (
            "未提供",
            "助理設定",
            "使用者設定",
            "角色約束",
            "對話風格",
            "語言設定",
            "使用者暱稱",
            "使用者自我介紹",
            "- 你是「{}」，使用者是「{}」。",
            "- 不要把自己當作使用者，不要混淆雙方身分。",
            "- 若使用者明確指定回答語言，以使用者指定為準。",
            "預設使用繁體中文回答。",
        ),
        _ => (
            "未提供",
            "助理设定",
            "用户设定",
            "角色约束",
            "对话风格",
            "语言设定",
            "用户昵称",
            "用户自我介绍",
            "- 你是“{}”，用户是“{}”。",
            "- 不要把自己当作用户，不要混淆双方身份。",
            "- 若用户明确指定回答语言，以用户指定为准。",
            "默认使用中文回答。",
        ),
    };
    let user_intro_display = if user_intro.trim().is_empty() {
        not_provided_label.to_string()
    } else {
        user_intro.trim().to_string()
    };
    let role_identity_text = role_identity_line
        .replacen("{}", &xml_escape_prompt(&agent.name), 1)
        .replacen("{}", &xml_escape_prompt(user_name), 1);

    let preamble = format!(
        "{}\n\
## {}\n\
{}\n\
\n\
## {}\n\
- {}：{}\n\
- {}：{}\n\
\n\
## {}\n\
{}\n\
{}\n\
\n\
## {}\n\
- 当前风格：{}\n\
{}\n\
\n\
## {}\n\
- {}\n\
- {}\n\
\n",
        highest_instruction_md,
        assistant_settings_label,
        agent.system_prompt,
        user_settings_label,
        user_nickname_label,
        xml_escape_prompt(user_name),
        user_intro_label,
        xml_escape_prompt(&user_intro_display),
        role_constraints_label,
        role_identity_text,
        role_confusion_line,
        conversation_style_label,
        response_style.name,
        response_style.prompt,
        language_settings_label,
        language_instruction,
        language_follow_user_line
    );

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();

    let mut latest_user_text = String::new();
    let mut latest_user_time_text = String::new();
    let mut latest_user_system_text = String::new();
    let mut latest_images = Vec::<(String, String)>::new();
    let mut latest_audios = Vec::<(String, String)>::new();

    if let Some(msg) = latest_user {
        let ChatMessage {
            created_at,
            parts,
            extra_text_blocks,
            ..
        } = msg;
        latest_user_time_text = format_message_time_text(&created_at);
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
                } => {
                    let resolved = if let Some(path) = data_path {
                        match resolve_stored_binary_base64(path, &bytes_base64) {
                            Ok(value) => value,
                            Err(err) => {
                                eprintln!(
                                    "[PROMPT] resolve image attachment failed (mime={}, data_path={}): {err}",
                                    mime,
                                    path.to_string_lossy()
                                );
                                bytes_base64
                            }
                        }
                    } else {
                        bytes_base64
                    };
                    if !resolved.trim().is_empty() {
                        latest_images.push((mime, resolved));
                    }
                }
                MessagePart::Audio {
                    mime, bytes_base64, ..
                } => {
                    let resolved = if let Some(path) = data_path {
                        match resolve_stored_binary_base64(path, &bytes_base64) {
                            Ok(value) => value,
                            Err(err) => {
                                eprintln!(
                                    "[PROMPT] resolve audio attachment failed (mime={}, data_path={}): {err}",
                                    mime,
                                    path.to_string_lossy()
                                );
                                bytes_base64
                            }
                        }
                    } else {
                        bytes_base64
                    };
                    if !resolved.trim().is_empty() {
                        latest_audios.push((mime, resolved));
                    }
                }
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
        latest_user_time_text,
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
