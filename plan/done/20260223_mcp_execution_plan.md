# MCP 页面与动态工具接入执行计划

## 1. 目标与边界
1. 新增独立 `MCP` 配置页面（不混在现有 Tools Tab）。
2. 支持 MCP Server 卡片管理：新增、编辑、删除、部署开关。
3. 卡片内支持 JSON 定义输入框。
4. 可拉取并展示该 MCP 暴露的工具列表（名称+描述）。
5. 每个工具单独开关，可持久化。
6. 暂不做额外安全拦截与白名单（按当前需求）。
### Security Baseline
1. 基础输入校验：对 `definition_json` 做 JSON 解析、字段类型校验、长度上限校验（默认拒绝 >1MB），并对 `command/args/url/env` 做最小必要的字符与结构合法性检查，避免注入拼接。
2. 资源访问边界：仅允许配置中声明的 MCP server 被连接；stdio 仅执行用户显式配置命令且按最小权限运行，不自动提升权限；HTTP 仅访问配置 URL，不做额外隐式跳转能力。
3. 错误与日志过滤：错误信息输出前移除或掩码密钥字段（`api_key`、`Authorization`、`bearer token`、`env_http_headers` 值），日志中禁止打印完整敏感头和值。
4. 安全增强路线图：后续分阶段引入命令/域名白名单、凭据托管、审计日志与策略开关；上线门槛为“注入与敏感信息泄露测试全通过 + 权限策略回归测试通过”。

## 2. 数据模型设计
1. 新增 `McpServerConfig`：
   - `id` `name` `enabled`
   - `transport`（`stdio` / `streamable_http`）
   - `definition_json`（原始用户 JSON）
   - `last_status` `last_error` `updated_at`
2. 新增 `McpToolPolicy`：
   - `server_id` `tool_name` `enabled`
   - `description_cache`（可选）
3. 配置存储新增 `mcp_servers` 与 `mcp_tool_policies`。
4. 保持与现有 `api.tools` 并存，MCP 独立管理。

## 3. 统一 JSON 规范（应用内部标准）
1. 顶层支持 `mcpServers` 字典。
2. 每个 server 统一字段：
   - `version`（当前固定 `1.0`）
   - `transport`
   - `command/args/env/cwd`（stdio）
   - `url/bearerTokenEnvVar/httpHeaders/envHttpHeaders`（HTTP）
   - `enabledTools/disabledTools`（可选）
3. 后端做解析与标准化，前端保留原始 JSON 便于迁移外部格式。
4. 定义并执行 JSON Schema 校验：
   - 在后端维护 `mcpServers` 规范的 JSON Schema（含 `version` 必填与兼容规则）。
   - `mcp_validate_definition` 运行时执行 schema 校验并返回结构化结果（`error_code/details`）。
   - 版本策略：`1.0` 直接通过；缺失版本或 `0.x` 触发受控迁移并返回 `migrated_definition_json`；其他版本返回 `unsupported_version` 并附升级建议。

## 4. 后端能力拆分
1. 新增命令：
   - `mcp_validate_definition`
   - `mcp_save_server`
   - `mcp_remove_server`
   - `mcp_deploy_server`
   - `mcp_undeploy_server`
   - `mcp_list_server_tools`
   - `mcp_set_tool_enabled`
   - `mcp_list_servers`
2. 新增 `McpRuntimeManager`：
   - 负责启动/停止连接
   - 缓存 server 连接状态
   - 维护工具清单与可用性
3. 聊天工具装配阶段合并：
   - 内置工具 + 启用的 MCP 工具（按 server/tool 开关过滤）。
4. 扩展 `McpRuntimeManager` 运行控制：
   - 连接超时与工具调用超时可配置（构造参数注入，并在连接/调用路径生效）。
   - 连接重试采用 `retryWithBackoff`（`maxRetries` + backoff 策略）并用于 `start/establishConnection`。
   - 引入连接池与并发上限（按 server 维度跟踪 borrow/return）。
   - 优雅停机：`shutdownGracefully/stop` 处理 in-flight 调用，支持 drain timeout。
   - 周期健康检查：`healthCheckLoop` 标记不健康并触发重连/重试。
   - 状态流转需原子更新，避免并发覆盖（部署、失败、回滚状态一致性）。

