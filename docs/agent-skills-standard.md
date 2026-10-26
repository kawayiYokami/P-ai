# Agent Skills 通用标准完整文档

> 状态：外部标准摘录，不是 PAI 自己的实现说明。项目内 Skill 的发现、安装与边界见助理空间的配置指南，不把本文当时代码入口。

## 一、什么是 Agent Skills

Agent Skills 是一个**开放格式标准**，用于通过可发现的指令、脚本和资源文件夹来赋予 AI Agent 新的能力和专业技能。

**核心理念：Write Once, Use Everywhere**

- 一个技能可以在多个 AI Agent 中使用
- 无论是 Claude Desktop、Cursor、GitHub Copilot、VS Code 还是其他平台
- 一次编写，到处运行

---

## 二、为什么需要 Agent Skills

### 2.1 解决的核心问题

传统方法将所有指令塞入系统提示词会导致：

- **上下文爆炸**：提示词变得过长，成本飙升
- **维护困难**：修改一个功能需要重构整个提示词
- **无复用性**：无法在不同 Agent 间共享能力
- **扩展性差**：添加新功能需要重新设计架构

### 2.2 Agent Skills 的优势

- **渐进式披露**：按需加载，显著减少初始上下文消耗
- **模块化**：每个技能独立，易于测试和版本控制
- **跨平台兼容**：统一规范，支持 20+ 平台
- **可移植性**：技能只是文件，易于编辑、版本控制和分享

---

## 三、标准规范

### 3.1 目录结构

一个技能是一个目录，至少包含一个 `SKILL.md` 文件：

```
skill-name/
├── SKILL.md          # 必需：指令 + 元数据
├── LICENSE.txt       # 可选：许可证文件
├── scripts/          # 可选：可执行代码
│   ├── process.py
│   └── helper.sh
├── references/       # 可选：参考文档
│   ├── REFERENCE.md
│   ├── FORMS.md
│   └── domain-specific.md
└── assets/           # 可选：模板、资源
    ├── template.json
    └── images/
```

### 3.2 SKILL.md 格式

SKILL.md 文件必须包含 **YAML 前置数据** 后跟 **Markdown 内容**：

```yaml
---
name: skill-identifier
description: Brief description of what this skill does and when to use it
license: MIT
compatibility: Node.js >= 20
metadata:
  author: Your Name
  version: 1.0.0
allowed-tools: read_file write_file run_shell_command
---

# Skill Instructions

Your detailed instructions here...
```

### 3.3 前置数据字段（必需）

| 字段 | 必需 | 约束 |
|------|------|------|
| `name` | 是 | 最多 64 字符。仅小写字母、数字和连字符。不能以连字符开头或结尾。不能包含连续连字符（`--`）。必须与父目录名匹配 |
| `description` | 是 | 最多 1024 字符。非空。描述技能做什么以及何时使用 |

### 3.4 前置数据字段（可选）

| 字段 | 约束 |
|------|------|
| `license` | 许可证名称或对捆绑许可证文件的引用 |
| `compatibility` | 最多 500 字符。指示环境要求（预期产品、系统包、网络访问等） |
| `metadata` | 任意键值映射，用于附加元数据 |
| `allowed-tools` | 空格分隔的预批准工具列表（实验性） |

### 3.5 name 字段规则

- 必须 1-64 个字符
- 仅包含 ASCII 小写英文字母、数字和连字符（`a-z0-9` 和 `-`）
- 不能以 `-` 开头或结尾
- 不能包含连续连字符（`--`）
- **必须与父目录名匹配**

### 3.6 description 字段规则

- 必须 1-1024 个字符
- 应描述技能做什么以及何时使用
- 应包含有助于 Agent 识别相关任务的特定关键词

### 3.7 正文内容

前置数据后的 Markdown 正文包含技能指令。没有格式限制。

**推荐部分：**
- 分步说明
- 输入和输出示例
- 常见边缘情况

**最佳实践：**
- 将 SKILL.md 保持在 500 行以下
- 将详细参考材料移至单独文件

---

## 四、渐进式披露架构

这是 Agent Skills 最重要的创新。

### 4.1 三层加载机制

