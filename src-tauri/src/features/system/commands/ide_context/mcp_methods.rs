fn ide_chat_mcp_list_servers_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(mcp_list_servers_inner(state)?)
}

fn ide_chat_mcp_validate_definition_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpDefinitionValidateInput>(params, "input")?;
    ide_chat_serialize(mcp_validate_definition_inner(input)?)
}

async fn ide_chat_mcp_fix_definition_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpFixDefinitionInput>(params, "input")?;
    ide_chat_serialize(mcp_fix_definition_inner(input, state).await?)
}

fn ide_chat_mcp_save_server_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpServerInput>(params, "input")?;
    ide_chat_serialize(mcp_save_server_inner(input, state)?)
}

async fn ide_chat_mcp_remove_server_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpServerIdInput>(params, "input")?;
    ide_chat_serialize(mcp_remove_server_inner(input, state).await?)
}

async fn ide_chat_mcp_list_server_tools_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpServerIdInput>(params, "input")?;
    ide_chat_serialize(mcp_list_server_tools_inner(input, state).await?)
}

fn ide_chat_mcp_list_server_tools_cached_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpServerIdInput>(params, "input")?;
    ide_chat_serialize(mcp_list_server_tools_cached_inner(input, state)?)
}

async fn ide_chat_mcp_deploy_server_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpServerIdInput>(params, "input")?;
    ide_chat_serialize(mcp_deploy_server_inner(input, state).await?)
}

async fn ide_chat_mcp_undeploy_server_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpServerIdInput>(params, "input")?;
    ide_chat_serialize(mcp_undeploy_server_inner(input, state).await?)
}

fn ide_chat_mcp_set_tool_enabled_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<McpSetToolEnabledInput>(params, "input")?;
    ide_chat_serialize(mcp_set_tool_enabled_inner(input, state)?)
}

async fn ide_chat_mcp_refresh_mcp_and_skills_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(crate::commands::mcp_refresh_mcp_and_skills_inner(state).await?)
}

fn ide_chat_mcp_list_skills_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(crate::commands::mcp_list_skills_inner(state)?)
}

fn ide_chat_mcp_save_skill_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'content'".to_string())?;
    let name = params.get("name").and_then(|v| v.as_str());
    let description = params.get("description").and_then(|v| v.as_str());
    ide_chat_serialize(crate::commands::mcp_save_skill_inner(state, path, content, name, description)?)
}

fn ide_chat_mcp_read_skill_file_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
    ide_chat_serialize(crate::commands::mcp_read_skill_file_inner(state, path)?)
}

fn ide_chat_skill_open_item_dir_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
    ide_chat_serialize(crate::commands::skill_open_item_dir_inner(state, path)?)
}
