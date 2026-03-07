# LLM Workspace MCP/SKILL 统一架构实现归档

文档日期：2026-02-24  
用途：记录“当前已上线实现”的真实架构与行为约束，供后续维护/重构/排障直接使用。

## 1. 当前架构总览

### 1.1 MCP
MCP 采用“定义 / 策略 / 运行态”三层分离：
1. 定义层（持久化）：`llm-workspace/mcp/servers/*.json`
2. 策略层（持久化）：`llm-workspace/mcp/policies/*.json`
3. 运行态（内存）：部署状态、错误信息、运行期工具列表

### 1.2 SKILL
SKILL 采用“目录扫描 + 对话注入”：
1. Skill 源：`llm-workspace/skills/*/SKILL.md`
2. 每轮新对话：后端重扫一次技能摘要并隐藏注入
3. 配置页 Skill Tab：前端缓存，首次加载或手动刷新才重扫

## 2. 目录与文件契约

### 2.1 MCP 目录
1. `llm-workspace/mcp/servers/<serverId>.json`
   - 仅保存 MCP 定义（推荐单层命名对象）
   - 不保存部署状态/错误/缓存工具
2. `llm-workspace/mcp/policies/<serverId>.json`
   - 保存服务总开关 + 工具开关
   - 示例：
```json
{
  "serverId": "context7",
  "enabled": true,
  "tools": [
    { "toolName": "resolve-library-id", "enabled": true }
  ]
}
```

### 2.2 SKILL 目录
1. `llm-workspace/skills/<skill-name>/SKILL.md`
2. 需包含 frontmatter（至少 `name` / `description`）

## 3. 核心行为约束（必须遵守）

### 3.1 MCP 部署与策略
1. “部署”会把 `policies.enabled` 置为 `true`
2. “停止”会把 `policies.enabled` 置为 `false`
3. 部署成功后，按“增量合并”更新工具开关：
   - 仅补新增工具
   - 已有工具开关不覆盖
4. 工具最终启用判定：
   - `policy.tools[tool].enabled`
   - AND definition 过滤：当同时存在 `definition.enabledTools` 与 `definition.disabledTools` 时，`definition.enabledTools` 作为白名单优先（忽略 `definition.disabledTools`）；若仅存在 `definition.disabledTools`，则其作为黑名单生效

### 3.2 MCP 运行态
1. 部署状态仅在内存：
   - `deploying/deployed/failed/stopped`
2. `lastError`、运行期工具描述列表仅在内存
3. 应用重启后运行态清空（符合设计）

### 3.3 刷新工具语义
`refresh_mcp_and_skills`（内置工具）当前语义：
1. 重扫工作区 MCP/SKILL 文件
2. 停止所有 MCP
3. 按 `policies.enabled == true` 全量重部署 MCP
4. 返回成功/失败列表与 skill 摘要

注意：这是一种“运维级刷新”，不是轻量 UI 刷新。

### 3.4 Skill Tab 语义
1. 不触发 MCP 重部署
2. 使用 `mcp_list_skills` 获取技能列表
3. 前端缓存命中时直接展示，减少重复扫描与 loading

## 4. 关键代码入口（维护索引）

### 4.1 MCP
1. `src-tauri/src/features/mcp/workspace.rs`
   - MCP servers/policies 读写
   - policy 增量合并逻辑
2. `src-tauri/src/features/mcp/commands.rs`
   - deploy/undeploy/list/set-tool-enabled/refresh辅助逻辑
3. `src-tauri/src/features/mcp/runtime_manager.rs`
   - MCP client cache
   - MCP runtime state store（内存）
4. `src-tauri/src/features/mcp/README.workspace.md`
   - 工作区 README 模板

### 4.2 SKILL
1. `src-tauri/src/features/skill/workspace.rs`
   - SKILL 扫描与摘要
2. `src-tauri/src/features/skill/commands.rs`
   - `mcp_list_skills`
   - `mcp_refresh_mcp_and_skills`
3. `src/features/config/views/config-tabs/SkillTab.vue`
   - Skill 页缓存与手动刷新行为

## 5. 已解决问题（对应本次重构）

1. MCP 与 SKILL 模块拆分，不再混杂在同一实现块
2. MCP 定义文件不再混入运行态脏字段
3. 工具开关持久化且可跨重部署保持
4. Skill 页不再误触发 MCP 全量重部署
5. 打开目录为系统文件管理器行为（非加载目录内容）

## 6. 已知限制（后续可优化）

1. MCP 运行态内存化后，重启应用不会保留“已部署”状态
2. Skill 页缓存为页面级缓存，未做文件变更监听
3. `refresh_mcp_and_skills` 语义偏重，适合 LLM 运维，不适合高频 UI 轻刷新

## 7. 后续改动建议

1. 若要“自动感知 skill 文件变更”，建议加文件系统 watcher，而不是切页重扫
2. 若要“MCP 启动恢复”，可在启动时读取 `policies.enabled` 自动部署
3. 若要“轻量 MCP 刷新”，新增单独命令，仅刷新内存视图，不触发重连

## 8. 回归检查清单（修改此架构前后必跑）

1. MCP：
   - 新增 server 定义后可部署
   - 关闭某工具后重部署仍保持关闭
   - `refresh_mcp_and_skills` 后按 policy 全量重部署
2. SKILL：
   - 新增 skill 目录后手动刷新可见
   - 调用 `refresh_mcp_and_skills` 后校验返回的 skill 摘要列表：数量正确，且每项关键字段（如 `name`、`description`、`path`）与预期一致；若缺失或字段不符则判定失败
   - 切页不触发 MCP 部署
3. 构建检查：
   - `cargo check`（`src-tauri`）
   - `pnpm typecheck`（项目根目录）
