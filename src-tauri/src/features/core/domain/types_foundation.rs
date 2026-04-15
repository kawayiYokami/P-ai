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
    let title = source.title.trim();
    let title = if title.is_empty() {
        "系统准则"
    } else {
        title
    };
    let mut out = String::new();
    for rule in &source.rules {
        let line = rule.trim();
        if !line.is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    prompt_xml_block(title, out)
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
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "openai_tts")]
    OpenAITts,
    #[serde(rename = "openai_stt")]
    OpenAIStt,
    #[serde(rename = "openai_embedding")]
    OpenAIEmbedding,
    #[serde(rename = "openai_rerank")]
    OpenAIRerank,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "gemini_embedding")]
    GeminiEmbedding,
    #[serde(rename = "anthropic")]
    Anthropic,
}

impl RequestFormat {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAI),
            "openai_responses" => Some(Self::OpenAIResponses),
            "codex" => Some(Self::Codex),
            "openai_tts" => Some(Self::OpenAITts),
            "openai_stt" => Some(Self::OpenAIStt),
            "openai_embedding" => Some(Self::OpenAIEmbedding),
            "openai_rerank" => Some(Self::OpenAIRerank),
            "gemini" => Some(Self::Gemini),
            "gemini_embedding" => Some(Self::GeminiEmbedding),
            "deepseek/kimi" => Some(Self::OpenAI),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::OpenAIResponses => "openai_responses",
            Self::Codex => "codex",
            Self::OpenAITts => "openai_tts",
            Self::OpenAIStt => "openai_stt",
            Self::OpenAIEmbedding => "openai_embedding",
            Self::OpenAIRerank => "openai_rerank",
            Self::Gemini => "gemini",
            Self::GeminiEmbedding => "gemini_embedding",
            Self::Anthropic => "anthropic",
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

    fn is_openai_style(self) -> bool {
        matches!(self, Self::OpenAI | Self::OpenAIResponses | Self::Codex)
    }

    fn is_openai_responses_family(self) -> bool {
        matches!(self, Self::OpenAIResponses | Self::Codex)
    }

    fn is_codex(self) -> bool {
        matches!(self, Self::Codex)
    }

    fn is_chat_text(self) -> bool {
        matches!(
            self,
            Self::OpenAI
                | Self::OpenAIResponses
                | Self::Codex
                | Self::Gemini
                | Self::Anthropic
        )
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
            id: "remember".to_string(),
            command: "builtin".to_string(),
            args: vec!["remember".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "recall".to_string(),
            command: "builtin".to_string(),
            args: vec!["recall".to_string()],
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
            id: "command".to_string(),
            command: "builtin".to_string(),
            args: vec!["command".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "plan".to_string(),
            command: "builtin".to_string(),
            args: vec!["plan".to_string()],
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
            id: "todo".to_string(),
            command: "builtin".to_string(),
            args: vec!["todo".to_string()],
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
            id: "remote_im_send".to_string(),
            command: "builtin".to_string(),
            args: vec!["remote_im_send".to_string()],
            enabled: false,
            values: serde_json::json!({}),
        },
    ]
}

fn default_agent_tools() -> Vec<ApiToolConfig> {
    default_api_tools()
}
