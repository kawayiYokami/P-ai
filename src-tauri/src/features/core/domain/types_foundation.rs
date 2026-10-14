#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseStylePreset {
    id: String,
    name: String,
    prompt: String,
}

fn built_in_response_styles() -> &'static Vec<ResponseStylePreset> {
    static STYLES: OnceLock<Vec<ResponseStylePreset>> = OnceLock::new();
    STYLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../src/constants/response-styles.json"
        ))
        .unwrap_or_else(|_| {
            vec![ResponseStylePreset {
                id: DEFAULT_RESPONSE_STYLE_ID.to_string(),
                name: "简洁".to_string(),
                prompt: "- 用最少但足够的信息回答。".to_string(),
            }]
        })
    })
}

fn default_response_style_id() -> String {
    DEFAULT_RESPONSE_STYLE_ID.to_string()
}

fn default_pdf_read_mode() -> String {
    DEFAULT_PDF_READ_MODE.to_string()
}

fn normalize_pdf_read_mode(value: &str) -> String {
    match value.trim() {
        "text" => "text".to_string(),
        "image" => "image".to_string(),
        _ => default_pdf_read_mode(),
    }
}

fn default_background_voice_screenshot_keywords() -> String {
    "看看,这个,屏幕上,see,look,watch".to_string()
}

fn default_background_voice_screenshot_mode() -> String {
    DEFAULT_BACKGROUND_VOICE_SCREENSHOT_MODE.to_string()
}

fn normalize_background_voice_screenshot_mode(value: &str) -> String {
    match value.trim() {
        "desktop" => "desktop".to_string(),
        "focused_window" => "focused_window".to_string(),
        _ => default_background_voice_screenshot_mode(),
    }
}

fn normalize_response_style_id(value: &str) -> String {
    let id = value.trim();
    if built_in_response_styles().iter().any(|s| s.id == id) {
        id.to_string()
    } else {
        default_response_style_id()
    }
}

fn response_style_preset(id: &str) -> ResponseStylePreset {
    built_in_response_styles()
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .or_else(|| built_in_response_styles().first().cloned())
        .unwrap_or(ResponseStylePreset {
            id: DEFAULT_RESPONSE_STYLE_ID.to_string(),
            name: "简洁".to_string(),
            prompt: "- 用最少但足够的信息回答。".to_string(),
        })
}

fn response_style_preset_optional(id: &str) -> Option<ResponseStylePreset> {
    let preset = response_style_preset(id);
    if preset.id == "none" {
        None
    } else {
        Some(preset)
    }
}

fn highest_instruction_markdown() -> String {
    prompt_xml_block(
        "system rules",
        include_str!("../../../../resources/prompts/highest-instruction.md").trim(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiToolConfig {
    id: String,
    command: String,
    args: Vec<String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    values: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum RequestFormat {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "deepseek/kimi")]
    DeepSeekKimi,
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "fireworks")]
    Fireworks,
    #[serde(rename = "together")]
    Together,
    #[serde(rename = "groq")]
    Groq,
    #[serde(rename = "mimo")]
    Mimo,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "moonshot")]
    Moonshot,
    #[serde(rename = "nebius")]
    Nebius,
    #[serde(rename = "xai")]
    Xai,
    #[serde(rename = "zai")]
    Zai,
    #[serde(rename = "bigmodel")]
    BigModel,
    #[serde(rename = "aliyun")]
    Aliyun,
    #[serde(rename = "baidu")]
    Baidu,
    #[serde(rename = "cohere")]
    Cohere,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "ollama_cloud")]
    OllamaCloud,
    #[serde(rename = "vertex")]
    Vertex,
    #[serde(rename = "github_copilot")]
    GithubCopilot,
    #[serde(rename = "opencode_go")]
    OpenCodeGo,
    #[serde(rename = "bedrock_api")]
    BedrockApi,
    #[serde(rename = "openai_tts")]
    OpenAITts,
    #[serde(rename = "openai_stt")]
    OpenAIStt,
    #[serde(rename = "mimo_asr")]
    MimoAsr,
    #[serde(rename = "openai_embedding")]
    OpenAIEmbedding,
    #[serde(rename = "openai_rerank")]
    OpenAIRerank,
    #[serde(rename = "gemini_embedding")]
    GeminiEmbedding,
}

