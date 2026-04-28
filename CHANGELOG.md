# 变更日志

## 进行中

- 修复（chat-queue-recall-full-text）：队列消息撤回回填输入框时改用后端返回的完整消息原文，不再使用 UI 裁剪后的 `messagePreview`，并保持撤回成功后才回填，避免假撤回错觉。
- 修复（terminal-git-read-whitelist-write-flags）：终端只读白名单收紧 `git branch`、`git tag`、`git config` 参数判定，删除/移动/复制分支、删除/创建/强制标签以及写入 Git 配置等形式不再被误判为只读命令。
- 修复（task-skip-advance-next-run）：任务调度遇到会话忙碌、队列非空、并发上限、会话不可解析或同轮同会话冲突等跳过场景时，统一走原子跳过方法，同步更新 `last_triggered_at_utc` 并写入运行日志，确保重复任务会推进下次触发时间而不是卡在已到期状态。
- 修复（tool-review-department-and-json-fallback）：生成审查报告弹窗新增审查部门选择，默认当前会话部门并展示对应供应商/模型；代码审查委托按所选部门调度。终端只读 Git 白名单补齐常见读取命令并在权限拦截返回中提示只读白名单不受目录授权限制；审查报告前端解析支持从 Markdown 文本与 ```json 代码块中提取 JSON 结构化渲染。
- 修复（chat-queue-conversation-scope-and-dedup）：聊天输入区队列展示改为只显示当前会话队列；用户消息写入 `requestId` 并在调度入队前识别同请求重复事件，避免切换/重试路径把已落地消息再次排队。
- 修复（tool-review-code-review-skill-and-terminal-read-whitelist）：内置 code-review 预设 skill 并由工具审查包装层固定注入 JSON 协议，避免用户可改 skill 破坏机器消费结构；终端只读白名单补充纯输出命令与诊断反馈，权限不足时拆分提示白名单/非白名单指令；同时收紧审查提示，未确认真实缺陷时必须返回空 findings。
- 功能（file-reader-window）：本地代码与 Markdown 文件链接新增独立只读阅读窗口，支持多标签页、Shiki 直出高亮、行号、Markdown 渲染、地址栏面包屑与默认应用打开；文件阅读窗口拆分最小 Tauri capability，并补齐隐藏文件识别、标签缓存释放与打开失败提示。
- 修复（chat-code-block-header-theme-color）：代码块标题栏文字与操作按钮改为使用主题文本色，避免在自定义主题下仍显示固定灰蓝色。
- 优化（chat-conversation-list-compact-title-width）：会话列表简约模式不再在标题行显示部门/工作区标签，释放标题可用宽度，减少标题过早裁剪。
- 修复（tools-no-session-id-and-apply-patch-diagnostics）：工具对外返回不再暴露 `sessionId`；统一 apply_patch 格式口径，补强补丁 header、hunk 前缀、上下文不匹配与上下文多处命中的诊断反馈，并拒绝不唯一上下文以避免误改。
- 修复（tool-review-copy-finding-location）：复制工具审查报告中的选中意见时补齐文件路径与行号，避免只复制标题和正文后缺少定位信息。

## 发布：v0.9.54

- 发布（release-0.9.54）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.54`，纳入本轮会话列表简约模式、上下文压缩计划块空行修复、README 致谢与能力描述调整，以及流式期间督工入口优化。

- 优化（chat-supervision-menu-during-streaming）：聊天流式/忙碌期间允许打开工具栏“更多”并发起督工任务，不可用的分支、工作目录与独立窗口操作改为隐藏；督工弹窗新增按语言预填的“持续推进计划”模板。
- 修复（context-compaction-active-plans-spacing）：上下文压缩消息在摘要正文追加执行中计划时，保留计划块前的空行，避免 `<active_plans>` 与摘要正文挤在同一段。
- 优化（chat-conversation-list-compact-mode）：会话列表新增详细/简约图标切换；简约模式将首行收口为标题与工作空间，第二行仅显示最新一条消息，并隐藏原双行预览。

## 发布：v0.9.53

- 发布（release-0.9.53）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.53`，纳入本轮远程联系人“本轮绑定联系人”路由统一、聊天底部工具栏左右边距修复，以及流式/忙碌期间附件粘贴与拖拽入队放开。

- 修复（chat-toolbar-padding-and-busy-attachment-paste）：聊天历史区底部工具栏补齐左右边距；流式/忙碌期间不再拦截附件粘贴与拖拽入队，仅在强制归档期间继续禁止，允许用户在助理回复中提前准备下一条附件输入。
- 修复（remote-im-bound-contact-round-routing）：远程联系人轮次新增“本轮绑定联系人”运行时语义；联系人工具注入改为只看本轮是否已绑定唯一联系人，不再依赖联系人会话；`contact_reply` / `contact_send_files` 改为按当前轮次绑定联系人解析目标；轮末自动外发与提示词说明统一收口到“本轮绑定联系人”，并补强 `contact_send_files.file_paths` 必须传真实本地文件路径的约束；压缩续跑与 follow-up 路径同步继承该绑定联系人。

## 发布：v0.9.52

- 发布（release-0.9.52）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.52`，纳入本轮未归档会话索引同步、message store 新格式写入收口、chat_index 脏数据清理、新建会话概览推送与远程联系人忙碌待办状态修复。

- 修复（remote-im-busy-pending-no-failure）：远程联系人在场且当前会话忙碌时，新入站消息只标记待办并等待当前轮次收尾续跑，不再立即触发重复激活，避免“当前最后一条消息来自助理自身，无需重复激活”被误报为请求失败。
- 修复（chat-unarchived-conversation-index-sync）：未归档会话新建与索引同步链路改为直接 upsert `chat_index`，新建后由后端主动推送最新会话概览，前端补齐概览事件监听；同时根除新建/全量写入回落旧会话 JSON 的路径，清理孤儿索引项，并在摘要不一致但真实会话仍可见时修正索引而非删除会话，避免新建会话或摘要变更后列表需刷新才出现、迁移门禁被误触发或会话永久从未归档列表消失。

## 发布：v0.9.51

- 发布（release-0.9.51）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.51`，纳入本轮计划确认续调度激活、百炼 Coding 多模态 base64 回退、运行态拆分旧测试修复、终端会话绑定与审查报告列表样式收口。

- 修复（aliyun-coding-multimodal-base64-fallback）：百炼 Coding 地址不再触发多模态临时 URL 上传，统一保留/解析为 base64 请求；百炼通用地址上传失败或 API Key 缺失时也回退 base64 继续发送，避免 URL 预处理失败直接中断候选模型。
- 修复（chat-plan-confirm-active-plans-and-activation）：计划确认状态从聊天消息流剥离到会话级 `active_plans.jsonl`；用户同意计划时由后端原子写入进行中计划、按需压缩上下文并创建 `activateAssistant=true` 的续调度事件，LLM 调用 `plan complete` 时完成最新进行中计划，摘要与请求上下文统一注入全部进行中计划；同时为 `round_started/round_completed/round_failed` 补齐 activation/request 关联字段，前端按激活事件创建草稿并在撤回、计划确认前清理流式运行态，避免压缩后不续跑或旧草稿残影冒泡。
- 修复（terminal-current-session-and-check-whitelist）：终端与补丁工具移除对外 `session_id` 入参，统一使用当前聊天会话绑定的工作区，避免模型自造会话 ID 后绕开会话级 full_access 配置退回系统工作区；同时将 `pnpm typecheck`、`cargo check/test`、`vue-tsc`、`tsc` 等只读校验命令纳入本地读取白名单。
- 修复（chat-message-list-edge-padding）：聊天消息列表去掉横向滚动容器内边距，仅保留纵向间距，避免头像被列表 padding 与消息行 padding 叠加推离窗口边缘。
- 功能（tool-review-code-review-reports）：代码审查链路接入独立委托会话与每会话审查报告 JSONL 流；报告生成先落 `pending` 草稿，再更新为 `success` / `failed`，前端侧边栏拆分“工具解释 / 审查报告”并支持原始 JSON 解析渲染 Findings、风险等级彩色点、勾选复制/附加与报告重新生成。
- 重构（delegate-approval-guard-hashset）：委托线程工具审批免弹窗机制从 `AtomicBool` 改为 `HashSet<String>`（存委托 conversation_id）。进入时 insert、离开时 remove，审批链路通过 `!table.is_empty()` 判断是否有活跃委托，消除并发竞态（A 离开误将 B 的标记抹掉）。
- 文档（readme-rewrite）：重写 README.md，突出项目世界领先设计与独立开发背景。新增 Release badge、双语 punchline、组织系统说明与关键数据（1698 提交/98 版本/50+ 计划），将能力跃迁拆为六大深度小节（工具治理、部门委派、长期记忆、工作区感知、远程 IM、高并发架构），重构典型工作流为日常/开发/长期/远程四场景，并补充对比表与快速开始。

## 发布：v0.9.50

- 发布（release-0.9.50）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.50`，纳入本轮聊天流式轮次边界、工具评估侧边栏分组与审查报告操作优化。
- 优化（chat-round-started-event-boundary）：聊天流式事件新增 `round_started` 作为助理轮次启动信号，`history_flushed` 收口为只表达“消息已落入正式历史”，前端不再依赖历史刷写事件推进等待态；同时抽取外部聊天事件 payload 序列化辅助函数并补充失败日志，避免 JSON 序列化异常被静默吞掉
- 修复（guided-queue-after-tool-loop-close）：引导消息检查点移动到工具执行完成后的续调度分叉；命中引导时先闭合当前工具轮次并保留 assistant/tool 历史，再由外层引导队列插入消息并重开调度，避免在助理激活前提前消费引导消息或伪造助手文本污染历史
- 优化（chat-assistant-draft-loading-dots）：助理草稿已创建但首段文本、推理或工具状态尚未到达时，聊天气泡显示三点 loading，占住回复位置并提供明确等待反馈；真实内容到达后自动切回原有 markdown / 工具状态展示，不写入后端消息
- 优化（tool-review-grouped-display）：工具评估侧边栏改为按终端与补丁分组展示，终端项直接显示命令，补丁项按真实工作目录路径压缩后的文件路径分组并显示新建/修改/删除语义，卡片标题补齐文件名且状态胶囊保持最小宽度；同时将“审查”状态口径统一调整为“评估”，避免工具名与绝对路径噪音干扰改动审阅
- 优化（tool-review-report-actions）：审查报告按钮区改为“无报告时生成、生成中禁用、生成完成后仅自动弹出一次、有报告时查看”的状态机，并在报告弹窗左下角新增复制与附加到输入面板按钮，复用现有附件队列将报告作为 markdown 附件发送

## 发布：v0.9.48

- 修复（manual-compaction-should-not-switch-foreground-conversation）：手动压缩沿用对话维护链路时，不再误消费归档语义下的 `activeConversationId` 切换前台会话；压缩完成后的消息刷新固定回到发起压缩的原会话，避免手动压缩后被错误跳回主会话
- 发布（release-0.9.48）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.48`，纳入本轮“手动压缩后保持当前会话”的修复
## 发布：v0.9.47

- 修复（foreground-snapshot-readback-sync）：前台轻量快照在标记会话已读后，立即回填该会话最新的运行态与 Todo 真值；前端清除会话角标时也同步把未归档列表中的 `unreadCount` 归零，避免切入会话后红点已清但列表运行态或未读数仍短暂残留旧值
- 修复（chat-stop-follows-round-completed-source-of-truth）：聊天停止链路收口为“后端持久化停止结果并主动发出 round_completed，前端按普通完成事件统一收尾”的单一真源；前端 stop 成功后不再本地重复 finalize 或额外 reload，避免停止成功与外部完成事件并发时出现双收尾、状态闪烁或草稿被提前清空的问题
- 优化（terminal-exec-response-slimming-and-chat-stream-debug-gate）：终端执行结果收口为前端当前真实使用的最小返回字段，不再重复携带 shell 路径、会话目录和审查上下文等大块调试信息；同时前台聊天流式重绑相关日志改为默认关闭的调试开关，减少正常使用时的控制台噪音，保留排查时按需开启的能力
- 修复（startup-deputy-department-agent-self-check）：启动期新增副手部门人格自检；若内置副手部门或副手部门被错误写成默认人格，会在启动时自动修正回副手人格并写回配置，避免聊天发送阶段因为“部门未分配该人格”直接报错；同时发送链路补充更明确的中文错误提示，直接指出异常部门与人格，方便用户回到设置页修正
- 修复（chat-streaming-draft-preserve-during-tool-phase）：聊天流式草稿在工具阶段切会话、重绑或请求失败时，优先保留当前已见内容与工具调用上下文，不再因为展示态被清空而让草稿瞬间消失；恢复显示时会尽量复用已有草稿正文与流式缓存，减少工具执行中断后的空白闪烁与内容回退
- 修复（tool-loop-provider-serial-round-scope-and-review-fallback-copywriting）：GenAI 工具循环把供应商级串行锁收窄到单轮真实模型请求，避免整段工具续调期间长期占锁拖慢同供应商后续请求；同时终端审查模型不可用时不再误提示“已降级为本地规则审查”，而是明确告知“已跳过自动审查，请直接确认是否允许执行”，防止用户对当前审批依据产生误判
- 修复（chat-stream-freeze-cache-preserve）：聊天前台轮次在流式输出中被打断或切换时，不再提前清空当前会话的流式展示缓存；改为先把已生成的展示态同步回会话缓存，再结束当前轮次，修复中断后已出现的内容瞬间消失、切回会话又需要重新等待回填的问题
- 功能（chat-conversation-terminal-approval-panel）：终端审批主入口从全局模态改为当前会话输入区内的审批面板，按会话归属展示待审批项，切走会话时不再抢占别的聊天；审批项可直接逐条批准/驳回并查看命令或补丁详情，同时复用现有审查详情弹窗承载大段预览，避免全局弹窗打断当前阅读
- 优化（chat-todo-floating-intent-label-and-report-closure-guide）：聊天窗口顶部悬浮待办条改为显示“{人格名} 打算{当前待办}”，直接暴露当前人格准备推进的下一步，降低用户把阶段汇报误判为卡住的概率；同时 `todo` 工具指南补充“最后一条通常应为向用户汇报结果的收尾步骤、汇报前必须先把已完成步骤和收尾待办划为 completed、阶段进展不要伪装成最终完成”的约束，推动 todo 状态与汇报时机保持闭合
- 修复（chat-unread-backend-truth-source）：未归档会话未读数改由后端基于 `active_chat_view_bindings` 统一判定与维护；只要任一活跃聊天窗口正在查看目标会话，消息落库、助手追加与历史刷写等链路就不再继续累计未读，并在命中活跃视图时清理残留未读；前端移除本地 unread 自增/清零推测逻辑，改为只消费后端返回的 overview/snapshot 真值，修复当前窗口或独立聊天窗口正在阅读时仍被错误增加未读的问题
- 修复（terminal-approval-smart-review-allow-pass-through）：终端审查链路改为收缩本地 Git 硬拦截范围，普通 `git commit/pull/fetch/merge/rebase/checkout/switch/restore/stash/apply` 不再在本地规则层直接拒绝；`approval` 目录下的 `shell_exec` 智能审查提示词补齐“白名单外但明显无副作用”判定方法，测试/检查/编译校验命令与仅输出到终端的 `curl/wget/Invoke-WebRequest` 可由模型直接返回 `allow=true` 放行，而写文件、下载落盘、重定向写文件、管道执行脚本与无法确认副作用的命令返回 `allow=false`；同时终端后处理改为在 `approval` 下尊重模型 `allow=true` 结果，避免模型已判安全后仍再次弹用户审批
- 发布（release-0.9.47）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.47`，纳入本轮会话内终端审批、流式停止收口、未读真值同步、副手部门启动自检与终端执行返回瘦身等更新


## 发布：v0.9.46

- 修复（deepseek-kimi-reasoning-tool-history）：DeepSeek/Kimi 工具调用链路将 assistant `tool_calls` 对应的 `reasoning_content` 写入会话工具历史，并在会话 JSON -> 抽象 `PreparedHistoryMessage` -> genai 请求体的全过程中原样传递；带 `tool_calls` 的 assistant 历史消息不参与相邻 assistant 合并，避免思维链错位、被拼接或被清洗
- 修复（chat-model-picker-deepseek-kimi）：聊天输入区模型下拉的文本模型白名单补充 `deepseek/kimi`，避免设置页已配置 DeepSeek/Kimi 协议后发送栏仍无法选择
- 修复（mcp-windows-process-tree-cleanup）：Windows 下 stdio MCP 连接新增 Job Object 进程树托管，应用退出、断开连接或异常崩溃后会随句柄关闭一起回收整棵 MCP 子进程树，避免 `cmd /C -> npx/node` 链残留后台驻留
- 发布（release-0.9.46）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.46`，纳入本轮 DeepSeek/Kimi reasoning 传递修复与 Windows MCP 进程树清理修复

## 发布：v0.9.45

- 功能（deepseek-kimi-protocol-and-reasoning-roundtrip）：API 供应商新增 `DeepSeek / Kimi` 协议选项与常见供应商预设，按 OpenAI-compatible 链路接入聊天、工具调用、视觉描述、模型列表刷新、推理网关与部门模型过滤；DeepSeek/Kimi 工具调用续轮会捕获并回传 assistant `reasoning_content`，即使为空也保留字段，避免工具调用后的 thinking-mode 上下文丢失
- 优化（deepseek-kimi-protocol-warning）：模型卡片在非 `DeepSeek / Kimi` 协议下检测到模型名包含 DeepSeek/Kimi 时显示警告提示，避免用户误用普通 OpenAI 协议；不自构造 DeepSeek thinking 请求体，也不在 UI 暴露 genai 当前未支持的 DeepSeek thinking 等级，降低后续维护风险
- 发布（release-0.9.45）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.45`，纳入本轮 DeepSeek/Kimi 协议与 reasoning 回传收口

## 发布：v0.9.43

- 优化（chat-tool-running-indicator）：聊天消息工具摘要行在工具执行中时改用成功色 DaisyUI loading 指示器覆盖原绿点展示，完成后恢复普通成功色圆点，避免执行中与已完成状态观感混淆且不撑开布局
- 优化（chat-runtime-status-and-unread-indicators）：聊天调度状态改为按会话广播工作中/错误状态，侧边栏头像使用 DaisyUI 角标显示工作中绿点与错误红点，成功后自动清除；标题栏会话列表按钮改为未读红点提示，未读数从旧 `last_read_message_id` 游标推导改为 `unread_count` 计数器，联系人会话不参与未读统计，前台拉取最新消息成功即清零
- 优化（chat-single-user-flush-fast-path）：`history_flushed` 只包含单条本地用户正式消息时，前端直接用正式消息替换乐观草稿，不再进入全量消息合并路径，降低发送后“草稿变正式消息”的显示延迟
- 修复（remote-im-media-conversation-scoped-storage）：远程 IM 入站图片/音频在接收边界即外置化到 `llm-workspace/downloads/<conversation_id>/`，消息正文只保留 `@download:` 引用；消息块迁移/写入前增加残留 base64 附件清洗兜底，旧引用不搬迁，新落盘不再混入全局媒体池
- 功能（conversation-block-message-store-migration）：会话消息存储新增目录型 JSONL 仓库与启动期迁移门禁，底层 shard 读写在保持业务调用方不感知的前提下支持 `Conversation -> ConversationBlock -> Message` 三层布局；迁移生成 `blocks/*.jsonl` 与最小 `messages.idx.json`，索引仅保留 `messageId / blockId / offset / byteLen`，联系人会话按压缩边界或本地 04:00 日界线切块，旧 block 自动瘦身工具/MCP/运行态挂件但保留文本和外置/URL 媒体引用
- 修复（apply-patch-full-access-review-bypass）：`apply_patch` 的工具智能审查改为仅在工作目录安全等级为“审批”时触发；命中“完全”权限的目录按本地授权直接执行补丁，避免已授予完全访问后仍反复弹出审查确认
- 优化（tool-loop-copy-slimming）：工具调度循环改为移动消费工具调用字段与工具历史事件，避免重复 clone 工具参数、调用 ID 与结果文本，降低超大工具参数/历史事件在调度热路径上的内存压力

## 发布：v0.9.42

- 功能（chat-queue-guided-handoff）：队列消息新增“引导中”状态与前端引导按钮；当前轮次不被立即打断，而是在自然返回后先完成历史落盘，再由引导队列立即接管下一轮；引导消息在下一轮进入忙碌态后立刻从队列移除，不再依赖激活最终成功，避免已写入历史的引导消息因后续失败残留在队列中
- 优化（chat-queue-guided-ui-and-recall-boundary）：聊天输入区队列列表收口为更轻量的满宽列表样式，每条消息最前面新增“排队 / 引导中”状态标签；普通队列消息仍可退回输入框，引导消息一旦成立则不可退回，队列侧不再暴露误导性的删除语义
- 优化（chat-status-banner-full-width-rect）：聊天区顶部“已停止当前请求”等状态横幅改为直角铺满样式，移除原有胶囊圆角观感，让中断/报错提示在窗口顶部更直接稳定
- 发布（release-0.9.42）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.42`，作为本轮“引导投送、队列状态标签与状态横幅样式收口”的发布版本

## 发布：v0.9.41

- 优化（department-deputy-permission-and-reset-polish）：部门页收紧副手与权限配置交互，主部门隐藏副手开关，顶部回转按钮改为还原未保存部门草稿；副手部门默认只暴露调查型内置工具并隐藏预设 Skill，保留用户自建 Skill 与 MCP 按配置可用，同时统一人格下拉的用户/预设/普通排序与副手人格候选过滤
- 功能（explicit-deputy-department-delegation）：部门新增“副手部门”显式开关；只有副手部门会出现在委托候选并能通过后端委托校验，副手部门人格与非副手部门互斥，内置不可删除的副手人格作为默认兜底，避免普通部门人格被误用为委托执行者
- 优化（runtime-log-code-block-view）：后台运行日志窗口从逐条虚拟列表改为单个 `pre/code` 日志块展示，保留级别/模块过滤与复制能力，提高日志扫描与复制的连续性
- 优化（llm-round-log-capacity-setting）：调用日志缓存数量改为可配置三档 `1 / 3 / 10`，默认保留最近 3 条；后端按当前配置裁剪内存中的调用日志展示缓存，设置页日志页可直接切换档位
- 优化（workspace-agents-md-size-guard）：会话主工作目录的 `AGENTS.md` 自动注入新增 32KiB 大小上限；超过上限时仅记录带路径与字节数的跳过日志，不再读取正文，避免超大项目说明进入提示词上下文
- 优化（welcome-completion-progress-theme-color）：设置首页欢迎卡的配置完成度圆环改为跟随主题主色，保留 DaisyUI `radial-progress` 默认结构，避免中性色在自定义深色主题下显得脏灰或与背景层级冲突
- 优化（generated-dark-theme-base-swap）：自定义深色主题保持现有色阶生成算法不变，仅在生成后对调 `base-100` 与 `base-300`，让最深背景落到 `base-300`，同时保留亮色自定义主题与预设主题行为
- 优化（conversation-read-agent-runtime-snapshot）：会话读取层的 active snapshot / active messages / 历史前后分页 fallback 分支不再通过 `data.clone()` 合并整份运行数据来解析有效人格，改为仅复制 agents 小快照并合并私有组织；会话索引与消息页仍基于原 `AppData` 处理，保留 active fallback 创建/归一化语义，同时减少大消息数据在读取路径上的额外复制
- 优化（conversation-read-and-scheduler-copy-slimming）：归档与远程 IM 联系人会话的纯读接口改为借用缓存中的 `AppData` 直接提取目标摘要/消息，不再为列表或消息读取 clone 整份运行数据；聊天调度提交阶段改为消费临时 prepared batch，消息落库与 history-flushed 推送只保留必要的一次消息副本，继续降低大消息批次出队时的内存峰值
- 优化（chat-provider-meta-move-on-write）：聊天发送与调度链路中多处 `provider_meta` / `tool_history_events` 写回改为 move-on-write，避免为了追加记忆命中、远程 IM 决策或图片外置标记而复制整块原始 JSON；助手消息落库也改为移动大字段进最终消息，仅保留必要的一次消息副本用于返回与自动发送
- 优化（archive-and-dispatch-clone-slimming）：压缩/归档、聊天调度与工具调用热路径继续削减大对象 clone，归档报告会话改为按需借用、请求体与原始 JSON 生命周期收窄、调用日志避免复制整轮请求重字段；同时将归档/压缩记忆归属修正为严格从会话所属部门的唯一人格解析，拒绝消息 hint / fallback 人格投票，并为任务 target_scope 降级补充带上下文的诊断日志，降低超长消息与原始请求体导致线程栈溢出的风险
- 优化（chat-foreground-snapshot-and-tool-review-fast-read-path）：前台轻量快照与工具审查批次读取这两条高频读链继续切到只读快路径，命中 `conversationId` 时统一直接借用缓存中的目标会话引用，不再经由整份 `AppData` clone 后再切片，进一步把切换会话与工具审查面板首读压到近乎瞬时
- 优化（chat-history-page-fast-read-path）：会话补历史/补后续消息这条高频读取链路新增只读快路径；当已明确传入 `conversationId` 时，不再 clone 整份 `AppData` 再切局部消息页，而是直接借用缓存中的会话引用，只克隆目标分页消息并继续做最小 media materialize，显著压低补消息热路径的锁持有与读取耗时
- 修复（archive-list-title-source-correction）：归档消息列表标题改为优先使用会话自身 `title`，仅在标题为空时才回退到首条用户消息预览，避免归档窗口左侧列表把正文预览误当标题展示
- 优化（chat-rewind-lightweight-preview-and-agent-check-removal）：撤回消息链路移除与当前历史回滚无关的 agent 存活检查、private org 合并与 overview 全量刷新依赖，未归档会话预览读取改为纯读且瘦身重字段；撤回成功后前端仅在本地已握有足够最近正式消息时才局部推送当前会话预览，否则不做额外读取或刷新，显著压低撤回耗时
- 优化（chat-virtual-scroll-compensation-and-own-message-alignment）：聊天虚拟滚动继续收口动态高度与补历史补偿语义，首屏快照与历史分页保持前端可控条数，消息统一补稳定 id，向上补历史改为以 `scrollHeight` 差值做主补偿、消息头像锚点做兜底，向上滚动时旧项测量优先吃缓存避免二次顶动；同时恢复“用户消息正式落库后再平滑顶到上缘”的行为，保留群组渲染与工具栏同层布局，修复长消息历史补位、用户消息对齐与滚动稳定性问题
- 优化（chat-dynamic-scroller-migration）：聊天虚拟滚动底座切换到 `vue-virtual-scroller` 默认 `DynamicScroller / DynamicScrollerItem` 方案，移除自研虚拟列表 composable 与 TanStack 虚拟滚动依赖，统一由库接管动态高度测量、item 池化与滚动区渲染
- 功能（provider-serial-request-gate）：供应商设置新增“允许并发请求”开关，默认关闭；后端按 `providerId` 维护全局异步串行门，默认将同一供应商下全部模型请求串行化，并把该约束下沉到真实模型调用层，覆盖主聊天、工具循环、视觉链路、tool review 与归档摘要等请求入口；配置变更仅影响后续请求，不追溯影响当前已开始的请求
- 重构（conversation-service-boundary-and-writeback-foundation）：建立 `ConversationService` 目录化模块边界，收口会话快照、分页、单条消息、tool review、remote IM、归档与高频写路径读取语义；会话持久化补齐 write-back 基础设施与锁顺序修正，工具审查、前台轻量快照、会话分页等热路径进一步统一走服务和内存缓存，并补全文档中的 persistence worker、`write_mode`、并发冲突与异常回归清单
- 优化（chat-foreground-windowing-and-review-refresh）：聊天前台切换改为直接从底部最新窗口进入，首屏轻量快照收敛为最近 10 条、向上滚动分页每次补 10 条，并引入动态高度虚拟滚动以降低超长会话渲染卡顿；同时收紧会话/工具审查读取热路径，优先走内存缓存快照，工具审查改为按 messageId 精确刷新单条消息，避免把 review 误当成“补后续消息”
- 功能（codex-rate-limits-and-spark-buckets）：Codex 供应商登录状态卡新增官方 usage 快照查询，复用现有 OAuth/本地凭证链路请求 `/wham/usage`，展示主 Codex 与 GPT-5.3-Codex-Spark 两组 `5h/weekly` 额度、重置时间与 credits 状态，并兼容当前默认 `https://chatgpt.com/backend-api/codex` 基址的 usage 查询归一化
- 重构（compaction-close-and-restart-dispatch）：上下文压缩现在被明确建模为调度边界事件；无论是发送前自动整理还是工具续调前整理，一旦命中压缩，当前调度都会闭口结束，并在压缩完成后以新的 `request_id / dispatch_id` 重开一次调度，不再在同一次调度里跨过压缩消息继续原地续跑；同时统一首发与续调两条链的压缩判定入口，确保真相层、展示层与抽象调度层都一致表现为“消息组 -> 压缩消息 -> 新消息组”
- 修复（prompt-usage-source-unification-and-conversation-cache-removal）：提示词服务的上下文占用查询现在统一以最近一条 assistant 消息的 `providerMeta` 为真实真源，找不到时才回退到本地估算；同时移除 `Conversation.last_context_usage_ratio` 与 `Conversation.last_effective_prompt_tokens` 这组伪状态字段，避免前后端围绕“会话级缓存”和“消息级真实值”继续分叉，强制压缩判断、手动整理上下文预览与相关测试夹具也同步切到同一套真源语义
- 重构（conversation-prompt-service-phase-1）：引入会话提示词服务第一阶段骨架，先收口提示词 owner 与只读 snapshot，不替换 `Conversation.messages` 作为持久化真源；系统提示词最终合成与对话消息投影入口开始统一经过服务层，并新增缓存命中稳定性与 `prompt_revision` 边界测试，确保 `todo/task` 与 `memory_recall` 不会误触发系统提示词 revision
- 重构（conversation-prompt-service-phase-2）：继续收口提示词服务 owner，系统侧块生成统一改为由服务内部发起，并把主聊天、`SummaryContext`、工具安全审查、工具审查提交、vision 描述这几条高频真实业务入口的 latest user / prepared prompt 生成动作收进服务内部；外部主链只再传原始条件与场景意图，不再手搓系统块或 latest user 文本

