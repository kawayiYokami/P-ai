#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    reasoning_standard: String,
    reasoning_inline: String,
    tool_history_events: Vec<Value>,
}

async fn call_model_openai_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let mut content_items: Vec<UserContent> = Vec::new();
    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text));
    }
    if !prepared.latest_user_time_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_time_text));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_system_text));
    }

    for (mime, bytes) in prepared.latest_images {
        content_items.push(UserContent::image_base64(
            bytes,
            image_media_type_from_mime(&mime),
            Some(ImageDetail::Auto),
        ));
    }

    for (mime, bytes) in prepared.latest_audios {
        content_items.push(UserContent::audio(bytes, audio_media_type_from_mime(&mime)));
    }

    let prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;

    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;

    let agent = client
        .completions_api()
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .build();
    let prompt_message = RigMessage::User {
        content: prompt_content,
    };

    let assistant_text = agent
        .prompt(prompt_message)
        .await
        .map_err(|err| err.to_string())?;
    Ok(ModelReply {
        assistant_text,
        reasoning_standard: String::new(),
        reasoning_inline: String::new(),
        tool_history_events: Vec::new(),
    })
}

fn normalize_gemini_rig_base_url(raw: &str) -> String {
    let mut base = raw.trim().trim_end_matches('/').to_string();
    for suffix in ["/v1beta/openai", "/v1beta", "/openai"] {
        if base.ends_with(suffix) {
            base = base.trim_end_matches(suffix).trim_end_matches('/').to_string();
            break;
        }
    }
    base
}

fn has_alpha_unit_with_boundaries(text: &str, unit: &str) -> bool {
    if unit.is_empty() {
        return false;
    }
    let source = text.as_bytes();
    for (idx, _) in text.match_indices(unit) {
        let prev_ok = if idx == 0 {
            true
        } else {
            !source[idx - 1].is_ascii_alphanumeric()
        };
        let end = idx + unit.len();
        let next_ok = if end >= source.len() {
            true
        } else {
            !source[end].is_ascii_alphanumeric()
        };
        if prev_ok && next_ok {
            return true;
        }
    }
    false
}

fn has_numeric_unit_with_boundaries(text: &str, unit: &str) -> bool {
    if unit.is_empty() {
        return false;
    }
    let source = text.as_bytes();
    for (idx, _) in text.match_indices(unit) {
        let prev_ok = if idx == 0 {
            true
        } else {
            !source[idx - 1].is_ascii_digit()
        };
        let end = idx + unit.len();
        let next_ok = if end >= source.len() {
            true
        } else {
            !source[end].is_ascii_digit()
        };
        if prev_ok && next_ok {
            return true;
        }
    }
    false
}

fn gemini_additional_params_for_model(
    model_name: &str,
    api_config: &ResolvedApiConfig,
) -> Value {
    let mut params = serde_json::Map::new();
    params.insert(
        "safetySettings".to_string(),
        serde_json::json!([
            {
                "category": "HARM_CATEGORY_HARASSMENT",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_HATE_SPEECH",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                "threshold": "BLOCK_NONE"
            }
        ]),
    );
    let normalized = model_name.trim().to_ascii_lowercase();
    let is_gemini = has_alpha_unit_with_boundaries(&normalized, "gemini");
    let is_v25 = is_gemini && has_numeric_unit_with_boundaries(&normalized, "2.5");
    let is_v3 = is_gemini && has_numeric_unit_with_boundaries(&normalized, "3");
    let is_flash = has_alpha_unit_with_boundaries(&normalized, "flash");

    let selected_level = api_config.gemini_thinking_level;
    let effective_v3_level = if is_v3 && is_flash {
        match selected_level {
            GeminiThinkingLevel::Minimal
            | GeminiThinkingLevel::Low
            | GeminiThinkingLevel::Medium
            | GeminiThinkingLevel::High => selected_level,
            GeminiThinkingLevel::Dynamic => GeminiThinkingLevel::High,
        }
    } else {
        match selected_level {
            GeminiThinkingLevel::Low | GeminiThinkingLevel::High => selected_level,
            GeminiThinkingLevel::Minimal
            | GeminiThinkingLevel::Medium
            | GeminiThinkingLevel::Dynamic => GeminiThinkingLevel::High,
        }
    };
    let v3_budget_from_level = match effective_v3_level {
        GeminiThinkingLevel::Minimal => 0,
        GeminiThinkingLevel::Low => 512,
        GeminiThinkingLevel::Medium => 2048,
        GeminiThinkingLevel::High | GeminiThinkingLevel::Dynamic => -1,
    };
    let thinking_config =
        if is_v25 {
            Some(serde_json::json!({
                "includeThoughts": true,
                "thinkingBudget": api_config.gemini_thinking_budget.max(-1)
            }))
        } else if is_v3 {
            Some(serde_json::json!({
                // rig current gemini parser expects thinkingBudget to exist;
                // include both so Gemini 3 level control remains available.
                "includeThoughts": true,
                "thinkingLevel": effective_v3_level.as_str(),
                "thinkingBudget": v3_budget_from_level
            }))
        } else {
            None
        };
    if let Some(thinking_config) = thinking_config {
        params.insert(
            "generationConfig".to_string(),
            serde_json::json!({
                "thinkingConfig": thinking_config
            }),
        );
    }
    Value::Object(params)
}

