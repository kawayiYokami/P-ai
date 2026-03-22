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
                    },
                    "webpQuality": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "description": "webp 压缩质量"
                    }
                },
                "required": [],
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
                    api_config_id: preview_api_id,
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
