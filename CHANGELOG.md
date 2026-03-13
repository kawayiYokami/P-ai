# Changelog

## v0.3.2 - 2026-03-13

- fix(multimodal): stabilize latest media prompt semantics
  - remove previous-message media backfill for `latest_images/latest_audios`
  - switch overrides to tri-state semantics (`None` / `Some([])` / `Some([...])`)
  - keep media parsing logic unified with shared resolver
- fix(multimodal): improve filtered-media transparency
  - append explicit text notice when image input is filtered
  - append explicit notice on model-side image rejection fallback
- fix(audio): map `input_audio.format` to OpenAI-compatible short format
  - use `wav/mp3/...` instead of full MIME in deepseek/openai-compatible payloads
  - keep debug request log format consistent with real request payload
