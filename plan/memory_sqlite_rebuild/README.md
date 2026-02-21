# 记忆系统重建计划（SQLite + rusqlite）

## 目标
- 完全放弃当前 JSON 记忆实现。
- 使用 `SQLite` 作为唯一真相源，`FTS5` 作为检索加速层。
- 先交付无向量模式（模式 A）：`SQL + FTS5`。
- 架构预留向量层扩展位，但本阶段不实现向量检索。

## 文档索引
- `plan/memory_sqlite_rebuild/01_architecture_and_scope.md`
- `plan/memory_sqlite_rebuild/02_schema_and_sql_plan.md`
- `plan/memory_sqlite_rebuild/03_implementation_work_breakdown.md`
- `plan/memory_sqlite_rebuild/04_cutover_and_operations.md`
- `plan/memory_sqlite_rebuild/05_acceptance_and_test_plan.md`
- `plan/memory_sqlite_rebuild/06_sqlite_vec_provider_switch_plan.md`

## 执行顺序（建议）
1. 冻结旧记忆入口，建立新模块骨架。
2. 落地 SQL schema + 仓储层 + 事务规范。
3. 落地 FTS5 分索引、增量同步、全量重建。
4. 接入命令层与 UI，跑通端到端。
5. 补齐巡检、备份恢复、可观测指标。
6. 按验收标准收口并删除旧 JSON 记忆代码。

## 非目标
- 不做旧数据迁移。
- 不做兼容读写。
- 不做向量检索实现。

## 关键原则
- 所有写入先落中央 SQL。
- FTS5 可重建，不承载业务权威字段。
- 记忆与笔记索引物理分离。
- 检索可解释、可观测、可降级。