## 5. 前端页面实现
1. 配置左侧菜单新增 `MCP` 入口。
2. 页面顶部：
   - 新增按钮
   - 全局刷新按钮
3. MCP 卡片内容：
   - 名称、传输协议、部署开关
   - JSON 输入框（编辑态）
   - 校验/保存按钮
   - 工具列表与每工具开关
   - 状态与错误提示
4. 交互规则：
   - “可获取工具列表”即允许保存
   - 工具开关改动自动保存或轻量显式保存（建议自动）。

## 6. 联调与测试
1. 单元测试（后端）：
   - JSON 解析与标准化
   - stdio/http 配置转换
   - 工具开关持久化
2. 集成测试（后端）：
   - mock MCP server `tools/list` 成功/失败
   - 部署失败恢复状态
3. 前端测试：
   - 卡片增删改
   - 部署开关状态流转
   - 工具开关回显
4. 手工验收：
   - 接入 1 个 stdio MCP
   - 接入 1 个 streamable HTTP MCP
   - 聊天中能被模型调用并看到 tool trace。
5. 边界测试：
   - 用例：超大 JSON（>1MB）输入。
   - 预期行为：校验失败，返回明确 `error_code` 与体积超限说明，不触发部署。
   - 判定标准：无崩溃、无卡死、错误可读可定位。
   - 最小复现：粘贴 >1MB JSON 到定义框，点击校验。
   - 用例：畸形/异常字符 JSON（控制字符、截断、乱码）。
   - 预期行为：解析失败并返回结构化错误（含定位信息）。
   - 判定标准：失败可复现、不会污染已保存配置。
   - 最小复现：输入非法 JSON，点击校验。
   - 用例：网络超时/连接中断。
   - 预期行为：部署失败并可重试，状态从 deploying 正确转为 failed。
   - 判定标准：状态字段一致、前端可见失败原因。
   - 最小复现：配置不可达 URL，点击部署。
   - 用例：MCP 返回异常/不完整数据。
   - 预期行为：工具列表拉取失败但主流程不中断，返回可操作错误。
   - 判定标准：应用不崩溃，后续可继续操作其他 server。
   - 最小复现：对接返回异常 JSON 的 mock MCP。
6. 性能基线：
   - 指标：并发接入多个 MCP（例如 10/20 并发连接）成功率与平均建立时延。
   - 指标：工具列表刷新响应时间（P50/P95）。
   - 指标：部署/回滚吞吐量（单位时间可完成次数）。
   - 预期行为：在基线负载下响应稳定，无明显 UI 阻塞。
   - 判定标准：达到团队设定阈值并形成可追踪基线记录。
   - 最小复现：批量创建 server，循环部署/停止并记录耗时。
7. 安全测试：
   - 用例：输入注入（命令注入、路径遍历）。
   - 预期行为：输入被拒绝或被安全处理，不执行非预期命令。
   - 判定标准：无越权执行、无路径逃逸。
   - 最小复现：构造含注入片段的 command/args/cwd 并校验部署。
   - 用例：敏感信息泄露（日志/错误暴露密钥）。
   - 预期行为：日志与错误输出仅显示掩码，不输出原文密钥。
   - 判定标准：检索日志无明文凭据。
   - 最小复现：配置带 token 的 header，触发失败并检查日志。
   - 用例：权限/认证错误处理。
   - 预期行为：返回明确认证失败原因，不影响其他 MCP。
   - 判定标准：单点失败隔离、状态一致。
   - 最小复现：配置错误 token 并部署，验证错误与隔离性。

## 7. 实施顺序（建议）
1. 先落地数据结构与后端命令壳子。
2. 再做 `McpRuntimeManager` + `tools/list`。
3. 然后做 MCP 页面与卡片交互。
4. 最后接聊天工具装配与端到端测试。

## 8. 交付物
1. 新的 MCP 配置页。
2. 可部署 MCP server 并显示工具。
3. 工具级开关生效到聊天调用链。
4. 文档：`docs/mcp_配置与接入说明.md`（含 JSON 示例）。
