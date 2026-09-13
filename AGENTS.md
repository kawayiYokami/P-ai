# AGENTS.md

This file provides guidance to AI coding when working with code in this repository.

## 项目概述

PAI 是一个 Windows 优先的桌面 AI 助手，使用全局热键呼出/隐藏对话窗口，常驻系统托盘。技术栈为 Tauri 2 (Rust) + Vue 3 (TypeScript) + Vite + DaisyUI，包管理使用 pnpm。

当前发布策略为 Windows + Linux + macOS：
- Windows 安装版使用 NSIS
- Windows 便携版使用 zip + `PORTABLE` 标记文件
- Linux 发布构建产物至少保留 `.deb` / `AppImage`
- macOS 发布构建产物使用 Intel + Apple Silicon 通用 DMG
- 应用内自动更新当前仅覆盖 Windows 安装版与便携版

## 构建与开发命令

```bash
# 开发模式（前端热重载 + Rust 自动重编译）
pnpm tauri dev

# 仅启动前端 dev server（端口 1420）
pnpm dev

# 类型检查
pnpm typecheck                              # 前端 Vue + TypeScript
cd src-tauri && cargo check                  # Rust

# 测试
pnpm test                                    # 前端 vitest
cd src-tauri && cargo test                   # Rust 测试
pnpm smoke                                   # Windows 集成冒烟测试（PowerShell）

# 生产构建
pnpm build                                   # tsc + vite build
pnpm tauri build                             # 完整打包（含 Rust 编译）
pnpm tauri:build:macos                       # macOS universal DMG（Intel + Apple Silicon）

# VS Code 侧边栏扩展
pnpm package:vscode-sidebar
pnpm publish:vscode-sidebar
```

## 架构概览

### 前后端通信

```
Vue 组件 → invokeTauri() → Tauri invoke() → Rust #[tauri::command] → 返回 Result
流式消息: Rust 通过 tauri::Channel<T> 向前端推送 delta 事件
```

### Rust 后端 — include! 单入口模式

`src-tauri/src/main.rs` 通过 `include!()` 宏将所有模块拉入同一编译单元：

