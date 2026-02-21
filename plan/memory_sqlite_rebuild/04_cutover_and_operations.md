# 04. 切换与运维方案

## 4.1 切换策略（一次性）
- 发布版本 `X.Y.0` 起启用新架构。
- 启动后仅使用 `memory_store.db`。
- 旧 JSON 记忆字段忽略，不做迁移。

## 4.2 备份恢复
- 备份对象：`memory_store.db`（含 WAL/SHM 协同处理）。
- 提供命令：
  - `backup_memory_db(target_path)`
  - `restore_memory_db(source_path)`
- 恢复后自动触发 FTS 一致性巡检。

## 4.3 巡检机制
- 巡检项：
  - `memory_record` 数量 vs `memory_fts` 数量
  - `note_index_record` 数量 vs `note_fts` 数量
  - 孤儿关系（rel 指向不存在主表）
- 巡检结果：`ok/warn/fail` + 修复动作记录。

## 4.4 故障降级
- FTS5 构建失败：标记 degraded，禁止静默失败。
- 自动 fallback：真相表 LIKE 查询（低性能兜底）。
- 后台重试重建 FTS，成功后自动恢复。

## 4.5 日志与可观测
- 关键日志：
  - 写入耗时
  - 查询耗时
  - FTS 同步延迟
  - 巡检结果
- 关键指标：
  - recall hit rate
  - P50/P95 latency
  - rebuild success rate

## 4.6 发布守门
- Release Checklist 增加阻断项：
  - 无向量模式稳定性
  - FTS 可重建能力
  - 备份恢复可用性
  - 数据一致性达标
