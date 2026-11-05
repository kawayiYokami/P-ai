// ==================== 搜索引擎调度 ====================
//
// 五个内置搜索引擎（exa / parallel / firecrawl / tavily / deepseek），两种调度模式：
// - Auto：候选集随机取一个，本会话上次成功的优先；429 记冷却换源，网络错误与 5xx 换源不记冷却。
// - Fixed：只用锁定的那个引擎，不换源不看冷却，失败原样返回。
// 五个引擎全失败时按引擎逐条列出失败原因，不设任何兜底抓取。

const EXA_SEARCH_MCP_URL: &str = "https://mcp.exa.ai/mcp";
const PARALLEL_SEARCH_MCP_URL: &str = "https://search.parallel.ai/mcp";
const FIRECRAWL_SEARCH_MCP_URL: &str = "https://mcp.firecrawl.dev/v2/mcp";
const TAVILY_SEARCH_URL: &str = "https://api.tavily.com/search";
const DEEPSEEK_SEARCH_URL: &str = "https://api.deepseek.com/v1/tools/search";

const MCP_TOOL_NAME_EXA_SEARCH: &str = "web_search_exa";
const MCP_TOOL_NAME_PARALLEL_SEARCH: &str = "web_search";
const MCP_TOOL_NAME_FIRECRAWL_SEARCH: &str = "firecrawl_search";

const SEARCH_RESULT_LIMIT: usize = 10;
const SEARCH_SNIPPET_CHAR_LIMIT: usize = 320;
const SEARCH_FAILURE_SNIPPET_CHAR_LIMIT: usize = 240;
const DEFAULT_SEARCH_COOLDOWN_SECONDS: u64 = 60;
const MAX_SEARCH_COOLDOWN_SECONDS: u64 = 900;

// ==================== 引擎表 ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchEngine {
    Exa,
    Parallel,
    Firecrawl,
    Tavily,
    Deepseek,
}

impl SearchEngine {
    const ALL: [SearchEngine; 5] = [
        SearchEngine::Exa,
        SearchEngine::Parallel,
        SearchEngine::Firecrawl,
        SearchEngine::Tavily,
        SearchEngine::Deepseek,
    ];

    fn id(self) -> &'static str {
        match self {
            SearchEngine::Exa => "exa",
            SearchEngine::Parallel => "parallel",
            SearchEngine::Firecrawl => "firecrawl",
            SearchEngine::Tavily => "tavily",
            SearchEngine::Deepseek => "deepseek",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            SearchEngine::Exa => "Exa",
            SearchEngine::Parallel => "Parallel",
            SearchEngine::Firecrawl => "Firecrawl",
            SearchEngine::Tavily => "Tavily",
            SearchEngine::Deepseek => "DeepSeek",
        }
    }

    /// 免密钥可用。DeepSeek 必须填 key；Firecrawl 的 keyless 调用已被服务端关闭，
    /// 不填 key 时每次都会白打一次必然失败的请求，因此也不算免密钥。
    fn is_keyless(self) -> bool {
        !matches!(self, SearchEngine::Deepseek | SearchEngine::Firecrawl)
    }

    fn from_id(raw: &str) -> Option<Self> {
        let normalized = raw.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|engine| engine.id() == normalized)
    }
}

// ==================== 失败分类 ====================

/// 一次引擎调用的失败类别，决定调度器下一步动作。
#[derive(Debug)]
enum SearchCallError {
    /// 429 限流：记冷却后换下一个源
    Cooldown { seconds: u64, message: String },
    /// 网络错误 / 5xx / 响应不可解析：换下一个源，不记冷却
    Transient(String),
    /// 401/403：密钥无效。换下一个源，但把原因原样留在失败列表里，别掩盖配置问题
    Auth(String),
}

impl SearchCallError {
    fn transient(message: impl Into<String>) -> Self {
        Self::Transient(message.into())
    }

    fn message(self) -> String {
        match self {
            SearchCallError::Cooldown { message, .. } => message,
            SearchCallError::Transient(message) => message,
            SearchCallError::Auth(message) => message,
        }
    }
}

// ==================== 调度状态（会话级内存态，不落盘） ====================

/// 会话级「上次成功引擎」表的容量上限，超过后只保留当前会话的那一条。
const MAX_TRACKED_SEARCH_SESSIONS: usize = 128;

#[derive(Default)]
struct SearchEngineScheduler {
    /// 引擎 id → 冷却到期时刻
    cooldown_until: std::collections::HashMap<String, std::time::Instant>,
    /// 会话 id → 上次成功的引擎 id
    last_success: std::collections::HashMap<String, String>,
    /// 引擎 id → 正在飞的请求数；并发搜索不该全撞在同一个源上
    in_flight: std::collections::HashMap<String, usize>,
}

impl SearchEngineScheduler {
    fn is_cooling(&self, engine: SearchEngine, now: std::time::Instant) -> bool {
        self.cooldown_until
            .get(engine.id())
            .is_some_and(|until| *until > now)
    }

    fn in_flight_count(&self, engine: SearchEngine) -> usize {
        self.in_flight.get(engine.id()).copied().unwrap_or(0)
    }

    fn begin_in_flight(&mut self, engine: SearchEngine) {
        *self
            .in_flight
            .entry(engine.id().to_string())
            .or_insert(0) += 1;
    }

    fn finish_in_flight(&mut self, engine: SearchEngine) {
        let key = engine.id().to_string();
        let Some(count) = self.in_flight.get_mut(&key) else {
            return;
        };
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.in_flight.remove(&key);
        }
    }

    fn set_cooldown(&mut self, engine: SearchEngine, seconds: u64, now: std::time::Instant) {
        let seconds = seconds.clamp(1, MAX_SEARCH_COOLDOWN_SECONDS);
        self.cooldown_until.insert(
            engine.id().to_string(),
            now + std::time::Duration::from_secs(seconds),
        );
    }

    fn remember_success(&mut self, session_key: &str, engine: SearchEngine) {
        if session_key.trim().is_empty() {
            return;
        }
        self.last_success
            .insert(session_key.to_string(), engine.id().to_string());
    }

    fn last_success_engine(&self, session_key: &str) -> Option<SearchEngine> {
        let raw = self.last_success.get(session_key)?;
        SearchEngine::from_id(raw)
    }

    /// 回收已经没用上的记账：冷却到期的丢掉；会话表给一个上限，避免随会话数无限长大。
    fn prune_stale_state(&mut self, session_key: &str, now: std::time::Instant) {
        self.cooldown_until.retain(|_, until| *until > now);
        if self.last_success.len() > MAX_TRACKED_SEARCH_SESSIONS {
            self.last_success.retain(|key, _| key == session_key);
        }
    }

    /// 本次调用的尝试顺序：冷却中的剔掉，正在飞的也剔掉；本会话上次成功的排第一；其余随机打散。
    fn plan(
        &mut self,
        session_key: &str,
        candidates: &[SearchEngine],
        now: std::time::Instant,
    ) -> Vec<SearchEngine> {
        self.prune_stale_state(session_key, now);
        let not_cooling = candidates
            .iter()
            .copied()
            .filter(|engine| !self.is_cooling(*engine, now))
            .collect::<Vec<_>>();
        let idle = not_cooling
            .iter()
            .copied()
            .filter(|engine| self.in_flight_count(*engine) == 0)
            .collect::<Vec<_>>();
        // 全都在飞（或候选只剩一个）时退回不过滤：宁可复用同一个源，也不能给出空候选。
        let mut available = if idle.is_empty() { not_cooling } else { idle };
        if let Some(previous) = self.last_success_engine(session_key) {
            if let Some(index) = available.iter().position(|engine| *engine == previous) {
                let preferred = available.remove(index);
                shuffle_search_engines(&mut available);
                available.insert(0, preferred);
                return available;
            }
        }
        shuffle_search_engines(&mut available);
        available
    }
}

/// 在飞计数的作用域守卫：拿到就 +1，成功、失败、提前 return 或 panic 都由 Drop 收尾 -1。
struct SearchEngineLease {
    engine: SearchEngine,
}

impl Drop for SearchEngineLease {
    fn drop(&mut self) {
        let engine = self.engine;
        with_search_scheduler(|scheduler| scheduler.finish_in_flight(engine));
    }
}

