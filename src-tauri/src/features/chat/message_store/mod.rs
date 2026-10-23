mod message_store {
    // 第一阶段先建立可独立测试的迁移边界，运行路径接入会在后续阶段完成。
    #![allow(dead_code)]

    use super::*;

    include!("paths.rs");
    include!("manifest.rs");
    include!("meta.rs");
    include!("index.rs");
    include!("active_plan.rs");
    include!("sqlite.rs");
    include!("jsonl_snapshot.rs");
    include!("zstd_blocks.rs");
    include!("group_lines.rs");
    include!("verification.rs");
    include!("store.rs");
    include!("persist.rs");
    include!("migration.rs");
    include!("migration_v4.rs");
    include!("usage_trail.rs");
    include!("repair.rs");
}
