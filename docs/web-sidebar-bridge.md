# Web/Sidebar 桥接实现说明

本文记录当前 Web 端、VS Code 侧边栏与桌面 App 共用聊天界面的实现方式。这里的 Web 端指用户在浏览器中打开 `http://<主机>:<端口>/sidebar` 后进入的 PAI 聊天界面。

## 核心设计：统一传输适配层

APP 与 Web/VS Code 的传输差异只收敛在 [tauri-api.ts](../src/services/tauri-api.ts)。业务层（composable / 组件）不感知宿主差异，只调用 `invokeTauri` 与 `onTransportNotification`。

### invokeTauri 双运行时分发

`invokeTauri(command, args)`（[tauri-api.ts](../src/services/tauri-api.ts)）：

1. 按 `isTauriRuntimeAvailable()` 判断运行时：Tauri 桌面走原生 `invoke()`，Web/VS Code 走 WebSocket JSON-RPC。
2. 命令名先经 `TRANSPORT_COMMAND_CANONICAL_NAMES` 归一为规范名（legacy 别名 → 当前命令），再按运行时映射为 wire 名。
3. Web 运行时对 `WEB_BRIDGE_NATIVE_ONLY_COMMANDS` 集合内的命令直接拒绝，返回 `WEB_NATIVE_CAPABILITY_UNAVAILABLE`，不发起网络请求。
4. 结果经 `normalizeTransportResult` 按命令做形态归一（如 `workspace.directory.list` 的 directories 字段、`app.bootstrapSnapshot` 的 workspace 合并），保证双端返回同构。

### 流式通道抽象

业务层只依赖 `TransportChannel<T>` 的 `onmessage`（[tauri-api.ts](../src/services/tauri-api.ts)）：

- 桌面运行时：对象就是 Tauri 原生 `Channel`。
- Web 运行时：虚拟通道，流式事件由桥接通知订阅（`chat.assistantDelta` 等）承接，请求参数中剥离原生 Channel 对象。

### 会话与轮次语义禁止按宿主分支

- 会改变会话、轮次或消息状态的运行时流程（发送、停止、队列、引导、压缩、rewind、分支）不得按宿主分支或复制实现。
- 主题、设置按钮、文件路径、能力显隐可以按宿主变化（`data-host` 属性、`WEB_BRIDGE_NATIVE_ONLY_COMMANDS`），但不得改变聊天运行时语义。
- 新增宿主入口必须复用 `main-chat`，不能新建一套聊天状态机。

## 入口与端口

桥接服务由 `src-tauri/src/features/system/commands/ide_context.rs` 启动。默认端口为 `8429`（`default_web_access_port`，[types_config.rs](../src-tauri/src/features/core/domain/types_config.rs)），可通过 `web_access_port` 配置（1024-65535 合法，非法回退默认）。监听地址为 `0.0.0.0`，所以同一台机器、局域网机器以及外部可达网络都可以访问该端口。

配置页的"网络访问"tab 通过 `get_web_access_info` 获取当前状态：

- `localUrl`：本机访问地址，形如 `http://127.0.0.1:<端口>/sidebar`。
- `remoteUrls`：当前识别到的局域网访问地址。
- `remotePassword`：非本机访问 `/chat` 时使用的访问密码。
- `configuredPort` / `port`：配置端口与实际监听端口。

桥接服务同时处理 HTTP 与 WebSocket：

- `GET /`、`GET /sidebar`、`GET /sidebar.html`：返回侧边栏页面。
- `GET /settings`、`GET /settings.html`：返回设置页面。
- `GET /assets/...`：返回前端构建资源（拒绝 `..` 路径穿越）。
- `GET /favicon.ico`、`/favicon.png`：内嵌图标。
- `WS /chat`：Web/侧边栏聊天 JSON-RPC 通道。
- `WS /ide-context`：VS Code 扩展上报 IDE 上下文快照的通道。