async fn call_model_gemini_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String> {
    let mut client_builder = gemini::Client::builder().api_key(&api_config.api_key);
    let normalized_base = normalize_gemini_rig_base_url(&api_config.base_url);
    if !normalized_base.is_empty() {
        client_builder = client_builder.base_url(&normalized_base);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Gemini client via rig: {err}"))?;

    let gemini_additional_params = gemini_additional_params_for_model(model_name, api_config);

    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .additional_params(gemini_additional_params)
        .build();

    let mut content_items: Vec<UserContent> = Vec::new();
    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text));
    }
    if !prepared.latest_user_time_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_time_text));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_system_text));
    }

    for (mime, bytes) in prepared.latest_images {
        if mime.trim().eq_ignore_ascii_case("application/pdf") {
            content_items.push(UserContent::document(bytes, Some(DocumentMediaType::PDF)));
        } else {
            content_items.push(UserContent::image_base64(
                bytes,
                image_media_type_from_mime(&mime),
                Some(ImageDetail::Auto),
            ));
        }
    }

    for (mime, bytes) in prepared.latest_audios {
        content_items.push(UserContent::audio(bytes, audio_media_type_from_mime(&mime)));
    }

    let prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;

    let prompt_message = RigMessage::User {
        content: prompt_content,
    };

    let mut stream = agent
        .stream_completion(prompt_message, Vec::<RigMessage>::new())
        .await
        .map_err(|err| format!("rig stream completion build failed: {err}"))?
        .stream()
        .await
        .map_err(|err| format!("rig stream start failed: {err}"))?;

    let mut assistant_text = String::new();
    let mut reasoning_standard = String::new();
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        match chunk {
            Ok(StreamedAssistantContent::Text(text)) => {
                if let Some(ch) = on_delta {
                    let _ = ch.send(AssistantDeltaEvent {
                        delta: text.text.clone(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                }
                assistant_text.push_str(&text.text);
            }
            Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                let merged = reasoning.reasoning.join("\n");
                if !merged.is_empty() {
                    if let Some(ch) = on_delta {
                        let _ = ch.send(AssistantDeltaEvent {
                            delta: merged.clone(),
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                    reasoning_standard.push_str(&merged);
                }
            }
            Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                if !reasoning.is_empty() {
                    if let Some(ch) = on_delta {
                        let _ = ch.send(AssistantDeltaEvent {
                            delta: reasoning.clone(),
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                    reasoning_standard.push_str(&reasoning);
                }
            }
            Ok(StreamedAssistantContent::Final(_))
            | Ok(StreamedAssistantContent::ToolCall { .. })
            | Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
            Err(err) => return Err(format!("rig streaming failed: {err}")),
        }
    }
    Ok(ModelReply {
        assistant_text,
        reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events: Vec::new(),
    })
}

async fn call_model_anthropic_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let mut content_items: Vec<UserContent> = Vec::new();
    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text));
    }
    if !prepared.latest_user_time_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_time_text));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_system_text));
    }

    for (mime, bytes) in prepared.latest_images {
        content_items.push(UserContent::image_base64(
            bytes,
            image_media_type_from_mime(&mime),
            Some(ImageDetail::Auto),
        ));
    }

    for (mime, bytes) in prepared.latest_audios {
        content_items.push(UserContent::audio(bytes, audio_media_type_from_mime(&mime)));
    }

    let prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;

    let mut client_builder: anthropic::ClientBuilder =
        anthropic::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Anthropic client via rig: {err}"))?;

    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .build();
    let prompt_message = RigMessage::User {
        content: prompt_content,
    };

    let assistant_text = agent
        .prompt(prompt_message)
        .await
        .map_err(|err| err.to_string())?;
    Ok(ModelReply {
        assistant_text,
        reasoning_standard: String::new(),
        reasoning_inline: String::new(),
        tool_history_events: Vec::new(),
    })
}

