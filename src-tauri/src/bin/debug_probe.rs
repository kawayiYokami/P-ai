use std::{fs, path::PathBuf};

use rig::{
    completion::{message::UserContent, Message, Prompt},
    prelude::CompletionClient,
    providers::openai,
    OneOrMany,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugApiConfig {
    provider: Option<String>,
    base_url: String,
    api_key: String,
    model: String,
    fixed_test_prompt: Option<String>,
    enabled: Option<bool>,
}

fn load_debug_config() -> Result<DebugApiConfig, String> {
    let candidates = [
        PathBuf::from(".debug").join("api-key.json"),
        PathBuf::from("..").join(".debug").join("api-key.json"),
    ];

    for path in candidates {
        if path.exists() {
            let content = fs::read_to_string(&path)
                .map_err(|err| format!("Read {} failed: {err}", path.display()))?;
            let cfg = serde_json::from_str::<DebugApiConfig>(&content)
                .map_err(|err| format!("Parse {} failed: {err}", path.display()))?;
            return Ok(cfg);
        }
    }

    Err(".debug/api-key.json not found".to_string())
}

fn main() {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("[probe] create tokio runtime failed: {err}");
            std::process::exit(1);
        }
    };

    rt.block_on(async_main());
}

async fn async_main() {
    let cfg = match load_debug_config() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("[probe] {err}");
            std::process::exit(1);
        }
    };

    if !cfg.enabled.unwrap_or(true) {
        eprintln!("[probe] .debug/api-key.json has enabled=false");
        std::process::exit(1);
    }

    let provider = cfg
        .provider
        .as_deref()
        .map(str::trim)
        .unwrap_or("openai_compatible");
    if !provider.eq_ignore_ascii_case("openai_compatible") {
        eprintln!("[probe] unsupported provider: {provider}");
        std::process::exit(1);
    }

    if cfg.api_key.trim().is_empty() {
        eprintln!("[probe] apiKey is empty in .debug/api-key.json");
        std::process::exit(1);
    }

    let fixed_prompt = cfg
        .fixed_test_prompt
        .unwrap_or_else(|| "EASY_CALL_AI_CACHE_TEST_V1".to_string());

    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(cfg.api_key.trim());
    if !cfg.base_url.trim().is_empty() {
        client_builder = client_builder.base_url(cfg.base_url.trim());
    }

    let client = match client_builder.build() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[probe] create client failed: {err}");
            std::process::exit(1);
        }
    };

    let agent = client.completions_api().agent(cfg.model.trim()).build();
    let prompt = Message::User {
        content: OneOrMany::one(UserContent::text(fixed_prompt.clone())),
    };

    println!("[probe] sending fixed prompt: {}", fixed_prompt);
    match agent.prompt(prompt).await {
        Ok(text) => {
            println!("[probe] success");
            println!("{text}");
        }
        Err(err) => {
            eprintln!("[probe] request failed: {err}");
            std::process::exit(1);
        }
    }
}
