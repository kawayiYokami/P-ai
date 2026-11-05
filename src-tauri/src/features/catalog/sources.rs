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
    // 缓存存的是归一化后的 CatalogEntry；解析口径变化时必须升版本，
    // 否则旧缓存里已冻结的错误字段（如把 schema 键名当环境变量）会继续被原样读出。
    let key = format!("v2|{source}|{query}|{page}|{page_size}");
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

/// 魔搭 `EnvSchema` 是标准 JSON Schema：环境变量名在 `required` 数组里。
/// 上游曾把 schema 顶层键名（properties/required/type）误当变量名下发，
/// 这里只认 `required` 里的字符串元素，无法解析时按无变量处理。
fn modelscope_required_env_names(item: &Value) -> Vec<String> {
    item.get("EnvSchema")
        .and_then(|schema| schema.get("required"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|name| name.as_str())
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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
        required_env: modelscope_required_env_names(item),
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

// ==================== 官方 MCP Registry ====================

const OFFICIAL_REGISTRY_URL: &str = "https://registry.modelcontextprotocol.io/v0/servers";

/// 官方注册中心条目的 GitHub owner：仓库地址落在 github.com 上时取路径第一段，
/// 名字以 `io.github.` 开头时取前缀之后那段；都不是则返回空。
fn official_registry_github_owner(full_name: &str, repository_url: &str) -> String {
    let from_repository = repository_url
        .trim()
        .strip_prefix("https://github.com/")
        .or_else(|| repository_url.trim().strip_prefix("https://www.github.com/"))
        .map(|rest| rest.trim_start_matches('/').split('/').next().unwrap_or_default())
        .unwrap_or_default()
        .trim()
        .to_string();
    if !from_repository.is_empty() {
        return from_repository;
    }
    full_name
        .trim()
        .strip_prefix("io.github.")
        .map(|rest| rest.split('/').next().unwrap_or_default())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 官方注册中心条目的作者：优先用 GitHub owner，
/// 否则把命名空间按点切开后倒序拼成域名形态（`com.example` → `example.com`）。
fn official_registry_author(full_name: &str, repository_url: &str) -> String {
    let owner = official_registry_github_owner(full_name, repository_url);
    if !owner.is_empty() {
        return owner;
    }
    let namespace = full_name.trim().split('/').next().unwrap_or_default();
    let mut parts: Vec<&str> = namespace.split('.').filter(|part| !part.is_empty()).collect();
    parts.reverse();
    parts.join(".")
}

/// GitHub owner 的头像地址，尺寸 96。
fn github_avatar_url(owner: &str) -> String {
    format!("https://github.com/{}.png?size=96", owner.trim())
}

/// 官方注册中心条目的图标：取自带 `icons` 里第一个 `https://` 地址；
/// 没有自带图标时，只有确认了 GitHub owner 才用头像兜底，不给非 GitHub 来源编造地址。
fn official_registry_icon(server: &Value, github_owner: &str) -> String {
    if let Some(items) = server.get("icons").and_then(|value| value.as_array()) {
        for item in items {
            let src = json_str(item, "src");
            if src.starts_with("https://") {
                return src;
            }
        }
    }
    if github_owner.trim().is_empty() {
        return String::new();
    }
    github_avatar_url(github_owner)
}

/// 官方注册中心按版本返回：同一个 server 的 1.0.0 / 1.0.1 / 2.0.0 各占一条。
/// 只留 `isLatest == true` 且 `status == "active"` 的那条，否则一页里同一个 MCP 会重复两三次。
/// 缺 `_meta` 或字段缺失时放行，避免接口形态变化时整页被滤空。
fn official_registry_item_is_latest_active(item: &Value) -> bool {
    let Some(meta) = item.get("_meta").and_then(|value| {
        value.get("io.modelcontextprotocol.registry/official")
    }) else {
        return true;
    };
    let is_latest = meta
        .get("isLatest")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let status = meta
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("active");
    is_latest && status == "active"
}

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
    let repository_url = server
        .get("repository")
        .map(|value| json_str(value, "url"))
        .unwrap_or_default();
    let github_owner = official_registry_github_owner(&full_name, &repository_url);
    let author = official_registry_author(&full_name, &repository_url);
    let icon = official_registry_icon(&server, &github_owner);
    CatalogEntry {
        id: full_name.clone(),
        name: if title.is_empty() { full_name.clone() } else { title },
        description: json_str(&server, "description"),
        kind: CATALOG_KIND_MCP.to_string(),
        source: CATALOG_SOURCE_OFFICIAL.to_string(),
        categories: Vec::new(),
        popularity: 0,
        author,
        homepage: String::new(),
        icon,
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

// ==================== skills.sh ====================

const SKILLS_SH_HOME_URL: &str = "https://www.skills.sh/";
const SKILLS_SH_SEARCH_URL: &str = "https://www.skills.sh/api/search";
/// 检索一次取 40 条：服务端的 limit 不是硬上限，这里比一页 30 条留些余量。
/// 没有分页参数，取多了就是一次拉回整份 JSON，别往大调。
const SKILLS_SH_SEARCH_LIMIT: usize = 40;
/// 带一个正常的浏览器 UA 与 Accept 就够了；实测 curl 默认 UA 同样 200，不是硬要求。
/// 保留它只是给请求一个明确身份，不要据此以为需要整套浏览器头。
const SKILLS_SH_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// skills.sh 的 `source` 必须是 `owner/repo` 形态，其余视为脏数据丢弃。
fn skills_sh_source_is_repo(source: &str) -> bool {
    let mut parts = source.split('/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return false;
    }
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    };
    valid(owner) && valid(repo)
}

/// skills.sh 条目 id：`owner/repo/skillId` 三段，安装时用来反解仓库与目录。
fn skills_sh_entry_id(source: &str, skill_id: &str) -> String {
    format!("{}/{}", source.trim(), skill_id.trim())
}

/// skills.sh 单条记录 -> 条目；`source` 或 `skillId` 不合法时丢弃。
fn skills_sh_entry(raw: &Value) -> Option<CatalogEntry> {
    let source = json_str(raw, "source");
    let skill_id = json_str(raw, "skillId");
    // skillId 含斜杠时 id 会变成四段，安装时 rsplitn(3) 会反解出错的仓库去查别人的仓库树。
    if !skills_sh_source_is_repo(&source) || skill_id.trim().is_empty() || skill_id.contains('/') {
        return None;
    }
    let owner = source.split('/').next().unwrap_or_default().trim().to_string();
    let name = {
        let name = json_str(raw, "name");
        if name.trim().is_empty() {
            skill_id.clone()
        } else {
            name
        }
    };
    Some(CatalogEntry {
        id: skills_sh_entry_id(&source, &skill_id),
        name,
        // 检索结果里没有描述，首版留空，不为它多打一次请求。
        description: String::new(),
        kind: CATALOG_KIND_SKILL.to_string(),
        source: CATALOG_SOURCE_SKILLS_SH.to_string(),
        categories: Vec::new(),
        popularity: json_i64(raw, "installs"),
        author: owner.clone(),
        homepage: format!("https://github.com/{}", source.trim()),
        // skills.sh 不给图标；owner 是 GitHub 账号，用头像兜底。
        icon: github_avatar_url(&owner),
        transport: String::new(),
        definition_json: String::new(),
        required_env: Vec::new(),
        tools: Vec::new(),
        detail_url: String::new(),
        install_ready: true,
        installed: false,
        enabled: false,
        local_id: String::new(),
    })
}

fn skills_sh_entries(raw_items: &[Value]) -> Vec<CatalogEntry> {
    raw_items.iter().filter_map(skills_sh_entry).collect()
}

/// 热门榜：页面数据里 `initialSkills":` 之后紧跟一个 JSON 数组，直接解码，不解析 DOM。
/// 页面把这段 JSON 放在 JS 字符串里，键名的引号实测带一层反斜杠转义，
/// 也有不转义的形态，两种都认。
/// 页面把数组放在 JS 字符串里，引号与反斜杠各多一层转义。做一次单遍还原：
/// `\"` -> `"`，`\\` -> `\`，其余序列原样留给 JSON 解析器。
/// 不能用两次 replace 代替：`\\"` 无论先换哪一个都会丢字符。
fn skills_sh_unescape_js_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some(next @ ('\\' | '"')) => out.push(next),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn skills_sh_trending_entries(page: &str) -> Vec<CatalogEntry> {
    const ESCAPED_KEY: &str = "initialSkills\\\":";
    const PLAIN_KEY: &str = "initialSkills\":";
    let tail = if let Some(start) = page.find(ESCAPED_KEY) {
        &page[start + ESCAPED_KEY.len()..]
    } else if let Some(start) = page.find(PLAIN_KEY) {
        &page[start + PLAIN_KEY.len()..]
    } else {
        return Vec::new();
    };
    // 交给 serde 的流式解码器：它自己按括号与字符串状态扫到数组结束，
    // 不会像正则那样贪到数组之外，所以这里不需要手写括号计数。
    let decoded = skills_sh_unescape_js_string(tail);
    let mut stream = serde_json::Deserializer::from_str(&decoded).into_iter::<Vec<Value>>();
    match stream.next() {
        Some(Ok(items)) => skills_sh_entries(&items),
        _ => Vec::new(),
    }
}

/// skills.sh 的 GET：带 UA 与 Accept，5xx 只重试一次。
async fn skills_sh_get(state: &AppState, url: &str, accept: &str) -> Result<String, String> {
    let mut attempt = 0u8;
    loop {
        attempt += 1;
        let resp = state
            .shared_http_client
            .get(url)
            .header("User-Agent", SKILLS_SH_USER_AGENT)
            .header("Accept", accept)
            .send()
            .await
            .map_err(|err| format!("请求 skills.sh 失败：{err}"))?;
        let status = resp.status();
        if status.is_success() {
            return resp
                .text()
                .await
                .map_err(|err| format!("读取 skills.sh 响应失败：{err}"));
        }
        // 5xx 当服务端点冷或暂忙，等一秒再问一次；其余状态码直接报错。
        if status.is_server_error() && attempt == 1 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }
        return Err(format!("skills.sh 返回 {status}"));
    }
}

async fn fetch_skills_sh_page(
    state: &AppState,
    query: &str,
    page: u32,
) -> Result<CatalogRawPage, String> {
    // 没有分页参数：页码恒按 1 处理，总数按未知标记。
    if page > 1 {
        return Ok(CatalogRawPage {
            entries: Vec::new(),
            total: -1,
        });
    }
    // 空查询是浏览态：拉一次首页热门榜，靠磁盘缓存 TTL 挡住重复拉取。
    // 单字查询在 load_catalog_page 就返回空页了，不会走到这里。
    let trimmed = query.trim();
    if trimmed.is_empty() {
        let html = skills_sh_get(state, SKILLS_SH_HOME_URL, "text/html").await?;
        return Ok(CatalogRawPage {
            entries: skills_sh_trending_entries(&html),
            total: -1,
        });
    }
    let url = format!(
        "{SKILLS_SH_SEARCH_URL}?q={}&limit={SKILLS_SH_SEARCH_LIMIT}",
        urlencoding_encode(query.trim())
    );
    let body = skills_sh_get(state, &url, "application/json").await?;
    let payload = serde_json::from_str::<Value>(&body)
        .map_err(|err| format!("解析 skills.sh 检索响应失败：{err}"))?;
    // 只读 skills[]：响应里的 count 是本次实际返回条数，不是总数，不能拿来算分页。
    let items = payload
        .get("skills")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(CatalogRawPage {
        entries: skills_sh_entries(&items),
        // 没有分页也没有总数；中文查询是模糊匹配兜底到热门榜，质量由服务端决定，这里不做补救。
        total: -1,
    })
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
        CATALOG_SOURCE_SKILLS_SH => fetch_skills_sh_page(state, query, page).await,
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
                    .map(|items| {
                        items
                            .iter()
                            .filter(|item| official_registry_item_is_latest_active(item))
                            .map(official_registry_entry)
                            .collect::<Vec<_>>()
                    })
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
                // 判据只能看游标有没有：某页 30 条恰好全是非 latest 版本时，
                // 过滤后的条数为空不代表没有下一页，按条数判会把目标页整个判空。
                if !has_more {
                    entries = Vec::new();
                    break;
                }
            }
            if !reached {
                entries = Vec::new();
            }
            // smithery 包装发布的条目（`ai.smithery/...`）排在后面：
            // 撤掉 smithery 独立源之后，注册中心里这类条目数量很多，不压后会把列表占满。
            entries.sort_by_key(|entry| entry.id.starts_with("ai.smithery/"));
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

/// skills.sh 的单字查询：不发请求、不写缓存，直接给空页。
/// 首页 93 万字节，用户在搜索框里敲一个删一个会反复触发；而这个源单字命中本来就差，
/// 不如让界面直接提示至少输入两个字。
fn catalog_skills_sh_query_too_short(source: &str, query: &str) -> bool {
    source == CATALOG_SOURCE_SKILLS_SH && query.chars().count() == 1
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
    // skills.sh 的单字查询在碰缓存层之前就返回：既不拉首页，也不为每个单字留一份空缓存文件。
    if catalog_skills_sh_query_too_short(&source, &query) {
        return Ok(CatalogPage {
            kind: catalog_kind_for_source(&source).to_string(),
            source,
            page,
            page_size,
            total: -1,
            entries: Vec::new(),
            from_cache: false,
            updated_at: String::new(),
        });
    }
    let cache_file = catalog_cache_file_name(&source, &query, page, page_size);

    let (cache, from_cache) = ensure_remote_catalog_cache(
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
    // 图标地址登记进白名单：只有本页出现过的地址才允许取图。
    catalog_remember_icon_addresses(&entries);
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
        from_cache,
        updated_at: cache.updated_at,
    })
}