HTTP 返回 `sidebar.html` 时会注入 `window.__PAI_SIDEBAR_BRIDGE__`，其中 `chatUrl` 使用当前请求的 `Host` 生成，例如 `ws://192.168.1.10:8429/chat`。因此用户直接在浏览器输入 IP 和端口即可进入同一套聊天界面。

## 访问规则

`/chat` 的访问规则按 TCP peer 地址判断：

- 本机访问：peer IP 是 loopback（`127.0.0.1` / `::1`），直接进入，不需要密码。
- 局域网或外网访问：peer IP 不是 loopback，先收到 `bridge.ready`，其中 `authRequired: true`，前端弹出远程访问密码框。
- 远程密码通过 `auth.login` 校验，校验成功后才允许调用其他聊天 JSON-RPC 方法。
- WebSocket 握手带 Origin 校验（`ide_context_ws_origin_allowed`），不允许的 Origin 返回 403。

远程连接在密码通过前不会加入 `IDE_CONTEXT_CHAT_CLIENTS`，因此不会收到会话广播。登录成功后才注册为可广播客户端。

`/ide-context` 仍使用隐藏 `authToken` 机制给 VS Code 扩展同步上下文快照；这条链路与浏览器远程访问密码分开。

## 前端复用

Web/侧边栏入口不再维护一套独立的低配聊天 UI，而是直接复用主聊天入口：

- [main-sidebar.ts](../src/features/sidebar/main-sidebar.ts) `import "../../main-chat"` 复用主聊天入口，连接与文件选择器等边界能力由 tauri-api 统一处理，不复制聊天状态机。
- 宿主标记：`document.documentElement.setAttribute("data-host", "vscode" | "web")` 在挂载前设置，`sidebar-theme.css` 按 data-host 分流（vscode 跟随 VS Code 主题，web 走应用主题）。
- 主聊天唯一前台运行时收敛在 [use-chat-foreground-runtime.ts](../src/features/chat/composables/use-chat-foreground-runtime.ts)，APP 与 Web 都从 main-chat 进入。

当前 Web/侧边栏支持：

- 左右侧边栏，包括会话、远程联系人、任务和工具审查面板。
- 统一标题栏与窗口模式判断（iframe 嵌入且非 VS Code 宿主时隐藏窗口栏，由远程壳层提供 header）。
- 会话列表、远程联系人列表与任务侧栏切换。
- 新建会话、新建任务、编辑任务、删除任务与任务草稿优化。
- 发送文本、图片和 PDF 附件。
- 工具审查、代码审查、委托状态、上下文压缩、rewind、分支等聊天能力。

Web/侧边栏只保留一个明确限制：不允许从浏览器唤起本机目录选择或打开本机目录（`WEB_BRIDGE_NATIVE_ONLY_COMMANDS` 集合），相关操作会显示 `sidebar.openDirectoryRestricted`。

## 会话占用模型

会话列表状态增加了 viewer 维度：

- `openViewerId`：当前占用该会话的窗口或访问端 ID。
- `currentViewerId`：当前列表请求所属的访问端 ID。

桌面聊天窗口、独立窗口、Web/侧边栏各自使用不同 viewer ID。列表展示时，如果某会话已经被其他 viewer 打开，当前 viewer 会看到禁用态，避免多个普通会话窗口同时编辑同一会话。

系统通知会话是例外：

- 不参与占用冲突。
- 不允许普通发言。
- 保留系统任务、委托入口和通知展示职责。

## 首屏选择

Web/侧边栏启动后不再展示单独的首屏选择 UI。初始化流程会先拉取完整会话列表，然后按以下优先级进入会话：

1. 如果 VS Code/侧边栏带有工作目录，则优先打开匹配该目录的最新普通会话。
2. 否则优先恢复当前访问端记录的最后打开会话。
3. 再否则进入系统通知会话或列表中的第一个可打开会话。

## JSON-RPC 范围

