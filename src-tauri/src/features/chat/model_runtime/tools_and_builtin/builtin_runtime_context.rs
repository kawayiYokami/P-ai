async fn builtin_desktop_wait(ms: u64) -> Result<Value, String> {
    let res = run_wait_tool(WaitRequest {
        mode: WaitMode::Sleep,
        ms,
    })
    .await
    .map_err(|err| to_tool_err_string(&err))?;
    serde_json::to_value(res).map_err(|err| format!("序列化桌面等待结果失败：{err}"))
}


async fn builtin_reload(app_state: &AppState) -> Result<Value, String> {
    let mut result = refresh_workspace_mcp_and_skills(app_state)?;
    match mcp_redeploy_all_from_policy(app_state).await {
        Ok(deploy_errors) => {
            if !deploy_errors.is_empty() {
                result.mcp_failed.extend(deploy_errors);
            }
        }
        Err(err) => {
            result.mcp_failed.push(WorkspaceLoadError {
                item: "mcp_redeploy_all_from_policy".to_string(),
                error: err,
            });
        }
    }
    refresh_global_tool_schema_cache(app_state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(app_state);
    serde_json::to_value(result).map_err(|err| format!("序列化刷新结果失败：{err}"))
}

async fn builtin_organize_context(
    app_state: &AppState,
    session_id: &str,
    api_config_id: &str,
    agent_id: &str,
) -> Result<Value, String> {
    let (selected_api, resolved_api, source, effective_agent_id) = {
        let app_config = state_read_config_cached(app_state)?;
        let selected_api = resolve_selected_api_config(&app_config, Some(api_config_id))
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let (_, session_agent_id, session_conversation_id) = delegate_parse_session_parts(session_id);
        let requested_agent_id = agent_id.trim();
        let effective_agent_id = if requested_agent_id.is_empty() {
            session_agent_id.trim().to_string()
        } else {
            requested_agent_id.to_string()
        };
        if effective_agent_id.trim().is_empty() {
            return Err("缺少人格 ID，无法整理上下文。".to_string());
        }
        let conversation_id = session_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "缺少当前工具调用会话 ID，无法整理上下文。".to_string())?;
        let source = state_read_conversation_cached(app_state, conversation_id)
            .map_err(|err| format!("当前工具调用会话不可整理：{}", err))?;
        if source.messages.len() < 10 {
            return Ok(serde_json::json!({
                "ok": false,
                "shouldArchive": false,
                "message": "此时不应该整理：当前对话少于 10 句。"
            }));
        }
        let usage_ratio = conversation_prompt_service()
            .latest_real_prompt_usage(&source, &selected_api)
            .map(|usage| usage.usage_ratio.max(0.0))
            .unwrap_or(0.0);
        if usage_ratio < 0.10 {
            return Ok(serde_json::json!({
                "ok": false,
                "shouldArchive": false,
                "usageRatio": usage_ratio,
                "message": "此时不应该整理：当前上下文占用不足 10%。"
            }));
        }
        (selected_api, resolved_api, source, effective_agent_id)
    };

    spawn_organize_context_auto_compaction(
        app_state,
        selected_api,
        resolved_api,
        source.clone(),
        effective_agent_id.clone(),
    );
    Ok(serde_json::json!({
        "ok": true,
        "applied": true,
        "terminal": true,
        "scheduled": true,
        "conversationId": source.id,
        "message": "上下文整理已接管当前轮，完成后会立即续跑。"
    }))
}

