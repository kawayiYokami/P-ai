# 聊天消息流转断言

本文档定义正确格式与流转语义，同时描述当前真相层存储形态（V4 多行组 + zstd 压缩块）。

## 1. 基本原则

1. 真相层存储的是聚合消息：一次 AI 调度 = 一条 ChatMessage（聚合多个工具调用、正文与思维链）。
2. 只有一次 assistant 调度会被聚合成一条 assistant 消息。
3. user 消息永远是离散消息，不参与 assistant 调度聚合。
4. 请求体层必须把聚合 assistant 消息拆回消息序列。
5. 调度中缓存的是"新增请求体消息"，不是聚合消息。
6. 完成或中止时，必须把缓存里的新增请求体消息重新折叠为一条 assistant 聚合消息。

## 2. 思维链字段职责

### 2.1 tool round 思维链

- 字段：`toolCall[*].reasoning_content`
- 语义：某一轮 assistant 发起该次工具调用之前的思维链
- 作用域：只属于这一轮 tool-call assistant 消息
- 禁止：
  - 用最终回答的思维链覆盖它
  - 用多轮累计思维链覆盖它
  - 在重建请求体时省略它

### 2.2 最终回答思维链

- 字段：`assistant.providerMeta.reasoningStandard`
- 语义：本条聚合 assistant 消息中，最终答复文本对应的思维链
- 作用域：只属于最终 assistant 文本
- 禁止：
  - 解释为整次调度累计思维链
  - 解释为本次调度所有 assistant 子消息的聚合思维链
  - 回灌到每一轮 `toolCall[*].reasoning_content`
  - 在请求体重建时挪到 tool-call assistant 消息上

## 3. 真相层存储格式（V4）

真相层 = 一条聚合 ChatMessage = 一组多行（`chat/conversations/<id>/blocks/*.jsonl.zstd`，zstd 压缩）。

### 3.1 组行 schema

- **工具行**：`tool_call` 数组的一个元素一行 `{"kind":"tool", id, role, createdAt, speakerAgentId?, "event":{...}}`。按序拆行/按序拼接，不做 call/result 配对假设（并行调用是 1 个 call 元素内含 N 个 tool_calls、后跟 N 个 result 元素；还有截图等第三方形态，逐行原样搬运对任何形态成立）。
- **正文行**：最终正文到达后追加一行 `{"kind":"assistant", id, role, createdAt, speakerAgentId?, parts, extraTextBlocks, providerMeta, memeAnnotations?, mcpCall?}`。
- **普通消息（无工具）**：单行 `{"kind":"message", "message":{完整 ChatMessage}}`。
- 公共字段（id/role/createdAt/speakerAgentId）冗余在组内每一行——正文行最后才写，未闭合组只有工具行，必须能独立还原消息骨架。

### 3.2 组行示例

一次工具调度（工具调用 + 最终回答）的聚合 ChatMessage，落盘为多行组：

```jsonl
{"kind":"tool","id":"msg_1","role":"assistant","createdAt":"2026-08-23T10:00:00.000Z","event":{"role":"assistant","content":null,"reasoning_content":"先调用终端工具查看 PowerShell 版本。","tool_calls":[{"id":"call_1","type":"function","function":{"name":"exec","arguments":"{\"command\":\"pwsh --version\"}"}}]}}
{"kind":"tool","id":"msg_1","role":"assistant","createdAt":"2026-08-23T10:00:00.000Z","event":{"role":"tool","tool_call_id":"call_1","content":"PowerShell 7.5.4"}}
{"kind":"assistant","id":"msg_1","role":"assistant","createdAt":"2026-08-23T10:00:00.000Z","parts":[{"type":"text","text":"终端版本是 PowerShell 7.5.4。"}],"providerMeta":{"reasoningStandard":"我已经拿到工具结果，现在直接回答用户终端版本。"}}
```

普通消息（无工具调用）单行：

```jsonl
{"kind":"message","message":{"id":"msg_2","role":"user","createdAt":"2026-08-23T10:00:00.000Z","parts":[{"type":"text","text":"查一下 PowerShell 版本"}]}}
```

