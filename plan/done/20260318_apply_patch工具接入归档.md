# apply_patch 工具接入归档

## 归档时间

2026-03-19

## 目标

在现有内置工具体系中新增 `apply_patch`，支持通过结构化补丁对工作区文件进行增删改与重命名，并保证路径边界与审批行为可控。

## 需求达成对照

1. 工具形态
   - 已完成：新增独立内置工具 `apply_patch`。
   - 已完成：与 `exec` 分离，文件编辑不再依赖 shell 命令。

2. 补丁语法
   - 已完成：支持 Codex 风格补丁语法。
   - 已覆盖：`*** Begin Patch` / `*** End Patch`。
   - 已覆盖：`*** Add File:` / `*** Delete File:` / `*** Update File:`。
   - 已覆盖：`*** Move to:`。
   - 已覆盖：`@@` hunk 与 `+/-/ ` 行前缀。

3. 能力要求
   - 已完成：支持文件新增、删除、更新、重命名。
   - 已完成：返回结构化执行结果（成功状态、变更项、错误信息、耗时等）。

4. 路径与边界安全
   - 已完成：仅允许在当前会话工作区范围内操作。
   - 已完成：路径越界（含 `../` 逃逸、越界重命名）会拒绝执行。

5. 安全判定规则（三态）
   - 已完成：保留 `AutoApprove / AskUser / Reject` 三态判定流程。
   - 已完成：LLM 默认工作区免审批（自动通过）。
   - 已完成：用户选择的工具工作区执行补丁会进入三态判定。

6. 配置与运行时接入
   - 已完成：进入默认工具配置列表。
   - 已完成：接入 runtime tool assembly 与工具状态检查链路。
   - 已完成：遵守部门工具可用性限制。
   - 已调整：根据确认结果，`exec` 与 `apply_patch` 默认状态改为开启。

7. 日志与可观测性
   - 已完成：新增中文日志输出。
   - 已覆盖：输出任务名、会话、变更数量、耗时、失败原因。

8. 验收标准
   - 已通过：模型可调用 `apply_patch` 完成增删改移。
   - 已通过：默认工作区修改无需审批。
   - 已通过：用户工具区修改按三态规则运行。
   - 已通过：越界路径拒绝且返回清晰原因。
   - 已通过：工具在配置页可控，状态检查正常。

## 实际落地文件

- `src-tauri/src/features/system/tools/patch.rs`
- `src-tauri/src/features/system/tools.rs`
- `src-tauri/src/features/chat/model_runtime/tools_and_builtin.rs`
- `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`
- `src-tauri/src/features/system/commands/chat_and_runtime/tools_and_cache.rs`
- `src-tauri/src/features/core/domain.rs`
- `src/features/config/utils/builtin-tools.ts`
- `src/features/config/views/config-tabs/ToolsTab.vue`
- `src-tauri/src/features/system/tools/terminal/workspace.rs`

## 验证记录

- `cargo check` 通过。
- `cargo test apply_patch_tool_tests` 通过（3/3）。
- 实际对话验证：绝对路径、越界路径被拒绝；删除、批量补丁、上下文不匹配拒绝等行为符合预期。
