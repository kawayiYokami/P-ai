---
name: p-ai-config-guide
description: 当需要配置 PAI 自身能力时阅读我：Skill 的发现安装与制作、MCP 的安装启停与排查、助理私有人格与组织维护、助理空间与项目 .pai 的边界划分，以及 AGENTS.md、Skill、workflow、计划等项目能力资产的沉淀方式。
---

# PAI 配置指南

## 三层边界

| 位置 | 管什么 |
| --- | --- |
| 当前会话工作区 | 当前项目的源码、配置、构建、测试与用户要求的交付 |
| 项目 `.pai/` | 只属于当前项目的 Skill、计划、报告、工作流、参考项目、隔离工作树、临时材料 |
| 助理空间 | 跨项目复用的预置与私有 Skill、MCP、人格私有配置、共享资源 |

助理空间不是通用临时目录：一次性调试报告、截图、实验脚本、构建产物放项目 `.pai/temp/`。不要为每个项目在助理空间建副本、`projects/` 或平行计划目录，也不要用助理空间绕开当前项目的 AGENTS.md、用户指令、权限约束与 Git 规则。

涉及当前项目时优先用当前会话工作区；发现项目级 Skill 或工作流缺失、过期、错误，就在项目范围内修，不要拿助理空间里的替代。

## 怎么用 config

PAI 自身配置一律走 config 工具，一次调用执行一条命令，先 `config "help"` 看命令面。当前开放五类：`agent`（人格与组织关系）、`skill`（只有 `skill ls`）、`mcp`（连接器）、`store`（能力商店）、`approot`（应用数据目录）。组织关系由人格之间的父子边构成，没有部门这一层。

写入流程固定：先看现状，再导出改文件，然后校验、预览、落盘。

```text
config "agent ls"                        # 或 get / example / tree
config "agent export <name-or-id> <file>"
# 用文件工具修改导出的 JSON
config "agent check <file>"
config "agent diff <name-or-id> <file>"
config "agent update <name-or-id> <file>"
```

- 不要直接手改 PAI 配置源文件；供应商配置当前未开放。
- 删除类命令是破坏性的，必须用户明确同意后才能执行，并且要带 `--confirmed`。
- `agent new` 与 `mcp add` 会立即持久化，先确认参数再执行。

### config 覆盖不到时

只有 config 的命令面确实覆盖不到时，才去数据目录里手动查看或编辑。先去问应用自己：

```text
config "approot"
```

它返回 appRoot（应用数据目录）、configPath（app_config.toml 全路径）、workspaceRoot（工作区）。安装版跟着系统约定走，便携版跟着可执行文件走，两者没有统一规律，一律以这个返回为准，不要按系统习惯猜路径。

appRoot 下常见内容：`config/`（主配置 app_config.toml、agents.json，以及其下的 `llm-workspace/` 工作区）、`state/`（运行态）、`chat/`（会话）、`memory/`、`task/`、`delegate/`、`avatars/`。

## Skill

- 用户提到 skill、技能、插件、市场、热门 skill，或你不确定当前任务是否已有现成 skill 时，先找 skill，不要直接硬做。
- 先用 config 的商店命令找和装：

```text
config "store ls"                        # 列出来源，--kind skill|mcp 只列一类
config "store search <关键词>"            # 默认搜 Skill，--source 指定来源，--kind mcp 搜 MCP，--page 翻页
config "store install <来源> <条目>"       # 装到工作区，默认关闭，需要在配置页启用
```

- 搜索结果里的来源与条目 id 原样照抄给 install，不要自己拼；已安装的条目会标记出来，别重复装。
- ClawHub 也可以用命令行直连：`npx clawhub@latest search "<关键词>"`、`npx clawhub@latest inspect <slug>`、`npx clawhub@latest install <slug> --workdir "<skill 目录的上一级>"`（必须显式给 `--workdir`）；网页 `https://clawhub.ai/` 用于人工浏览和交叉确认。
- 公开市场出现过恶意内容，安装或借鉴前先读全文，再看脚本和外链，确认没有危险命令。
- 只借鉴一部分时，按当前格式重写，不要整篇复制无关内容。

自己制作时，在 system skill directory path 下新建 `<skill-name>/`，至少包含 `SKILL.md`：

