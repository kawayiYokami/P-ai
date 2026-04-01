// ---- 共享类型 ----
include!("provider_and_stream/types.rs");

// ---- 截图缓存基础设施 ----
include!("provider_and_stream/screenshot_cache.rs");

// ---- prompt 构建 ----
include!("provider_and_stream/prompt_build.rs");

// ---- 流式收集 ----
include!("provider_and_stream/stream_collect.rs");

// ---- 工具组装 ----
include!("provider_and_stream/tool_assembly.rs");

// ---- 统一工具循环 ----
include!("provider_and_stream/tool_loop.rs");

// ---- OpenAI provider ----
include!("provider_and_stream/openai_style.rs");

// ---- Gemini provider ----
include!("provider_and_stream/gemini.rs");

// ---- Anthropic provider ----
include!("provider_and_stream/anthropic.rs");

// ---- 路由分发 + 日志 ----
include!("provider_and_stream/router.rs");

// ---- Vision API ----
include!("provider_and_stream/vision.rs");
