# 聊天消息管线

> 更新于 2026-03-13

## 需求

- 所有消息先入队，再批量写入正式历史，再决定是否激活助理。
- 同一时刻只有一个主助理轮次。
- 支持用户 / 任务 / 委托 / 系统四种消息来源。
- 前端通过流式 Channel 接收增量事件。
- 支持文本 / 图片 / 音频 / 文件附件。
- 不支持该模态时自动回退（STT / Vision）。

## 执行链

```
前端 sendChat()
  → invokeTauri("send_chat_message")
    → send_chat_message
      → enqueue_chat_event
      → trigger_chat_queue_processing
        → process_chat_queue
          → dequeue_batch
          → 按 conversation_id 分组
          → process_conversation_batch
            → 写入正式历史
            → 通知前端 history_flushed
            → 若 activate_assistant=true
                → activate_main_assistant (trigger_only=true)
                  → send_chat_message_inner
                    → 自动归档 / 记忆召回 / 提示词组装
                    → call_model_openai_style → 按 RequestFormat 分发
                    → onDelta 流式回传
                    → 写入 assistant 消息
                → 通知前端 round_completed
```

## 前端事件流

```
channel.onmessage:
  history_flushed  → 合并消息到 allMessages
  delta            → 增量渲染
  round_completed  → 重新加载最终快照
  round_failed     → 错误处理
```

## 状态机

| 状态 | 可出队 |
| --- | --- |
| Idle | 是 |
| AssistantStreaming | 否 |
| OrganizingContext | 否 |

## 协议分流

| RequestFormat | 协议 |
| --- | --- |
| openai | OpenAI Chat |
| openai_responses | OpenAI Responses |
| deepseek-kimi | DeepSeek / Kimi |
| gemini | Gemini |
| anthropic | Anthropic |

## 附件管线

```
前端 queue_local_file_attachment → 构建 AttachmentMetaInput → 合并到 payload.attachments
后端 normalize_payload_attachments → merge_provider_meta_with_attachments → 写入 provider_meta
前端 extractMessageAttachmentFiles → 从 providerMeta.attachments 提取 → 渲染气泡
```

## 多模态回退

```
图片/音频 → API 支持 → 直接发送
         → API 不支持音频 → STT 转文字 → 并入消息
         → API 不支持图片 → Vision 转文字 → 缓存结果 → 并入消息
```
