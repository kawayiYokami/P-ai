---
name: skill-setup
description: 如何自行安装和编写 skill。适用于需要补齐、创建、整理工作区 skill 的场景。
---

# Skill Setup

## 规则
- llm-workspace 只是工作区根目录占位写法，表示当前 LLM 的工作区，不是固定目录名。
- 自己查官方文档再安装或编写，不要空想格式。
- skill 放在 `llm-workspace/skills/`。
- 每个 skill 一个目录，至少包含 `SKILL.md`。
- 需要时再补 `scripts/`、`references/`、`assets/`。
- 完成后做一次最小验证，确认结构正确、内容可触发。

## SKILL.md 格式
- 文件必须以 YAML frontmatter 开头和结束。
- frontmatter 至少有两个字段：`name`、`description`。
- `name` 建议小写加连字符，和目录名保持一致。
- `description` 要写清楚这个 skill 做什么、什么时候该用。
- frontmatter 后面再写 Markdown 正文，放执行规则、步骤、边界处理。

## 最小结构
```text
llm-workspace/skills/
  your-skill/
    SKILL.md
```

## 最小示例
```md
---
name: your-skill
description: 处理某类任务，适用于某些场景
---

# Instructions

1. 明确输入和目标。
2. 按固定流程执行。
3. 输出结构化结果。
```

## 目录说明
- `scripts/`：放可执行脚本，适合固定流程或需要稳定复用的逻辑。
- `references/`：放按需加载的参考资料，不要把大段细节全塞进 `SKILL.md`。
- `assets/`：放模板、静态资源或产物依赖文件。

## 输出
- 说明新增或修改了什么 skill。
- 说明采用了什么安装或编写方式。
- 说明验证结果。
- 没做的项要写明原因。