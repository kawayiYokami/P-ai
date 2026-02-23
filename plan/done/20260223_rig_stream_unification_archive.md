# Rig 流式统一改造归档（以实现为准）

## 1. 结论

本次改造已按“统一链路”目标落地：聊天主路径为统一入口分流，不再保留并行独立主通路。`openai_responses` 已作为独立协议接入并生效。

---

## 2. 最终实现摘要

1. 新增并打通 `openai_responses` 协议（前后端一致可选、可保存、可调用）。
2. 运行时协议分流修正：
   - `openai` 走 Chat Completions 语义链路
   - `openai_responses` 走 Responses 语义链路
3. Responses 链路已支持流式增量回传（文本 + reasoning delta），不再是最终整包一次返回。
4. OpenAI 与 OpenAI Responses 的 rig 流式调用已收敛为统一内部实现，避免重复逻辑扩散。
5. 构建与测试通过（`cargo check` / `cargo test` / `npm build`）。

---

## 3. 关键落地文件

- `src-tauri/src/features/core/domain.rs`
- `src-tauri/src/features/system/commands/chat_and_runtime.rs`
- `src-tauri/src/features/system/commands/inference_gateway.rs`
- `src-tauri/src/features/chat/model_runtime/provider_and_stream.rs`
- `src-tauri/src/features/chat/model_runtime/tools_and_builtin.rs`
- `src/types/app.ts`
- `src/features/config/composables/use-config-core.ts`
- `src/features/config/views/config-tabs/ApiTab.vue`
- `src/features/config/views/config-tabs/MemoryTab.vue`
- `src/App.vue`

---

## 4. 与原计划的对齐说明

1. “统一入口 + 协议分流”已达成。  
2. “移除错误路由（协议显示 A，实际走 B）”已达成。  
3. “保证流式体验”已达成（可见增量，不是瞬时整包）。  
4. 以工程稳态为优先，采用了“先修正确性、再逐步深度收敛”的执行路径。

---

## 5. 当前状态标记

- 状态：`Archived / Done`
- 后续若继续精简接口数量，作为新计划单独立项，不回写本归档。

