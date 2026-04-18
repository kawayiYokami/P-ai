const MCP_SCREENSHOT_SERVER_FLAG: &str = "--mcp-screenshot-server";

#[derive(Debug, Clone, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct McpDesktopScreenshotArgs {
    #[serde(default)]
    #[serde(rename = "note")]
    #[schemars(description = "Optional note for the screenshot request. Ignored by the tool runtime.")]
    _note: Option<String>,
}

#[derive(Debug, Clone)]
struct DesktopScreenshotMcpServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl DesktopScreenshotMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[rmcp::tool_router(router = tool_router)]
impl DesktopScreenshotMcpServer {
    #[rmcp::tool(
        name = "desktop_screenshot",
        description = "Capture current full desktop screenshot and return metadata + image base64."
    )]
    async fn desktop_screenshot(
        &self,
        rmcp::handler::server::wrapper::Parameters(_args): rmcp::handler::server::wrapper::Parameters<
            McpDesktopScreenshotArgs,
        >,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let request = ScreenshotRequest {
            mode: ScreenshotMode::Desktop,
            monitor_id: None,
            region: None,
            save_path: None,
            webp_quality: 75.0,
        };
        let res = run_screenshot_tool(request).await.map_err(|err| {
            rmcp::ErrorData::internal_error(
                "desktop_screenshot failed",
                Some(serde_json::json!({ "error": err.message })),
            )
        })?;
        let payload = serde_json::json!({
            "ok": true,
            "width": res.width,
            "height": res.height,
            "elapsedMs": res.elapsed_ms,
            "imageMime": res.image_mime,
            "imageBase64": res.image_base64,
            "response": {
                "ok": true,
                "width": res.width,
                "height": res.height,
                "imageMime": res.image_mime,
                "elapsedMs": res.elapsed_ms
            }
        });
        let text = serde_json::to_string(&payload).map_err(|err| {
            rmcp::ErrorData::internal_error(
                "serialize screenshot payload failed",
                Some(serde_json::json!({ "error": err.to_string() })),
            )
        })?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(
            text,
        )]))
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for DesktopScreenshotMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_instructions("P-ai desktop screenshot MCP server")
    }
}

fn run_desktop_screenshot_mcp_server() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("Build MCP screenshot runtime failed: {err}"))?;
    rt.block_on(async move {
        let server = DesktopScreenshotMcpServer::new()
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|err| format!("Start MCP screenshot server failed: {err}"))?;
        server
            .waiting()
            .await
            .map_err(|err| format!("MCP screenshot server join failed: {err}"))?;
        Ok::<(), String>(())
    })
}