fn acquire_search_engine(engine: SearchEngine) -> SearchEngineLease {
    with_search_scheduler(|scheduler| scheduler.begin_in_flight(engine));
    SearchEngineLease { engine }
}

/// 洗牌用 uuid v4 的随机位，不额外引入随机数依赖。
fn shuffle_search_engines(engines: &mut [SearchEngine]) {
    if engines.len() < 2 {
        return;
    }
    for index in (1..engines.len()).rev() {
        let random = uuid::Uuid::new_v4().as_u128() as usize;
        engines.swap(index, random % (index + 1));
    }
}

// 冷却与上次成功记录都是运行时内存态，随进程结束清空。
static SEARCH_ENGINE_SCHEDULER: once_cell::sync::Lazy<std::sync::Mutex<SearchEngineScheduler>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(SearchEngineScheduler::default()));

fn with_search_scheduler<T>(action: impl FnOnce(&mut SearchEngineScheduler) -> T) -> T {
    let mut guard = SEARCH_ENGINE_SCHEDULER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    action(&mut guard)
}

// ==================== 配置读取 ====================

/// 一个可用于本次调用的引擎配置（已过滤停用与缺密钥的情形）。
struct ReadySearchEngine {
    engine: SearchEngine,
    api_key: Option<String>,
}

fn search_engine_config_entry<'a>(
    config: &'a AppConfig,
    engine: SearchEngine,
) -> Option<&'a SearchEngineConfig> {
    config
        .search_engines
        .iter()
        .find(|entry| entry.id.trim().eq_ignore_ascii_case(engine.id()))
}

fn search_engine_api_key(config: &AppConfig, engine: SearchEngine) -> Option<String> {
    search_engine_config_entry(config, engine)
        .and_then(|entry| entry.api_key.as_deref())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
}

fn search_engine_enabled(config: &AppConfig, engine: SearchEngine) -> bool {
    search_engine_config_entry(config, engine).is_some_and(|entry| entry.enabled)
}

/// 自动模式的候选集：已启用且（有密钥 或 本身免密钥）。
fn active_search_engines(config: &AppConfig) -> Vec<ReadySearchEngine> {
    SearchEngine::ALL
        .into_iter()
        .filter(|engine| search_engine_enabled(config, *engine))
        .filter_map(|engine| {
            let api_key = search_engine_api_key(config, engine);
            if api_key.is_none() && !engine.is_keyless() {
                return None;
            }
            Some(ReadySearchEngine { engine, api_key })
        })
        .collect()
}

/// 锁定模式的目标引擎。只有「已启用 + 填了密钥」才成立，否则回落自动模式。
fn fixed_search_engine(config: &AppConfig) -> Option<ReadySearchEngine> {
    let SearchMode::Fixed { engine_id } = &config.search_mode else {
        return None;
    };
    let engine = SearchEngine::from_id(engine_id)?;
    if !search_engine_enabled(config, engine) {
        return None;
    }
    let api_key = search_engine_api_key(config, engine)?;
    Some(ReadySearchEngine { engine, api_key: Some(api_key) })
}

// ==================== 密钥打码与配置规范化 ====================

fn mask_search_engine_key(value: &str) -> String {
    let trimmed = value.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() <= 4 {
        return "****".to_string();
    }
    let head = chars.iter().take(2).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}***{tail}")
}

/// 回传前端前把密钥换成掩码。掩码本身带 `***`，保存时据此识别「未改动」。
fn mask_search_engine_keys(config: &mut AppConfig) {
    for entry in &mut config.search_engines {
        if let Some(key) = entry.api_key.as_deref() {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                entry.api_key = None;
            } else {
                entry.api_key = Some(mask_search_engine_key(trimmed));
            }
        }
    }
}

/// 保存时合并掩码：前端把掩码原样带回，说明用户没改这一栏，沿用磁盘上的旧密钥。
fn merge_masked_search_engine_keys(stored: &AppConfig, incoming: &mut AppConfig) {
    for entry in &mut incoming.search_engines {
        let Some(incoming_key) = entry
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| key.contains("***"))
            .map(ToOwned::to_owned)
        else {
            continue;
        };
        let stored_key = SearchEngine::from_id(&entry.id)
            .and_then(|engine| search_engine_config_entry(stored, engine))
            .and_then(|stored_entry| stored_entry.api_key.as_deref())
            .map(str::trim)
            .filter(|key| !key.is_empty() && mask_search_engine_key(key) == incoming_key)
            .map(ToOwned::to_owned);
        // 对不上就清空，绝不把掩码本身写进配置。
        entry.api_key = stored_key;
    }
}

/// 规范化：补齐缺失的内置引擎、剔掉未知 id、去重；锁定目标不合法时回落自动模式。
fn normalize_search_engines(config: &mut AppConfig) {
    let defaults = default_search_engines();
    let mut normalized = Vec::<SearchEngineConfig>::with_capacity(defaults.len());
    for default_entry in defaults {
        let engine = SearchEngine::from_id(&default_entry.id);
        let existing = engine.and_then(|engine| {
            config
                .search_engines
                .iter()
                .find(|entry| entry.id.trim().eq_ignore_ascii_case(engine.id()))
                .cloned()
        });
        let mut entry = existing.unwrap_or(default_entry);
        entry.id = entry.id.trim().to_ascii_lowercase();
        entry.api_key = entry
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(ToOwned::to_owned);
        normalized.push(entry);
    }
    config.search_engines = normalized;

    if let SearchMode::Fixed { engine_id } = config.search_mode.clone() {
        let engine = SearchEngine::from_id(&engine_id);
        let lockable = engine.is_some_and(|engine| {
            let has_key = search_engine_api_key(config, engine).is_some();
            has_key && search_engine_enabled(config, engine)
        });
        if !lockable {
            runtime_log_warn(format!(
                "[搜索引擎] 锁定目标不可用，回落自动模式：engine_id={engine_id}"
            ));
            config.search_mode = SearchMode::Auto;
        }
    }
}

// ==================== 单个引擎调用 ====================

fn parse_retry_after_seconds(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(DEFAULT_SEARCH_COOLDOWN_SECONDS)
        .min(MAX_SEARCH_COOLDOWN_SECONDS)
}

fn search_status_error(
    status: reqwest::StatusCode,
    retry_after: Option<&str>,
    body: &str,
) -> Option<SearchCallError> {
    if status.is_success() {
        return None;
    }
    let code = status.as_u16();
    let body_snippet = truncate_by_chars(&clean_text(body), SEARCH_FAILURE_SNIPPET_CHAR_LIMIT);
    let detail = if body_snippet.is_empty() {
        format!("status {code}")
    } else {
        format!("status {code}: {body_snippet}")
    };
    if code == 429 {
        return Some(SearchCallError::Cooldown {
            seconds: parse_retry_after_seconds(retry_after),
            message: detail,
        });
    }
    if code == 401 || code == 403 {
        return Some(SearchCallError::Auth(detail));
    }
    Some(SearchCallError::Transient(detail))
}

async fn perform_search_request(
    request: reqwest::RequestBuilder,
) -> Result<(String, Option<String>), SearchCallError> {
    let response = request
        .send()
        .await
        .map_err(|err| SearchCallError::transient(format!("请求失败：{err}")))?;
    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let body = response
        .text()
        .await
        .map_err(|err| SearchCallError::transient(format!("读取响应失败：{err}")))?;
    if let Some(error) = search_status_error(status, retry_after.as_deref(), &body) {
        return Err(error);
    }
    Ok((body, retry_after))
}

/// 解析 MCP 响应体：可能是纯 JSON，也可能是 SSE（每行 `data: {...}`）。
fn parse_mcp_result(body: &str) -> Result<Value, SearchCallError> {
    let mut payloads = body
        .lines()
        .filter_map(|line| line.trim_end().strip_prefix("data:"))
        .map(|rest| rest.trim().to_string())
        .filter(|payload| !payload.is_empty())
        .collect::<Vec<_>>();
    if payloads.is_empty() {
        payloads.push(body.trim().to_string());
    }
    let mut last_error: Option<String> = None;
    for payload in payloads {
        let value: Value = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(err) => {
                last_error = Some(format!("响应不是合法 JSON：{err}"));
                continue;
            }
        };
        if let Some(error) = value.get("error") {
            return Err(SearchCallError::transient(format!("MCP 返回错误：{error}")));
        }
        if let Some(result) = value.get("result") {
            return Ok(result.clone());
        }
    }
    Err(SearchCallError::transient(
        last_error.unwrap_or_else(|| "MCP 响应缺少 result".to_string()),
    ))
}

