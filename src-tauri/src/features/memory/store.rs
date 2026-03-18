use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::collections::{HashMap as StdHashMap, HashSet as StdHashSet};

// ==================== Memory Store（聚合入口） ====================
// TODO(memory/store): 将 runtime_state 键收敛为常量枚举，减少字符串散落。
// TODO(memory/store): 为 provider 索引与维护流程补充更细粒度单测分组。

include!("store/types.rs");
include!("store/db.rs");
include!("store/crud.rs");
include!("store/ownership.rs");
include!("store/import_export.rs");
include!("store/archive_feedback.rs");
include!("store/provider_index.rs");
include!("store/maintenance.rs");
include!("store/tests.rs");
