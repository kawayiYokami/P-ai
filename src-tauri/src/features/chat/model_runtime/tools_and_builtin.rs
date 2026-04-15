// ==================== tools_and_builtin 模块导图 ====================
// 1) core_provider: provider 调用与通用错误/工具开关
// 2) builtin_*: 内置工具业务实现（网络/记忆/上下文/task/delegate/remote_im）
// 2.5) builtin_plan: 计划呈现/完成协议工具
// 3) tool_arg_types: 各工具参数类型与反序列化辅助
// 4) tool_impls: Builtin*Tool 的 Tool trait 封装层
include!("tools_and_builtin/core_provider.rs");
include!("tools_and_builtin/builtin_network.rs");
include!("tools_and_builtin/builtin_memory.rs");
include!("tools_and_builtin/builtin_runtime_context.rs");
include!("tools_and_builtin/builtin_plan.rs");
include!("tools_and_builtin/tool_arg_types.rs");
include!("tools_and_builtin/builtin_task_delegate.rs");
include!("tools_and_builtin/tool_impls.rs");
include!("tools_and_builtin/builtin_remote_im.rs");
