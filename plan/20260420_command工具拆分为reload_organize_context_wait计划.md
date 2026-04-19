# 20260420 command 工具拆分为 reload / organize_context / wait 计划

## 背景

当前内置 `command` 工具属于“统一命令入口”，通过一个字符串参数复用多个系统动作：

- `help`
- `reload`
- `organize_context`
- `wait <ms>`

这会带来几个问题：

1. 工具语义混杂，LLM 需要先理解二级命令协议，再决定具体动作。
2. `help` 只是给工具本身做说明，价值低，反而增加模型心智负担。
3. `reload / organize_context / wait` 已经是三个独立能力，继续塞在 `command.command` 里不利于提示词、目录、日志和测试保持一致。

本次目标是删除 `help`，并把 `command` 拆成三个独立工具：

- `reload`
- `organize_context`
- `wait`

## 目标

1. 删除 `command` 工具，不再对运行时、工具目录、提示词和前端展示暴露。
2. 删除 `help` 子命令，不保留兼容入口。
3. 将 `reload / organize_context / wait` 改为三个一等内置工具。
4. 保持原有三个能力的实际行为不变，只改工具协议与装配方式。
5. 清理所有旧的 `command + organize_context` 兼容判断、文案和测试。

## 不做

1. 不改变 `reload` 的刷新逻辑和提示词缓存标脏语义。
2. 不改变 `organize_context` 的压缩策略和触发条件。
3. 不改变 `wait` 的超时范围与执行实现。
4. 不顺带重做其他工具类型分层。

## 影响范围

### Rust 后端

- `src-tauri/src/features/chat/model_runtime/tools_and_builtin/tool_impls.rs`
  - 删除 `BuiltinCommandTool`
  - 新增 `BuiltinReloadTool`、`BuiltinOrganizeContextTool`、`BuiltinWaitTool`
- `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`
  - 移除 `command`
  - 直接装配 `reload / organize_context / wait`
- `src-tauri/src/features/chat/conversation.rs`
  - 更新工具规则文案
- `src-tauri/src/features/system/commands/chat_and_runtime/tool_catalog.rs`
  - 工具目录改为展示三个独立工具
- `src-tauri/src/features/system/commands/chat_and_runtime/tools_and_cache.rs`
  - 工具状态改为独立三项
- `src-tauri/src/features/config/storage_and_stt.rs`
  - 配置归一化与旧工具名兼容逻辑要同步收口

### 前端

- `src/features/chat/views/ChatView.vue`
  - 去掉 `command` 调用 `organize_context` 的特殊识别
- 若工具目录或只读工具页存在 `command` 说明，也一并切换到三个独立工具

## 实施方案

### 1. 删除 command 统一入口

- 删除 `BuiltinCommandTool` 定义与 provider metadata
- 删除 `command_help_text()`
- 删除 `help` 子命令，不保留兼容

### 2. 新增三个独立工具

#### `reload`

- 无参数
- 直接复用当前 `builtin_reload(app_state)` 逻辑
- 工具描述直接说明“重载工作区 MCP 与技能”

#### `organize_context`

- 无参数
- 直接复用当前 `builtin_organize_context(app_state, api_config_id, agent_id)` 逻辑
- 保持当前返回结构

#### `wait`

- 参数：
  - `ms: integer`
- 直接复用当前 `builtin_desktop_wait(ms)` 逻辑
- 保持当前 1~120000 的范围限制

### 3. 更新工具装配与展示

- 运行时装配时不再挂 `command`
- 工具目录直接展示：
  - `reload`
  - `organize_context`
  - `wait`
- 状态页不再出现 `command`

### 4. 更新提示词与前端兼容

- 工具规则中不再出现 `command`
- 如果有“统一命令工具”相关文案，改为独立能力说明
- 前端流式工具调用中，`organize_context` 只识别真正的 `organize_context`，不再兼容 `command`

## Todo

1. 删除 `BuiltinCommandTool` 并补上 `BuiltinReloadTool / BuiltinOrganizeContextTool / BuiltinWaitTool`
2. 更新运行时工具装配、工具目录和状态查询，彻底移除 `command`
3. 更新提示词文案、前端识别逻辑和旧兼容分支
4. 补测试并运行 `pnpm typecheck`、`cargo check`、相关定向测试

## 验收标准

1. LLM 侧不再看到 `command` 工具。
2. LLM 侧可以直接调用：
   - `reload`
   - `organize_context`
   - `wait`
3. `help` 不再存在，也没有任何旧文案残留。
4. 工具目录、状态查询、聊天流式展示都不再依赖 `command`。
5. `pnpm typecheck` 与 `cargo check` 通过，相关测试通过。