impl RequestFormat {
    fn from_str(value: &str) -> Option<Self> {
        let raw = value.trim().to_ascii_lowercase();
        let normalized = raw.replace([' ', '-'], "_");
        match raw.as_str() {
            "openai" => Some(Self::OpenAI),
            "auto" => Some(Self::Auto),
            "deepseek" => Some(Self::DeepSeek),
            "deepseek/kimi" => Some(Self::DeepSeekKimi),
            "openai_responses" => Some(Self::OpenAIResponses),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "anthropic" => Some(Self::Anthropic),
            "fireworks" => Some(Self::Fireworks),
            "together" => Some(Self::Together),
            "groq" => Some(Self::Groq),
            "mimo" => Some(Self::Mimo),
            "minimax" => Some(Self::MiniMax),
            "moonshot" => Some(Self::Moonshot),
            "nebius" => Some(Self::Nebius),
            "xai" | "grok" => Some(Self::Xai),
            "zai" => Some(Self::Zai),
            "bigmodel" => Some(Self::BigModel),
            "aliyun" => Some(Self::Aliyun),
            "baidu" => Some(Self::Baidu),
            "cohere" => Some(Self::Cohere),
            "ollama" => Some(Self::Ollama),
            "ollama_cloud" => Some(Self::OllamaCloud),
            "vertex" => Some(Self::Vertex),
            "github_copilot" => Some(Self::GithubCopilot),
            "opencode_go" => Some(Self::OpenCodeGo),
            "bedrock_api" => Some(Self::BedrockApi),
            "openai_tts" => Some(Self::OpenAITts),
            "openai_stt" => Some(Self::OpenAIStt),
            "mimo_asr" => Some(Self::MimoAsr),
            "openai_embedding" => Some(Self::OpenAIEmbedding),
            "openai_rerank" => Some(Self::OpenAIRerank),
            "gemini_embedding" => Some(Self::GeminiEmbedding),
            _ => match normalized.as_str() {
                "" | "default" | "automatic" => Some(Self::Auto),
                "openai_compatible" | "openaicompatible" | "compatible_openai" | "openai_compat" => {
                    Some(Self::OpenAI)
                }
                "deepseek_kimi" => Some(Self::DeepSeekKimi),
                "kimi" | "moonshot_kimi" => Some(Self::Moonshot),
                "mini_max" => Some(Self::MiniMax),
                "openai_responses" | "responses" => Some(Self::OpenAIResponses),
                "claude" => Some(Self::Anthropic),
                "google" | "google_gemini" => Some(Self::Gemini),
                "ollama_cloud" => Some(Self::OllamaCloud),
                "github_copilot" => Some(Self::GithubCopilot),
                "baidu_qianfan" | "qianfan" => Some(Self::Baidu),
                "opencode_go" | "opencodego" => Some(Self::OpenCodeGo),
                "bedrock_api" | "bedrock" => Some(Self::BedrockApi),
                "stt" | "openai_stt" => Some(Self::OpenAIStt),
                "mimo_asr" => Some(Self::MimoAsr),
                "tts" | "openai_tts" => Some(Self::OpenAITts),
                "embedding" | "embeddings" | "openai_embedding" => Some(Self::OpenAIEmbedding),
                "rerank" | "openai_rerank" => Some(Self::OpenAIRerank),
                "gemini_embedding" => Some(Self::GeminiEmbedding),
                _ => None,
            },
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Auto => "auto",
            Self::DeepSeek => "deepseek",
            Self::DeepSeekKimi => "deepseek/kimi",
            Self::OpenAIResponses => "openai_responses",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic",
            Self::Fireworks => "fireworks",
            Self::Together => "together",
            Self::Groq => "groq",
            Self::Mimo => "mimo",
            Self::MiniMax => "minimax",
            Self::Moonshot => "moonshot",
            Self::Nebius => "nebius",
            Self::Xai => "xai",
            Self::Zai => "zai",
            Self::BigModel => "bigmodel",
            Self::Aliyun => "aliyun",
            Self::Baidu => "baidu",
            Self::Cohere => "cohere",
            Self::Ollama => "ollama",
            Self::OllamaCloud => "ollama_cloud",
            Self::Vertex => "vertex",
            Self::GithubCopilot => "github_copilot",
            Self::OpenCodeGo => "opencode_go",
            Self::BedrockApi => "bedrock_api",
            Self::OpenAITts => "openai_tts",
            Self::OpenAIStt => "openai_stt",
            Self::MimoAsr => "mimo_asr",
            Self::OpenAIEmbedding => "openai_embedding",
            Self::OpenAIRerank => "openai_rerank",
            Self::GeminiEmbedding => "gemini_embedding",
        }
    }

    fn is_stt(self) -> bool {
        matches!(self, Self::OpenAIStt | Self::MimoAsr)
    }

    fn is_gemini(self) -> bool {
        matches!(self, Self::Gemini)
    }

    fn is_anthropic(self) -> bool {
        matches!(self, Self::Anthropic)
    }

