# 20260405 rust-genai全量迁移计划

## 1. 说明

本计划用于把当前后端模型接入层从 `rig-core` 全量迁移到 `rust-genai`。

这里的“全量迁移”不是只替换 OpenAI Responses，也不是只修 Codex 的 `instructions` 问题，而是彻底移除 `rig` 在当前项目中的运行时职责，统一改为由 `rust-genai` 承担多 provider 聊天调用。

本计划已获得用户确认，并已进入实现阶段。

## 1.1 当前执行进度

截至 `2026-04-05`，已完成以下改造：

1. 已建立项目内第一批运行时抽象，开始把工具定义从 `rig` 类型中拆出。
2. 前端工具目录已不再依赖 `rig::tool::Tool::definition(...)`，改为读取项目自有 tool schema。
3. 已引入 `rust-genai` 依赖。
4. OpenAI Chat Completions 的无工具主链已切到 `rust-genai`。
5. OpenAI Chat Completions 的工具循环主链已切到 `rust-genai`。
6. OpenAI Responses 的无工具主链已切到 `rust-genai`，不再经过 `rig` 的错误 Responses 实现。
7. OpenAI Responses 的工具循环主链也已切到 `rust-genai`，provider 侧不再依赖 `rig agent + tool_server_handle`。
8. Gemini 的无工具主链已切到 `rust-genai`。
9. Gemini 的工具循环主链已切到项目自有 `genai` tool loop，不再依赖 `rig agent + stream_completion`。
10. Anthropic 的无工具主链已切到 `rust-genai`。
11. Anthropic 的工具循环主链已切到项目自有 `genai` tool loop，不再依赖 `rig agent + stream_completion`。
12. 已增加回归测试，固定 OpenAI Responses / Gemini / Anthropic 的“system prompt 保持在 top-level system/instructions 语义”约束。
13. 运行时工具抽象已从 `rig::ToolDyn` 切到项目自有 `RuntimeToolDyn`。
14. 内建 MCP 工具与工作区 MCP 工具已不再通过 `rig::tool::rmcp::McpTool` 挂接，而是改为项目自有 `rmcp` wrapper。
15. MCP rich result 已开始保留图片 / 音频 / 资源分块，并在 tool loop 中按模型可消费的后续消息继续转发。
16. `main.rs` 中的 `rig` 全局导入已清理完成，当前源码运行链路已无 `rig::` 直接引用。
17. 旧 `rig` tool loop / message replay / prompt build / stream collect 死代码已删除，活跃调用命名也已从 `*_rig_style` 收敛为中性命名。
18. 已重新通过 `cargo check` 与 `cargo test keep_system_at_top_level`，OpenAI Responses / Gemini / Anthropic 的 top-level system 行为继续锁定。
19. 已新增项目自有 Gemini payload builder / stream parser，不再依赖 `rust-genai` 内部未开放的 Gemini 发包细节。
20. Gemini 无工具流与 Gemini 工具循环续调都已恢复 `safetySettings = BLOCK_NONE`。
21. 已新增 Gemini `BLOCK_NONE` payload 与 schema 转换回归测试，并重新通过 `cargo check`。
22. 已新增运行时迁移护栏测试，扫描 `src-tauri/src`，防止 `rig::` / `tool_server_handle` / `stream_completion(` 等旧运行时标记重新回流进源码。

尚未完成的重点：

1. 调试日志中的“构造请求体”和各 provider 最终真实发包之间仍可继续收紧一致性。

## 2. 背景

当前项目已经把真正复杂的运行时能力掌握在自己手里：

1. Prompt 组装与上下文裁剪由项目自身负责。
2. 工具执行、工具循环、中断、轮次控制由项目自身负责。
3. 会话持久化、调试日志、前端流式增量更新由项目自身负责。
4. Provider 路由与 fallback 策略也已由项目自身承担。

因此，`rig` 在本项目中实际承担的主要职责已经收缩为：