#### Layer 1: 元数据（始终加载）
- **内容**：YAML 前置数据（name + description）
- **加载时机**：启动时
- **大小**：约 100 tokens
- **目的**：发现和匹配

#### Layer 2: 指令（按需加载）
- **内容**：完整的 SKILL.md 正文
- **加载时机**：技能激活时
- **大小**：推荐 < 5000 tokens
- **目的**：执行任务

#### Layer 3: 资源（按需加载）
- **内容**：scripts/、references/、assets/ 中的文件
- **加载时机**：需要时
- **大小**：动态
- **目的**：补充资源

### 4.2 工作流程

```
启动：加载所有技能的 name + description（每个技能约 100 tokens；若有 N 个技能，总计约 N × 100 tokens）
   ↓
匹配：识别相关技能
   ↓
激活：读取完整 SKILL.md 指令（约 5000 tokens）
   ↓
执行：按需加载脚本、资源文件
```

### 4.3 优势

- **显著减少初始上下文消耗**
- **支持数百个技能而不影响性能**
- **降低 API 调用成本**

---

## 五、跨平台兼容性

### 5.1 支持的平台

Agent Skills 是一个开放标准，被以下平台支持：

| 平台 | 技能目录 | 作用域 |
|------|----------|--------|
| **Claude Desktop** | `~/.claude/skills/` | 全局 |
| **Claude Code** | `~/.claude/skills/` 或 `.claude/skills/` | 全局/项目 |
| **Cursor** | `.cursor/skills/` | 项目级 |
| **GitHub Copilot** | `.github/skills/` | 项目级 |
| **VS Code** | `.vscode/skills/` | 项目级 |
| **Windsurf** | `.windsurf/skills/` | 项目级 |
| **Gemini CLI** | `~/.gemini/skills/` | 全局 |
| **Kilo Code** | `~/.kilocode/skills/` | 全局 |
| **OpenCode** | `~/.opencode/skills/` | 全局 |
| **Codex CLI** | `~/.codex/skills/` | 全局 |

以及 20+ 其他平台...

### 5.2 互操作性

同一个技能文件夹可以在所有支持 Agent Skills 标准的平台上工作，无需修改。

---

## 六、技能设计原则

### 6.1 原则 1：明确 Agent 不应该决定什么

**核心思想**：如果需要一致性，不要留给模型。

**示例**：
- 评分逻辑 → 放入脚本
- CLI 命令 → 硬编码
- SQL 查询 → 预定义
- 命名约定 → 固定规则

**原因**：
- LLM 不确定性会导致不一致
- 相同输入应该产生相同输出
- 可重现性是关键

### 6.2 原则 2：明确 Agent 应该决定什么

**核心思想**：如果需要理解上下文、生成新内容或对话，这是 Agent 的工作。

**Agent 的优势**：
- **解释**：理解结果并用自然语言解释
- **行动**：基于上下文生成新内容
- **对话**：根据用户需求提供建议

**双区域架构**：

| 区域 | 负责方 | 原因 |
|------|--------|------|
| 规则和执行 | 脚本、模板、硬规则 | 相同输入，相同输出 — 每次 |
| 解释和行动 | Agent | 每个项目都不同；每次对话都不同 |

### 6.3 原则 3：写宪法，而不是建议

**核心思想**：SKILL.md 是契约，不是建议。

**为什么需要**：
- LLM 天性乐于助人
- 会软化坏消息
- 会添加警告
- 会跳过"不必要"的步骤

**防御性设计**：
- 明确规则
- 具体步骤
- 边缘情况处理
- 精确约束

**示例**：
```markdown
## 约束
- 永远不要覆盖、调整或重新计算脚本的任何分数
- 永远不要从报告中添加或删除检查
- 如果脚本说检查失败，原样显示
- 严格遵循特定的格式模板
```

### 6.4 原则 4：为弧线设计

**核心思想**：最好的技能不只是工具，它们创造对话弧线。

**设计问题**：用户在看到第一个结果后会想要做什么？

**示例**：
- 报告说"缺少 AGENTS.md"
- 用户问："什么是 AGENTS.md？"
- Agent 解释
- 用户问："你能帮我起草一个吗？"
- Agent 生成
- 用户说："再次运行检查"
- 分数实时提升