    fn genai_adapter_kind(self) -> Option<genai::adapter::AdapterKind> {
        match self {
            Self::OpenAI => Some(genai::adapter::AdapterKind::OpenAI),
            Self::DeepSeek => Some(genai::adapter::AdapterKind::DeepSeek),
            Self::DeepSeekKimi => Some(genai::adapter::AdapterKind::Moonshot),
            Self::OpenAIResponses | Self::Codex => Some(genai::adapter::AdapterKind::OpenAIResp),
            Self::Gemini => Some(genai::adapter::AdapterKind::Gemini),
            Self::Anthropic => Some(genai::adapter::AdapterKind::Anthropic),
            Self::Fireworks => Some(genai::adapter::AdapterKind::Fireworks),
            Self::Together => Some(genai::adapter::AdapterKind::Together),
            Self::Groq => Some(genai::adapter::AdapterKind::Groq),
            Self::Mimo => Some(genai::adapter::AdapterKind::Mimo),
            Self::MiniMax => Some(genai::adapter::AdapterKind::MiniMax),
            Self::Moonshot => Some(genai::adapter::AdapterKind::Moonshot),
            Self::Nebius => Some(genai::adapter::AdapterKind::Nebius),
            Self::Xai => Some(genai::adapter::AdapterKind::Xai),
            Self::Zai => Some(genai::adapter::AdapterKind::Zai),
            Self::BigModel => Some(genai::adapter::AdapterKind::BigModel),
            Self::Aliyun => Some(genai::adapter::AdapterKind::Aliyun),
            Self::Baidu => Some(genai::adapter::AdapterKind::Baidu),
            Self::Cohere => Some(genai::adapter::AdapterKind::Cohere),
            Self::Ollama => Some(genai::adapter::AdapterKind::Ollama),
            Self::OllamaCloud => Some(genai::adapter::AdapterKind::OllamaCloud),
            Self::Vertex => Some(genai::adapter::AdapterKind::Vertex),
            Self::GithubCopilot => Some(genai::adapter::AdapterKind::GithubCopilot),
            Self::OpenCodeGo => Some(genai::adapter::AdapterKind::OpenCodeGo),
            Self::BedrockApi => Some(genai::adapter::AdapterKind::BedrockApi),
            Self::Auto
            | Self::OpenAITts
            | Self::OpenAIStt
            | Self::MimoAsr
            | Self::OpenAIEmbedding
            | Self::OpenAIRerank
            | Self::GeminiEmbedding => None,
        }
    }

    fn is_genai_chat(self) -> bool {
        self.genai_adapter_kind().is_some()
    }

    fn is_openai_style(self) -> bool {
        matches!(
            self,
            Self::OpenAI | Self::Auto | Self::DeepSeek | Self::DeepSeekKimi | Self::OpenAIResponses | Self::Codex
        ) || self.is_genai_chat()
    }

    fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }

    fn is_openai_responses_family(self) -> bool {
        matches!(self, Self::OpenAIResponses | Self::Codex)
    }

    fn is_codex(self) -> bool {
        matches!(self, Self::Codex)
    }

    fn is_chat_text(self) -> bool {
        matches!(self, Self::Auto) || self.is_genai_chat()
    }
}

impl std::fmt::Display for RequestFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for RequestFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_str(&raw).unwrap_or(Self::Auto))
    }
}

fn default_request_format() -> RequestFormat {
    RequestFormat::Auto
}

fn default_false() -> bool {
    false
}

fn default_api_tools() -> Vec<ApiToolConfig> {
    vec![
        ApiToolConfig {
            id: "fetch".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@iflow-mcp/fetch".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "websearch".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "bing-cn-mcp".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "operate".to_string(),
            command: "builtin".to_string(),
            args: vec!["operate".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "exec".to_string(),
            command: "builtin".to_string(),
            args: vec!["exec".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "write".to_string(),
            command: "builtin".to_string(),
            args: vec!["write".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "delete".to_string(),
            command: "builtin".to_string(),
            args: vec!["delete".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "update".to_string(),
            command: "builtin".to_string(),
            args: vec!["update".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "move".to_string(),
            command: "builtin".to_string(),
            args: vec!["move".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "read".to_string(),
            command: "builtin".to_string(),
            args: vec!["read".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "task".to_string(),
            command: "builtin".to_string(),
            args: vec!["task".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "get_session".to_string(),
            command: "builtin".to_string(),
            args: vec!["get_session".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "background".to_string(),
            command: "builtin".to_string(),
            args: vec!["background".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "inform_session".to_string(),
            command: "builtin".to_string(),
            args: vec!["inform_session".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "delegate".to_string(),
            command: "builtin".to_string(),
            args: vec!["delegate".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "create_goal".to_string(),
            command: "builtin".to_string(),
            args: vec!["create_goal".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "update_goal".to_string(),
            command: "builtin".to_string(),
            args: vec!["update_goal".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "meme".to_string(),
            command: "builtin".to_string(),
            args: vec!["meme".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "image_generate".to_string(),
            command: "builtin".to_string(),
            args: vec!["image_generate".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "image_edit".to_string(),
            command: "builtin".to_string(),
            args: vec!["image_edit".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "image_to_video".to_string(),
            command: "builtin".to_string(),
            args: vec!["image_to_video".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
    ]
}

fn default_agent_tools() -> Vec<ApiToolConfig> {
    let mut tools = default_api_tools();
    tools.insert(
        2,
        ApiToolConfig {
            id: "remember".to_string(),
            command: "builtin".to_string(),
            args: vec!["remember".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
    );
    tools.insert(
        3,
        ApiToolConfig {
            id: "recall".to_string(),
            command: "builtin".to_string(),
            args: vec!["recall".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
    );
    tools.insert(
        9,
        ApiToolConfig {
            id: "plan".to_string(),
            command: "builtin".to_string(),
            args: vec!["plan".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
    );
    tools.insert(
        11,
        ApiToolConfig {
            id: "todo".to_string(),
            command: "builtin".to_string(),
            args: vec!["todo".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
    );
    tools
}
