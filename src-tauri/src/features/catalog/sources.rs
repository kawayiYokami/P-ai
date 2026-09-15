// 远端源适配层：把各源的原始响应归一化为 CatalogEntry。
// 各源的接口形态差异（GET/PUT、分页方式、字段命名）全部收敛在这里。

fn catalog_cache_key_hash(raw: &str) -> String {
    // FNV-1a 64bit，仅用于生成稳定的短缓存键，不做安全用途。
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in raw.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn catalog_cache_file_name(source: &str, query: &str, page: u32, page_size: u32) -> String {
    let key = format!("{source}|{query}|{page}|{page_size}");
    let digest = catalog_cache_key_hash(&key);
    format!(
        "catalog_cache_{}_{}.json",
        sanitize_mcp_server_id_for_filename(source),
        digest
    )
}

fn json_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn json_i64(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(|item| item.as_i64()).unwrap_or(0)
}

fn json_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn json_object_keys(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|item| item.as_object())
        .map(|map| map.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default()
}

// ==================== 魔搭 ====================

const MODELSCOPE_MCP_LIST_URL: &str = "https://modelscope.cn/api/v1/dolphin/mcpServers";
const MODELSCOPE_SKILL_LIST_URL: &str = "https://modelscope.cn/api/v1/dolphin/skills";

async fn call_modelscope_list(
    state: &AppState,
    url: &str,
    query: &str,
    page: u32,
    page_size: u32,
) -> Result<Value, String> {
    let body = serde_json::json!({
        "PageSize": page_size,
        "PageNumber": page,
        "Query": query,
        "Criterion": [],
    });
    let resp = state
        .shared_http_client
        .put(url)
        .header("x-modelscope-accept-language", "zh_CN")
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("请求魔搭接口失败：{err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let snippet = resp.text().await.unwrap_or_default().chars().take(300).collect::<String>();
        return Err(format!("请求魔搭接口失败：{status} | {snippet}"));
    }
    resp.json::<Value>()
        .await
        .map_err(|err| format!("解析魔搭响应失败：{err}"))
}

/// 从魔搭的三种传输配置中挑出可直接使用的 `mcpServers` 定义。
fn modelscope_pick_definition(item: &Value) -> Option<(String, Value)> {
    let first_of = |key: &str| -> Option<Value> {
        item.get(key)
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .cloned()
    };
    if let Some(def) = first_of("ServerConfig") {
        return Some(("stdio".to_string(), def));
    }
    if let Some(def) = first_of("StreamableHTTPServerConfig") {
        return Some(("streamable_http".to_string(), def));
    }
    if let Some(def) = first_of("SSEServerConfig") {
        return Some(("sse".to_string(), def));
    }
    None
}

fn modelscope_mcp_entry(item: &Value) -> CatalogEntry {
    let chinese = json_str(item, "ChineseName");
    let english = json_str(item, "Name");
    let name = if chinese.is_empty() { english.clone() } else { chinese };
    let scope = json_str(item, "Path");
    let picked = modelscope_pick_definition(item);
    let (transport, definition_json) = match picked {
        Some((transport, def)) => (
            transport,
            serde_json::to_string(&def).unwrap_or_default(),
        ),
        None => (String::new(), String::new()),
    };
    let tools = item
        .get("Tools")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|tool| {
                    let name = json_str(tool, "Name");
                    if name.is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    CatalogEntry {
        id: format!("{scope}/{english}"),
        name,
        description: {
            let cn = json_str(item, "AbstractCN");
            if cn.is_empty() {
                json_str(item, "Abstract")
            } else {
                cn
            }
        },
        kind: CATALOG_KIND_MCP.to_string(),
        source: CATALOG_SOURCE_MODELSCOPE_MCP.to_string(),
        categories: json_string_list(item, "Category"),
        popularity: json_i64(item, "CallVolume"),
        author: scope.trim_start_matches('@').to_string(),
        homepage: json_str(item, "FromSiteUrl"),
        icon: json_str(item, "FromSiteIcon"),
        transport,
        install_ready: !definition_json.is_empty(),
        definition_json,
        required_env: json_object_keys(item, "EnvSchema")
            .into_iter()
            .filter(|_| {
                item.get("EnvSchema")
                    .and_then(|schema| schema.get("required"))
                    .and_then(|value| value.as_array())
                    .is_some()
            })
            .collect(),
        tools,
        detail_url: String::new(),
        installed: false,
        enabled: false,
        local_id: String::new(),
    }
}

fn modelscope_skill_entry(item: &Value) -> CatalogEntry {
    let display = json_str(item, "DisplayName");
    let name = json_str(item, "Name");
    let owner = json_str(item, "Owner");
    let l1_name = item
        .get("L1")
        .map(|l1| {
            let cn = json_str(l1, "ChineseName");
            if cn.is_empty() {
                json_str(l1, "Name")
            } else {
                cn
            }
        })
        .unwrap_or_default();
    CatalogEntry {
        id: format!("{owner}/{name}"),
        name: if display.is_empty() { name.clone() } else { display },
        description: {
            let cn = json_str(item, "Description");
            if cn.is_empty() {
                json_str(item, "DescriptionEn")
            } else {
                cn
            }
        },
        kind: CATALOG_KIND_SKILL.to_string(),
        source: CATALOG_SOURCE_MODELSCOPE_SKILL.to_string(),
        categories: if l1_name.is_empty() {
            Vec::new()
        } else {
            vec![l1_name]
        },
        popularity: json_i64(item, "DownloadCount"),
        author: owner,
        homepage: json_str(item, "SourceURL"),
        icon: json_str(item, "SourceAvatar"),
        transport: String::new(),
        definition_json: String::new(),
        required_env: Vec::new(),
        tools: Vec::new(),
        install_ready: skill_source_is_installable(&json_str(item, "SourceURL")),
        detail_url: json_str(item, "SourceURL"),
        installed: false,
        enabled: false,
        local_id: String::new(),
    }
}

// ==================== Smithery ====================

const SMITHERY_LIST_URL: &str = "https://registry.smithery.ai/servers";

fn smithery_entry(item: &Value) -> CatalogEntry {
    let qualified = json_str(item, "qualifiedName");
    CatalogEntry {
        id: qualified.clone(),
        name: {
            let display = json_str(item, "displayName");
            if display.is_empty() {
                qualified.clone()
            } else {
                display
            }
        },
        description: json_str(item, "description"),
        kind: CATALOG_KIND_MCP.to_string(),
        source: CATALOG_SOURCE_SMITHERY.to_string(),
        categories: Vec::new(),
        popularity: json_i64(item, "useCount"),
        author: json_str(item, "namespace"),
        homepage: json_str(item, "homepage"),
        icon: json_str(item, "iconUrl"),
        transport: "streamable_http".to_string(),
        definition_json: String::new(),
        required_env: Vec::new(),
        tools: Vec::new(),
        // Smithery 列表不含启动配置，安装时再取详情。
        install_ready: !qualified.is_empty(),
        detail_url: if qualified.is_empty() {
            String::new()
        } else {
            format!("{SMITHERY_LIST_URL}/{qualified}")
        },
        installed: false,
        enabled: false,
        local_id: String::new(),
    }
}

/// Smithery 详情 -> `mcpServers` 定义。
fn smithery_definition_from_detail(detail: &Value, fallback_name: &str) -> Option<Value> {
    let connections = detail.get("connections")?.as_array()?;
    let first = connections.first()?;
    let url = json_str(first, "deploymentUrl");
    if url.is_empty() {
        return None;
    }
    let entry_name = {
        let qualified = json_str(detail, "qualifiedName");
        if qualified.is_empty() {
            fallback_name.to_string()
        } else {
            qualified.replace('/', "-")
        }
    };
    Some(serde_json::json!({
        "mcpServers": {
            entry_name: {
                "url": url,
                "type": "streamable_http"
            }
        }
    }))
}

fn smithery_required_env_from_detail(detail: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(connections) = detail.get("connections").and_then(|value| value.as_array()) {
        for connection in connections {
            if let Some(required) = connection
                .get("configSchema")
                .and_then(|schema| schema.get("required"))
                .and_then(|value| value.as_array())
            {
                for item in required {
                    if let Some(name) = item.as_str() {
                        if !names.contains(&name.to_string()) {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    names
}

// ==================== 官方 MCP Registry ====================

const OFFICIAL_REGISTRY_URL: &str = "https://registry.modelcontextprotocol.io/v0/servers";

fn official_registry_entry(item: &Value) -> CatalogEntry {
    let server = item.get("server").cloned().unwrap_or(Value::Null);
    let full_name = json_str(&server, "name");
    let title = json_str(&server, "title");
    let remotes = server
        .get("remotes")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let first_remote = remotes.first();
    let remote_url = first_remote.map(|r| json_str(r, "url")).unwrap_or_default();
    let entry_name = if title.is_empty() {
        full_name.replace('/', "-")
    } else {
        title.replace('/', "-")
    };
    let definition_json = if remote_url.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&serde_json::json!({
            "mcpServers": {
                entry_name: {
                    "url": remote_url,
                    "type": "streamable_http"
                }
            }
        }))
        .unwrap_or_default()
    };
    CatalogEntry {
        id: full_name.clone(),
        name: if title.is_empty() { full_name.clone() } else { title },
        description: json_str(&server, "description"),
        kind: CATALOG_KIND_MCP.to_string(),
        source: CATALOG_SOURCE_OFFICIAL.to_string(),
        categories: Vec::new(),
        popularity: 0,
        author: full_name
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string(),
        homepage: String::new(),
        icon: String::new(),
        transport: if remote_url.is_empty() {
            String::new()
        } else {
            "streamable_http".to_string()
        },
        install_ready: !definition_json.is_empty(),
        definition_json,
        required_env: Vec::new(),
        tools: Vec::new(),
        detail_url: String::new(),
        installed: false,
        enabled: false,
        local_id: String::new(),
    }
}

// ==================== 统一入口 ====================

struct CatalogRawPage {
    entries: Vec<CatalogEntry>,
    total: i64,
}

async fn fetch_catalog_raw_page(
    state: &AppState,
    source: &str,
    query: &str,
    page: u32,
    page_size: u32,
) -> Result<CatalogRawPage, String> {
    match source {
        CATALOG_SOURCE_MODELSCOPE_MCP => {
            let payload = call_modelscope_list(
                state,
                MODELSCOPE_MCP_LIST_URL,
                query,
                page,
                page_size,
            )
            .await?;
            let data = payload.get("Data").cloned().unwrap_or(Value::Null);
            let mcp_server = data.get("McpServer").cloned().unwrap_or(Value::Null);
            let entries = mcp_server
                .get("McpServers")
                .and_then(|value| value.as_array())
                .map(|items| items.iter().map(modelscope_mcp_entry).collect::<Vec<_>>())
                .unwrap_or_default();
            let total = {
                let nested = json_i64(&mcp_server, "TotalCount");
                if nested > 0 {
                    nested
                } else {
                    json_i64(&data, "TotalCount")
                }
            };
            Ok(CatalogRawPage { entries, total })
        }
        CATALOG_SOURCE_MODELSCOPE_SKILL => {
            let payload = call_modelscope_list(
                state,
                MODELSCOPE_SKILL_LIST_URL,
                query,
                page,
                page_size,
            )
            .await?;
            let data = payload.get("Data").cloned().unwrap_or(Value::Null);
            let entries = data
                .get("SkillList")
                .and_then(|value| value.as_array())
                .map(|items| items.iter().map(modelscope_skill_entry).collect::<Vec<_>>())
                .unwrap_or_default();
            Ok(CatalogRawPage {
                entries,
                total: json_i64(&data, "TotalCount"),
            })
        }
        CATALOG_SOURCE_SMITHERY => {
            let url = format!(
                "{SMITHERY_LIST_URL}?q={}&page={page}&pageSize={page_size}",
                urlencoding_encode(query)
            );
            let resp = state
                .shared_http_client
                .get(&url)
                .send()
                .await
                .map_err(|err| format!("请求 Smithery 失败：{err}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                return Err(format!("请求 Smithery 失败：{status}"));
            }
            let payload = resp
                .json::<Value>()
                .await
                .map_err(|err| format!("解析 Smithery 响应失败：{err}"))?;
            let entries = payload
                .get("servers")
                .and_then(|value| value.as_array())
                .map(|items| items.iter().map(smithery_entry).collect::<Vec<_>>())
                .unwrap_or_default();
            let total = payload
                .get("pagination")
                .map(|p| json_i64(p, "totalCount"))
                .unwrap_or(-1);
            Ok(CatalogRawPage { entries, total })
        }
        CATALOG_SOURCE_OFFICIAL => {
            // 官方 Registry 用游标分页：从第 1 页起逐页推进到目标页，末页无游标即终止。
            let mut cursor = String::new();
            let mut entries = Vec::<CatalogEntry>::new();
            let mut has_more = false;
            let mut reached = false;
            for current in 1..=page {
                let mut url = format!(
                    "{OFFICIAL_REGISTRY_URL}?limit={page_size}&search={}",
                    urlencoding_encode(query)
                );
                if !cursor.is_empty() {
                    url.push_str(&format!("&cursor={}", urlencoding_encode(&cursor)));
                }
                let resp = state
                    .shared_http_client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|err| format!("请求官方 MCP Registry 失败：{err}"))?;
                if !resp.status().is_success() {
                    let status = resp.status();
                    return Err(format!("请求官方 MCP Registry 失败：{status}"));
                }
                let payload = resp
                    .json::<Value>()
                    .await
                    .map_err(|err| format!("解析官方 MCP Registry 响应失败：{err}"))?;
                entries = payload
                    .get("servers")
                    .and_then(|value| value.as_array())
                    .map(|items| items.iter().map(official_registry_entry).collect::<Vec<_>>())
                    .unwrap_or_default();
                cursor = payload
                    .get("metadata")
                    .and_then(|value| value.get("nextCursor"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                has_more = !cursor.is_empty();
                if current == page {
                    reached = true;
                    break;
                }
                // 目标页之前已无更多数据：目标页为空。
                if entries.is_empty() || !has_more {
                    entries = Vec::new();
                    break;
                }
            }
            if !reached {
                entries = Vec::new();
            }
            Ok(CatalogRawPage {
                entries,
                // 官方 Registry 不返回总数，用「已翻页数 + 是否还有下一页」折算，
                // 保证前端「下一页」按钮只在确有下一页时出现。
                total: if reached && has_more {
                    (page as i64) * (page_size as i64) + 1
                } else {
                    page as i64 * page_size as i64
                },
            })
        }
        other => Err(format!("未知的商店来源：{other}")),
    }
}

/// 极简百分号编码，仅用于查询串。
fn urlencoding_encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// 读取一页商店清单，命中未过期缓存时直接返回。
/// 按本地工作区状态标注条目的「已安装 / 启用状态」。
/// 必须在读出缓存之后调用：缓存只存远端原始数据，安装状态随本地文件变化。
fn mark_catalog_install_state(state: &AppState, source: &str, entries: &mut [CatalogEntry]) {
    if catalog_kind_for_source(source) == CATALOG_KIND_SKILL {
        let installed = load_workspace_skill_summaries_with_errors(state)
            .map(|(skills, _)| skills)
            .unwrap_or_default();
        for entry in entries.iter_mut() {
            let dir_name = skill_dir_name_from_entry(entry);
            let hit = installed.iter().find(|skill| {
                std::path::Path::new(&skill.path)
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|value| value.to_str())
                    == Some(dir_name.as_str())
            });
            if let Some(skill) = hit {
                entry.installed = true;
                entry.enabled = skill.enabled;
                // Skill 的启用状态按 frontmatter 中的技能名索引。
                entry.local_id = skill.name.clone();
            }
        }
        return;
    }

    let policies = mcp_policy_enabled_map(state);
    for entry in entries.iter_mut() {
        let local_id = catalog_entry_local_id(&entry.source, &entry.id);
        let file_stem = sanitize_mcp_server_id_for_filename(&local_id);
        let server_path = match llm_workspace_mcp_servers_dir(state) {
            Ok(dir) => dir.join(format!("{file_stem}.json")),
            Err(_) => continue,
        };
        if server_path.is_file() {
            entry.installed = true;
            entry.enabled = policies.get(&file_stem).copied().unwrap_or(false);
            entry.local_id = file_stem;
        }
    }
}

/// 读取 MCP policies 目录，得到「服务器本地标识 -> 是否启用」映射。
fn mcp_policy_enabled_map(
    state: &AppState,
) -> std::collections::HashMap<String, bool> {
    let mut map = std::collections::HashMap::new();
    let Ok(dir) = llm_workspace_mcp_policies_dir(state) else {
        return map;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return map;
    };
    for entry in entries.filter_map(|item| item.ok()) {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&raw) else {
            continue;
        };
        let Some(file_stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let enabled = value.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        map.insert(file_stem.to_string(), enabled);
    }
    map
}

async fn load_catalog_page(
    state: &AppState,
    input: &CatalogSearchInput,
    forced_source: Option<&str>,
) -> Result<CatalogPage, String> {
    let source = forced_source
        .unwrap_or(input.source.trim())
        .trim()
        .to_string();
    let source = if source.is_empty() {
        let kind = catalog_kind_for_source("");
        catalog_sources_for_kind(kind)
            .first()
            .map(|item| item.id.clone())
            .ok_or_else(|| "没有可用的商店来源".to_string())?
    } else {
        source
    };
    let query = input.query.trim().to_string();
    let page = input.page.max(1);
    let page_size = input.page_size.clamp(1, 100);
    let cache_file = catalog_cache_file_name(&source, &query, page, page_size);

    let cache = ensure_remote_catalog_cache(
        state,
        &cache_file,
        CATALOG_CACHE_MAX_AGE_MS,
        "能力商店",
        || async {
            let raw = fetch_catalog_raw_page(state, &source, &query, page, page_size).await?;
            Ok(serde_json::json!({
                "entries": raw.entries,
                "total": raw.total,
            }))
        },
    )
    .await?;

    let mut entries = cache
        .payload
        .get("entries")
        .and_then(|value| serde_json::from_value::<Vec<CatalogEntry>>(value.clone()).ok())
        .unwrap_or_default();
    // 安装状态随本地文件变化，必须在读出缓存之后标注，不能写回缓存。
    mark_catalog_install_state(state, &source, &mut entries);
    let total = cache
        .payload
        .get("total")
        .and_then(|value| value.as_i64())
        .unwrap_or(-1);
    Ok(CatalogPage {
        kind: catalog_kind_for_source(&source).to_string(),
        source,
        page,
        page_size,
        total,
        entries,
        from_cache: true,
        updated_at: cache.updated_at,
    })
}
