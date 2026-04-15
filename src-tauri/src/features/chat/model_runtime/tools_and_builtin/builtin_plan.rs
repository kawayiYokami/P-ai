pub(crate) fn plan_tool_description() -> String {
    [
        "计划协议工具，是一个完结工具。",
        "用途：",
        "- 当你已经完成需求理解、上下文调查，并消除了关键疑问后，用 present 向用户呈现执行计划。",
        "- 当计划执行完成后，用 complete 向用户汇报结果。",
        "参数：",
        "- action: present | complete",
        "- context: 计划内容或完成汇报",
        "使用规则：",
        "- 只有当目标明确、约束明确、相关现状已调查充分，并且不存在会明显改变计划骨架的关键疑问时，才能调用 present。",
        "- 如果仍有关键疑问，优先继续调查；调查后仍无法消除时，再向用户提出最少必要的问题。",
        "- 不要为了形式而提问，不要为了追求零疑问而提问。",
        "- 只为会明显改变计划结构、影响范围、用户可见行为或风险判断的关键分叉提问。",
        "- 调用本工具后应结束当前调度，不要继续实施其他步骤。",
    ]
    .join("\n")
}

pub(crate) fn builtin_plan(args: PlanToolArgs) -> Result<Value, String> {
    let action = args.action.trim().to_ascii_lowercase();
    if action != "present" && action != "complete" {
        return Err("plan.action 必须是 present 或 complete".to_string());
    }
    let context = args.context.trim().to_string();
    if context.is_empty() {
        return Err("plan.context 不能为空".to_string());
    }
    Ok(serde_json::json!({
        "action": action,
        "context": context,
    }))
}
