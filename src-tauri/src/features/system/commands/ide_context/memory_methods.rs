fn ide_chat_list_memories_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(list_memories_inner(state)?)
}

fn ide_chat_delete_memory_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<DeleteMemoryInput>(params, "input")?;
    ide_chat_serialize(delete_memory_inner(state, input)?)
}

fn ide_chat_preview_export_memories_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(preview_export_memories_inner(state)?)
}

fn ide_chat_export_memories_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let selected_scopes = match params {
        Value::Object(mut map) => map
            .remove("input")
            .and_then(|value| {
                value
                    .get("scopes")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .map(|item| item.as_str().unwrap_or_default().to_string())
                            .collect::<Vec<_>>()
                    })
            }),
        _ => None,
    };
    ide_chat_serialize(export_memories_inner(state, selected_scopes)?)
}

fn ide_chat_import_memories_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ImportMemoriesInput>(params, "input")?;
    ide_chat_serialize(import_memories_inner(state, input)?)
}

fn ide_chat_preview_import_angel_memories_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<PreviewImportAngelMemoriesInput>(params, "input")?;
    ide_chat_serialize(preview_import_angel_memories_inner(input)?)
}

fn ide_chat_import_angel_memories_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ImportAngelMemoriesInput>(params, "input")?;
    ide_chat_serialize(import_angel_memories_inner(state, input)?)
}

fn ide_chat_search_memories_mixed_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SearchMemoriesMixedInput>(params, "input")?;
    ide_chat_serialize(search_memories_mixed_inner(state, input)?)
}

fn ide_chat_search_chat_history_slices_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ChatHistorySearchInput>(params, "input")?;
    ide_chat_serialize(chat_history_search_slices(state, &input)?)
}

async fn ide_chat_search_conversation_messages_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ConversationSearchInput>(params, "input")?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        conversation_search_messages(&app_state, &input)
            .and_then(ide_chat_serialize)
    })
    .await
    .map_err(|err| format!("会话搜索任务异常：{err}"))?
}

fn ide_chat_get_memory_provider_bindings_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(get_memory_provider_bindings_inner(state)?)
}

fn ide_chat_get_memory_embedding_sync_progress_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(get_memory_embedding_sync_progress_inner(state)?)
}

fn ide_chat_test_memory_embedding_provider_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TestMemoryEmbeddingProviderInput>(params, "input")?;
    ide_chat_serialize(test_memory_embedding_provider_inner(input, state)?)
}

fn ide_chat_test_memory_rerank_provider_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TestMemoryRerankProviderInput>(params, "input")?;
    ide_chat_serialize(test_memory_rerank_provider_inner(input, state)?)
}

fn ide_chat_save_memory_embedding_binding_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SaveMemoryEmbeddingBindingInput>(params, "input")?;
    ide_chat_serialize(save_memory_embedding_binding_inner(input, state)?)
}

fn ide_chat_save_memory_rerank_binding_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SaveMemoryRerankBindingInput>(params, "input")?;
    ide_chat_serialize(save_memory_rerank_binding_inner(input, state)?)
}

fn ide_chat_get_agent_private_memory_count_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<AgentPrivateMemoryCountInput>(params, "input")?;
    ide_chat_serialize(get_agent_private_memory_count_inner(input, state)?)
}

fn ide_chat_set_agent_memory_recall_mode_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetAgentMemoryRecallModeInput>(params, "input")?;
    ide_chat_serialize(set_agent_memory_recall_mode_inner(input, state)?)
}

fn ide_chat_disable_agent_private_memory_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<DisableAgentPrivateMemoryInput>(params, "input")?;
    ide_chat_serialize(disable_agent_private_memory_inner(input, state)?)
}
