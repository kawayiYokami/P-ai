const MCP_OPERATE_SERVER_FLAG: &str = "--mcp-operate-server";
const MCP_OPERATE_TOOL_NAME: &str = "operate";

#[derive(Debug, Clone)]
struct OperateMcpServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl OperateMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[rmcp::tool_router(router = tool_router)]
impl OperateMcpServer {
    #[rmcp::tool(
        name = "operate",
        description = "统一桌面脚本工具。入参只有 script:string，一行一个动作。\n可用语法：\nmouse <button> click @x,y [repeat=n] [delay=s] [pre_delay=s] [press=s]\nmouse scroll_up [repeat=n] [delay=s] [pre_delay=s]\nmouse scroll_down [repeat=n] [delay=s] [pre_delay=s]\nkey <combo> [repeat=n] [delay=s] [pre_delay=s] [press=s]\ntext \"内容\" [repeat=n] [delay=s] [pre_delay=s]\nwait <seconds>\nscreenshot [focused_window] [region=@x,y,w,h] [save=\"绝对路径\"] [quality=1..100]\n参数说明：button=left|right|middle|back|forward；combo 用 + 连接按键，如 Control+L、Control+Shift+P、Enter；x/y/w/h 为 0~1 百分比坐标；repeat=1~100；delay/pre_delay/press=0~300 秒；save 必须是绝对路径；quality 默认 75。规则：screenshot 对模型只保留最新一张，旧画面视为已经离去。"
    )]
    async fn operate(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<
            OperateRequest,
        >,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let started = std::time::Instant::now();
        let script_line_count = args
            .script
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        let script_char_count = args.script.chars().count();
        let result = run_operate_tool(args).await.map_err(|err| {
            rmcp::ErrorData::internal_error(
                "operate failed",
                Some(serde_json::json!({
                    "error": err.message,
                })),
            )
        })?;
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        runtime_log_info(format!(
            "[MCP] 完成，任务=operate，触发条件=script_lines={}, script_chars={}，count={}，latest_screenshot={}，elapsed_ms={}",
            script_line_count,
            script_char_count,
            result.executed_count,
            result
                .latest_screenshot
                .as_ref()
                .map(|shot| format!("mode={},width={},height={}", shot.mode, shot.width, shot.height))
                .unwrap_or_else(|| "none".to_string()),
            elapsed_ms
        ));
        let text = serde_json::to_string(&result).map_err(|err| {
            rmcp::ErrorData::internal_error(
                "serialize operate payload failed",
                Some(serde_json::json!({
                    "error": err.to_string(),
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
impl rmcp::ServerHandler for OperateMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some("P-ai operate MCP server".to_string()),
            ..Default::default()
        }
    }
}

fn run_operate_mcp_server() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("构建 MCP operate 运行时失败: {err}"))?;
    rt.block_on(async move {
        let server = OperateMcpServer::new()
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|err| format!("启动 MCP operate 服务失败: {err}"))?;
        server
            .waiting()
            .await
            .map_err(|err| format!("等待 MCP operate 服务结束失败: {err}"))?;
        Ok::<(), String>(())
    })
}
