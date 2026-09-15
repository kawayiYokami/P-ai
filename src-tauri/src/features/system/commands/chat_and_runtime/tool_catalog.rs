fn frontend_tool_definition(function: ProviderToolDefinition) -> FrontendToolDefinition {
    FrontendToolDefinition {
        kind: "function".to_string(),
        function: FrontendToolFunctionDefinition {
            name: function.name,
            description: function.description,
            parameters: function.parameters,
        },
    }
}

async fn builtin_tool_definitions_for_frontend(
    state: &AppState,
) -> Vec<FrontendToolDefinition> {
    let preview_session_id = "__frontend_tool_preview__".to_string();
    let _preview_api_id = "__frontend_tool_preview__".to_string();
    let preview_agent_id = DEFAULT_AGENT_ID.to_string();
    let preview_memory_context = build_memory_agent_context(&preview_agent_id, false, true)
        .unwrap_or(MemoryAgentContext {
            owner_agent_id: None,
            effective_agent_id: preview_agent_id.clone(),
            private_memory_enabled: false,
            recall_enabled: true,
        });
    let out = vec![
        frontend_tool_definition(
            BuiltinFetchTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinBingSearchTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinRememberTool {
                app_state: state.clone(),
                memory_context: preview_memory_context.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinRecallTool {
                app_state: state.clone(),
                memory_context: preview_memory_context.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(operate_provider_tool_definition()),
        frontend_tool_definition(read_provider_tool_definition()),
        frontend_tool_definition(read_media_provider_tool_definition()),
        frontend_tool_definition(
            BuiltinPlanTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinTerminalExecTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                executor_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinConfigTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinWriteFileTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                executor_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinDeleteFileTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                executor_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinUpdateFileTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                executor_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinMoveFileTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                executor_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinTodoTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinCreateGoalTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinUpdateGoalTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinGetSessionTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinBackgroundTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinInformSessionTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinTaskTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                api_config_id: String::new(),
                executor_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinDeepRecallTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                source_agent_id: preview_agent_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinDeepRecallSearchTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                agent_id: preview_agent_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinDeepRecallContextTool {
                app_state: state.clone(),
                session_id: preview_session_id.clone(),
                agent_id: preview_agent_id.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinDelegateTool {
                app_state: state.clone(),
                session_id: preview_session_id,
                source_agent_id: String::new(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinContactSendFilesTool {
                app_state: state.clone(),
                session_id: "__frontend_tool_preview__".to_string(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinMemeTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinImageGenerateTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
        frontend_tool_definition(
            BuiltinImageEditTool {
                app_state: state.clone(),
            }
            .provider_tool_definition(),
        ),
    ];
    out
}

fn permission_catalog_item(
    name: &str,
    description: &str,
    group: &str,
) -> Option<PermissionCatalogItem> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(PermissionCatalogItem {
        name: name.to_string(),
        description: description.trim().to_string(),
        group: group.trim().to_string(),
    })
}

fn sorted_unique_catalog_items(
    values: impl IntoIterator<Item = PermissionCatalogItem>,
) -> Vec<PermissionCatalogItem> {
    let mut out = values.into_iter().collect::<Vec<_>>();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    // 不去重：同名工具共存，由用户自行安排生效顺序
    out
}

#[tauri::command]
async fn list_tool_catalog(state: State<'_, AppState>) -> Result<Vec<FrontendToolDefinition>, String> {
    list_tool_catalog_inner(&state).await
}

async fn list_tool_catalog_inner(state: &AppState) -> Result<Vec<FrontendToolDefinition>, String> {
    Ok(builtin_tool_definitions_for_frontend(state).await)
}

#[tauri::command]
async fn list_permission_catalog(
    state: State<'_, AppState>,
) -> Result<PermissionCatalog, String> {
    list_permission_catalog_inner(&state).await
}

async fn list_permission_catalog_inner(
    state: &AppState,
) -> Result<PermissionCatalog, String> {
    let builtin_tools = sorted_unique_catalog_items(
        builtin_tool_definitions_for_frontend(state)
            .await
            .into_iter()
            .filter_map(|item| {
                if !builtin_tool_visible_in_permission_lists(&item.function.name) {
                    return None;
                }
                permission_catalog_item(
                    &item.function.name,
                    &item.function.description,
                    "",
                )
            }),
    );

    let skills = load_workspace_skill_summaries_with_errors(state)
        .map(|(skills, _errors)| {
            sorted_unique_catalog_items(skills.into_iter().filter_map(|item| {
                permission_catalog_item(&item.name, &item.description, "")
            }))
        })
        .unwrap_or_default();
    let mcp_tools = {
        // 与注册层 build_global_tool_schema_cache 一致：仅已启用 server 的运行时真实工具
        let servers = load_workspace_mcp_servers(state)?;
        let runtime_tools = servers
            .iter()
            .filter(|server| server.enabled)
            .flat_map(|server| {
                list_tools_from_runtime(server)
                    .into_iter()
                    .filter(|tool| tool.enabled)
                    .map(move |tool| (server, tool))
            })
            .collect::<Vec<_>>();
        sorted_unique_catalog_items(
            runtime_tools
                .into_iter()
                .filter_map(move |(server, tool)| {
                    // 与注册层一致：直接使用探测时生成的别名 tool_name
                    let provider_tool_name = tool.tool_name.clone();
                    permission_catalog_item(
                        &provider_tool_name,
                        &tool.description,
                        &server.name,
                    )
                }),
        )
    };
    Ok(PermissionCatalog {
        builtin_tools,
        skills,
        mcp_tools,
    })
}

#[cfg(test)]
mod tool_catalog_tests {
    use super::*;

    fn frontend_definition_json(definition: &FrontendToolDefinition) -> serde_json::Value {
        serde_json::to_value(definition).expect("serialize frontend tool definition should succeed")
    }

    async fn catalog_tool_definition_by_name(tool_name: &str) -> Result<FrontendToolDefinition, String> {
        let state = AppState::new()?;
        builtin_tool_definitions_for_frontend(&state)
            .await
            .into_iter()
            .find(|definition| definition.function.name == tool_name)
            .ok_or_else(|| format!("frontend catalog tool definition not found: {tool_name}"))
    }

    #[test]
    fn frontend_catalog_tools_should_match_runtime_definitions() {
        let operate_catalog = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for tool catalog tests should succeed")
            .block_on(catalog_tool_definition_by_name(OPERATE_TOOL_NAME))
            .expect("load operate definition from frontend catalog should succeed");
        let operate_runtime =
            frontend_tool_definition(operate_provider_tool_definition());
        assert_eq!(
            frontend_definition_json(&operate_catalog),
            frontend_definition_json(&operate_runtime),
            "frontend catalog operate definition drifted from builtin definition"
        );

        let read_catalog = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for tool catalog tests should succeed")
            .block_on(catalog_tool_definition_by_name(READ_TOOL_NAME))
            .expect("load read definition from frontend catalog should succeed");
        let read_runtime =
            frontend_tool_definition(read_provider_tool_definition());
        assert_eq!(
            frontend_definition_json(&read_catalog),
            frontend_definition_json(&read_runtime),
            "frontend catalog read definition drifted from builtin definition"
        );

        let config_catalog = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for config catalog tests should succeed")
            .block_on(catalog_tool_definition_by_name("config"))
            .expect("load config definition from frontend catalog should succeed");
        let config_runtime = frontend_tool_definition(
            BuiltinConfigTool {
                app_state: AppState::new().expect("create app state for config definition"),
            }
            .provider_tool_definition(),
        );
        assert_eq!(
            frontend_definition_json(&config_catalog),
            frontend_definition_json(&config_runtime),
            "frontend catalog config definition drifted from runtime builtin definition"
        );
        let properties = config_catalog.function.parameters["properties"]
            .as_object()
            .expect("config parameters should contain object properties");
        assert_eq!(properties.len(), 1);
        assert_eq!(
            properties["command"]["type"].as_str(),
            Some("string"),
            "config tool must expose exactly one string parameter"
        );

        let todo_catalog = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for tool catalog tests should succeed")
            .block_on(catalog_tool_definition_by_name(TODO_TOOL_NAME))
            .expect("load todo definition from frontend catalog should succeed");
        let todo_runtime = frontend_tool_definition(
            BuiltinTodoTool {
                app_state: AppState::new().expect("create app state for todo definition"),
                session_id: "__frontend_tool_preview__".to_string(),
            }
            .provider_tool_definition(),
        );
        assert_eq!(
            frontend_definition_json(&todo_catalog),
            frontend_definition_json(&todo_runtime),
            "frontend catalog todo definition drifted from runtime builtin definition"
        );

        let goal_state = AppState::new().expect("create app state for goal definitions");
        for (tool_name, runtime_definition) in [
            (
                "create_goal",
                frontend_tool_definition(
                    BuiltinCreateGoalTool {
                        app_state: goal_state.clone(),
                        session_id: "__frontend_tool_preview__".to_string(),
                    }
                    .provider_tool_definition(),
                ),
            ),
            (
                "update_goal",
                frontend_tool_definition(
                    BuiltinUpdateGoalTool {
                        app_state: goal_state.clone(),
                        session_id: "__frontend_tool_preview__".to_string(),
                    }
                    .provider_tool_definition(),
                ),
            ),
        ] {
            let catalog_definition = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime for goal catalog tests should succeed")
                .block_on(catalog_tool_definition_by_name(tool_name))
                .expect("load goal definition from frontend catalog should succeed");
            assert_eq!(
                frontend_definition_json(&catalog_definition),
                frontend_definition_json(&runtime_definition),
                "frontend catalog {tool_name} definition drifted from runtime builtin definition"
            );
        }
    }

    #[test]
    fn permission_catalog_should_hide_fixed_session_tools() {
        let state = AppState::new().expect("create app state for permission catalog");
        let catalog = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime for permission catalog tests should succeed")
            .block_on(list_permission_catalog_inner(&state))
            .expect("load permission catalog");
        let builtin_names = catalog
            .builtin_tools
            .iter()
            .map(|item| item.name.as_str())
            .collect::<std::collections::HashSet<_>>();

        for hidden_name in ["todo", "plan", "task", "create_goal", "update_goal", "get_session", "inform_session"] {
            assert!(
                !builtin_names.contains(hidden_name),
                "permission catalog should hide fixed session tool {hidden_name}"
            );
        }
        assert!(
            builtin_names.contains("exec"),
            "permission catalog should still include adjustable builtin tools"
        );
    }
}
