# 归档驱动记忆衰退与合并计划（待审查）

## 1. 目标与范围
- 在归档阶段引入结构化 JSON 输出，驱动：
  - 有用记忆强化（useful IDs）
  - 无用召回衰退
  - 新记忆写入
  - 重复记忆合并
  - T0 自然衰减与删除
- 以当前 SQLite 记忆库为唯一实现目标（不考虑旧实现兼容）。

## 2. 主持人格与记忆输入规则
### 2.1 基础定义
- `公有人格`：`private_memory_enabled = false`
- `私有人格`：`private_memory_enabled = true`
- 会话内每个人格统计发言量（按消息条数）。

### 2.2 主持人格选择
- 若存在公有人格：
  - 选择“发言量最高的公有人格”为主持人格。
- 若全是私有人格：
  - 使用当前主会话人格为主持人格。

### 2.3 平票规则（稳定可复现）
- 先比较最近发言时间（更近者优先）
- 再比较 `agent_id` 字典序（升序）

### 2.4 整理输入范围
- 主持人格：`自己的私有记忆 + 公共记忆 + 会话记录`
- 非主持人格：`自己的私有记忆 + 会话记录`

## 3. 归档 JSON 协议（LLM 输出）
> 说明：`ownerAgentId` 不由 LLM 输出，后端根据当前人格上下文强制写入。

```json
{
  "summary": "string",
  "usefulMemoryIds": ["memory-id-1", "memory-id-2"],
  "newMemories": [
    {
      "memoryType": "knowledge|skill|emotion|event",
      "judgment": "string",
      "reasoning": "string",
      "tags": ["tag1", "tag2"]
    }
  ],
  "mergeGroups": [
    {
      "sourceIds": ["id-a", "id-b"],
      "target": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["tag1", "tag2"]
      }
    }
  ]
}
```

### 3.1 协议校验规则
- `summary` 必须非空。
- `usefulMemoryIds` 必须是会话 `memory_recall_table` 中 ID 的子集（归档阶段统一反馈）。
- `newMemories[*].judgment`、`memoryType`、`tags` 必填。
- `mergeGroups[*].sourceIds` 长度 >= 2，且 source ID 必须存在。
- 任意字段非法：进入降级路径（只保存 summary，不执行强化/衰退/合并）。

## 4. 三档衰退策略（TMD）
### 4.1 默认参数
- `tier0_threshold = 3.0`
- `tier1_threshold = 10.0`
- `consolidate_speed = 2.5`
- `useful_boost = 1`
- `cycle_tier0_days = 3`
- `forget_speed = 1.0`
- `tier0_forget_speed = 1.0`

### 4.2 执行规则
- 有用反馈（`usefulMemoryIds`）：
  - `strength += 1`
  - `useful_count += 1`
  - `useful_score += 2.5`
  - `last_recalled_at = now`
- 无用召回（`recalled_ids - useful_ids`）：
  - 仅 T1：`strength -= 1`
  - T0：忽略（由自然衰减处理）
  - T2：忽略（永不遗忘）
- T0 自然衰减：
  - 按周期批量扣减 `strength`
  - 更新时间 `last_decay_at`
- 删除条件：
  - `strength <= 0 && is_active = false`

## 5. 事务与执行顺序
在单事务内执行以下步骤（失败回滚）：
1. 解析并校验归档 JSON
2. 从会话 `memory_recall_table` 读取 `recalled_ids`，并应用 useful/无用反馈
3. 写入 `newMemories`
4. 执行 `mergeGroups`
5. 执行 T0 自然衰减
6. 删除失效记忆
7. 写归档 summary 与执行报告

## 6. 模块改造点
- `src-tauri/src/features/system/commands/archive_and_memory.rs`
  - 扩展归档输出解析与执行编排
- `src-tauri/src/features/memory/store.rs`
  - 新增反馈更新、自然衰减、删除、合并的事务接口
- `src-tauri/src/features/memory/decay.rs`（新）
  - 档位计算与策略函数（纯逻辑）

## 7. 测试计划
### 7.1 单元测试
- 主持人格选择（公有优先、全私有回退、平票）
- JSON 解析/校验（合法/非法）
- T0/T1/T2 分档与衰退规则
- 合并后档位不降（`useful_score`、`last_recalled_at` 取 max）

### 7.2 集成测试
- 归档全链路（summary + useful + new + merge + decay + delete）
- LLM 输出坏 JSON 降级路径
- 多人格场景下输入隔离是否符合规则

## 8. 验收标准
- 每次归档产出结构化执行报告（强化条数/衰退条数/合并条数/删除条数）。
- 主持人格选择结果可复现且可解释。
- 非主持人格不会读取公共记忆。
- 归档失败不污染现有记忆状态（事务回滚）。

## 9. 审查结论落地说明
- 已采纳：
  - 明确 T0/T1/T2 的无用召回行为。
  - 明确私有记忆查询条件：`owner_agent_id = agent_id`。
  - 增加主持人格选择日志与平票规则测试、降级路径测试。
- 不采纳：
  - “归档阶段没有 recalled_ids”结论不成立；本方案使用 `memory_recall_table` 作为 recalled 来源。
  - “merge 不使用 ID”建议不采纳；首版保持 ID 驱动避免误合并。
