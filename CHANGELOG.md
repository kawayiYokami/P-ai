# 变更日志

## 发布（v0.8.6）：统一 PDF 图片分页、后台归档与多图转发

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
