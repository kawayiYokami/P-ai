const MCP_READ_FILE_SERVER_FLAG: &str = "--mcp-read-file-server";
const MCP_READ_FILE_TOOL_NAME: &str = "read_file";
const MCP_READ_FILE_SESSION_FLAG: &str = "--mcp-read-file-session-id";
const MCP_READ_FILE_API_FLAG: &str = "--mcp-read-file-api-config-id";

#[derive(Debug, Clone, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct McpReadFileArgs {
    absolute_path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
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
        description = "读取本地文件内容。自动识别文本、图片、PDF 与 Office 文件；absolute_path 必须是绝对路径；offset/limit 用于分页，文本结果按现有文本续读语义处理，PDF 图片结果按页继续读取。"
    )]
    async fn read_file(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<
            McpReadFileArgs,
        >,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let absolute_path = args.absolute_path.clone();
        let started = std::time::Instant::now();
        eprintln!(
            "[MCP read_file] 开始，任务=read_file，session_id={}，api_config_id={}，absolute_path={}",
            self.session_id,
            self.api_config_id,
            absolute_path
        );
        let request = ReadFileRequest {
            absolute_path: args.absolute_path,
            offset: args.offset,
            limit: args.limit,
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
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        eprintln!(
            "[MCP read_file] 完成，任务=read_file，session_id={}，api_config_id={}，absolute_path={}，elapsed_ms={}",
            self.session_id,
            self.api_config_id,
            absolute_path,
            elapsed_ms
        );
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
        rmcp::model::ServerInfo {
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some("P-ai read_file MCP server".to_string()),
            ..Default::default()
        }
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
    eprintln!(
        "[MCP] 开始，任务=read_file_mcp_server，session_id={}，api_config_id={}",
        session_id,
        api_config_id
    );
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