### 3.3 块与定位

- 块文件：`blocks/{block_id:06}.jsonl.zstd`，整块单帧 zstd 压缩，原子写入（`.jsonl.zstd.tmp` 临时文件替换）。
- 明文坐标 = 组级：`message_locator` 表的 `byte_offset` 指向组首行、`byte_len` 覆盖整组多行；`messages.idx.json` 是块内定位索引。
- 定位权威：`chat/chat_metadata.sqlite.message_locator`（conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at）。

### 3.4 解压脚本

仓库提供 [scripts/unpack_message_blocks.py](../scripts/unpack_message_blocks.py)：

```bash
python scripts/unpack_message_blocks.py <data-dir>
python scripts/unpack_message_blocks.py <data-dir> --conversation <conversation-id>
```

- 流式解压全部 `.jsonl.zstd` 块到输出目录（默认输入目录同级 `unpacked-blocks/`）
- 对照 `chat_metadata.sqlite` 的 `message_locator` 做结束偏移对账（`MAX(byte_offset + byte_len)` == 明文字节数）
- 需要 `pip install zstandard`

### 3.5 组装规则（组 → ChatMessage）

1. 公共字段（id/role/createdAt/speakerAgentId）取组内第一行（各行冗余一致）
2. 工具行 → `tool_call` 数组（每行一个元素按序 push，不配对）
3. 正文行 → `parts` / `extraTextBlocks` / `providerMeta` / `memeAnnotations` / `mcpCall`
4. 未闭合组（只有工具行、无正文行）→ parts 等默认值，仅工具调用有效
5. 普通消息行必须是组内唯一有效行；「message 行 + 其他行」混排是帧落盘但事务未提交的崩溃形态，显式报错让上层走整块校验

## 4. 请求体层正确格式

聚合 assistant 消息（第 3.1 节的语义等价物）拆开后，必须变成以下消息序列。

```json
[
  {
    "role": "user",
    "content": "查一下 PowerShell 版本"
  },
  {
    "role": "assistant",
    "content": null,
    "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
    "tool_calls": [
      {
        "id": "call_1",
        "type": "function",
        "function": {
          "name": "exec",
          "arguments": "{\"command\":\"pwsh --version\"}"
        }
      }
    ]
  },
  {
    "role": "tool",
    "tool_call_id": "call_1",
    "content": "PowerShell 7.5.4"
  },
  {
    "role": "assistant",
    "content": "终端版本是 PowerShell 7.5.4。",
    "reasoning_content": "我已经拿到工具结果，现在直接回答用户终端版本。"
  }
]
```

## 5. 调度缓存层正确格式

调度未完成前，缓存中保存的是新增请求体消息。

### 5.1 模型刚发出 tool call

```json
[
  {
    "role": "assistant",
    "content": null,
    "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
    "tool_calls": [
      {
        "id": "call_1",
        "type": "function",
        "function": {
          "name": "exec",
          "arguments": "{\"command\":\"pwsh --version\"}"
        }
      }
    ]
  }
]
```

### 5.2 工具执行完成

```json
[
  {
    "role": "assistant",
    "content": null,
    "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
    "tool_calls": [
      {
        "id": "call_1",
        "type": "function",
        "function": {
          "name": "exec",
          "arguments": "{\"command\":\"pwsh --version\"}"
        }
      }
    ]
  },
  {
    "role": "tool",
    "tool_call_id": "call_1",
    "content": "PowerShell 7.5.4"
  }
]
```

### 5.3 最终回答生成

```json
[
  {
    "role": "assistant",
    "content": null,
    "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
    "tool_calls": [
      {
        "id": "call_1",
        "type": "function",
        "function": {
          "name": "exec",
          "arguments": "{\"command\":\"pwsh --version\"}"
        }
      }
    ]
  },
  {
    "role": "tool",
    "tool_call_id": "call_1",
    "content": "PowerShell 7.5.4"
  },
  {
    "role": "assistant",
    "content": "终端版本是 PowerShell 7.5.4。",
    "reasoning_content": "我已经拿到工具结果，现在直接回答用户终端版本。"
  }
]
```

