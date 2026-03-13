# 迁移策略

> 更新于 2026-03-13

## 当前阶段

项目处于早期阶段，不保证旧配置兼容。配置缺失或无效时自动归一化到安全状态。

## 归一化规则

### Config

1. api_configs 为空 → 使用默认配置。
2. selected_api_config_id 无效 → 回退到第一个 API 配置。
3. assistant_department_api_config_id 无效或不支持文本 → 回退到第一个文本 API 配置。
4. stt_api_config_id 无效或不支持音频 → 清空。
5. vision_api_config_id 无效或不支持图片 → 清空。
6. departments 中引用的 api_config_id / agent_id 无效 → 移除无效引用。
7. mcp_servers 中 definition_json 解析失败 → 标记 last_status 为错误。

### AppData

1. version 不匹配 → 按版本号执行迁移。
2. agents 为空 → 创建默认 agent。
3. assistant_department_agent_id 无效 → 回退到默认 agent。
4. response_style_id 无效 → 回退到 "concise"。
5. conversations 中 api_config_id / agent_id 无效 → 保留对话但标记无效。
