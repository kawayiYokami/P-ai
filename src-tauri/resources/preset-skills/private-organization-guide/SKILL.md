---
name: private-organization-guide
description: 当需要在助理私域中维护私有人格、组织关系、模型或权限时，必须立刻阅读我。我会告诉你如何用 JSON 文件声明私有组织，并通过 reload 让配置、Skill 与 MCP 权限生效。
---

# 私有组织指南

私有组织用于给当前助理补充专属人格。
它们存放在助理空间中，不写回应用主配置。

组织关系由人格自身承载：每个人格记录自己的直接下级人格，不另设部门这一层。

## 目录

`{Assistant Space}` 是 PAI 的系统级助理空间，对应终端工作空间中 level 为“系统”的路径。

- 私有人格：`{Assistant Space}/private-organization/personas/`

只在这个目录中新增或修改 JSON 文件。
不要假设或访问助理空间外路径。

## 工作流

1. 先设计私有人格，以及它与其他人格的上下级关系。
2. 写入 JSON 文件。
3. 调用 `reload`，或在 config 工具完成相关变更后让运行态 reload。
4. 根据 reload 返回的 `repairSummary` / `repairItems` 修复错误。

应用启动时会自动加载一次工作区；手动 `reload` 会清理缓存并重新加载 MCP、Skill 和私有人格。

## 私有人格 JSON

每个文件只写一个人格对象：

```json
{
  "id": "market-watcher",
  "name": "市场观察员",
  "systemPrompt": "你负责持续关注财经新闻、市场动向与重点信号，输出简洁结论。",
  "summary": "持续关注财经新闻与市场动向，输出简洁结论。",
  "apiConfigIds": ["openai::gpt-4.1-mini"],
  "childAgentIds": [],
  "residentSkillNames": [],
  "optionalSkillNames": [],
  "permissionControl": {
    "enabled": true,
    "mode": "blacklist",
    "builtinToolNames": ["task"],
    "skillNames": [],
    "mcpToolNames": []
  }
}
```

必填字段：

- `id`
- `name`
- `systemPrompt`

可选字段：

- `tools`
- `avatarPath`
- `summary`：一句话人格简介，用于人格列表展示与委托清单里的说明位。
- `apiConfigIds`：该人格的驱动模型；首个值作为主模型。
- `childAgentIds`：该人格的直接下级人格 id，构成组织树的父子边，语义为「可直接委托」。
- `residentSkillNames`：常驻 skill，全文注入该人格的系统提示词。
- `optionalSkillNames`：可选 skill，只注入名字，模型需要时自行读取。
- `permissionControl`：该人格的权限约束。

兼容说明：

- `prompt` 仍兼容旧格式，但新写法优先使用 `systemPrompt`。
- 一个文件只写一个人格对象，不要包数组。

约束说明：

- `childAgentIds` 里引用的人格必须真实存在。
- `apiConfigIds` 若存在，首个值会作为主模型。
- `permissionControl.skillNames` 只应引用自定义工作区 skill；内置预设 skill 不需要写入这里。
- `permissionControl.mcpToolNames` 只应引用已启用 MCP 暴露出的工具名。
- 如果刚安装或更新 MCP/Skill，先 reload，再决定权限字段写什么。

不要手写这些运行时字段：

- `createdAt`
- `updatedAt`
- `source`
- `scope`
- `privateMemoryEnabled`
- `isBuiltInUser`
- `isBuiltInSystem`

## MCP 与 Skill 权限

配置私有人格权限前，先确认工具目录：

```text
config "mcp ls"
config "mcp tools <name-or-id>"
config "skill ls"
```

`skill ls` 面向自定义 skill，不返回内置预设 skill。
浏览器自动化能力应通过 Playwright MCP 提供，不要用一次性 shell/CLI 启动浏览器；后续工具调用无法稳定复用同一个浏览器实例、页面上下文和会话状态。

如果要新增浏览器自动化能力：

```text
config "mcp add playwright -- npx -y @playwright/mcp@latest"
config "mcp enable playwright"
config "mcp tools playwright"
```

## 约束

- 不能使用系统保留 ID。
- 不能与主配置中的人格同 ID。
- 私有人格默认不使用私有记忆。
- 不要擅自发明字段；不确定时保持最小 JSON。
- 删除 MCP 或 Skill 必须先得到用户明确同意，并使用对应 `--confirmed` 命令。
