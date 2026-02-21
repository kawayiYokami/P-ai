# 03. 实施任务拆解（WBS）

## 3.1 里程碑 M1：基础设施
- 引入依赖：`rusqlite`（含 `bundled`）、`time`、`uuid`、`serde`。
- 新建 `memory_store` 模块并在 `features` 层注册。
- 实现 DB 路径策略（例如 `config_dir/memory_store.db`）。
- 接入启动初始化：建库、建表、设置 PRAGMA。

## 3.2 里程碑 M2：真相层 CRUD
- 实现 MemoryRecord CRUD：创建、更新、删除、列表、按条件查询。
- 实现 Tag CRUD 与关系绑定。
- 实现 NoteIndexRecord 与 NoteTagRel 仓储。
- 所有写入事务化，错误码结构化（便于前端展示）。

## 3.3 里程碑 M3：FTS5 与同步机制
- 实现 `upsert_memory_fts(item_id, tags, judgment)`。
- 实现 `upsert_note_fts(item_id, tags)`。
- 实现删除同步（真相表删除 -> FTS 删除）。
- 实现全量重建命令（truncate + rebuild）。
- 实现增量同步队列（本地事件触发）。

## 3.4 里程碑 M4：检索服务
- `search_memories(query, top_k)`。
- `search_notes(query, top_k)`。
- 返回 explain 元数据（关键词命中、来源字段、排序分）。
- 接入现有 prompt 构造路径，替代旧 JSON matcher。

## 3.5 里程碑 M5：遗忘算法
- 落地 useful_score 三档规则（T0/T1/T2）。
- 落地 consolidate / decay / cycle 计算。
- 参数读取优先级：默认值 < policy override。
- 定时任务入口：自然遗忘批处理。

## 3.6 里程碑 M6：命令层与前端改造
- 替换 `add/update/delete/list/export/import memories` 命令实现。
- 前端 memory viewer 对接新 API。
- 新增诊断入口：索引健康检查、重建按钮。

## 3.7 里程碑 M7：清理与收口
- 删除旧 `AppData.memories` 相关读写代码。
- 删除旧 matcher 缓存逻辑。
- 文档更新：runbook、schema、release checklist。

## 3.8 预估节奏
1. Week 1: M1-M2
2. Week 2: M3-M4
3. Week 3: M5-M6
4. Week 4: M7 + 验收 + 发布
