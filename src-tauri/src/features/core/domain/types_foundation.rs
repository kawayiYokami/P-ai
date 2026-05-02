#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseStylePreset {
    id: String,
    name: String,
    prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HighestInstruction {
    title: String,
    rules: Vec<String>,
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

fn highest_instruction() -> &'static HighestInstruction {
    static INSTRUCTION: OnceLock<HighestInstruction> = OnceLock::new();
    INSTRUCTION.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../src/constants/highest-instruction.json"
        ))
        .unwrap_or_else(|_| HighestInstruction {
            title: "系统准则".to_string(),
            rules: vec![
                "你必须基于客观事实回答问题，不编造数据、来源或结论。".to_string(),
                "若信息不足或不确定，直接说明不确定，并给出可验证路径。".to_string(),
                "优先给出可执行、可验证、与用户问题直接相关的结论。".to_string(),
            ],
        })
    })
}

fn highest_instruction_markdown() -> String {
    let source = highest_instruction();
    let mut out = String::new();
    for rule in &source.rules {
        let line = rule.trim();
        if !line.is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    prompt_xml_block("system rules", out)
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
    #[serde(rename = "openai_tts")]
    OpenAITts,
    #[serde(rename = "openai_stt")]
    OpenAIStt,
    #[serde(rename = "openai_embedding")]
    OpenAIEmbedding,
    #[serde(rename = "openai_rerank")]
    OpenAIRerank,
    #[serde(rename = "gemini_embedding")]
    GeminiEmbedding,
}

impl RequestFormat {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
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
            "nebius" => Some(Self::Nebius),
            "xai" => Some(Self::Xai),
            "zai" => Some(Self::Zai),
            "bigmodel" => Some(Self::BigModel),
            "aliyun" => Some(Self::Aliyun),
            "cohere" => Some(Self::Cohere),
            "ollama" => Some(Self::Ollama),
            "ollama_cloud" => Some(Self::OllamaCloud),
            "vertex" => Some(Self::Vertex),
            "github_copilot" => Some(Self::GithubCopilot),
            "openai_tts" => Some(Self::OpenAITts),
            "openai_stt" => Some(Self::OpenAIStt),
            "openai_embedding" => Some(Self::OpenAIEmbedding),
            "openai_rerank" => Some(Self::OpenAIRerank),
            "gemini_embedding" => Some(Self::GeminiEmbedding),
            _ => None,
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
            Self::Nebius => "nebius",
            Self::Xai => "xai",
            Self::Zai => "zai",
            Self::BigModel => "bigmodel",
            Self::Aliyun => "aliyun",
            Self::Cohere => "cohere",
            Self::Ollama => "ollama",
            Self::OllamaCloud => "ollama_cloud",
            Self::Vertex => "vertex",
            Self::GithubCopilot => "github_copilot",
            Self::OpenAITts => "openai_tts",
            Self::OpenAIStt => "openai_stt",
            Self::OpenAIEmbedding => "openai_embedding",
            Self::OpenAIRerank => "openai_rerank",
            Self::GeminiEmbedding => "gemini_embedding",
        }
    }

    fn is_openai_stt(self) -> bool {
        matches!(self, Self::OpenAIStt)
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
            Self::DeepSeek | Self::DeepSeekKimi => Some(genai::adapter::AdapterKind::DeepSeek),
            Self::OpenAIResponses | Self::Codex => Some(genai::adapter::AdapterKind::OpenAIResp),
            Self::Gemini => Some(genai::adapter::AdapterKind::Gemini),
            Self::Anthropic => Some(genai::adapter::AdapterKind::Anthropic),
            Self::Fireworks => Some(genai::adapter::AdapterKind::Fireworks),
            Self::Together => Some(genai::adapter::AdapterKind::Together),
            Self::Groq => Some(genai::adapter::AdapterKind::Groq),
            Self::Mimo => Some(genai::adapter::AdapterKind::Mimo),
            Self::Nebius => Some(genai::adapter::AdapterKind::Nebius),
            Self::Xai => Some(genai::adapter::AdapterKind::Xai),
            Self::Zai => Some(genai::adapter::AdapterKind::Zai),
            Self::BigModel => Some(genai::adapter::AdapterKind::BigModel),
            Self::Aliyun => Some(genai::adapter::AdapterKind::Aliyun),
            Self::Cohere => Some(genai::adapter::AdapterKind::Cohere),
            Self::Ollama => Some(genai::adapter::AdapterKind::Ollama),
            Self::OllamaCloud => Some(genai::adapter::AdapterKind::OllamaCloud),
            Self::Vertex => Some(genai::adapter::AdapterKind::Vertex),
            Self::GithubCopilot => Some(genai::adapter::AdapterKind::GithubCopilot),
            Self::Auto
            | Self::OpenAITts
            | Self::OpenAIStt
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
        Self::from_str(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!("unsupported request format '{}'", raw.trim()))
        })
    }
}

fn default_request_format() -> RequestFormat {
    RequestFormat::OpenAI
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
            id: "read_file".to_string(),
            command: "builtin".to_string(),
            args: vec!["read_file".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "apply_patch".to_string(),
            command: "builtin".to_string(),
            args: vec!["apply_patch".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "reload".to_string(),
            command: "builtin".to_string(),
            args: vec!["reload".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "organize_context".to_string(),
            command: "builtin".to_string(),
            args: vec!["organize_context".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "wait".to_string(),
            command: "builtin".to_string(),
            args: vec!["wait".to_string()],
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
            id: "delegate".to_string(),
            command: "builtin".to_string(),
            args: vec!["delegate".to_string()],
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
