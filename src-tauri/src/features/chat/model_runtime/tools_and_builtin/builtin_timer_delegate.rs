// 当前文件保留 include! 方式，复用同一作用域内的大量私有类型与函数，避免额外暴露可见性。
// ========== timer 工具实现 ==========
include!("builtin_timer.rs");

// ========== delegate 工具实现 ==========
include!("builtin_delegate.rs");
