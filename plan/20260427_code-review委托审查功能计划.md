# 20260427_code-review 委托审查功能计划

## 当前状态

本计划已进入实现后回顾状态。早期设想中的“主 AI 先获取 diff、再把 diff 传给副手、最后把 JSON 转 Markdown 写回 batch.report”已经废弃。

当前真实链路是：前端只负责选择审查目标；后端包装层创建审查报告记录并发起 `delegate sync`；副手部门按 `code-review` skill 自行只读获取必要 diff 与上下文；后端保存副手最终原始文本；前端负责 JSON 解析与结构化展示。

关键口径：

1. 前端不运行 git，不拼 diff，不判断审查内容。
2. 后端不把副手 JSON 转 Markdown，也不把 JSON 解析失败视为任务失败。
3. 报告主数据是副手最终原始文本 `reportText`。
4. 前端解析 JSON 成功则渲染结构化 UI；解析失败则展示原文。
5. `review_title` 只用于报告列表标题提取，解析失败不影响报告保存。

## 目标

在 P-ai 工具审查侧边栏中提供「AI 审查代码改动」能力，并保持职责分层清晰：

1. 用户在前端选择审查范围：未提交改动 / 与主分支差异 / 指定 commit / 自定义。
2. 前端把范围与目标提交给后端命令，不直接参与 git 分析。
3. 后端创建 `pending` 报告记录，然后用 `delegate sync` 委托副手部门审查。
4. 副手部门基于 `code-review` skill 在工作区内只读获取真实 diff 与必要上下文。
5. 后端把副手最终原始文本保存为报告 `reportText`，成功/失败状态写入报告记录。
6. 前端按报告记录展示历史，JSON 解析成功则结构化展示 Findings，失败则展示原文。

## 非目标

- 不做 Guardian 式的实时审批审查（那是另一层，P-ai 走的是用户事后审路线）
- 不让前端承担 diff 获取与审查判断
- 不做 PR 级别的多 subagent 并行审查
- 首版不接入外部 CI/CD
- 不再把审查报告伪装成聊天消息或 batch.report
- 不把副手结果转换成 Markdown 后再持久化

## 核心设计

### 1. 审查入口

**位置**：工具审查侧边栏的“审查报告”链路。

入口职责只做目标选择与提交，不做审查判断。支持范围：

| 范围 | git 命令 | 说明 |
|---|---|---|
| 未提交改动 | 由副手按规约只读获取 | 包含已暂存与未暂存 |
| 与主分支差异 | 由副手按规约只读获取 | 相对于 main 的所有改动 |
| 指定 commit | 前端只传目标 commit | 单次提交的改动 |
| 自定义范围 | 传递用户原始描述 | 不由前端猜测命令 |

### 2. 审查提示词（Skill）

创建 `code-review` skill，位于 [SKILL.md](E:/github/paimonhome/skills/code-review/SKILL.md)。

目录规范：

- 放在 `paimonhome/skills` 根下
- 每个 skill 一个目录
- 目录名与 skill name 一致
- 不额外增加分类子目录

SKILL.md 定义内容：

- 审查标准：缺陷判定、优先级、置信度、整体结论。
- 输出协议：纯 JSON，不包 Markdown 代码块。
- 副手执行约束：必须基于真实 diff 与必要上下文，不凭空扩展审查范围。
- 主助理回填要求：保存副手最终原始文本，不转 Markdown。

审查指令写入委托 `instruction` 字段；背景只描述审查目标与工作区，不预塞主 AI 猜出来的 diff。

这样做的原因只有一个：**既是内置能力，用户又能直接改 skill 文本**。

### 3. 委托调度

```text
用户在 ToolReviewSidebar 选择审查范围
  → 前端调用 submit_tool_review_code
  → 后端创建 pending 报告记录
  → 后端 delegate sync → 副手部门
     - instruction: 审查目标、输出协议、交付要求
     - background: 工作区与范围信息
  → 副手按 skill 自行只读读取真实 diff
  → 副手最终文本返回
  → 后端保存原始最终文本到 reportText
  → 前端报告列表刷新并展示 success / failed
  → JSON 解析成功渲染 UI，失败展示原文
```

