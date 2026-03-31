# 20260401 SummaryContext统一归档与压缩计划

## 1. 说明

本计划基于当前暂存实现反向整理，目标是把“归档 + 上下文压缩 + 记忆整理”已经形成的统一设计写清楚，方便和原始设想对照。

## 2. 背景

从当前暂存代码看，旧方案存在三类分裂：

1. 聊天用一套 prompt 组装逻辑。
2. 上下文压缩用另一套独立 prompt 逻辑。
3. 会话归档再走第三套归档 prompt 与后置异步记忆整理逻辑。

这会带来两个问题：

1. 模型在不同场景下看到的上下文形状不一致。
2. `summary`、记忆保留、记忆合并、用户画像提炼分散在多次模型调用和多条代码路径里，失败与回退也不好统一。

当前暂存代码已经把这三条链路收敛为统一的 `SummaryContext` 设计。

## 3. 目标

本次重构后的实际目标可以整理为：

1. 统一“上下文压缩”和“会话归档”的模型任务语义。
2. 统一模型输出 JSON 结构。
3. 统一记忆结果应用流程。
4. 统一失败回退与日志口径。
5. 保持压缩场景与归档场景仅在“摘要用途”上存在差异，而不是整套实现完全分叉。

## 4. 当前方案

### 4.1 统一模式名

`PromptBuildMode` 中旧的 `Archive` 模式已经被替换为 `SummaryContext`。

这表示现在不是为归档单独维护一套“弱化版聊天 prompt”，而是复用正常聊天 prompt 构造历史上下文，再通过 latest user payload 覆盖成当前任务。

### 4.2 SummaryContext 场景枚举

当前实现把 SummaryContext 明确分成两种场景：

1. `Compaction`
2. `Archive`

两者共用：

1. 同一套 prompt 组装入口
2. 同一套 JSON contract
3. 同一套结果解析器
4. 同一套结果写库逻辑

两者仅在以下方面有差异：

1. `summary` 的用途描述不同
2. 回退文案不同
3. 最终写回位置不同

### 4.3 统一输出 JSON

模型输出被统一为以下五字段：

1. `summary`
2. `usefulMemoryIds`
3. `newMemories`
4. `mergeGroups`
5. `profileMemories`

这意味着：

1. 摘要生成和记忆整理不再拆成两次独立语义。
2. 压缩场景也可以顺带完成记忆反馈、记忆新增、记忆合并、画像记忆提炼。
3. 归档场景直接输出最终归档摘要，不再先做预压缩、再做归档总结的双阶段模型组织。

### 4.4 Prompt 组装方式

SummaryContext 仍然复用 `build_prompt(...)` 生成完整系统前言和历史消息，再通过 `ChatPromptOverrides` 把最新用户载荷替换成任务块：

1. `latest_user_text`
   - 放 `summary_requirement`
2. `latest_user_meta_text`
   - 放 `memory_curation_context`
3. `latest_user_extra_blocks`
   - 放 `json_contract`

这样做的含义是：

1. 模型依然看到完整对话历史。
2. 最后一轮“用户输入”不再是假用户消息，而是当前任务说明。
3. SummaryContext 与普通聊天共享更多 prompt 基础能力，减少两套 prompt 漂移。

### 4.5 用户画像在 SummaryContext 中的地位

SummaryContext 不再只接收“本次会话曾经命中的记忆”。

现在它还会收到：

1. 当前完整用户画像记忆板
2. 带短 ID 的画像候选
3. 可用于纠错、合并、引用的完整画像上下文

因此 SummaryContext 的职责已经从“只做会话摘要”扩展成：

1. 生成摘要
2. 反馈本轮命中记忆是否有用
3. 提炼可保存的新记忆
4. 合并重复记忆
5. 维护用户画像记忆集合

### 4.6 结果解析

`parse_memory_curation_draft` 已支持：

1. `summary`
2. `profileMemories`
3. 数字型短 ID
4. 字符串型短 ID
5. 传统 UUID

说明当前实现明确允许模型直接输出短 ID，例如 `12`，后端再通过 alias map 解析回真实 `memory_id`。

### 4.7 统一结果应用

当前实现通过 `apply_summary_context_result(...)` 一次性完成：

1. `usefulMemoryIds` 的反馈应用
2. `newMemories` 的写入
3. `mergeGroups` 的合并
4. `profileMemories` 的链接或新建

这替代了旧的“压缩写回后再异步生成记忆”的双阶段方案。

### 4.8 压缩场景落地方式

压缩场景下：

1. SummaryContext 返回的 `summary` 会被写成新的 `[上下文整理]` 用户消息
2. 该消息可附带当前用户画像快照
3. 会话本身会更新 `user_profile_snapshot`
4. 同时应用本轮的记忆反馈、合并和画像更新

也就是说，压缩不再只是“写一条摘要消息”，而是一次完整的 SummaryContext 收敛动作。