- 发布（release-0.9.41）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.41`，作为本轮消息 JSONL 外置与压缩段分页重构继续推进前的新版本基线

## 发布：v0.9.40

- 优化（chat-virtual-scroll-compensation-and-own-message-alignment）：聊天虚拟滚动继续收口动态高度与补历史补偿语义，首屏快照与历史分页保持前端可控条数，消息统一补稳定 id，向上补历史改为以 `scrollHeight` 差值做主补偿、消息头像锚点做兜底，向上滚动时旧项测量优先吃缓存避免二次顶动；同时恢复“用户消息正式落库后再平滑顶到上缘”的行为，保留群组渲染与工具栏同层布局，修复长消息历史补位、用户消息对齐与滚动稳定性问题
- 优化（chat-rewind-lightweight-preview-and-agent-check-removal）：撤回消息链路移除与当前历史回滚无关的 agent 存活检查、private org 合并与 overview 全量刷新依赖，未归档会话预览读取改为纯读且瘦身重字段；撤回成功后前端仅在本地已握有足够最近正式消息时才局部推送当前会话预览，否则不做额外读取或刷新，显著压低撤回耗时
- 修复（archive-list-title-source-correction）：归档消息列表标题改为优先使用会话自身 `title`，仅在标题为空时才回退到首条用户消息预览，避免归档窗口左侧列表把正文预览误当标题展示
- 优化（chat-history-page-fast-read-path）：会话补历史/补后续消息这条高频读取链路新增只读快路径；当已明确传入 `conversationId` 时，不再 clone 整份 `AppData` 再切局部消息页，而是直接借用缓存中的会话引用，只克隆目标分页消息并继续做最小 media materialize，显著压低补消息热路径的锁持有与读取耗时
- 优化（chat-foreground-snapshot-and-tool-review-fast-read-path）：前台轻量快照与工具审查批次读取这两条高频读链继续切到只读快路径，命中 `conversationId` 时统一直接借用缓存中的目标会话引用，不再经由整份 `AppData` clone 后再切片，进一步把切换会话与工具审查面板首读压到近乎瞬时
- 发布（release-0.9.40）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.40`，纳入本轮已完成的“虚拟滚动补偿与用户消息对齐收口、撤回链与会话预览瘦身、归档标题修正、补消息/轻量快照/工具审查快路径”等更新

## 发布：v0.9.39

- 优化（chat-virtual-scroll-windowing-and-snapshot-throttle）：聊天窗口虚拟滚动切回更贴近现有成熟实现的 `@tanstack/vue-virtual` 结构，去掉 active turn 自动对齐与相关高频观察器，补齐稳定 `getItemKey`、只针对当前虚拟窗口取首个可见锚点、折叠态思维链/工具卡懒渲染、代码块与布局观察器延后一帧更新，并将首次切屏与向上补历史的消息页大小都收敛到 2 条，显著降低长消息会话中的滚动抖动、连续补历史和上滚卡顿
- 发布（release-0.9.39）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.39`，纳入本轮已完成的“聊天虚拟滚动降重、前端控制首屏快照条数与历史分页收紧”等更新

## 发布：v0.9.38

- 优化（chat-dynamic-scroller-migration）：聊天虚拟滚动底座切换到 `vue-virtual-scroller` 默认 `DynamicScroller / DynamicScrollerItem` 方案，移除自研虚拟列表 composable 与 TanStack 虚拟滚动依赖，统一由库接管动态高度测量、item 池化与滚动区渲染；同时将会话切换与“滚到底”锚定收口到 scroller 自身暴露的方法，修正 `ChatView` 模板层级，避免 `0.9.37` 中出现的长会话滚动异常
- 发布（release-0.9.38）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.38`，纳入本轮已完成的“聊天虚拟滚动底座迁移、滚动恢复可用与布局层级修正”等更新

## 发布：v0.9.37

- 优化（chat-virtual-list-scroll-stability-and-render-keys）：聊天虚拟滚动收口为稳定 item key 渲染，去掉基于渲染区间的整段 subtree 重建；无 id 消息补稳定前端 renderId，向上滚动时仅对完全位于视口上方的高度修正做滚动补偿，并为加载更早历史补充锚点恢复，减少长会话中的闪烁、跳动与滚动抢夺
- 发布（release-0.9.37）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.37`，纳入本轮已完成的“聊天虚拟滚动稳定性收口、稳定 render key 与历史锚点恢复”等更新

## 发布：v0.9.36

- 修复（chat-send-ingress-and-dispatch-stack-overflow）：发送链路从收到前端 `send_chat_message` 到调度结束这段，去掉了目标会话解析阶段的前置快速持久化与整份 `Conversation/agents` 按值返回；发送入口与用户@委托发送改为先完成 ingress 再触发后台调度，mention 计划直接基于当前 `AppData` 借用构造，不再额外背整份会话快照；调度层同步移除 `agents.clone()` 与 flush 后整会话 `clone()`，降低发送前准备与调度边界上的主线程/运行态栈压力，修复 release 新会话首发即闪退的问题
- 发布（release-0.9.36）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.36`，纳入本轮已完成的“发送前准备链与调度链爆栈修复、release 诊断收口与版本发布”更新

## 发布：v0.9.35

- 功能（parameterized-theme-generator-and-mode-presets）：主题系统新增“预设 / 自定义”双入口，支持以少量参数即时合成并预览自定义主题；自定义浅色与自定义深色各自独立缓存参数，切换明暗不会互相覆盖，并恢复与预设主题并存的切换能力；同时聊天消息、工具审查、标题栏与外观页样式联动改为跟随新的主题 token，补齐中英繁文案、运行时动态注入与相关前端测试
- 发布（release-0.9.35）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.35`，纳入本轮已完成的“参数化主题生成、自定义主题明暗双缓存、主题页预设/自定义切换与样式收口”等更新

## 发布：v0.9.30

- 优化（chat-selection-branch-action-copywriting）：聊天窗口多选操作栏中的“会话分支”按钮文案调整为“创造会话分支”，让当前动作语义更直接

## 发布：v0.9.21

- 优化（chat-message-action-layout-tighten）：聊天消息最外层行间距与上下内边距进一步收紧，减少相邻消息之间的空白；同时把用户消息的撤回按钮移到气泡下方右侧，并去掉操作区额外顶部间距，让消息气泡与操作按钮整体更紧凑
- 功能（chat-conversation-list-search-filter）：未归档会话侧边栏与会话切换弹层新增本地搜索过滤，支持按会话标题和最近两条预览消息筛选结果，并补齐中英繁搜索占位与空态文案
- 优化（chat-conversation-pin-action-icon-clarify）：会话列表与会话切换弹层的置顶按钮图标改按“动作”表达语义，未置顶会话显示 `Pin`、已置顶会话显示 `PinOff`，主会话不再显示置顶按钮，避免把按钮误读成状态徽标
- 优化（chat-conversation-list-spacing-tighten）：收紧聊天窗口会话列表与会话切换弹层的左右留白、分组间距和条目内部 padding/gap，让列表整体更紧凑，减少无效留白
- 修复（terminal-git-read-whitelist-and-local-rule-review）：终端只读白名单补齐 PowerShell 下常见只读 `git` 子命令（如 `status/diff/show/log`），避免 `git diff` 这类纯读命令被误送 AI 审查；同时对 `git pull/push/fetch/commit/merge/rebase/reset/checkout/clean/stash/apply` 等明确危险命令改为本地规则直接拦截，不再重复走 AI 审查，并统一返回带本地化说明的 `toolReview.kind=local_rule` 结果，其中 `git push/pull --force/-f` 会作为特别高危单独提示
- 修复（chat-todo-sticky-hit-area）：聊天窗口顶部当前待办条的 sticky 外层容器不再拦截整行点击，点击命中范围收回到中间待办按钮本体，避免下方左右两侧按钮被遮挡后无法点击
- 修正（json-only-default-streaming）：撤回“`json_only` 默认强制非流式”的行为，恢复为默认沿用流式请求，仅在已有流式降级缓存命中时才切到非流式；避免部分上游要求 `stream=true` 时因错误强制非流式触发 `400 Bad Request`
- 修正（streaming-fallback-predicate-tightening）：默认仍优先走流式，但自动降级到非流式的判定条件已收紧为“请求体 / 请求 schema 格式异常”，不再把流式响应解析异常、超时、5xx 等其他错误误判成应当禁用流式
- 优化（tokio-worker-stack-and-llm-log-slimming）：Tauri 后端异步运行时改为显式使用 8MB Tokio worker 栈，缓解超大模型请求在主进程 worker 上执行时更容易触发栈溢出的风险；同时调用日志移除完整 `request` 持久化，不再为每轮模型请求额外保留一整份请求 JSON，仅继续保留计时、headers、工具信息、原始响应、错误与聚合轮次信息，降低大上下文场景下日志对象本身的内存占用与链路负担；工具安全审查、工具审查提交、归档摘要与主聊天轮次的模型调用日志也改为在外层调用点统一组装，不再由共享推理入口在内部直接落日志；`chat_pipeline` 聚合主卡现在会优先从 pending round buffer 读取最近一轮真实 `headers/tools`，避免聚合改造后回退到兜底值
- 修复（json-only-force-non-stream-across-openai-responses-anthropic）：`json_only` 模型请求现在会统一强制走非流式，不再只在 OpenAI Chat 路径生效；同时为 `OpenAIResponses/Codex` 与 `Anthropic` 补齐真正的非流式调用实现，避免工具安全审查、上下文整理与归档摘要这类只需要完整 JSON 结果的请求误走流式或名义非流式、实际仍落回流式链路
- 发布（release-0.9.21）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.21`，纳入本轮已完成的“配置窗口内联更新日志卡片、远程 changelog 代理读取修正、多语言补齐与流式跨工具分段相关脏逻辑清理”等更新
- 优化（request-log-pipeline-aggregation-and-ui）：设置页“调用日志”改为以调度 `chat_pipeline` 为主语展示，单次调度内的多轮 `chat` 会先在内存中暂存并在 pipeline 收口时聚合进同一条主日志；主列表仅按最近 10 次调度裁剪，同时日志页改成“调度主卡 + 轮次摘要列表 + 轮次详情弹层”，不再把多次工具轮转平铺成多张同级卡片
- 优化（prompt-dirty-split-system-source-environment）：提示词缓存的脏标记拆分为“系统源变化”和“系统环境变化”两类，部门/人格配置与 shell workspace / remote IM 等会话环境不再共用同一种重建原因；同时补充回归测试，锁定系统源 dirty 不会误伤会话环境缓存、系统环境 dirty 不会误伤部门系统缓存
- 优化（chat-header-context-ring-uniform-style）：聊天窗口顶部上下文进度环改为统一样式，只通过环长度表达占用比例；移除基于阈值的颜色/光效变化，并统一 tooltip 与环本体使用同一归一化百分比，避免 hover 显示与环的视觉表现不一致
- 修复（chat-stream-text-boundary-delta-after-tool-turn）：后端工具循环在进入后续正文轮次时，会在该轮正文首个流式文本包前额外补发一个 `\n` delta，让聊天窗口流式阶段也能对齐正式消息的文本边界；避免工具后的下一段正文继续黏在上一段末尾，同时不引入前端猜边界或额外渲染分支
- 修复（terminal-readonly-powershell-rg-whitelist）：终端只读白名单现在允许 PowerShell 场景下的 `Set-Location/cd` 作为纯切目录辅助动作，并将 `rg` / `findstr` 识别为可放行的只读搜索命令；修复只读工作区内 `Set-Location ...; rg -n ...` 这类纯读命令被误判为非白名单而拦截的问题

## 更新：配置窗口内联更新日志卡片

- 功能（config-inline-remote-changelog-card）：配置窗口左上角新增“更新日志”入口，点击后会先弹出占据窗口约 90% 的应用内卡片，再懒加载远程 `CHANGELOG.md` 并直接按 Markdown 渲染展示，支持关闭按钮、遮罩点击与 `Esc` 关闭
- 修复（remote-changelog-raw-proxy-fallback）：远程更新日志读取链路避开 `api.github.com/contents` 经代理时可能出现的响应体解码异常，改为通过 `gh-proxy -> edgeone.gh-proxy -> hk.gh-proxy` 读取 `raw.githubusercontent.com` 原始 Markdown 文本，并在更新检查弹窗中优先复用最新 changelog 摘要
- 优化（inline-changelog-i18n）：补齐内联更新日志卡片的中英繁文案，统一走 i18n，避免按钮标题、加载态与空态继续硬编码

## 更新：会话分支与转发到会话口径统一

- 重构（chat-branch-and-forward-wording-unification）：项目内原“派生”统一更名为“会话分支”，原“投送”统一更名为“转发到会话”；同步收口前后端命令名、输入输出结构、忙态字段、事件名、状态提示、多语言文案与测试标题，避免同一功能在字段、英文名与中文口径上继续混用旧术语

## 更新：压缩上下文构造与压缩入口收口

- 修复（summary-context-preserve-full-history-and-department-context）：`SummaryContext` 在归档与上下文压缩场景下，继续完整复用正常聊天消息组建，不再篡改历史工具调用记录；同时恢复传入当前可用 `agents/departments`，避免压缩模型丢失当前会话部门上下文
- 优化（summary-context-json-only-tool-disabled）：正式归档与上下文压缩两种 SummaryContext 提示词现在都会明确声明“你的工具都已经被禁用，你只能生成 JSON 完成任务”，限制当前摘要模型的能力边界，但不修改原始消息历史
- 修复（compaction-ui-only-soft-threshold）：压缩“会话较短 / 占用较低”的建议性门槛统一收回到前端 UI，仅作为提示展示；后端压缩入口只保留真正的硬条件拦截，避免手动压缩时再被二次报错挡住
- 优化（compaction-banner-info-tone）：聊天窗口中“正在压缩上下文”状态条改为 `info` 语义色，和普通状态、错误状态更容易区分

## 更新：归档与压缩摘要提示词明确禁用工具

- 修复（archive-summary-json-only-tool-disabled）：正式归档与上下文压缩两种 SummaryContext 提示词现在都会明确声明“你的工具都已经被禁用，你只能生成 JSON 完成任务”，避免摘要模型在归档/压缩阶段继续尝试走工具链

## 更新：流式正文在多次工具调用之间保留分段

- 修复（chat-streaming-text-paragraph-break-between-tool-rounds）：聊天窗口中的流式助理草稿在一次调度内跨多次工具调用继续发言时，不再把后续发言直接黏成同一段；当工具开始新一轮执行后，下一段流式正文首包会自动补上段落分隔，保持流式观感与最终落库消息更接近

## 发布：v0.9.20

- 发布（release-0.9.20）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.20`，纳入本轮已完成的“会话置顶分组、主会话常驻置顶、图钉快速切换与分割线分组展示”等更新

## 更新：会话置顶分组与图钉快速切换

- 功能（chat-conversation-pin-grouping）：未归档会话列表与顶栏会话列表新增“置顶 / 其他”两组，主会话始终视为置顶；置顶会话按后置顶的更靠前排序，非置顶会话按最后消息时间倒序排列，图钉按钮放到时间右侧，并统一用分割线展示两组边界
- 优化（chat-conversation-pin-fast-toggle）：切换会话置顶改为只写运行态中的 `pinned_conversation_ids`，并通过轻量事件 `easy-call:conversation-pin-updated` 让前端本地 patch 当前会话的置顶状态与顺序；不再为了更新图钉去重建整份会话概览或重新拉取全量列表

## 更新：主会话禁止归档与删除

- 修复（main-conversation-archive-and-delete-locked）：主会话现在明确禁止归档与删除；后端 `force_archive_current`、归档预览与 `delete_unarchived_conversation` 会直接拦截主会话，前端归档弹窗、归档窗口删除按钮与删除动作也同步禁用或给出提示，避免主会话被误删/误归档后丢失入口

## 更新：分享导出头像与聊天布局修正

- 修复（share-export-avatar-side-and-data-url）：分享 HTML 与图片导出的聊天布局改为真正使用左右两侧头像列；修正 `chat-end` 时用户头像错误出现在左侧的问题，并复用前端现成的用户头像与人格头像 data URL，导出页优先显示真实头像，仅在缺失时退回首字占位
- 优化（share-export-shell-and-chat-layout）：移除分享导出页最外层多余边框，继续收口为更接近 DaisyUI 聊天组件的简洁结构，保持“思维链 -> 工具 -> 正文”的固定聚合顺序，同时图片导出继续基于这份简化 HTML 渲染，不再依赖原对话窗复杂结构

## 更新：流式首包重绑与工具消息聚合顺序修正

- 修复（chat-stream-start-rebind-without-touching-main-dispatch）：首个可见流式包现在只会额外触发一次 `stream_start` 重绑事件，不再额外复制首包 delta，也不再中断原本的正常调度链，避免 `todo` 等工具调用后的首包分发被重复投递或提前截断
- 修复（chat-tool-block-before-final-text）：带工具调用历史的助理消息在聊天窗口中改为固定聚合顺序“思维链 -> 工具 -> 正文”，同时移除正文被工具块互斥吞掉的旧模板结构，确保这类消息既能看到工具过程，也能稳定看到最终正文

## 发布：v0.9.19

- 发布（release-0.9.19）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.19`，纳入本轮已完成的“页面缩放热键开放、配置页缩放布局修复、对话窗口切换模型改为替换首项、内置主题配色替换”等更新

## 更新：替换若干内置主题配色

- 优化（theme-palette-refresh）：替换内置 `dark`、`winter`、`autumn`、`night`、`lofi` 主题的 DaisyUI 配色参数与圆角/边框风格，使深浅主题的视觉方向更统一，并为后续继续批量替换主题保留同名入口

## 更新：对话窗口切换模型改为替换首项

- 修复（chat-model-switch-replace-primary-candidate）：在对话窗口切换当前部门模型时，不再将新模型头插到候选列表前面并保留旧主模型；现在会直接替换候选列表首项，并同步移除后续重复项，避免列表不断积累历史主模型

## 更新：开放页面缩放热键并修复配置页缩放布局

- 功能（webview-zoom-hotkeys-enabled）：应用内放开 WebView 缩放热键，支持 `Ctrl/Command + 滚轮`、`Ctrl/Command + + / - / 0` 直接缩放当前页面，便于快速放大界面而无需额外设计单独的外观模式入口
- 修复（config-tabs-zoom-layout-alignment）：修复 Shell 工作空间、技能页与远程渠道页在页面放大后头部按钮、下拉框与开关容易被挤压、错位或裁切的问题；这些区域统一改为可换行、可收缩、带最小宽度的布局，缩放后仍能保持按钮和开关完整可见

## 更新：常见请求错误改为友好提示并保留原始报错

- 优化（chat-friendly-request-errors-with-raw-detail）：常见 LLM 请求错误（如 429 限流、503 服务不可用、401/403 鉴权与权限问题、网络失败、请求超时、模型不存在、上下文超限、余额不足、维护中）现在会优先显示本地化友好说明，并在后面保留原始错误信息，避免直接把抽象错误码原样抛给用户；同时 `CHAT_ABORTED_BY_USER` 改为直接显示“已停止当前请求”

## 更新：流式状态草稿与滚动体验收口

- 重构（chat-streaming-status-and-draft-projection）：重做流式阶段的前后端联动状态显示；发送后会立刻生成助理预流式草稿，先显示“正在消息准备中 / 正在等待回应中”等状态，再在正文、思维链或工具事件真正到达后切入正式流式内容；同时将流式缓存与前端草稿投影分离，确保会话切换回来或草稿丢失后仍能按事件恢复显示
- 优化（chat-streaming-bubble-status-ui）：助理草稿完成前不再显示名字与时间，消息头改为直接显示流式状态；预流式阶段不再出现空白白气泡，正文进入前仅保留轻量等待态，完成后恢复正常名字与时间排版；同时去掉助理气泡入场闪动，收紧名字/时间/状态字的字号、透明度与对齐表现
- 优化（chat-scroll-large-distance-no-smooth）：聊天区滚动策略新增“超过一页不平滑”规则；无论是对齐当前轮次到顶部，还是用户主动点击下滑到底部，跨越过长距离时都会直接跳转，避免十几页历史被强行平滑滚动导致卡顿

## 更新：多选消息分享图片与 HTML

- 功能（chat-selection-share-image-and-html）：聊天窗口的多选操作条补齐真实分享能力；点击“分享”后不再提示暂不支持，而是弹出分享方式对话框，允许将已选消息导出为长图 PNG 或独立静态 HTML 历史页
- 功能（share-export-static-html-and-png）：新增前端分享渲染链路与后端文件写出命令；HTML 分享会生成不依赖应用运行时的单文件静态历史页，图片分享会将同一份静态内容渲染为 PNG 长图并保存到本地
- 修复（share-export-hardening-and-logs）：补齐分享导出链路的本地化错误提示、路径跳转防护与中文运行日志；单张图片读取失败不再拖垮整批分享，导出命令会记录目标路径、字节大小与耗时，便于诊断导出失败

## 更新：检查更新代理与后台预下载

- 优化（updater-proxy-fallback-and-ready-to-restart）：检查更新、manifest 下载与更新包下载统一接入 `gh-proxy -> edgeone.gh-proxy -> hk.gh-proxy` 三段降级链路，每个地址最多重试 3 次，总计最多 9 次尝试；同时 release workflow 生成的 updater manifest 资产 URL 也切到代理地址，避免远端构建产物仍回落直连 GitHub
- 重构（updater-background-prepare-and-apply）：更新流程拆成“下载并准备更新”和“应用已准备更新”两阶段；自动检查更新时改为后台静默预下载，下载完成后左上角动作切换为“更新并重启”；手动检查更新时保持前台进度展示，不再静默下载

## 更新：归档窗口 UI 收口与预览标题修正

- 优化（archives-window-ui-polish）：重排归档窗口布局为“顶部控制栏 + 左侧列表栏 + 右侧内容栏”，顶部模式切换改为 `tabs-border`，移除右侧冗余标题栏，列表时间字号收小，联系人列表收成双行显示，整体间距与结构更接近真正的侧边栏内容窗口
- 修复（archive-preview-title-skip-system-compaction）：归档列表与未归档会话列表生成预览标题时，跳过由系统人格写入且 `role=user` 的“上下文整理”伪用户消息，重新回到真正的用户第一句话，避免列表标题错误显示为 `[上下文整理]`
- 优化（memory-text-wrap-polish）：顺手统一记忆页与记忆导入预览中的长文本换行样式，避免英文长词和路径在卡片内撑破布局
- 修复（archives-user-speaker-label）：归档窗口中的用户消息说话人标签不再显示 `user/用户`，改为复用当前用户人格名称

## 更新：会话持久化改为定向写入与索引增量更新

- 优化（conversation-persistence-targeted-write-paths）：除全量归档导入外，移除各类读接口、联系人配置更新、任务分发、委托回退、归档整理与未归档会话操作中对 `persist_app_data_conversation_runtime_delta(...)` 的误用；这些路径现在分别改为单会话写入、定向会话写入或仅运行态写入，不再为局部变更触发整份会话列表序列化比较
- 优化（chat-index-incremental-update）：单会话与定向删改路径的 `chat_index` 改为按会话条目增量 `upsert / remove`，不再每次基于全部 `conversations` 重建整个索引文件；纯运行态变更继续只写 `runtime_state`，避免无意义触碰会话文件与聊天索引
- 优化（archive-dialog-preview-frontend-only）：归档操作弹窗的预览改为前端本地计算，直接复用当前会话消息、会话概览运行态与最近一次上下文占用信息；打开弹窗时不再额外调用后端 `preview_force_archive_current / preview_force_compact_current`，避免重复读取会话与两次 `conversation_lock` 持有

## 更新：切换对话模型不再自动刷新当前会话历史

- 修复（chat-model-switch-should-not-auto-reload-history）：移除聊天窗口中基于 `activeChatApiConfigId` 的自动消息刷新 watch；切换前景部门模型时不再自动重读当前会话历史，避免违反“由显性保存动作驱动刷新”的约定，也避免无意义触发后端 `get_active_conversation_messages` 重路径与会话锁耗时

## 发布：v0.9.18

- 发布（release-0.9.18）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.18`，纳入本轮已完成的“Shell 工作区提示词修正、助理私人目录命名统一、聊天气泡入场动画收口、撤回 Todo 恢复”等稳定性修复

## 更新：Shell 工作区提示词与目录命名统一

- 修复（shell-workspace-prompt-alignment）：系统提示词中的 `<shell workspace>` 不再只显示系统级目录；现在会按当前会话直接列出助理私人目录、Shell 默认启动/执行目录以及当前允许的全部工作目录，避免前端已保存的主目录和附加目录在系统提示词预览里缺失
- 修复（assistant-private-workspace-labeling）：对外文案统一将原“系统工作目录 / 系统目录 / system workspace”收口为“助理私人目录 / Assistant Private”，仅调整用户可见命名，不变更内部存储字段与工作区 level/id

## 更新：聊天流式草稿不再触发历史气泡重播动画

- 修复（chat-history-bubble-enter-animation-regression）：聊天区消息气泡的入场动画不再无条件挂在所有消息节点上；现在只对乐观用户草稿与助理流式草稿启用入场动画，避免助理流式草稿出现时历史消息因补流 patch 重新挂载而整屏再次闪动

## 更新：撤回会话时恢复 Todo 状态

- 修复（rewind-conversation-restore-todos）：会话撤回不再只截断消息；现在会从剩余消息里回溯最近一次 `todo` 工具调用并恢复对应 Todo 状态，若找不到历史 Todo 则清空当前 Todo；同时补发 `conversation-todos-updated` 与会话概览更新事件，确保前端撤回后立刻看到正确的 Todo 面板状态

## 更新：主工作目录 Git 幽灵快照撤回骨架

- 功能（main-workspace-git-ghost-snapshot-demo）：为主工作目录 Git 幽灵快照撤回补齐实验性骨架，拆出独立模块 `git_ghost_snapshot.rs`，打通“用户消息附带快照记录 / 撤回优先尝试 Git 恢复 / 非 Git 或失败自动降级”的主链路，并补充定向测试验证 provider_meta 记录与工作区恢复流程
- 调整（main-workspace-git-ghost-snapshot-disabled）：当前版本明确不实装 Git 幽灵快照能力，运行时无条件跳过创建与恢复，仅保留后续研究所需的最小骨架，不影响现有撤回逻辑
- 结论（main-workspace-git-ghost-snapshot-not-adopted）：暂不采用 Git 幽灵快照作为正式撤回方案，原因是其会额外消耗性能、适用范围不够广、存在错误撤回用户后续手动更改的风险，并且当前没有观察到其相较现有工具回滚链路具有显著更好的实际效果

## 发布：v0.9.17

- 发布（release-0.9.17）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.17`，纳入本轮“删除当前未归档会话时优先切换到相邻会话”的交互修复

