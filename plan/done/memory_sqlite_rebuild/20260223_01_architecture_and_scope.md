# 01. 架构与范围

## 1.1 目标架构
- 真相层：SQLite（单文件 DB，WAL 模式）。
- 稀疏检索层：FTS5（`memory_fts`、`note_fts` 两张独立虚表）。
- 维护层：启动预构建、运行期增量同步、定时巡检修复。

## 1.2 分层边界
- 真相层负责：业务字段、事务一致性、备份恢复。
- FTS5 负责：关键词召回性能，不保存业务语义真相。
- 应用层负责：检索融合策略、阈值、可观测日志。

## 1.3 模块拆分（Rust）
- `src-tauri/src/features/memory_store/`
- `db.rs`：连接池/连接工厂、PRAGMA、事务入口。
- `schema.rs`：建表 SQL、索引 SQL、版本迁移。
- `repo_memory.rs`：MemoryRecord CRUD。
- `repo_note.rs`：NoteIndexRecord CRUD。
- `repo_tag.rs`：Tag 与关系表操作。
- `fts_sync.rs`：FTS 增量同步 + 全量重建。
- `search.rs`：FTS 查询、TopK、解释信息。
- `decay.rs`：遗忘算法与参数应用。
- `health.rs`：一致性巡检与自动修复。

## 1.4 命令层改造
- `commands/archive_and_memory.rs` 中记忆相关命令全部改调新仓储层。
- 新增命令：
  - `memory_rebuild_indexes`
  - `memory_health_check`
  - `memory_backup_db`
  - `memory_restore_db`

## 1.5 切换策略
- 单次大版本切换，不保留旧 JSON 记忆。
- 启动时检测 DB 不存在则初始化 schema。
- 启动时检测旧 memories 字段：忽略，不迁移。

## 1.6 性能与稳定性基础设定
- SQLite PRAGMA:
  - `journal_mode=WAL`
  - `synchronous=NORMAL`
  - `temp_store=MEMORY`
  - `foreign_keys=ON`
- 所有写操作走显式事务。
- 所有查询提供超时/取消钩子（命令层超时控制）。
