# 聊天 UI 数据流说明

> 更新于 2026-10-26（核对照代码，映射关系仍成立；不重写结构）

## 目的

这份文档只做一件事：把"真相层聚合 assistant 消息"如何映射到聊天 UI 说明清楚，并且与 `docs/chat-message-flow-assertions.md` 保持一致。

本文只采用下面三层口径：

1. 真相层
2. 请求体层
3. UI 投影层

## 1. 先看正确口径

本项目必须以 `docs/chat-message-flow-assertions.md` 为准。

最重要的断言只有四条：

1. 真相层存储的是聚合 assistant 消息，不是请求体消息序列。
2. `toolCall` 只保存 tool round，不保存最后一条 assistant final text。
3. 最终答复文本写入 `parts[*].text`。
4. 最终答复文本对应思维链写入 `providerMeta.reasoningStandard`。

## 2. 真相层正确结构

真实类型入口：

- `src/types/app.ts` 的 `MessagePart`
- `src/types/app.ts` 的 `ChatMessage`

对 assistant 来说，真相层的一条聚合消息关键字段：

| 字段 | 语义 |
|---|---|
| `parts[*].text` | 最终给用户看的正文 |
| `providerMeta.reasoningStandard` | 最终正文对应思维链 |
| `contentBlocks` | 流式快照直接写入的增量块（`AssistantStreamBlock[]`）；完成/停止不得重建 |
| `toolCall[* role=assistant]` | 某一轮 tool round 的 assistant 请求 |
| `toolCall[* role=tool]` | 对应工具返回 |

注意：

```text
真相层的 toolCall[* role=assistant].content 默认可以是 null
最终 assistant 正文不在 toolCall 里
```

## 3. 请求体层正确结构

同一条真相层聚合 assistant 消息，拆回请求体后才是消息序列。这里才存在两种 assistant 消息：

1. tool-call assistant

```json
{
  "role": "assistant",
  "content": null,
  "reasoning_content": "...",
  "tool_calls": [...]
}
```

2. final-text assistant

```json
{
  "role": "assistant",
  "content": "终端版本是 PowerShell 7.5.4。",
  "reasoning_content": "..."
}
```

这两者不能混为一谈。具体格式与折叠规则见 `docs/chat-message-flow-assertions.md`。

## 4. 关于"工具消息正文"的正确说法

如果后续要在 UI 上处理"工具消息正文"，必须先说清是请求体层还是真相层。

### 4.1 真相层

在真相层里，tool round assistant 消息通常是 `content: null`，按断言文档，默认不存在可显示正文。

### 4.2 请求体层

在请求体层里，tool-call assistant 消息理论上可以带正文：

```json
{
  "role": "assistant",
  "content": "我先去读一下 config.toml。",
  "reasoning_content": "先读取配置文件确认字段名。",
  "tool_calls": [...]
}
```

这里：

- `content` 才是"工具消息正文"
- `reasoning_content` 是该轮工具调用前的思维链
- `tool_calls` 是工具调用声明

所以以后如果说"把图标挂在工具正文后面"，准确口径应该是：

```text
挂在请求体层 tool-call assistant 消息的 content 后面
仅当该 content 非空时显示
```

## 5. 当前前端是怎么读真相层数据的

当前前端主要直接吃真相层 `ChatMessage`，而不是直接渲染请求体消息序列。

关键入口：

- `src/utils/chat-message.ts` 的 `renderMessage()` / `messageText()`
- `src/utils/chat-message-semantics.ts` 的 `assistantContentBlocksFromMessage()`
- `src/utils/chat-message-semantics.ts` 的 `assistantTextFromStreamBlocks()`
- `src/utils/chat-message-semantics.ts` 的 `normalizeMessageToolHistoryEvents()`
- `src/utils/chat-message-semantics.ts` 的 `projectChatActivityForDisplay()`
- `src/utils/chat-message-semantics.ts` 的 `projectMessageForDisplay()`

## 6. 正文与工具活动的数据源

当前正文与工具活动有两条来源，按状态切换：

### 6.1 流式正文：contentBlocks

流式输出期间，正文直接以 `contentBlocks`（`AssistantStreamBlock[]`）增量写入消息，前端从 `contentBlocks` 全量重算显示文本（`assistantTextFromStreamBlocks`），不再维护独立的流式分段字段。

### 6.2 最终正文

`projectMessageForDisplay()` 会先从真相层消息中提取最终正文。正文来源是 `parts[*].text`，不是 `toolCall[* role=assistant].content`。

### 6.3 工具过程

前端把 `toolCall[]` 解析成工具活动数据，而不是把它平铺成多条外层聊天行。工具过程来源是 `toolCall[]`，解析后变成：

- `toolCallCount`
- `lastToolName`
- `toolCalls`
- `activityItems`

### 6.4 停止后的兜底校验

停止后若 `contentBlocks` 有内容但投影为空（`projection.toolCalls` / `projection.activityItems` 均为空），会触发 `console.warn` 提示"停止后消息投影缺失"，这是显示层的一致性守护，不代表真相层结构本身。

## 7. 从真相层消息到渲染块

`src/features/chat/composables/use-chat-turns.ts` 的 `buildMessageBlocks()` 会把一条 `ChatMessage` 投影成一个或多个 `ChatMessageBlock`，并带缓存（`messageBlockCache`，按消息签名失效）。

对典型 assistant 聚合消息来说，主要结果是一个主 `ChatMessageBlock`，字段见 `src/types/app.ts` 的 `ChatMessageBlock`（`text`、`toolCallCount`、`lastToolName`、`toolCalls`、`activityItems`、`images`、`audios`、`attachmentFiles`、`activityStatus` 等）。

也就是说：

```text
真相层一条聚合 assistant 消息
→ 前端仍然只渲染成一条主聊天项
→ 工具过程显示在该聊天项内部
```

## 8. 最终 UI 是怎么组织的

渲染组件入口：

- `src/features/chat/views/ChatView.vue`
- `src/features/chat/components/ChatMessageItem.vue`

对 assistant 聚合消息，当前 UI 大致分三块：

1. 头部：说话人、时间、流式状态
2. 主正文气泡：来自 `parts[*].text` 投影后的 `block.text`
3. 活动面板：来自 `toolCall[]` / `activityItems`（流式时从 `contentBlocks` 重算）

正文渲染支持两种模式：

- 分段 Markdown（`assistantMarkdownPieces`，按 `TOOL_TEXT_BREAK_PLACEHOLDER` 分段后走 `AppMarkdownRenderer`）
- 整段 Markdown（`AppMarkdownRenderer` / `PlainMarkdownRenderer` 调试模式）

所以当前用户看到的是：

```text
一条 assistant 聊天气泡
├─ 正文
└─ 工具活动面板
```

而不是：

```text
assistant(tool-call)
tool(result)
assistant(final text)
```

这种请求体层平铺序列。

## 9. 一句话总结

一句话总结当前正确口径：

```text
真相层保存的是一条聚合 assistant 消息：
最终正文在 parts，
最终正文思维链在 providerMeta.reasoningStandard，
toolCall 只保存 tool round，
流式增量在 contentBlocks；
当前聊天 UI 把这条聚合消息渲染成"一条主气泡 + 一个工具活动面板"。
```

而"工具正文后面的小图标"这个需求，应该挂在：

```text
请求体层 tool-call assistant.content
```

仅当该 `content` 非空时显示。
