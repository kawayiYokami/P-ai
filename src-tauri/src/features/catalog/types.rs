// 能力商店的数据类型定义。

/// 商店条目类型。
const CATALOG_KIND_MCP: &str = "mcp";
const CATALOG_KIND_SKILL: &str = "skill";

/// 源标识。
const CATALOG_SOURCE_MODELSCOPE_MCP: &str = "modelscope-mcp";
const CATALOG_SOURCE_MODELSCOPE_SKILL: &str = "modelscope-skill";
const CATALOG_SOURCE_SMITHERY: &str = "smithery";
const CATALOG_SOURCE_OFFICIAL: &str = "official-registry";

/// 远端清单缓存有效期：1 天。
const CATALOG_CACHE_MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogEntry {
    /// 源内唯一标识，安装时用于再次定位远端条目。
    id: String,
    /// 展示名。
    name: String,
    #[serde(default)]
    description: String,
    /// `mcp` 或 `skill`。
    kind: String,
    /// 来源标识。
    source: String,
    #[serde(default)]
    categories: Vec<String>,
    /// 热度：调用量 / 下载量 / 使用数。
    #[serde(default)]
    popularity: i64,
    #[serde(default)]
    author: String,
    #[serde(default)]
    homepage: String,
    #[serde(default)]
    icon: String,
    /// 传输类型（仅 mcp）：`stdio` / `streamable_http` / `sse`。
    #[serde(default)]
    transport: String,
    /// 可直接写入工作区的 `mcpServers` 定义（仅 mcp）。
    #[serde(default)]
    definition_json: String,
    /// 需要用户填写的环境变量名。
    #[serde(default)]
    required_env: Vec<String>,
    /// 工具名清单（仅 mcp）。
    #[serde(default)]
    tools: Vec<String>,
    /// 详情/正文来源地址：Smithery 详情接口或 Skill 的 SKILL.md 地址，安装时按需拉取。
    #[serde(default)]
    detail_url: String,
    /// 是否具备可安装的配置（缺配置的条目不可安装）。
    #[serde(default)]
    install_ready: bool,
    /// 是否已安装到本地工作区。由后端在读出缓存后按本地文件状态标注，不写入缓存。
    #[serde(default)]
    installed: bool,
    /// 已安装时该条目的启用状态；未安装时无意义。
    #[serde(default)]
    enabled: bool,
    /// 本地标识：Skill 为 frontmatter 中的技能名，MCP 为服务器本地标识。未安装时为空。
    #[serde(default)]
    local_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogPage {
    source: String,
    kind: String,
    page: u32,
    page_size: u32,
    /// 该源命中的总条目数；未知时为 -1。
    total: i64,
    entries: Vec<CatalogEntry>,
    /// 本次结果是否来自本地缓存。
    from_cache: bool,
    /// 缓存写入时间（ISO）。
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSourceInfo {
    id: String,
    name: String,
    kind: String,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSearchInput {
    /// 来源标识；为空时使用默认源。
    #[serde(default)]
    source: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    page: u32,
    #[serde(default)]
    page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogInstallInput {
    source: String,
    /// 源内条目标识。
    entry_id: String,
    /// 用户确认后覆盖的环境变量值。
    #[serde(default)]
    env_values: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogInstallResult {
    kind: String,
    /// 落盘后的本地标识。
    local_id: String,
    /// 安装后的启用状态。
    enabled: bool,
    /// 已写入的相对路径清单，便于前端提示。
    written_paths: Vec<String>,
}

fn catalog_sources_for_kind(kind: &str) -> Vec<CatalogSourceInfo> {
    if kind == CATALOG_KIND_SKILL {
        vec![CatalogSourceInfo {
            id: CATALOG_SOURCE_MODELSCOPE_SKILL.to_string(),
            name: "魔搭 Skill 市场".to_string(),
            kind: CATALOG_KIND_SKILL.to_string(),
            description: "ModelScope 社区汇总的开源 Skill".to_string(),
        }]
    } else {
        vec![
            CatalogSourceInfo {
                id: CATALOG_SOURCE_MODELSCOPE_MCP.to_string(),
                name: "魔搭 MCP 广场".to_string(),
                kind: CATALOG_KIND_MCP.to_string(),
                description: "ModelScope 社区汇总的 MCP Server".to_string(),
            },
            CatalogSourceInfo {
                id: CATALOG_SOURCE_SMITHERY.to_string(),
                name: "Smithery".to_string(),
                kind: CATALOG_KIND_MCP.to_string(),
                description: "英文社区 MCP 注册表".to_string(),
            },
            CatalogSourceInfo {
                id: CATALOG_SOURCE_OFFICIAL.to_string(),
                name: "官方 MCP Registry".to_string(),
                kind: CATALOG_KIND_MCP.to_string(),
                description: "Model Context Protocol 官方注册表".to_string(),
            },
        ]
    }
}

fn catalog_kind_for_source(source: &str) -> &'static str {
    match source {
        CATALOG_SOURCE_MODELSCOPE_SKILL => CATALOG_KIND_SKILL,
        _ => CATALOG_KIND_MCP,
    }
}
