const TASK_DB_FILE_NAME: &str = "task_store.db";
const TASK_RUNTIME_CURRENT_TRACKED_KEY: &str = "current_tracked_task_id";
const TASK_STATE_ACTIVE: &str = "active";
const TASK_STATE_COMPLETED: &str = "completed";
const TASK_STATE_FAILED_COMPLETED: &str = "failed_completed";
const TASK_IMMEDIATE_RETRY_SECONDS: i64 = 60;
const TASK_SCHEDULER_INTERVAL_SECONDS: u64 = 30;
const TASK_MAX_BOARD_ITEMS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskTriggerInputLocal {
    #[serde(default, alias = "runAt", alias = "run_at")]
    run_at_local: Option<String>,
    #[serde(default)]
    every_minutes: Option<u32>,
    #[serde(default, alias = "endAt", alias = "end_at")]
    end_at_local: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskTriggerView {
    #[serde(default)]
    run_at_local: Option<String>,
    #[serde(default)]
    every_minutes: Option<u32>,
    #[serde(default)]
    end_at_local: Option<String>,
    #[serde(default)]
    next_run_at_local: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskTriggerStored {
    run_at_utc: Option<String>,
    every_minutes: Option<u32>,
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
    title: String,
    cause: String,
    goal: String,
    flow: String,
    todos: Vec<String>,
    status_summary: String,
    completion_state: String,
    #[serde(default)]
    completion_conclusion: String,
    progress_notes: Vec<TaskProgressNoteView>,
    #[serde(default)]
    stage_key: String,
    #[serde(default)]
    stage_updated_at_local: Option<String>,
    trigger: TaskTriggerView,
    created_at_local: String,
    updated_at_local: String,
    #[serde(default)]
    last_triggered_at_local: Option<String>,
    #[serde(default)]
    completed_at_local: Option<String>,
    current_tracked: bool,
}

#[derive(Debug, Clone)]
struct TaskRecordStored {
    task_id: String,
    conversation_id: Option<String>,
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
    current_tracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskBoardSnapshot {
    #[serde(default)]
    current_tracked_task_id: Option<String>,
    #[serde(default)]
    tracked_task: Option<TaskEntry>,
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

#[derive(Debug, Clone)]
struct TaskDispatchQueueItem {
    task_id: String,
    queued_at_local: String,
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
    title: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    cause: String,
    #[serde(default)]
    goal: String,
    #[serde(default)]
    flow: String,
    #[serde(default)]
    todos: Vec<String>,
    #[serde(default)]
    status_summary: String,
    trigger: TaskTriggerInputLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdateInput {
    task_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cause: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    flow: Option<String>,
    #[serde(default)]
    todos: Option<Vec<String>>,
    #[serde(default)]
    status_summary: Option<String>,
    #[serde(default)]
    stage_key: Option<String>,
    #[serde(default)]
    append_note: Option<String>,
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
    #[serde(default)]
    status_summary: String,
    #[serde(default)]
    append_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskGetInput {
    task_id: String,
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

fn task_entry_view_from_stored(record: &TaskRecordStored) -> TaskEntry {
    TaskEntry {
        task_id: record.task_id.clone(),
        conversation_id: record.conversation_id.clone(),
        order_index: record.order_index,
        title: record.title.clone(),
        cause: record.cause.clone(),
        goal: record.goal.clone(),
        flow: record.flow.clone(),
        todos: record.todos.clone(),
        status_summary: record.status_summary.clone(),
        completion_state: record.completion_state.clone(),
        completion_conclusion: record.completion_conclusion.clone(),
        progress_notes: record
            .progress_notes
            .iter()
            .map(task_progress_note_view_from_stored)
            .collect(),
        stage_key: record.stage_key.clone(),
        stage_updated_at_local: record
            .stage_updated_at_utc
            .as_deref()
            .map(format_utc_storage_time_to_local_rfc3339),
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
        current_tracked: record.current_tracked,
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
