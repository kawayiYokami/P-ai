# Changelog

## Unreleased

- refactor(chat-pipeline): 将多模态兼容处理前移到出队阶段，按“逐条消息”定型
  - 在批次写入历史前按会话模型能力处理每条 user 消息
  - 模型支持图片时保留图片，不做额外注入
  - 模型不支持图片时移除图片，并按规则注入文本：
    - 命中已有图转文缓存：注入对应图转文内容
    - 无图转文缓存：注入提示“这里有一张图片，但当前模型不支持图片输入，所以已忽略。”
- fix(chat-runtime): 收敛运行时重复改写，避免二次注入
  - 保留 router 防御性清理，但移除运行时二次文本注入逻辑
  - 统一由“出队定型结果”驱动后续 prompt 构建与请求序列化

## v0.4.0 - 2026-03-16

- feat(chat-ui): rework chat window rendering pipeline for stable streaming UX
  - unify streaming/history bubble behavior and keep layout stable during stream start/finish
  - fix assistant draft lifecycle to avoid duplicate bubbles and message overwrite flashes
  - keep avatars, spacing, and action areas consistent while streaming state transitions
- feat(chat-control): improve stop/regenerate interaction model
  - allow immediate stop right after send (queued stage), and keep backend interruption in sync
  - make send button switch to stop during active round for faster interruption
  - only allow regenerate when the latest message is assistant
- refactor(chat-runtime): remove coarse global refresh coupling
  - drop window-wide `easy-call:refresh` listeners/emits for chat flow
  - prevent stale delayed events from reviving cancelled rounds

## v0.3.8 - 2026-03-16

- feat(remote-im): complete remote IM backend integration and main pipeline wiring
  - add inbound enqueue flow, queue scheduling, and outbound adapter routing
  - improve OneBot/NapCat channel lifecycle handling and runtime reconciliation
- feat(remote-contacts): add per-contact controls and activation strategy
  - split permissions into `allow_receive` and `allow_send` (default off)
  - add activation mode (`never` / `always` / `keyword`) with cooldown support
  - add activation decision logs and contact-level activation config command
- refactor(remote-im): improve maintainability and diagnostics
  - refactor large validation/parsing paths into helpers
  - add structured logs for Dingtalk/OneBot send paths and token flow
  - reduce unsafe/high-volume event logging payloads
- fix(ui/types): improve config and runtime consistency
  - fix conversation id/type mismatches and several silent catch blocks
  - fix RemoteIm tab behavior, list layout updates, and Tailwind class issue
  - add dynamic app version display in About tab (no hardcoded version text)
- chore(branding): rename project-facing brand to `P-ai` / `π师傅`
  - update visible app titles, docs links, and release/update repository URLs
  - keep legacy data/storage identifiers for compatibility where required

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
