const TASK_DB_FILE_NAME: &str = "task_store.db";
const TASK_STATE_ACTIVE: &str = "active";
const TASK_STATE_COMPLETED: &str = "completed";
const TASK_STATE_FAILED_COMPLETED: &str = "failed_completed";
const TASK_SCHEDULER_INTERVAL_SECONDS: u64 = 30;
const TASK_IMMEDIATE_RETRY_SECONDS: i64 = TASK_SCHEDULER_INTERVAL_SECONDS as i64;
const TASK_MAX_BOARD_ITEMS: usize = 4;
const TASK_TARGET_SCOPE_DESKTOP: &str = "desktop";
const TASK_TARGET_SCOPE_CONTACT: &str = "contact";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskTriggerInputLocal {
    #[serde(default, alias = "runAt", alias = "run_at")]
    run_at_local: Option<String>,
    #[serde(default)]
    every_minutes: Option<f64>,
    #[serde(default, alias = "endAt", alias = "end_at")]
    end_at_local: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskTriggerView {
    #[serde(default)]
    run_at_local: Option<String>,
    #[serde(default)]
    every_minutes: Option<f64>,
    #[serde(default)]
    end_at_local: Option<String>,
    #[serde(default)]
    next_run_at_local: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskTriggerStored {
    run_at_utc: Option<String>,
    every_minutes: Option<f64>,
    end_at_utc: Option<String>,
    next_run_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskProgressNoteView {
    #[serde(default)]
    at_local: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskProgressNoteStored {
    #[serde(alias = "at", alias = "atUtc")]
    at_utc: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskEntry {
    task_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
    order_index: i64,
    goal: String,
    why: String,
    todo: String,
    completion_state: String,
    #[serde(default)]
    completion_conclusion: String,
    progress_notes: Vec<TaskProgressNoteView>,
    trigger: TaskTriggerView,
    created_at_local: String,
    updated_at_local: String,
    #[serde(default)]
    last_triggered_at_local: Option<String>,
    #[serde(default)]
    completed_at_local: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskRecordStored {
    task_id: String,
    conversation_id: Option<String>,
    target_scope: String,
    order_index: i64,
    title: String,
    cause: String,
    goal: String,
    flow: String,
    todos: Vec<String>,
    status_summary: String,
    completion_state: String,
    completion_conclusion: String,
    progress_notes: Vec<TaskProgressNoteStored>,
    stage_key: String,
    stage_updated_at_utc: Option<String>,
    trigger: TaskTriggerStored,
    created_at_utc: String,
    updated_at_utc: String,
    last_triggered_at_utc: Option<String>,
    completed_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskBoardSnapshot {
    tasks: Vec<TaskEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRunLogEntry {
    id: i64,
    task_id: String,
    triggered_at_local: String,
    outcome: String,
    note: String,
}

#[derive(Debug, Clone)]
struct TaskRunLogStored {
    id: i64,
    task_id: String,
    triggered_at_utc: String,
    outcome: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRunLogListInput {
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskCreateInput {
    goal: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    target_scope: Option<String>,
    #[serde(default)]
    why: String,
    #[serde(default)]
    todo: String,
    trigger: TaskTriggerInputLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdateInput {
    task_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    target_scope: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    todo: Option<String>,
    #[serde(default)]
    trigger: Option<TaskTriggerInputLocal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskCompleteInput {
    task_id: String,
    completion_state: String,
    #[serde(default)]
    completion_conclusion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskDeleteInput {
    task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskGetInput {
    task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskDispatchNowInput {
    task_id: String,
}

fn task_every_minutes_to_duration(every_minutes: f64) -> Option<time::Duration> {
    if !every_minutes.is_finite() || every_minutes <= 0.0 {
        return None;
    }
    let millis = (every_minutes * 60_000.0).round() as i64;
    if millis <= 0 {
        return None;
    }
    Some(time::Duration::milliseconds(millis))
}

fn task_trigger_view_from_stored(trigger: &TaskTriggerStored) -> TaskTriggerView {
    TaskTriggerView {
        run_at_local: trigger
            .run_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
        every_minutes: trigger.every_minutes,
        end_at_local: trigger
            .end_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
        next_run_at_local: trigger
            .next_run_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
    }
}

fn task_progress_note_view_from_stored(note: &TaskProgressNoteStored) -> TaskProgressNoteView {
    TaskProgressNoteView {
        at_local: format_utc_storage_time_to_local_rfc3339(&note.at_utc),
        note: note.note.clone(),
    }
}

fn task_goal_from_legacy_fields(title: &str, goal: &str) -> String {
    let normalized_goal = goal.trim();
    if !normalized_goal.is_empty() {
        return normalized_goal.to_string();
    }
    title.trim().to_string()
}

fn task_why_from_legacy_record(record: &TaskRecordStored) -> String {
    let normalized_goal = task_goal_from_legacy_fields(&record.title, &record.goal);
    let normalized_title = record.title.trim();
    let mut parts = Vec::<String>::new();
    if !record.cause.trim().is_empty() {
        parts.push(record.cause.trim().to_string());
    }
    if !normalized_title.is_empty() && normalized_title != normalized_goal {
        parts.push(format!("原标题：{}", normalized_title));
    }
    if !record.flow.trim().is_empty() {
        parts.push(format!("原流程：{}", record.flow.trim()));
    }
    if !record.stage_key.trim().is_empty() {
        parts.push(format!("原阶段：{}", record.stage_key.trim()));
    }
    parts.join("\n")
}

fn task_todo_from_legacy_fields(status_summary: &str, todos: &[String]) -> String {
    let normalized_status = status_summary.trim();
    let normalized_todos = todos
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let mut parts = Vec::<String>::new();
    if !normalized_status.is_empty() {
        parts.push(normalized_status.to_string());
    }
    if !normalized_todos.is_empty() {
        let joined = normalized_todos.join("；");
        if normalized_status.is_empty() {
            parts.push(joined);
        } else {
            parts.push(format!("待办：{}", joined));
        }
    }
    parts.join("\n")
}

fn task_legacy_title_from_goal(goal: &str) -> String {
    goal.trim().to_string()
}

fn task_legacy_goal_from_goal(goal: &str) -> String {
    goal.trim().to_string()
}

fn task_legacy_cause_from_why(why: &str) -> String {
    why.trim().to_string()
}

fn task_legacy_flow_from_why(_why: &str) -> String {
    String::new()
}

fn task_legacy_todos_from_todo(todo: &str) -> Vec<String> {
    let normalized = todo.trim();
    if normalized.is_empty() {
        return Vec::new();
    }
    vec![normalized.to_string()]
}

fn task_legacy_status_summary_from_todo(todo: &str) -> String {
    todo.trim().to_string()
}

fn task_target_scope_normalized(value: &str) -> &'static str {
    match value.trim() {
        TASK_TARGET_SCOPE_CONTACT => TASK_TARGET_SCOPE_CONTACT,
        _ => TASK_TARGET_SCOPE_DESKTOP,
    }
}

fn task_entry_view_from_stored(record: &TaskRecordStored) -> TaskEntry {
    TaskEntry {
        task_id: record.task_id.clone(),
        conversation_id: record.conversation_id.clone(),
        order_index: record.order_index,
        goal: task_goal_from_legacy_fields(&record.title, &record.goal),
        why: task_why_from_legacy_record(record),
        todo: task_todo_from_legacy_fields(&record.status_summary, &record.todos),
        completion_state: record.completion_state.clone(),
        completion_conclusion: record.completion_conclusion.clone(),
        progress_notes: record
            .progress_notes
            .iter()
            .map(task_progress_note_view_from_stored)
            .collect(),
        trigger: task_trigger_view_from_stored(&record.trigger),
        created_at_local: format_utc_storage_time_to_local_rfc3339(&record.created_at_utc),
        updated_at_local: format_utc_storage_time_to_local_rfc3339(&record.updated_at_utc),
        last_triggered_at_local: record
            .last_triggered_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
        completed_at_local: record
            .completed_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
    }
}

fn task_run_log_view_from_stored(record: &TaskRunLogStored) -> TaskRunLogEntry {
    TaskRunLogEntry {
        id: record.id,
        task_id: record.task_id.clone(),
        triggered_at_local: format_utc_storage_time_to_local_rfc3339(&record.triggered_at_utc),
        outcome: record.outcome.clone(),
        note: record.note.clone(),
    }
}
