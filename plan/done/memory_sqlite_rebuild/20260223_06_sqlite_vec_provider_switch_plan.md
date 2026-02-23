# 06. sqlite-vec 向量化与 Provider 切换计划

## 6.1 目标
- 在不改变真相层（SQLite 业务表）语义的前提下，引入 `sqlite-vec` 作为可选语义检索层。
- 支持多 Embedding Provider 并行共存。
- Provider 切换时不阻塞主流程，采用异步 `IndexRebuilder.sync()`。

## 6.2 总体原则
- 真相层不变：`memory_record` / `note_index_record` 仍是唯一权威数据。
- 向量层可重建：任何 provider 的向量索引都可从真相层重算。
- provider 隔离：不同 provider 不能共用同一向量表。
- 可回退：切换失败不影响当前 active provider 的可用性。
- 重建期间默认无向量：检索固定降级为纯 FTS，避免半成品索引参与查询。

## 6.3 数据模型（Provider 分表）

### 6.3.1 provider 注册表
- `embedding_provider`
  - `provider_id TEXT PRIMARY KEY`
  - `dimension INTEGER NOT NULL`
  - `model_name TEXT NOT NULL`
  - `is_active INTEGER NOT NULL DEFAULT 0`
  - `created_at TEXT NOT NULL`
  - `updated_at TEXT NOT NULL`

### 6.3.2 provider 元信息
- `kb_runtime_state`
  - `key TEXT PRIMARY KEY`
  - `value TEXT NOT NULL`
- 关键键值：
  - `active_index_provider_id`
  - `rebuild_status`（idle/running/failed）
  - `rebuild_trace_id`

### 6.3.3 provider 分向量表（关键）
- 表名模板：`memory_vec_{provider_id_norm}`
- 每个 provider 一张独立向量表（sqlite-vec 虚表）。
- 字段建议：
  - `chunk_id TEXT PRIMARY KEY`
  - `embedding BLOB`（vec float32）
  - `updated_at TEXT NOT NULL`

说明：
- `provider_id_norm` 只允许 `[a-z0-9_]`，非法字符转 `_`。
- provider 切换时仅操作目标 provider 对应表，避免互相污染。

## 6.4 读写路径

### 6.4.1 写入（新增/更新文档）
1. 先写真相层文档表。
2. 触发异步同步任务（按 active provider 更新其向量表）。
3. 同步失败仅记录告警，不回滚真相层写入。

### 6.4.2 检索
- 默认：`FTS5 -> rerank(optional vec)`。
- 启用向量时：只读取 `active_index_provider_id` 对应向量表。
- 向量层不可用时：自动降级纯 FTS。
- Provider 切换重建中：强制纯 FTS（无向量模式），直到重建完成并切换 active provider。

## 6.5 Provider 切换流程（对应你的流程图）

1. 用户请求切换 Embedding Provider。
2. `KnowledgeBaseManager.update_kb()` 检测 `embedding_provider_id` 变更。
3. 若 `new_id == active_index_provider_id`：直接返回。
4. 否则启动异步 `IndexRebuilder.sync(new_id)`：
   - 计算向量表名：`memory_vec_{new_id_norm}`。
   - 若表不存在：创建空向量表。
   - 若表存在：加载其 `chunk_id` 集合（可复用）。
5. 计算 Diff：
   - `doc_ids` = 真相层 documents 全量 chunk_id
   - `index_ids` = 目标 provider 向量表全量 chunk_id
   - `to_delete = index_ids - doc_ids`
   - `to_add = doc_ids - index_ids`
   - 当目标表为空时：`to_add = doc_ids`（等效全量重建）
6. 执行删除：从目标 provider 向量表删除 `to_delete`。
7. 执行批量向量化：
   - 从真相层读取 `to_add` 文本
   - 按 `batch_size` 调 `new_provider.get_embeddings()`
   - 写入目标 provider 向量表
8. 全部成功后更新 `active_index_provider_id = new_id`。
9. 可选清理：删除旧 provider 向量表（默认不自动删，改为延迟清理策略）。

## 6.6 失败与一致性策略
- 切换期间 `active_index_provider_id` 不提前改写。
- 只有 `sync` 全部成功才原子切换 active provider。
- 若失败：
  - `rebuild_status=failed`
  - 检索继续纯 FTS（默认无向量）
  - 记录 `trace_id`、失败批次、失败原因

## 6.7 实施接口建议（Rust）
- `trait EmbeddingProvider`:
  - `id() -> &str`
  - `dimension() -> usize`
  - `get_embeddings(texts: &[String]) -> Result<Vec<Vec<f32>>, Error>`
- `trait VectorIndexAdapter`:
  - `ensure_table(provider_id)`
  - `list_ids(provider_id) -> HashSet<String>`
  - `delete_ids(provider_id, ids)`
  - `upsert_embeddings(provider_id, rows)`
  - `search(provider_id, query_vec, top_k)`

## 6.8 任务拆解（增量）
1. 引入 `sqlite-vec` 扩展加载与健康检查。
2. 新增 provider 注册表与 runtime state 表。
3. 实现 provider 分表创建器。
4. 实现 `IndexRebuilder.sync()`（Diff + 批处理 + 原子切换）。
5. 接入切换命令与前端状态展示。
6. 增加失败重试与后台巡检。

## 6.9 验收标准
- 同一批文档在不同 provider 下可独立建索引。
- provider 切换不阻塞主流程，失败不影响旧 provider 查询。
- 切换成功后语义检索只命中新 provider 向量表。
- 关闭向量层后系统仍可纯 FTS 稳定运行。
