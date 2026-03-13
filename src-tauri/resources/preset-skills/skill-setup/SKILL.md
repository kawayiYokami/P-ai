---
name: skill-setup
description: 如何自行安装和编写 skill。适用于需要补齐、创建、整理工作区 skill 的场景。
---

# Skill Setup

## 规则
- `<workspace>` 只是占位符，不是固定目录名；它表示你当前 shell 的启动工作空间。
- 你只能在这个工作空间内工作；不要假设或访问工作空间外路径。
- 自己查官方文档再安装或编写，不要空想格式。
- 可以去 `https://clawhub.ai/` 搜索公开 skill，把它当作灵感来源、现成模板来源、GitHub 来源。
- skill 放在 `<workspace>/skills/`。
- 每个 skill 一个目录，至少包含 `SKILL.md`。
- 需要时再补 `scripts/`、`references/`、`assets/`。
- 完成后做一次最小验证，确认结构正确、内容可触发。

## 如何使用 ClawHub
- 打开 `https://clawhub.ai/`，搜索你要的能力关键词。
- 进入 skill 详情页，先看说明、文件列表、更新时间、作者、下载量。
- 优先查看 skill 的 `SKILL.md` 正文和关联 GitHub 仓库，理解它到底怎么工作。
- 不要直接照搬别的生态的安装命令；这里更适合把它的说明、模板、脚本思路迁移到当前工作区。
- 如果内容合适，就在 `<workspace>/skills/<skill-name>/` 下新建目录，把整理后的 `SKILL.md`、脚本、参考资料放进去。
- 如果只是借鉴其中一部分，也要按当前项目的格式重写，不要原封不动复制一大堆无关说明。
- 最近公开 skill 市场出现过恶意内容争议，安装或迁移前先阅读全文，再看脚本和外链，确认没有危险命令。

## SKILL.md 格式
- 文件必须以 YAML frontmatter 开头和结束。
- frontmatter 至少有两个字段：`name`、`description`。
- `name` 建议小写加连字符，和目录名保持一致。
- `description` 要写清楚这个 skill 做什么、什么时候该用。
- frontmatter 后面再写 Markdown 正文，放执行规则、步骤、边界处理。

## 最小结构
```text
<workspace>/skills/
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
