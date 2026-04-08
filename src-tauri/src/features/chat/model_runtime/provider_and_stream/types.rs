#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScreenshotForwardImagePayload {
    mime: String,
    base64: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScreenshotForwardPayload {
    images: Vec<ScreenshotForwardImagePayload>,
}

#[derive(Debug, Clone)]
struct ScreenshotArtifactEntry {
    images: Vec<ScreenshotForwardImagePayload>,
    created_seq: u64,
}

const SCREENSHOT_ARTIFACT_MAX_ITEMS: usize = 24;

type ReadFileMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;
type OperateMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

struct RuntimeToolAssembly {
    tools: Vec<Box<dyn RuntimeToolDyn>>,
    tool_manifest: Vec<Value>,
    unavailable_tool_notices: Vec<String>,
    _mcp_read_file_client: Option<ReadFileMcpClient>,
    _mcp_operate_client: Option<OperateMcpClient>,
}