## 6. 完成持久化后的正确折叠结果

完成或中止时，必须把第 5.3 节中的缓存重新折叠为一条聚合 assistant 消息，再以 V4 多行组格式写入真相层（第 3.1-3.2 节）。

折叠规则：

1. 最后一条有文本的 assistant 消息，写入 `parts[*].text`
2. 该最后一条 assistant 消息的 `reasoning_content`，写入 `providerMeta.reasoningStandard`
3. 之前所有带 `tool_calls` 的 assistant 消息，按顺序进入 `toolCall`
4. 对应的 tool 消息，按顺序进入 `toolCall`
5. 禁止把最后一条 assistant 消息也塞进 `toolCall`
6. 禁止把 tool round 的 `reasoning_content` 覆盖到 `providerMeta.reasoningStandard`

硬断言：

1. 第一条 tool-call assistant 的 `reasoning_content` 必须进入 `toolCall[0].reasoning_content`
2. tool 结果必须进入对应的 `toolCall[* role=tool]`
3. 最终 assistant 文本必须进入 `parts[*].text`
4. 最终 assistant 文本对应的思维链必须进入 `providerMeta.reasoningStandard`
5. `providerMeta.reasoningStandard` 只对应最终答复文本，不对应整次调度
6. 禁止把多轮 tool-call 思维链拼接后写入 `providerMeta.reasoningStandard`

## 7. 多轮工具调用断言

若 assistant 一次调度中发生多轮工具调用，则必须满足：

1. `toolCall` 中每个 assistant tool-call 事件都独立携带自己的 `reasoning_content`
2. 请求体重建后，消息顺序必须严格为：
   - `assistant(tool_calls round 1)`
   - `tool(round 1 result)`
   - `assistant(tool_calls round 2)`
   - `tool(round 2 result)`
   - ...
   - `assistant(final text)`
3. `providerMeta.reasoningStandard` 只对应最后一条 `assistant(final text)`
4. `providerMeta.reasoningStandard` 不表示这一整次调度的总 thinking
5. 不允许把 round 1/2/3 的思维链累计拼接成最终回答思维链

## 8. 必须成立的硬断言

### 8.1 真相层 -> 请求体层

1. assistant 聚合消息拆开后，tool-call assistant 消息数量必须等于 `toolCall` 中 assistant 事件数量
2. tool 消息数量必须等于 `toolCall` 中 tool 事件数量
3. 每个 tool-call assistant 的 `reasoning_content` 必须原样保留
4. 最终 assistant 文本的 `reasoningStandard` 必须出现在最后一条 assistant 请求体消息中
5. 该 `reasoningStandard` 只属于最后一条 assistant 最终答复消息

### 8.2 请求体层 -> 调度缓存层

1. 每次模型新增的 assistant/tool 消息必须按原样进入缓存
2. 缓存中不得丢失任何 `reasoning_content`
3. 缓存中不得提前聚合为单条 assistant 消息

### 8.3 调度缓存层 -> 真相层

1. 完成时必须把缓存折叠回一条 assistant 聚合消息
2. 中止时若已有完整 tool round，也必须按相同规则折叠并持久化
3. 折叠后不得丢失任何 tool round 的 `reasoning_content`
4. 折叠后不得把 tool round 的 `reasoning_content` 写进 `providerMeta.reasoningStandard`
5. 折叠后 `providerMeta.reasoningStandard` 只表示最终答复文本的思维链
6. 落盘时 `toolCall` 每个元素按序拆为工具行（不配对），最终正文作为正文行，公共字段冗余在每行

## 9. 一句话总结

正确模型是：

- `toolCall[*].reasoning_content` 负责"每一轮工具调用前的思维链"
- `providerMeta.reasoningStandard` 负责"最终答复文本的思维链"
- 二者都必须全链路保留，但绝不能混用
- 真相层以 V4 多行组（工具行/正文行/普通消息行）zstd 压缩块存储，解压用 `scripts/unpack_message_blocks.py`
