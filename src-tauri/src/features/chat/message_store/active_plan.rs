const ACTIVE_PLAN_STATUS_IN_PROGRESS: &str = "in_progress";
const ACTIVE_PLAN_STATUS_COMPLETED: &str = "completed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivePlanRecord {
    plan_id: String,
    source_message_id: String,
    status: String,
    #[serde(default)]
    path: String,
    created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completion_text: Option<String>,
}

fn active_plan_records_in_progress(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Vec<ActivePlanRecord>, String> {
    Ok(chat_metadata_store_read_active_plans(data_path, conversation_id)?
        .unwrap_or_default()
        .into_iter()
        .filter(|record| record.status.trim() == ACTIVE_PLAN_STATUS_IN_PROGRESS)
        .collect())
}

pub(super) fn active_plan_append_in_progress(
    data_path: &PathBuf,
    conversation_id: &str,
    source_message_id: &str,
    path: &str,
) -> Result<(), String> {
    let paths = message_store_paths(data_path, conversation_id)?;
    let record = ActivePlanRecord {
        plan_id: Uuid::new_v4().to_string(),
        source_message_id: source_message_id.trim().to_string(),
        status: ACTIVE_PLAN_STATUS_IN_PROGRESS.to_string(),
        path: path.trim().to_string(),
        created_at: now_iso(),
        completed_at: None,
        completion_text: None,
    };
    if record.source_message_id.is_empty() {
        return Err("sourceMessageId 为空，无法写入执行中计划。".to_string());
    }
    if record.path.is_empty() {
        return Err("计划路径为空，无法写入执行中计划。".to_string());
    }
    with_conversation_mutation_for_data_path(
        data_path,
        conversation_id,
        "active_plan_append_in_progress",
        || chat_metadata_store_append_active_plan(&paths, &record),
    )
}

pub(super) fn active_plan_complete_by_path(
    data_path: &PathBuf,
    conversation_id: &str,
    path: &str,
    completion_text: Option<&str>,
) -> Result<bool, String> {
    let normalized_path = path.trim();
    if normalized_path.is_empty() {
        return Err("计划路径为空，无法完成执行中计划。".to_string());
    }
    let paths = message_store_paths(data_path, conversation_id)?;
    with_conversation_mutation_for_data_path(
        data_path,
        conversation_id,
        "active_plan_complete_by_path",
        || {
            chat_metadata_store_complete_active_plan_by_path(
                &paths,
                normalized_path,
                completion_text,
            )
        },
    )
}

#[cfg(test)]
#[test]
fn active_plan_records_in_progress_should_return_newest_first() {
    let root = std::env::temp_dir().join(format!("eca-active-plan-order-{}", Uuid::new_v4()));
    let conversation_id = "conv-active-plan-order";
    let data_path = root.join("config_mark");
    let paths = message_store_paths(&data_path, conversation_id).expect("message store paths");
    let conversation = Conversation {
        id: conversation_id.to_string(),
        title: "active plan order".to_string(),
        agent_id: DEFAULT_AGENT_ID.to_string(),
        bound_conversation_id: None,
        parent_conversation_id: None,
        child_conversation_ids: Vec::new(),
        fork_message_cursor: None,
        unread_count: 0,
        conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
        root_conversation_id: None,
        delegate_id: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        last_user_at: None,
        last_assistant_at: None,
        status: "active".to_string(),
        user_profile_snapshot: String::new(),
        shell_workspace_path: None,
        shell_workspaces: Vec::new(),
        shell_autonomous_mode: false,
        shell_work_mode: default_shell_work_mode(),
        shell_work_branch: String::new(),
        archived_at: None,
        messages: Vec::new(),
        fast_request_turns: Vec::new(),
        current_todos: Vec::new(),
        memory_recall_table: Vec::new(),
        plan_mode_enabled: false,
        preferred_api_config_id: None,
        auto_push_remote_contact_id: None,
        active_goal: None, last_error: None,
        cumulative_usage: ConversationCumulativeUsage::default(),
        is_draft: false,
    };
    chat_store_write_snapshot(&paths, &conversation).expect("write ready snapshot");
    let append_record = |plan_id: &str, status: &str, path: &str, created_at: &str| {
        chat_metadata_store_append_active_plan(
            &paths,
            &ActivePlanRecord {
                plan_id: plan_id.to_string(),
                source_message_id: "msg-1".to_string(),
                status: status.to_string(),
                path: path.to_string(),
                created_at: created_at.to_string(),
                completed_at: None,
                completion_text: None,
            },
        )
        .expect("append active plan record")
    };
    append_record("old", "in_progress", "C:/old.md", "2026-01-01T00:00:00Z");
    append_record("done", "completed", "C:/done.md", "2026-01-01T00:00:01Z");
    append_record("new", "in_progress", "C:/new.md", "2026-01-01T00:00:02Z");

    let records =
        active_plan_records_in_progress(&data_path, conversation_id).expect("read active plans");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].plan_id, "new");
    assert_eq!(records[1].plan_id, "old");

    let _ = fs::remove_dir_all(root);
}