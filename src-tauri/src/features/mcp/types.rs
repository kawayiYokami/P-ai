use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    SerdeSerialize,
    SerdeDeserialize,
)]
enum McpTransportKind {
    Stdio,
    StreamableHttp,
    Sse,
}

impl McpTransportKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable_http",
            Self::Sse => "sse",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ParsedMcpServerDefinition {
    transport: McpTransportKind,
    command: Option<String>,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
    cwd: Option<String>,
    url: Option<String>,
    bearer_token_env_var: Option<String>,
    http_headers: std::collections::HashMap<String, String>,
    env_http_headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerInput {
    id: String,
    name: String,
    enabled: bool,
    definition_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerIdInput {
    server_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpOAuthStatusResult {
    server_id: String,
    status: String,
    message: String,
    #[serde(default)]
    auth_url: String,
    #[serde(default)]
    has_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpDefinitionValidateInput {
    definition_json: String,
    /// 全工作区其他卡片的组内成员名集合，用于跨卡片重名检测
    #[serde(default)]
    existing_member_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpDefinitionValidateResult {
    ok: bool,
    transport: Option<String>,
    server_name: Option<String>,
    message: String,
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    details: Vec<String>,
    #[serde(default)]
    issues: Vec<McpValidationIssue>,
    #[serde(default)]
    migrated_definition_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpFixDefinitionInput {
    definition_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpFixDefinitionResult {
    ok: bool,
    #[serde(default)]
    fixed_definition_json: Option<String>,
    message: String,
    #[serde(default)]
    issues: Vec<McpValidationIssue>,
    #[serde(default)]
    model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolDescriptor {
    tool_name: String,
    description: String,
    enabled: bool,
    #[serde(default)]
    compatibility_error: Option<String>,
    #[serde(default)]
    member_name: String,
    #[serde(default)]
    raw_tool_name: String,
    #[serde(default)]
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpListServerToolsResult {
    server_id: String,
    tools: Vec<McpToolDescriptor>,
    elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpSetToolEnabledInput {
    server_id: String,
    tool_name: String,
    enabled: bool,
}
