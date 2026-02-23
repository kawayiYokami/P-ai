# 02. 数据模型与 SQL 方案

## 2.1 目标表
- `memory_record`
- `global_tag`
- `memory_tag_rel`
- `note_index_record`
- `note_tag_rel`
- `memory_event_log`（可选，审计/回放）
- `memory_decay_policy`（可选，参数覆盖）

## 2.2 核心字段（MemoryRecord）
- `id TEXT PRIMARY KEY`
- `memory_type TEXT NOT NULL`
- `judgment TEXT NOT NULL`
- `reasoning TEXT NOT NULL DEFAULT ''`
- `strength INTEGER NOT NULL DEFAULT 0`
- `is_active INTEGER NOT NULL DEFAULT 1`
- `memory_scope TEXT NOT NULL DEFAULT 'public'`
- `useful_count INTEGER NOT NULL DEFAULT 0`
- `useful_score REAL NOT NULL DEFAULT 0`
- `last_recalled_at TEXT`
- `last_decay_at TEXT`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

## 2.3 标签与关系
- `global_tag(id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL)`
- `memory_tag_rel(memory_id TEXT, tag_id TEXT, PRIMARY KEY(memory_id, tag_id))`
- `note_tag_rel(source_id TEXT, tag_id TEXT, PRIMARY KEY(source_id, tag_id))`

## 2.4 笔记索引主表
- `source_id TEXT PRIMARY KEY`
- `note_short_id INTEGER UNIQUE NOT NULL`
- `file_id TEXT NOT NULL`
- `source_file_path TEXT NOT NULL`
- `heading_h1..heading_h6 TEXT`
- `total_lines INTEGER NOT NULL DEFAULT 0`
- `updated_at TEXT NOT NULL`

## 2.5 FTS5 设计（物理分离）
- `memory_fts(item_id UNINDEXED, tags, judgment)`
- `note_fts(item_id UNINDEXED, tags)`
- FTS 表只承载检索字段，不承载真相字段。

## 2.6 SQL 迁移规范
- 使用 `schema_version`（`PRAGMA user_version`）维护版本。
- `V1`：落地所有真相表 + FTS5 虚表 + 基础索引。
- `V2+`：只增量 ALTER，禁止破坏性自动变更。

## 2.7 索引与约束
- memory:
  - `idx_memory_updated_at`
  - `idx_memory_scope_active`
  - `idx_memory_useful_score`
- note:
  - `idx_note_updated_at`
  - `idx_note_file_id`
- rel:
  - `idx_memory_tag_tag_id`
  - `idx_note_tag_tag_id`

## 2.8 检索流程（模式 A）
1. 用户查询 -> 标准化关键词。
2. 先查 `memory_fts` / `note_fts`。
3. 取 Top-K 候选后回查真相表补齐字段。
4. 返回结果与 explain（命中标签、匹配字段、排序分数）。