## 更新：删除当前会话时立即切换相邻会话

- 修复（delete-conversation-optimistic-switch）：删除或丢弃当前未归档会话时，前端现在会先按当前列表乐观切换到相邻会话，不再等待后端删除完成后才切走；若当前不存在相邻会话，则先清空前景并在删除完成后用后端返回的 `activeConversationId` 兜底恢复，避免“删当前会话后停在空白态”或切换明显滞后

## 更新：记忆备份导入导出弹窗重构

- 重构（memory-backup-import-export-dialogs）：记忆页的导入导出入口改为两个独立组件与独立弹窗；列表标题右侧保留小图标按钮，但交互改为“先打开弹窗，再选择文件或勾选记忆域”，不再一点击就直接选文件
- 重构（memory-backup-export-scope-selection）：新增导出记忆域预览能力，导出前会先读取当前内部记忆域并允许勾选目标作用域，后端导出链路按所选 `scopes` 过滤生成统一三表结构 JSON
- 修复（memory-backup-import-export-cleanup）：移除前端 `Angel` 专项命名与旧文案残留，导入/导出成功提示统一回到记忆页外层展示；同时补齐记忆导入链路的中文错误信息与跳过日志，便于排查敏感内容或标签归一化导致的省略记录

## 更新：系统准则补充删除前确认

- 修复（system-rules-delete-confirmation）：系统准则新增“删除前确认”；当需要删除文件或目录时，必须先明确说明删除范围、删除目的与可能影响，并取得用户明确指示后才能执行，禁止因主观判断“看起来无用”而自行删除

## 发布：v0.9.16

- 发布（release-0.9.16）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.16`，纳入本轮已完成的“工具类型重做、系统工具目录全量展示、部门 skill 依赖 exec 前端限制、系统人格默认名调整”等收口

## 更新：工具类型重做与系统工具目录收口

- 重构（tool-type-rework-and-contact-tools）：重做系统工具类型边界；固定系统工具收口为 `todo / remember / recall`，本地会话固定工具收口为 `plan`，联系人专用工具重做为 `contact_reply / contact_send_files / contact_no_reply`，仅在联系人会话中挂载且不再受部门权限控制
- 重构（remote-im-contact-tools-and-auto-send）：废除旧的 `remote_im_send` 联系人决策协议，联系人会话改为“中途回应 / 发附件 / 明确不回复”三工具模型；若未调用 `contact_no_reply`，系统会在轮次结束时自动向当前联系人发送最终 assistant 回复
- 重构（command-tool-split）：删除旧 `command` 与 `help`，将其拆分为三个独立内置工具：`reload`、`organize_context`、`wait`；默认工具目录、前端工具定义、旧配置归一化与运行时装配同步收口到新协议
- 修复（system-tool-catalog-show-all-tools）：工具页的系统工具目录改为真正展示全量系统工具，不再显示“当前部门 / 当前人格 / 运行模型”等运行态信息；联系人专用工具现在也会出现在目录中，方便统一查看说明、参数与示例
- 修复（department-skill-permission-requires-exec-ui）：部门权限页中，技能权限现在依赖终端 `exec`；当白名单未允许 `exec` 或黑名单禁用了 `exec` 时，所有 skill 选项会自动灰掉并清空已有勾选，避免出现“没有终端却还能选 skill”的前端状态
- 修复（system-persona-default-name）：内置系统人格默认名从 `凯瑟琳` 调整为 `pai system`，同步对齐依赖默认名称的测试断言

## 更新：系统提示词管理与部门缓存收口

- 重构（system-prompt-manager-and-cache）：新增 `PromptManager`，将 `system prompt` 与 `conversation prompt` 解耦；系统提示词按“固定系统准则 / 部门提示词 / 内部工具规则 / 特殊运行环境 / IM 规则”分层装配，并分别引入部门提示词缓存、会话环境提示词缓存与最终系统提示词缓存，避免高频轮次重复重建
- 修复（system-prompt-cache-invalidation-whitelist）：系统提示词缓存失效机制改为显性入口白名单；移除发送链路和底层配置写入中的自动标脏，只保留“人格保存、部门保存、工具/skill 手动刷新或 reload、用户保存 AI 可访问目录”四类入口触发重建标记，非用户操作不再误伤 system prompt 成品缓存
- 修复（remote-im-two-no-reply-leave-away）：远程联系人状态机新增连续 `no_reply` 计数；同一联系人连续两轮都由 LLM 决定“不回复”时，现在会在第二轮结束后立即切回离场，`send / send_async / 异步发送成功 / away -> present` 会重置计数，避免联系人在长时间连续不回应时仍错误保持在场
- 修复（system-prompt-order-and-tag-alignment）：系统提示词组件顺序改为显式 stage 输出，不再混用“先拼一坨再追加”；同时统一核心 XML 标签为英文，如 `<system rules>`、`<persona settings>`、`<admin user settings>`、`<role constraints>`、`<conversation style>`、`<language settings>`，并去掉块间多余空行
- 修复（department-tool-rules-only-follow-department）：系统提示词中的工具规则不再受已废止的 API 工具开关影响；当前内置工具说明仅按部门权限判断是否注入，`plan` 规则从固定系统准则移入工具规则层，普通会话也不再误注入远程联系人 IM 规则
- 优化（skill-and-terminal-prompt-cache）：隐藏技能快照改为启动预热和显式刷新时更新，发送主链路只读内存缓存；终端运行环境块改为复用缓存配置并缓存工作区规范化结果，不再每轮重复读配置和 `canonicalize()` 多个工作目录
- 优化（prompt-build-timing-breakdown）：细化聊天主链路提示词计时日志，新增技能快照、AGENTS 注入、Todo 指南、IM 运行块、任务板、附件提示与终端环境块等阶段打点，便于后续继续定位真实热点
- 修复（startup-lock-contention-and-preview-stability）：去掉启动期 MCP 重部署与微信轮询中的无意义 `conversation_lock` 争抢，降低配置窗口白屏概率；同时补齐 `PromptManager` poison 恢复日志上下文，输出完整异常信息便于排障

## 发布：v0.9.15

- 发布（release-0.9.15）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.15`，纳入本轮已验证通过的“部门权限白黑名单与工具页去人格化”相关实现与交互收口

## 更新：部门权限白黑名单与工具页去人格化

- 功能（department-permission-control-and-tools-page-readonly）：部门配置新增 `permissionControl` 草稿字段，支持白名单 / 黑名单两种机制，并分别按名称存储 `builtinToolNames / skillNames / mcpToolNames`；运行时装配阶段会统一按部门权限过滤内置工具、Skill 快照与 MCP 工具，名称即使与当前目录不匹配也允许保留存储，只在“实际可发现项”上做白名单保留或黑名单排除
- 功能（department-permission-catalog-and-ui）：后端新增部门权限目录查询能力，统一下发内置工具、技能与 MCP 工具的名称与说明；部门页新增权限控制卡，支持草稿编辑、黑白名单切换、模式说明、说明截断、自定义选中态样式，以及“恢复初始化”时同步重置默认权限配置
- 重构（tools-page-system-catalog-only）：工具页不再按人格切换或绑定工具开关，改为只读的系统工具展示页；工具可用性从“人格 tools 配置”切换为“系统默认工具 + 部门权限”联合决定，前端工具页文案同步收口为“部门权限到部门去修改”
- 优化（config-sticky-layout-and-department-draft-mode）：抽出 `SettingsStickyLayout` 公共固定头部容器，统一供应商页与部门页的“顶部固定、下方滚动”布局；部门页改为显式保存的草稿模式，字段编辑、模型候选与权限勾选都先落本地草稿，点击保存后才真正写回配置

## 更新：远程客服部门强制启用回复决策工具

- 修复（remote-customer-service-force-remote-im-send）：远程客服部门下的 `remote_im_send` 不再被误判为“部门不允许”或“当前人格未启用”；现在该工具在远程客服部门中按业务语义强制可用，工具状态页会明确显示“远程客服部门已强制启用远程联系人回复决策工具（支持 list/send/no_reply）”，并在前端禁用对这个开关的误操作切换，避免出现“办事指南要求必须使用，但工具面板显示被禁用”的自相矛盾

## 更新：远程客服内置部门与默认办事指南

- 修复（remote-customer-service-built-in-defaults）：废除内置“前台”部门，新增内置“远程客服”部门，并将远程联系人默认绑定部门改为 `remote-customer-service-department`；旧配置里已经存在的 `front-desk-department` 不做删除或迁移，仅取消其内置部门身份，避免误动用户历史数据
- 修复（remote-customer-service-guide-and-restore）：远程客服的默认概述与默认办事指南改为独立模块维护，不再使用占位文案；前端系统部门操作统一收口为“还原”，现在点击“还原”会把 `远程客服 / 助理部门 / 副手` 恢复为各自内置默认名称、概述与办事指南
- 修复（remote-im-send-guide-alignment）：`remote_im_send` 工具说明与参数说明对齐远程客服办事指南中的最终裁决口径，明确联系人消息必须通过 `list / send / no_reply` 完成决策，且 `no_reply` 的 7 秒等待与“不刷新上次成功回复时间”语义与默认指南保持一致

## 更新：远程联系人发送权限提示收口

- 修复（remote-im-send-permission-guard-polish）：`remote_im_send` 的发送权限提示按联系人开关语义收口：当联系人禁止发信时，工具会明确告知模型“仍可继续处理任务，但任务结束后应立刻使用 `remote_im_send(action=no_reply, status=done)` 结束本轮”；当联系人禁止发送文件时，仅拦截非图片文件，图片不再算作“文件禁止”范围

## 更新：远程联系人状态机文档

- 文档（remote-im-state-machine-doc）：新增远程联系人状态机说明文档，使用业务语言梳理“离场 / 在场 / 忙碌 / 空闲 / 待办 / 上次成功回复时间”的含义，并补充 Mermaid 状态图、自动离场条件、待办续跑逻辑，以及“新消息从进入到处理完成”的完整流程说明，便于后续继续讨论产品语义而不是反查代码

## 更新：远程联系人待办续跑与 no_reply 冷却

- 修复（remote-im-pending-follow-up-and-no-reply-cooldown）：远程联系人在忙碌期间收到新消息后，`has_pending` 不再只是保留一个挂起标志；当前轮次结束后若存在待办，会立刻自动续跑下一轮处理。与此同时，`no_reply` 现在内置固定 `7` 秒冷却，语义改为“发呆 7 秒后结束本轮”，并且不会刷新“上次成功回复时间”，避免把明确不回复误算成一次真实回复

## 更新：远程联系人终局工具排序

- 修复（remote-im-send-done-tail-reorder）：当同一批工具调用里同时存在其他工具与 `remote_im_send(status=done)` 时，现在会自动把这个“终局发送”后置到本批最后，保持其他工具原顺序不变，避免模型先吐出 `done` 提前截断后续工具链

## 更新：远程联系人列表手动刷新

- 修复（remote-im-contact-list-manual-refresh）：远程 IM 配置页联系人列表不再每 3 秒自动轮询刷新；当前只保留页面首次进入、登录/同步/保存联系人配置后的主动刷新，以及顶部手动刷新按钮，避免打开联系人设置时后台持续触发 `remote_im_list_contacts` 与无意义会话锁日志

## 更新：远程联系人耐心离场

- 修复（remote-im-patience-leave-on-inbound）：远程联系人在 `keyword` 激活模式下，若当前处于在场且空闲状态，新入站消息又未命中激活关键字，并且“当前时间 - 上次成功回复时间”已超过 `patience_seconds`，现在会在入站判定当下直接切换为离场且不再激活助理，不必再等 LLM 先跑出 `no_reply`；同时将 `patience_seconds` 的默认值从 `420` 秒下调为 `60` 秒，并同步前后端联系人配置默认口径

## 更新：个人微信媒体发送链路

- 修复（weixin-oc-media-send-and-response-parse）：个人微信渠道补齐 `getuploadurl -> AES-ECB 加密上传 CDN -> sendmessage 引用媒体` 发送链路，`remote_im_send` 现在会按内容项顺序拆成“文本 -> 图片/文件 -> 文本”分别发送，贴纸与其他媒体不再只能卡死在纯文本路径
- 修复（weixin-oc-sendmessage-empty-response-success）：个人微信 `sendmessage` 在服务端返回空对象 `{}` 但实际已送达时，不再因本地把缺失的 `ret` 误判为 `-1` 而报假失败；现已按参考实现将缺失 `ret/errcode` 视为成功默认值 `0`
- 修复（weixin-oc-upload-field-alignment）：对齐个人微信 `getuploadurl` 返回字段名，兼容 `upload_param / upload_full_url` 与 camelCase 别名，修正媒体上传阶段误判“响应缺少 upload_param / upload_full_url”的问题；同时补充媒体发送分段与上传阶段中文日志，便于继续定位渠道侧兼容问题

## 更新：表情贴纸系统

- 功能（meme-sticker-system）：新增基于 `.meme` 工作目录的贴纸系统；模型可在回复正文中直接使用 `:happy:` 这类分类语法，后端会在助理消息写入历史时按分类随机选定具体图片并落入 `providerMeta.memeSegments`，本地聊天与归档视图统一按已持久化结果渲染贴纸，远程联系人发送时则按同一份 `memeSegments` 拆成“文本 -> 图片 -> 文本”顺序发送，保证本地显示与远程实际发送一致
- 功能（meme-yoink-inventory）：新增内置 `meme` 贴纸入库工具，按 `name / category / path` 把当前可见图片收进系统工作目录 `.meme/<category>/`；提示词不再依赖列表工具查询，而是直接注入当前可用分类，要求模型在需要贴纸时直接输出 `:分类名:`
- 修复（meme-dhash-dedup-and-format-fix）：贴纸入库新增 `dHash` 去重与 `.meme/image_dhash_index.json` 索引；每次更新索引时会同步清理已不存在文件的陈旧项，并在新图入库前检测近似重复图后直接跳过保存；同时补齐“按文件头识别真实图片格式”的自动修正逻辑，解决文件扩展名与真实格式不一致（如文件名是 `.jpg` 但文件头实际是 `GIF89a`）时偷图失败的问题，保存时会自动改用真实扩展名

## 更新：前台切会话轻量快照

- 优化（foreground-conversation-light-snapshot）：前台切会话主路径新增轻量快照接口 `get_foreground_conversation_light_snapshot`，只返回当前会话最近消息、`hasMoreHistory` 与当前 `todo/todos`，不再在首屏热路径中同步构建整份 `unarchivedConversations`
  - Rust 侧保留原有 `switch_active_conversation_snapshot(...)` 作为重型兼容入口，但新增共享 helper 将“当前会话最近消息快照”抽出为独立核心逻辑，避免切会话首屏继续被会话列表摘要、全局归一化与持久化副作用拖慢
  - 前端 `UnifiedWindowApp.vue` 的切会话、新建、会话分支、转发到会话与前台恢复链路改为优先调用轻量快照接口，只在独立链路中刷新未归档会话摘要，同时保留现有异步补消息 `request_conversation_messages_after_async(...)` 不变
  - 这样前台切换首屏只依赖“当前会话内容”，会话列表摘要与补消息解耦，实测切会话卡顿显著下降

## 发布：v0.9.14

- 修复（remote-im-bound-department-agent-fallback）：远程 IM 联系人已绑定部门但未显式绑定人格时，不再错误回退到全局 `default-agent`；现在会优先从目标部门自身的 `agent_ids` 中选择可用人格，避免出现 `agentId 与部门不匹配` 导致消息持续入队失败
- 修复（chat-draft-scroll-and-render-stability）：普通发送现在会像 `@人格` 路径一样在本地用户草稿插入后立刻上推，不再等到助理流式草稿出现；同时收口聊天消息列表在 `history_flushed / 助理草稿开始` 阶段的重绘范围，移除历史消息对全局 `chatting` 等状态的无差别订阅、稳定消息 `renderId`，并在前台消息合并时尽量复用旧消息对象，降低助理草稿出现时整列消息一起闪动的概率
- 发布（release-0.9.14）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.14`，用于触发本轮版本更新构建

## 发布：v0.9.13
- 修复（tool-review-sidebar-polish）：修正工具审查批次在 `currentBatchKey` 指向无效 key 时出现“计数为 0 仍可点开 / 明明有历史批次却打开空侧栏”的问题；同时收口工具审查侧栏列表与折叠卡层级，统一分页贴底、状态徽标、按钮样式与右侧面板留白
- 修复（markdown-renderer-polish）：适配新版 `markstream-vue` 后续渲染样式回归，收口聊天消息与工具审查侧栏中的 Markdown 排版、列表/引用字号、代码块主题与本地化文案；补齐新版代码块头部与正文配色、展开按钮显隐判断，并移除旧版 `cropperjs/dist/cropper.css` 入口以兼容 `cropperjs 2.x` 构建路径变更
- 维护（deps-phase-3-high-risk）：按依赖升级盘点计划完成第三阶高风险升级；前端提升 `vite`、`@vitejs/plugin-vue`、`typescript`、`vue-i18n`、`shiki`、`lucide-vue-next`、`cropperjs`，后端提升 `time`、`rmcp`、`reqwest`、`genai`、`rusqlite`、`tantivy`、`zip`，并补齐新版 `cropperjs` 裁剪流程、`rmcp` builder/HTTP 头签名、`tantivy` 搜索 collector 与若干终端/记忆兼容改造；通过 `pnpm typecheck` 与 `cargo check` 校验
- 维护（deps-phase-2-medium-risk）：按依赖升级盘点计划完成第二阶中风险升级；前端提升 `mermaid` 与 `markstream-vue`，后端提升 `async-openai`、`tokio`、`tokio-tungstenite`、`pdf_oxide`、`xcap`、`windows-sys`，并收口 `ChatShikiCodeBlockNode` 对新版 `markstream-vue` 的 `themes` 类型兼容；通过 `pnpm typecheck` 与 `cargo check` 校验
- 维护（deps-phase-1-low-risk）：按依赖升级盘点计划完成第一阶低风险升级；前端提升 `@tauri-apps/plugin-dialog`、`@tauri-apps/cli`、`vue`、`vue-tsc`、`tailwindcss`、`@tailwindcss/postcss`、`postcss`、`katex`、`stream-markdown`，后端提升 `tauri`、`tauri-build`、`tauri-plugin-dialog`、`tauri-plugin-updater`、`tauri-plugin-single-instance` 与 `captis`，并通过 `pnpm typecheck`、`cargo check`、`pnpm exec tauri info` 校验
- 功能（github-update-entry-and-badge）：更新检查流程收口为 GitHub 单源；配置窗口启动后与每天凌晨 4 点会静默检查更新，检测到新版本时会在设置侧栏“关于”项显示角标，并在配置页标题栏左上角显示“立即更新”入口；按钮点击会复用现有更新确认/下载弹窗，更新进行中支持重新唤起进度窗口
- 修复（chat-tool-phase-stream-rebind）：聊天活动视图绑定升级为真实 Delta 通道；工具开始执行时，后端会发出 `stream-rebind-required` 事件，前端收到后立即为当前会话重建并重绑流式通道，后续 LLM 流式阶段优先走最新活动通道发送；同时补齐重绑链路的开始/完成/失败日志与投递失败告警，降低工具阶段后半程前端实时黑洞的概率
- 发布（release-0.9.13）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.13`，用于触发本轮版本更新构建

## 发布：v0.9.12

