# MCP 实施归档（本轮）

日期：2026-02-22

## 已完成
1. 新增 MCP 后端模块（types/parser/runtime_manager/commands）并注册 tauri 命令。
2. AppConfig 接入 mcp_servers 持久化，含归一化逻辑。
3. 新增 MCP 配置页面与卡片化组件（McpTab/McpServerCard/McpToolList）。
4. MCP 工具接入聊天运行时（内置工具 + MCP 工具合并，失败降级）。
5. mcp_validate_definition 增强：
   - 版本字段校验（version=1.0）
   - 结构化 schema 校验结果
   - 受控迁移（缺失版本/0.x 自动迁移）
6. 修复 mcp_deploy_server 的 TOCTOU 覆盖风险（仅更新状态相关字段）。
7. 修复前端 toggleDeploy 与 saveServer 的 loading 竞态。
8. 进入 MCP 页时，已启用 server 自动回填工具列表。
9. Windows stdio 启动兼容增强：默认命令自动走 cmd /C 兼容 npx/cmd。

## 验证
1. cargo check 通过。
2. pnpm tsc --noEmit 通过。

## 备注
1. 本次提交不包含与 MCP 无关的 UI 微调与 tantivy_probe 格式化改动。