```md
---
name: your-skill
description: 当需要处理某类任务时阅读我。写清做什么、什么时候用。
---

# 正文：执行规则、步骤、边界处理
```

需要时再补 `scripts/`（可复用脚本）、`references/`（按需加载的细节）、`assets/`（模板与静态资源）。做完做一次最小验证，确认结构正确、内容可触发。

修改共享 Skill 前先确认它会影响哪些项目与会话；删除、重命名、覆盖已有 Skill 前先确认使用者和影响范围。

## MCP

用 config 管理 MCP，不要手改 MCP JSON。`mcp enable <name>` 之后由运行态负责启动、探测和刷新工具目录。

```text
config "mcp ls"
config "mcp get <name-or-id>"
config "mcp add <name> -- <command> [args...]"
config "mcp enable <name-or-id>"
config "mcp disable <name-or-id>"
config "mcp tools <name-or-id>"
config "mcp delete <name-or-id> --confirmed"
```

- 需要复杂定义时走 `export` → 改文件 → `check` → `diff` → `update`。MCP 的连接定义与策略文件在助理空间 `mcp/servers/` 与 `mcp/policies/` 下，正常情况下不要手写。
- 不要用 shell/exec 或普通 CLI 启动需要跨调用维持实例的 MCP server、浏览器自动化实例或守护进程；shell 只用于查环境、装依赖、看文件和跑短任务。
- 用户需要网页操作、浏览器自动化、Web UI 测试时用 Playwright MCP：`config "mcp add playwright -- npx -y @playwright/mcp@latest"`，然后 enable、tools。不要直接 `npx @playwright/mcp@latest`，那样挂不进运行态，后续调用无法复用同一个浏览器实例和页面上下文。
- 其他常用：`context7`（库文档）、`deepwiki`（仓库问答）、`tavily`（联网搜索，通常需要用户自己的 key）。
- 安装完成后至少确认 `mcp ls` 能看到、`mcp enable` 不报错、`mcp tools` 能看到工具或给出可排查的错误；失败要说明失败阶段、错误信息和下一步需要用户做什么。

## 私有人格与私有组织

私有人格放在 `{Assistant Space}/private-organization/personas/`，一个文件一个人格对象，不写回应用主配置。组织关系由人格自身承载：每个人格记录自己的直接下级人格，没有部门这一层。人格与父子边用 config 工具的 `agent` 组维护（`agent new`、`agent set-parent`、`agent clear-parent`、`agent parent`、`agent children`），不要用手改文件的方式调组织结构。

必填字段：`id`、`name`、`systemPrompt`。常用可选字段：

- `summary`：一句话简介，用于列表与委托清单。
- `apiConfigIds`：驱动模型，首个为主模型。
- `childAgentIds`：直接下级人格 id，语义是「可直接委托」。
- `residentSkillNames` / `optionalSkillNames`：常驻注入全文 / 只注入名字。
- `permissionControl`：权限约束，`skillNames` 只引用自定义工作区 skill，内置预设 skill 不必写。

写完调用 reload，并按返回的 `repairSummary` / `repairItems` 修错。约束：不能用系统保留 ID，不能与主配置中的人格同 ID，`childAgentIds` 引用的人格必须真实存在；`prompt` 仍兼容旧格式但优先写 `systemPrompt`；不要手写 `createdAt`、`updatedAt`、`source`、`scope` 这类运行态字段。

配置权限前先确认工具目录：`config "mcp ls"`、`config "mcp tools <name-or-id>"`、`config "skill ls"`。

## 项目能力资产（.pai）

在当前项目 `.pai/` 下生成和维护能力资产：AGENTS.md、Skill、workflow、计划与协作说明。写入和更新范围固定限制在该项目 `.pai/` 内；可以读项目上下文理解约束，但不承担 `.pai/` 之外的业务实现任务。用 exec 时只跑理解项目结构、检查能力资产和最小验证所需的命令，不要借助脚本改 `.pai/` 之外的文件。

## 完成前检查

1. 变更确实属于该位置该管的东西，而不是越界替别的层做决定。
2. 名称、说明、适用范围一致，当前配置下确实可用。
3. 没碰到当前项目的未提交改动和无关私有配置。
4. 删除或迁移前说明范围和影响，并取得用户明确同意。
5. 汇报变更位置、实际影响范围和验证结果；没做的项写明原因。
