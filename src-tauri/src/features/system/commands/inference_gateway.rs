#[derive(Debug, Clone)]
struct CallPolicy {
    scene: &'static str,
    timeout_secs: Option<u64>,
    json_only: bool,
}

impl CallPolicy {
    fn archive_json(timeout_secs: u64) -> Self {
        Self {
            scene: "Archive summary",
            timeout_secs: Some(timeout_secs),
            json_only: true,
        }
    }

    fn debug_probe() -> Self {
        Self {
            scene: "Debug probe",
            timeout_secs: None,
            json_only: false,
        }
    }
}

async fn invoke_model_rig_by_format(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    match resolved_api.request_format {
        RequestFormat::OpenAI | RequestFormat::DeepSeekKimi => {
            call_model_openai_rig_style(resolved_api, model_name, prepared).await
        }
        RequestFormat::Gemini => call_model_gemini_rig_style(resolved_api, model_name, prepared).await,
        RequestFormat::Anthropic => {
            call_model_anthropic_rig_style(resolved_api, model_name, prepared).await
        }
        RequestFormat::OpenAITts
        | RequestFormat::OpenAIStt
        | RequestFormat::GeminiEmbedding
        | RequestFormat::OpenAIEmbedding
        | RequestFormat::OpenAIRerank => Err(format!(
            "Request format '{}' is not supported for this non-stream inference.",
            resolved_api.request_format
        )),
    }
}

async fn invoke_model_rig_by_format_with_timeout(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    timeout_secs: u64,
    scene: &str,
) -> Result<ModelReply, String> {
    let call_started = std::time::Instant::now();
    tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        invoke_model_rig_by_format(resolved_api, model_name, prepared),
    )
    .await
    .map_err(|_| {
        format!(
            "{scene} request timed out (elapsed={}ms, timeout={}s)",
            call_started.elapsed().as_millis(),
            timeout_secs
        )
    })?
}

async fn invoke_model_with_policy(
    resolved_api: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    policy: CallPolicy,
) -> Result<ModelReply, String> {
    if policy.json_only {
        // json_only is enforced by prompt contract + caller-side JSON parse.
    }
    if let Some(timeout_secs) = policy.timeout_secs {
        invoke_model_rig_by_format_with_timeout(
            resolved_api,
            model_name,
            prepared,
            timeout_secs,
            policy.scene,
        )
        .await
    } else {
        invoke_model_rig_by_format(resolved_api, model_name, prepared).await
    }
}

async fn call_archive_summary_model_with_timeout(
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    prepared: PreparedPrompt,
    timeout_secs: u64,
) -> Result<ModelReply, String> {
    invoke_model_with_policy(
        resolved_api,
        &selected_api.model,
        prepared,
        CallPolicy::archive_json(timeout_secs),
    )
    .await
}