- 修复（chat-local-link-percent-decode）：聊天消息里的本地文件链接在浏览器自动编码中文文件名或 `file:` URL 后，现在会先做安全解码再交给本地打开链路；修正 `%E8%8E%89...` 这类 UTF-8 百分号编码路径无法被 Windows 资源管理器识别的问题，并兼容盘符路径、`file://` 形式与 UNC 路径
- 功能（chat-selection-derive-and-deliver）：聊天窗口新增消息多选模式；可从单条消息操作区进入多选，整行勾选消息，并在输入区切换为 `会话分支 / 复制 / 分享 / 转发到会话 / 取消` 操作条；其中 `复制` 已支持按 `[角色名]: 内容` 格式汇总已选消息，`分享` 当前保留为暂不支持提示，`转发到会话` 会把已选原消息连同工具调用与元数据一起插入目标会话末尾，`会话分支` 会继承当前会话部门/人格/计划模式/工作区等设置，并以“最新压缩消息 + 已选原消息”生成新会话；同时新增“继承当前会话”创建入口、会话分支/转发到会话忙态遮罩，以及目标会话忙碌时的拒绝保护
- 功能（archive-report-and-fork-scope）：会话分叉与归档语义继续收口；前台会话关系以父会话与分叉点为主，归档统一生成结论汇报，并在存在有效 `fork_message_cursor` 时仅总结分叉点之后的讨论；处理当前会话弹窗收口为“压缩 / 丢弃 / 归档”，不再在当前前台主流程暴露归档投放目标
- 修复（prompt-adjacent-assistant-normalization）：最终请求体构建新增连续 assistant 归一化，`build_prompt`、工具回放追加、最终 JSON 序列化与 provider request 构建前都会消除相邻 assistant 消息，并合并 `text / reasoning_content / tool_calls`；同时修复非 self persona 历史消息与最新用户消息的说话人/时间元数据落位，避免 prompt 中 speaker block 丢失
- 修复（selection-deliver-and-copy-hardening）：修正转发到会话链路在会话锁外预读运行态导致的 TOCTOU 竞态，改为先拿 `conversation_lock` 再检查目标会话流式/整理态；补齐多选转发到会话目标有效性校验、选择态 `v-memo` 依赖、剪贴板复制异常处理，以及多选摘要/忙态提示/分享提示的 i18n 文案
- 修复（read-file-start-count-rename）：`read_file` 工具参数从 `offset/limit` 重命名为 `start/count`，并同步更新返回字段与续读提示；现在对文本/代码/Office 明确表示“起始行 + 行数”，对 PDF 明确表示“起始页 + 页数”，降低模型把分页参数误解为通用偏移量的概率
- 修复（chat-session-binding-rebind）：聊天发送与 `@人格` 发送前新增会话绑定纠偏；当旧会话引用的部门已不存在时，直接提示“部门已经消失”；当部门仍在但原人格已不再属于该部门时，会自动切换到该部门当前可用人格并回写会话绑定，避免继续抛出 `Agent ... is not assigned to department ...`
- 修复（mcp-tool-parameter-visibility）：MCP 配置页工具列表新增参数展示；后端会把工具 `input_schema` 一并返回前端，已部署工具现在可在名称下看到参数类型、必填标记、枚举/范围与示例内容，避免只能看见工具名和描述却无法判断调用入参
- 功能（remote-im-proactive-state-machine）：远程联系人新增 `away/present + idle/busy` 主动应答状态机；入站消息先做联系人在场判定，支持忙时挂单次待办、`away -> present` 时插入轻量上下文边界、联系人消息游标与压缩边界持久化，以及基于 `patienceSeconds` 的耐心离场策略；同时补齐自动发送成功回写、关键后端测试与联系人配置页新字段
- 修复（remote-im-round-finalize-fallback）：聊天调度在远程联系人轮次收尾失败时不再短路前端完成/失败通知；现在会优先完成 `emit_round_completed_event` / `emit_round_failed_event` 与 pending 事件收尾，再把状态机收尾异常记为警告日志，并统一修正远程 IM 运行时锁错误文案为中文
- 发布（release-0.9.12）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.12`，用于触发本轮版本更新构建

## 发布：v0.9.11

- 功能（user-mention-async-delegate）：聊天输入区新增结构化 `@人格` 异步委托链路；候选按人格去重，只允许选择存在部门归属的人格，点击工具栏头像或输入裸 `@` 均可 toggle mention；带 mention 的消息会走独立 `send_user_mention_message` 路径，不再触发当前会话负责部门主回答，而是按人格映射首部门并发派发用户级异步委托；mentions 会写入消息元数据、随撤回/重生恢复，并在用户气泡与发给模型的正文前缀中统一显示为 `@A,@B`
- 修复（delegate-result-append-event）：所有委托结果（用户 mention 委托与 LLM delegate）统一改为“写回原会话后直接追加消息事件”推送前端，不再依赖 `history_flushed` 才能看见结果；同时移除请求体中误泄漏的 `targetAgentId/targetDepartmentId` 内部上下文字段，补齐委托失败写回日志，降低“后端完成但前端黑洞”的偶发问题
- 发布（release-0.9.11）：同步前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本号到 `0.9.11`，用于触发本轮版本更新构建
- 重构（terminal-command-analyzer）：新增终端命令语义分析器，按 shell 语义区分读取白名单、重定向与路径访问意图；读取类白名单命令允许跨目录读取，非白名单命令再按工作目录权限等级处理，`full_access` 下跳过 AI 审查但仍保留路径边界校验；同时修复 `2>/dev/null` / `2>nul` 误判、补齐相关测试，并让终端/补丁拒绝结果继续回给模型而不再非人为打断整轮工具调用

## 发布：v0.9.10

- 功能（config-migration-package）：新增数据迁移页与加密迁移包导入导出链路，支持同版本迁移包预检、供应商与记忆增量导入、配置备份后导入与自动重启，并收口迁移页交互与本地元数据/授权文件迁移边界
- 功能（models-dev-cache）：后端新增 `models.dev api.json` 原始表缓存；配置页读取模型元数据时只读本地缓存，手动刷新模型时才按 1 天过期策略更新缓存，API 页面切入后会自动用缓存表回填当前模型能力上限
- 优化（config-api-model-card）：API 配置页模型卡片调整下拉区位置与展开图标，补齐模型刷新成功提示，支持刷新模型时直接使用当前草稿中的协议与 Base URL，无需先保存即可生效
- 性能（chat-send-hotpath-persist）：发送热路径改为“单会话快速写入”，不再走整份 `before/after` 运行态比较；会话概览从消息链路中移出，工具审查批次改为首次进入会话与轮次完成时刷新，显著降低发送前锁持有与无意义后端轮询
- 性能（conversation-rewind-fast-persist）：会话撤回热路径改为单会话快速写入，不再为当前会话撤回走整份 `before/after` 运行态比较，降低撤回时的锁持有时间
- 修复（runtime-log-visibility）：补齐 `app_handle` 锁异常日志，工具审查批次读取聚合耗时改为 debug 级结构化日志，避免静默吞错与高频 stderr 噪音
- 修复（release-lockfile-checksum）：修正发布时误把第三方依赖 `memmap2` 从 `0.9.9` 改成 `0.9.10` 的锁文件问题，恢复正确版本与校验值，避免 CI 因 checksum mismatch 构建失败
- 修复（tool-review-anchor-context）：提交工具审查报告时，聊天上下文改为以当前审查批次为锚点向前回溯；后续新聊天不会再污染本次审查，且命中上下文摘要时会整条保留摘要内容
- 修复（delegate-same-agent-guard）：委托候选部门会排除与当前人格相同的部门；若模型仍尝试委托给同人格部门，运行时会直接拦截并返回“该部门主管就是你自己，自己解决。”
- 修复（chat-paste-path-text）：聊天输入框不再把纯文本 Windows 路径误判为本地文件附件，粘贴 `C:\...` / UNC 路径时会按普通文本进入输入框
- 优化（supervision-dialog-history-polish）：督工弹窗移除顶部说明文案，最近督工记录区只保留目标文本，底部操作按钮固定回到右侧
- 修复（tool-review-button-reset）：切换会话时立即清空工具审查批次与加载状态，避免上一条会话的可审查状态残留，导致空会话里审查按钮仍可点击
- 重构（config-and-persona-command-split）：继续拆分 `config_and_persona.rs`，把会话快照与人格/聊天设置相关命令下沉到独立子模块，收口单文件复杂度
- 修复（tool-review-refresh-guard）：批量评估、单项评估与提交审查报告后会刷新前台工具调用消息；当当前会话没有可审查内容时，审查按钮保持禁用且不会再打开空侧栏
- 修复（codex-spark-capability-guard）：Codex 供应商默认不再向 `OpenAI Responses` 下发 `reasoning.summary`，并强制 `gpt-5.3-codex-spark` 关闭图片能力，避免该模型因不支持相关参数而请求失败
- 发布（release-0.9.10）：同步版本号，并纳入本轮工具审查上下文锚点修复与近期已验证交互修复
- 修复（updater-installer-dir）：Windows 安装版自动更新时，显式把当前运行中的安装目录传给 NSIS 安装器，避免更新后回落到默认安装目录
- 文档（readme-refresh）：重写 README，补齐 0.8 之后的关键能力演进、当前产品定位与核心依赖致谢
- 前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本统一升级到 `0.9.10`

## 更新：安装版自动更新保留自定义目录

- 修复（updater-installer-dir）：Windows 安装版自动更新时，显式把当前运行中的安装目录传给 NSIS 安装器，避免用户最初装在自定义目录时，更新后又回落到默认安装目录

## 更新：工具审查栏与批次审查

- 功能（tool-review-sidebar）：新增右侧工具审查栏、批次切换与最终审查报告链路，并补充工具审查前后端拆分与交互收口
  - 聊天工具栏新增“审查（N）”入口，点击后在右侧打开工具审查栏；按“用户发言 -> 后续工具调用”对 `shell_exec` / `apply_patch` 进行分批展示
  - 每个工具项支持懒加载展开、单项评估、批量评估、查看更改与查看审查报告；最终审查报告改为隐藏消息持久化，并按 `batchKey` 绑定与原位更新
  - 最终审查报告支持 Markdown 弹窗展示、失败态分离、自动生成与重新生成；查看按钮与加载状态交互已收口，避免列表与报告错误混杂
  - Rust 侧将工具审查相关类型、helper 与 Tauri 命令从 `config_and_persona.rs` 拆分到独立 `tool_review.rs`，降低单文件复杂度并保持命令签名兼容
  - 修复工具审查前端若干一致性问题：报告错误栈保留、`title` 可选 props、批量评估英文文案、补丁 diff 背景透明度与审查按钮括号计数样式

## 更新：工具审查

- 功能（tool-review）：为终端命令与 `apply_patch` 增加可配置的工具审查模型，并统一接入“智能审查 / 本地降级审批 / 原始返回兜底”的审批链路
  - 设置页新增“工具审查模型”配置项，未配置时保持原有审批路径；已配置时，非明显安全的工具执行会先请求审查模型给出放行判断与面向普通用户的审查意见
  - 审批弹窗重做为“审查意见 + 影响范围 + 原始代码/补丁预览”的双视角结构，既保留程序员可读的补丁信息，也补充普通用户能看懂的审查说明
  - 审查模型不可用时会明确提示已降级为本地规则审查；模型返回不符合约定的 JSON 时，会直接展示原始返回供用户判断
  - 智能审查结果会写入工具返回记录，便于后续在工具消息中回看；同时修复智能审查后又继续触发本地审批、导致重复弹窗的问题

## 更新：UnifiedWindowApp 职责拆分

- 重构（unified-window-app-splitting）：持续拆分 `UnifiedWindowApp.vue` 的壳层职责，把审批弹窗、弹窗宿主、计划模式、确认计划、督工任务、窗口动作、聊天工作区选择器与附件选择等外围编排逻辑迁移到独立 composable / 组件
  - 新增 `ShellDialogsHost`、`TerminalApprovalDialog` 等壳层组件，统一承接更新提示、运行日志、撤回确认、归档预览、技能占位、强制归档与终端审批等弹窗
  - 新增 `use-shell-dialog-flows`、`use-window-actions`、`use-conversation-plan-mode`、`use-confirm-plan`、`use-supervision-task`、`use-chat-workspace-picker-flow`、`use-chat-attachment-picker-flow` 等 composable，收口原本堆叠在根组件中的非核心 UI / 流程逻辑
  - `UnifiedWindowApp.vue` 继续保留聊天总控与跨域编排角色，但已显著减轻模板噪音与外围状态负担，为后续按“聊天同步总控域”继续细拆铺路

## 更新：终端与补丁审批链路重构

- 重构（terminal-approval-and-apply-patch-review）：重做终端命令与 `apply_patch` 的审批链路、风险展示与补丁预览，收口为“先预检，再审批，再执行”的一致模型
  - 前端新增独立审批模块，补齐风险等级、影响范围、补丁分页、`mockup-code` 预览与“批准 / 拒绝”操作，审批弹窗不再展示超时、工作目录等噪音信息
  - `apply_patch` 审批改为先做 dry-run 预检，`Update File` 会在审批前真实匹配 hunk；匹配失败直接报错，不再把根本不可应用的补丁交给用户审查
  - 审批预览不再信任模型给出的原始行号，而是由后端根据真实匹配结果重新生成补丁视图；`Add File / Delete File / Update File` 都会携带可用于前端渲染的真实 diff 内容
  - 终端审批移除超时机制，审批请求会一直等待用户明确“批准 / 拒绝”，不再因为桌面端暂未查看而自动失败
  - 用户拒绝 `exec` 或 `apply_patch` 后，结果会写入工具历史并立即终止本轮调度，不再把“已拒绝”的工具结果继续喂给模型续跑

## 更新：聊天图片输入规范化

- 修复（chat-image-normalization）：聊天图片进入消息链路前会先做统一规范化，避免 8K 截图等超大分辨率图片直接送模时触发上游尺寸/像素限制报错
  - 聊天图片现在会在入链路时按长边上限 `1280` 等比缩放，小图不放大
  - 缩放后统一编码为 `WebP`，质量设为 `75`，并补充了针对超大 PNG 的回归测试

## 更新：远程 IM 渠道启停竞态修复

- 修复（remote-im-channel-lifecycle-locking）：收紧 OneBot / 个人微信 / 钉钉 Stream 渠道服务的启停生命周期管理，避免快速启停时并发穿透导致同一渠道被重复启动、旧任务失管或出现假死
  - 为每个 `channel_id` 增加渠道级生命周期锁，把 `start / stop / reconcile` 串行化到同一条执行链中，确保“先停旧实例，再起新实例”不再被并发打断
  - OneBot、个人微信、钉钉 Stream 的生命周期锁 key 已统一与现有运行态 map 使用同一份原始 `channel_id` 表示，避免锁表与 `states / tasks / stop_senders / channel_shutdowns` 等 map 出现 key 规则不一致

## 更新：存储层分片读写重构与兼容层收口

- 重构（storage-shard-write-refactor）：保留现有 `config/state/chat/conversations` 文件结构，仅重写 `app_data` 服务层边界，把热路径从整份 `AppData` 读改写切到分片读写、分片缓存与分片持久化
  - 新增 `agents`、`runtime_state`、`chat/index`、`conversation:<id>` 分片级读写与缓存同步能力，常见配置、人格、单会话更新、会话创建删除、聊天消息追加、Remote IM 与归档链路不再默认走整份状态写回
  - `write_app_data()` 保留为兼容聚合入口，但补充 deprecated 标注、兼容层文档与结构化诊断日志；生产热路径已收口到分片 API，测试专用全量 state helper 也已显式隔离
  - 修复 `runtime_state` 分片遗漏的 `image_text_cache`、`pdf_text_cache`、`pdf_image_cache` 等缓存字段，统一图片转文与 PDF 缓存读写口径，避免读写分裂与缓存丢失
  - 补齐缓存命中条件、poisoned lock 错误处理、会话读取失败日志和 chat index 去重/差量写入逻辑，防止 `None == None` 假命中与静默吞错
  - 新增静态守卫工作流，阻止新增 `write_app_data()` 业务调用点，并补充分片写入、chat index、PDF 缓存等回归测试

## 发布：v0.9.8

- 发布（release-0.9.8）：同步版本号，并纳入本轮聊天输入区模型切换与部门首位模型即时同步修复
- 前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本统一升级到 `0.9.8`

## 更新：聊天输入区模型切换改为即时更新部门首位模型

- 修复（chat-primary-model-switch-sync）：聊天输入区切换模型时，改为直接更新当前前台部门的首位模型并持久化到后端配置，避免只改前端显示导致发送仍走旧模型
  - 新增后端专用命令，直接调整目标部门的首位模型，保留原队列中其余模型顺序，不再发生首位模型覆盖后丢失旧项的问题
  - 输入面板切换模型时改为调用后端真实修改路径，并在前端做同样的队列重排预览；失败时会回滚本地状态
  - 聊天区相关模型读取与消息刷新统一对齐到当前前台部门的首位模型，不再误盯主助理默认模型配置

## 更新：聊天 Todo 浮层改回原生组件样式

- 调整（chat-todo-dropdown-theme-polish）：将聊天区顶部 Todo 浮层从手写样式收回到原生 DaisyUI 组件组合，并把触发按钮底色稳定到 `base-300`
  - Todo 触发器改为 `dropdown + btn`，明细面板改为 `dropdown-content + card`，不再保留专用浮层背景与动画样式
  - 触发按钮使用 `base-300` 底色与 `base-300` 边框，避免透明感和深浅主题下的视觉漂移

## 更新：计划工具与聊天计划模式协议化

- 功能（plan-tool-and-chat-plan-mode）：新增 `plan` 协议工具与聊天计划模式，正式把“先计划、确认后执行”的流程从普通文本约束升级为可渲染、可确认、可恢复的协议链路
  - 后端新增内置 `plan` 工具，仅保留 `action + context` 两个参数，支持 `present / complete` 两种完结动作，并将工具结果转换为 `plan_present / plan_complete` 消息元数据
  - 聊天区新增计划卡片渲染，`plan.present` 会展示执行计划与“我同意，并执行计划”按钮，`plan.complete` 会展示完成汇报
  - 输入区新增“计划模式”开关按钮，并支持 `Shift+Tab` 快捷切换；用户确认执行计划时会自动关闭计划模式
  - 计划模式改为会话运行时信号，后端统一根据当前会话计划状态决定是否注入计划提示，前端不再直接拼接计划提示词正文
  - 计划提示注入位置调整到最新用户消息元信息区域，排在说话人/时间文本之前，确保模型能稳定感知计划模式要求
  - 上下文压缩链路补充“最后一个未完成计划”恢复逻辑：若最近存在 `plan_present` 且后续未被 `plan_complete` 关闭，则会把待执行计划带入压缩摘要
  - 设置页工具目录、工具状态检查与默认工具绑定补齐 `plan`，并统一“缺失工具项默认按开启处理”的口径，避免前端显示已开启但运行时不可用

## 更新：前台部门不再强保留

- 调整（front-desk-department-removable）：移除 `front-desk-department` 的系统强保留身份，避免“前台”角色命名继续误导主助理/副手之外的部门结构
  - 部门设置中 `front-desk-department` 不再属于不可删除的内置部门，现有用户若保留了该部门，可自行删除
  - 部门排序中不再给 `front-desk-department` 预留固定内置顺位，仅保留主助理与副手优先
  - 欢迎页部门统计同步更新，不再把 `front-desk-department` 视为系统保留部门排除在自定义部门之外

## 更新：聊天输入区与设置侧栏细节优化

- 调整（chat-toolbar-and-config-sidebar-polish）：收紧聊天输入区与设置页侧栏的前端细节，使模型切换入口更顺手、侧栏布局更贴边
  - 聊天输入区在语音按钮右侧新增首要模型下拉框，直接复用当前会话/前台部门的首位文本模型选择链路，只切换首要模型
  - 对话窗口顶部“压缩/归档当前会话”按钮图标由 `Minimize2` 调整为 `FoldVertical`，语义更贴近当前操作
  - 设置页左侧菜单移除外层与内容区之间的额外横向空隙，并让 DaisyUI `menu` 铺满侧栏容器，避免右侧残留留白

## 更新：设置标题栏搜索跳转

- 功能（config-header-search-tabs）：在设置窗口标题栏新增搜索框，支持按当前语言文案搜索相关设置并直接跳转到对应 tab
  - 新增设置搜索索引模块，基于 i18n 文案聚合各个配置 tab 的可搜索文本，搜索结果按 tab 去重返回
  - 标题栏在 `config` 视图下显示搜索框与结果下拉，选择结果后直接切换到对应设置 tab，不做页内定位与高亮
  - 补齐聊天设置中遗漏的硬编码文案 i18n，确保搜索来源统一走多语言文本，不再混用写死字符串
  - 标题栏布局改为纯 CSS 三列居中方案，移除按钮宽度测量与镜像占位逻辑，搜索框宽度收口为更紧凑的标题栏居中输入

## 更新：收拢指令预设的交互与布局

- 调整（instruction-preset-ui-tightening）：将指令预设收口为“短指令正文”模型，移除标题心智，并同步简化设置页与输入面板交互
  - 设置页中的指令预设改为真正的列表样式，每项只保留单行正文输入与删除按钮，不再使用卡片嵌套或标题字段
  - 输入面板左下角的指令按钮改为纯 `Layers2` 图标按钮，指令面板改为无边框、无额外内外边距的流式布局，按内容长度自然伸缩并截断显示
  - 指令面板支持 `Tab` 开关、四方向键切换焦点、`Enter` 选中后立即关闭，已选指令 badge 也统一直接显示正文
  - 前端保留对旧 `name` 字段的兼容读取，但当前保存与展示语义已统一以 `prompt` 正文为准

## 发布：v0.9.7

- 发布（release-0.9.7）：同步版本号并纳入本轮 Codex 协议接入、百炼多模态缓存、会话主工作目录 `AGENTS.md` 注入、输入面板指令系统、流式工具历史与配置保存链路修复
- 前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` / `Cargo.lock` 版本统一升级到 `0.9.7`

## 更新：移除 watch 链上的配置回写

- 修复（config-watch-no-writeback）：收紧配置页 watch 职责，禁止在 watch 中回写 config，避免切换图转文 / 打开预览时叠加保存导致会话锁连锁竞争
  - 删除会话 API 设置在 `use-app-watchers` 中的自动保存 watch，`assistantDepartmentApiConfigId / visionApiConfigId / sttApiConfigId / sttAutoSend` 不再因为 watch 变化而重复写配置
  - 删除 `use-app-watchers` 中根据部门变化直接回写 `assistantDepartmentApiConfigId` 的逻辑，保留只同步派发给界面所需的助理人格 ID
  - 删除 `use-app-watchers` 中根据 `enableTools` 自动补工具列表的 config 回写，避免只因视图切换就修改配置对象
  - 删除 `ApiTab` 与 `CodexProviderPanel` 中在 watch 里自动应用 Codex 默认值的逻辑，把默认值收口到显式操作路径，避免选择 provider 时隐式改配置
  - 删除请求体预览 / 系统提示词预览前的预保存链，预览改为直接读取当前内存态，不再因为“看一眼”先执行 `savePersonas / saveChatPreferences / saveConversationApiSettings`

## 更新：流式打断保留已完成工具历史

- 修复（stream-stop-preserve-completed-tool-history）：聊天流式阶段被 stop 打断时，已完成工具不再随 draft 一起消失，而会写入最终 assistant 消息
  - 后端新增 inflight completed tool history 运行时缓存，按会话记录当前轮次已经完成并可持久化的工具历史
  - 工具循环在工具结果落入 `tool_history_events` 后立即同步缓存，stop 时只保留这些已完成事件，不记录仍在执行中的工具
  - `stop_chat_message` 的持久化条件扩展为“部分文本 / reasoning / 已完成工具历史”三者任一存在即可落消息
  - stop 生成的 assistant message 现在会带上已完成 `tool_call` 历史，前端沿用现有回填链路即可自然显示
  - 正常完成链路结束后会清理运行时缓存；用户 stop 时保留缓存供 stop 持久化使用，避免已成立工具事实丢失
  - 补充前端消息块生成条件：即使 assistant 文本为空，只要正式消息带有工具历史，也必须生成消息块并渲染工具时间线

## 更新：流式工具列表改为顺序状态展示

- 调整（streaming-tool-sequence-status）：收紧聊天流式阶段的工具展示逻辑，改为按调用顺序追加，并只维护“前序 done / 当前 doing”的简化前端状态
  - 流式工具列表项新增临时 `status` 字段，前端不再强依赖单槽位 `toolStatusText / toolStatusState` 作为主展示来源
  - 每当新的工具开始执行时，会把上一条流式工具标记为 `done`，并把当前工具追加为 `doing`，避免多次串行调用时状态文案来回覆盖
  - 聊天区工具折叠面板改为按每条工具自己的临时状态渲染时间线与标签，使工具执行过程更稳定、更直观
  - 该状态仅用于流式阶段的过程展示，不追求与真实成功/失败完全一致；正式消息落地后仍以最终持久化的工具历史为准

## 更新：Codex 默认开启图片与工具能力

- 修复（codex-capability-defaults）：修正 Codex 协议下模型能力默认值与保存归一化逻辑，避免 Codex 被错误标记为不支持图片或被手动关闭工具调用
  - 前端新建 Codex provider / model 时，默认开启 `enableImage` 与 `enableTools`
  - 前端本地绑定归一化时，若 `requestFormat === codex`，强制 provider 与 model 的图片能力、工具能力保持开启
  - 后端保存配置归一化时，若 `request_format.is_codex()`，强制 provider、model 与 api config 的 `enable_image / enable_tools` 为开启状态
  - 这样可以保证 Codex 视图、保存结果与实际能力语义一致，不再出现默认无多模态或可关闭工具调用的偏差

## 更新：输入面板指令系统

- 功能（composer-instruction-presets）：在聊天输入面板新增可复用的用户指令系统，支持从设置页维护短指令预设，并在发送时作为文本附件加入本轮消息
  - 聊天设置新增 `instructionPresets` 持久化字段，指令预设跟随应用运行数据保存与加载，不进入项目工作区或 skill 目录
  - 设置页新增“指令预设”管理，支持新增、编辑、删除与保存，空项会在保存时自动清理
  - 输入面板新增“指令”按钮，支持点击展开，也支持在输入框内按 `Tab` 唤起指令面板
  - 指令面板支持上下键切换焦点，按 `Enter` 使用当前指令；单次消息可选多个指令，同轮重复选择自动去重
  - 已选指令会以 badge 形式显示在输入区上方，可单独移除；切换会话或发送后会清空当前轮次已选指令
  - 发送链路复用现有 `extraTextBlocks`，将每条指令包装为 `<user instruction>` 文本附件随用户消息一并发送
  - 顺手修复输入面板中的 IDE 提示：移除重复的 `instructionPresets` 透传、将“指令 / 暂无指令预设”接入 i18n，并恢复左右方向键在文本框中的正常移动光标行为

## 更新：会话主工作目录 AGENTS 自动注入系统提示词

- 功能（workspace-main-agents-md-injection）：当当前会话存在用户指定的 main 工作目录时，自动读取其根目录下的全大写 `AGENTS.md`，并追加到系统提示词末尾
  - 只检查当前会话 `shell_workspaces` 中 `built_in=false` 且 `level=main` 的工作目录，不再回退到 system workspace
  - 仅检查 main workspace 根目录下的 `AGENTS.md`，不递归搜索，不扫描 secondary workspace，也不读取小写 `agents.md`
  - 正式聊天发送链路与 prompt 预览链路统一接入同一套 `AGENTS.md` 注入逻辑，避免预览与真实请求不一致
  - `AGENTS.md` 不存在、为空或读取失败时降级跳过，并记录 `[AGENTS注入]` 日志，不阻断聊天主流程
  - 新增回归测试，覆盖用户 main workspace 命中注入、built-in workspace 跳过、仅 secondary workspace 跳过三类边界

## 更新：补齐 Codex 新字段后的测试初始化

- 修复（codex-config-test-fixture-backfill）：补齐测试与辅助构造中因 Codex 新字段引入而缺失的初始化字段，恢复相关单测编译通过
  - 为 `RefreshModelsInput` 测试补齐 `provider_id / codex_auth_mode / codex_local_auth_path`
  - 为 `ApiConfig` 测试样例补齐 `codex_auth_mode / codex_local_auth_path / reasoning_effort`
  - 为 `ApiProviderConfig` 与 `ApiModelConfig` 测试样例补齐 `codex_auth_mode / codex_local_auth_path / reasoning_effort`
  - 已验证 `build_workspace_agents_md_block`、`fetch_models_openai_should_read_models_from_base_url`、`delegate_target_chat_api_config_ids_should_only_keep_current_department_models` 三组测试可通过

## 更新：百炼多模态临时 URL 缓存与预览同路

- 功能（aliyun-bailian-multimodal-url-cache）：为阿里云百炼新增多模态临时 URL 缓存，并让请求体预览与真实发送共用同一套媒体注入分支
  - 当 API `base_url` 的 host 包含 `aliyuncs` 时，识别为百炼供应商，支持百炼通用与百炼编程两条渠道
  - 新增消息级 `provider_meta.aliyunMultimodalCache`，按“媒体类型 + 模型名 + 内容哈希 + 过期时间”缓存百炼临时 URL
  - 百炼发送前会优先复用未过期临时 URL；若没有可用缓存，则优先使用原图 `saved_path` 读取本地文件生成新的百炼 URL，仅在拿不到原图路径时才回退到 base64
  - 百炼请求自动补 `X-DashScope-OssResourceResolve: enable`，并在 `genai` 请求构造阶段支持把媒体注入为 URL 而不是固定 base64
  - 请求体预览改为先走同一套百炼 URL 处理逻辑，再输出 JSON，避免预览与真实发送不一致
  - 顺手收敛了百炼上传响应体错误处理、异步文件读取和缓存回写去重逻辑

## 更新：新增 Codex 协议与账号登录接入

- 功能（codex-protocol-and-auth-integration）：新增独立 `codex` 协议，完成本地凭证读取、应用内 OAuth 登录、静态模型列表与配置页独立面板接入
  - 前后端新增 `codex` 协议、`codexAuthMode / codexLocalAuthPath / reasoningEffort` 等配置字段，并将 Codex 纳入文本模型与部门模型选择
  - 配置页新增独立 Codex 面板，支持“读取本地”与“自行登录”两种认证方式，隐藏 API Key、Base URL、温度、最大输出与手动上下文配置
  - 后端新增 Codex 凭证解析与托管 OAuth 存储，支持读取 `~/.codex/auth.json`、本地回调登录、运行时 token 刷新与退出登录
  - Codex 模型列表固定为 `gpt-5.4 / gpt-5.4-mini / gpt-5.3-codex / gpt-5.3-codex-spark / gpt-5.2`，思维强度支持 `low / medium / high / xhigh`
  - Codex 运行时继续复用 `genai OpenAIResp`，并统一固定 `base_url=https://chatgpt.com/backend-api/codex`、上下文窗口 `272000`

## 更新：非主会话归档/抛弃时标记关联任务为"会话丢失"

- 修复（task-session-lost-cleanup）：非主会话被归档或抛弃时，绑定到该会话的 active 任务无法继续执行但仍保持 active 状态
  - 新增 `mark_tasks_as_session_lost` 辅助函数，查找绑定到指定 conversation_id 的 active 任务并标记为 `failed_completed`，`completion_conclusion` 设为"会话丢失"
  - 在 `run_archive_pipeline_inner` 的四个删除/归档路径中，对非主会话调用任务标记
  - 在 `delete_unarchived_conversation` 中，对非主会话调用任务标记
  - 仅对非主会话标记，主会话归档/抛弃后会自动创建新会话，任务可以迁移
  - 标记操作容错处理，查询或标记失败只打印日志不阻断归档/删除流程

## 发布：v0.9.5

- 发布（release-0.9.5）：同步版本号并纳入本轮标题栏环形进度条、会话标题合并显示与督工按钮图标优化
- 前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` 版本统一升级到 `0.9.5`

## 更新：督工按钮添加 Timer 图标

- 优化（chat-supervision-button-icon）：督工按钮增加 Timer 计时器图标，更直观表达定时监督的含义

## 更新：聊天窗口标题栏环形进度条与会话标题显示优化

- 优化（chat-header-ring-progress-and-title）：将标题栏的上下文占用百分比数字改为环形进度条，并合并显示会话标题
  - 用 SVG 环形进度条替换原来的百分比数字显示，悬停时提示"当前上下文已使用 X%"
  - 进度条在占用 >= 70% 时显示警告色
  - 中间区域改为合并显示"会话标题 · 部门 · 人格"，用间隔号分隔
  - 主会话时显示"主会话"标题
  - 扩大标题区域最大宽度至 50% 以适应更长内容

## 更新：收拢会话工作目录语义并移除锁定概念

- 修复（chat-workspace-layout-remove-lock-semantics）：将会话工作目录统一收口为“目录列表 + 至少一个 main”的模型，移除旧的锁定/解锁残留语义
  - 前端会话工作目录弹窗保存后仅更新目录布局，不再隐式执行“解锁”或重置当前执行目录
  - 聊天窗口相关状态、事件与文案移除 `workspaceLocked / workspaceUnlocked` 旧概念，避免 UI 与真实行为不一致
  - Tauri 命令侧删除 `lock_chat_shell_workspace / unlock_chat_shell_workspace` 与 `locked` 返回字段，终端默认根目录始终跟随当前会话的 `main` 工作目录
  - 保留兼容旧数据所需的结构字段，但运行逻辑已不再依赖会话级“锁定路径”

## 更新：终端工作区提示词对齐会话路径

- 优化（terminal-prompt-align-conversation-workspace）：终端相关提示词改为优先展示当前会话工作路径，并移除“不要在命令中使用绝对路径”的固定提示
  - 聊天发送与提示词预览都会把会话级 `shell_workspace_path` 传入终端工作区提示块，减少提示词与实际执行根目录不一致的问题
  - 当会话工作路径与默认工作区不同，提示词会同时展示“当前会话工作路径”和“助理系统工作目录”
  - 先只调整提示词文案，不改变执行层对绝对路径的现有拦截行为

## 修复：所有todo完成后仍然显示胶囊卡片

- 修复（chat-todo-capsule-hide-when-all-completed）：当会话中所有 todo 均已完成时，隐藏顶部浮动 todo 胶囊卡片
  - 新增 `hasActiveOrPendingTodo` 计算属性，仅当存在 `pending` 或 `in_progress` 状态的 todo 时才渲染胶囊
  - 解决全部完成后胶囊仍占位悬浮、影响阅读体验的问题

## 更新：收拢督工任务语义与任务调度日志

- 修复（supervision-task-semantics-and-scheduler-logging）：统一督工任务的字段语义、提示词结构与任务调度表现
  - `task` 工具面向 LLM 的主字段统一为 `goal / how / why`，督工隐藏提示词改为结构化 `<task_remind>`，减少与 `todo` 工具的语义冲突
  - `task.complete` 改为终止型工具，成功后直接将 `completion_conclusion` 作为本轮最终回复返回，不再回喂模型续一轮
  - 任务提醒消息改为只显示结构化卡片，不再把隐藏提示词原文重复渲染到消息正文下方
  - 督工弹窗字段文案统一为“目标 / 为什么 / 怎么做”，去掉“原因 / 待办”这类易混淆表述
  - 任务调度数据库扫描周期恢复为 30 秒，并将“会话…的任务…，投递中”日志后移到真正投递前，避免预检查阶段高频刷屏

## 发布：v0.9.4

- 发布（release-0.9.4）：同步版本号并纳入本轮督工任务、任务调度与终止型任务完成回复修复
  - 前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` 版本统一升级到 `0.9.4`
  - 承接当前督工任务结构化提示词、调度扫描恢复 30 秒与 `task.complete` 终态回复直出等修复

## 更新：收紧 OpenAI 流式降级并回迁 todo 到主进程

- 修复（openai-stream-fallback-and-builtin-todo）：收紧 OpenAI 兼容接口的流式降级条件，并将 `todo` 工具从 MCP 子进程回迁为主进程内置工具
  - 非流式兜底仅在真正的流格式不兼容时触发，不再把 `504`、超时、空响应等普通链路错误误判为“应切到非流式”
  - 流式降级缓存改为按 `request_format + base_url + model` 维度记录，TTL 10 分钟；流式成功后立即清除对应缓存
  - OpenAI 非流式路径补齐工具循环能力，避免一旦降级就丢失工具调用
  - `todo` 工具改回主进程 builtin 执行，删除 `todo` MCP 子进程挂载与入口，避免跨进程双写、`app_handle` 误报与会话状态不同步
  - 会话 Todo 更新路径恢复对委托线程运行态的兼容，并保持“全部 completed 后自动清空当前 Todo”语义

## 发布：v0.9.3

- 发布（release-0.9.3）：同步版本号并承接当前 `rust-genai` 运行时迁移成果
  - 前端 `package.json`、Tauri `tauri.conf.json` 与 Rust `Cargo.toml` 版本统一升级到 `0.9.3`
  - 承接上一轮 `rust-genai` 全量迁移与 `rig-core` 依赖移除后的当前发布状态

