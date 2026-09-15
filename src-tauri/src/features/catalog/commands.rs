// 能力商店的 Tauri 命令入口。

#[tauri::command]
async fn catalog_list_sources(kind: Option<String>) -> Result<Vec<CatalogSourceInfo>, String> {
    Ok(catalog_list_sources_inner(kind.as_deref()))
}

fn catalog_list_sources_inner(kind: Option<&str>) -> Vec<CatalogSourceInfo> {
    let kind = kind.unwrap_or_default().trim();
    let kind = if kind.is_empty() {
        CATALOG_KIND_MCP
    } else {
        kind
    };
    catalog_sources_for_kind(kind)
}

#[tauri::command]
async fn catalog_search(
    state: State<'_, AppState>,
    input: CatalogSearchInput,
) -> Result<CatalogPage, String> {
    catalog_search_inner(state.inner(), input).await
}

async fn catalog_search_inner(
    state: &AppState,
    input: CatalogSearchInput,
) -> Result<CatalogPage, String> {
    load_catalog_page(state, &input, None).await
}

#[tauri::command]
async fn catalog_install(
    state: State<'_, AppState>,
    input: CatalogInstallInput,
) -> Result<CatalogInstallResult, String> {
    catalog_install_inner(state.inner(), input).await
}

async fn catalog_install_inner(
    state: &AppState,
    input: CatalogInstallInput,
) -> Result<CatalogInstallResult, String> {
    install_catalog_entry_inner(state, &input).await
}
