---
name: mcp-setup
description: 当需要自行安装和配置 MCP，补齐当前工作区的 MCP 能力时，必须立刻阅读我。我会约束目录位置、资料来源和安装配置流程。
---

# MCP Setup

## 规则
- `<workspace>` 只是占位符，不是固定目录名；它表示你当前 shell 的启动工作空间。
- 你只能在这个工作空间内工作；不要假设或访问工作空间外路径。
- 自己查官方文档再安装和配置，不要空想连接定义。
- MCP 放在 `<workspace>/mcp/`。
- `servers/` 放连接定义。
- `policies/` 放工具开关。
- 完成后做一次最小验证。
- 缺依赖、缺权限、缺密钥时，直接说明阻塞点。

## 目录结构
```text
<workspace>/mcp/
  servers/
    <serverId>.json
  policies/
    <serverId>.json
```

## servers 格式
- `servers/<serverId>.json` 只放单个 server 的裸定义。
- `<serverId>` 来自文件名，不需要在 JSON 里再包一层同名 key。

最简示例：
```json
{
  "transport": "stdio",
  "command": "npx",
  "args": ["-y", "@upstash/context7-mcp"]
}
```

支持的常见接入方式：
- `stdio`
- `streamable_http`
- `stdio + mcp-remote`

## policies 格式
- 成功部署后，系统会为同名 server 自动创建 `policies/<serverId>.json`。

示例：
```json
{
  "serverId": "context7",
  "enabled": true,
  "tools": [
    { "toolName": "resolve-library-id", "enabled": true },
    { "toolName": "get-library-docs", "enabled": true }
  ]
}
```

## 开关规则
- `enabled` 是服务级总开关，控制该 MCP 是否参与全量重部署。
- `tools[].enabled` 是工具级开关，控制单个工具是否启用。
- 重部署时只补新增工具，不覆盖已有工具开关。
- 用户手动关闭某个工具后，重部署仍保持关闭。

## 推荐 MCP
- `deepwiki`：建议优先部署。适合仓库文档、代码结构、模块关系问答。
- `context7`：适合官方库文档、API 用法、版本差异查询。
- `tavily`：适合联网搜索、新闻检索、网页提取。通常需要用户自己的 API key，没有就先指导注册。

## 说明
- 刷新会重新加载并部署已启用的 MCP。
- 想拿到最新工具名，先部署一次。
- 完成安装后，至少做一次最小验证。

## 输出
- 说明安装了哪些 MCP。
- 说明采用了什么安装方式。
- 说明验证结果。
- 没做的项要写明原因。