## 更新：会话卡片工作空间显示追加部门

- 优化（chat-conversation-workspace-label-with-department）：会话列表卡片中的工作空间文案追加负责部门，直接显示为 `工作空间（部门）`
  - 侧边会话列表与悬浮会话列表统一使用相同展示格式
  - 部门名为空时保持仅显示工作空间名称，避免出现空括号
  - 不新增组件，不改动现有卡片结构与交互

## 更新：当前会话标题支持直接改名

- 功能（chat-current-conversation-rename）：支持在会话列表中直接修改当前会话标题，并同步刷新前台未归档会话概览
  - 侧边会话列表与悬浮会话列表均支持点击当前会话标题进入内联编辑
  - 新增未归档会话改名命令，保存后立即回写 `app_data.json` 并推送会话概览刷新事件
  - 主会话与“整理上下文中”的会话保持禁止改名，避免破坏现有固定语义与运行态约束

## 更新：聊天本地文件链接统一为正斜杠

- 修复（chat-local-link-normalize-slashes）：规范聊天区本地文件链接的路径格式，避免 Windows 反斜杠进入 `href` 后被浏览器编码成 `%5C`
  - 前端点击本地文件链接前先做统一路径规范化，继续兼容已有 `E:\...` 与已编码 `%5C` 的历史消息
  - Markdown 渲染后主动将本地文件链接改写为 `E:/...` 形式，修复悬浮提示与复制链接时的异常观感
  - 系统提示中的 Windows 文件链接示例改为优先使用正斜杠，减少模型继续输出反斜杠路径

## 更新：修复图片附件路径与图转文链路错位

- 修复（image-attachment-path-and-vision-fallback-flow）：收拢聊天图片消息的单一事实来源，恢复“先存引用、发送前再决定是否图转文”的正确链路
  - 前端发送图片时保留 `savedPath`，并将图片附件路径一并写入消息附件元数据，避免图片路径在发送链路中丢失
  - 调度器写历史时不再把“不支持图片”的消息提前改写为“已忽略”，而是先将图片外置为稳定媒体引用再落库
  - 模型请求前新增统一图转文回退：当前模型不支持图片但已配置图转文 AI 时，按缓存或视觉模型将图片描述注入提示词，而不破坏原始消息事实

## 更新：视觉与语音设置切换后立即保存

- 修复（chat-settings-immediate-save-on-select-change）：补齐对话设置页下拉与开关切换后的即时保存，减少视觉与语音配置因未手动保存造成的错觉
  - 图转文 AI 下拉框切换后立即触发保存，避免选择已改变但尚未落盘
  - STT 模型与自动发送开关切换后也立即保存，并在禁用 STT 时同步写回自动发送状态

## 更新：完成 rust-genai 全量迁移并移除 rig 依赖

- 重构（rust-genai-full-migration-and-remove-rig）：完成聊天运行时向 `rust-genai` 的全量迁移，并正式移除 `rig-core` 依赖
  - OpenAI / OpenAI Responses / Gemini / Anthropic 的主链与工具循环统一收口到项目自有 `genai` 运行时，不再保留 `rig` 运行链路
  - MCP 工具桥接、运行时工具抽象、Gemini schema 清洗与工具多模态回传统一切到项目自有实现，继续保留图片、音频与资源结果转发
  - 修复忙碌态发送时误插入用户草稿的问题，避免“排队中”消息与主消息区草稿同时出现
  - 补充运行时迁移护栏测试与中文错误日志，最终从 `Cargo.toml` / `Cargo.lock` 中彻底移除 `rig-core`

## 更新：会话处理补回丢弃入口并提前校验发布签名

- 修复（discard-entry-and-release-signing-precheck）：恢复会话处理弹窗中的正式“丢弃”能力，并让远程发布在开头就校验签名配置
  - “处理当前会话”弹窗恢复为 `压缩 / 归档 / 丢弃` 三项并列，不再把删除当前会话藏成仅在双禁用时出现的兜底按钮
  - 前后端预览字段统一从易误读的 `canDiscard` 改为 `canDropConversation`，减少把“丢弃会话”误解成“废弃功能”的风险
  - 发布工作流新增签名预检步骤，提前校验私钥、公钥与 `tauri.conf.json` 中 updater 公钥一致性，并让预检与正式 `pnpm tauri build` 共用同一套签名环境变量，避免再走偏离正式构建链路的额外参数分支
  - 删除 `scripts/run-tauri-with-updater-pubkey.mjs` 包装层，`pnpm tauri` 改回直接调用官方 Tauri CLI，避免构建时再临时改写 `tauri.conf.json`
  - 系统工具说明补充 `apply_patch` 的代码级正确示例与失败处理规则，减少模型将补丁编辑误退回为 `exec` 文本写文件的概率

## 发布：v0.9.1

- 发布（release-0.9.1）：整合委托功能修复与聊天交互打磨后发布补丁版本
  - 包含委托会话模型解析与候选模型校验修复，避免委托链路误用无效会话模型
  - 包含会话顶部 Todo 胶囊的悬停展开、状态样式与浮层阅读体验优化
  - 包含思考区与工具调用区折叠气泡的层级、状态提示与时间线展示调整

## 更新：系统提示词改为强制中文并补充提问规划规约

- 调整（system-prompt-force-zh-and-question-planning-rule）：统一聊天系统提示词核心规则为中文，并补充提问与规划约束
  - `conversation.rs` 中部门上下文、语言设定、远程联系人规则、委托线程约束等系统提示词统一改为中文输出，不再随 `ui_language` 切换
  - 新增 `system tools rule` 与 `question and planning rule`，补充 `todo / delegate / task / exec` 的使用规约，以及“提问之法 / 规划之道”的行为约束

## 更新：调整聊天消息中的思考与工具气泡样式

- 调整（chat-message-bubble-reasoning-and-tool-style-tuning）：统一对话消息里思考区与工具区的折叠气泡样式
  - 思考内容与工具调用区统一改为左侧竖线样式，弱化原先厚重卡片感，让消息层次更贴近正文气泡
  - 折叠标题补充更明确的状态图标、悬停反馈与字号权重，流式思考时继续保留“正在思考中”提示
  - 工具调用详情改为时间线式展示，每个工具步骤的名称、顺序与参数结构更容易快速扫读

## 更新：优化会话 Todo 胶囊的悬停展开体验

- 修复（chat-todo-pill-hover-panel）：调整对话页顶部 Todo 胶囊的展开交互与视觉表现
  - Todo 胶囊支持悬停自动展开当前会话的完整待办列表，不再需要额外点击切换
  - 展开区改为简洁数字列表，使用编号圆点表达步骤顺序，并按状态区分颜色
  - 当前进行中的待办文字加粗，已完成项显示删除线，浮层宽度根据文本内容自动调整

## 更新：接入 Windows 安装版与便携版自动更新

- 新增（windows-installer-and-portable-updater）：为 Windows 安装版与便携版接入真正可执行的自动更新链路
  - 后端新增统一 updater 模块，支持 GitHub Release 检查更新、安装版通过 `tauri-plugin-updater` 静默升级、便携版通过 `zip + staging + helper + 备份回滚` 完成替换
  - 应用启动与数据目录逻辑新增 `PORTABLE` 运行形态识别，便携版可使用可执行文件同级 `data` 目录，并在更新时按当前运行形态自动分流
  - 前端更新弹窗改为真实更新入口，支持“立即更新 / 强制更新”、下载进度展示、失败信息透出，不再只是跳转 Releases 页面
  - Windows 发布流改为仅产出 NSIS 安装包与便携版 zip，同时生成 updater 元数据与签名文件，并补充便携版 staging 校验测试

## 更新：清理剩余原生确认框

- 修复（replace-remaining-window-confirm-dialogs）：将归档页与任务页残留的原生 `window.confirm` 全部替换为 DaisyUI 模态框
  - 归档页删除归档、删除未归档会话、删除联系人消息统一改为页面内确认弹窗
  - 任务页放弃未保存修改、删除任务改为页面内确认弹窗，避免继续弹出浏览器原生确认框

## 更新：工作目录迁移支持三选一保存与全链路生效

- 修复（workspace-migration-and-effective-root-fixes）：补齐工作目录修改、迁移确认、初始化与真实运行根目录之间的断裂
  - 配置页保存工作目录时改为 DaisyUI 三选一确认：支持“迁移并保存 / 不迁移，仅保存 / 取消”，并展示后台迁移实时进度与失败原因
  - 后端工作目录迁移由跨盘不可靠的 `rename` 改为“复制 -> 校验 -> 删除旧目录”，通过 Tauri 事件持续向前端推送阶段与进度
  - Shell、MCP、Skills、私有部门/人格、附件相对路径、打开工作区文件、PDF 与 remote_im 等链路统一改为优先使用当前配置中的工作目录
  - “初始化工作空间 / 打开目录”不再只认旧默认路径，而是直接作用于当前填写并保存的工作目录

## 更新：修复 Rust 测试与旧配置归一化断裂

- 修复（rust-tests-and-legacy-normalize-fixes）：收口测试断裂与若干旧结构兼容问题
  - 修复用户画像记忆前言拼接、Windows 终端工作区路径比较与旧 `api_config` 到新 `provider::model` 端点 id 的映射
  - 更新多处 Rust 单测以匹配当前会话 replacement main、记忆注入位置与部门顺序的真实行为
  - 重新打通整套 Rust 测试，当前结果为 `188 passed, 0 failed, 1 ignored`

## 更新：会话 Todo 改为持久化并实时推送

- 修复（conversation-todo-persistence-and-sync）：补齐会话 Todo 的持久化、实时推送与首次快照恢复链路
  - 后端新增会话 Todo 原子更新方法，统一负责写入 `current_todos`、写盘并向前端推送 `conversation-todos-updated`
  - `todo` 工具调用时会立即把步骤列表写回当前会话，不再只停留在流式临时状态，刷新页面后也能恢复
  - 会话快照与前端状态同步补齐 `currentTodos` 数组链路，首次加载、切换会话与流式过程中都能拿到同一份 Todo 数据

## 更新：重构供应商配置页与多 Key / 多模型配置链路

- 重构（api-provider-config-refactor）：将 API 配置从单条 `apiConfig` 模式升级为“供应商 + 模型卡片”结构，并补齐迁移、轮询与配置页交互
  - 新增供应商级 `apiProviders` 结构，支持同一供应商下多 API Key 轮询、多个模型卡片独立配置，并在读取旧配置时自动迁移
  - 后端将真实发起网络请求后的 Key 选择改为供应商级轮询推进，模型列表刷新改为前端顺序尝试多个 Key，并补充相关测试
  - 配置页重做为供应商选择器 + 模型卡片编辑流，恢复文本/语音/向量三类切换、供应商预设链接助手、模型元数据回填与当前供应商级保存/回滚
  - 修复多项回归问题，包括旧配置误丢失、保存卡死、聊天输入区误出现模型选择器、滑条失效、删除确认缺失、文案与布局细节错误等

## 发布：v0.9.0

- 发布（release-0.9.0）：整理并发布当前这一轮聊天窗口、Markdown 渲染、流式输出与会话链路重构成果
  - 包含聊天窗口前端重构、代码块渲染方案切换、宽窗会话列表恢复、滚动与流式体验打磨等多项交互更新
  - 包含聊天热路径优化、启动快照整合、默认人格初始化收口、摘要快捷跳转等一批后端与跨窗口协同改进

## 更新：聊天流式输出改为更轻的透明渐显

- 调整（chat-streaming-fade-tuning）：收口聊天流式动画与结束收尾逻辑，突出更自然的逐字显现体验
  - 移除前端自定义流式缓冲层，恢复收到 delta 后直接写入草稿，避免 Markdown 流式渲染被二次节流打断
  - 将聊天消息的 typewriter 动画改为纯透明度渐显，不再使用位移或裁切效果，减少整段闪动感
  - 延长透明渐显过渡曲线，让流式输出更柔和，同时保持最终内容仍按原始流顺序即时落入消息草稿

## 更新：默认人格初始化只保留首次创建

- 重构（default-agent-init-on-first-create-only）：移除读取路径中的默认人格隐式修复，默认人格仅在首次创建 `AppData` 时初始化
  - 删除 `ensure_default_agent` 及其在聊天、归档、配置、任务调度、桌面工具等读取链路中的全部调用
  - 保留 `AppData::default()` 作为首次创建数据时的默认人格注入入口，后续常规读取不再顺手修复或写回
  - 清理围绕默认人格修复产生的冗余写回分支，避免热路径再次被隐式持久化逻辑污染

## 更新：启动阶段合并配置与人格初始化读取

- 优化（app-bootstrap-snapshot-init）：新增启动快照接口，减少启动阶段重复持锁与重复读取
  - 后端新增 `load_app_bootstrap_snapshot`，一次性返回配置、人格列表与聊天设置
  - 前端窗口初始化改为优先走启动快照链路，不再串行请求 `load_config`、`load_agents`、`load_chat_settings`
  - 保留原有独立读取接口作为局部刷新入口，降低改动风险并保持现有界面行为不变

## 更新：发送用户消息时前端即时回显

- 优化（chat-user-message-optimistic-draft）：发送后立即显示用户消息，消除发送瞬间的空窗感
  - 聊天发送链路新增本地 optimistic user draft，点击发送后用户消息会先在前端即时插入
  - 历史回流合并时会自动过滤并替换本地草稿，避免正式消息落库后出现双份用户消息
  - 停止发送、发送失败、会话冻结与前台清理等路径同步补齐草稿回收，避免残留临时消息

## 更新：聊天压缩分隔条支持一键查看当前会话摘要

- 新增（chat-compaction-summary-shortcut）：聊天消息中的压缩分隔条改为可点击的“查看摘要”入口
  - 将“上文已压缩”分隔条替换为 `history` 图标加“查看摘要”，减少额外按钮占位
  - 点击后会先定位当前未归档会话，再打开归档窗口并自动切换到“当前会话”视图
  - 归档页补充跨窗口焦点请求消费逻辑，确保窗口已打开、重新聚焦或首次挂载时都能正确落到目标会话

## 更新：重构聊天窗口前端与 Markdown 渲染链路

- 重构（chat-window-frontend-refactor）：重构聊天窗口标题栏、工具栏、消息区、输入区与会话侧栏布局
  - 重新编排宽窗与窄窗下的聊天窗口结构，恢复宽窗常驻会话列表，并收口标题栏按钮与工具栏入口
  - 调整输入面板、新建会话、会话切换、归档入口、滚到底部按钮与悬浮 todo 的展示位置和交互逻辑
  - 修复宽窗布局下消息区偶发消失、标题栏拖动失效、弹层层级不正确等一批前端窗口问题

- 调整（chat-markdown-rendering-refactor）：重构聊天 Markdown 渲染与代码块呈现方案
  - 聊天消息中的 `code_block` 节点改为通过 `setCustomComponents('chat-markstream', ...)` 覆盖为 `MarkdownCodeBlockNode`
  - 新增聊天专用 Shiki 代码块包装组件，移除 Monaco 路线，代码块固定使用暗色主题并稳定保留语法高亮
  - 修复 ````diff```` 代码块在新渲染链路下被错误改写成空的 `Plain Text` 块，改为直接按原始 diff 文本交给 Shiki 渲染
  - 优化 Mermaid 与宽消息气泡布局，收口消息区滚动、宽块占位与滚动到底按钮显示时机

- 调整（theme-system-cleanup）：精简主题系统并调整主题配置页展示
  - 默认主题改为 `business`，移除一批低质量或不再保留的 DaisyUI 主题选项
  - 主题选择面板改为按亮色组与暗色组分开展示，避免不同风格混排
  - 收口聊天代码块与应用主题的耦合，避免前后台切换或主题切换时代码块尺寸与配色抖动

- 扩展（conversation-todo-and-local-link-surface）：补充当前会话 todo 与本地文件链接相关能力
  - 前端补充当前会话 todo 展示所需的数据透出，支持在聊天区悬浮显示当前进行中的 todo
  - 聊天 Markdown 绝对路径链接支持直接打开本地目录并定位文件，保持原有外链打开行为不变

## 更新：聊天 Markdown 本地文件链接支持目录定位打开

- 新增（chat-markdown-local-file-open）：聊天消息中的 Markdown 绝对路径链接支持直接打开本地目录并定位文件
  - 前端聊天链接点击逻辑新增本地绝对路径识别，支持 Windows 盘符路径、UNC 路径与 Unix 风格绝对路径
  - 后端新增 `open_local_file_directory` Tauri 命令，文件路径会尝试在系统文件管理器中定位选中，目录路径则直接打开目录
  - Windows 下补充路径规范化与 `explorer /select,` 稳定传参，修复点击合法本地文件链接却意外打开桌面的情况
  - 原有 `http/https` 外链打开行为保持不变

## 更新：优化聊天界面 UI 样式统一

- 优化（chat-ui-style-unification）：统一聊天界面按钮与标签样式，提升视觉一致性
  - 将所有 `badge-outline` 改为 `badge-ghost`，弱化边框视觉效果
  - 将输入面板所有按钮从 `bg-base-100` 改为 `btn-ghost`，实现无边框幽灵按钮风格
  - 工作空间工具栏按钮统一为 `btn-ghost` 样式
  - 新增会话按钮图标尺寸增大至 `h-4 w-4`，提升视觉平衡
  - 输入框最小高度从 50px 降低至 32px，优化空间利用率
  - 优化输入框自动高度调整逻辑，确保最小高度限制生效

## 更新：优化 Markdown 渲染样式与聊天滚动体验

- 优化（chat-message-markdown-style）：重构 Markdown 内容样式规范，提升可读性与视觉一致性
  - 统一字体大小为 0.9rem，行高为 1.5，优化阅读舒适度
  - 调整段落间距为 0.25rem，保持紧凑同时保留空行效果
  - 标题、链接、加粗、引用、代码块、表格等元素样式统一收口至 ChatMessageItem 组件
  - 移除 ChatView 中重复的样式定义，避免维护负担
  - 优化链接样式，增加悬停效果与下划线间距
  - 代码块与内联代码增加边框与背景，提升可识别性
  - 表格样式优化，增加边框与间距

- 优化（chat-scroll-behavior）：优化聊天窗口滚动体验
  - 切换会话时使用自动滚动（auto），避免平滑滚动动画延迟
  - 跳转到底部时使用自动滚动，提升响应速度
  - 新增 `suppressNextAnimatedConversationScroll` 标志，精准控制滚动行为

## 更新：会话绑定部门与主会话固定

- 新增（conversation-bound-department-and-main-session-pin）：前台会话支持独立绑定部门，并修复主会话选择规则
  - 前台未归档会话新增 `department_id` 绑定字段，新建会话时可显式选择部门，创建后会话按该部门锁定模型与负责人
  - 聊天发送、停止、提示词预览、强制归档与压缩等前台主链路改为跟随“当前会话绑定部门”，不再一律使用全局主部门
  - 主会话缺失时不再从现有副会话中自动提拔新的主会话，而是始终新建一个绑定主部门的全新主会话
  - 未归档会话列表恢复以后端顺序为准，主会话固定置顶，避免被前端二次按时间重排冲掉
  - 新建会话弹窗增加部门原生下拉，选项显示为“部门 / 负责人”，便于快速选定会话归属
  - 会话列表与侧边栏同步优化：主会话直接显示标题“主会话”，时间移动到右上角，消息条数区域改为显示状态或未读数字胶囊，并统一按“今天/本周/今年/跨年”四档规则格式化显示
  - 前台未归档会话新增 `last_read_message_id` 已读锚点；当前会话不显示未读，切走/隐藏/最小化/关闭时立即结清已读，后台会话只显示数字未读胶囊

## 更新：工具循环改为重复调用熔断

- 调整（tool-repeat-fuse-instead-of-max-iterations）：移除前端“工具最大调用轮次”配置，改为固定防死循环规则
  - 前端工具配置页不再展示“工具最大调用轮次”，相关类型、快照、保存与事件回流链路同步移除该字段
  - 后端工具循环不再依赖前端配置的最大轮次，而是改为在单轮会话内跟踪“同工具 + 同参数”的连续调用
  - 当相同工具与相同参数连续出现 10 次后，第 11 次会被系统强制阻止，并把明确的失败结果回喂给模型，促使其调整参数或结束调用
  - 参数比较增加稳定化处理，JSON 参数即使 key 顺序不同，也会视为同一组参数

## 更新：运行日志支持复制并收口默认级别

- 调整（runtime-log-copy-and-level-cleanup）：优化运行日志弹窗与默认日志分级
  - 运行日志弹窗新增“复制”按钮，可按当前筛选结果一次性复制完整日志文本，不再受虚拟滚动限制
  - 运行日志弹窗默认级别从“全部”改为 `INFO`，避免打开时直接混入大量 `DEBUG`
  - `[会话锁]`、`[应用数据耗时]`、`[远程IM][联系人会话]`、`[工具调用]` 与 `[TOOL-DEBUG]` 等高频排障日志统一降为 `DEBUG`，减少默认视图噪音

## 更新：收口前端配置纠偏并修复部门模型空值语义

- 修复（config-boundary-and-department-empty-model）：继续清理前端擅自改写配置语义的行为
  - 前端配置加载、保存回流与事件回流链路改为更接近“原样接收后端结果”，不再用 `||` 默认值、自动夹紧或补默认选项覆盖后端返回
  - 部门模型选择支持显式“未配置”，新增部门不再自动补第一个模型，删除最后一个模型时也不再自动切回其它模型
  - 助手部门模型为空时，前后端统一保留空值语义；Rust 侧不再把无效或空的部门模型强行 fallback 到其它文本模型
  - 配置数值字段补充防护：`minRecordSeconds`、`maxRecordSeconds`、`toolMaxIterations`、`maxOutputTokens` 统一增加有限值校验、默认值回退与范围约束，避免 `NaN` 或空白值写入状态
  - API 页去掉基于模型元数据的自动回写，不再偷偷改 `contextWindowTokens`、`maxOutputTokens`、`enableImage`、`enableTools`
  - 前端常见网站列表移除已失效的 `iFlow` 入口

## 更新：聊天发送链路提速并收口默认日志

- 优化（chat-send-latency-and-log-reduction）：重构聊天发送主链路的锁与持久化路径
  - 全局 `state_lock` 拆分为 `conversation_lock` 与 `memory_lock`，聊天发送、调度、记忆召回不再共用一把大锁
  - 聊天发送与批量调度改为“短锁读快照 -> 锁外构建 prompt/召回/估算 -> 短锁提交”的结构，显著降低 `prepare_context` 前置等待
  - 应用数据后台持久化改为通过 `spawn_blocking` 承接阻塞文件 I/O，避免在异步运行时线程内直接写盘与读取 mtime
  - 修复 usage/token 缓存更新语义：成功且供应商未返回 usage 时才本地估算，失败与中断视为无事发生，不再错误清空旧缓存
  - 修复显式会话发送与直接激活链路中的竞态问题，避免出现“最后一条消息来自助理自身，无需重复激活”的误判

- 调整（chat-runtime-log-cleanup）：收口聊天运行期默认输出
  - 聊天耗时日志默认只保留中文关键阶段汇总，不再逐条输出大量内部 `stage` 明细
  - 移除提示词当前消息组装、记忆命中挂回、history_flushed 准备/成功、round_completed/failed 准备/成功等高频成功态日志
  - 保留慢会话锁日志、最终耗时汇总、状态转换和真实失败日志，方便继续排障但不再刷屏
  - 多处锁错误、MCP 与内置运行时日志统一改为中文文案，并补充锁名、文件位置与模块上下文

- 调整（new-conversation-reminder-instead-of-limit）：新建会话由数量限制改为记忆提醒
  - 前端不再因为未归档会话达到 8 个而禁用“新建会话”按钮
  - 新建会话弹窗新增提醒，明确“未归档的对话不会形成记忆，重要内容请及时归档或整理”
  - 同步补齐中英文与繁体文案，避免旧的“最多 8 个会话”提示继续误导用户

- 调整（client-identity-headers-for-ai-requests）：统一 AI 请求的客户端身份标识
  - 参考 Codex 风格，为主聊天链路统一补充稳定的 `User-Agent` 与 `originator`
  - 默认标识统一为 `p_ai_desktop`，避免不同请求路径对外暴露的客户端身份不一致
  - 共享 `reqwest` 客户端与 `rig` provider client 均接入该标识，覆盖 OpenAI/Gemini/Anthropic 主请求路径

- 重构（core-domain-file-split）：按职责拆分核心领域总文件
  - `src-tauri/src/features/core/domain.rs` 改为仅保留 `include!` 入口，常量、客户端标识、类型、运行态分别落到独立文件
  - 巨大的 `types.rs` 继续拆分为基础枚举、配置类型、请求类型、会话类型、存储类型 5 个子文件，降低后续导航和冲突成本
  - `runtime.rs` 再拆分为运行态类型、应用状态、缓存持久化、锁实现、默认人格与旧数据补齐 5 个子文件
  - 本次仅做结构整理，不改动现有业务语义，`cargo check` 已通过

## 更新：收口聊天耗时与内置 MCP 噪音日志

- 调整（log-noise-reduction-for-chat-and-mcp）：清理聊天链路中的低价值日志输出
  - 聊天耗时改为仅保留最后一条中文汇总日志，不再在运行阶段逐条输出 `stage=...` 调试行
  - 移除启动期常规 `[配置路径]` 输出，避免每次启动都打印目录选择细节
  - 收口内置 `read_file` / `operate` MCP 的开始与完成日志，仅保留失败和异常场景
  - MCP 自动重部署在无异常时不再打印“完成”提示，仅在存在错误时输出异常信息

## 更新：移除 DeepSeek/Kimi 旧协议通路

- 重构（remove-legacy-deepseek-kimi-protocol）：清理独立 `deepseek/kimi` 请求格式与运行时分支
  - Rust `RequestFormat` 移除 `DeepSeekKimi` 枚举与对应聊天、视觉、模型刷新、内存 Provider 的专用分支，统一回到标准 OpenAI 兼容链路
  - 删除 `src-tauri/src/features/chat/model_runtime/provider_and_stream/deepseek.rs`，不再维护旧协议专用 provider 实现
  - 前端 API 配置、部门模型过滤、类型定义与 Base URL 参考中移除 `deepseek/kimi` 选项，避免继续创建新配置
  - 为旧配置保留兼容映射：历史 `deepseek/kimi` 值在读取时自动按 `openai` 处理，避免升级后配置失效

## 更新：发布 0.8.9 版本

- 发布（release-version-0-8-9）：同步更新应用版本号并收口本轮上下文整理修复
  - `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 统一从 `0.8.8` 更新为 `0.8.9`
  - 聊天发送前的自动压缩判断改为只基于本次实际组装请求的 `estimatedPromptTokens`，不再使用错误的会话粗估口径
  - 清理会话粗估在撤回、停止、压缩预览等链路中的残留写入，避免错误 usage 再次回流主链
  - 压缩中提示改为无底色轻量说明，减少蓝色状态卡片带来的视觉干扰

## 更新：聊天图片改为前台轻载与按需懒加载

- 调整（chat-image-frontend-lazy-load）：聊天图片不再默认随前台消息链路下发 base64
  - `history_flushed` 事件发往前端前，会将图片 part 转为 `@media:` 引用，减少前台事件载荷体积
  - 聊天/归档等前端消息读取链路统一不再默认展开图片 base64，暂时仅保留音频沿用旧行为
  - 前端 `ChatMessageItem` 与归档视图改为按图片引用懒加载，并在内存中缓存已解析的数据 URL
  - 新增 `read_chat_image_data_url` Tauri 命令，供前端按需读取本地图片内容

## 更新：发布 0.8.8 版本

- 发布（release-version-0-8-8）：同步更新应用版本号
  - `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 统一从 `0.8.71` 更新为 `0.8.8`

## 更新：补充需求对齐与反馈非指令规则

- 调整（highest-instruction-alignment-guardrails）：系统准则补充需求对齐与反馈处理边界
  - `src/constants/highest-instruction.json` 新增“先对齐口径”，要求先确认目标、范围、约束与成功标准一致，再进入执行
  - 新增“反馈不等于指令”，明确当用户只是表达不满、指出问题、质疑结果或描述现象时，助手应先复述理解并对齐下一步，而不是擅自开工

## 更新：新增会话级 Todo MCP 工具与压缩摘要待办段