### 4.9 归档场景落地方式

归档场景下：

1. SummaryContext 直接产出最终归档摘要
2. 该摘要直接交给 `archive_conversation_now(...)`
3. 记忆反馈、合并、画像链接同样立即应用
4. 归档完成后再切换到一个新的前台会话

归档前的“预整理消息写回当前会话”已经被移除，当前设计更偏向：

1. 用同一次 SummaryContext 直接完成总结
2. 不额外向即将归档的会话再写一条中间消息

### 4.10 新前台会话的种子消息

归档后切到的新前台会话，不再是空白会话。

当前实现会自动写入一条初始 SummaryContext 消息，其中包含：

1. 最近一次归档摘要
2. 用户画像快照

这相当于把“上次聊到哪里 + 用户长期背景”固化成下一轮会话的第一条上下文。

### 4.11 回退策略

当前实现的回退链路已经统一成：

压缩场景：

1. 先重试 SummaryContext
2. 失败后降级到“已有压缩信息 + 最后三轮正文”
3. 再失败则降级到“最后三轮正文”

归档场景：

1. 先执行 SummaryContext
2. 失败后降级到“已有压缩信息 + 最后三轮正文”
3. 再失败则降级到“最后三轮正文”

## 5. 非目标

当前暂存实现没有表现出以下目标，因此本计划将其列为非目标或后续事项：

1. 本次不引入新的独立数据库表来持久化 SummaryContext 结果全文。
2. 本次不把压缩摘要与归档摘要拆成两种完全不同的数据结构。
3. 本次不做“多版本摘要历史”管理。
4. 本次不把 SummaryContext 结果直接暴露成单独 UI 面板。

## 6. 涉及文件

后端核心文件：

1. `src-tauri/src/features/system/commands/archive_pipeline.rs`
2. `src-tauri/src/features/system/commands/archive_summary_parser.rs`
3. `src-tauri/src/features/system/commands/prompt_assembly.rs`
4. `src-tauri/src/features/system/commands/memory_curation/prompt_contract.rs`
5. `src-tauri/src/features/system/commands.rs`
6. `src-tauri/src/features/system/commands/archive_commands.rs`

相关协作文件：

1. `src-tauri/src/features/chat/conversation.rs`
2. `src-tauri/src/features/chat/scheduler.rs`
3. `src-tauri/src/features/system/commands/archive_host_selector.rs`

## 7. 风险点

### 7.1 单次模型调用职责过多

风险：

1. SummaryContext 现在一次要同时产出摘要、记忆反馈、记忆新增、记忆合并、画像提炼。
2. 若模型质量波动，五个字段可能一起受影响。

处理：

1. 解析器对 JSON 做严格约束。
2. 回退路径至少保证 `summary` 可降级。
3. 记忆相关字段为空时不应阻塞摘要主流程。

### 7.2 画像提炼可能过度

风险：

1. 若模型把临时上下文错误放入 `profileMemories`，会污染长期画像。

处理：

1. prompt contract 已限制只允许事实属性、职业技能、关系图谱、活跃项目。
2. `emotion` 被禁止进入画像。
3. 只有 `knowledge/skill/event` 可作为画像类型。

### 7.3 新前台会话种子消息过重

风险：

1. 若归档摘要或画像快照过长，新会话首屏上下文会膨胀。

处理：

1. 当前只写快照，不写完整画像板。
2. 快照接口已有 `limit` 参数。
3. 后续如继续膨胀，再单独收紧快照条数与内容密度。

## 8. 验证

### 8.1 代码层

1. `PromptBuildMode::Archive` 已被 `SummaryContext` 取代。
2. `summary` 已成为 SummaryContext 的必填核心输出。
3. 压缩与归档都调用 `apply_summary_context_result(...)`。
4. 旧的独立 `context_compaction/prompt_contract.rs` 与 `conversation_archive/prompt_contract.rs` 已退出主入口。

### 8.2 行为层

1. 强制归档时，最终摘要来自 SummaryContext 单次返回。
2. 上下文压缩时，压缩消息写回后能同步带上更新后的用户画像快照。
3. SummaryContext 失败时，仍能得到可继续使用的降级摘要。
4. 压缩与归档完成日志都应带出 merged memories、merge groups、profile linked 等指标。

### 8.3 建议自测

1. `cd src-tauri && cargo test`
2. 对同一会话分别触发一次上下文压缩与一次强制归档
3. 检查返回摘要、记忆反馈、画像链接是否都落地
4. 检查归档后新会话是否已带初始 SummaryContext 种子消息

## 9. 结果预期

如果当前设计保持成立，那么这轮重构的真实成果应当是：

1. 压缩与归档终于共用同一套语义中心。
2. 记忆整理不再是挂在后面的异步尾巴，而是 SummaryContext 正式输出的一部分。
3. 新会话不再从“纯空白”开始，而是从“上次归档摘要 + 用户画像快照”开始。