委托类型：`sync`。结构化消费必须优先读取 `final_response_text`，中间工具前自言自语只属于会话历史，不属于委托交付物。

### 4. JSON 输出协议

副手返回的 JSON 结构：

```json
{
  "findings": [
    {
      "title": "≤ 80 字符，祈使句",
      "body": "解释为什么这是问题，引用文件/行号",
      "confidence_score": 0.95,
      "priority": 0,
      "code_location": {
        "absolute_file_path": "E:/project/src/foo.rs",
        "line_range": { "start": 10, "end": 15 }
      }
    }
  ],
  "overall_correctness": "patch is correct",
  "overall_explanation": "1-3 句判决理由",
  "overall_confidence_score": 0.9
}
```

- `priority`：P0=0, P1=1, P2=2, P3=3
- `review_title`：10 到 20 个中文字符，用于报告列表标题
- `code_location` 必须落在 diff 范围内
- JSON 不允许被 markdown 代码块包裹

### 5. 前端渲染

前端直接以 `reportText` 为主数据：

- JSON 解析成功：展示整体判定、判定说明、置信度、Findings 折叠列表。
- Findings 标题行展示：复选框、风险等级彩色点、标题、置信度。
- P0/P1/P2/P3 用四种颜色点表达，不在标题行额外显示文字。
- 展开 Finding 后展示 body 与位置。
- `findings: []` 也走结构化 UI，显示“未发现问题”。
- JSON 解析失败：展示原始 `reportText`。
- 复制 / 附加只输出已勾选 Findings 的语言无关文本格式：`[标题]\n正文`。

## 实施步骤

### 已落地步骤

1. `code-review` skill 已放在 `E:/github/paimonhome/skills/code-review/SKILL.md`。
2. 前端审查入口已接入 `submit_tool_review_code` / `submit_tool_review_batch`。
3. 后端报告记录已采用独立报告流，不再写入 `batch.report`。
4. 委托结果语义已拆分：展示/持久化历史与最终交付文本分离。
5. 报告详情已改为“原始文本为主数据，前端解析渲染”的模式。

## 实际影响文件

### 新增

- [SKILL.md](E:/github/paimonhome/skills/code-review/SKILL.md) — 审查 skill 定义

### 修改

- [ToolReviewSidebar.vue](E:/github/easy_call_ai/src/features/chat/components/ToolReviewSidebar.vue) — 审查报告 tab、报告列表、详情弹窗、Findings 结构化 UI
- [use-chat-tool-review.ts](E:/github/easy_call_ai/src/features/chat/composables/use-chat-tool-review.ts) — 审查报告列表、提交、删除、重新生成状态
- [ChatView.vue](E:/github/easy_call_ai/src/features/chat/views/ChatView.vue) — 接线报告提交、删除、重新生成
- [tool_review.rs](E:/github/easy_call_ai/src-tauri/src/features/system/commands/tool_review.rs) — 审查报告命令、委托包装、报告持久化

### 不动

- 委托基础设施（`delegate_dispatch` / `delegate_runtime`）— 已为持久化委托会话和最终文本语义做过配合调整
- 副手部门配置 — 不变
- 终端审批链路 — 不改
- `ToolReviewChangesDialog` / 现有报告模态窗 — 不改，直接复用

## 已确认决策

1. **审查入口位置**：工具审查侧边栏内，不在聊天输入区新增入口
2. **结果展示位置**：审查报告 tab 的报告记录与详情弹窗
3. **skill 存放**：放在 `paimonhome/skills/code-review/`，既内置又允许用户直接修改
4. **报告主数据**：保存副手最终原始文本，不转 Markdown
5. **结构化展示**：只在前端 JSON 解析成功后渲染 UI，失败显示原文