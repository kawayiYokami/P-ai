fn frontend_tool_definition(function: ToolDefinition) -> FrontendToolDefinition {
    FrontendToolDefinition {
        kind: "function".to_string(),
        function: FrontendToolFunctionDefinition {
            name: function.name,
            description: function.description,
            parameters: function.parameters,
        },
    }
}

fn frontend_screenshot_tool_definition() -> FrontendToolDefinition {
    FrontendToolDefinition {
        kind: "function".to_string(),
        function: FrontendToolFunctionDefinition {
            name: "screenshot".to_string(),
            description: "桌面截图工具。抓取当前桌面截图并返回图像结果。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["desktop", "focused_window"],
                        "description": "截图模式"
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        },
    }
}

fn frontend_read_file_tool_definition() -> FrontendToolDefinition {
    FrontendToolDefinition {
        kind: "function".to_string(),
        function: FrontendToolFunctionDefinition {
            name: "read_file".to_string(),
            description: "读取本地文件内容。自动识别文本、图片、PDF 与 Office 文件；absolute_path 必须是绝对路径，文本结果最多返回 30000 字符，超出时请继续使用 offset/limit 分页。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "absolute_path": { "type": "string", "description": "文件绝对路径" },
                    "offset": { "type": "integer", "minimum": 0, "description": "0-based 起始行号，用于分页读取大文件" },
                    "limit": { "type": "integer", "minimum": 1, "description": "最多读取的行数，配合 offset 使用" }
                },
                "required": ["absolute_path"],
                "additionalProperties": false
            }),
        },
    }
}

async fn builtin_tool_definitions_for_frontend(
    state: &AppState,
) -> Vec<FrontendToolDefinition> {
    let preview_session_id = "__frontend_tool_preview__".to_string();
    let preview_api_id = "__frontend_tool_preview__".to_string();
    let preview_agent_id = DEFAULT_AGENT_ID.to_string();
    vec![
        frontend_tool_definition(rig::tool::Tool::definition(&BuiltinFetchTool, String::new()).await),
        frontend_tool_definition(
            rig::tool::Tool::definition(&BuiltinBingSearchTool, String::new()).await,
        ),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinRememberTool {
                    app_state: state.clone(),
                },
                String::new(),
            )
            .await,
        ),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinRecallTool {
                    app_state: state.clone(),
                },
                String::new(),
            )
            .await,
        ),
        frontend_screenshot_tool_definition(),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinCommandTool {
                    app_state: state.clone(),
                    api_config_id: preview_api_id.clone(),
                    agent_id: preview_agent_id,
                },
                String::new(),
            )
            .await,
        ),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinTerminalExecTool {
                    app_state: state.clone(),
                    session_id: preview_session_id.clone(),
                },
                String::new(),
            )
            .await,
        ),
        frontend_read_file_tool_definition(),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinApplyPatchTool {
                    app_state: state.clone(),
                    session_id: preview_session_id.clone(),
                },
                String::new(),
            )
            .await,
        ),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinTaskTool {
                    app_state: state.clone(),
                },
                String::new(),
            )
            .await,
        ),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinDelegateTool {
                    app_state: state.clone(),
                    session_id: preview_session_id,
                },
                String::new(),
            )
            .await,
        ),
        frontend_tool_definition(
            rig::tool::Tool::definition(
                &BuiltinRemoteImSendTool {
                    app_state: state.clone(),
                },
                String::new(),
            )
            .await,
        ),
    ]
}

#[tauri::command]
async fn list_tool_catalog(state: State<'_, AppState>) -> Result<Vec<FrontendToolDefinition>, String> {
    Ok(builtin_tool_definitions_for_frontend(state.inner()).await)
}
