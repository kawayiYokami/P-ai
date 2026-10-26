# 聊天通信、调度与前台状态机

> 本文记录当前实现的唯一边界。最近核对时间：2026-10-26。传输差异仍只允许收敛在 `src/services/tauri-api.ts`。

## 1. 不可破坏的设计目标

聊天业务只有一个通信入口：`src/services/tauri-api.ts`。

```text
Vue 组件 / composable
        │
        ▼
invokeTauri()、onTransportNotification()、TransportChannel
        │
        ▼
统一传输适配器（桌面 IPC 或 Web JSON-RPC）
        │
        ▼
Rust 调度器与持久化会话
```

除适配器内部外，业务代码不得：

- 导入 `@tauri-apps/*`、创建 `WebSocket` 或调用宿主桥；
- 读取 Tauri window label、运行时探针或 VS Code API；
- 为 App/Web 复制消息状态机、流式订阅、恢复判断或命令参数；
- 通过 `postMessage`、直接事件监听或自定义 RPC 绕过适配器。

App/Web 只允许影响产品明确的能力差异：按钮是否显示，以及文件选择/本机文件能力。聊天消息、字号、主题、状态、错误、队列、恢复和调度语义必须相同。

边界由 `tests/transport-boundary.spec.ts` 持续检查。

## 2. 三层事实

### 2.1 持久化消息真相

会话服务负责正式消息、消息 ID、附件、工具调用和 metadata。assistant 轮次开始前，后端先创建真实 `assistantMessageId`；前端流式投影不能自行伪造另一条消息。

### 2.2 后端运行态

调度器按 `conversationId` 保存：

- `idle`、`assistant_streaming`、`organizing_context` 等运行态；
- `isProcessing`、待处理队列计数；
- 当前轮次的 `streamCache`（activation、request、assistant ID、文本、块、工具状态和 revision）。

运行态快照是轻量读取，不应被前端的连接状态覆盖。

### 2.3 前端投影

`useChatFlow` 将正式消息、队列态、流式块、工具态和错误投影到视图。流式 metadata 是投影，不是持久化真相；正式消息到达后必须按 ID 覆盖并清理投影。

## 3. 统一传输适配器

### 3.1 调用与命令名

业务使用可读的协议方法名，例如：

- `conversation.archive`；
- `conversation.compact`；
- `conversation.foregroundLightSnapshot`；
- `conversation.runtimeSnapshot`、`conversation.resumeSubscription`、`conversation.streamProbe`。

适配器在桌面端把方法名映射到 Tauri command，在 Web 端发送同名 JSON-RPC；参数包装和返回值归一化也只在适配器完成。业务不再维护 snake_case/native 与 Web 方法两套调用。

### 3.2 流式通道

`TransportChannel<T>` 是两端共用的最小通道接口：

- 桌面端内部持有真实 Tauri `Channel<T>`；
- Web 端内部把 `chat.assistantDelta`、`chat.roundStarted`、`chat.roundFinished` 等通知转发到虚拟通道。

`bindTransportConversationStream`、`unbindTransportConversationStream` 和 `probeTransportConversationStream` 是唯一的绑定生命周期入口。每个 binding 有 binding ID、会话 ID和顺序保护；新绑定失败时保留仍健康的旧绑定，解绑竞态不能恢复过期 binding。

探针必须收到当前会话的回环事件才算健康：RPC 返回成功本身不等于流式通道可用。

### 3.3 通知

业务通过 `onTransportNotification(method, handler)` 接收统一事件。适配器负责把桌面事件和 Web JSON-RPC 通知归一化为相同的事件名和 payload；业务只按会话、轮次和 generation 过滤。

## 4. 共享聊天状态机

所有聊天前台（桌面窗口、Web 页面、VS Code 侧栏）共用：

