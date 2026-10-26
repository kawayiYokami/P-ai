# 文档索引

这份索引只回答一件事：打开 `docs/` 里某篇时，该不该把它当现状。

发布说明在 [changelog](changelog/latest.md)，由发版流程生成，不要手改。

对外介绍看仓库根 [README.md](../README.md)。编码约定看 [AGENTS.md](../AGENTS.md)。

## 现状说明

仍按当前实现理解，过期处应改文档而不是改代码去迁就它。

| 文档 | 管什么 |
|---|---|
| [data-schema-v2.md](data-schema-v2.md) | 本地存储布局与现役 SQLite / 压缩块 |
| [migration.md](migration.md) | 旧格式如何迁到最新，生产路径只认最新 |
| [queue-and-guided-semantics.md](queue-and-guided-semantics.md) | 队列与引导的产品语义 |
| [chat_transport_and_state_machine.md](chat_transport_and_state_machine.md) | 聊天通信、调度与前台状态机 |
| [chat-message-flow-assertions.md](chat-message-flow-assertions.md) | 消息真相层格式与流转断言 |
| [chat-message-group-to-ui.md](chat-message-group-to-ui.md) | 聚合消息如何映射到聊天 UI |
| [会话服务设计图.md](会话服务设计图.md) | 会话服务职责边界 |
| [remote_im_state_machine.md](remote_im_state_machine.md) | 远程联系人私聊 / 群聊状态机 |
| [remote_im_context_management.md](remote_im_context_management.md) | 远程联系人各角色实际拿到的上下文 |
| [remote-im-group-energy-design.md](remote-im-group-energy-design.md) | 远程群聊能量机制（已落地） |
| [web-sidebar-bridge.md](web-sidebar-bridge.md) | Web / VS Code 侧边栏与桌面共用聊天界面 |
| [markdown-style-overrides.md](markdown-style-overrides.md) | 自研 Markdown 渲染与样式 |

## 多语言 README

| 文档 | 说明 |
|---|---|
| [readme/README.zh-CN.md](readme/README.zh-CN.md) | 简体中文 |
| [readme/README.zh-TW.md](readme/README.zh-TW.md) | 繁體中文 |
| [readme/README.en-US.md](readme/README.en-US.md) | English |
| [readme/README.ja-JP.md](readme/README.ja-JP.md) | 日本語 |

根 README 是英文入口，四语内容应对齐。

## 历史文稿

保留原文，不当作当前实现手册。

| 文档 | 身份 |
|---|---|
| [AI_Memory_System_Design_New.md](AI_Memory_System_Design_New.md) | 记忆系统设计哲学原稿 |
| [tiered_memory_decay_paper.md](tiered_memory_decay_paper.md) | 三档记忆衰减论文稿 |
| [桌面端迁移_多层索引与中央记忆库架构说明.md](桌面端迁移_多层索引与中央记忆库架构说明.md) | 迁移期架构说明 |
| [agent-skills-standard.md](agent-skills-standard.md) | Agent Skills 外部标准摘录 |
| [chat_message_pipeline.md](chat_message_pipeline.md) | 2026-03 聊天管线草案 |
