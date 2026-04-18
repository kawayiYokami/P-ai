const MCP_READ_FILE_SERVER_FLAG: &str = "--mcp-read-file-server";
const MCP_READ_FILE_TOOL_NAME: &str = "read_file";
const MCP_READ_FILE_SESSION_FLAG: &str = "--mcp-read-file-session-id";
const MCP_READ_FILE_API_FLAG: &str = "--mcp-read-file-api-config-id";

#[derive(Debug, Clone, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct McpReadFileArgs {
    /// 要读取的本地文件绝对路径。
    absolute_path: String,
    /// 分页起点。对文本、Office 等非 PDF 内容按行计数；对 PDF 按页计数。默认从 0 开始。
    #[serde(default)]
    start: Option<usize>,
    /// 分页大小。对文本、Office 等非 PDF 内容表示返回多少行；对 PDF 表示返回多少页。
    #[serde(default)]
    count: Option<usize>,
}

#[derive(Debug, Clone)]
struct ReadFileMcpServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
    app_state: AppState,
    session_id: String,
    api_config_id: String,
}

impl ReadFileMcpServer {
    fn new(app_state: AppState, session_id: String, api_config_id: String) -> Self {
        Self {
            tool_router: Self::tool_router(),
            app_state,
            session_id,
            api_config_id,
        }
    }
}

#[rmcp::tool_router(router = tool_router)]
impl ReadFileMcpServer {
    #[rmcp::tool(
        name = "read_file",
        description = "读取本地文件内容。自动识别文本、图片、PDF 与 Office 文件；absolute_path 必须是绝对路径；分页统一使用 start/count：对文本、代码、Office 等非 PDF 内容，start 表示起始行、count 表示返回行数；对 PDF，start 表示起始页、count 表示返回页数。搜索工具若返回命中行号，可直接把该行号附近换算成 start 继续读取。"
    )]
    async fn read_file(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<
            McpReadFileArgs,
        >,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let absolute_path = args.absolute_path.clone();
        let started = std::time::Instant::now();
        let request = ReadFileRequest {
            absolute_path: args.absolute_path,
            start: args.start,
            count: args.count,
        };
        let result = builtin_read_file(
            &self.app_state,
            &self.session_id,
            &self.api_config_id,
            request,
        )
        .await
        .map_err(|err| {
            let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            eprintln!(
                "[MCP read_file] 失败，任务=read_file，session_id={}，api_config_id={}，absolute_path={}，elapsed_ms={}，error={}",
                self.session_id,
                self.api_config_id,
                absolute_path,
                elapsed_ms,
                err
            );
            rmcp::ErrorData::internal_error(
                "read_file failed",
                Some(serde_json::json!({
                    "error": err,
                    "absolutePath": absolute_path,
                    "elapsedMs": elapsed_ms
                })),
            )
        })?;
        let text = serde_json::to_string(&result).map_err(|err| {
            let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            rmcp::ErrorData::internal_error(
                "serialize read_file payload failed",
                Some(serde_json::json!({
                    "error": err.to_string(),
                    "absolutePath": absolute_path,
                    "elapsedMs": elapsed_ms
                })),
            )
        })?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(
            text,
        )]))
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for ReadFileMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_instructions("P-ai read_file MCP server")
    }
}

fn mcp_arg_value(flag: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next().map(|value| value.trim().to_string());
        }
    }
    None
}

pub fn run_read_file_mcp_server() -> Result<(), String> {
    let session_id = mcp_arg_value(MCP_READ_FILE_SESSION_FLAG)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Missing --mcp-read-file-session-id for MCP read_file server".to_string())?;
    let api_config_id = mcp_arg_value(MCP_READ_FILE_API_FLAG)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Missing --mcp-read-file-api-config-id for MCP read_file server".to_string())?;
    let app_state = AppState::new()?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("Build MCP read_file runtime failed: {err}"))?;
    rt.block_on(async move {
        let server = ReadFileMcpServer::new(app_state, session_id, api_config_id)
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|err| format!("Start MCP read_file server failed: {err}"))?;
        server
            .waiting()
            .await
            .map_err(|err| format!("MCP read_file server join failed: {err}"))?;
        Ok::<(), String>(())
    })
}
