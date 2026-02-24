fn build_archive_instruction(agent: &AgentProfile, user_alias: &str) -> String {
    format!(
        "你要做归档总结。输出严格 JSON，不要 markdown，不要代码块。\n\
         ## 强制要求（MUST）\n\
         A) 输出必须是合法 JSON，且仅包含 summary/usefulMemoryIds/newMemories/mergeGroups 四个字段。\n\
         B) 不得输出 markdown、代码块、解释性前后缀。\n\
         C) 你是 {assistant_name}，用户称谓是 {user_name}。\n\
         \n\
         ## 摘要要求（仅约束 summary）\n\
         1) summary 必填，必须按时间顺序写，语言自然、具体，不要模板化空话。\n\
         2) summary 必须使用以下 8 个固定段落标题，且顺序不可变：\n\
            - Current Progress\n\
            - Current State\n\
            - User Decisions\n\
            - Open Issue (Root Cause)\n\
            - What Changed\n\
            - What Remains\n\
            - Constraints / Preferences\n\
            - Quick References\n\
         3) 每个段落都必须有内容；没有信息时写“无”。\n\
         4) Open Issue (Root Cause) 必须写出问题根因；若有证据，附文件路径或可追溯线索。\n\
         5) What Remains 必须写可执行下一步（可用 1/2/3 编号）。\n\
         6) Quick References 只写关键路径或关键对象，不写长解释。\n\
         7) 如有多个论题，必须逐个说明，禁止合并成笼统描述。\n\
         8) 必须保留可追溯锚点：关键对象、关键时间点、关键数字或约束条件；不确定就写“待确认”，禁止猜测。\n\
         \n\
         ## 记忆要求（仅约束 usefulMemoryIds/newMemories/mergeGroups）\n\
         7) newMemories 最多 7 条；非必要不生成；memoryType 只能是 knowledge/skill/emotion/event（禁止 task）。\n\
         8) usefulMemoryIds 只能从“本次会话使用过的记忆”中选择。\n\
         9) mergeGroups 不是必须，默认输出 []；仅当语义等价或高度重复且合并后不丢信息时才允许填写。\n\
         10) mergeGroups.sourceIds 只能从“本次会话使用过的记忆”中选择，且每组至少 2 个；不确定时必须保持 []。\n\
         11) newMemories 中的 judgment/reasoning/tags 必须使用当前用户本轮使用的语言，禁止夹杂其他语言。\n\
         12) reasoning 定义：给出“支撑该 judgment 的论据/证据”；若没有可靠理由可以留空。\n\
         13) reasoning 必须写“支撑该 judgment 的论据/证据”，不得写流程话术。\n\
         14) reasoning 只允许描述对话中可追溯的依据，不得写“为了归档/为了生成记忆”。\n\
         15) reasoning 应尽量简洁具体；若没有可靠理由或证据不足，可留空字符串。\n\
         16) judgment 必须能被 reasoning 支撑；若无法支撑，宁可不生成该条记忆。\n\
         17) tags/judgment/reasoning 必须使用当前用户本轮语言（专有名词除外）。\n\
         18) 不要记录高风险敏感信息（密码、密钥、身份证、银行卡等）。",
        assistant_name = agent.name,
        user_name = user_alias
    )
}

fn build_archive_latest_user_text(
    instruction: &str,
    used_memories: &str,
    example_output: &str,
) -> String {
    format!(
        "<压缩上下文的提示>\n{}\n</压缩上下文的提示>\n\n<本次会话使用过的记忆>\n{}\n</本次会话使用过的记忆>\n\n<示例输出>\n{}\n</示例输出>",
        instruction, used_memories, example_output
    )
}
