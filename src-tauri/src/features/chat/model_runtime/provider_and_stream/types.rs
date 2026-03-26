#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScreenshotForwardPayload {
    mime: String,
    base64: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct ScreenshotArtifactEntry {
    mime: String,
    base64: String,
    width: u32,
    height: u32,
    created_seq: u64,
}

const SCREENSHOT_ARTIFACT_MAX_ITEMS: usize = 24;

type ScreenshotMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;
type ReadFileMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

struct RuntimeToolAssembly {
    tools: Vec<Box<dyn ToolDyn>>,
    tool_manifest: Vec<Value>,
    unavailable_tool_notices: Vec<String>,
    _mcp_screenshot_client: Option<ScreenshotMcpClient>,
    _mcp_read_file_client: Option<ReadFileMcpClient>,
}
