# 终端工具与路径授权实施计划

## 目标
- 在现有 `toolcall` 架构中新增可控的终端执行能力，保持“每次执行独立进程（无状态）”。
- 提供跨平台 shell 选择逻辑，并满足 Windows 启动优先级：`pwsh -> powershell -> cmd`。
- 引入 LLM 持久私人工作目录（全局单目录），在该目录内默认合法执行。
- 新增路径授权工具，允许在“当前会话”内扩展可执行目录范围。

## 需求冻结（来自本次确认）
1. 终端工具默认关闭。
2. 每次终端指令独立执行，不保留上一条命令的 shell 上下文。
3. Windows shell 探测优先级：
   - `C:\Program Files\PowerShell\7\pwsh.exe`
   - `powershell`
   - `cmd`
4. 非法工作路径需要先授权。
5. 增加 `terminal_request_path_access` 工具，请求通过后仅当前会话可用。
6. 不做“同命令免审批”记忆。
7. 引入“LLM 私人空间”作为持久目录，在此目录内全部操作默认合法。
8. 本阶段先完成可运行版本，复杂审批 UI 后续可迭代增强。

## 架构决策
- 集成方式：内置 `toolcall`（不先做 MCP 服务化）。
- 执行内核：`tokio::process::Command`。
- 终端模式：`stateless`（每次独立进程）。
- 授权模型：
  - 默认允许目录：`llm workspace`（持久目录）。
  - 额外目录：通过 `terminal_request_path_access` 写入“会话内授权集合”。
- 会话标识：以当前聊天会话标识（conversation/session key）作为授权隔离键。

## 详细实现项

### A. 数据与状态层
- `AppState` 增加：
  - `llm_workspace_path`
  - `terminal_path_grants`（会话 -> 已授权路径集合）
- 启动时确保 `llm_workspace_path` 目录存在。

### B. 终端与安全核心
- 新增终端执行模块：
  - 跨平台 shell 探测函数
  - 命令执行函数（超时、输出截断、退出码）
  - cwd 规范化与合法性判定
- `terminal_exec` 返回结构化结果（stdout/stderr/exitCode/shellPath/cwd 等）。

### C. 新增工具
- `terminal_exec`
- `terminal_request_path_access`
- 将两者接入：
  - DeepSeek tools schema
  - OpenAI/Gemini/Anthropic rig tools 列表
  - 运行时 `check_tools_status`

### D. 配置与 UI 同步
- 默认工具列表加入：
  - `terminal-exec`（默认 `enabled=false`）
  - `terminal-request-path-access`（默认 `enabled=false`）
- 工具页显示文案与状态。

### E. 验证
- `cargo check`
- `pnpm typecheck`
- 手测建议：
  - 在 workspace 下执行 `pwd/cd/git status` 等
  - 未授权目录执行应被拒绝
  - 先调用路径授权工具后可在该路径执行

## 风险与后续
- 当前阶段先聚焦“路径授权 + 无状态执行”，更细粒度的“文件修改审批策略”后续增强。
- 后续可扩展：审批弹窗、命令风险分级、审计日志落盘、网络命令策略。

## 完成标准（DoD）
- 工具可被模型调用并稳定返回结构化结果。
- 默认仅 workspace 可执行，路径授权后会话内生效。
- 终端工具默认关闭，配置页可见可控。
- 三端 shell 探测逻辑可运行（重点验证 Windows 优先级）。

## 实施进度（2026-02-17）
- [x] A. `AppState` 已增加 `llm_workspace_path`、会话路径授权集合、审批等待队列。
- [x] B. 已完成跨平台 shell 探测、无状态执行、输出截断、超时与高风险命令阻断。
- [x] C. `terminal_exec` 与 `terminal_request_path_access` 已接入各模型工具链与状态检查。
- [x] D. 配置与工具页已加入终端工具并保持默认关闭。
- [x] E. 已通过 `cargo check` 与 `pnpm typecheck`。
- [x] 终端审批已从系统对话框切换为应用内 DaisyUI 弹窗（事件请求 + 前端回传）。
