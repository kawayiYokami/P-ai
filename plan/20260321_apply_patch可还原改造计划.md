# apply_patch 可还原改造计划

## 1. 背景

当前 `apply_patch` 已支持：

- 新增文件：撤回时校验内容后删除。
- 修改文件：撤回时通过反向 hunk 恢复。
- 重命名文件：撤回时先反向修改再移回原路径。

当前缺口：

- `Delete File` 执行后会直接删除工作区文件，没有保存原始内容快照。
- 会话撤回依赖原始补丁生成反向操作，因此遇到删除文件会明确报错，无法完整撤回。
- 目前没有一套面向 `apply_patch` 的临时备份目录与索引，无法统一管理新增/删除/修改的恢复材料。

目标是把 `apply_patch` 改造成“新增、删除、修改、重命名都可还原”的链路，并在上下文压缩时清理这批临时缓存。

## 2. 目标

### 2.1 功能目标

1. `apply_patch` 执行前/执行时，为每个成功变更生成可恢复记录。
2. 删除文件前，先备份到临时目录。
3. 修改文件前，保存修改前原文快照。
4. 新增文件时，记录该文件由本次补丁创建，撤回时按“内容未漂移才删除”处理。
5. 重命名文件时，记录原路径、目标路径与原内容，使撤回不依赖纯反向推导。
6. 会话撤回 `undo_apply_patch` 时，优先按备份记录恢复，并在恢复成功后删除对应缓存。
7. 上下文压缩时，清理 `apply_patch` 临时目录与索引，避免旧缓存无限堆积。

### 2.2 非目标

1. 不做独立于会话消息之外的长期版本管理系统。
2. 不做 git 级别历史恢复。
3. 不支持目录级别删除恢复，本次仍以文件级操作为范围。

## 3. 设计概览

### 3.1 临时目录布局

在应用数据根目录新增一组 `apply_patch` 临时目录，例如：

- `temp/apply_patch/records/`
- `temp/apply_patch/blobs/`

其中：

- `records/` 存 JSON 索引，按 `tool_call_id` 或 `session + 时间戳 + uuid` 建记录。
- `blobs/` 存原始文件备份内容，避免把大文本直接塞进聊天消息。

说明：

- 这里的 `temp` 是应用数据目录下的受控目录，不依赖系统随机 temp，便于应用自身清理。
- 若仓库里已有统一 temp 目录约定，实际落地时优先复用现有根路径。

### 3.2 记录模型

新增一份 `apply_patch` 撤回记录模型，至少包含：

- `record_id`
- `tool_call_id`
- `session_id`
- `cwd`
- `created_at`
- `entries`

每条 `entry` 至少包含：

- `kind`: `add` / `delete` / `update` / `move`
- `path`
- `from_path` / `to_path`（按需）
- `backup_blob_path`（删除/修改/移动前的原文快照）
- `expected_post_content`（新增文件或必要校验场景）

### 3.3 前向执行策略

#### Add

- 正常创建文件。
- 记录该文件路径和创建后的期望内容。

#### Delete

- 删除前先读取原文件内容。
- 将原内容写入 `temp/apply_patch/blobs/`。
- 写入索引后再删除工作区文件。

#### Update

- 更新前先读取旧内容并备份。
- 成功写入新内容后记录原文件快照位置。

#### Move

- 移动前同样保存源文件原文。
- 记录源路径和目标路径。
- 撤回时直接按记录恢复，不只依赖反向 hunk。

## 4. 撤回策略

`try_undo_apply_patch_from_removed_messages` 改为：

1. 先从移除消息中收集成功的 `apply_patch` 调用。
2. 为每个调用定位对应 temp 记录。
3. 按记录逆序恢复：
   - `add`：若文件内容仍等于本次创建内容，则删除。
   - `delete`：若目标路径不存在，则把备份文件复制回原路径。
   - `update`：若当前文件仍处于本次补丁产物状态，直接用备份原文覆盖恢复。
   - `move`：若目标文件仍存在且未漂移，则按原路径恢复并删除目标路径。
4. 成功恢复后删除该记录引用的 blob 与 record。
5. 若某条记录缺失或目标已漂移，返回明确错误，阻止“误恢复覆盖用户新改动”。

## 5. 压缩清理策略

在上下文压缩成功后，增加一次 `apply_patch temp` 清理：

1. 删除 `temp/apply_patch/records/` 中全部记录。
2. 删除 `temp/apply_patch/blobs/` 中全部备份文件。
3. 保留目录结构，便于后续继续写入。
4. 输出中文 INFO 日志，包含清理条数和耗时。

注意：

- 只要聊天消息还在、对应 `temp/apply_patch` 记录还在、目标文件未发生不可安全覆盖的漂移，则应用重启后仍应支持撤回。
- 压缩清理意味着压缩点之前的补丁缓存不再可撤回，这是符合“压缩后丢弃旧临时上下文”的预期。
- 若后续希望更细粒度保留，可再按 conversation/session 做定向清理；本次先实现全量清理。

## 6. 涉及文件

- `src-tauri/src/features/system/tools/patch.rs`
- `src-tauri/src/features/system/tools/patch_rewind.rs`
- `src-tauri/src/features/system/commands/archive_pipeline.rs`
- 可能补充到：
  - `src-tauri/src/features/config/app_data_layout.rs`
  - `src-tauri/src/features/core/domain.rs`（如需新增数据结构）

## 7. 验收点

1. `Delete File` 后会生成备份记录，撤回可恢复原文件。
2. `Update File` 撤回不再只依赖 hunk，可直接恢复原文。
3. `Add File` 仍保持“内容未漂移才删除”的安全策略。
4. `Move + Update` 可恢复到原路径与原内容。
5. 撤回成功后对应 temp 记录与 blob 被清空。
6. 上下文压缩后，`temp/apply_patch` 目录被清空。
7. Rust 单测覆盖新增/删除/修改/移动/缓存清理场景。

## 8. 自测建议

- `cargo test apply_patch_tool_tests`
- `cargo test rewind_apply_patch_tests`
- 视实现补充单测：
  - 删除文件备份与恢复
  - 修改文件备份与恢复
  - move+update 恢复
  - 压缩清理 temp

## 9. 风险与注意事项

1. 需要避免 temp 记录和真实工作区路径越界，所有记录文件路径都要走受控目录。
2. 恢复时必须做漂移校验，不能直接覆盖用户后续手改内容。
3. 如果一次补丁包含多文件操作，记录和清理要保证原子性，避免半成功半失败后无法判断状态。
4. 若补丁执行成功但记录落盘失败，应整体报错并停止后续操作，避免产生“已修改但不可恢复”的新状态。