`/chat` 通道使用 JSON-RPC 2.0，分发入口 [jsonrpc_dispatch.rs](../src-tauri/src/features/system/commands/ide_context/jsonrpc_dispatch.rs)。点分方法名以该文件 `match` 臂为准（当前约 139 个，另有一批无点号的兼容别名，不要把两者加总后当产品方法数）。主要方法：

- 会话：`conversation.list`、`conversation.create`、`conversation.createSide`、`conversation.openDraft`、`conversation.updateDraft`、`conversation.blockPage`、`conversation.messageById`、`conversation.messagesBefore`、`conversation.messagesAfterAsync`、`conversation.rename`、`conversation.pin`、`conversation.markRead`、`conversation.archive`、`conversation.batchArchive`、`conversation.delete`、`conversation.rewind`、`conversation.rewindPreview`、`conversation.branchFromMessage`、`conversation.branchFromSelection`、`conversation.compact`、`conversation.compactPreview`、`conversation.fastRequestTurns`、`conversation.autoPush`、`conversation.rebindRecipient`、`conversation.forwardSelection`、`conversation.forwardRemoteContact`、`conversation.changedSince`、`conversation.freshnessSnapshot`、`conversation.runtimeSnapshot`、`conversation.setActive`、`conversation.resumeSubscription`、`conversation.streamProbe`、`conversation.foregroundLightSnapshot`
- 发送/调度：`chat.send`、`chat.stop`、`chat.queueAttachment`、`chat.queueMarkGuided`、`chat.queueRecall`、`chat.queueSnapshot`、`chat.sessionStateSnapshot`
- 归档：`archives.list`、`archives.blockPage`、`archives.unarchive`、`archives.delete`、`archives.export`
- 任务/委托/目标：`task.list`、`task.create`、`task.update`、`task.delete`、`task.dispatchNow`、`task.optimizeDraft`、`delegate.submit`、`delegate.statuses`、`delegate.blockPage`、`delegate.delete`、`delegate.abort`、`goal.create`、`goal.cancel`、`goal.current`
- 模型：`model.list`、`model.select`
- 工作区：`workspace.permission`、`workspace.permission.select`、`workspace.ensureHostRoot`、`workspace.list`、`workspace.directory.list`、`workspace.gitRootCheck`、`workspace.layout.save`
- 文件阅读：`fileReader.directory.list`、`fileReader.readFile`、`fileReader.readFileBlock`
- 工具审查：`toolReview.*`
- git 面板：`git_panel_*`（统一收敛到 `git_panel_dispatch`）
- 配置迁移：`configMigration.preview`、`configMigration.apply`、`configMigration.export`
- 其他：`bridge.ping`、`app.bootstrapSnapshot`、`ideContext.query`、`ideContext.upsert`、`transport.accessInfo`、`prompt.preview`、`prompt.systemPreview`、`terminalApproval.*`

后端广播使用同一 WebSocket 下发，例如：

- `bridge.ready`、`bridge.shutdown`
- `conversation.overviewUpdated`、`conversation.overviewItemUpdated`、`conversation.messageAppended`、`conversation.runtimeStateUpdated`、`conversation.workStatus`、`conversation.todosUpdated`、`conversation.delegateStatusUpdated`、`conversation.goalUpdated`、`conversation.pinUpdated`
- `chat.roundStarted`、`chat.roundFinished`、`chat.roundFailed`、`chat.assistantDelta`、`chat.streamProbeAck`、`chat.streamRebindRequired`、`chat.historyFlushed`、`chat.rewindCompleted`
- `agentWork.started`、`agentWork.stopped`
- `codeReview.requested`、`config.updated`、`ideContext.updated`

## 验证点

本次实现相关的最小验证项：

- `pnpm typecheck`
- `cargo check --manifest-path src-tauri/Cargo.toml`（workdir 固定仓库根）
- `cargo test --manifest-path src-tauri/Cargo.toml ide_context_tests`（workdir 固定仓库根）