- `useChatFlow`：发送、停止、队列、Channel binding、delta/round 事件和最终收口；
- `chat-message-state-machine`：按消息 ID 合并正式消息与流式投影；
- `useConversationViewRuntime`：视图级状态和历史分页；
- `reconcileForegroundConversation`：前台恢复协调；
- `decideForegroundRecovery`：平台无关的纯恢复决策；
- `useChatRuntime` 与 `useConversationMaintenanceDialog`：归档、压缩、删除预览和执行。

侧栏只是宿主布局和能力注入层，使用同一 `ChatView`、`ChatComposerPanel`、消息状态机、维护对话框和运行时 composable。不得重新引入侧栏专用流式适配器、恢复判断或压缩命令。

### 4.1 轮次生命周期

```text
sendChat
  → 适配器调用统一命令
  → 后端写入 user/队列消息
  → historyFlushed
  → roundStarted（真实 assistant ID 已建立）
  → assistantDelta / 工具事件
  → roundFinished 或 roundFailed
  → 正式消息按 ID 合并，清理 streaming 投影
```

事件可能丢失、重复、乱序或晚到。处理规则：

1. 会话 ID、assistant message ID、activation/request ID和前端 generation 必须匹配；
2. 有 `streamCache` 时按快照覆盖，不能把同一 delta 再拼一次；
3. 正式完成消息优先于残留 stream cache；
4. 旧会话、旧轮次和旧 Channel 事件直接丢弃；
5. 不得因为前端暂时没有 delta 就把后端运行态改成 idle。

## 5. 前台恢复

恢复先对账事实，再选择最小动作：

```text
runtime snapshot
  → assistant message ID
  → activation/request
  → stream revision
  → transport probe
  → resume stream / refresh target / reload conversation
```

共享决策动作的含义：

| 动作 | 含义 |
| --- | --- |
| `keep` | 投影和通道可信，不刷新消息 |
| `probe_stream` | 双方认为仍在流式，验证当前 binding |
| `resume_stream` | 恢复当前目标流或订阅映射 |
| `refresh_target_message` | 只按 ID读取完成的目标消息 |
| `reload_conversation` | 轻量路径缺少必要身份时的最后兜底 |

`keep` 不能调用完整会话打开；目标消息已知时不能整读历史。恢复失败不能用默认数据或重新提交用户输入掩盖。

## 6. 允许的能力差异

`getTransportCapabilities()` 只暴露产品明确需要的能力：

- `nativeWindowControls`：窗口控制按钮；
- `nativeFilePicker`：文件选择按钮；
- `localFileSystem`：本机文件操作入口。

这些能力只能用于按钮显隐、文件选择和本机文件操作。字体、字号、主题、消息布局、流式状态、错误文案和调度结果不得依赖能力值。

## 7. 读取与性能边界

- 消息读取默认使用 metadata、recent snapshot、block page 或 message-by-id；
- 前台恢复优先轻量读取，只有身份无法安全建立时才完整打开会话；
- 新增消息功能前必须说明为什么轻量读取不成立；
- 归档/压缩预览只读取当前需要的块页，不直接整读 `Conversation.messages`。

## 8. 代码索引

### 传输适配器

- `src/services/tauri-api.ts`
- `src/services/tauri-api.spec.ts`
- `tests/transport-boundary.spec.ts`

### 共享聊天前台

- `src/features/chat/composables/use-chat-flow.ts`
- `src/features/chat/composables/use-chat-flow-channel-binding.ts`
- `src/features/chat/composables/chat-message-state-machine.ts`
- `src/features/chat/composables/chat-foreground-coordinator.ts`
- `src/features/chat/composables/foreground-recovery-decision.ts`
- `src/features/chat/composables/use-chat-runtime.ts`
- `src/features/chat/composables/use-conversation-maintenance-dialog.ts`
- `src/features/chat/components/dialogs/ConversationMaintenanceDialog.vue`

### 相关验证

```text
pnpm typecheck
pnpm vitest run src/services/tauri-api.spec.ts tests/transport-boundary.spec.ts
pnpm vitest run src/features/chat/composables/use-chat-flow-channel-binding.spec.ts
```

任何新增的 App/Web 分支都必须先证明它属于按钮或文件能力，否则视为错误实现。