fn debug_value_snippet(value: &Value, max_chars: usize) -> String {
    let raw = serde_json::to_string(value).unwrap_or_else(|_| "<invalid json>".to_string());
    if raw.chars().count() <= max_chars {
        raw
    } else {
        let head = raw.chars().take(max_chars).collect::<String>();
        format!("{head}...")
    }
}

fn send_tool_status_event(
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    tool_name: &str,
    tool_status: &str,
    message: &str,
) {
    let send_result = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
        tool_name: Some(tool_name.to_string()),
        tool_status: Some(tool_status.to_string()),
        message: Some(message.to_string()),
    });
    eprintln!(
        "[TOOL-DEBUG] tool_status_event send={:?} name={} status={} message={}",
        send_result, tool_name, tool_status, message
    );
}

fn tool_enabled(selected_api: &ApiConfig, id: &str) -> bool {
    selected_api.enable_tools
        && selected_api
            .tools
            .iter()
            .any(|tool| tool.id == id && tool.enabled)
}

#[derive(Debug)]
struct ToolInvokeError(String);

impl std::fmt::Display for ToolInvokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolInvokeError {}

impl From<String> for ToolInvokeError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_image_unsupported_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("unknown variant `image_url`")
        || lower.contains("expected `text`")
        || lower.contains("does not support image")
        || lower.contains("image input")
}

fn truncate_by_chars(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}

async fn builtin_fetch(url: &str, max_length: usize) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let resp = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .send()
        .await
        .map_err(|err| format!("Fetch url failed: {err}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("Fetch url failed with status {status}"));
    }
    let html = resp
        .text()
        .await
        .map_err(|err| format!("Read body failed: {err}"))?;
    let document = Html::parse_document(&html);
    let body_selector =
        Selector::parse("body").map_err(|err| format!("Parse selector failed: {err}"))?;
    let raw = document
        .select(&body_selector)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| document.root_element().text().collect::<Vec<_>>().join(" "));
    let cleaned = clean_text(&raw);
    let truncated = truncate_by_chars(&cleaned, max_length);
    Ok(serde_json::json!({
      "url": url,
      "content": truncated
    }))
}

async fn builtin_bing_search(query: &str, num_results: usize) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let mut last_error: Option<String> = None;
    for base in ["https://cn.bing.com", "https://www.bing.com"] {
        let url = format!("{base}/search?q={}", urlencoding::encode(query));
        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await;
        let Ok(resp) = resp else {
            last_error = Some("request failed".to_string());
            continue;
        };
        if !resp.status().is_success() {
            last_error = Some(format!("status {}", resp.status()));
            continue;
        }
        let html = resp
            .text()
            .await
            .map_err(|err| format!("Read search body failed: {err}"))?;
        let doc = Html::parse_document(&html);
        let item_sel =
            Selector::parse("li.b_algo").map_err(|err| format!("Parse selector failed: {err}"))?;
        let title_sel =
            Selector::parse("h2").map_err(|err| format!("Parse selector failed: {err}"))?;
        let a_sel =
            Selector::parse("h2 a").map_err(|err| format!("Parse selector failed: {err}"))?;
        let p_sel = Selector::parse("p").map_err(|err| format!("Parse selector failed: {err}"))?;
        let mut rows = Vec::new();
        for item in doc.select(&item_sel).take(num_results.max(1)) {
            let title = item
                .select(&title_sel)
                .next()
                .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            let link = item
                .select(&a_sel)
                .next()
                .and_then(|n| n.value().attr("href"))
                .unwrap_or_default()
                .to_string();
            let snippet = item
                .select(&p_sel)
                .next()
                .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            if !title.is_empty() && !link.is_empty() {
                rows.push(serde_json::json!({"title": title, "url": link, "snippet": snippet}));
            }
        }
        if !rows.is_empty() {
            return Ok(serde_json::json!({"query": query, "results": rows}));
        }
        last_error = Some("no results parsed".to_string());
    }
    Err(format!(
        "bing search failed: {}",
        last_error.unwrap_or_else(|| "unknown".to_string())
    ))
}