- 新增（conversation-todo-mcp）：新增会话级 `todo` MCP 工具
  - `Conversation` 新增 `current_todos` 字段，Todo 状态直接跟随会话持久化，不再单独维护运行时临时表
  - 新增 `src-tauri/src/features/system/tools/todo_mcp.rs`，按当前工具会话定位目标会话，并全量覆盖当前 Todo 列表
  - `todo` 工具返回改为 `## Current Todo List` 步骤板文本，使用 `✓ / → / ○` 标记已完成、进行中和未开始步骤；全部完成时追加“请向用户进行汇报”
  - 默认工具目录、运行时 MCP 挂载、前端工具目录与工具状态检查同步接入 `todo`

- 新增（todo-guide-and-board-prompting）：新增 Todo 专用系统提示词与会话快照注入
  - 聊天系统提示词新增固定 `todo guide`，明确 Todo 适用场景、`3~7` 步建议范围、单一 `in_progress` 规则与“全部完成后直接汇报”
  - 当前会话存在 Todo 时，聊天请求会额外注入 `todo board`，让模型直接看到当前步骤板而不是只依赖上下文回忆

- 新增（summary-context-current-todo-list）：上下文压缩摘要追加当前待办段
  - SummaryContext 压缩/归档链路在记忆相关段落后追加 `## Current Todo List`
  - 新建会话摘要种子与压缩消息都会带上当前会话 Todo，避免上下文压缩后丢失“当前做到哪一步”

- 测试（todo-mcp-and-compaction-regression）：补齐 Todo 关键回归
  - Rust 单测覆盖 Todo 覆盖写入、完成后自动清空与步骤板文本返回格式
  - Rust 单测覆盖压缩消息会在记忆块后追加 `## Current Todo List`
  - 工具目录测试覆盖前端 `todo` schema 与运行时 MCP 定义一致

## 更新：统一 SummaryContext、记忆RAG 与压缩消息结构标记

- 重构（summary-context-memory-rag-unification）：统一 SummaryContext、记忆 RAG 与用户画像链路
  - 归档与上下文压缩统一收敛为 `SummaryContext`，模型输出统一为 `summary/usefulMemoryIds/newMemories/mergeGroups/profileMemories` 五字段 JSON
  - 聊天主链路改为先写 `retrieved_memory_ids`，再在 prompt 组装阶段按消息延迟注入 `<memory_context>`，避免把记忆块直接揉进消息正文
  - 记忆存储新增 `memory_no` 与 `profile_memory_link`，支持用短 ID 管理记忆与用户画像记忆
  - 请求体预览新增 `chat / compaction / archive` 三模式，便于直接核对 SummaryContext 与聊天请求体
  - 同步补齐本轮重构的总览与模块计划文档，便于后续按实现回看设计边界

- 修复（compaction-message-structured-meta-and-seed-order）：压缩消息改为结构化标记并修正旧会话摘要种子位置
  - 压缩消息与摘要种子消息不再靠正文关键词识别，统一改为写入 `provider_meta.message_meta.kind`
  - 前后端只按结构化 `kind` 识别压缩类消息，不再因为正文里出现“上下文压缩/压缩摘要”等词组误判为压缩边界
  - 旧会话缺少摘要种子时，补种消息改为插入消息列表最开头，而不是追加到最后一条消息之后
  - 压缩消息的 `speaker_agent_id` 统一挂到系统人格 `system-persona / 凯瑟琳`，避免后续链路把它误当成用户本人发言

- 测试（summary-context-and-compaction-regression）：补齐关键回归
  - Rust 单测覆盖“普通消息提到上下文压缩但不得触发压缩边界”
  - Rust 单测覆盖“即使正文以 `[上下文整理]` 开头，只要没有结构化 meta 也不得认作压缩消息”
  - 前端类型检查覆盖 `providerMeta` 透传到 `ChatMessageBlock` 的链路


## 更新（v0.8.71）：优化发布构建与静态网页抓取请求头

- 调整（release-build-profile-tightening）：收紧发布构建产物体积与链接策略
  - `src-tauri/Cargo.toml` 新增 `release` profile，启用 `opt-level = "z"`、`lto = true`、`codegen-units = 1`、`strip = true` 与 `debug = false`
  - 新增 `.cargo/config.toml`，为 Windows MSVC 链接补充 `/OPT:REF` 与 `/OPT:ICF`，进一步裁剪无用符号与重复代码

- 调整（builtin-fetch-browser-like-headers）：内置 `fetch` 改为浏览器风格请求头
  - `src-tauri/src/features/chat/model_runtime/tools_and_builtin/builtin_network.rs` 为静态网页抓取补充完整 Chrome 风格 `User-Agent`
  - 同步增加 `Accept`、`Accept-Language`、`Cache-Control`、`Pragma` 与 `Upgrade-Insecure-Requests`，降低 `TopHub` 这类站点将抓取请求直接拦截为异常流量的概率

- 调整（rustfmt-sync-cleanup）：同步收口零散 Rust 格式化改动
  - `src-tauri/src/bin/litchi_probe.rs` 与 `src-tauri/src/main.rs` 仅保留 `cargo fmt` 带来的格式整理，不涉及行为变更

## 更新（v0.8.7）：聊天会话列表卡片与切换链路重构

- 调整（chat-conversation-overview-card-refresh）：聊天窗口改为弹出式会话列表卡片
  - 输入区上方原有紧凑会话条改为“新建 + 会话列表”入口，点击后以内联弹层展示全部未归档会话，减少横向挤压带来的辨识成本
  - 新会话列表卡片补充标题、主会话/当前会话标记、工作空间、最近更新时间、消息条数、运行状态与最近两条消息摘要，并支持展示最后发言人的头像或首字母
  - 会话列表支持点击外部关闭与 `Escape` 关闭，后台正在整理上下文的会话会被禁用，避免切换到不可操作状态

- 调整（chat-conversation-overview-push-sync）：未归档会话概览改为后端主动推送同步
  - Rust 侧新增未归档会话概览 payload，统一复用会话 summary 收集逻辑，并在新建、删除、归档刷新、轮次完成/失败等关键节点主动推送前端
  - 前端聊天窗口改为监听 `easy-call:conversation-overview-updated` 事件更新会话概览，并在当前前台会话丢失时按推荐会话或排序结果自动恢复
  - `switch_active_conversation_snapshot` 支持空会话 ID 回退解析，减少窗口激活、列表刷新与前台会话恢复之间的链路分叉

- 测试（chat-conversation-preview-regression）：补齐会话预览消息回归
  - 未归档会话 summary 新增工作空间标签与最近两条预览消息，支持文本、图片、PDF、语音、附件等摘要信息
  - Rust 单测覆盖最近两条消息提取与附件标记，降低后续继续演进会话列表卡片时的回归风险

- 调整（chat-conversation-sidebar-and-topic-create）：收口侧栏样式与主题化新建会话
  - 宽窗口下会话列表改为左侧常驻侧栏，主会话固定排在第一位；侧栏列表与弹出卡片分离，避免样式互相牵连
  - 左下角加号改为先弹出主题输入小窗，并展示最近使用的 7 个会话主题候选；确认后以主题直接创建新会话
  - 修复前台会话恢复竞态、归档链路概览推送持锁顺序与概览推送中文日志，补齐消息数本地化与头像 fallback 居中样式

## 更新（v0.8.6）：移除发布构建中的常驻 devtools feature

- 调整（tauri-release-devtools-feature-cleanup）：收紧 Tauri devtools 编译范围
  - `src-tauri/Cargo.toml` 移除 `tauri/devtools` 常驻 feature，避免 release 构建继续编入生产环境 devtools 支持
  - 保留现有开发态控制逻辑，`pnpm tauri dev` 下仍可按需通过开发环境开关打开 devtools

## 更新（v0.8.6）：内置抓取切换为静态网页正文提取

- 调整（builtin-fetch-static-trafilatura）：内置 `fetch` 改为静态网页正文抓取
  - `fetch` 工具接入 `rs-trafilatura`，默认按通用保守策略提取正文，优先过滤菜单、广告、推荐区等静态页面噪音
  - 提取参数收口为正文优先：启用 `favor_precision` 与表格保留，关闭评论、图片、链接与 Markdown 输出，避免抓取结果过于复杂
  - 当正文提取失败或结果为空时，仍自动回退到原有整页文本提取，降低异常页面上的功能回退风险

- 调整（builtin-fetch-description-refresh）：统一抓取工具文案
  - 内置 `fetch` 工具说明改为“静态网页抓取工具”，移除“优先使用其他抓取工具”的引导
  - 中英文与繁体本地化描述同步更新，确保配置页与运行时工具说明一致

## 更新（v0.8.6）：收口模型参数手动控制与下拉交互

- 调整（api-config-optional-manual-tuning）：温度与最大输出改为按需手动配置
  - API 配置页中的温度与最大输出均新增简化开关，关闭时禁用滑条，不再默认强迫用户覆盖模型侧最佳参数
  - 除 Anthropic 外，最大输出默认不手动下发；Anthropic 保持强制开启，避免上游请求缺少必要的输出上限
  - 前后端配置结构与请求组装同步改为“仅在开启时下发对应参数”，调试请求预览也与真实请求保持一致

- 调整（api-config-metadata-driven-token-sliders）：上下文窗口与最大输出改为跟随模型元数据
  - 最大输出不再继续展示固定“建议 4096”，上下文窗口与最大输出的中间刻度、上限显示统一改为动态跟随模型元数据
  - 选中并保存模型后，会自动同步模型能力元数据；旧默认值会优先收敛到可查询到的真实输出上限

- 修复（api-config-model-picker-toggle-behavior）：修正模型下拉收起后的误触发问题
  - 模型选择器改为显式开关控制，点按钮展开/收起，点外部关闭，避免收起状态下点击原位置仍被强制弹出
  - 搜索框与列表交互同步收口，减少配置页模型切换时的焦点干扰

## 更新（v0.8.6）：配置页多个标签改为卡片化布局

- 调整（config-tabs-card-layout-refresh）：统一配置页多个标签的卡片化视觉结构
  - API、聊天设置、快捷键、人格、部门、日志、MCP、记忆、技能等标签改为更清晰的 DaisyUI 卡片分组，减少长表单平铺带来的阅读压力
  - 按钮、未选中态和辅助操作区统一使用更一致的浅层背景与边框层级，收紧了区块间距与标题节奏
  - 快捷操作、缓存信息、能力开关等次级区域补齐容器化展示，配置页整体扫描路径更稳定

- 修复（config-tabs-api-template-cleanup）：清理 API 配置页模板结构异常
  - 修正 `ApiTab` 在美化过程中遗留的模板标签污染与闭合层级问题，避免模型下拉、按钮区和能力配置区域出现结构性渲染风险

## 更新（v0.8.6）：移除任务面板手工新建入口

- 调整（task-board-remove-manual-create-entry）：任务面板移除手工新建入口
  - 配置页任务面板头部不再显示“新建”按钮，避免用户被引导手工编写复杂任务
  - 任务说明与选择提示文案同步收口，不再继续强调“支持新建”或“或新建任务”

## 更新（v0.8.6）：新增全局运行上下文并开始渐进接入

- 新增（runtime-context-foundation）：新增统一运行上下文 `RuntimeContext`
  - Rust 侧新增 `RuntimeContext`，统一承载 `request_id`、`dispatch_id`、`origin_conversation_id`、`target_conversation_id`、`root_conversation_id`、`executor_agent_id`、`executor_department_id`、`model_config_id`、`event_source`、`dispatch_reason`
  - 新增最小 helper，用于统一生成 `request_id` 与基础上下文字段，避免不同链路继续各自猜测来源和目标 ID
  - `SendChatRequest` 与 `ChatPendingEvent` 均已支持携带 `RuntimeContext`，为后续渐进接入提供统一挂点

- 调整（runtime-context-key-paths）：优先接入高风险 ID 漂移链路
  - 用户 `send_chat` 入口会创建并向下游传递 `RuntimeContext`
  - 任务创建、任务调度投递、委托触发与委托结果回发已开始挂载 `RuntimeContext`
  - 聊天触发轮次与 LLM round log 会优先沿用已有上下文，不再只依赖散落的 `trace_id` / `conversation_id`

- 测试（runtime-context-regression）：补齐基础回归
  - 新增 `RuntimeContext` helper 单测，锁定 request id 选择规则与默认 `event_source / dispatch_reason`
  - 保留并验证“副会话发起任务仍回副会话”的回归测试，确保引入统一上下文后不破坏现有任务绑定链路

## 更新（v0.8.6）：简化任务追踪机制为按会话调度

- 调整（task-dispatch-by-conversation-slot）：任务调度改为按目标会话分组逐个派发
  - 不再维护全局 `current_tracked_task_id` 与复杂优先级竞争，改为“扫描到点任务后，按目标会话分组，每个会话只取最旧的一条未完成任务”
  - 若目标会话当前正在对话，或最后一条消息来自系统人格，则本轮跳过，避免重复提醒或插入冲突轮次
  - 联系人任务在原会话丢失时直接跳过；桌面任务在原副会话丢失时回退到桌面主会话

- 调整（task-target-scope-and-session-binding）：补齐任务目标会话范围与来源绑定
  - 任务数据层新增内部 `target_scope`，显式区分 `desktop` 与 `contact`，避免原会话丢失后无法判断是否应回主会话
  - 聊天里通过 `task` 工具创建任务时会保留来源会话，并兼容两段式 `agent::conversation` 与三段式 `api::agent::conversation` 工具会话标识
  - 修复副会话发起任务时会话 ID 被解析丢失、触发后误投主会话的问题

- 调整（task-board-remove-tracked-concept）：前端任务面板同步移除追踪态概念
  - 任务面板不再展示“当前追踪任务”，筛选项收敛为 `进行中 / 已完成 / 清空`
  - 列表与编辑卡文案一并收口，不再暴露“追踪中”徽章和相关状态语义
  - 隐藏任务板改为展示当前活跃任务摘要，而不是单个 tracked task

## 更新（v0.8.6）：任务字段瘦身为 goal / why / todo

- 调整（task-goal-why-todo-field-slimming）：统一任务主字段为 `goal / why / todo`
  - 任务对外模型、前端编辑表单、任务列表摘要与聊天任务触发卡统一改为 `goal / why / todo` 三字段
  - `goal` 兼任任务标题展示，`why` 用于保留方向约束，`todo` 用于表达当前下一步
  - 任务完成链路仅保留完成状态与完成结论，移除旧的主编辑字段 `title / cause / flow / todos / statusSummary / stageKey / appendNote`

- 调整（task-legacy-storage-and-tool-compat）：保留旧存储列并兼容旧工具入参
  - 数据库存储暂不强制迁列，继续兼容旧 `task_record` 字段，并在读写时自动映射到新三字段口径
  - `task` 工具 schema 改为主推 `goal / why / todo`，同时兼容历史 `title / cause / flow / todos / statusSummary / stageKey / appendNote` 入参
  - 隐藏任务板、任务触发 provider meta 与调度日志一并切换到新字段语义，减少模型继续沿用旧口径的机会

- 调整（task-skill-and-locale-alignment）：统一技能文案与界面词汇
  - `task-guide` 的推荐工作流、反模式和质量要求全部改写到 `goal / why / todo` 方案
  - 中英繁 locale 删除旧字段标签与占位词，任务面板、编辑卡和聊天触发卡的文案同步更新
  - 补充兼容解析，旧任务触发消息卡仍可回退读取旧字段，避免历史消息直接失去展示内容

## 更新（v0.8.6）：重构任务面板为列表 + 编辑卡，并引入日期时间选择组件

- 新增（task-board-editor-card-redesign）：任务面板重构为“列表 + 独立编辑卡”
  - 任务面板支持直接新建任务、点击列表项进入编辑，并移除旧的只读详情卡
  - 任务编辑改为 90% 窗口大小的独立卡片，编辑区与运行日志区改为互斥手风琴展示
  - 任务筛选区改为 DaisyUI `form.filter` 形式，支持清空筛选并按 5 条一页分页浏览

- 新增（task-editor-local-date-time-picker）：为任务调度字段新增本地日期时间选择组件
  - 新增 `TaskDateTimeInput`，左侧使用 `Cally` 选择日期，右侧使用原生时间输入与 `-10m / 现在 / +10m / 清空` 微调
  - 前端时间工具补齐 local RFC3339 组装、拆分、分钟微调与“现在”快捷值，继续保持任务工具参数使用当地时间口径
  - `Cally` web component 与 Vite 自定义元素配置一并接入，任务编辑卡中的开始时间、结束时间改为复用统一组件

- 调整（task-board-log-and-layout-cleanup）：收口任务日志展示与编辑卡布局
  - 任务页外层重复的“运行日志”卡移除，只保留编辑卡中的任务日志，避免信息重复和视线冲突
  - 编辑卡去掉多余标题栏，底部动作统一为删除 / 关闭 / 完成 / 保存，并收平字段布局与间距
  - 日志区按钮语义修正为“刷新日志”，不再错误地重载整个任务详情覆盖未保存编辑

- 新增（task-delete-command-and-editor-wire-up）：补齐任务删除链路
  - 后端新增 `task_delete_task` 命令，删除任务记录时同时清理对应运行日志并刷新当前追踪任务
  - 前端编辑卡新增删除按钮与确认提示，删除后会刷新任务列表并保持本地状态一致

## 更新（v0.8.6）：统一任务时间口径与 local/utc 语义

- 新增（task-time-semantics-local-utc-unification）：统一任务、计时与提示词时间语义
  - 后端新增统一时间语义模块，集中收口 UTC 存储、本地时间展示与 RFC3339 归一化转换
  - `task` 工具、任务提示板、provider meta、任务卡片与 `wait` 回执统一改为“参数用当地时间、反馈用当地时间、数据层与调度用 UTC”
  - 时间相关命名统一收敛到 `local / utc` 词汇体系，减少 `run_at`、`triggered_at`、`now_iso` 一类裸命名继续扩散

- 调整（task-time-storage-schema-and-compat-migration）：统一任务存储字段与兼容迁移
  - 任务存储模型、运行日志与进度注记显式拆分为 `*_local` 输出模型与 `*_utc` 存储模型，避免同一结构混装两层语义
  - `task_record`、`task_runtime_state`、`task_run_log` 的时间列统一收敛为 `_utc` 命名，旧列名继续兼容读取并在后续写入时收敛
  - 任务触发时间统一要求先从本地 RFC3339 转成 UTC 再入库，保证跨时区移动后仍能对齐同一真实时刻

- 调整（task-time-ui-skill-and-schema-alignment）：统一前端展示、Skill 文案与工具字段名
  - 前端新增统一时间展示 helper，任务页、聊天任务卡片与等待回执改为复用同一套本地时间格式化逻辑
  - `task` 工具 schema、前端类型与消息语义统一显式使用 `runAtLocal`、`endAtLocal`、`nextRunAtLocal`、`startedAtLocal`、`finishedAtLocal`
  - `task-guide` 与时间统一计划文档同步更新，统一 local/utc 命名、章节编号与 RFC3339 表述

- 修复（prompt-preview-latest-user-ordering）：修正请求体预览里的 latest user 顺序问题
  - 共享提示词组装改为仅当尾消息本身是 `user` 时才抽取 `latest_user`，避免历史里已回复的用户消息被挪到数组末尾
  - 请求体预览沿用统一消息组装回路，`latest_user` 为空时不再额外补一条空 user 消息
  - 补充回归测试，覆盖“尾消息是 assistant 时不抽 latest user”和“空 latest user 不出现在预览 JSON”两条场景

## 更新（v0.8.6）：统一桌面脚本工具与 MCP 定义对齐

- 新增（desktop-script-operate-mcp）：将桌面操作统一收敛为 MCP 版 `operate` 脚本工具
  - `operate` 改为一次接收 `script` 多行脚本，统一支持 `mouse`、`key`、`text`、`wait`、`screenshot`
  - 鼠标与截图区域统一使用 `0~1` 百分比坐标，截图只向模型保留最新一张，可用 `save="绝对路径"` 额外落盘
  - 桌面脚本解析、原子动作执行、结果汇总拆分为独立模块，补齐脚本参数校验与结构化错误提示

- 调整（desktop-tools-merge-and-mcp-only-exposure）：收口桌面工具暴露策略
  - 模型侧不再单独暴露旧 `screenshot` 工具，统一并入 `operate`
  - `operate` 与 `read_file` 均通过 MCP 形态挂载，前台工具目录不再手写第二套说明
  - 前台工具目录改为直接读取真实 MCP `list_all_tools()` 定义，确保用户看到的说明与模型拿到的定义完全一致

- 修复（operate-catalog-logging-and-multi-monitor-region）：补强目录日志与多显示器区域截图
  - 前台工具目录补齐 MCP client 取消失败与定义加载失败日志，避免 `.ok()` / 丢弃 `Result` 导致排障信息丢失
  - `operate` MCP 服务与执行层补齐中文 INFO 日志，统一记录开始、逐步完成、完成统计与关键计数
  - 区域截图坐标换算补齐显示器偏移，修正多显示器场景下 `region` 截图位置错误
  - `Enigo` 初始化失败等核心错误文案改为中文，保持日志口径一致

- 测试（operate-script-and-mcp-catalog-regression）：补齐桌面脚本与目录一致性回归
  - 新增脚本解析测试，覆盖键盘、文本、归一化坐标、截图区域与绝对路径校验
  - 新增多显示器区域偏移测试，覆盖 `normalized_region_to_screen`
  - 新增前台工具目录与真实 MCP 定义完全一致的回归测试，防止说明文案再次漂移

## 发布（v0.8.6）：统一 PDF 图片分页、后台归档与多图转发

- 修复（openai-responses-system-message-user-fallback）：修正 OpenAI Responses 在上游拒绝 system message 时的降级链路
  - 运行态新增按 `base_url` 记忆的 `system -> user` 降级缓存，命中 `System messages are not allowed` 后同次运行内后续请求会直接改写提示词
  - Responses 聊天主链路提取公共重试逻辑，统一处理标记降级、移动 system prompt、刷新请求日志与重试发送
  - 请求预览与调试日志不再为已清空的 `preamble` 生成空的 `system` 消息，避免排查时误判降级未生效
  - OpenAI / OpenAI Responses 的 rig builder 在 `preamble` 为空时不再调用 `.preamble(...)`，避免空 system prompt 继续被上游识别为 system message

- 修复（background-force-archive-and-organizing-session-guard）：强制归档改为后台执行并禁用归档中的会话切换
  - 强制归档入口改为先切出当前会话，再后台启动归档任务，不再长时间占用前台聊天窗口
  - 未归档会话列表新增运行态字段，当前处于 `organizing_context` 的会话会在聊天视图与归档视图中显示为禁用，避免归档过程中误切回
  - 前后端切换会话命令补齐运行态校验，正在后台归档或整理上下文的会话会拒绝切换
  - 后台归档任务补齐 panic 兜底，异常路径会主动恢复会话运行态到 `Idle`，避免会话卡死在整理上下文状态
  - 归档视图的“后台归档中，暂时不能切换”提示改为走 i18n 词条，补齐中英繁 locale

- 修复（read-file-pdf-image-pagination-and-multi-image-forwarding）：统一 `read_file` 的 PDF 图片分页与多图转发链路
  - `read_file` 读取 PDF 时，改为跟随现有 `pdf_read_mode` 与当前聊天模型 `enable_image` 自动决定走文本提取还是图片提取，不再固定只走文本模式
  - PDF 图片结果新增 `offset/limit` 分页支持，图片模式下按页偏移返回多页图片，并补齐 `nextOffset`、`returnedPageCount`、`returnedImageCount`、`totalPages` 等元信息
  - `read_file` 工具描述同步更新：`offset/limit` 仍用于分页，文本结果延续现有文本续读语义，PDF 图片结果按页继续读取
  - 工具结果缓存与转发链路补齐多图支持，避免 PDF 图片模式返回多页时只把首张图片继续转发给模型
  - 补充 Rust 测试，覆盖 PDF 图片分页结果与多图工具结果转发

- 修复（chat-stream-render-isolation-and-jump-button-anchor）：切分消息渲染热路径并修正滚到底按钮定位
  - 聊天消息列表改为由独立 `ChatMessageItem` 子组件承接单条消息渲染，配合父层 `v-memo`，避免流式输出时整列表陪跑重渲染
  - 单条消息渲染补齐时间格式化器复用与流式 Markdown 解析节流，显著降低长输出时的主线程压力
  - “滚到最下”按钮改为根据输入面板高度动态上浮，并修正底部状态响应式更新，避免按钮常驻与遮挡输入区

- 调整（skill-setup-trigger-clarification）：收紧 skill-setup 的触发规则
  - `skill-setup` 的描述与规则改为“显式 skill 查询触发 + 能力缺口触发”的双触发口径
  - 当用户明确询问 skill/技能/插件/扩展/市场/热门 skill 等意图时，模型应优先读取 `skill-setup`，避免先直接搜索网页或直接回答

- 修复（chat-auto-disable-image-on-unsupported-endpoint）：遇到明确不支持图片输入的模型端点时自动关闭图片模态
  - 当模型请求返回明确的 image input unsupported 错误时，聊天重试链路会自动将对应 API config 的 `enable_image` 持久化写回为 `false`
  - 自动关闭后，同一候选模型的后续重试会按无图能力继续请求，减少重复触发同类 404/不支持图片输入错误

- 调整（prompt-xml-blocks-and-skill-discovery-guidance）：统一提示词包裹格式并重写 skill 发现说明
  - 系统提示主路径新增统一 XML block 包裹函数，收口系统准则、部门上下文、技能索引、任务板、委托任务、终端工作区、远程 IM 规则等提示块格式
  - 聊天基础栏目不再混用 `#`、`##` 与 `[HIDDEN ...]` 标题，改为独立并列的 XML block，减少块内 markdown 与外层提示冲突
  - skill 提示统一改用 `System skill directory path` 术语，不再把 skill 目录与工作空间概念混写
  - `skill-setup` 重写为“发现 / 安装 / 自己制作”三段式说明，并补充实测可用的 `clawhub search / inspect / install --workdir` 用法
  - 远程 IM 联系人规则收缩为身份边界与发送工具要求，不再在提示词中堆叠字段级细节

- 重构（message-semantics-unification-and-tool-call-protocols）：统一消息语义层并收口多协议工具历史回放
  - 后端新增统一消息语义层，集中解释 `ChatMessage` 中的 `tool_call` sidecar，并统一生成聊天/归档可复用的 Prompt 历史消息
  - 聊天 Prompt、归档 Prompt 与归档 markdown 导出改为复用统一语义解释，不再分别手写工具历史展开逻辑
  - 前端新增统一展示语义模块，聊天消息块、归档工具摘要与会话撤回 patch 判断改为消费统一投影结果
  - 工具历史回放显式区分 `invocation_id` 与 `provider_call_id`，统一处理 OpenAI Chat、DeepSeek、OpenAI Responses、Gemini、Anthropic 四类协议差异
  - OpenAI Responses 缺少必要 `call_id` 时统一降级为文本历史；其他 Chat-like 协议保留结构化回放，避免再次出现“一刀切”降级
  - 补充 Rust 与前端测试，覆盖统一语义层、归档工具历史展开、多协议 tool replay 与前端展示/撤回辅助逻辑

- 修复（archive-summary-tool-history-shape）：修正归档摘要请求中的工具历史结构
  - `PromptBuildMode::Archive` 不再把整段 `tool_call` 事件列表误塞进 `PreparedHistoryMessage.tool_calls`
  - 归档/上下文整理摘要会像聊天主链路一样展开为独立的 assistant/tool 历史消息，避免 OpenAI 兼容非流式请求携带非法 `tool_calls` 结构
  - 补充归档历史组装测试，覆盖 `tool_calls.id/type` 必须存在的严格校验场景

- 清理（active-chat-view-binding-unused-channel）：移除前台聊天流绑定中的未使用字段
  - 删除 `ActiveChatViewBinding` 中未被读取的 `on_delta` 字段，收敛活动会话绑定结构
  - 保留绑定命令入参兼容，避免仅为清理 warning 扩大前后端调用面变更

## 发布（v0.8.4）：统一远程 IM 激活来源自动发送并补强消息链路回归