/// 取 MCP `content[].text`；`isError: true` 视为一次普通失败（不记冷却）。
fn mcp_result_text(result: &Value) -> Result<String, SearchCallError> {
    if result.get("isError").and_then(Value::as_bool) == Some(true) {
        let text = result
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(SearchCallError::transient(format!(
            "引擎返回错误：{}",
            truncate_by_chars(&clean_text(&text), SEARCH_FAILURE_SNIPPET_CHAR_LIMIT)
        )));
    }
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return Err(SearchCallError::transient("引擎返回空内容"));
    }
    Ok(text)
}

async fn call_mcp_search_tool(
    client: &reqwest::Client,
    url: &str,
    tool_name: &str,
    arguments: Value,
    api_key: Option<&str>,
) -> Result<Value, SearchCallError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool_name, "arguments": arguments },
    });
    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&body);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {key}"));
    }
    let (response_body, _retry_after) = perform_search_request(request).await?;
    parse_mcp_result(&response_body)
}

fn search_result_row(title: &str, url: &str, snippet: &str) -> Option<Value> {
    let title = clean_text(title);
    let url = url.trim();
    if title.is_empty() || url.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "title": title,
        "url": url,
        "snippet": truncate_by_chars(&clean_text(snippet), SEARCH_SNIPPET_CHAR_LIMIT),
    }))
}

/// Exa 把结果拼成一段带 `Title: / URL: / Highlights:` 前缀的文本，按前缀切块还原。
fn parse_exa_search_text(text: &str) -> Vec<Value> {
    let mut rows = Vec::<Value>::new();
    let mut title = String::new();
    let mut url = String::new();
    let mut snippet = String::new();
    let mut in_highlights = false;

    fn flush(rows: &mut Vec<Value>, title: &str, url: &str, snippet: &str) {
        if let Some(row) = search_result_row(title, url, snippet) {
            rows.push(row);
        }
    }

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Title: ") {
            flush(&mut rows, &title, &url, &snippet);
            title = value.trim().to_string();
            url.clear();
            snippet.clear();
            in_highlights = false;
        } else if let Some(value) = line.strip_prefix("URL: ") {
            url = value.trim().to_string();
        } else if line.starts_with("Highlights:") {
            in_highlights = true;
        } else if line.starts_with("Published:") || line.starts_with("Author:") {
            in_highlights = false;
        } else if in_highlights && !line.trim().is_empty() {
            if !snippet.is_empty() {
                snippet.push(' ');
            }
            snippet.push_str(line.trim());
        }
    }
    flush(&mut rows, &title, &url, &snippet);
    rows
}

/// Parallel 的 `content[0].text` 是一段 JSON 文本，results 里带 excerpts。
fn parse_parallel_search_text(text: &str) -> Result<Vec<Value>, SearchCallError> {
    let parsed: Value = serde_json::from_str(text.trim())
        .map_err(|err| SearchCallError::transient(format!("Parallel 结果不是 JSON：{err}")))?;
    let items = parsed
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rows = items
        .iter()
        .filter_map(|item| {
            let title = item.get("title").and_then(Value::as_str).unwrap_or_default();
            let url = item.get("url").and_then(Value::as_str).unwrap_or_default();
            let snippet = item
                .get("excerpts")
                .and_then(Value::as_array)
                .and_then(|excerpts| excerpts.first())
                .and_then(Value::as_str)
                .unwrap_or_default();
            search_result_row(title, url, snippet)
        })
        .collect::<Vec<_>>();
    Ok(rows)
}

/// Firecrawl 的返回结构随版本变化，按「含 url 的对象」递归收集，缺字段就跳过。
fn collect_url_objects(value: &Value, rows: &mut Vec<Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_url_objects(item, rows);
            }
        }
        Value::Object(map) => {
            if let Some(url) = map.get("url").and_then(Value::as_str) {
                let title = map
                    .get("title")
                    .or_else(|| map.get("metadata").and_then(|meta| meta.get("title")))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let snippet = map
                    .get("description")
                    .or_else(|| map.get("snippet"))
                    .or_else(|| map.get("markdown"))
                    .or_else(|| map.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if let Some(row) = search_result_row(title, url, snippet) {
                    rows.push(row);
                }
                return;
            }
            for nested in map.values() {
                collect_url_objects(nested, rows);
            }
        }
        _ => {}
    }
}

fn parse_firecrawl_search_text(text: &str) -> Vec<Value> {
    let mut rows = Vec::<Value>::new();
    if let Ok(parsed) = serde_json::from_str::<Value>(text.trim()) {
        collect_url_objects(&parsed, &mut rows);
    }
    rows
}

fn parse_rest_results(items: &[Value], snippet_fields: &[&str]) -> Vec<Value> {
    items
        .iter()
        .filter_map(|item| {
            let title = item.get("title").and_then(Value::as_str).unwrap_or_default();
            let url = item.get("url").and_then(Value::as_str).unwrap_or_default();
            let snippet = snippet_fields
                .iter()
                .find_map(|field| {
                    item.get(*field)
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                })
                .unwrap_or_default();
            search_result_row(title, url, snippet)
        })
        .collect()
}

async fn exa_search(
    client: &reqwest::Client,
    api_key: Option<&str>,
    query: &str,
) -> Result<Vec<Value>, SearchCallError> {
    let result = call_mcp_search_tool(
        client,
        EXA_SEARCH_MCP_URL,
        MCP_TOOL_NAME_EXA_SEARCH,
        serde_json::json!({
            "query": query,
            // Exa 的 schema 把 objective 也标成必填，没有单独目标时用查询本身。
            "objective": query,
            "numResults": SEARCH_RESULT_LIMIT,
        }),
        api_key,
    )
    .await?;
    let text = mcp_result_text(&result)?;
    Ok(parse_exa_search_text(&text))
}

async fn parallel_search(
    client: &reqwest::Client,
    api_key: Option<&str>,
    query: &str,
) -> Result<Vec<Value>, SearchCallError> {
    let result = call_mcp_search_tool(
        client,
        PARALLEL_SEARCH_MCP_URL,
        MCP_TOOL_NAME_PARALLEL_SEARCH,
        serde_json::json!({
            "objective": query,
            "search_queries": [query],
        }),
        api_key,
    )
    .await?;
    let text = mcp_result_text(&result)?;
    parse_parallel_search_text(&text)
}

async fn firecrawl_search(
    client: &reqwest::Client,
    api_key: Option<&str>,
    query: &str,
) -> Result<Vec<Value>, SearchCallError> {
    let result = call_mcp_search_tool(
        client,
        FIRECRAWL_SEARCH_MCP_URL,
        MCP_TOOL_NAME_FIRECRAWL_SEARCH,
        serde_json::json!({
            "query": query,
            "limit": SEARCH_RESULT_LIMIT,
        }),
        api_key,
    )
    .await?;
    let text = mcp_result_text(&result)?;
    Ok(parse_firecrawl_search_text(&text))
}

async fn tavily_search(
    client: &reqwest::Client,
    api_key: Option<&str>,
    query: &str,
) -> Result<Vec<Value>, SearchCallError> {
    let mut request = client
        .post(TAVILY_SEARCH_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": query,
            "max_results": SEARCH_RESULT_LIMIT,
            "include_answer": false,
        }));
    // 没填密钥就走服务商的免密钥额度，这个头是它的开关。
    request = match api_key {
        Some(key) => request.header("Authorization", format!("Bearer {key}")),
        None => request.header("X-Tavily-Access-Mode", "keyless"),
    };
    let (body, _retry_after) = perform_search_request(request).await?;
    let parsed: Value = serde_json::from_str(body.trim())
        .map_err(|err| SearchCallError::transient(format!("Tavily 响应不是 JSON：{err}")))?;
    let items = parsed
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(parse_rest_results(&items, &["content", "snippet", "description"]))
}

