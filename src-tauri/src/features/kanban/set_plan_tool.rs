// ==================== 队伍议题看板：设置计划文件工具 ====================
// 独立于 kanban 工具：那个管议题本身的增改查，这个只管议题与磁盘计划的绑定。
// 拆开是因为两件事的触发时机完全不同——改状态每轮都可能发生，设计划是少数几次。
//
// 只有设置一件事：不提供卸载（计划不能清空）、也不提供读取（模型自己会读文件）。
pub(crate) const KANBAN_SET_PLAN_TOOL_NAME: &str = "kanban_set_plan";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanSetPlanToolRequest {
    /// 操作哪一条议题。
    #[serde(default)]
    pub(crate) number: Option<u32>,
    /// 磁盘上的绝对路径。已挂的会被覆盖，不能传空。
    #[serde(default)]
    pub(crate) plan_path: Option<String>,
}

pub(crate) fn kanban_set_plan_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        KANBAN_SET_PLAN_TOOL_NAME,
        "队伍议题看板：给议题设置磁盘上的计划文件（队长专属）。\
         一条议题只挂一个计划，只记路径、不存正文，正文以磁盘上那份为准。\
         已经挂了再设就是覆盖成新路径；不能清空，也没有卸载动作。\
         设置的时候不检查文件是否存在（计划可能还没写），要看正文就自己去读那个文件。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "议题编号"
                },
                "planPath": {
                    "type": "string",
                    "description": "计划文件在磁盘上的绝对路径，例如 C:/repo/.pai/plan/chat/xxx.md。已挂会被覆盖，不能传空"
                }
            },
            "required": ["number", "planPath"]
        }),
    )
}

pub(crate) fn builtin_kanban_set_plan(
    app_state: &AppState,
    session_id: &str,
    args: KanbanSetPlanToolRequest,
) -> Result<String, String> {
    let leader_conversation_id = kanban_leader_conversation_id(app_state, session_id)?;
    let number = args
        .number
        .ok_or_else(|| "kanban_set_plan 必须提供 number：说明要操作哪一条议题".to_string())?;
    let plan_path = args
        .plan_path
        .as_deref()
        .ok_or_else(|| "kanban_set_plan 必须提供 planPath：计划只能更新不能清空".to_string())?;

    let result = kanban_set_plan(app_state, &leader_conversation_id, number, plan_path)?;
    kanban_emit_updated(app_state, &leader_conversation_id);

    Ok(format!("#{} plan set\n{}", result.number, result.plan_path))
}
