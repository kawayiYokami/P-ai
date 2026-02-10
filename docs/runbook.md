# Easy Call AI Runbook

## Quick Smoke

Run:

```powershell
pnpm smoke
```

It validates:

1. TypeScript type check
2. Rust compile check
3. Rust test compile (`--no-run`)

## Debug API Mode

Place `.debug/api-key.json` in project root and fill provider info.

When enabled, chat routing will use debug config for low-cost validation.

## Core Runtime Paths

1. Chat API: `chatApiConfigId`
2. Audio fallback (STT): `sttApiConfigId` (`openai_tts` only)
3. Image fallback (Vision): `visionApiConfigId`

## Multimodal Rules

1. If chat API supports image/audio, payload goes directly.
2. If chat API does not support audio, app calls STT (`/audio/transcriptions`) and merges text.
3. If chat API does not support image, app calls vision model, caches converted text by `hash + vision_api_id`, then merges text.

## Troubleshooting

1. `Current chat API does not support audio and no 音转文AI is configured.`
   - Configure `sttApiConfigId` with `openai_tts`.
2. `Current chat API does not support image and no 图转文AI is configured.`
   - Configure `visionApiConfigId`.
3. `Request format ... not implemented ...`
   - For chat/vision use openai-style format (`openai` / `deepseek/kimi`).