async fn deepseek_search(
    client: &reqwest::Client,
    api_key: Option<&str>,
    query: &str,
) -> Result<Vec<Value>, SearchCallError> {
    let Some(api_key) = api_key else {
        return Err(SearchCallError::Auth("DeepSeek 搜索必须填写 API key".to_string()));
    };
    let request = client
        .post(DEEPSEEK_SEARCH_URL)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&serde_json::json!({
            "query": truncate_by_chars(query, 128),
            "count": SEARCH_RESULT_LIMIT,
        }));
    let (body, _retry_after) = perform_search_request(request).await?;
    let parsed: Value = serde_json::from_str(body.trim())
        .map_err(|err| SearchCallError::transient(format!("DeepSeek 响应不是 JSON：{err}")))?;
    let items = parsed
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(parse_rest_results(&items, &["snippet", "summary", "content"]))
}

async fn call_search_engine(
    client: &reqwest::Client,
    engine: SearchEngine,
    api_key: Option<&str>,
    query: &str,
) -> Result<Vec<Value>, SearchCallError> {
    let rows = match engine {
        SearchEngine::Exa => exa_search(client, api_key, query).await?,
        SearchEngine::Parallel => parallel_search(client, api_key, query).await?,
        SearchEngine::Firecrawl => firecrawl_search(client, api_key, query).await?,
        SearchEngine::Tavily => tavily_search(client, api_key, query).await?,
        SearchEngine::Deepseek => deepseek_search(client, api_key, query).await?,
    };
    if rows.is_empty() {
        return Err(SearchCallError::transient("没有解析出可用结果"));
    }
    Ok(rows)
}

// ==================== 对外：一次搜索 ====================

fn search_payload(engine_id: &str, engine_name: &str, results: Vec<Value>) -> Value {
    serde_json::json!({
        "engine": { "id": engine_id, "name": engine_name },
        "results": results,
    })
}

const SEARCH_FAILED_HEADER: &str =
    "搜索失败：当前搜索引擎免费额度耗尽，使用注册 key 可以获得更大额度";
const SEARCH_NO_ENGINE_MESSAGE: &str =
    "搜索失败：没有启用任何搜索引擎。请在设置 → 搜索引擎里启用一个（exa / parallel / tavily 免密钥可用）";
/// 引擎回了结果但一条都不沾查询时的说明行：宁可交回真实结果，也别告诉用户搜不到。
const WEAK_RELEVANCE_NOTICE: &str = "以下结果与查询的相关性可能较低，请核对后再使用";

/// 可用引擎全在冷却：限流没到期，跟额度、密钥都无关，不能套「额度耗尽」那句。
fn search_all_cooling_message(cooling: &str) -> String {
    format!("搜索失败：可用引擎都在冷却中（限流未到期），请稍后重试\n- {cooling} 冷却中")
}

/// 失败文案：一行主标题 + 逐条原因，永远不出现空条目。
/// 原因照实列：鉴权失败、网络错误、限流冷却在原因里一眼能区分，不会被主标题带偏。
fn search_failure_message(failures: &[String]) -> String {
    if failures.is_empty() {
        return format!("{SEARCH_FAILED_HEADER}\n- 没有拿到任何失败原因");
    }
    format!("{SEARCH_FAILED_HEADER}\n- {}", failures.join("\n- "))
}

fn search_payload_with_notice(
    engine_id: &str,
    engine_name: &str,
    results: Vec<Value>,
    notice: &str,
) -> Value {
    serde_json::json!({
        "engine": { "id": engine_id, "name": engine_name },
        "notice": notice,
        "results": results,
    })
}

/// 一个引擎的一次结果：相关性过关的留用，原始行留着当兜底。
fn split_engine_rows(
    query: &str,
    engine: SearchEngine,
    rows: Vec<Value>,
) -> (Vec<Value>, Vec<Value>) {
    let kept = filter_relevant_search_results(query, rows.clone());
    if kept.is_empty() {
        runtime_log_warn(format!(
            "[搜索引擎] 结果与查询无关，先留原始结果备用：engine={}，query={query}，条数={}",
            engine.id(),
            rows.len()
        ));
    }
    (kept, rows)
}

enum SearchAttempt {
    Hit(Value),
    Missed {
        failures: Vec<String>,
        weak: Option<(SearchEngine, Vec<Value>)>,
    },
}

/// 按顺序试候选引擎。`call` 抽成参数，行为用例才能不依赖真实网络。
async fn try_candidates_with<C, Fut>(
    raw_query: &str,
    session_key: &str,
    candidates: &[ReadySearchEngine],
    ordered: Vec<SearchEngine>,
    mut call: C,
) -> SearchAttempt
where
    C: FnMut(SearchEngine, Option<String>) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Value>, SearchCallError>>,
{
    let mut failures = Vec::<String>::new();
    let mut weak: Option<(SearchEngine, Vec<Value>)> = None;
    for engine in ordered {
        let Some(candidate) = candidates.iter().find(|item| item.engine == engine) else {
            continue;
        };
        let api_key = candidate.api_key.clone();
        // 选源与占用成对：离开本轮作用域（任何分支）都会把在飞计数减回去。
        let _lease = acquire_search_engine(engine);
        match call(engine, api_key).await {
            Ok(rows) => {
                let (kept, raw) = split_engine_rows(raw_query, engine, rows);
                if !kept.is_empty() {
                    with_search_scheduler(|scheduler| {
                        scheduler.remember_success(session_key, engine)
                    });
                    return SearchAttempt::Hit(search_payload(
                        engine.id(),
                        engine.display_name(),
                        kept,
                    ));
                }
                if weak.is_none() {
                    weak = Some((engine, raw));
                }
            }
            Err(SearchCallError::Auth(message)) => {
                // 一个填错的旧 key 不该让其它源一起失效：换源，但原因原样留下。
                runtime_log_warn(format!(
                    "[搜索引擎] 鉴权失败，换下一个源：engine={}，error={message}",
                    engine.id()
                ));
                failures.push(format!(
                    "{}: 鉴权失败，请检查密钥（{message}）",
                    engine.display_name()
                ));
            }
            Err(SearchCallError::Cooldown { seconds, message }) => {
                with_search_scheduler(|scheduler| {
                    scheduler.set_cooldown(engine, seconds, std::time::Instant::now())
                });
                runtime_log_warn(format!(
                    "[搜索引擎] 限流，记冷却后换源：engine={}，冷却秒数={seconds}，error={message}",
                    engine.id()
                ));
                failures.push(format!(
                    "{}: 限流，冷却 {seconds} 秒（{message}）",
                    engine.display_name()
                ));
            }
            Err(SearchCallError::Transient(message)) => {
                runtime_log_warn(format!(
                    "[搜索引擎] 调用失败，换下一个源：engine={}，error={message}",
                    engine.id()
                ));
                failures.push(format!("{}: {message}", engine.display_name()));
            }
        }
    }
    SearchAttempt::Missed { failures, weak }
}

