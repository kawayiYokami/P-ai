# 变更日志

## 未发布

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
  - 在需要兼容时保留旧数据/存储标识

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
