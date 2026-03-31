fn build_memory_generation_instruction(agent: &AgentProfile, user_alias: &str) -> String {
    let must_rules = format!(
        "A) 输出必须是合法 JSON，且仅包含 summary/usefulMemoryIds/newMemories/mergeGroups/profileMemories 五个字段。\n\
         B) 不得输出 markdown、代码块、解释性前后缀。\n\
         C) 你是 {assistant_name}，用户称谓是 {user_name}。",
        assistant_name = agent.name,
        user_name = user_alias
    );
    let memory_rules = "1) newMemories 最多 7 条；非必要不生成；memoryType 只能是 knowledge/skill/emotion/event。\n\
         2) usefulMemoryIds 只能从“本次会话使用过的记忆”中选择；若上下文里看到的是短ID（如 12），就直接输出该短ID。\n\
         3) mergeGroups 不是必须，默认输出 []；仅当语义等价或高度重复且合并后不丢信息时才允许填写。\n\
         4) mergeGroups.sourceIds 只能从“本次会话使用过的记忆”中选择，且每组至少 2 个；不确定时必须保持 []；若上下文里看到的是短ID，就直接输出短ID。\n\
         5) profileMemories 不是必须，默认输出 []；每一项只能二选一：要么填写 memoryId 引用已有记忆，要么填写 memory 直接创建新记忆；若引用已有记忆，优先输出上下文里展示的短ID。\n\
         6) 只有以下四类内容才允许进入 profileMemories：事实属性、职业/技能、关系图谱、活跃项目。\n\
         7) profileMemories 严禁放入 emotion，严禁放短期任务、临时情绪、一次性即时上下文。\n\
         8) 若已有记忆已足够表达该用户画像，应优先填写 memoryId，不要重复新建记忆。\n\
         9) 若必须新建 profileMemories.memory，其 memoryType 只能是 knowledge/skill/event。\n\
         10) newMemories 与 profileMemories.memory 中的 judgment/reasoning/tags 必须使用当前用户本轮使用的语言（专有名词除外），禁止夹杂其他语言。\n\
         11) reasoning 定义：给出“支撑该 judgment 的论据/证据”；若没有可靠理由可以留空。\n\
         12) reasoning 必须写“支撑该 judgment 的论据/证据”，不得写流程话术。\n\
         13) reasoning 只允许描述对话中可追溯的依据，不得写“为了归档/为了生成记忆”。\n\
         14) reasoning 应尽量简洁具体；若没有可靠理由或证据不足，可留空字符串。\n\
         15) judgment 必须能被 reasoning 支撑；若无法支撑，宁可不生成该条记忆。\n\
         16) tags 中的每一项都必须是独立、紧凑、稳定、可检索的词元；不要写整句，不要写短语拼接，不要写“用户喜欢极简风格”这类带关系的长表达。\n\
         17) tags 只写检索锚点本身，例如 人名、项目名、偏好词、主题词、技能词、物品名；同一项里不要混入多个语义。\n\
         18) 不要记录高风险敏感信息（密码、密钥、身份证、银行卡等）。";
    format!(
        "你要做记忆整理。输出严格 JSON，不要 markdown，不要代码块。\n{}\n{}",
        prompt_xml_block("memory curation must", must_rules),
        prompt_xml_block("memory curation rules", memory_rules)
    )
}

fn memory_curation_example_output_block() -> &'static str {
    r###"{
  "summary": "string",
  "usefulMemoryIds": ["12"],
  "newMemories": [
    {
      "memoryType": "knowledge|skill|emotion|event",
      "judgment": "string",
      "reasoning": "string",
      "tags": ["string"]
    }
  ],
  "mergeGroups": [
    {
      "sourceIds": ["12", "19"],
      "target": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    }
  ],
  "profileMemories": [
    {
      "memoryId": "12"
    },
    {
      "memory": {
        "memoryType": "knowledge|skill|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    }
  ]
}"###
}