/// 一次搜索的入口：按配置选择引擎、调度、收口。
async fn builtin_search(
    state: &AppState,
    query: &str,
    session_key: &str,
) -> Result<Value, String> {
    let raw_query = query.trim();
    if raw_query.is_empty() {
        return Err("搜索关键词为空".to_string());
    }
    let config = state_read_config_cached(state)?;

    // 锁定模式：只用指定的引擎，不看冷却、不换源，失败原样返回。
    if let Some(fixed) = fixed_search_engine(&config) {
        let rows = call_search_engine(
            &state.shared_http_client,
            fixed.engine,
            fixed.api_key.as_deref(),
            raw_query,
        )
            .await
            .map_err(|err| {
                format!("{} 搜索失败：{}", fixed.engine.display_name(), err.message())
            })?;
        // 引擎说成功不等于搜到了东西：乱码查询下它照样回一屏无关页面。
        let (kept, raw) = split_engine_rows(raw_query, fixed.engine, rows);
        if kept.is_empty() {
            runtime_log_info(format!(
                "[搜索引擎] 锁定引擎的结果都不沾查询，交回原始结果：engine={}，query={raw_query}",
                fixed.engine.id()
            ));
            return Ok(search_payload_with_notice(
                fixed.engine.id(),
                fixed.engine.display_name(),
                raw,
                WEAK_RELEVANCE_NOTICE,
            ));
        }
        return Ok(search_payload(
            fixed.engine.id(),
            fixed.engine.display_name(),
            kept,
        ));
    }

    let candidates = active_search_engines(&config);
    if candidates.is_empty() {
        return Err(SEARCH_NO_ENGINE_MESSAGE.to_string());
    }
    let candidate_engines = candidates
        .iter()
        .map(|candidate| candidate.engine)
        .collect::<Vec<_>>();
    let now = std::time::Instant::now();
    let ordered = with_search_scheduler(|scheduler| {
        scheduler.plan(session_key, &candidate_engines, now)
    });
    let cooling = with_search_scheduler(|scheduler| {
        candidates
            .iter()
            .filter(|candidate| scheduler.is_cooling(candidate.engine, now))
            .map(|candidate| candidate.engine.display_name())
            .collect::<Vec<_>>()
    });
    if ordered.is_empty() {
        runtime_log_info(format!(
            "[搜索引擎] 候选引擎都在冷却中：query={raw_query}，冷却中的引擎={}",
            cooling.join("、")
        ));
        return Err(search_all_cooling_message(&cooling.join("、")));
    }

    let mut plan_failures = Vec::<String>::new();
    if !cooling.is_empty() {
        // 部分冷却也要留痕，否则用户看不出某个源这次被跳过了。
        plan_failures.push(format!(
            "{} 冷却中（限流未到期），本次跳过",
            cooling.join("、")
        ));
    }
    let attempt = try_candidates_with(
        raw_query,
        session_key,
        &candidates,
        ordered,
        |engine, api_key| {
            let client = state.shared_http_client.clone();
            let query = raw_query.to_string();
            async move { call_search_engine(&client, engine, api_key.as_deref(), &query).await }
        },
    )
    .await;
    match attempt {
        SearchAttempt::Hit(payload) => Ok(payload),
        SearchAttempt::Missed {
            failures: mut engine_failures,
            weak,
        } => {
            plan_failures.append(&mut engine_failures);
            if let Some((engine, rows)) = weak {
                runtime_log_info(format!(
                    "[搜索引擎] 没有引擎给出沾边的结果，交回 {} 的原始结果：query={raw_query}，条数={}",
                    engine.id(),
                    rows.len()
                ));
                return Ok(search_payload_with_notice(
                    engine.id(),
                    engine.display_name(),
                    rows,
                    WEAK_RELEVANCE_NOTICE,
                ));
            }
            Err(search_failure_message(&plan_failures))
        }
    }
}

/// 测试连接：拿一次真实查询打过去，只回报成功/失败原因，不写冷却也不记成功。
async fn test_search_engine_connection(
    state: &AppState,
    engine_id: &str,
    api_key: Option<String>,
) -> Result<Value, String> {
    let engine = SearchEngine::from_id(engine_id)
        .ok_or_else(|| format!("未知的搜索引擎：{engine_id}"))?;
    let typed_key = api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains("***"))
        .map(ToOwned::to_owned);
    // 输入框里可能是掩码（用户没改过这一栏），这种情况下拿磁盘上的真密钥去测。
    let key = match typed_key {
        Some(key) => Some(key),
        None => state_read_config_cached(state)
            .ok()
            .and_then(|config| search_engine_api_key(&config, engine)),
    };
    if key.is_none() && !engine.is_keyless() {
        return Err(format!("{} 必须填写 API key 才能使用", engine.display_name()));
    }
    match call_search_engine(
        &state.shared_http_client,
        engine,
        key.as_deref(),
        "P-ai search connectivity test",
    )
    .await
    {
        Ok(rows) => Ok(serde_json::json!({
            "ok": true,
            "engine": { "id": engine.id(), "name": engine.display_name() },
            "resultCount": rows.len(),
        })),
        Err(err) => Err(format!("{} 连接失败：{}", engine.display_name(), err.message())),
    }
}

#[cfg(test)]
mod search_dispatch_tests {
    use super::*;

    fn instant() -> std::time::Instant {
        std::time::Instant::now()
    }

    fn all_engines() -> Vec<SearchEngine> {
        SearchEngine::ALL.to_vec()
    }

    #[test]
    fn scheduler_plan_should_cover_every_candidate_without_history() {
        let mut scheduler = SearchEngineScheduler::default();
        let planned = scheduler.plan("session-a", &all_engines(), instant());
        let mut sorted = planned.clone();
        sorted.sort_by_key(|engine| engine.id());
        let mut expected = all_engines();
        expected.sort_by_key(|engine| engine.id());
        assert_eq!(sorted, expected, "没有历史时不能漏掉候选");
    }

    #[test]
    fn scheduler_plan_should_prefer_last_success_engine() {
        let mut scheduler = SearchEngineScheduler::default();
        scheduler.remember_success("session-a", SearchEngine::Tavily);
        let planned = scheduler.plan("session-a", &all_engines(), instant());
        assert_eq!(planned.first(), Some(&SearchEngine::Tavily));
        let other = scheduler.plan("session-b", &all_engines(), instant());
        assert_eq!(other.len(), all_engines().len());
    }

    #[test]
    fn scheduler_plan_should_skip_cooling_engine_until_expiry() {
        let mut scheduler = SearchEngineScheduler::default();
        let now = instant();
        scheduler.set_cooldown(SearchEngine::Exa, 60, now);
        let planned = scheduler.plan("session-a", &all_engines(), now);
        assert!(!planned.contains(&SearchEngine::Exa), "冷却中的引擎不该进候选");
        assert_eq!(planned.len(), all_engines().len() - 1);
        let later = now + std::time::Duration::from_secs(61);
        let planned_later = scheduler.plan("session-a", &all_engines(), later);
        assert!(planned_later.contains(&SearchEngine::Exa), "冷却到期后要重新可选");
    }

    #[test]
    fn scheduler_plan_should_return_empty_when_all_cooling() {
        let mut scheduler = SearchEngineScheduler::default();
        let now = instant();
        for engine in all_engines() {
            scheduler.set_cooldown(engine, 30, now);
        }
        assert!(scheduler.plan("session-a", &all_engines(), now).is_empty());
    }

    #[test]
    fn parse_retry_after_should_fallback_and_clamp() {
        assert_eq!(parse_retry_after_seconds(None), DEFAULT_SEARCH_COOLDOWN_SECONDS);
        assert_eq!(parse_retry_after_seconds(Some("")), DEFAULT_SEARCH_COOLDOWN_SECONDS);
        assert_eq!(parse_retry_after_seconds(Some("abc")), DEFAULT_SEARCH_COOLDOWN_SECONDS);
        assert_eq!(parse_retry_after_seconds(Some("0")), DEFAULT_SEARCH_COOLDOWN_SECONDS);
        assert_eq!(parse_retry_after_seconds(Some("17")), 17);
        assert_eq!(parse_retry_after_seconds(Some("999999")), MAX_SEARCH_COOLDOWN_SECONDS);
    }

