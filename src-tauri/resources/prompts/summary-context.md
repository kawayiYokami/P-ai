<system_remind>
当前不是用户的发言，是上下文到达上限后的系统提醒。
你应该立刻停下手上的工作，不要再调用任何工具，按后续的指南生成json，完成中场总结反思。
你必须以后续可以无缝继续当前工作为目标。
</system_remind>

<memory_curation_context>
你是 {{assistant_name}}，用户称谓是 {{user_name}}。
描述用户及其画像时，必须使用这个用户称谓；不要用“用户”泛称当前人类用户。

你还需要在同一次中场总结反思里判断记忆：
- usefulMemoryIds 表示本次上下文仍然有用的旧记忆。
- memoryActions 表示需要新增、修正或合并的记忆。

<memory generation rules>
{{memory_generation_rules}}
</memory generation rules>
</memory_curation_context>

<json_contract>
输出这个 JSON，不要 markdown 代码块，不要解释性前后缀：

{
  "title": "...",
  "summary": "...",
  "usefulMemoryIds": ["12"],
  "memoryActions": [...]
}

title：一句中文短语，不超过 12 个字，写具体主题，不带标点。从 summary 里挑最核心的那件事。

summary：一个字符串，分节写清这些内容：目标（当前要做什么）、已完成（做完的）、进行中（正在推进但没完成的）、阻塞（卡住的和卡在哪）、禁令（不该做、容易走错的）、决策（已定下要沿用的）、下一步（最自然的动作）、关键重要情报（不知道就会接错活的约束、偏好、环境事实）、相关文件（续跑会用到的，格式 `相对路径:行号: 简要说明`）。每节用一个二级标题开头，标题就是节名。摘要应是结构化的。

usefulMemoryIds：本次上下文仍然有用的旧记忆 id，从注入的记忆里挑。

memoryActions：需要新增、修正或合并的记忆，每项是：

{"action": "create|update|merge", "sourceMemoryIds": ["12"], "memory": {"memoryType": "knowledge|skill|emotion|event", "judgment": "...", "reasoning": "...", "tags": ["..."]}}

create 不写 sourceMemoryIds，update 与 merge 必写。

字符串内容里要引用任何东西，一律用「」，嵌套用『』，不用英文双引号。
</json_contract>
