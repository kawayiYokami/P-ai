fn memory_generation_rules_body() -> &'static str {
    let raw = include_str!("../../../../../resources/preset-skills/memory-generation/SKILL.md");
    let trimmed = raw.trim_start();
    trimmed
        .strip_prefix("---")
        .and_then(|rest| rest.trim_start_matches(['\r', '\n']).split_once("\n---"))
        .map(|(_, body)| body.trim_start_matches(['\r', '\n']).trim())
        .unwrap_or(raw.trim())
}

fn memory_curation_example_output_block() -> &'static str {
    r###"{
  "title": "string",
  "summary": "string",
  "usefulMemoryIds": ["12"],
  "memoryActions": [
    {
      "action": "create",
      "memory": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    },
    {
      "action": "update",
      "sourceMemoryIds": ["12"],
      "memory": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    },
    {
      "action": "merge",
      "sourceMemoryIds": ["12", "19"],
      "memory": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    }
  ]
}"###
}

fn archive_reflection_example_output_block() -> &'static str {
    r###"{
  "usefulMemoryIds": ["12"],
  "memoryActions": [
    {
      "action": "create",
      "memory": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    },
    {
      "action": "update",
      "sourceMemoryIds": ["12"],
      "memory": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    },
    {
      "action": "merge",
      "sourceMemoryIds": ["12", "19"],
      "memory": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    }
  ]
}"###
}
