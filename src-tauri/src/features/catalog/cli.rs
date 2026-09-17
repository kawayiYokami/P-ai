// 能力商店的 config 工具命令面：store ls / store search / store install。
// 商店接口都是异步且依赖 AppState，所以命令只在运行态生效，由 config 工具的异步分支分派到这里。

/// 搜索默认落在 Skill 商店；查 MCP 时显式给 --kind mcp。
const STORE_DEFAULT_KIND: &str = CATALOG_KIND_SKILL;
const STORE_SEARCH_PAGE_SIZE: u32 = 20;
const STORE_DESCRIPTION_LIMIT: usize = 240;
const STORE_USAGE: &str =
    "用法: store ls [--kind skill|mcp] | store search <关键词> [--source <来源>] [--kind skill|mcp] [--page <页码>] | store install <来源> <条目>";

/// store 命令的只读判定：列来源与搜索不改动本地状态。
fn store_command_is_readonly(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        None | Some("ls") | Some("list") | Some("search")
    )
}

struct StoreArgs {
    positional: Vec<String>,
    flags: std::collections::HashMap<String, String>,
}

impl StoreArgs {
    fn flag(&self, name: &str) -> Option<&str> {
        self.flags
            .get(name)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    fn positional_at(&self, index: usize) -> Option<&str> {
        self.positional.get(index).map(|value| value.trim())
    }
}

/// 解析 store 的参数：`--name value` 形式进 flags，其余按顺序进 positional（第 0 位是子命令）。
fn parse_store_args(args: &[String]) -> StoreArgs {
    let mut positional = Vec::<String>::new();
    let mut flags = std::collections::HashMap::<String, String>::new();
    let mut iter = args.iter().peekable();
    while let Some(item) = iter.next() {
        let trimmed = item.trim();
        if let Some(name) = trimmed.strip_prefix("--") {
            let mut value = String::new();
            if let Some(next) = iter.peek() {
                if !next.trim().starts_with("--") {
                    value = next.trim().to_string();
                    let _ = iter.next();
                }
            }
            flags.insert(name.to_ascii_lowercase(), value);
        } else if !trimmed.is_empty() {
            positional.push(trimmed.to_string());
        }
    }
    StoreArgs { positional, flags }
}

fn store_sources_for_kind(kind: Option<&str>) -> Result<Vec<CatalogSourceInfo>, String> {
    match kind {
        None => {
            let mut all = catalog_sources_for_kind(CATALOG_KIND_SKILL);
            all.extend(catalog_sources_for_kind(CATALOG_KIND_MCP));
            Ok(all)
        }
        Some(CATALOG_KIND_SKILL) => Ok(catalog_sources_for_kind(CATALOG_KIND_SKILL)),
        Some(CATALOG_KIND_MCP) => Ok(catalog_sources_for_kind(CATALOG_KIND_MCP)),
        Some(other) => Err(format!("未知的商店类型: {other}（可用 skill / mcp）")),
    }
}

fn store_default_source(kind: Option<&str>) -> Result<String, String> {
    let kind = match kind {
        None => STORE_DEFAULT_KIND,
        Some(value) if value == CATALOG_KIND_SKILL || value == CATALOG_KIND_MCP => value,
        Some(other) => return Err(format!("未知的商店类型: {other}（可用 skill / mcp）")),
    };
    catalog_sources_for_kind(kind)
        .first()
        .map(|item| item.id.clone())
        .ok_or_else(|| "没有可用的商店来源。".to_string())
}

fn store_truncate_description(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= STORE_DESCRIPTION_LIMIT {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(STORE_DESCRIPTION_LIMIT).collect::<String>();
    out.push('…');
    out
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreEntryView {
    id: String,
    name: String,
    kind: String,
    source: String,
    author: String,
    popularity: i64,
    description: String,
    installed: bool,
    enabled: bool,
    local_id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreSearchView {
    source: String,
    kind: String,
    page: u32,
    total: i64,
    from_cache: bool,
    entries: Vec<StoreEntryView>,
}

/// 执行 store 命令，返回可直接回给模型的 JSON 文本。
async fn run_store_command(state: &AppState, args: &[String]) -> Result<String, String> {
    let parsed = parse_store_args(args);
    match parsed.positional.first().map(String::as_str).unwrap_or("ls") {
        "ls" | "list" => {
            let sources = store_sources_for_kind(parsed.flag("kind"))?;
            serde_json::to_string(&sources).map_err(|err| format!("序列化商店来源失败：{err}"))
        }
        "search" => {
            let query = parsed
                .positional_at(1)
                .ok_or_else(|| STORE_USAGE.to_string())?
                .to_string();
            let source = match parsed.flag("source") {
                Some(source) => source.to_string(),
                None => store_default_source(parsed.flag("kind"))?,
            };
            let page = match parsed.flag("page") {
                Some(raw) => raw
                    .parse::<u32>()
                    .map_err(|_| format!("页码无效: {raw}"))?,
                None => 1,
            };
            let input = CatalogSearchInput {
                source,
                query,
                page,
                page_size: STORE_SEARCH_PAGE_SIZE,
            };
            let page_result = load_catalog_page(state, &input, None).await?;
            let view = StoreSearchView {
                source: page_result.source,
                kind: page_result.kind,
                page: page_result.page,
                total: page_result.total,
                from_cache: page_result.from_cache,
                entries: page_result
                    .entries
                    .into_iter()
                    .map(|entry| StoreEntryView {
                        id: entry.id,
                        name: entry.name,
                        kind: entry.kind,
                        source: entry.source,
                        author: entry.author,
                        popularity: entry.popularity,
                        description: store_truncate_description(&entry.description),
                        installed: entry.installed,
                        enabled: entry.enabled,
                        local_id: entry.local_id,
                    })
                    .collect(),
            };
            serde_json::to_string(&view).map_err(|err| format!("序列化搜索结果失败：{err}"))
        }
        "install" => {
            let source = parsed
                .positional_at(1)
                .ok_or_else(|| STORE_USAGE.to_string())?
                .to_string();
            let entry_id = parsed
                .positional_at(2)
                .ok_or_else(|| STORE_USAGE.to_string())?
                .to_string();
            let input = CatalogInstallInput {
                source,
                entry_id,
                env_values: std::collections::HashMap::new(),
            };
            let result = install_catalog_entry_inner(state, &input).await?;
            let next_step = if result.kind == CATALOG_KIND_SKILL {
                "Skill 已写入工作区且默认关闭；用 config \"skill ls\" 查看，并在配置页的 Skill 列表里启用。"
            } else {
                "MCP 已写入工作区且默认关闭；用 config \"mcp ls\" 查看，需要时再启用并检查工具清单。"
            };
            let view = serde_json::json!({
                "kind": result.kind,
                "localId": result.local_id,
                "enabled": result.enabled,
                "writtenPaths": result.written_paths,
                "nextStep": next_step,
            });
            serde_json::to_string(&view).map_err(|err| format!("序列化安装结果失败：{err}"))
        }
        other => Err(format!("未知 store 命令: {other}。{STORE_USAGE}")),
    }
}
