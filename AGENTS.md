# AGENT.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

π师傅 是一个 Windows 优先的桌面 AI 助手，使用全局热键呼出/隐藏对话窗口，常驻系统托盘。技术栈为 Tauri 2 (Rust) + Vue 3 (TypeScript) + Vite + DaisyUI，包管理使用 pnpm。

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
| `features/system/windowing.rs` | 窗口定位、显示、隐藏、托盘 |
| `features/memory/matcher.rs` | 记忆搜索与匹配 |

### Vue 前端 — Composable 驱动

前端无全局状态库（无 Vuex/Pinia），状态通过 Vue Composition API 的 reactive refs 管理。核心逻辑封装在 composables 中，组件层很薄。

关键 composable 分组：
- **shell/**: `use-app-bootstrap` (初始化)、`use-app-theme`、`use-window-shell`、`use-app-lifecycle`
- **chat/**: `use-chat-flow` (流式缓冲与 delta 处理)、`use-chat-runtime` (会话持久化)、`use-chat-turns` (上下文窗口计算)、`use-chat-media` (图片/音频)、`use-speech-recording` (本地+远程 STT)
- **config/**: `use-config-persistence` (加载/保存)、`use-config-autosave` (250ms 防抖自动保存)、`use-config-runtime` (模型列表刷新)

### 多窗口

Tauri 管理 3 个无边框窗口：`main`（配置，400×620）、`chat`（对话，420×700）、`archives`（归档，520×720）。`App.vue` 根据窗口 label 切换视图模式。

### 数据持久化

所有数据以 JSON/TOML 文件存储在 `ProjectDirs` 应用目录下（无数据库）：
- `config.toml` — API 配置、热键、工具开关
- `conversations.json` — 活跃与归档对话
- `agents.json` — 人格配置
- `memories.json` — 记忆条目

### 支持的 API 格式

`openai`（OpenAI/DeepSeek/Kimi）、`anthropic`（Claude）、`gemini`（Google）、`openai_tts`（远程 STT）

## 开发约定

### Rust 规则

- 禁止 `unwrap()` / `expect()`（测试除外），统一使用 `Result` 传递可读错误
- 网络与 I/O 走异步，不阻塞 UI
- 改动后优先保证 `cargo check` 通过
- 文件 < 1500 行，函数 < 100 行，用注释分区（`// ========== xxx ==========`）

### 前端规则

- DaisyUI 组件优先，避免手写重复样式
- 配置页"有改动才允许保存"，保存后状态立即回写
- 对话窗口保持极简，外链走系统浏览器

### 代码组织原则

- 保持模块拆分，避免超大单文件；`紧凑优先` 是起步策略，不是长期豁免。
- 文件软阈值：建议单文件 500-800 行、公开函数不超过 20 个；超过后需在评审中说明并跟踪拆分计划。
- 当文件持续增长超过阈值时，重点关注导航困难、合并冲突、测试回归成本；必要时增加评审频率或补充变更统计。
- 注释说明意图而非实现。

#### “紧凑优先” 与 “按功能组织” 决策指南

- `紧凑优先` 适用：功能仍集中在一个场景、强耦合且变更总在同一处，使用注释分区（如 `// ==================== 配置管理 ====================`）可快速定位。
- `按功能组织` 适用：已经出现清晰职责边界，或多人并行开发导致同文件冲突频繁，应按 `features/chat/`、`features/archive/` 等目录分组。
- 判断标准：
  - 规模：单文件接近 500-800 行，或公开函数数量接近 20。
  - 耦合：同文件内出现多个低相关子域（如配置读写、网络调用、视图拼装混在一起）。
  - 协作：多人经常改同文件、冲突率高。
  - 可测试性：难以对局部逻辑做独立单测。
  - 变更成本：每次重构/编译/回归都要牵动大范围代码。
- 迁移触发器：单文件超过约 800 行或公开函数超过约 20 个，且模块边界已明显分离时，优先拆分。
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

### 提交信息规范
- 采用约定式提交（Conventional Commits），推荐格式：`type(scope): 简要中文描述`。
- 提交信息默认使用中文，便于与现有项目历史保持一致。
- 常用类型：`feat`、`fix`、`perf`、`refactor`、`docs`、`chore`。
- 每次 `git commit` 前必须先更新 `CHANGELOG.md`，确保变更可追溯；未更新时禁止提交。

### 计划与归档流程
- 新功能开发必须先产出计划文档（放在 `plan/` 目录）。
- 计划必须先得到用户明确确认后，才可进入实现阶段。
- 功能完成后，必须先等待用户在生产环境测试并明确反馈“通过”，之后才可将计划归档到 `plan/done/`。
- 归档判定以用户结论为准，不以开发者自测或主观判断替代。
- 归档需要根据最新实现情况修正计划书，进行归档报告。
- 归档文件命名必须带日期，如 `20260220_重启FTS5混合检索计划.md`

### 日志标准
- 日志文案默认使用中文，避免中英混杂；仅在必要时保留英文标识（如集合名、配置键名、异常类名）。
- 任务型日志统一前缀：`[睡眠维护]`、`[睡眠]`、`[简单记忆回灌]`，便于平台日志检索。
- 状态表达统一使用：`开始`、`完成`、`跳过`、`失败`，不要再使用 `status=success/failed/skipped` 风格。
- 日志内容应包含可排障字段：任务名、触发条件（如供应商变化/模式）、关键计数（如写入条数/失败条数）、耗时毫秒。
- 异常日志必须带异常信息（Rust 使用 `{:?}` 或 `Display`，TypeScript 包含 `error.message` 和必要的 `error.stack`），避免只打印"失败"无上下文。
- 高频循环日志仅输出聚合信息，避免每条记录都打印 info/warn 级别日志；明细使用 debug/trace 级别。
- 用户可见行为变化（例如切换模式、禁用功能、跳过原因）必须有一条清晰 INFO 日志。