    #[test]
    fn search_status_error_should_classify_rate_limit_auth_and_transient() {
        match search_status_error(reqwest::StatusCode::TOO_MANY_REQUESTS, Some("12"), "slow down") {
            Some(SearchCallError::Cooldown { seconds, .. }) => assert_eq!(seconds, 12),
            _ => panic!("429 应该记冷却"),
        }
        for status in [reqwest::StatusCode::UNAUTHORIZED, reqwest::StatusCode::FORBIDDEN] {
            match search_status_error(status, None, "bad key") {
                Some(SearchCallError::Auth(_)) => {}
                _ => panic!("401/403 应该判成鉴权失败"),
            }
        }
        match search_status_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, None, "boom") {
            Some(SearchCallError::Transient(_)) => {}
            _ => panic!("5xx 应该判成可换源失败"),
        }
        assert!(search_status_error(reqwest::StatusCode::OK, None, "").is_none());
    }

    #[test]
    fn parse_exa_search_text_should_split_title_url_and_highlights() {
        let text = "Title: 第一篇\nURL: https://a.example/1\nPublished: 2026-01-01\nAuthor: x\nHighlights:\n第一段摘要\n第二段摘要\n\nTitle: 第二篇\nURL: https://b.example/2\nHighlights:\n只有一段";
        let rows = parse_exa_search_text(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["title"], "第一篇");
        assert_eq!(rows[0]["url"], "https://a.example/1");
        let first_snippet = rows[0]["snippet"].as_str().unwrap_or_default();
        assert!(first_snippet.contains("第一段摘要"), "snippet={first_snippet}");
        assert!(first_snippet.contains("第二段摘要"), "snippet={first_snippet}");
        assert_eq!(rows[1]["title"], "第二篇");
    }

    #[test]
    fn parse_exa_search_text_should_skip_rows_without_url() {
        let rows = parse_exa_search_text("Title: 没有链接\nHighlights:\n内容");
        assert!(rows.is_empty());
    }

    #[test]
    fn parse_parallel_search_text_should_map_excerpts() {
        let text = r#"{"search_id":"s1","results":[{"url":"https://a.example/1","title":"标题一","excerpts":["摘要一","摘要二"]},{"url":"https://b.example/2","title":"","excerpts":[]}]}"#;
        let rows = parse_parallel_search_text(text).expect("应能解析");
        assert_eq!(rows.len(), 1, "缺标题的条目要跳过");
        assert_eq!(rows[0]["title"], "标题一");
        assert_eq!(rows[0]["snippet"], "摘要一");
        assert!(parse_parallel_search_text("not json").is_err());
    }

    #[test]
    fn parse_firecrawl_search_text_should_collect_nested_url_objects() {
        let text = r#"{"data":{"web":[{"url":"https://a.example/1","title":"标题一","description":"描述一"},{"url":"https://b.example/2","metadata":{"title":"标题二"},"markdown":"正文二"}]}}"#;
        let rows = parse_firecrawl_search_text(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["title"], "标题一");
        assert_eq!(rows[0]["snippet"], "描述一");
        assert_eq!(rows[1]["title"], "标题二");
        assert_eq!(rows[1]["snippet"], "正文二");
        assert!(parse_firecrawl_search_text("不是 JSON").is_empty());
    }

    #[test]
    fn parse_rest_results_should_fallback_between_snippet_fields() {
        let items = vec![
            serde_json::json!({"title":"一","url":"https://a/1","summary":"摘要兜底"}),
            serde_json::json!({"title":"二","url":"https://b/2","snippet":"直接摘要"}),
            serde_json::json!({"title":"三"}),
        ];
        let rows = parse_rest_results(&items, &["snippet", "summary"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["snippet"], "摘要兜底");
        assert_eq!(rows[1]["snippet"], "直接摘要");
    }

    #[test]
    fn active_search_engines_should_respect_enabled_key_and_keyless() {
        let mut config = AppConfig::default();
        let active = active_search_engines(&config);
        assert_eq!(active.len(), 3, "默认开启 exa / parallel / tavily");
        assert!(active.iter().all(|item| item.engine != SearchEngine::Deepseek));

        if let Some(entry) = config.search_engines.iter_mut().find(|entry| entry.id == "tavily") {
            entry.enabled = false;
        }
        assert_eq!(active_search_engines(&config).len(), 2);

        for entry in &mut config.search_engines {
            entry.enabled = false;
        }
        if let Some(entry) = config.search_engines.iter_mut().find(|entry| entry.id == "deepseek") {
            entry.enabled = true;
        }
        assert!(active_search_engines(&config).is_empty(), "DeepSeek 没密钥不进候选");

        if let Some(entry) = config.search_engines.iter_mut().find(|entry| entry.id == "deepseek") {
            entry.api_key = Some("sk-test".to_string());
        }
        let active = active_search_engines(&config);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].engine, SearchEngine::Deepseek);
    }

    #[test]
    fn fixed_search_engine_should_require_key_and_enabled_state() {
        let mut config = AppConfig::default();
        config.search_mode = SearchMode::Fixed { engine_id: "tavily".to_string() };
        assert!(fixed_search_engine(&config).is_none(), "免密钥引擎没填密钥不能锁定");

        if let Some(entry) = config.search_engines.iter_mut().find(|entry| entry.id == "tavily") {
            entry.api_key = Some("tvly-test".to_string());
        }
        let fixed = fixed_search_engine(&config).expect("填了密钥应可锁定");
        assert_eq!(fixed.engine, SearchEngine::Tavily);

        if let Some(entry) = config.search_engines.iter_mut().find(|entry| entry.id == "tavily") {
            entry.enabled = false;
        }
        assert!(fixed_search_engine(&config).is_none(), "停用后锁定失效");
    }

    #[test]
    fn normalize_search_engines_should_fill_defaults_and_drop_unknown() {
        let mut config = AppConfig::default();
        config.search_engines = vec![
            SearchEngineConfig {
                id: " Tavily ".to_string(),
                enabled: false,
                api_key: Some("  tvly-key  ".to_string()),
            },
            SearchEngineConfig { id: "unknown-engine".to_string(), enabled: true, api_key: None },
        ];
        normalize_search_engines(&mut config);
        let ids = config
            .search_engines
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["exa", "parallel", "firecrawl", "tavily", "deepseek"]);
        let tavily = config
            .search_engines
            .iter()
            .find(|entry| entry.id == "tavily")
            .expect("tavily 应保留");
        assert!(!tavily.enabled);
        assert_eq!(tavily.api_key.as_deref(), Some("tvly-key"));
    }

    #[test]
    fn normalize_search_engines_should_fall_back_to_auto_when_lock_invalid() {
        let mut config = AppConfig::default();
        config.search_mode = SearchMode::Fixed { engine_id: "deepseek".to_string() };
        normalize_search_engines(&mut config);
        assert_eq!(config.search_mode, SearchMode::Auto, "没有密钥的锁定目标要回落自动");

        let mut config = AppConfig::default();
        if let Some(entry) = config.search_engines.iter_mut().find(|entry| entry.id == "deepseek") {
            entry.enabled = true;
            entry.api_key = Some("sk-test".to_string());
        }
        config.search_mode = SearchMode::Fixed { engine_id: "deepseek".to_string() };
        normalize_search_engines(&mut config);
        assert_eq!(config.search_mode, SearchMode::Fixed { engine_id: "deepseek".to_string() });
    }

    #[test]
    fn mask_and_merge_should_round_trip_keys() {
        let mut stored = AppConfig::default();
        if let Some(entry) = stored.search_engines.iter_mut().find(|entry| entry.id == "tavily") {
            entry.api_key = Some("tvly-secret-value".to_string());
        }

        let mut to_frontend = stored.clone();
        mask_search_engine_keys(&mut to_frontend);
        let masked = to_frontend
            .search_engines
            .iter()
            .find(|entry| entry.id == "tavily")
            .and_then(|entry| entry.api_key.clone())
            .expect("应有掩码");
        assert_ne!(masked, "tvly-secret-value");
        assert!(masked.contains("***"));

        let mut incoming = to_frontend.clone();
        merge_masked_search_engine_keys(&stored, &mut incoming);
        let merged = incoming
            .search_engines
            .iter()
            .find(|entry| entry.id == "tavily")
            .and_then(|entry| entry.api_key.clone());
        assert_eq!(merged.as_deref(), Some("tvly-secret-value"));

        let mut mismatched = to_frontend;
        if let Some(entry) = mismatched.search_engines.iter_mut().find(|entry| entry.id == "tavily") {
            entry.api_key = Some("zz***zz".to_string());
        }
        merge_masked_search_engine_keys(&stored, &mut mismatched);
        let cleared = mismatched
            .search_engines
            .iter()
            .find(|entry| entry.id == "tavily")
            .and_then(|entry| entry.api_key.clone());
        assert!(cleared.is_none(), "掩码对不上时必须清空，不能把掩码写进配置");
    }

    #[test]
    fn search_payload_should_carry_engine_identity() {
        let payload = search_payload("tavily", "Tavily", vec![serde_json::json!({"title": "t"})]);
        assert_eq!(payload["engine"]["id"], "tavily");
        assert_eq!(payload["engine"]["name"], "Tavily");
        assert_eq!(payload["results"].as_array().map(|items| items.len()), Some(1));
    }

    #[test]
    fn default_config_should_enable_keyless_engines_that_work() {
        let config = AppConfig::default();
        let enabled = config
            .search_engines
            .iter()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();
        // firecrawl 免密钥实测已不可用，默认关闭；DeepSeek 需要密钥，默认关闭。
        assert_eq!(enabled, vec!["exa", "parallel", "tavily"]);
        assert_eq!(config.search_mode, SearchMode::Auto);
    }

    #[test]
    fn search_mode_should_serialize_as_tagged_object() {
        let auto = serde_json::to_value(SearchMode::Auto).expect("序列化 Auto");
        assert_eq!(auto, serde_json::json!({"mode": "auto"}));
        let fixed = serde_json::to_value(SearchMode::Fixed { engine_id: "exa".to_string() })
            .expect("序列化 Fixed");
        assert_eq!(fixed, serde_json::json!({"mode": "fixed", "engineId": "exa"}));
        let parsed: SearchMode =
            serde_json::from_value(serde_json::json!({"mode": "fixed", "engineId": "exa"}))
                .expect("反序列化 Fixed");
        assert_eq!(parsed, SearchMode::Fixed { engine_id: "exa".to_string() });
    }

    #[test]
    fn old_config_without_search_fields_should_get_defaults() {
        let raw = serde_json::json!({"hotkey": "Alt+·"});
        let config: AppConfig = serde_json::from_value(raw).expect("老配置应能读入");
        assert_eq!(config.search_engines.len(), 5);
        assert_eq!(config.search_mode, SearchMode::Auto);
    }

    // ========== 并发分散 ==========

    #[test]
    fn scheduler_plan_should_spread_concurrent_calls_across_engines() {
        let mut scheduler = SearchEngineScheduler::default();
        let now = instant();
        let first = scheduler.plan("session-a", &all_engines(), now)[0];
        scheduler.begin_in_flight(first);
        let second = scheduler.plan("session-a", &all_engines(), now)[0];
        assert_ne!(first, second, "并发请求不该挑同一个引擎");
        assert!(
            !scheduler.plan("session-a", &all_engines(), now).contains(&first),
            "正在飞的引擎不该再进候选"
        );
        assert_eq!(scheduler.in_flight_count(first), 1);
        scheduler.finish_in_flight(first);
        assert_eq!(scheduler.in_flight_count(first), 0);
        assert!(
            scheduler.plan("session-a", &all_engines(), now).contains(&first),
            "请求结束后该引擎要重新可选"
        );
    }

    #[test]
    fn scheduler_plan_should_fall_back_when_every_engine_is_in_flight() {
        let mut scheduler = SearchEngineScheduler::default();
        let now = instant();
        for engine in all_engines() {
            scheduler.begin_in_flight(engine);
        }
        let planned = scheduler.plan("session-a", &all_engines(), now);
        assert_eq!(
            planned.len(),
            all_engines().len(),
            "全在飞时也要给出可用引擎，不能返回空候选"
        );
    }

    #[test]
    fn scheduler_plan_should_reuse_single_engine_even_when_in_flight() {
        let mut scheduler = SearchEngineScheduler::default();
        scheduler.begin_in_flight(SearchEngine::Tavily);
        let planned = scheduler.plan(
            "session-a",
            &[SearchEngine::Tavily],
            instant(),
        );
        assert_eq!(planned, vec![SearchEngine::Tavily]);
    }

    #[test]
    fn search_engine_lease_should_release_in_flight_count_on_drop() {
        let lease = acquire_search_engine(SearchEngine::Exa);
        assert_eq!(
            with_search_scheduler(|scheduler| scheduler.in_flight_count(SearchEngine::Exa)),
            1
        );
        drop(lease);
        assert_eq!(
            with_search_scheduler(|scheduler| scheduler.in_flight_count(SearchEngine::Exa)),
            0
        );
    }

    #[test]
    fn weak_relevance_payload_should_carry_notice_and_raw_rows() {
        let payload = search_payload_with_notice(
            "exa",
            "Exa",
            vec![serde_json::json!({"title": "日本酒店预订"})],
            WEAK_RELEVANCE_NOTICE,
        );
        assert_eq!(payload["engine"]["id"], "exa");
        assert_eq!(payload["notice"], WEAK_RELEVANCE_NOTICE);
        assert_eq!(payload["results"].as_array().map(|items| items.len()), Some(1));
        assert!(
            WEAK_RELEVANCE_NOTICE.contains("相关性"),
            "说明行要讲清是相关性存疑，不是搜不到"
        );
    }

    #[test]
    fn search_no_engine_message_should_point_to_settings() {
        // 没有已启用引擎是配置问题，不能套「额度耗尽」那句。
        assert!(SEARCH_NO_ENGINE_MESSAGE.contains("没有启用任何搜索引擎"));
        assert!(SEARCH_NO_ENGINE_MESSAGE.contains("设置"));
        assert!(!SEARCH_NO_ENGINE_MESSAGE.contains("额度"));
    }

    #[test]
    fn search_all_cooling_message_should_blame_rate_limit_not_quota() {
        let message = search_all_cooling_message("Exa、Tavily");
        assert!(message.contains("冷却中"));
        assert!(message.contains("Exa、Tavily"), "要说清是哪些引擎在冷却");
        assert!(!message.contains("额度"), "全冷却不能套额度文案：{message}");
        assert!(!message.contains("- \n"), "不要挂空条目：{message}");
    }

    #[test]
    fn split_engine_rows_should_keep_raw_rows_when_nothing_is_relevant() {
        let rows = vec![
            serde_json::json!({"title": "Booking hotels", "url": "https://hotel.example/1", "snippet": "reserve a room"}),
            serde_json::json!({"title": "Modern novel", "url": "https://novel.example/2", "snippet": "chapter one"}),
        ];
        let (kept, raw) = split_engine_rows("原神 纳西妲", SearchEngine::Exa, rows);
        assert!(kept.is_empty(), "英文页面跟中文查询不沾边");
        assert_eq!(raw.len(), 2, "原始结果要留着兜底，不能直接丢");
    }

    // ========== 失败文案 ==========

    #[test]
    fn search_failure_message_should_not_leave_empty_lines() {
        let message = search_failure_message(&[]);
        assert!(!message.contains("- \n"), "不要挂空条目：{message}");
        assert!(!message.trim_end().ends_with('-'));
        assert!(message.contains("没有拿到任何失败原因"));
    }

    #[test]
    fn search_failure_message_should_say_quota_and_keep_reasons() {
        // 原因只用真实路径能产生的三种：鉴权失败、限流冷却、网络/解析失败。
        let message = search_failure_message(&[
            "Exa: 鉴权失败，请检查密钥（401 Unauthorized）".to_string(),
            "Tavily: 限流，冷却 30 秒（429 Too Many Requests）".to_string(),
            "Parallel: 连接超时".to_string(),
        ]);
        let mut lines = message.lines();
        assert_eq!(
            lines.next(),
            Some(SEARCH_FAILED_HEADER),
            "主标题就是额度那句"
        );
        assert!(message.contains("免费额度耗尽"));
        assert!(message.contains("注册 key"));
        assert!(message.contains("鉴权失败，请检查密钥"), "原因要照实列");
        assert!(message.contains("冷却"), "冷却要说得出来");
        assert!(!message.contains("兜底"), "不能再暗示还有别的兜底");
        assert!(!message.contains("所有搜索引擎都没能返回结果"));
    }

    // ========== 记账回收 ==========

    #[test]
    fn scheduler_plan_should_drop_expired_cooldowns() {
        let mut scheduler = SearchEngineScheduler::default();
        let now = instant();
        scheduler.set_cooldown(SearchEngine::Exa, 30, now);
        assert_eq!(scheduler.cooldown_until.len(), 1);
        let later = now + std::time::Duration::from_secs(31);
        let _ = scheduler.plan("session-a", &all_engines(), later);
        assert!(
            scheduler.cooldown_until.is_empty(),
            "冷却到期后表里不该还留着条目"
        );
    }

    #[test]
    fn scheduler_should_cap_tracked_sessions() {
        let mut scheduler = SearchEngineScheduler::default();
        let now = instant();
        for index in 0..(MAX_TRACKED_SEARCH_SESSIONS + 10) {
            scheduler.remember_success(&format!("session-{index}"), SearchEngine::Exa);
        }
        let _ = scheduler.plan("session-keep", &all_engines(), now);
        assert!(
            scheduler.last_success.len() <= MAX_TRACKED_SEARCH_SESSIONS,
            "会话表要封顶，实际 {}",
            scheduler.last_success.len()
        );
    }

    // ========== 密钥不外泄 ==========

    fn config_with_search_key(key: &str) -> AppConfig {
        let mut config = AppConfig::default();
        config.search_engines[0].api_key = Some(key.to_string());
        config
    }

    #[test]
    fn transport_config_should_mask_search_engine_keys() {
        let config = config_for_transport(config_with_search_key("sk-plaintext-123456"));
        let key = config.search_engines[0].api_key.clone().unwrap_or_default();
        assert!(key.contains("***"), "回传前必须打码：{key}");
        assert!(!key.contains("plaintext"));
    }

    #[test]
    fn save_transport_config_should_not_leak_the_key_it_merged_back() {
        // 复现保存链路：前端带回掩码 → 合并出明文 → 回传前必须打回掩码。
        let stored = config_with_search_key("sk-plaintext-123456");
        let mut masked = stored.clone();
        mask_search_engine_keys(&mut masked);
        assert!(masked.search_engines[0]
            .api_key
            .as_deref()
            .unwrap_or_default()
            .contains("***"));

        let mut incoming = masked.clone();
        merge_masked_search_engine_keys(&stored, &mut incoming);
        assert_eq!(
            incoming.search_engines[0].api_key.as_deref(),
            Some("sk-plaintext-123456"),
            "合并这一步本来就该拿到明文（要落盘）"
        );

        let outbound = config_for_transport(incoming);
        let key = outbound.search_engines[0].api_key.clone().unwrap_or_default();
        assert!(key.contains("***"), "回传/广播的配置必须打码：{key}");
    }

    // ========== Web 端参数形状 ==========

    #[test]
    fn test_search_engine_params_should_accept_input_wrapper() {
        // 前端走 invokeTauri("test_search_engine", { input: {...} })，Web 侧收到的是原样 params。
        let params = serde_json::json!({"input": {"engineId": "tavily", "apiKey": "k-1"}});
        let input = ide_chat_parse_param_field::<TestSearchEngineInput>(params, "input")
            .expect("带 input 包裹的参数应能解析");
        assert_eq!(input.engine_id, "tavily");
        assert_eq!(input.api_key.as_deref(), Some("k-1"));
    }

    #[test]
    fn test_search_engine_params_should_accept_bare_fields() {
        let params = serde_json::json!({"engineId": "exa"});
        let input = ide_chat_parse_param_field::<TestSearchEngineInput>(params, "input")
            .expect("裸字段参数也应能解析");
        assert_eq!(input.engine_id, "exa");
        assert_eq!(input.api_key, None);
    }

    // ========== 换源与相关性兜底（行为用例，不碰网络） ==========

    fn candidate(engine: SearchEngine, api_key: Option<&str>) -> ReadySearchEngine {
        ReadySearchEngine {
            engine,
            api_key: api_key.map(str::to_string),
        }
    }

    fn relevant_row() -> Value {
        serde_json::json!({
            "title": "原神纳西妲技能详解",
            "url": "https://wiki.example/nahida",
            "snippet": "纳西妲是原神里的草系角色",
        })
    }

    fn english_row() -> Value {
        serde_json::json!({
            "title": "Booking hotels",
            "url": "https://hotel.example/1",
            "snippet": "reserve a room tonight",
        })
    }

    #[tokio::test]
    async fn try_candidates_should_move_on_after_auth_failure() {
        // 一个填错的旧 key 不该让免密钥引擎一起失效。
        let candidates = vec![
            candidate(SearchEngine::Exa, Some("stale-key")),
            candidate(SearchEngine::Tavily, None),
        ];
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<SearchEngine>::new()));
        let order_in_call = order.clone();
        let attempt = try_candidates_with(
            "原神 纳西妲",
            "session-auth-switch",
            &candidates,
            vec![SearchEngine::Exa, SearchEngine::Tavily],
            move |engine, _api_key| {
                let order = order_in_call.clone();
                async move {
                    order.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(engine);
                    match engine {
                        SearchEngine::Exa => {
                            Err(SearchCallError::Auth("401 Unauthorized".to_string()))
                        }
                        _ => Ok(vec![relevant_row()]),
                    }
                }
            },
        )
        .await;
        assert_eq!(
            *order.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![SearchEngine::Exa, SearchEngine::Tavily],
            "鉴权失败后要继续试下一个源"
        );
        match attempt {
            SearchAttempt::Hit(payload) => {
                assert_eq!(payload["engine"]["id"], "tavily", "换源成功要交回第二个源的结果")
            }
            _ => panic!("鉴权失败后应该换源成功"),
        }
    }

    #[tokio::test]
    async fn try_candidates_should_keep_auth_reason_when_every_source_rejects() {
        let candidates = vec![
            candidate(SearchEngine::Exa, Some("bad-1")),
            candidate(SearchEngine::Tavily, None),
        ];
        let attempt = try_candidates_with(
            "原神 纳西妲",
            "session-all-auth",
            &candidates,
            vec![SearchEngine::Exa, SearchEngine::Tavily],
            |engine, _api_key| async move {
                Err(SearchCallError::Auth(format!("{} 401", engine.id())))
            },
        )
        .await;
        match attempt {
            SearchAttempt::Missed { failures, weak } => {
                assert!(weak.is_none(), "没有原始结果可交回");
                assert_eq!(failures.len(), 2);
                assert!(
                    failures.iter().all(|item| item.contains("鉴权失败，请检查密钥")),
                    "配置问题不能被换源掩盖：{failures:?}"
                );
                let message = search_failure_message(&failures);
                assert!(message.contains("鉴权失败，请检查密钥"));
                assert!(!message.contains("兜底"));
            }
            _ => panic!("两源都鉴权失败时不该有命中"),
        }
    }

    #[tokio::test]
    async fn try_candidates_should_hand_back_raw_rows_when_nothing_is_relevant() {
        // 纯中文查询 + 引擎只回英文页：不算引擎失败，原始结果要留一份。
        let candidates = vec![candidate(SearchEngine::Exa, None)];
        let attempt = try_candidates_with(
            "原神 纳西妲",
            "session-weak",
            &candidates,
            vec![SearchEngine::Exa],
            |_engine, _api_key| async move { Ok(vec![english_row()]) },
        )
        .await;
        match attempt {
            SearchAttempt::Missed { failures, weak } => {
                assert!(failures.is_empty(), "全被滤掉不算失败：{failures:?}");
                let (engine, rows) = weak.expect("必须留一份原始结果兜住");
                assert_eq!(engine, SearchEngine::Exa);
                assert_eq!(rows.len(), 1);
                let payload = search_payload_with_notice(
                    engine.id(),
                    engine.display_name(),
                    rows,
                    WEAK_RELEVANCE_NOTICE,
                );
                assert_eq!(payload["notice"], WEAK_RELEVANCE_NOTICE);
                assert_eq!(
                    payload["results"].as_array().map(|items| items.len()),
                    Some(1),
                    "宁可给可能不够准的结果，也不能说搜不到"
                );
            }
            _ => panic!("没有沾边结果时要走 Missed + weak 分支"),
        }
    }

    // ========== P1：明文密钥不许从任何一条传输路径出去 ==========

    #[test]
    fn outbound_config_should_only_flow_through_masked_transport_helper() {
        // 打码必须收在唯一出口内部：调用点一律拿已打码的返回值，不许在个别分支上漏。
        let window_and_update = include_str!(
            "../../../system/commands/config_and_persona/window_and_update.rs"
        );
        let persona_settings = include_str!(
            "../../../system/commands/config_and_persona/persona_and_chat_settings.rs"
        );
        for source in [window_and_update, persona_settings] {
            assert!(
                !source.contains("runtime_config_with_private_organization"),
                "对外配置只允许走 runtime_config_for_transport"
            );
            assert!(
                !source.contains("config_for_transport(runtime_config_for_transport"),
                "调用点不许在出口外面再套一层打码"
            );
        }
        let definition_start = window_and_update
            .find("fn runtime_config_for_transport(")
            .expect("传输出口必须存在");
        let definition = &window_and_update[definition_start..];
        let body_end = definition.find("\n}\n").expect("函数体应闭合");
        assert!(
            definition[..body_end].contains("config_for_transport("),
            "打码要放在出口内部，新增的写入类命令才不会漏"
        );
    }
}
