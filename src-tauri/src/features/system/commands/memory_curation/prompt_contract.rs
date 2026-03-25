fn build_memory_generation_instruction(agent: &AgentProfile, user_alias: &str) -> String {
    format!(
        "你要做记忆整理。输出严格 JSON，不要 markdown，不要代码块。\n\
         ## 强制要求（MUST）\n\
         A) 输出必须是合法 JSON，且仅包含 usefulMemoryIds/newMemories/mergeGroups 三个字段。\n\
         B) 不得输出 markdown、代码块、解释性前后缀。\n\
         C) 你是 {assistant_name}，用户称谓是 {user_name}。\n\
         \n\
         ## 记忆要求（仅约束 usefulMemoryIds/newMemories/mergeGroups）\n\
         1) newMemories 最多 7 条；非必要不生成；memoryType 只能是 knowledge/skill/emotion/event。\n\
         2) usefulMemoryIds 只能从“本次会话使用过的记忆”中选择。\n\
         3) mergeGroups 不是必须，默认输出 []；仅当语义等价或高度重复且合并后不丢信息时才允许填写。\n\
         4) mergeGroups.sourceIds 只能从“本次会话使用过的记忆”中选择，且每组至少 2 个；不确定时必须保持 []。\n\
         5) newMemories 中的 judgment/reasoning/tags 必须使用当前用户本轮使用的语言，禁止夹杂其他语言。\n\
         6) reasoning 定义：给出“支撑该 judgment 的论据/证据”；若没有可靠理由可以留空。\n\
         7) reasoning 必须写“支撑该 judgment 的论据/证据”，不得写流程话术。\n\
         8) reasoning 只允许描述对话中可追溯的依据，不得写“为了归档/为了生成记忆”。\n\
         9) reasoning 应尽量简洁具体；若没有可靠理由或证据不足，可留空字符串。\n\
         10) judgment 必须能被 reasoning 支撑；若无法支撑，宁可不生成该条记忆。\n\
         11) tags/judgment/reasoning 必须使用当前用户本轮语言（专有名词除外）。\n\
         12) tags 中的每一项都必须是独立、紧凑、稳定、可检索的词元；不要写整句，不要写短语拼接，不要写“用户喜欢极简风格”这类带关系的长表达。\n\
         13) tags 只写检索锚点本身，例如 人名、项目名、偏好词、主题词、技能词、物品名；同一项里不要混入多个语义。\n\
         14) 不要记录高风险敏感信息（密码、密钥、身份证、银行卡等）。",
        assistant_name = agent.name,
        user_name = user_alias
    )
}

fn build_memory_generation_latest_user_text(
    instruction: &str,
    all_compaction_context: &str,
    used_memories: &str,
    example_output: &str,
) -> String {
    format!(
        "<记忆整理提示>\n{}\n</记忆整理提示>\n\n<本会话全部上下文整理信息>\n{}\n</本会话全部上下文整理信息>\n\n<本次会话使用过的记忆>\n{}\n</本次会话使用过的记忆>\n\n<示例输出>\n{}\n</示例输出>",
        instruction, all_compaction_context, used_memories, example_output
    )
}
