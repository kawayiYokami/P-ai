# Data Schema v2

## Config (`config.toml`)

```toml
hotkey = "Alt+·"
selectedApiConfigId = "api-config-id-for-config-ui"
chatApiConfigId = "api-config-id-for-chat"
sttApiConfigId = "optional-api-config-id-for-audio-to-text"
visionApiConfigId = "optional-api-config-id-for-image-to-text"

[[apiConfigs]]
id = "default-openai"
name = "Default OpenAI"
requestFormat = "openai" # openai | gemini | deepseek/kimi
enableText = true
enableImage = true
enableAudio = true
enableTools = false
baseUrl = "https://api.openai.com/v1"
apiKey = ""
model = "gpt-4o-mini"
```

## App Data (`app_data.json`)

- `version`: schema version
- `agents`: agent profiles
- `selectedAgentId`: active agent
- `userAlias`: user display name for system prompt
- `conversations`: active conversations
- `archivedConversations`: archived conversations

## Notes

1. `selectedApiConfigId` is only for the config page "currently editing API config".
2. `chatApiConfigId` is the runtime default for chat requests.
3. `sttApiConfigId` must point to an API config with `enableAudio=true`.
4. `visionApiConfigId` must point to an API config with `enableImage=true`.
