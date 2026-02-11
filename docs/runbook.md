# Easy Call AI 运行手册

## 一键冒烟检查

执行：

```powershell
pnpm smoke
```

会依次检查：

1. TypeScript 类型检查
2. Rust 编译检查
3. Rust 测试编译（`--no-run`，不实际执行测试）

## Debug API 模式

在项目根目录放置 `.debug/api-key.json`，填写你的测试供应商信息。

开启后，对话路由会优先使用 debug 配置，便于低成本验证。

## 核心运行配置

1. 对话模型：`chatApiConfigId`
2. 音转文回退：`sttApiConfigId`（仅支持 `openai_tts`）
3. 图转文回退：`visionApiConfigId`

## 多模态处理规则

1. 如果对话 API 支持图片/音频，直接发送原始多模态内容。
2. 如果对话 API 不支持音频，自动调用 STT（`/audio/transcriptions`）转文字，再并入本次消息文本。
3. 如果对话 API 不支持图片，自动调用图转文模型；结果按 `hash + vision_api_id` 缓存，并入本次消息文本。

## 常见问题排查

1. `Current chat API does not support audio and no 音转文AI is configured.`
   - 配置 `sttApiConfigId`，并确保其请求格式为 `openai_tts`。
2. `Current chat API does not support image and no 图转文AI is configured.`
   - 配置 `visionApiConfigId`。
3. `Request format ... not implemented ...`
   - 对话/图转文请使用 openai 风格请求格式（`openai` / `deepseek/kimi`）。