| include 文件 | 职责 |
|---|---|
| `features/core/domain.rs` | 数据模型、常量、响应风格 |
| `features/config/storage_and_stt.rs` | 配置读写 (TOML)、本地/远程 STT |
| `features/chat/conversation.rs` | 对话生命周期、自动归档逻辑 |
| `features/chat/model_runtime.rs` | LLM 多供应商适配 (OpenAI/Gemini/Anthropic)，使用 rig-core |
| `features/chat/model_runtime/provider_and_stream.rs` | 供应商具体实现与流式处理 |
| `features/chat/model_runtime/tools_and_builtin.rs` | 工具执行（内置 + MCP） |
| `features/system/commands.rs` | Tauri 命令处理入口，分拆至 commands/*.rs |
| `features/system/tools.rs` | 桌面工具基础设施 (screenshot/wait/operate) |
| `features/system/updater.rs` | GitHub Release 自动更新、便携版 helper、staging/回滚 |
| `features/system/windowing.rs` | 窗口定位、显示、隐藏、托盘 |
| `features/memory/matcher.rs` | 记忆搜索与匹配 |

### Vue 前端 — Composable 驱动

前端无全局状态库（无 Vuex/Pinia），状态通过 Vue Composition API 的 reactive refs 管理。核心逻辑封装在 composables 中，组件层很薄。

关键 composable 分组：
- **shell/**: `use-app-bootstrap` (初始化)、`use-app-theme`、`use-window-shell`、`use-app-lifecycle`、`use-github-update`
- **chat/**: `use-chat-flow` (流式缓冲与 delta 处理)、`use-chat-runtime` (会话持久化)、`use-chat-turns` (上下文窗口计算)、`use-chat-media` (图片/音频)、`use-speech-recording` (本地+远程 STT)
- **config/**: `use-config-persistence` (加载/保存)、`use-config-runtime` (模型列表刷新)、`use-config-core` / `use-config-editors` (供应商与模型配置编辑)

### VS Code 侧边栏扩展

- `src/entries/sidebar.html` 是 Vite 多入口源码之一，`pnpm build` 时和主应用前端一起产出到仓库根 `dist/`
- `src/features/sidebar/extension/` 只是 VS Code 扩展壳；打 `.vsix` 前必须先把仓库根 `dist/` 同步到该目录下的 `dist/`
- 本地调试时扩展会优先读取 `src/features/sidebar/extension/dist/`，找不到才回退到仓库根 `dist/`；但 VSIX 打包只会收录扩展目录自己的 `dist/**`
- 打包和发布步骤不要再手敲长命令，统一走 `pnpm package:vscode-sidebar` / `pnpm publish:vscode-sidebar`

### 多窗口

Tauri 管理 3 个无边框窗口：`main`（配置，900×900）、`chat`（对话，618×1000）、`archives`（归档，900×900）。`App.vue` 根据窗口 label 切换视图模式。

### 数据持久化

默认情况下，配置与运行数据存储在 `ProjectDirs` 配置目录；若可执行文件同级存在 `PORTABLE` 标记文件，则切换到可执行文件同级 `data/` 目录（无数据库）。

应用根目录 `app_root` 由 `app_root_from_data_path` 从 `config_mark` 锚点反推（平台感知：父目录名是 `config` 则取上级，否则用自身）。目录布局在 `app_root` 直下层：

- `app_config.toml` — 配置、API 供应商、热键、工具开关
- `config_mark` — 路径锚点占位文件（从不读写内容，仅用于反推 app_root，替代早期伪装的 `app_data.json`）
- `config/` — agents.json 等代理配置
- `state/` — runtime_state.json 等运行时状态
- `chat/conversations/` — 会话数据
- `backups/` — 备份
- `memory/` — 记忆库
- `task/` — 任务存储
- `delegate/` — 委托存储
- `avatars/`、`media/`、`exports/` — 资源目录
- `llm-workspace/` — Shell / Skills / MCP 等运行工作区

### 支持的 API 格式

`openai`（OpenAI/DeepSeek/Kimi）、`anthropic`（Claude）、`gemini`（Google）、`openai_tts`（远程 STT）

## 开发约定

### Rust 规则

- 禁止 `unwrap()` / `expect()`（测试除外），统一使用 `Result` 传递可读错误
- 网络与 I/O 走异步，不阻塞 UI
- 改动后优先保证 `cargo check` 通过
- 保持函数职责单一、意图清晰，不以行数为硬性标准；用注释分区（`// ========== xxx ==========`）标识意图

### 前端规则

- DaisyUI 组件优先，避免手写重复样式
- 滚动区统一使用 `OverlayScrollArea`（`src/features/shared/components/OverlayScrollArea.vue`）：普通 div 滚动容器一律用它包裹，不要裸挂 `FloatingScrollbar`、不要直接写 `overflow-y-auto` 了事；需要外部同步时用它的 expose（`scrollerRef` / `updateThumb` / `reveal` / `hide`），限高与布局类通过 `scroller-class` 传给内部 scroller。具名例外：textarea 自滚动、聊天主区与代码区等虚拟列表 scroller（事件与联动密集）仍直接挂 `FloatingScrollbar`
- 配置页"有改动才允许保存"，保存后状态立即回写
- 不要默认引入 watch/autosave；当前配置页以显式保存为主
- APP 与 Web/VS Code 的传输差异只能收敛在 `tauri-api`；会改变会话、轮次或消息状态的运行时流程不得按宿主分支或复制实现。
- **Web 端功能缺失在任何情况下都不可接受。** WEBUI（Web/VS Code 侧边栏）是产品本体，不是附属品；任何功能默认必须在两端一致可用，同一套业务逻辑，差异只允许收敛在 `tauri-api` 传输层。遇到「Web 端实现困难」时，默认动作是把功能做出来，禁止用 native-only 声明、测试豁免、能力裁剪等方式让 Web 端功能缺失变成「合规」。任何「某端不可用」的声明一律视为一次功能删除，必须由用户明确批准才算数，AI 无权自行决定。
- 主题、设置按钮、文件路径和能力显隐可以按宿主变化，但评审时必须确认它们不改变聊天运行时语义；新增宿主入口必须复用 `main-chat`。
- 对话窗口保持极简，外链走系统浏览器

### 验证与测试规则

- 禁止无用测试：测试必须和本次改动的影响范围、风险等级直接相关，不要为了“安心”反复跑无关测试或全量测试。
- 开发过程中只进行最小相关测试；跨模块、公共类型、持久化、启动链路、构建配置等高风险改动，也只扩大到能覆盖本次风险的最小必要范围。
- 只有在准备提交时才进行全量测试或全量检查；不要在阶段开发过程中反复跑全量测试。
- 不要在每个小步骤后重复执行同一组耗时检查；除非中间改动触及了已验证路径，否则不重复测试。
- 不要默认运行 `cargo fmt` 或全仓格式化；只有用户明确要求、或当前改动本身需要格式化且不会制造无关 diff 时才执行。
- 当默认 `target/` 被运行中的进程占用、需要使用临时 Cargo 构建目录做验证时，统一使用仓库内的 `src-tauri/target-tests` 作为临时 `CARGO_TARGET_DIR`；不要再新建其他 `target-*` 目录。
- 执行 Cargo 验证时，workdir 一律固定在仓库根，`CARGO_TARGET_DIR` 只写仓库内全路径 `src-tauri/target-tests`；不存在相对路径简写，禁止在 `src-tauri/` 内直接执行验证。若误建了根目录 `target-tests/` 或 `src-tauri/src-tauri/target-tests/` 等错误目录，必须立刻删除并改回仓库约定路径。
- `cargo test` 位置参数一次只能直接接收一个测试名；需要跑多个离散测试时，必须分别执行多条命令，或改用能同时覆盖目标的单个过滤模式，不能把多个测试名直接并列写在同一条 `cargo test` 后面。
- 本地 Cargo 缓存清理优先使用 `cargo-reclaim` trim（只回收过期 incremental/deps/build 缓存，保留最终二进制与热构建产物）；轻量清 `incremental` 仍可用 `pnpm clean:cargo-cache`，整目录删除用 `pnpm clean:cargo-cache:all`；不要手搓其他 `target-*` 清理路径。
- 与用户沟通时不要主动暴露本机硬盘绝对路径，优先使用仓库内相对路径表述。
- 提交前仍必须修复并跑通本次改动影响到的必要测试；“禁止无用测试”不等于跳过必要验证。

### 更新与发布规则

- 当前仅支持 Windows 应用内自动更新；Linux 与 macOS 仍维护发布构建链路
- `src-tauri/tauri.conf.json` 中的 `plugins.updater.pubkey` 只是启动期占位，真正使用的公钥由构建时 `TAURI_UPDATER_PUBLIC_KEY` 注入并在 Rust 侧覆盖
- 便携版通过 `PORTABLE` 标记识别，自动更新走 `zip -> staging -> helper 替换 -> 备份回滚`
- 修改版本号时，必须同步更新以下文件：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.lock`，并新增对应的 `docs/changelog/releases/vX.Y.Z.md` 后执行 `pnpm changelog:build`
- 每次提升版本号时，默认先清理一次本地 Cargo 构建缓存，避免增量缓存长期膨胀：先退出正在运行的应用，再执行 `cargo-reclaim src-tauri --all --yes`（trim，只回收过期产物，保留热缓存）；若缓存异常或磁盘紧张，再用 `pnpm clean:cargo-cache:all`
- 开发阶段不写任何 changelog 文件，日常变更的唯一记录是 commit message；发布说明（`docs/changelog/releases/vX.Y.Z.md`）在发版时由发布流程根据上一个 tag 之后的 commit 汇总生成，面向用户。
- 修改 `src-tauri/Cargo.lock` 时，只允许更新本项目包 `easy-call-ai` 的版本条目；禁止使用全局替换批量改第三方依赖版本号，避免引入错误 checksum

### 代码组织原则

- 保持模块拆分，避免超大单文件；`紧凑优先` 是起步策略，不是长期豁免。
- 是否拆分以职责是否单一、内聚度、耦合度为唯一判据，不以行数或函数数量为硬性标准。
- 当同一文件内出现多个低相关子域，或协作冲突、测试/变更成本明显上升时，优先拆分；必要时增加评审频率或补充变更统计。
- 注释说明意图而非实现。

#### “紧凑优先” 与 “按功能组织” 决策指南

- `紧凑优先` 适用：功能仍集中在一个场景、强耦合且变更总在同一处，使用注释分区（如 `// ==================== 配置管理 ====================`）可快速定位。
- `按功能组织` 适用：已经出现清晰职责边界，或多人并行开发导致同文件冲突频繁，应按 `features/chat/`、`features/archive/` 等目录分组。
- 判断标准：
  - 职责：是否单一、是否只做一件事
  - 内聚：同文件内功能是否围绕同一领域
  - 耦合：同文件内是否出现多个低相关子域（如配置读写、网络调用、视图拼装混在一起）
  - 协作：多人经常改同文件、冲突率高
  - 可测试性：难以对局部逻辑做独立单测
  - 变更成本：每次重构/编译/回归都要牵动大范围代码
- 迁移触发器：当模块边界已明显分离，且上述任一成本显著上升时，优先拆分。
- 简单迁移步骤：
  1. 先抽“纯数据与类型”（如 `models.rs` / `types.ts`）。
  2. 再抽“稳定服务逻辑”（如 `service.rs` / `use-*.ts`）。
  3. 最后抽“入口协调层”（如 `controller.rs` / `view glue`），保证外部调用面不变。

#### “适度重复” 的边界与抽取策略

- 可接受重复：同一模块内短小重复（约 <= 20 逻辑行，且重复片段 <= 2 处）可保留，优先保证直观。
- 触发抽取共享逻辑的条件：
  - 相同处理步骤出现 >= 3 次；
  - 相同错误处理/回滚逻辑出现 >= 3 次；
  - 单段重复超过约 8-10 行且后续仍在增长；
  - 因重复导致一致性问题或测试成本明显上升。
- 平衡准则：先保留短小直接实现；当共享逻辑超过阈值，或已经带来一致性/测试负担，再抽成公用函数或基类。
- 示例（`process_text` / `process_image`）：
  - 若两者仅在 1-2 个分支不同，保留并排实现更清晰。
  - 若两者都有相同“预处理 -> 校验 -> 错误映射 -> 收尾”流程，且重复超过阈值，应抽公共 pipeline（如 `process_common`），各自只保留差异步骤。

### 新增字段链路追踪

给数据结构加字段时，追踪全链路：
- 后端：结构体 → Default → 所有构造字面量 → 序列化 → 测试 fixture
- 前端：类型 → composable → persistence（含输入校验） → 组件 → i18n
- 运行时缓存若依赖该字段，须处理配置热更新时的失效/重建

### 消息读取特别警告

- 聊天消息读取默认禁止直接整读 `Conversation.messages` 或 `state_read_conversation_cached`；新增功能必须先按“只读当前所需最小消息子集”设计，优先使用 metadata、recent page、message by id、before/after anchor、block page 等轻量路径。
- 只有在需求天然依赖完整消息集时，才允许整读；实现前必须先明确写出“为什么轻量读取不成立”，否则视为未完成设计。
- 任何“先直接整读，后面再优化”的做法默认不允许；如果缺少所需原子接口，应先补接口、记入 backlog 或停止扩张需求范围，不能把整读当临时方案长期留下。
- 新增或修改消息相关功能后，必须反向扫描调用链，确认没有顺手引入新的整读入口；不能只修当前点位，而放任后续功能继续复制整读写法。

### 多字段关联的 UI 设计

多个字段表达同一用户意图时，用一个控件统一表达，通过 encode/decode 转换 UI 状态与数据模型，避免无效状态组合。

### 提交信息规范
- 采用约定式提交（Conventional Commits），推荐格式：`type(scope): 简要中文描述`。
- 提交信息默认使用中文，便于与现有项目历史保持一致。
- 常用类型：`feat`、`fix`、`perf`、`refactor`、`docs`、`chore`。
- 标题可以只作概括（如 `重做 XX`、`统一为 XX`），不必在标题里堆细节；但凡涉及拆/合、增/删、展开/收起这类方向性变化，必须在该提交的正文里写清「从什么变成什么」，不能只留一个概括标题。发布汇总时正文与标题同等重要，只读标题写出的发布说明一律视为采集未完成。
- `fix` 提交必须写清楚修复的是哪个行为、由哪个 commit 引入：正文里注明引入提交的短哈希或标题，例如 `fix(chat): 修复 xxx 在流式渲染重构后未冲刷的问题（引入于 f8fefff28）`。发布流程据此区分「对旧版本真实 bug 的修复」与「本次发布范围内新功能开发中的中间修复」。
- 开发阶段不维护任何 changelog 文件，变更记录一律以 commit message 为准。
- Changelog 只在发版时生成：发布流程检查上一个 tag 到 HEAD 的全部 commit，按用户可感知的行为变化汇总为 `docs/changelog/releases/vX.Y.Z.md`。文案必须是用户视角，禁止搬运 commit 的技术描述（变量名、函数名、机制细节）。同一改动可保留 commit 与 changelog 两套表述。
- 汇总 changelog 时，只写用户可感知的行为变化（新增、改变或修复面向用户的能力）；纯底层改动——重构、内部接口收敛、异步化、性能实现细节、测试修复、脚手架、文档、规则调整——默认不写。若底层改动带来了用户可感知的体验变化（如切换会话不再卡顿、界面响应变快），才写，且必须写成用户口吻，不得描述实现手段。拿不准一条改动是否值得写时，默认不写。
- 不要把新变更追加到既有版本号文件；未提升版本号时，禁止执行 `pnpm changelog:build` 或更新生成文件；只有提升版本号并生成对应的 `docs/changelog/releases/vX.Y.Z.md` 时，才执行 `pnpm changelog:build`。
- 每次 `git commit` 前必须先修复并跑通本次改动影响到的全部测试；存在失败项时禁止提交。
- 不要把测试留到最后一次性再跑；开发过程中应边改边验证，尽早发现并修复失败。

### 计划与归档流程
- 仅在以下情况才生成计划文档：
  - 重构（大规模模块拆分、架构调整、技术栈迁移）
  - 全新功能域设计（涉及多模块、跨前后端、接口协议或数据模型设计）
  - 用户明确要求写计划
- 常规功能迭代、简单 UI 改动、文档更新、小修小补等日常工作，直接实现，不需要生成计划。
- 计划必须先得到用户明确确认后，才可进入实现阶段。
- 归档判定以用户结论为准，不以开发者自测或主观判断替代。
- 归档需要根据最新实现情况修正计划书，进行归档报告。
- 归档文件命名必须带日期，如 `20260220_重启FTS5混合检索计划.md`

### 日志等级与用法

| 等级 | 函数 | 适用场景 |
|---|---|---|
| error | `runtime_log_error` | 操作失败、非预期异常 |
| warn | `runtime_log_warn` | 降级、兜底、跳过、重试 |
| info | `runtime_log_info` | 正常流程节点、服务状态切换、用户可见行为变化 |
| debug | `runtime_log_debug` | 开发者排查细节、变量值、状态快照 |
| trace | 无函数，仅 `[TRACE]` 前缀触发 | 执行追踪 |

#### 实际行为

- `eprintln!` 宏在 `main.rs:29-33` 被重定义为 `runtime_log_info`，约 532 处调用全部走 info 级别。**新增日志不要用 `eprintln!` 打非 info 级别的内容，直接用对应 `runtime_log_*` 函数。**
- `normalize_runtime_log`（`debug_log_commands.rs:343`）会从消息前缀自动提升级别：`[ERROR]`→error、`[WARN]`→warn、`[INFO]`→info、`[DEBUG]`→debug、`[TRACE]`→trace。

### 日志约定
- 日志文案默认使用中文，避免中英混杂；仅在必要时保留英文标识（如集合名、配置键名、异常类名）。
- 任务型日志统一前缀：`[睡眠维护]`、`[睡眠]`、`[简单记忆回灌]`，便于平台日志检索。
- 状态表达统一使用：`开始`、`完成`、`跳过`、`失败`，不要再使用 `status=success/failed/skipped` 风格。
- 日志内容应包含可排障字段：任务名、触发条件（如供应商变化/模式）、关键计数（如写入条数/失败条数）、耗时毫秒。
- 异常日志必须带异常信息（Rust 使用 `{:?}` 或 `Display`，TypeScript 包含 `error.message` 和必要的 `error.stack`），避免只打印"失败"无上下文。
- 高频循环日志仅输出聚合信息，避免每条记录都打印 info/warn 级别日志；明细使用 debug 级别。