1. Provider 客户端封装。
2. 消息类型转换。
3. 流式输出事件接头。
4. 工具 schema 挂接。
5. MCP 工具到模型工具协议的桥接。

当前最突出的问题是 OpenAI Responses 语义被 `rig` 内部实现错误劫持：

1. 官方 Responses API 定义了顶层 `instructions`。
2. 当前 `rig-core 0.33.0` 在 OpenAI Responses 路径中把 preamble 转成了 input `system message`。
3. 同时把顶层 `instructions` 固定为 `None`。
4. 这导致官方 OpenAI / Codex 路径反而无法正常使用官方协议字段。

这说明：

1. `rig` 在本项目里已经不是“减少复杂度”的关键组件。
2. 它反而在底层协议语义上引入了不可控行为。
3. 对当前项目这种产品级、多 provider、强控制需求的架构，继续保留 `rig` 的收益已经不足以覆盖风险。

## 3. 目标

### 3.1 功能目标

1. 完整移除 `rig-core` 作为聊天模型接入层。
2. OpenAI Chat Completions、OpenAI Responses、Gemini、Anthropic 全部切换到 `rust-genai`。
3. 保持现有前端聊天体验、工具循环体验、调试体验不回退。
4. OpenAI Responses 恢复官方语义，顶层 `instructions` 可正常传递。
5. 现有内建工具、MCP 工具、前端工具目录继续可用。

### 3.2 架构目标

1. 在项目内建立自己的 provider-neutral 调用边界，不再把第三方 SDK 类型直接渗透进业务逻辑。
2. 将“消息构造”“流式事件消费”“工具 schema 生成”“tool call 回放”收敛为项目自有结构。
3. 后续如果再更换底层 SDK，不应再次牵动上层工具循环与会话逻辑。

### 3.3 结果目标

1. `src-tauri/Cargo.toml` 中不再依赖 `rig-core`。
2. `src-tauri/src/main.rs` 中不再直接 `use rig::...`。
3. 聊天运行链路的 provider 调用统一从项目自有适配层发出。
4. OpenAI Responses 调试请求体能真实显示 `instructions + input` 语义。

当前状态补充：

1. 第 1、2、3、4 项已经完成。

## 4. 非目标

1. 本次不改前端视觉交互与聊天界面布局。
2. 本次不重新设计工具系统业务语义。
3. 本次不改现有会话、记忆、任务、委托的数据存储格式。
4. 本次不改 MCP server 定义格式与工作区目录结构。
5. 本次不顺手重做 provider 配置 UI。

## 5. 迁移判断

### 5.1 本次迁移不属于“重写产品”

原因：

1. 工具执行器本来就是项目自己调度。
2. Tool loop 本来就是项目自己控制。
3. 前端增量事件与消息持久化逻辑本来就在项目里。
4. Prompt 组装、压缩、归档、SummaryContext 逻辑本来就在项目里。

因此，本次迁移本质上是：

1. 替换模型接入层。
2. 替换消息类型接头。
3. 替换 provider stream 到项目增量事件的映射。
4. 替换 MCP 到模型工具协议的桥接。

### 5.2 本次迁移的真正难点

真正的难点不在工具执行，而在以下接口层：

1. `rig` 类型已经渗透到 `main.rs` 与 provider 调用文件中。
2. OpenAI / Gemini / Anthropic 的流式事件接头当前建立在 `rig` 的流式类型之上。
3. Tool schema 与 MCP bridge 当前直接依赖 `rig::tool`。
4. 历史消息回放当前建立在 `RigMessage / UserContent / AssistantContent` 之上。

## 6. 现状落点

当前 `rig` 主要落在以下位置：

### 6.1 全局入口

1. `src-tauri/src/main.rs`
2. `src-tauri/Cargo.toml`

### 6.2 Provider 调用与消息接头

