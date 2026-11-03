// ==================== 队伍议题看板 ====================
// 用 mod 包裹而不是直接平铺：include! 是文本替换，平铺时本文件的
// `use` 会和 features/memory/store.rs 引入的同名项在同一作用域重复定义。
// 包一层 mod 后这里拥有独立作用域，`use super::*` 仍能拿到 main.rs 的全部全局项。
mod kanban {
    use super::*;

    include!("kanban/domain.rs");
    include!("kanban/service.rs");
    include!("kanban/set_plan.rs");
    include!("kanban/tool.rs");
    include!("kanban/set_plan_tool.rs");
    include!("kanban/commands.rs");
    include!("kanban/tests.rs");
}