- 前后端版本号统一升级到 `0.8.4`
- 修复（remote-im-activation-source-unified-auto-send）：统一远程 IM 激活来源自动发送链路
  - 本轮调度新增运行态 `remote_im_activation_sources`，统一记录触发本轮的远程 IM 来源，不再依赖会话类型猜测外发目标
  - 当且仅当本轮由唯一一个远程 IM 来源激活，且模型未显式调用 `remote_im_send` 时，系统才会在回合结束后自动发送最终回复
  - 当本轮存在多个远程 IM 激活来源时，系统明确禁止自动外发，并在运行时提示词中要求模型显式调用 `remote_im_send`
  - 运行态存在远程 IM 激活来源时，会强制挂载 `remote_im_send` 工具，避免前台/后台链路遗漏外发能力

- 修复（remote-im-auto-send-decision-writeback-and-prompt-payload-guard）：补强自动发送回写与消息组建回归验证
  - 自动发送成功后会把 assistant 消息中的 `remoteImDecision.action` 从 `send_async` 回写为 `send`，失败时写为 `send_failed` 并保留错误信息
  - 回写 `remoteImDecision` 时不再覆盖既有 `conversationKind`、`processingMode` 与 `activationSourceCount`，避免状态字段被自动发送流程抹掉
  - 为远程 IM 发送适配器增加仅测试使用的 mock send/mock error 通道，补齐成功/失败两条无需外网的回归测试
  - 补充 DeepSeek 实际发送路径测试，确认压缩后的首轮 latest user 正文与 metadata 会一起保留，不会被组装成空消息

- 修复（prompt-preview-message-unification-and-dialog-resize）：统一请求预览消息构建并放大预览弹窗
  - 请求预览不再单独手拼 request body，改为复用与发送日志相同的 `messages` 构建入口，避免预览消息数组与实际发送链路不一致
  - 预览命令开始真正读取传入的 `apiConfigId`、`agentId` 与 `conversationId`，不再默认忽略当前上下文
  - 请求预览弹窗扩大到接近窗口 `90%`，并让文本框自动填满剩余区域，提升长消息数组的查看体验

- 发布（v0.8.3）：收口远程 IM、工作空间迁移与聊天前台同步修复
  - 前后端版本号统一升级到 `0.8.3`
  - 收口远程 IM snake_case 字段统一、联系人异步发送决策修正、默认工作空间迁移，以及聊天前台会话同步与路由保护

- 修复（remote-im-snake-case-and-async-decision）：统一远程 IM 联系人字段命名并修正异步发送决策
  - 远程 IM 会话 origin、`remote_im_send` 工具结果与联系人列表统一收口为 `snake_case` 字段，避免 `channelId/contactId` 与实际远端标识混淆
  - 联系人会话 prompt 与工具规则明确要求发送时使用 `action=list` 返回的 `contact_id`，不再误用联系人记录 UUID
  - 联系人会话兜底自动发送改为先标记 `send_async`，后台成功后再回写为 `send`，失败则落库为 `send_failed`
  - DeepSeek 工具循环补齐 `stop_tool_loop` 判定，远程 IM 完成发送后可及时结束后续工具链

- 修复（workspace-default-migration-and-window-shell-polish）：补齐默认工作空间迁移与窗口头部交互收尾
  - 默认 shell 工作空间补齐旧路径迁移、内置项修正与配置持久化，避免升级后工作空间指向漂移
  - 沙箱允许目录解析同步纳入默认工作空间归一化，保持终端与沙箱边界一致
  - 聊天窗口头部改用 Tauri 原生拖拽区域，并拆分聊天偏好自动保存，减少误触发与多余写盘

- 修复（chat-foreground-sync-and-routing-guard）：收紧前台会话同步并禁止单次请求静默串会话
  - 聊天窗口前后台激活同步增加防抖与前台会话清理逻辑，减少最小化、切换焦点时的残留状态
  - 前端流式轮次补齐 queued 阶段终态缓存，避免完成/失败事件早于 streaming 进入时丢失
  - 指定 `conversation_id` 的单次消息读取与分页在会话不存在时改为直接报错中止，不再静默回退到其他会话
  - 未归档会话列表刷新改为优先增量同步，收敛聊天主窗口的列表刷新与缓存写入成本

- 修复（remote-im-channel-create-and-weixin-login-save-flow）：优化渠道创建交互与微信扫码前保存流程
  - 配置页“新增渠道”改为先弹出平台列表，由用户直接选择 OneBot v11、飞书、钉钉或个人微信后再创建对应渠道
  - 新建渠道时按所选平台填充默认名称，并自动打开该渠道配置弹窗，减少后续切换步骤
  - 个人微信点击扫码登录前若当前渠道仍有未保存改动，会先自动保存一次；保存失败时中止扫码并提示用户
  - 补充中英繁平台选择相关文案，保持新增渠道弹层在多语言下行为一致

- 调整（built-in-deputy-and-front-desk-departments）：新增不可删除的副手与前台部门，并将远程 IM 默认绑定到前台
  - 在主部门之外新增两个内建部门：`副手` 与 `前台`，二者与主部门一样会在配置归一化时自动补回，且不可删除
  - `副手` 默认职责强调严格不越权、不擅自扩展需求、以最少工具调用快速完成上级明确派发任务
  - `前台` 默认职责专注承接远程 IM 消息，要求简短友好回复，并将复杂任务转交主部门
  - 远程 IM 新联系人默认处理部门改为 `前台`，让远程入口默认走前台链路而不是主部门

- 调整（remote-im-channel-logic-reshape-and-onebot-naming）：远程 IM 渠道代码按业务重组并统一 OneBot v11 命名
  - 将 OneBot v11 渠道主实现从 `napcat_ws` 命名统一收口为 `onebot_v11_ws`，避免把 NapCat 实现名误当成协议口径
  - OneBot v11 转发节点发送者名提取改为统一 helper，按 `nickname -> card -> user_id` 顺序解析，并补充对应测试
  - `dingtalk_stream`、`onebot_v11_ws`、`weixin_oc` 三个超大渠道文件改为按业务职责拆分，区分运行时、登录/生命周期、消息解析、媒体处理等模块
  - 飞书适配器从 `remote_im_adapters.rs` 独立抽出为单独文件，收敛到 `remote_im/` 目录下，统一各平台代码归位方式

- 发布（v0.8.1）：收口远程 IM 渠道结构与内建部门默认编排
  - 前后端版本号统一升级到 `0.8.1`
  - 收口 OneBot v11 命名、渠道业务拆分、飞书独立归位，以及副手/前台内建部门默认编排

- 修复（weixin-oc-multimedia-ingest-and-archive-links）：收口个人微信多媒体入站、去重与归档展示
  - 个人微信入站补齐图片、语音、文件、视频的媒体下载与解密流程，统一先作为附件入队；图片与语音继续额外作为多模态输入提供给模型
  - 个人微信长轮询改为每轮重新读取最新渠道凭证并按 `platform_message_id` 去重，修复重复拉取导致的消息洪泛
  - 主部门链路补发全局 `round-completed` / `round-failed` 事件，修复前端已结束但仍显示“助理循环中”的卡住状态
  - 归档页联系人消息删除改为“清空会话”而非删除联系人配置，并为归档消息新增附件链接打开能力
  - 个人微信联系人显示名改为优先使用渠道名，移除冗余的 `[图片]` 文本占位，避免界面重复展示媒体内容
  - 修复保存配置时误清空 STT 模型选择的问题，仅在前端明确删除对应 API 配置时才清空 STT 绑定

- 发布（v0.8.0）：完成个人微信接入与记忆看板提示收口
  - 前后端版本号统一升级到 `0.8.0`
  - 收敛个人微信扫码接入、配置页收尾与 MemoryBoard 提示补充，作为当前发布版本

- 新增（remote-im-weixin-oc-login-and-ui-polish）：完成个人微信扫码接入与配置页收口
  - 远程 IM 新增个人微信渠道接入，打通扫码登录、长轮询收发与渠道重启后的运行态恢复
  - 配置页改为手动保存流，补齐渠道/联系人设置弹窗、联系人发送文件权限与日志查看等收尾交互
  - 个人微信相关界面不再向用户暴露微信侧标识、联系人原始 ID 与扫码原始链接，统一改为安全状态文案

- 修复（responses-tool-id-replay-and-remote-im-platform-fallback）：修正 Responses 工具历史回放 ID，并放宽远程 IM 平台值读取
  - OpenAI Responses 工具历史回放改为严格区分 `id` 与 `call_id`，旧式缺少 `call_id` 的工具历史不再伪装为结构化 function call，避免因把 `call_*` 当作 `fc_*` 发送而触发 400
  - 远程 IM 渠道 `platform` 反序列化改为宽松模式，未知值统一按 `onebot_v11` 处理，避免单个旧值或手写值导致整份 `app_config.toml` 解析失败

- 调整（remote-im-contact-conversation-naming）：远程 IM 会话语义统一改称“联系人会话”
  - 前后端显示文案、日志与核心函数命名统一从“隐藏会话/隐藏线程”收敛为“联系人会话”
  - 联系人专属会话继续保持“渠道 + 联系人”唯一映射，避免名称误导为纯 UI 隐藏能力

- 修复（tool-error-surface-hardening）：统一工具失败回传格式并修正误导性状态日志
  - 所有内置工具与 MCP 工具在执行失败时，统一向模型回传结构化失败结果，明确包含 `ok=false`、工具名与错误原因
  - 工具状态日志不再把前端事件投递成功误写成工具执行成功，改为明确记录“事件投递结果”
  - 降低工具失败被模型忽略或被日志误判为成功的风险，覆盖 `remote_im_send` 等失败路径

- 修复（task-conversation-routing-fallback）：任务调度改为优先原会话，失效再回主会话
  - 任务数据新增 `conversation_id` 持久化字段，创建任务时会绑定当前发起会话
  - 任务触发与排队恢复时优先投递到任务原会话；原会话已归档、消失或不可用时，才回退到当前主会话
  - 补充任务存储与会话回退测试，防止多会话并发场景下任务再次串会话

- 修复（unarchived-message-fallback-restore）：恢复未归档消息读取在会话缺失时的安全回退
  - `get_active_conversation_messages`、`before`、`after` 重新统一为“指定会话优先，缺失则回退到最新未归档前台会话”
  - 补充未归档会话索引回退测试，避免前端历史加载因旧会话 id 失效而直接失败

- 调整（hide-legacy-deepseek-protocol）：前端隐藏 `deepseek/kimi` 协议选项
  - API 配置页不再对新配置暴露 `deepseek/kimi` 协议选择入口
  - 已存在的旧配置仍保留展示与兼容，避免用户存量配置被强制改写

- 发布（v0.7.2）：同步版本号以承接协议与提示词格式修正
  - 前后端版本号统一升级到 `0.7.2`
  - 便于区分 `deepseek/kimi` 协议隐藏与 MemoryBoard 系统提醒格式修正后的构建

- 修复（multi-conversation-send-routing）：移除多会话发送对单 active 状态的旧依赖
  - 发送消息时不再因目标会话 `status=inactive` 被后端静默回退到其他会话
  - 普通未归档前台会话统一保持 `active`，避免旧单会话时代遗留状态继续干扰并发会话发送
  - 补充会话状态归一化回归测试，覆盖旧数据中 `active/inactive` 混存场景

- 调整（stream-fallback-runtime-only）：流式失败后的非流式回退改为仅当前进程生效
  - 供应商流式失败后仅在内存中标记当前 base URL 本次运行内改走非流式，不再写入配置持久化
  - 聊天与视觉相关日志同步改成“本次运行内切换非流式”，避免将临时回退误解为永久策略

- 调整（archives-config-window-controls）：归档与设置窗口补齐标准窗口按钮
  - 归档窗口与设置窗口头部统一补齐最小化、最大化/还原与关闭按钮
  - 所有主窗口头部右侧统一采用三大窗口控制按钮布局，交互保持一致

- 调整（chat-window-controls-layout）：重排聊天窗口头部控制按钮
  - 钉住窗口按钮移到左侧工具区，不再占用右上角窗口控制位
  - 聊天窗口右上角补齐最小化、最大化/还原与关闭三大窗口按钮
  - 启动时同步窗口置顶与最大化状态，保证头部按钮状态与真实窗口一致

- 发布（v0.7.1）：更新应用版本号并准备发布标签
  - 前后端版本号统一升级到 `0.7.1`
  - 同步更新桌面应用打包版本与 Rust 包版本，便于后续发布与追踪

- 新增（single-instance-guard）：应用启动改为单实例守护
  - 接入 Tauri 单实例插件，重复启动时不再创建第二个应用进程
  - 当用户再次启动应用时，自动激活现有实例并优先拉起当前聊天窗口，否则回退显示主窗口

- 调整（assistant-bubble-full-width）：助理消息气泡统一改为吃满可用宽度
  - 移除助理消息外层额外的 `max-w-[92%]` 限制，统一改为 `max-w-full`
  - Mermaid 消息继续保持宽气泡策略，并随助理消息容器一起铺满当前布局可用宽度

- 修复（conversation-scrollback-restore）：恢复并发会话切换后的上拉历史加载
  - 修复切换到并发会话时前端错误清空 `hasMoreBackendHistory`，导致滚动到顶部后不再触发更早消息加载的问题
  - 会话前台切换改为根据当前缓存快照保留“可能仍有更早历史”的状态，异步补新消息时也不再覆盖该标志

- 重构（markdown-render-markstream-only）：聊天 Markdown 渲染统一收口到 `markstream-vue`
  - 移除旧的 `markdown-it`、`@mdit/plugin-*`、`DOMPurify`、`twemoji` 与手工 Mermaid 二次扫描链路，避免多套渲染体系并存导致的样式与性能问题
  - 聊天消息正文统一仅通过 `markstream-vue` 的节点解析与流式渲染输出，继续保留公式与 Mermaid 的显示能力
  - 代码块与 Mermaid 工具栏收口为轻量展示模式，仅保留基础头部与复制能力，关闭预览、展开、导出、全屏、缩放等非必要交互

- 修复（ui-scrollbar-and-markdown-theme）：统一滚动条预留策略并让聊天 Markdown 跟随 DaisyUI 主题切换
  - 全局滚动容器统一使用 `scrollbar-gutter: stable both-edges`，减少内容区在滚动条出现时的挤压与覆盖
  - 聊天 Markdown 渲染改为根据当前 DaisyUI 主题向 `markstream-vue` 显式传入亮暗模式，避免渲染层停留在独立的白天/夜间样式

- 修复（terminal-utf8-runtime）：补齐 Windows 终端拉起链路的 UTF-8 注入并为历史编码输出增加兜底解码
  - Git Bash 与 PowerShell 的终端包裹命令统一补充 `PYTHONUTF8=1`、`PYTHONIOENCODING=utf-8`，减少 Python 跟随系统 `cp936/gbk` 输出乱码
  - Windows 终端进程启动时统一注入 `LANG`、`LC_ALL` 与 Python UTF-8 环境变量，收敛 live session 与单次执行链路的编码行为
  - 终端输出展示改为 UTF-8 优先解码，并在 Windows 下结合系统代码页、`chardetng` 自动检测与 GBK 兜底，降低旧 CLI 或脚本残留乱码概率
  - 新增终端编码相关回归测试，覆盖 PowerShell / Git Bash UTF-8 注入、Windows GBK 输出兜底解码与 Windows-1252 标点自动识别

- 调整（fixed-model-retry-policy）：统一聊天模型失败重试策略并移除用户配置项
  - 聊天请求在同一驱动模型内固定重试 `3` 次，每次等待 `5` 秒，不再区分空回复、`429` 或其他错误类型
  - 同模型重试期间继续通过前端反馈渠道显示“正在重试”状态；同模型最终失败后，仍保留切换到下一个候选模型的提示
  - 配置页移除“失败重试”滑块与相关说明文案，不再向用户暴露该运行时参数
  - 后端运行时不再读取 `failure_retry_count` 作为聊天重试依据，统一由固定策略驱动

- 升级（v0.7.0-memory-rag-overhaul）：统一自动记忆检索挂点并完成记忆检索口径收敛
  - 自动记忆 RAG 从旧的“写消息分支”迁移到“消息组建节点”，改为基于“上一次助理消息之后的全部用户内容”统一检索，再注入当前轮次 prompt
  - 长文本查询压缩下沉到统一检索内核，自动 RAG 与左侧记忆搜索继续共用同一套检索核心，避免入口层分叉
  - 命中 `tags` 的长查询提取新增大小写无关去重，避免 `Apple` / `apple` 等等价词元重复进入检索表达
  - 记忆整理提示词与 `remember` 工具字段说明同步收紧，明确 `tags` 必须是独立、紧凑、稳定、可检索的词元
  - 为消息组建节点补齐 `[记忆RAG]` 运行日志，便于直接观察检索 query、命中 memory ids 与最终注入内容

- 优化（memory-tag-recall）：收紧长文本自动回忆查询并补强记忆标签说明
  - 当用户当前发言超过 `100` 字时，自动回忆优先从现有记忆 `tags` 中提取命中词元作为查询，降低长文本噪声对检索的干扰
  - 若当前长文本未命中任何已有 `tags`，则继续回退到原有全文查询行为，保持现有回忆链路兼容
  - 记忆整理提示词补充 `tags` 约束：必须是独立、紧凑、稳定、可检索的词元，禁止整句、拼接短语与多语义混写
  - `remember` 工具字段说明改写为语义化中文描述，明确 `judgment`、`memory_type`、`reasoning`、`tags` 的职责，并移除易误导的废弃类型提示

- 测试（read-file-test-cleanup）：移除依赖外部样本文件的 `read_file` 测试
  - 删除 `read_file` 中依赖仓库外部 `data/` Office / PPT 样本文件的测试用例
  - 保留其余无需外部样本的单元测试，避免测试与本机或其他项目目录耦合

- 调整（apply-patch-absolute-path）：`apply_patch` 统一为绝对路径输入
  - `apply_patch` 路径解析改为只接受绝对路径，并继续校验路径必须落在当前工作区内
  - 同步更新 `apply_patch` 撤回链路与解析测试，统一使用绝对路径补丁输入
  - 新增计划文档 `plan/20260324_apply_patch绝对路径统一计划.md`

- 调整（tool-copy-alignment）：统一工具说明中文文案并校准 `apply_patch` 约束
  - `apply_patch` 工具说明改为中文规则式描述，移除示例，明确要求绝对路径
  - 补充 `Update File` 必须先写 `@@` hunk 头的说明，并明确不接受完整 git diff 头
  - `fetch`、`websearch`、`task`、`exec` 等工具的可见说明与参数描述统一为中文
  - `read_file` 参数说明移除本机路径示例，截图工具目录不再暴露 `webpQuality` 参数

- 新增（read-file-mcp-experimental）：新增实验性 `read_file` MCP 工具并修正图片读取与会话模型选择
  - 新增 `read_file` 工具，支持文本、图片、PDF 与部分 Office 文件读取，文本结果统一按 `offset/limit` 分页并限制在 30000 字符内
  - `read_file` 运行时改为通过内建 MCP 服务器注册，不再直接以内建 tool 形式挂载，图片结果协议对齐截图工具链路
  - Office 读取实验性接入 `litchi`，补充 `litchi_probe` / `undoc_probe` 验证入口，并对 `.ppt` panic 做普通错误降级
  - 修复主聊天发送链路未优先使用 `session.api_config_id` 的问题，避免当前会话已选视觉模型时仍错误走图转文回退
  - 补齐 `read_file` / MCP 启动与执行日志，统一为中文诊断字段，便于排查路径、reader、耗时与错误详情

- 调整（chat-header-archives-shortcut）：聊天窗口左上角新增归档窗口快捷入口
  - 聊天窗口左上角在设置按钮旁新增归档图标按钮
  - 点击后直接打开现有归档窗口，复用既有 `show_archives_window` 链路
  - 头部入口保持图标化，不新增文字按钮

- 新增（remote-im-contact-thread-beta）：远程 IM 联系人会话测试版（尚未完善）
  - 联系人支持按处理部门自动路由：主部门默认进入主会话，非主部门默认进入该联系人的独占联系人会话
  - 联系人会话归联系人持有，切换处理部门时保留原有联系人会话历史，不再因为路由切换清空会话绑定
  - 后端新增联系人会话查询与消息查看接口，归档窗口新增“联系人消息”页签用于查看联系人会话
  - `remote_im_send` 新增 `no_reply`，联系人会话强制要求模型通过回复工具做出 `发送/不回复` 决策
  - 联系人处理模式已接入运行时：`无上下文` 对应一问一答，`有上下文` 对应自动管理上下文
  - 前端联系人高级设置已改成当前规则导向展示，但整体仍是测试版，交互、命名与边界处理后续还会继续收敛

- 修复（archive-force-target）：强制归档不再错误要求目标会话必须处于 active 状态
  - 修复 `archive_conversation_now` 仅允许 `status == active` 会话归档的问题
  - 现在强制归档会按明确传入的 `conversationId` 执行，只要该会话仍未归档即可正常归档
  - 修复多未归档会话场景下，预整理已成功写回但最终仍误报“活动对话已变化，请重试强制归档”的问题

- 文档（plan-archive）：归档会话级并发调度重构计划并修复前端类型校验
  - 将 `plan/20260322_会话级并发调度重构计划.md` 按最新实现改写为归档报告，并迁移到 `plan/done/20260322_会话级并发调度重构归档.md`
  - 归档内容确认后端主聊天第一步“会话级并发调度”已经落地完成，可作为后续架构演进稳定基线
  - 修复 `use-chat-flow.ts` 中轮次失败日志对联合类型的错误访问，恢复 `pnpm typecheck` 通过

- 重构（conversation-switch-lightweight）：会话切换改为前端先切换、后端异步补正式消息，并补齐归档/抛弃交互
  - 前端切换会话时改为立即更新当前前台会话并优先渲染本地缓存，不再同步等待 `switch_active_conversation_snapshot`
  - 锚点补拉语义收敛为“最后一条正式消息之后的消息”；锚点失效时只回退最近 `5` 条，不再全量重拉整会话
  - 后端新增 `request_conversation_messages_after_async`，请求会立即返回，后续异步通过事件推送补回正式消息
  - 切到后台的会话只保留最近 `5` 条缓存消息，切回前台后再按锚点补拉，减少白屏与切换阻塞
  - 主会话与前台会话语义正式拆开：主会话只决定固定位置与默认归宿，前台会话只决定当前正文显示
  - 会话托盘调整为“主会话固定首位 + 其他会话 + 最后一个新建按钮”，按钮文案改为工作空间名
  - 归档入口改为“预判断 + 归档/抛弃/取消”三按钮确认；`归档` 表示摘要与记忆，`抛弃` 表示直接删除当前会话
  - `抛弃` 动作不再复用归档忙碌态，不再错误显示“归档中”全屏遮罩
  - 新增归档文档 `plan/done/20260323_会话切换链路瘦身归档.md`

- 修复（stt-config-selection）：保存其他配置时不再意外清空 STT API 选择
  - 移除 `normalize_app_config` 中对 `stt_api_config_id` 的自动过滤，避免用户仅保存其他配置时丢失已选择的 STT API
  - 改为在 `save_config` 中基于前后配置差异判断：只有当已选 STT API 被删除，或其格式被改成不再支持 STT 时，才清空选择并关闭自动发送
  - 保留显式 STT 设置保存链路的原有行为，不影响用户主动切换 STT API

- 新增（command-tool-catalog）：统一命令工具并将工具页改为后端目录驱动
  - 原 `wait`、`reload`、`organize_context` 运行时入口收口为统一内置工具 `command`，支持 `help`、`reload`、`organize_context`、`wait <ms>`
  - 默认工具配置与运行时装配改为围绕 `command` 工作，旧 `desktop-wait` / `wait` / `reload` / `organize_context` 配置会自动迁移到 `command`
  - 新增 `list_tool_catalog` 命令，后端统一下发工具名称、说明与参数结构，前端工具页不再按工具 ID 写死描述
  - 工具页改为展示后端 catalog 中的参数摘要与状态信息，移除原页面内置调试按钮
  - 新增归档文档 `plan/done/20260322_工具命令整合与工具页动态化归档.md`，归并 2026-03-22 两份已完成计划

- 升级（rig）：升级 `rig-core` 并修复配套兼容
  - `rig-core` 从 `0.31.0` 升级到 `0.33.0`
  - `rmcp` 同步升级到 `0.16.0`，消除与 `rig-core` 的类型版本冲突
  - 适配 `rig::completion::Message::System` 新枚举分支，补齐运行时与测试路径匹配逻辑
  - 适配 `StreamableHttpClient::post_message` 新签名（新增 headers 参数）
  - 已通过 `cargo check` 与 deepseek/system_mcp 相关回归测试

- 修复（stream-fallback）：全渠道流式失败后永久降级非流式，并统一运行日志规范
  - 新增 `provider_non_stream_base_urls` 配置项：按 `base_url` 持久化“禁用流式”状态，重启后仍生效
  - 主聊天、归档总结、视觉图片转文本三条链路统一接入流式失败自动重试非流式逻辑
  - OpenAI / DeepSeekKimi 请求在流式失败后会立即写入持久化黑名单并当次非流式重试，避免重复报错不可用
  - 相关日志改为运行日志通道（`runtime_log_info` / `runtime_log_warn`），不再直写 `stderr`
  - 新增日志文案统一中文前缀（`[聊天]` / `[视觉]` / `[推理]`），并移除新增路径中的 `[CHAT]` 英文前缀

- 新增（assistant-interaction-guide）：补充用户协作引导类 preset skill，并统一内置 skill 触发文案
  - 新增 `assistant-interaction-guide`，用于引导用户如何与助理协作、如何提出任务，以及如何直接请求安装 Git、安装 Node.js、安装 skill、安装 MCP、安排部门与工作流
  - 为多个内置 preset skill 的 `description` 统一改成“当……时，必须立刻阅读我”的强触发风格
  - `skill-setup` 中的最小示例描述同步改为同一触发口径，避免模板与实际技能风格不一致

- 调整（terminal-windows-copy）：Windows 终端引导改为 Git 优先
  - 删除工具页中对 PowerShell 7 的推荐安装文案与按钮，避免把支持能力误写成首选要求
  - Windows 缺少终端时的前后端提示统一改为“请先安装 Git，并使用 Git Bash”，并直接附上 Git 官网下载链接
  - 保留运行时对 PowerShell / Git Bash 的兼容支持，但用户可见引导统一改为 Git-first
  - Windows 下 `AUTO` 终端优先级改为 `Git Bash -> PowerShell 7 -> Windows PowerShell 5.1`
  - Git 安装提示从工具说明区移动到 Shell 工作空间区域，避免和具体工具说明混在一起
  - 欢迎页新增 Git / Node.js 必装检查卡片，并提供官网安装入口

- 调整（instruction-copy）：收敛最高指令常量中的风险与澄清措辞
  - 将“拒绝绕过”改写为更通用的“遵循约束”，强调优先查明限制原因且不跳过既有校验
  - 将“安全优先”改写为“稳妥优先”，聚焦输入处理、权限控制、数据暴露与脚本执行风险
  - 将“不断提问”改写为“必要澄清”，强调仅在关键假设或信息缺失时做简短确认

- 新增（apply-patch-rewind）：为 `apply_patch` 补齐新增/删除/修改/移动的可恢复备份链路
  - `apply_patch` 成功执行后会在应用数据目录下写入 `temp/apply_patch/records` 与 `temp/apply_patch/blobs`，记录恢复索引与原始文件快照
  - `Delete File` 执行前改为先备份原文件内容，撤回时可直接恢复已删除文件
  - `Update File` 与 `Move + Update` 执行前都会保存原文快照，撤回时优先按 temp 记录恢复，不再只依赖反向 hunk
  - 会话撤回路径新增 temp 记录匹配与清理逻辑，恢复成功后自动删除对应记录与备份 blob
  - 上下文整理成功后会清空 `temp/apply_patch`，避免旧撤回缓存无限堆积
  - 新增并通过 `apply_patch_tool_tests`、`rewind_apply_patch_tests`，并完成 `cargo test -- --nocapture` 与 `cargo check`

