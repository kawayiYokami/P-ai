// ==================== 运行时共享抽象 ====================
include!("model_runtime/runtime_abstractions.rs");
include!("model_runtime/runtime_migration_guard.rs");

// ==================== 工具定义与内置能力 ====================
include!("model_runtime/tools_and_builtin.rs");

// ==================== Provider 调用与流式处理 ====================
include!("model_runtime/provider_and_stream.rs");