1. `src-tauri/src/features/chat/model_runtime/provider_and_stream/openai_style.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/gemini.rs`
3. `src-tauri/src/features/chat/model_runtime/provider_and_stream/anthropic.rs`
4. `src-tauri/src/features/chat/model_runtime/tools_and_builtin/core_provider_calls.rs`
5. `src-tauri/src/features/chat/model_runtime/provider_and_stream/stream_collect.rs`
6. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_loop.rs`

### 6.3 工具 schema 与运行时挂接

1. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`
2. `src-tauri/src/features/system/commands/chat_and_runtime/tool_catalog.rs`

### 6.4 MCP bridge

1. `src-tauri/src/features/mcp/runtime_manager.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`

## 7. 总体方案

### 7.1 总体原则

1. 不把 `rust-genai` 类型直接扩散到聊天业务层。
2. 先建立项目内部统一调用边界，再把 `rust-genai` 填进去。
3. 不保留 `rig` 与 `rust-genai` 双轨长期共存状态。
4. 工具执行继续由项目自己掌控，不把工具循环重新外包给 SDK。

### 7.2 新的内部边界

计划新增项目自有的统一抽象：

1. `ProviderRequestMessage`
2. `ProviderStreamEvent`
3. `ProviderToolSchema`
4. `ProviderToolCall`
5. `ProviderToolResult`
6. `ProviderChatClient`

这些类型不要求一开始就完全公开，但至少应把当前直接依赖 `RigMessage` 的边界收口。

### 7.3 rust-genai 的职责定位

`rust-genai` 在本项目中只承担：

1. Provider 请求组装与发送。
2. Provider 流式事件解析。
3. Provider 工具 schema 序列化。
4. Provider 响应对象基础解析。

不承担：

1. 工具执行。
2. 工具循环控制。
3. 会话管理。
4. 上下文压缩策略。
5. 调试日志策略。

## 8. 具体改造方案

### 8.1 依赖层改造

目标：

1. 删除 `rig-core` 依赖。
2. 引入 `rust-genai`。
3. 评估是否保留 `rmcp` 原依赖。

涉及文件：

1. `src-tauri/Cargo.toml`

动作：

1. 删除 `rig = { package = "rig-core", ... }`
2. 增加 `rust-genai` 依赖
3. 根据 `rust-genai` 的流式、OpenAI Responses、Gemini、Anthropic 能力补齐所需 feature

### 8.2 全局导入层改造

目标：

1. 清理 `main.rs` 中的 `rig` 全局导入
2. 建立项目自有 provider 类型入口

涉及文件：

1. `src-tauri/src/main.rs`
2. 新增 provider 抽象文件

动作：

1. 删掉 `use rig::{...}` 相关导入
2. 让各 include 文件改为依赖项目内部抽象类型

### 8.3 Provider 调用层改造

目标：

1. 用 `rust-genai` 替换 OpenAI / OpenAI Responses / Gemini / Anthropic 的调用实现

涉及文件：

1. `src-tauri/src/features/chat/model_runtime/provider_and_stream/openai_style.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/gemini.rs`
3. `src-tauri/src/features/chat/model_runtime/provider_and_stream/anthropic.rs`
4. `src-tauri/src/features/chat/model_runtime/tools_and_builtin/core_provider_calls.rs`

动作：

1. OpenAI Chat Completions 使用 `rust-genai` openai adapter
2. OpenAI Responses 使用 `rust-genai` openai_resp adapter
3. Gemini 使用 `rust-genai` gemini adapter
4. Anthropic 使用 `rust-genai` anthropic adapter
5. 响应 usage、reasoning、stop reason 等字段映射回项目内部结构

### 8.4 OpenAI Responses 语义修正

目标：

1. 恢复 OpenAI 官方 Responses 语义

动作：

1. 顶层 system prompt 进入 `instructions`
2. 历史消息与当前用户消息进入 `input`
3. 不再由底层 SDK 强制改写为 input `system` message
4. 调试日志应展示真实请求结构而不是 chat-completions 风格镜像

### 8.5 流式事件接头改造

目标：

1. 不改前端增量协议
2. 仅替换底层 provider stream 到 `AssistantDeltaEvent` 的映射

涉及文件：