- 重构（context-organization-memory-archive）：统一上下文整理、记忆整理、归档链路并改为自动后台记忆生成
  - 自动上下文整理与 `organize_context` 工具统一改为“写入当前会话的上下文整理消息”，并在消息落盘校验通过后才发送前端刷新事件，避免 UI 与后端状态不一致
  - 自动整理路径与归档前预整理路径都会在成功后异步触发记忆整理，不再阻塞聊天或归档主流程
  - 后台记忆整理统一基于“本会话全部上下文整理信息 + 本会话出现过的记忆全集（`memory_recall_table` 去重）”生成，支持新增记忆、保留有用记忆、重复记忆合并
  - 归档链路调整为“先做一次上下文整理，再把全部上下文整理信息带入归档”，归档提示词改为过去事实导向，不再描述“接下来要做什么”
  - 整理模型失败改为自动重试 3 次（每次 5 秒），后台记忆整理失败改为自动重试 3 次（每次 30 秒），超限后仅记日志，不阻断主会话继续
  - 提示词契约拆分为 `context_compaction/`、`memory_curation/`、`conversation_archive/` 三个独立模块目录，移除旧混合提示词文件
  - 前端归档消息查看支持展示上下文整理消息正文与额外文本块，归档导出 JSON 改为以 `source_conversation.summary` 为单一摘要来源，移除顶层重复 `summary`
  - 用户可见文案与注释统一改为“上下文整理”，同时保留对历史“上下文压缩”消息的兼容识别
  - 已完成 `cargo check` 与 `pnpm typecheck`

- 修复（remote-im-async-io）：远程 IM 附件读取改为异步并修正钉钉空发送返回
  - `remote_im_send` 附件路径解析与图片读取改为异步文件 I/O，避免阻塞运行时线程
  - OneBot 本地媒体读取改为 `tokio::fs::read`，保持原有错误映射语义
  - 远程适配层内容项读取改为异步，并统一中文错误文案
  - 钉钉 OpenAPI 发送链路不再在“全部内容被跳过”时返回 `ok`，改为明确返回跳过错误

- 文档（plan-archive）：合并归档远程 IM 渠道与联系人相关计划文档
  - 将 `plan/20260314_远程IM联系人管理计划.md`、`plan/20260314_远程IM联系人页面UI方案.md`、`plan/20260314_远程IM渠道接入计划.md`、`plan/远程IM渠道抽象设计.md`、`plan/远程IM渠道接口技术指南.md` 按最新实现统一整理
  - 新增归档文档 `plan/done/20260320_远程IM渠道与联系人能力归档.md`
  - 清理已过时的进行中计划，后续远程 IM 增量工作改为基于现状单独立项

- 修复（mcp-windows）：增强 MCP stdio 在 Windows 下的命令执行兼容性
  - `cmd` 调用补充 `/D /S /C` 参数，并在执行前切换到 UTF-8 代码页，降低中文路径/输出乱码导致的连接失败概率
  - 连接失败时的 `stderr` 读取改为先拷贝再裁剪，规避临时借用导致的文本处理不稳定
  - 保存会话 API 配置后立即执行本地绑定归一化，避免界面配置与运行时绑定状态短暂不一致

- 修复（remote-im-media）：完善远程 IM 三端媒体发送与 OneBot 入站文件解析
  - `remote_im_send` 支持 `file_paths`，并统一为“图片按图片发送、其他按文件发送”；文本可为空但不能与文件同时为空
  - 飞书发送链路补齐图片/文件上传后发送（`image_key` / `file_key`）
  - 钉钉发送链路补齐媒体上传后发送（`sampleImageMsg` / `sampleFile`），有附件时自动走 OpenAPI 路径
  - OneBot 入站消息不再仅用占位文本，图片/文件会真实入队到 `images/attachments`
  - OneBot `file_id` 解析增加多动作兼容降级（`get_file(url/path/data)`、`get_group_file_url`、`get_private_file_url`），修复相对文件引用导致的入队失败

- 调整（chat-ux）：主会话解耦人格并集中优化聊天底部交互体验
  - 主会话池不再按 `Conversation.agent_id` 绑定或过滤，主部门切换人格时直接影响 UI 展示与发言人格，不再影响主会话归属
  - 主会话新建、切换、归档回退与上下文摘要读取统一改为面向“未归档主会话池”，委托会话仍保持按目标人格独立运行
  - 3 句及以下短会话、空会话执行归档时改为直接删除当前会话，并自动切换到最新未归档会话；若不存在则自动补建新会话
  - 归档按钮链路改为静默处理删除/切换场景，不再弹出“当前对话为空，无需归档”或误导性的“开始归档”提示
  - 新建主会话改为创建后直接走单次快照切换，减少重复消息加载与会话列表刷新，缓解“新建会话明显卡顿”
  - 聊天输入框新增纯文本历史记录能力，支持上下方向键回溯最近 100 条发送内容，并持久化到 `localStorage`
  - 底部输入区 UI 调整为更轻量的无灰面板样式，统一按钮尺寸与顺序，去除助理头像下重复的停止按钮
  - 会话托盘恢复为横向 `xs` 胶囊样式，当前会话使用 `bg-neutral` 高亮，托盘增加更明显的中性色渐变底板并隐藏横向滚动条
  - 聊天窗口标题栏移除后台日志按钮，日志页顶部新增“后台日志”按钮以打开现有运行日志弹窗

- 重构（frontend-entry）：改造三窗口多入口并精简 `App.vue`
  - 前端新增 `config/chat/archives` 三套独立入口脚本与 HTML 页面
  - `vite` 改为多页面构建输入，产物包含 `index/chat/archives` 三页面
  - Tauri 窗口配置改为按窗口加载独立页面 URL（`index.html/chat.html/archives.html`）
  - 原超大入口迁移为 `UnifiedWindowApp.vue`，`src/App.vue` 精简为轻量代理
  - 清理聊天输入 placeholder 遗留 `hints` 逻辑，统一返回 `chat.placeholder`

- 修复（chat-switch）：会话切换改为单次快照并遵循“最近 5 条”设计
  - 新增 `switch_active_conversation_snapshot` 聚合命令，一次返回会话切换所需数据
  - 切换时只返回最近 5 条消息，并返回 `hasMoreHistory` 供前端“加载更多”使用
  - 前端切换流程改为单次请求后一次性刷新消息/会话列表/工作区标签，减少重复调用
  - 移除快照接口中无意义的 `api_config_id` 依赖，参数语义收敛为 `conversationId + agentId`

- 调整（chat-ui）：思维链与工具执行状态文案/动效优化
  - 思维链预览文案调整为“正在思考中 / 思考了XX秒 / 思考完成”
  - 思维链与工具执行中状态增加统一扫光动效（2.5 秒节奏）
  - 工具卡片状态文案调整为“工具执行中 / 工具执行毕”
  - 思维链预览去斜体、缩小字号并设置最小宽度，提升阅读稳定性

- 新增（chat-rewind）：实现会话撤回与工具修改逆向撤回能力
  - 新增撤回弹窗：支持“撤回消息并撤回修改 / 仅撤回消息 / 取消”三种操作
  - 后端命令支持 `undo_apply_patch`，可对可逆 `apply_patch` 工具修改执行反向撤回
  - 撤回目标统一定位到用户消息，兼容从助手消息触发撤回
  - 撤回链路补齐前后端日志与失败反馈，不可逆场景明确提示用户改用“仅撤回消息”

- 调整（runtime-logs）：禁用前端桥接并完善日志可读性与筛选
  - 禁用前端 `console` 到后端运行日志的桥接链路，避免高频 IPC 带来的性能压力
  - 保留后端运行日志内存缓冲与弹窗查看，前端日志改为仅通过 F12 查看
  - 运行日志支持连续重复聚合（`xN`）与秒级时间显示，降低噪声
  - 新增日志弹窗筛选：按级别、按模块过滤（与虚拟滚动联动）
  - 聊天流绑定等关键日志改为显式写入运行日志缓冲，避免“后端可见、弹窗不可见”
  - 清理多处英文与重复等级前缀日志（`[INFO]/[WARN]`），统一中文任务语义
  - 部分高频状态日志改为“仅首次输出”（离屏修复、`active=false`），减少刷屏

- 新增（runtime-logs）：内存运行日志与前端弹窗查看
  - 后端新增运行日志内存缓冲（仅内存，最大 10MB，超限丢弃最旧日志）
  - 新增 `list_recent_runtime_logs` / `clear_recent_runtime_logs` 命令
  - 记忆存储关键日志改为写入运行日志缓冲（开始/完成/失败/跳过），并保留终端输出
  - 聊天窗口标题栏新增“运行日志”按钮，可弹出日志窗口
  - 日志窗口支持刷新、清空与虚拟滚动，避免大列表渲染卡顿

- 重构（memory-store）：拆分记忆存储模块并清理 legacy 迁移路径
  - `memory/store.rs` 改为聚合入口，拆分为 `types/db/crud/ownership/import_export/archive_feedback/provider_index/maintenance/tests` 子模块
  - 移除 legacy memories 迁移能力（启动迁移调用、迁移函数、相关类型与测试）
  - `archive_feedback`、`provider_index`、`upsert/delete` 补齐中文任务日志（开始/完成/失败/跳过）与关键诊断字段
  - 修复 `health_check` 重复 rebuild 问题，改为单次 rebuild
  - 强化 provider table 名称校验与向量写入事务，降低 SQL 注入与原子性风险
  - 修复敏感内容日志泄露风险：敏感拦截仅记录 `judgment_len`，不记录原文

- 修复（chat-flow）：回归流式状态机与历史刷新时序
  - queued 阶段不再提前进入聊天中状态，`stopChat` 支持 queued 阶段中断
  - `history_flushed` 可见窗口计数兼容 `messageCount`，避免轮次显示错位
  - `stop` 成功路径补充历史刷新，修复测试期望不一致
  - `useChatRuntime` 的 `hasMoreBackendHistory` 改为可选并统一空值保护

- 重构（chat-runtime-tools）：拆分 `tools_and_builtin.rs` 并收敛审查问题
  - 将超大文件拆分为 `tools_and_builtin/` 目录下多个职责子文件（provider 调用、网络、记忆、task、delegate、remote_im、参数类型、Tool 实现）
  - `task/delegate/core_provider` 进一步分层为聚合入口 + 子模块，降低单文件复杂度并减少协作冲突
  - `memory_save` 接入敏感内容拦截，避免密码/密钥等信息被写入记忆存储
  - `task` 工具改为异步执行并使用 `spawn_blocking` 承载阻塞 I/O，补齐关键动作日志
  - 修复 `fetch/websearch` 错误细节丢失问题，完善 HTTP client 构建失败与请求失败的可观测性
  - 统一多处日志为中文与规范前缀，补齐状态词（开始/完成/跳过/失败）及关键字段
  - 调整委托分发逻辑：提取参数校验、前置检查、调用链检查与异步派发 helper，降低 `builtin_delegate` 体积

- 新增（chat-image-preview）：图片预览支持缩放与拖动
  - 双击打开的图片预览框新增放大/缩小/100% 还原控制
  - 支持鼠标滚轮缩放
  - 放大后支持按住拖动平移查看局部细节

- 修复（window-layout）：窗口尺寸越开越大与可拖到不可见问题
  - 修复窗口布局持久化逻辑中逻辑像素/物理像素混用导致的 DPI 放大累积
  - `chat` 与 `archives` 启动时统一按默认 `900x900` 打开（忽略历史保存尺寸）
  - 对话窗口增加真实最小拖拽约束 `600x600`，避免缩到不可见

- 调整（typecheck）：前端类型校验降噪与会话项类型补齐
  - `tsconfig` 增加 `skipLibCheck: true`，忽略第三方 d.ts 兼容噪声
  - 修复会话列表项 `updatedAt` 类型缺失导致的 IDE 报错

- 新增（voice-auto-screenshot）：后台语音关键词自动截图
  - 对话设置新增“后台语音截图关键词”与“截图范围（全屏/前台窗口）”
  - 关键词改为手动保存，输入框右侧提供“保存”按钮，避免实时保存卡顿
  - 关键词保存时自动将全角逗号 `，` 归一化为半角逗号 `,`
  - 匹配规则升级为“去空白 + 小写”后匹配，提升 `lookat / look at` 等识别鲁棒性
  - 命中关键词后自动截图并附加到消息；截图失败不阻断语音主流程
  - 默认关键词改为 `看看,这个,屏幕上,see,look,watch`，默认范围改为“前台窗口”

- 调整（window-main-size）：设置窗口固定尺寸并改为逻辑像素，恢复 DPI 缩放一致性
  - `main` 窗口改为不可调整大小（固定 `900x900`）
  - 运行时窗口尺寸设置由 `PhysicalSize` 改为 `LogicalSize`，避免高 DPI 下出现固定物理像素效果
  - `main` 窗口忽略历史保存的异常尺寸，按固定逻辑尺寸恢复显示

- 调整（wait-tool）：保留实现但永久禁用 wait 工具
  - 运行时工具装配阶段不再挂载 `wait`
  - 工具状态检查统一返回 `unavailable`，提示“wait 工具已永久禁用”

- 文档（plan-archive）：归档 apply_patch 工具接入计划
  - 将 `plan/20260318_apply_patch工具接入计划.md` 按实际落地结果补全为归档报告
  - 归档迁移至 `plan/done/20260318_apply_patch工具接入归档.md`

- 新增（apply-patch）：接入内置 `apply_patch` 工具并默认启用
  - 新增结构化补丁编辑工具，支持 `Add/Delete/Update/Move` 与 `@@` hunk 语法
  - 增加路径越界防护（禁止绝对路径与 `../` 逃逸）
  - 安全判定调整为：LLM 默认工作区免审批；用户工具区走 `AutoApprove/AskUser/Reject` 三态
  - 接入运行时工具装配、工具状态检查、配置默认工具列表与工具页文案
  - `exec` 与 `apply_patch` 默认开关调整为开启

- 修复（chat-settings-stt）：修复语音转写配置重启后丢失
  - STT 相关设置（视觉 API、STT API、完成后发送）在用户改动后立即保存
  - 修复仅依赖异步 watcher 导致的重启前未落盘问题

- 修复（workspace-multi-session）：多会话工作目录支持
  - 工作目录锁定从 `agent` 维度改为 `agent + conversation` 维度，不同会话可独立锁定目录
  - 会话胶囊显示改为“时间 + 工作目录备注名”，并统一使用后端返回的 `workspaceName`
  - 修复默认工作目录备注名回退为“默认工作空间”的问题，优先复用同路径已配置备注名
  - 修复首屏首次启动需切换一次会话才显示正确目录名的问题

- 文档（agents）：统一代理规范文件名
  - 将 `AGENT.md` 统一为 `AGENTS.md`，与项目约定保持一致

- 修复（archive-fallback）：归档失败降级链路升级为三层回退，并统一中文日志
  - 归档模型失败后优先使用“压缩内容 + 最后三轮正文对话”生成降级摘要
  - 若压缩降级也失败，再回退到“仅最后三轮正文对话”兜底摘要
  - 归档降级路径日志改为中文，保留 `trace_id`、`conversation_id`、`err` 等标识符字段
- 新增（context-compaction）：自动上下文治理改为“会话内压缩”路线，并对所有会话启用压缩提示词规则
  - 自动触发与 `organize_context` 工具改为写入压缩消息，不再迁移到归档会话
  - 压缩完成后通过 `history_flushed` 通知前端，保持当前会话继续工作
  - 组装提示词时全局生效：保留全部压缩消息（按时间顺序）+ 最后可用用户提示词
  - 若存在压缩边界，则压缩边界之前的普通历史不再参与后续推理

- 新增（多会话管理）：支持同时维护最多 8 个未归档对话
  - 对话按钮按创建顺序分配 8 种 DaisyUI 颜色（跳跃分配，最大化对比度）
  - 按钮显示消息数 + 相对时间（X分钟/小时/天前）
  - 宽度自适应挤压，最小显示圆点标识
  - 新建按钮在达到 8 个时禁用，提示归档旧对话
  - 移除手动重命名功能，简化交互

## v0.6.0 - 2026-03-17

- 解决部分配置无法保存的BUG
- 废除jieba

- 新增（pdf-read-mode）：对话设置新增 `PDF 阅读方式（文本/图片）` 并贯通前后端
  - 新增 `pdf_read_mode` 配置读写、事件同步与自动保存
  - 发送链路按 `pdf_read_mode == image && selected_api.enable_image` 判定
  - 预览链路同步使用同一判定，避免“预览与实发不一致”
  - 新增文案说明：即使设置为图片，模型不支持图片时仍自动回退文本

- 优化（pdf-render）：Hayro 页面渲染改为并行执行，PDF 图片模式显著提速
  - 引入 `rayon`，将按页渲染+编码改为 `par_iter` 并行
  - 同一 PDF 对比结果：`17757ms -> 3375ms`，约 `5.26x` 加速
  - 删除临时 benchmark demo，仅保留产品内并行实现

- 修复（pdf-path-utf8）：修复 PDF 路径 `to_str().unwrap_or(\"\")` 静默退化问题
  - 路径非 UTF-8 时改为显式跳过并打印日志
  - 避免空路径继续下游调用导致隐性异常

- 优化（pdf-text-clean）：PDF 文本清洗前移到提取入缓存阶段（一次性执行）
  - 删除换行符，删除中文字符之间空白
  - 缓存 key 升级 `v2`，避免旧缓存影响新清洗规则

- 优化（token-estimate）：改用 `tiktoken-rs` 进行 token 估算
  - 会话 token 估算由启发式字符规则切换为 `cl100k_base` 编码估算
  - tokenizer 初始化失败时保留旧启发式作为兜底，避免中断主流程

- 优化（pdf-text-limit）：PDF 文本模式新增 `30K token` 上限（`tiktoken-rs`）
  - PDF 转文字后按 token 截断到前 `30,000`
  - 超出部分不再注入 prompt，降低超长上下文风险

- 重构（memory-tokenizer）：移除 `jieba-rs`，改为原生分词路径
  - 内存检索切词改为本地实现（ASCII 词 + CJK 单字/双字）
  - Tantivy probe 同步移除 jieba 依赖
  - 删除 `jieba-rs` 依赖，减少构建与运行时复杂度

- 发布（release）：版本号提升到 `0.5.1`
  - 同步更新 `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`
  - 本地 `pnpm tauri build` 验证通过，已产出 Windows 可执行与安装包

- 调整（attachment-prompt）：附件提示文案去重，移除重复文件名
  - 附件提示仅保留路径字段，不再重复展示文件名
  - 保留“先定位、按类型选 skill/检索、未明确要求先询问用户”的行为约束

- 修复（chat-message-shape）：统一空消息兜底，避免仅文件/仅工具回合被组装为空消息后丢失
  - 组装 `latest_user_text` 文本块时，若文本/元信息/额外块/媒体均为空，自动注入单空格 `" "`
  - 历史消息组装阶段不再跳过“空文本且无媒体”消息，改为保留占位空格
  - Rig 历史回放中 user/assistant/tool 空文本统一回填 `" "`，避免 provider 端空内容报错
  - DeepSeek 消息序列化路径同步兜底：空 content 自动注入文本占位

- 修复（ide-findings）：完成本轮 IDE 指出的问题收敛与健壮性补强
  - 修正远程 IM 规则文案错字（`中間調用/中间调用`）
  - 增补聊天 ingress 行为日志（含 route/mode/key_count/duration_ms）
  - 统一部分聊天调度日志为中文规范前缀
  - 钉钉 Stream 生命周期与停机信号处理完善，避免重复启动泄漏并支持优雅停止
  - 钉钉回调解析日志去敏，不再记录原始 payload 片段
  - 钉钉文件下载加入大小上限与流式读取，避免一次性内存放大
  - 空消息/缺字段场景改为显式 skip 分支，避免错误入队
  - 队列与会话状态监听增加异常清理与状态值校验
  - 远程 IM 渠道密钥输入改为默认掩码并支持显隐切换
  - `tauri.conf` 默认禁用 devtools；开发态可用环境变量 `EASYCALL_DEVTOOLS` 显式开启
  - `round_failed` 保留原始错误上下文，避免固定兜底文案丢失信息

- 重构（app-shell）：拆分 `App.vue` 超大脚本，收敛页面职责
  - 抽离终端审批队列、更新检查、UI 字体策略为独立 composable
  - 抽离聊天工作空间、归档导入预览、回溯/重生会话操作为独立 composable
  - `App.vue` 从 1795 行下降至约 1538 行，保留原有行为并提升可维护性
- 修复（app-shell）：修复 `activeAssistantAgentId` 声明顺序导致的前端编译报错
  - 避免“声明前使用/赋值前使用”触发 `ts-plugin(2448/2454)`

- 调整（chat-queue）：优化出队时序与前端展示策略
  - 发送消息入队后优先尝试立即出队处理；仅在不可出队时退回异步调度
  - `history_flushed` 新增按 `activateAssistant` 分支处理：仅激活批次清屏，非激活批次按顺序追加消息
  - 修复 `history_flushed` 异步等待后的轮次竞态，避免旧轮次回写覆盖新一轮 `sendChat`
- 重构（chat-runtime-structure）：拆分 `chat_and_runtime.rs` 超大文件，按职责下沉到子模块目录
  - 入口文件收敛为 include 壳：`commands/chat_and_runtime.rs`
  - 核心链路拆分为 `core_helpers/core_send_inner/core_commands`
  - 模型与多媒体能力拆分为 `models_catalog/attachments_io/stt_transcribe/model_providers/tools_and_cache`
  - 在不改变既有行为前提下提升定位问题与后续改造效率

- 重构（chat-routing）：彻底移除“消息/会话绑定模型 API”路径，统一改为“部门 -> 模型”解析
  - `ChatSessionInfo` 改为仅携带 `department_id + agent_id`，不再持有 `api_config_id`
  - `send_chat_message_inner`、远程 IM 入队、任务/委托入队统一按部门解析模型
  - `Conversation` 移除 `api_config_id` 字段，清理会话层对应读写与筛选依赖
  - 停止/中断会话键移除 API 维度，统一以会话与人格标识定位
  - 归档与会话摘要链路改为按人格所属部门反查模型配置

- 重构（terminal）：拆分终端工具模块，降低耦合并提升可维护性
  - 将 `terminal.rs` 按职责拆分为 `runtime/workspace/approval/guards/exec` 子模块
  - 保留原有行为与接口，主文件收敛为模块入口
- 调整（terminal-approval）：默认机器人工作目录内的写操作跳过审批
  - 当 `cwd` 位于 `llm_workspace_path`（含子目录）时，`Existing/Unknown` 写风险不再触发审批弹窗
  - 保留原有锁定工作区跳过审批逻辑
- 测试（terminal）：新增 PowerShell / Git Bash 独立回归用例
  - 覆盖“默认工作目录写入已有文件不进入审批等待”的场景
  - 通过超时保护验证不会卡在审批通道
- 调整（terminal-approval）：审批超时策略收敛为 60 秒并明确本机审批要求
  - 审批等待超时后返回 `approval_timeout_local_required`，不再长时间卡住执行链路
  - 统一提示“当前本地并无管理员监守，非默认工作目录禁止高危操作；需本机审批”
  - 新增超时分支映射测试，覆盖结构化阻断返回

## v0.5.0 - 2026-03-16

- 新增（chat-render）：将助手 Markdown 渲染器切换为 `markstream-vue`
  - 用流式优先的渲染链路替换此前方案
  - 通过 `markstream-vue` 集成启用 Mermaid 与 KaTeX
  - 依赖集合对齐官方用法（`markstream-vue`、`stream-markdown`、`stream-monaco`、`shiki`、`@terrastruct/d2`）
- 修复（chat-stream）：降低流式结束时的二次淡入与重复收尾效果
  - 切换到基于节点的流式渲染模式（`nodes + final`），收尾更平滑
  - 停用旧的 Mermaid 二次扫描链路，避免额外重渲染
- 修复（chat-history）：修复加载更多逻辑与空屏上拉加载
  - 恢复 `hasMoreMessageBlocks` 与后端历史标志的绑定
  - 当消息列表为空时，上拉可直接拉取并显示最近 5 条消息

- 新增（chat-ui）：对话窗口 UI 逻辑重做与动效增强
  - 收敛消息渲染路径，减少流式/完成态切换时的布局跳动与重复渲染
  - 优化出队清屏与历史回放衔接，避免非预期自动刷新
  - 新增消息入场动画（由下至上淡入），并修正流式草稿转正式消息时误触发动画的问题
  - 调整对话操作位与气泡布局细节，提升整体稳定性与观感一致性

- 新增（remote-im-ui）：打磨联系人页布局与交互细节
  - 三列卡片改为纵向流式结构，统一卡片尺寸与滚动行为
  - 为渠道列表与联系人列表加入分页并优化页码/操作区对齐
  - 统一刷新为图标按钮，补齐还原图标，优化文案为“渠道列表/联系人列表”

- 修复（config-ui）：设置页统一预留滚动条占位（`scrollbar-gutter: stable`）
  - 避免各配置页在有/无滚动条时横向抖动
  - 满足强迫症：内容终于不再左右跳了

- 重构（chat-pipeline）：将多模态兼容处理前移到出队阶段，按“逐条消息”定型
  - 在批次写入历史前按会话模型能力处理每条 user 消息
  - 模型支持图片时保留图片，不做额外注入
  - 模型不支持图片时移除图片，并按规则注入文本：
    - 命中已有图转文缓存：注入对应图转文内容
    - 无图转文缓存：注入提示“这里有一张图片，但当前模型不支持图片输入，所以已忽略。”
- 修复（chat-runtime）：收敛运行时重复改写，避免二次注入
  - 保留 router 防御性清理，但移除运行时二次文本注入逻辑
  - 统一由“出队定型结果”驱动后续 prompt 构建与请求序列化

## v0.4.0 - 2026-03-16

- 新增（chat-ui）：重构对话窗口渲染管线，提升流式稳定性
  - 统一流式/历史气泡行为，保证流开始与结束阶段布局稳定
  - 修复助手草稿消息生命周期问题，避免重复气泡与覆盖闪烁
  - 保持头像、间距与操作区在流式状态切换时的一致性
- 新增（chat-control）：优化停止/重新生成交互模型
  - 发送后在排队阶段即可立即停止，并与后端中断保持一致
  - 活跃轮次中发送按钮自动切换为停止按钮，提升打断效率
  - 仅允许对“最后一条助手消息”执行重新生成
- 重构（chat-runtime）：移除粗粒度全局刷新耦合
  - 移除聊天流程中的窗口级 `easy-call:refresh` 监听与广播
  - 防止延迟事件误触发已取消轮次的恢复

## v0.3.9 - 2026-03-29

- 修复（chat-ui）：稳定会话切换后的列表与滚动行为
  - 切换/新建会话改为复用后端快照，同步消息、会话列表与工作区标签
  - 会话切换完成后自动平滑滚动到底部，并为异步补消息缺席场景增加兜底
  - 限制消息区外层横向溢出，避免离屏消息临时测量误差导致整列出现横向滚动条

## v0.3.8 - 2026-03-16

- 新增（remote-im）：完成远程 IM 后端集成与主流程接线
  - 增加入站入队、队列调度与出站适配器路由
  - 优化 OneBot/NapCat 渠道生命周期处理与运行态一致性
- 新增（remote-contacts）：新增联系人级控制与激活策略
  - 权限拆分为 `allow_receive` 与 `allow_send`（默认关闭）
  - 增加激活模式（`never` / `always` / `keyword`）与冷却支持
  - 增加激活决策日志与联系人级激活配置命令
- 重构（remote-im）：提升可维护性与诊断能力
  - 将大段校验/解析路径拆分为辅助函数
  - 为 Dingtalk/OneBot 发送链路与 token 流增加结构化日志
  - 减少高风险/高频事件日志的原始载荷输出
- 修复（ui/types）：提升配置与运行时一致性
  - 修复会话 id/类型不一致及多个静默 catch 问题
  - 修复 RemoteIm 页签行为、列表布局与 Tailwind 类名问题
  - 关于页版本号改为动态显示（移除硬编码）
- 杂项（branding）：将项目对外品牌统一为 `P-ai` / `π师傅`
  - 更新可见窗口标题、文档链接与发布/更新仓库地址
  - 统一清理旧品牌标识，后续以新标识为准

## v0.3.2 - 2026-03-13

- 修复（multimodal）：稳定 latest 媒体提示语义
  - 移除 `latest_images/latest_audios` 的“向前回填上一条消息媒体”行为
  - 覆盖语义切换为三态（`None` / `Some([])` / `Some([...])`）
  - 统一媒体解析逻辑，复用共享解析器
- 修复（multimodal）：提升媒体过滤可见性
  - 图片被过滤时追加显式文本提示
  - 模型侧拒绝图片时追加显式回退提示
- 修复（audio）：将 `input_audio.format` 映射为 OpenAI 兼容短格式
  - 在 deepseek/openai-compatible 请求中使用 `wav/mp3/...`，而非完整 MIME
  - 保持调试请求日志与真实请求载荷格式一致