**关键**：输出成为输入，使 Agent 在接下来的步骤中更有用。

---

## 七、技能分类

### 7.1 文档处理技能
- **pdf**：PDF 操作（提取、创建、合并、拆分）
- **docx**：Word 文档（创建、编辑、分析）
- **pptx**：PowerPoint（创建、编辑、分析）
- **xlsx**：Excel（数据分析、可视化）

### 7.2 开发技能
- **code-review**：代码质量分析
- **testing**：自动化测试
- **debugging**：调试辅助
- **mcp-builder**：MCP 服务器开发

### 7.3 工作流技能
- **commit**：提交工作流
- **review**：代码审查
- **deployment**：部署流程
- **experiment**：实验设置

### 7.4 专业领域技能
- **legal**：法律审查
- **finance**：财务分析
- **healthcare**：医疗流程
- **oncall**：运维手册

---

## 八、最佳实践

### 8.1 编写 SKILL.md

- **保持简洁**：保持在 500 行以下
- **结构清晰**：使用清晰的标题和章节
- **提供示例**：包含输入输出示例
- **处理边缘情况**：记录常见边缘情况

### 8.2 组织技能资源

- **scripts/**：自包含或清楚记录依赖
- **references/**：详细参考文档
- **assets/**：模板和静态资源

### 8.3 版本控制

- 使用 Git 管理技能版本
- 在 metadata 中包含版本信息
- 遵循语义化版本控制

### 8.4 许可证

- 明确指定许可证
- 在技能目录中包含 LICENSE 文件
- 尊重第三方许可证

---

## 九、技术实现

### 9.1 技能发现

Agent 通过以下方式发现技能：

1. **扫描目录**：扫描标准位置（`~/.claude/skills/`, `.github/skills/` 等）
2. **读取元数据**：仅读取 SKILL.md 的前置数据
3. **建立索引**：建立技能名称和描述的索引

### 9.2 技能匹配

Agent 通过以下方式匹配技能：

1. **关键词匹配**：用户请求包含技能描述中的关键词
2. **任务类型匹配**：任务类型与技能功能匹配
3. **上下文感知**：基于当前项目上下文自动选择
4. **显式调用**：用户明确指定使用某个技能

### 9.3 技能加载

1. **按需加载**：仅在需要时加载完整指令
2. **延迟加载**：资源文件按需加载
3. **缓存策略**：合理使用缓存减少重复加载

### 9.4 技能执行

1. **解析指令**：解析 SKILL.md 中的指令
2. **执行步骤**：按照指令逐步执行
3. **调用工具**：调用必要的工具
4. **返回结果**：返回执行结果

---

## 十、生态系统

### 10.1 官方资源

- **官方网站**：https://agentskills.io/
- **GitHub 仓库**：https://github.com/agentskills/agentskills
- **规范文档**：https://agentskills.io/specification

### 10.2 社区资源

- **Awesome Agent Skills**：https://github.com/VoltAgent/awesome-agent-skills（已验证）
- **AI Agent Skills Repository**：https://github.com/hoodini/ai-agents-skills（已验证）
- **Claude Skills Examples**：https://github.com/anthropics/skills

### 10.3 学习资源

- **Agent Skills 深度解析**：Medium 系列文章
- **Claude Agent Skills 指南**：Claude 官方文档
- **最佳实践**：各平台文档

---

## 十一、总结

Agent Skills 是 AI Agent 能力管理的未来方向。通过：

- **渐进式披露**：高效使用上下文
- **模块化设计**：易于维护和扩展
- **跨平台兼容**：一次编写，到处运行
- **标准化规范**：确保互操作性

它解决了传统方法的痛点，为 AI Agent 提供了可扩展、可维护、可移植的能力扩展机制。

---

## 参考资料

- Agent Skills Specification: https://agentskills.io/specification
- GitHub Repository: https://github.com/agentskills/agentskills
- Claude Documentation: https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview
- Block Engineering Blog: https://engineering.block.xyz/blog/3-principles-for-designing-agent-skills
