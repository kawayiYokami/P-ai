// ==================== 配置与人格命令 ====================
include!("commands/config_and_persona.rs");

// ==================== 提示词组装层 ====================
include!("commands/prompt_assembly.rs");

// ==================== 调试日志命令 ====================
include!("commands/debug_log_commands.rs");

// ==================== 记忆整理（独立模块） ====================
include!("commands/memory_curation/prompt_contract.rs");

// ==================== 归档JSON解析层 ====================
include!("commands/archive_summary_parser.rs");

// ==================== 推理网关层 ====================
include!("commands/inference_gateway.rs");

// ==================== 记忆命令 ====================
include!("commands/memory_commands.rs");

// ==================== 记忆供应商命令 ====================
include!("commands/memory_provider_commands.rs");

// ==================== 归档命令 ====================
include!("commands/archive_commands.rs");

// ==================== 归档导入导出命令 ====================
include!("commands/archive_io_commands.rs");

// ==================== 归档主持人格选择 ====================
include!("commands/archive_host_selector.rs");

// ==================== PDF文本服务 ====================
include!("services/pdf_text_service.rs");

// ==================== 归档执行流水线 ====================
include!("commands/archive_pipeline.rs");

// ==================== 对话与运行时命令 ====================
include!("commands/chat_and_runtime.rs");

// ==================== 桌面工具命令 ====================
include!("commands/desktop_tools.rs");
