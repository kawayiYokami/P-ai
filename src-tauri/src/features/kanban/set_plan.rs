// ==================== 议题设置计划文件：业务编排 ====================
// 一条议题最多挂一个计划文件，只存磁盘路径引用，不存正文、不做副本、不做缓存。
// 正文永远以磁盘上那份为准，要读就是模型自己去读那个文件——这里不代劳。
//
// 只有「设置」一件事：已挂的直接覆盖成新路径，既不能卸载、也没有独立的读动作。
// 设置时不校验文件是否存在（计划可能还没落盘）。
//
// 拆成「要 AppState 的入口 + 只吃议题列表的内核」：后者让行为可测，
// 与 kanban_apply_upsert / _in 同一套做法。

/// 设置结果：这条议题现在挂的是哪个文件。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KanbanSetPlanResult {
    number: u32,
    plan_path: String,
}

/// 路径只要求是绝对路径：计划可能落在别的工作树、别的仓库甚至临时目录，
/// 限制目录只会逼着先移动文件，把顺序问题塞进来。
///
/// 空值直接拒：没有卸载动作，传空串等于要求清空。
fn kanban_set_plan_path_checked(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(
            "planPath 不能为空：计划只能更新不能清空，要换就给出新的绝对路径".to_string(),
        );
    }
    if !std::path::Path::new(trimmed).is_absolute() {
        return Err(format!(
            "planPath 必须是磁盘上的绝对路径（如 C:/repo/.pai/plan/chat/xxx.md），收到的是相对路径：{trimmed}"
        ));
    }
    Ok(trimmed.to_string())
}

fn kanban_set_plan(
    state: &AppState,
    leader_conversation_id: &str,
    number: u32,
    plan_path: &str,
) -> Result<KanbanSetPlanResult, String> {
    let mut outcome: Option<KanbanSetPlanResult> = None;
    state_update_conversation_metadata_cached(state, leader_conversation_id, |conversation| {
        let mut issues = conversation.issues.clone();
        let result = kanban_set_plan_in(&mut issues, number, plan_path)?;
        conversation.issues = issues;
        outcome = Some(result);
        Ok(())
    })?;
    outcome.ok_or_else(|| format!("议题 #{number} 在设置计划后读不回来了"))
}

/// 已挂的直接覆盖：没有卸载动作，所以重复设置是常态，不是错误。
fn kanban_set_plan_in(
    issues: &mut [KanbanIssueEntry],
    number: u32,
    plan_path: &str,
) -> Result<KanbanSetPlanResult, String> {
    let plan_path = kanban_set_plan_path_checked(plan_path)?;
    let issue = issues
        .iter_mut()
        .find(|issue| issue.number == number)
        .ok_or_else(|| format!("议题 #{number} 不存在"))?;
    issue.plan_path = plan_path;
    issue.updated_at = now_utc_rfc3339();
    issue.version += 1;
    Ok(KanbanSetPlanResult {
        number: issue.number,
        plan_path: issue.plan_path.trim().to_string(),
    })
}
