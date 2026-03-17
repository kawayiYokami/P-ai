# 变更日志

## 未发布

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
