fn candidate_stt_urls(base_url: &str) -> Vec<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Vec::new();
    }
    let lower = base.to_ascii_lowercase();
    let mut urls = Vec::new();
    if lower.ends_with("/audio/transcriptions") {
        urls.push(base.to_string());
    } else if lower.ends_with("/v1") {
        urls.push(format!("{base}/audio/transcriptions"));
    } else {
        urls.push(format!("{base}/audio/transcriptions"));
        urls.push(format!("{base}/v1/audio/transcriptions"));
    }
    urls.sort();
    urls.dedup();
    urls
}

async fn call_openai_stt_transcribe(
    api_config: &ApiConfig,
    mime: &str,
    audio_raw: Vec<u8>,
) -> Result<String, String> {
    let model = api_config.model.trim();
    if model.is_empty() {
        return Err("STT model is empty.".to_string());
    }
    if api_config.api_key.trim().is_empty() {
        return Err("STT API key is empty.".to_string());
    }
    let urls = candidate_stt_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("STT base URL is empty.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Build STT HTTP client failed: {err}"))?;

    let mut errors = Vec::new();
    for url in urls {
        let file_part = reqwest::multipart::Part::bytes(audio_raw.clone())
            .file_name("speech.webm")
            .mime_str(if mime.trim().is_empty() {
                "audio/webm"
            } else {
                mime.trim()
            })
            .map_err(|err| format!("Build STT mime part failed: {err}"))?;
        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", model.to_string());
        let resp = client
            .post(&url)
            .bearer_auth(api_config.api_key.trim())
            .multipart(form)
            .send()
            .await;
        let Ok(resp) = resp else {
            errors.push(format!("{url} -> request failed"));
            continue;
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let raw = resp.text().await.unwrap_or_default();
            errors.push(format!(
                "{url} -> {status}: {}",
                raw.chars().take(220).collect::<String>()
            ));
            continue;
        }
        let body = resp
            .json::<Value>()
            .await
            .map_err(|err| format!("Parse STT response failed: {err}"))?;
        if let Some(text) = body.get("text").and_then(Value::as_str) {
            return Ok(text.trim().to_string());
        }
        if let Some(text) = body.get("transcript").and_then(Value::as_str) {
            return Ok(text.trim().to_string());
        }
        return Err(format!(
            "STT response does not contain text field: {}",
            body.to_string().chars().take(220).collect::<String>()
        ));
    }

    Err(format!(
        "STT request failed for all candidate URLs: {}",
        errors.join(" || ")
    ))
}

#[tauri::command]
async fn stt_transcribe(
    input: SttTranscribeInput,
    state: State<'_, AppState>,
) -> Result<SttTranscribeOutput, String> {
    if input.bytes_base64.trim().is_empty() {
        return Err("Audio payload is empty.".to_string());
    }

    let app_config = {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let cfg = read_config(&state.config_path)?;
        drop(guard);
        cfg
    };

    let selected_id = input
        .stt_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or(app_config.stt_api_config_id.as_deref())
        .ok_or_else(|| "No STT API selected. Using local transcription only.".to_string())?;
    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == selected_id)
        .cloned()
        .ok_or_else(|| "Selected STT API config not found.".to_string())?;
    if !api.request_format.is_openai_stt() {
        return Err("Selected STT API must use request_format='openai_stt'.".to_string());
    }

    let audio_raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode audio base64 failed: {err}"))?;
    let text = call_openai_stt_transcribe(&api, &input.mime, audio_raw).await?;
    Ok(SttTranscribeOutput { text })
}


