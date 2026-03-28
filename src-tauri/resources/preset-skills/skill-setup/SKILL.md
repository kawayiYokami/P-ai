---
name: skill-setup
description: 当用户明确询问 skill 的发现、查找、安装、使用、配置、市场、热门 skill，或当前任务明显需要先判断是否已有可复用 skill 时，必须立刻阅读我。我会告诉你何时先发现 skill、如何判断是否应安装现成 skill，以及何时才应该自己制作 skill。
---

# Skill Discovery

## 规则
- 任何用户查询中只要直接出现 `skill`、`技能`、`插件`、`扩展`、`market`、`市场`、`热门 skill`、`怎么找 skill`、`怎么安装 skill` 这类意图，都应优先阅读我，不要先直接搜索网页或直接回答。
- 当你不确定当前任务是否已有现成 skill 可用时，先找 skill，不要直接硬做。
- 自己查 skill 正文和来源信息再决定是否安装或迁移，不要空想格式。
- 可以用 `clawhub` 搜索、查看、安装公开 skill；网页 `https://clawhub.ai/` 更适合做人工浏览和交叉确认。
- skill 放在 system skill directory path 下；这个路径会在提示词中显式给出。
- 每个 skill 一个目录，至少包含 `SKILL.md`。
- 需要时再补 `scripts/`、`references/`、`assets/`。
- 如果使用 `clawhub install`，必须显式指定安装目标，不要假设它会自动安装到当前应用的 skill 目录。
- 完成后做一次最小验证，确认结构正确、内容可触发。

## 发现 Skill
- 优先使用命令行搜索：
  - `npx clawhub@latest search "<关键词>"`
- 如果需要减少 `npx` 重复安装，也可以先安装本地或全局 CLI 再运行：
  - `npm install clawhub@0.9.0`
  - `.\node_modules\.bin\clawhub.cmd search "<关键词>"`
- 找到候选 skill 后，先查看详情：
  - `npx clawhub@latest inspect <skill-slug>`
- 再去网页 `https://clawhub.ai/` 查看说明、文件列表、更新时间、作者等信息做人工确认。
- 先判断这个 skill 是否真的匹配当前任务，再决定下一步是安装它，还是只借鉴它的一部分。
- 如果只是借鉴其中一部分，也要按当前项目的格式重写，不要原封不动复制一大堆无关说明。
- 最近公开 skill 市场出现过恶意内容争议，安装或迁移前先阅读全文，再看脚本和外链，确认没有危险命令。

## 安装现成 Skill
- 如果找到可直接复用的 skill，优先安装到当前 system skill directory path。
- 实测可用命令：
  - `npx clawhub@latest install <skill-slug> --workdir "<directory-that-contains-the-system-skill-directory>"`
- 这里的 `--workdir` 应该传 System skill directory path 的上一级目录，而不是 skill 目录本身。
- 安装结果会落到：
  - `<System skill directory path>/<skill-slug>/`
- 例如：
  - `npx clawhub@latest install sonoscli --workdir "<directory-that-contains-the-system-skill-directory>"`
- 安装完成后，skill 会出现在：
  - `<System skill directory path>/sonoscli/SKILL.md`
- 不要依赖默认目录；省略安装目标后可能会安装到错误位置。
- 安装完成后，确认对应 skill 目录和 `SKILL.md` 已经真正出现在当前 system skill directory path 下。

## 自己制作 Skill
- 只有当没有合适的现成 skill，或你只想借鉴部分内容时，才自己制作 skill。
- 自己制作时，在 system skill directory path 下新建 `<skill-name>/` 目录。
- 把整理后的 `SKILL.md`、脚本、参考资料按当前项目格式放进去，不要原封不动复制无关内容。

## SKILL.md 格式
- 文件必须以 YAML frontmatter 开头和结束。
- frontmatter 至少有两个字段：`name`、`description`。
- `name` 建议小写加连字符，和目录名保持一致。
- `description` 要写清楚这个 skill 做什么、什么时候该用。
- frontmatter 后面再写 Markdown 正文，放执行规则、步骤、边界处理。

## 最小结构
```text
<system-skill-directory-path>/
  your-skill/
    SKILL.md
```

## 最小示例
```md
---
name: your-skill
description: 当需要处理某类任务时，必须立刻阅读我。我会告诉你它适用于哪些场景以及应该怎样执行。
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