fn normalize_memory_keywords(raw: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for item in raw {
        let v = item.trim().to_lowercase();
        if v.len() < 2 {
            continue;
        }
        if !out.iter().any(|x| x == &v) {
            out.push(v);
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

fn memory_contains_sensitive(content: &str, keywords: &[String]) -> bool {
    let mut full = content.to_lowercase();
    if !keywords.is_empty() {
        full.push('\n');
        full.push_str(&keywords.join(" ").to_lowercase());
    }
    let danger_tokens = [
        "password",
        "passwd",
        "api key",
        "apikey",
        "token",
        "secret",
        "private key",
        "sk-",
        "ssh-rsa",
        "验证码",
        "密码",
        "密钥",
        "身份证",
        "银行卡",
        "cvv",
    ];
    danger_tokens.iter().any(|token| full.contains(token))
}

#[derive(Debug, Clone)]
struct MemorySaveDraft {
    content: String,
    keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MemorySaveUpsertItemResult {
    saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keywords: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn parse_memory_save_draft(content: &str, keywords_raw: Vec<String>) -> Result<MemorySaveDraft, String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("memory_save.content is required".to_string());
    }
    let keywords = normalize_memory_keywords(&keywords_raw);
    if keywords.is_empty() {
        return Err("memory_save.keywords must contain at least one valid keyword".to_string());
    }
    Ok(MemorySaveDraft {
        content: content.to_string(),
        keywords,
    })
}

fn upsert_memories(
    app_state: &AppState,
    drafts: &[MemorySaveDraft],
) -> Result<(Vec<MemorySaveUpsertItemResult>, usize), String> {
    let guard = app_state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&app_state.data_path)?;
    let now = now_iso();
    let mut results = Vec::<MemorySaveUpsertItemResult>::new();
    let mut changed = 0usize;

    for draft in drafts {
        if memory_contains_sensitive(&draft.content, &draft.keywords) {
            eprintln!(
                "[TOOL-DEBUG] memory-save rejected sensitive content. keywords={}",
                draft.keywords.join(",")
            );
            results.push(MemorySaveUpsertItemResult {
                saved: false,
                id: None,
                keywords: None,
                updated_at: None,
                reason: Some("sensitive_rejected".to_string()),
            });
            continue;
        }

        let memory_id = if let Some(existing) = data
            .memories
            .iter_mut()
            .find(|m| m.content.trim() == draft.content)
        {
            existing.keywords = draft.keywords.clone();
            existing.updated_at = now.clone();
            existing.id.clone()
        } else {
            data.memories.push(MemoryEntry {
                id: Uuid::new_v4().to_string(),
                content: draft.content.clone(),
                keywords: draft.keywords.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
            });
            data.memories
                .last()
                .map(|m| m.id.clone())
                .unwrap_or_else(|| "created".to_string())
        };
        changed += 1;
        results.push(MemorySaveUpsertItemResult {
            saved: true,
            id: Some(memory_id.clone()),
            keywords: Some(draft.keywords.clone()),
            updated_at: Some(now.clone()),
            reason: None,
        });
        eprintln!(
            "[TOOL-DEBUG] memory-save saved. id={}, keywords={}, content_len={}",
            memory_id,
            draft.keywords.join(","),
            draft.content.chars().count()
        );
    }

    if changed > 0 {
        write_app_data(&app_state.data_path, &data)?;
        invalidate_memory_matcher_cache();
    }
    let total_memories = data.memories.len();
    drop(guard);
    Ok((results, total_memories))
}

fn builtin_memory_save(app_state: &AppState, args: Value) -> Result<Value, String> {
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_save.content is required".to_string())?;
    let keywords_raw = args
        .get("keywords")
        .and_then(Value::as_array)
        .ok_or_else(|| "memory_save.keywords is required".to_string())?
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let draft = parse_memory_save_draft(content, keywords_raw)?;
    let (results, total_memories) = upsert_memories(app_state, &[draft])?;
    let first = results
        .into_iter()
        .next()
        .ok_or_else(|| "memory_save failed to produce result".to_string())?;
    Ok(serde_json::json!({
      "saved": first.saved,
      "id": first.id,
      "keywords": first.keywords,
      "updatedAt": first.updated_at,
      "reason": first.reason,
      "totalMemories": total_memories
    }))
}

fn builtin_memory_save_batch(app_state: &AppState, args: Value) -> Result<Value, String> {
    const MAX_BATCH_ITEMS: usize = 7;
    let memories = args
        .get("memories")
        .and_then(Value::as_array)
        .ok_or_else(|| "memory_save_batch.memories is required".to_string())?;
    if memories.is_empty() {
        return Err("memory_save_batch.memories must not be empty".to_string());
    }

    let mut drafts = Vec::<MemorySaveDraft>::new();
    let mut truncated = false;
    for item in memories {
        if drafts.len() >= MAX_BATCH_ITEMS {
            truncated = true;
            break;
        }
        let content = item
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| "memory_save_batch.memories[].content is required".to_string())?;
        let keywords = item
            .get("keywords")
            .and_then(Value::as_array)
            .ok_or_else(|| "memory_save_batch.memories[].keywords is required".to_string())?
            .iter()
            .filter_map(Value::as_str)
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        drafts.push(parse_memory_save_draft(content, keywords)?);
    }

    let (items, total_memories) = upsert_memories(app_state, &drafts)?;
    let accepted = items.iter().filter(|it| it.saved).count();
    let rejected = items.len().saturating_sub(accepted);
    Ok(serde_json::json!({
      "saved": accepted > 0,
      "accepted": accepted,
      "rejected": rejected,
      "truncated": truncated,
      "items": items,
      "totalMemories": total_memories
    }))
}

async fn builtin_desktop_screenshot(webp_quality: Option<f32>) -> Result<Value, String> {
    let res = run_screenshot_tool(ScreenshotRequest {
        mode: ScreenshotMode::Desktop,
        monitor_id: None,
        region: None,
        save_path: None,
        webp_quality: normalize_webp_quality(webp_quality),
    })
    .await
    .map_err(|err| to_tool_err_string(&err))?;
    let mut value =
        serde_json::to_value(&res).map_err(|err| format!("serialize desktop screenshot result failed: {err}"))?;
    if let Some(obj) = value.as_object_mut() {
        let image_mime = res.image_mime.clone();
        let image_base64 = res.image_base64.clone();
        obj.insert(
            "response".to_string(),
            serde_json::json!({
                "ok": true,
                "width": res.width,
                "height": res.height,
                "imageMime": image_mime.clone(),
                "elapsedMs": res.elapsed_ms
            }),
        );
        obj.insert(
            "parts".to_string(),
            serde_json::json!([
                {
                    "type": "image",
                    "data": image_base64,
                    "mimeType": image_mime
                }
            ]),
        );
    }
    Ok(value)
}

async fn builtin_desktop_wait(ms: u64) -> Result<Value, String> {
    let res = run_wait_tool(WaitRequest {
        mode: WaitMode::Sleep,
        ms,
    })
    .await
    .map_err(|err| to_tool_err_string(&err))?;
    serde_json::to_value(res).map_err(|err| format!("serialize desktop wait result failed: {err}"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FetchToolArgs {
    url: String,
    #[serde(default)]
    max_length: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BingSearchToolArgs {
    query: String,
    #[serde(default)]
    num_results: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MemorySaveToolArgs {
    content: String,
    keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MemorySaveBatchItemArgs {
    content: String,
    keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MemorySaveBatchToolArgs {
    memories: Vec<MemorySaveBatchItemArgs>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DesktopScreenshotToolArgs {
    #[serde(default)]
    webp_quality: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DesktopWaitToolArgs {
    ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct BuiltinFetchTool;

impl Tool for BuiltinFetchTool {
    const NAME: &'static str = "fetch";
    type Error = ToolInvokeError;
    type Args = FetchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "fetch".to_string(),
            description: "Fetch webpage text.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "url": { "type": "string", "description": "URL" },
                "max_length": { "type": "integer", "description": "Max chars", "default": 1800 }
              },
              "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=fetch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_fetch(&args.url, args.max_length.unwrap_or(1800))
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=fetch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=fetch err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinBingSearchTool;

impl Tool for BuiltinBingSearchTool {
    const NAME: &'static str = "bing_search";
    type Error = ToolInvokeError;
    type Args = BingSearchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "bing_search".to_string(),
            description: "Search web with Bing.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "Query" },
                "num_results": { "type": "integer", "description": "Result count", "default": 5 }
              },
              "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=bing-search args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_bing_search(&args.query, args.num_results.unwrap_or(5))
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=bing-search result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=bing-search err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinMemorySaveTool {
    app_state: AppState,
}

impl Tool for BuiltinMemorySaveTool {
    const NAME: &'static str = "memory_save";
    type Error = ToolInvokeError;
    type Args = MemorySaveToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_save".to_string(),
            description: "保存与用户相关、长期有价值的记忆。禁止保存密码、密钥等敏感信息。"
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "content": { "type": "string", "description": "记忆正文，简洁具体" },
                "keywords": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "关键词列表，用于后续命中提示板"
                }
              },
              "required": ["content", "keywords"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::json!({
            "content": args.content,
            "keywords": args.keywords,
        });
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=memory-save args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_memory_save(&self.app_state, args_json).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=memory-save result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=memory-save err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinMemorySaveBatchTool {
    app_state: AppState,
}

impl Tool for BuiltinMemorySaveBatchTool {
    const NAME: &'static str = "memory_save_batch";
    type Error = ToolInvokeError;
    type Args = MemorySaveBatchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_save_batch".to_string(),
            description: "批量保存与用户相关、长期有价值的记忆（单次最多 7 条）。禁止保存敏感信息。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "memories": {
                  "type": "array",
                  "maxItems": 7,
                  "items": {
                    "type": "object",
                    "properties": {
                      "content": { "type": "string" },
                      "keywords": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["content", "keywords"]
                  }
                }
              },
              "required": ["memories"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::json!({
            "memories": args.memories,
        });
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=memory-save-batch args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result =
            builtin_memory_save_batch(&self.app_state, args_json).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=memory-save-batch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=memory-save-batch err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinDesktopScreenshotTool;

impl Tool for BuiltinDesktopScreenshotTool {
    const NAME: &'static str = "desktop_screenshot";
    type Error = ToolInvokeError;
    type Args = DesktopScreenshotToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_screenshot".to_string(),
            description: "Capture current full desktop screenshot and return metadata + image base64.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "webp_quality": { "type": "number", "minimum": 1, "maximum": 100, "default": 75 }
              }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=desktop-screenshot args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_desktop_screenshot(args.webp_quality)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=desktop-screenshot result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=desktop-screenshot err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinDesktopWaitTool;

impl Tool for BuiltinDesktopWaitTool {
    const NAME: &'static str = "desktop_wait";
    type Error = ToolInvokeError;
    type Args = DesktopWaitToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_wait".to_string(),
            description: "Wait for specified milliseconds.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "ms": { "type": "integer", "minimum": 1, "maximum": 120000, "description": "wait milliseconds" }
              },
              "required": ["ms"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=desktop-wait args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_desktop_wait(args.ms).await.map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=desktop-wait result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=desktop-wait err={err}"),
        }
        result
    }
}