1. `src-tauri/src/features/chat/model_runtime/provider_and_stream/stream_collect.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_loop.rs`

动作：

1. 将 `rust-genai` 流式输出映射为项目现有文本 delta
2. 将 reasoning 增量映射回现有 reasoning 字段
3. 将 tool call 增量与完成事件映射回工具循环所需结构
4. 保持现有 UI 不感知底层 SDK 切换

### 8.6 消息历史与工具回放改造

目标：

1. 去掉 `RigMessage` 依赖
2. 保持现有历史回灌、tool replay、reasoning replay 逻辑

涉及文件：

1. `src-tauri/src/features/chat/model_runtime/tools_and_builtin/core_provider_calls.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_loop.rs`

动作：

1. 用项目内部消息结构替代 `RigMessage`
2. 将 `PreparedPrompt` 转换为 `rust-genai` 所需消息结构
3. 保留对 assistant/tool 历史的 replay 能力
4. 保留 legacy tool history 降级文本策略

### 8.7 工具 schema 改造

目标：

1. 替换 `rig::tool::Tool::definition(...)`
2. 保持工具目录与模型工具协议一致

涉及文件：

1. `src-tauri/src/features/system/commands/chat_and_runtime/tool_catalog.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`

动作：

1. 新增项目自有 `ToolSchemaDefinition`
2. 内建工具 schema 不再通过 `rig` 生成
3. 前端工具目录直接读取项目自有 schema
4. Provider 层再把该 schema 转为 `rust-genai` 格式

### 8.8 MCP bridge 改造

目标：

1. 移除 `rig::tool::rmcp::McpTool::from_mcp_server(...)`
2. 保持 MCP 工具可继续参与模型工具调用

涉及文件：

1. `src-tauri/src/features/mcp/runtime_manager.rs`
2. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`

动作：

1. 把 MCP tool 定义抽成项目自有 schema
2. 调用时由项目自己执行 `rmcp` client 请求
3. MCP tool 与内建工具统一进入同一工具执行调度层

说明：

这是本次迁移中最需要仔细设计的一块，因为当前这里是 `rig` 提供的现成桥接点，迁移后必须改成项目自管。

### 8.9 调试与日志改造

目标：

1. 日志展示真实 provider 请求结构
2. 不再出现“实际走 Responses，但日志仍是 chat-completions 风格”的误导

涉及文件：

1. `src-tauri/src/features/system/commands/debug_log_commands.rs`
2. `src-tauri/src/features/system/commands/inference_gateway.rs`

动作：

1. OpenAI Responses 日志显示 `instructions`、`input`
2. OpenAI Chat 日志继续显示 `messages`
3. Gemini / Anthropic 显示各自真实 payload 结构
4. 保留当前调试面板兼容字段，但内部来源改为真实请求体

## 9. 文件影响范围

预计主要涉及：

1. `src-tauri/Cargo.toml`
2. `src-tauri/src/main.rs`
3. `src-tauri/src/features/chat/model_runtime/provider_and_stream/openai_style.rs`
4. `src-tauri/src/features/chat/model_runtime/provider_and_stream/gemini.rs`
5. `src-tauri/src/features/chat/model_runtime/provider_and_stream/anthropic.rs`
6. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_loop.rs`
7. `src-tauri/src/features/chat/model_runtime/provider_and_stream/stream_collect.rs`
8. `src-tauri/src/features/chat/model_runtime/provider_and_stream/tool_assembly.rs`
9. `src-tauri/src/features/chat/model_runtime/tools_and_builtin/core_provider_calls.rs`
10. `src-tauri/src/features/system/commands/chat_and_runtime/tool_catalog.rs`
11. `src-tauri/src/features/system/commands/debug_log_commands.rs`
12. `src-tauri/src/features/system/commands/inference_gateway.rs`
13. `src-tauri/src/features/mcp/runtime_manager.rs`

预计还会新增：

1. provider-neutral 类型定义文件
2. `rust-genai` 适配层封装文件
3. tool schema 通用定义文件
4. MCP tool adapter 文件

