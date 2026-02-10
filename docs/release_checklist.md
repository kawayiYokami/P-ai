# Release Checklist

## Preflight

1. `pnpm install`
2. `pnpm smoke`
3. Confirm git worktree is clean

## Functional Verification

1. API binding
   - Configure chat/stt/vision API independently
   - Restart app and verify settings persist
2. Chat text
   - Send text-only message
   - Verify streaming output
3. Image fallback
   - Use chat API with image disabled + vision configured
   - Paste image and send
   - Verify reply succeeds
   - Send same image again and confirm cache stats increase then remain stable
4. Audio fallback
   - Use chat API with audio disabled + stt configured
   - Hold record button (or hold configured key), release to stop
   - Send and verify transcription merges into user message
5. Cache panel
   - Open config -> 对话 tab
   - Verify cache stats refresh and clear works

## Window & UX

1. Hotkey toggles chat window visibility
2. Tray menu opens config/chat/archives
3. Chat window can drag / close / always-on-top works

## Regression Guard

1. Rust tests compile (`cargo test --no-run`)
2. No TypeScript errors (`pnpm -s exec tsc --noEmit`)
