# Parallel Search MCP / Parallel 网页搜索

[Parallel Search MCP](https://docs.parallel.ai/integrations/mcp/search-mcp) provides free web search (`web_search`) and page fetching (`web_fetch`) without an account or API key. Anonymous search uses Fast mode and is rate limited.

## Setup

1. Install PAI from [Releases](https://github.com/kawayiYokami/P-ai/releases), or use your existing PAI installation. No Node.js package is needed for this HTTP server.
2. Open Settings → MCP and add a new server. Paste the entire contents of [parallel-search.json](parallel-search.json) into the configuration JSON field, replacing the new card's sample definition.
3. Validate the definition, save it, then deploy the server. Tool discovery should list `parallel_web_search` and `parallel_web_fetch`. Keep the tools you need enabled.
4. To stop using it, stop this server. Other MCP servers and model providers are unaffected.

Use a new card for anonymous access. The example contains no authentication headers or environment variables; do not add a Bearer token or sign in with OAuth for the free path. If reusing a previously authenticated card, clear its OAuth credentials first.

Search accepts `objective` and a nonempty `search_queries` array. Fetch accepts a `urls` array and an `objective` to focus extracted excerpts. When supplying `session_id`, generate one UUID per conversation and reuse it across related search and fetch calls. See the linked MCP documentation for current schemas and limits. If the service returns a rate limit, wait as directed by its response before retrying.

## 配置步骤

Parallel Search MCP 无需账号或 API Key 即可免费搜索网页和提取页面内容，匿名搜索使用 Fast 模式，有速率限制。

1. 安装或打开 PAI，在「设置 → MCP」中新建服务卡片。
2. 将 [parallel-search.json](parallel-search.json) 的完整内容粘贴到配置 JSON 中，替换新卡片的示例定义。HTTP 服务不需要安装 Node.js 包。
3. 校验、保存并部署服务；工具列表应出现 `parallel_web_search` 和 `parallel_web_fetch`，按需启用。
4. 停止该服务即可停用，不影响其他 MCP 服务或模型供应商。

匿名访问请使用新卡片，不添加鉴权头、不登录 OAuth；复用已授权的卡片时，先清除其 OAuth 凭据。搜索参数包含 `objective` 和非空的 `search_queries` 数组，提取参数包含 `urls` 数组和用于聚焦内容的 `objective`。如果传入 `session_id`，每个会话生成一个 UUID，并在相关搜索和提取调用中复用。最新参数与限制请参阅上方文档；遇到限流时按响应要求等待后再重试。
