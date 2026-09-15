// 能力商店与 Skill 全局启用开关的 Web 桥接。

fn ide_chat_catalog_list_sources_for_web_settings(params: Value) -> Result<Value, String> {
    let kind = params
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_string);
    ide_chat_serialize(catalog_list_sources_inner(kind.as_deref()))
}

async fn ide_chat_catalog_search_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CatalogSearchInput>(params, "input")?;
    ide_chat_serialize(catalog_search_inner(state, input).await?)
}

async fn ide_chat_catalog_install_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CatalogInstallInput>(params, "input")?;
    ide_chat_serialize(catalog_install_inner(state, input).await?)
}

async fn ide_chat_skill_set_enabled_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required parameter 'name'".to_string())?;
    let enabled = params
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Missing required parameter 'enabled'".to_string())?;
    ide_chat_serialize(crate::commands::skill_set_enabled_inner(state, name, enabled).await?)
}

async fn ide_chat_skill_remove_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
    ide_chat_serialize(crate::commands::skill_remove_inner(state, path).await?)
}
