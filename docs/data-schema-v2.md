# 数据模型

> 更新于 2026-03-13

## Config (config.toml)

### AppConfig

| 字段 | 类型 |
| --- | --- |
| hotkey | String |
| ui_language | String |
| ui_font | String |
| record_hotkey | String |
| record_background_wake_enabled | bool |
| min_record_seconds | u32 |
| max_record_seconds | u32 |
| tool_max_iterations | u32 |
| selected_api_config_id | String |
| assistant_department_api_config_id | String |
| vision_api_config_id | Option\<String> |
| stt_api_config_id | Option\<String> |
| stt_auto_send | bool |
| terminal_shell_kind | String |
| shell_workspaces | Vec\<ShellWorkspaceConfig> |
| mcp_servers | Vec\<McpServerConfig> |
| departments | Vec\<DepartmentConfig> |
| api_configs | Vec\<ApiConfig> |

### ApiConfig

| 字段 | 类型 |
| --- | --- |
| id | String |
| name | String |
| request_format | RequestFormat |
| enable_text | bool |
| enable_image | bool |
| enable_audio | bool |
| enable_tools | bool |
| tools | Vec\<ApiToolConfig> |
| base_url | String |
| api_key | String |
| model | String |
| temperature | f64 |
| context_window_tokens | u32 |
| max_output_tokens | u32 |
| failure_retry_count | u32 |

### RequestFormat

openai, openai_responses, openai_tts, openai_stt, openai_embedding, openai_rerank, gemini, gemini_embedding, deepseek/kimi, anthropic

### ShellWorkspaceConfig

| 字段 | 类型 |
| --- | --- |
| name | String |
| path | String |
| built_in | bool |

### McpServerConfig

| 字段 | 类型 |
| --- | --- |
| id | String |
| name | String |
| enabled | bool |
| definition_json | String |
| tool_policies | Vec\<McpToolPolicy> |
| cached_tools | Vec\<McpCachedTool> |
| last_status | String |

### DepartmentConfig

| 字段 | 类型 |
| --- | --- |
| id | String |
| name | String |
| summary | String |
| guide | String |
| api_config_ids | Vec\<String> |
| api_config_id | String |
| agent_ids | Vec\<String> |
| created_at | String |
| updated_at | String |
| order_index | i64 |
| is_built_in_assistant | bool |
| source | String |
| scope | String |

## App Data (app_data.json)

### AppData

| 字段 | 类型 |
| --- | --- |
| version | u32 |
| agents | Vec\<AgentProfile> |
| assistant_department_agent_id | String |
| user_alias | String |
| response_style_id | String |
| conversations | Vec\<Conversation> |
| archived_conversations | Vec\<ConversationArchive> |
| image_text_cache | Vec\<ImageTextCacheEntry> |

### AgentProfile

| 字段 | 类型 |
| --- | --- |
| id | String |
| name | String |
| system_prompt | String |
| tools | Vec\<ApiToolConfig> |
| created_at | String |
| updated_at | String |
| avatar_path | Option\<String> |
| avatar_updated_at | Option\<String> |
| is_built_in_user | bool |
| is_built_in_system | bool |
| private_memory_enabled | bool |
| source | String |
| scope | String |

### Conversation

| 字段 | 类型 |
| --- | --- |
| id | String |
| title | String |
| api_config_id | String |
| agent_id | String |
| conversation_kind | String ("chat" / "delegate") |
| root_conversation_id | Option\<String> |
| delegate_id | Option\<String> |
| created_at | String |
| updated_at | String |
| last_user_at | Option\<String> |
| last_assistant_at | Option\<String> |
| last_context_usage_ratio | f64 |
| last_effective_prompt_tokens | u64 |
| status | String |
| summary | String |
| archived_at | Option\<String> |
| messages | Vec\<ChatMessage> |
| memory_recall_table | Vec\<String> |

### ChatMessage

| 字段 | 类型 |
| --- | --- |
| id | String |
| role | String |
| created_at | String |
| speaker_agent_id | Option\<String> |
| parts | Vec\<MessagePart> |
| extra_text_blocks | Vec\<String> |
| provider_meta | Option\<Value> |
| tool_call | Option\<Vec\<Value>> |
| mcp_call | Option\<Vec\<Value>> |

### MessagePart (enum)

- Text { text }
- Image { mime, bytes_base64, name, compressed }
- Audio { mime, bytes_base64, name, compressed }

### ConversationArchive

| 字段 | 类型 |
| --- | --- |
| archive_id | String |
| archived_at | String |
| reason | String |
| summary | String |
| source_conversation | Conversation |

### ImageTextCacheEntry

| 字段 | 类型 |
| --- | --- |
| hash | String |
| vision_api_id | String |
| text | String |
| updated_at | String |

## 序列化约定

所有结构体使用 `#[serde(rename_all = "camelCase")]`，Rust 下划线命名在 JSON/TOML 中变为驼峰。
