---
name: private-organization-guide
description: 当需要在助理私域中维护私有人格或私有部门时，必须立刻阅读我。我会告诉你如何用 JSON 文件声明它们，并通过 reload 让配置生效。
---

# 私有组织指南

你可以在当前工作区中维护“助理私域”的私有人格和私有部门。
- `<workspace>` 只是占位符，不是固定目录名；它表示你当前 shell 的启动工作空间。
- 你只能在这个工作空间内工作；不要假设或访问工作空间外路径。

## 目录
- 私有人格：`<workspace>/private-organization/personas/`
- 私有部门：`<workspace>/private-organization/departments/`

## 基本规则
- 只在上述私有目录中新增或修改 JSON 文件。
- 私有目录里的内容会作为运行时附加加载，不会写回应用主配置。
- 写完后调用 `reload`，让系统扫描并加载。
- 若 `reload` 返回报错，根据报错修改 JSON 再重试。

## 私有人格 JSON
每个文件只写一个人格，例如：

```json
{
  "id": "market-watcher",
  "name": "市场观察员",
  "prompt": "你负责持续关注财经新闻、市场动向与重点信号，输出简洁结论。"
}
```

可选字段：
- `tools`
- `avatarPath`

必填字段：
- `id`
- `name`
- `prompt`

## 私有部门 JSON
每个文件只写一个部门，例如：

```json
{
  "id": "market-intel",
  "name": "市场情报部",
  "summary": "负责追踪财经新闻、市场情绪和短期重点事件。",
  "guide": "接到任务后，先提炼关键事实，再给出可执行摘要。",
  "agentIds": ["market-watcher"]
}
```

必填字段：
- `id`
- `name`
- `agentIds`

可选字段：
- `summary`
- `guide`
- `apiConfigId`

## 约束
- 不能使用系统保留 ID。
- 不能与主配置中的人格或部门同 ID。
- 私有部门引用的人格必须真实存在。
- 若不写 `apiConfigId`，默认继承主助理部门当前模型。
- 若写了 `apiConfigId`，它必须指向可用于文本对话的模型配置。
- 私有人格不提供私有记忆开关，工具人默认不使用私有记忆。

## 推荐工作流
1. 先设计要新增的私有人格。
2. 再设计私有部门，并绑定对应人格。
3. 写入 JSON 文件。
4. 调用 `reload`。
5. 根据 `reload` 返回结果修正错误。
