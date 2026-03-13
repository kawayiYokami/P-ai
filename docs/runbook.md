# 运行手册

> 更新于 2026-03-13

## 冒烟检查

```powershell
pnpm smoke
```

依次执行：TypeScript 类型检查、Rust 编译检查、Rust 测试编译（`--no-run`）。

## Debug API 模式

在项目根目录放置 `.debug/api-key.json`，填写测试供应商信息。开启后对话路由优先使用 debug 配置。

## 核心运行配置

| 配置项 | 作用 |
| --- | --- |
| selected_api_config_id | 配置页面当前编辑的 API 配置 |
| assistant_department_api_config_id | 对话默认使用的 API 配置 |
| stt_api_config_id | 音转文回退 API（requestFormat 须为 openai_stt） |
| vision_api_config_id | 图转文回退 API |
| terminal_shell_kind | 终端 shell 类型 |
| mcp_servers | MCP 服务器列表 |
| departments | 部门配置列表 |

## 多模态回退规则

1. 对话 API 支持图片/音频 → 直接发送。
2. 不支持音频 → 自动调用 STT 转文字并入消息。
3. 不支持图片 → 自动调用 Vision 转文字，结果按 hash + vision_api_id 缓存。

## 常见错误

| 错误信息 | 处理 |
| --- | --- |
| does not support audio and no STT configured | 配置 stt_api_config_id |
| does not support image and no Vision configured | 配置 vision_api_config_id |
| Request format not implemented | 对话/图转文使用 openai 风格请求格式 |