## 10. 风险点

### 10.1 MCP 桥接是最大风险点

风险：

1. 当前 MCP 到模型工具协议的桥接明显依赖 `rig`
2. 若改造不完整，可能出现 MCP 工具目录能显示但运行时无法调用

处理：

1. 先把 MCP schema 与调用拆成项目自有层
2. 再让 provider 层只消费项目自有工具定义

### 10.2 流式 reasoning 与 tool-call 增量字段可能不完全等价

风险：

1. `rust-genai` 的 stream event 结构未必与 `rig` 完全同形
2. 如果映射过于草率，可能导致 reasoning 丢失或 tool delta 拼接错位

处理：

1. 明确建立项目自有 `ProviderStreamEvent`
2. 不把上层业务直接绑到 `rust-genai` 事件结构上

### 10.3 调试日志与真实请求体容易短期漂移

风险：

1. 迁移时若先改运行链路、后补日志，调试面板会短暂误导

处理：

1. 请求日志与运行链路必须在同一轮改动内一起替换

### 10.4 多 provider 细节差异可能暴露更多历史兼容代码

风险：

1. 过去被 `rig` 遮蔽的问题，迁移后会显性化
2. 尤其是 Gemini / Anthropic 的多模态与 tool history 细节

处理：

1. 明确按 provider 分测试
2. 不做“统一后再猜”的黑盒迁移

## 11. 验证

### 11.1 构建验证

1. `cd src-tauri && cargo check`
2. `pnpm typecheck`

### 11.2 Provider 验证

1. OpenAI Chat 普通文本对话正常
2. OpenAI Responses 普通文本对话正常
3. Codex / GPT-5 类模型顶层 `instructions` 正常生效
4. Gemini 文本与图片输入正常
5. Anthropic 文本与工具调用正常

### 11.3 工具验证

1. 内建工具 schema 能正确展示在前端工具目录
2. `command`、`exec`、`apply_patch`、`task`、`delegate` 正常
3. `operate`、`read_file`、`todo` 等 MCP 化工具正常
4. 外部 MCP server 工具可列出并实际调用

### 11.4 日志验证

1. OpenAI Responses 请求日志显示 `instructions`
2. OpenAI Responses 请求日志显示 `input`
3. 日志内容与真实发送请求一致

### 11.5 回归验证

1. 会话发送、流式输出、停止生成、重试、撤回不回退
2. SummaryContext 压缩与归档链路不回退
3. 提示词预览与真实请求继续一致

## 12. 建议执行顺序

虽然本计划目标是“彻底全量迁移”，但实现顺序仍建议这样安排，以降低返工：

1. 先建立项目自有 provider-neutral 类型层
2. 再接入 `rust-genai` 的 OpenAI / OpenAI Responses / Gemini / Anthropic
3. 再替换流式事件接头
4. 再替换工具 schema 生成
5. 最后替换 MCP bridge
6. 全部跑通后删除 `rig` 依赖与残留引用

这只是实现顺序，不是分阶段上线策略。

当前状态补充：

1. 第 6 项现已完成，`rig-core` 依赖已从 `src-tauri/Cargo.toml` 移除。

## 13. 结果预期

如果本计划按预期完成，项目将得到这些结果：

1. OpenAI 官方 Responses 语义恢复正常。
2. `Codex` 不再因为 `rig` 的错误兼容策略失去顶层 `instructions`。
3. Provider 调用边界转为项目自控，而不是继续被第三方 SDK 类型污染。
4. 工具执行权仍牢牢掌握在项目内部。
5. 后续无论替换 SDK 还是补 provider，都只需要改适配层，而不必再次牵动聊天业务层。

## 14. 已确认口径

本计划实施前已确认以下口径：

1. 本次是否接受“彻底移除 `rig`，不保留双轨兼容”。
2. 本次是否接受同步重做 MCP tool bridge。
3. 本次是否接受在迁移完成前不做其他并行功能开发，以避免 provider 接口反复漂移。
