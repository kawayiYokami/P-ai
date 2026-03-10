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
struct TaskTriggerInput {
    #[serde(default)]
    run_at: Option<String>,
    #[serde(default)]
    every_minutes: Option<u32>,
    #[serde(default)]
    end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskTrigger {
    #[serde(default)]
    run_at: Option<String>,
    #[serde(default)]
    every_minutes: Option<u32>,
    #[serde(default)]
    end_at: Option<String>,
    #[serde(default)]
    next_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskProgressNote {
    at: String,
    note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskEntry {
    task_id: String,
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
    progress_notes: Vec<TaskProgressNote>,
    #[serde(default)]
    stage_key: String,
    #[serde(default)]
    stage_updated_at: Option<String>,
    trigger: TaskTrigger,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    last_triggered_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
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
    triggered_at: String,
    outcome: String,
    note: String,
}

#[derive(Debug, Clone)]
struct TaskDispatchQueueItem {
    task_id: String,
    queued_at: String,
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
    cause: String,
    #[serde(default)]
    goal: String,
    #[serde(default)]
    flow: String,
    #[serde(default)]
    todos: Vec<String>,
    #[serde(default)]
    status_summary: String,
    trigger: TaskTriggerInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdateInput {
    task_id: String,
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
    trigger: Option<TaskTriggerInput>,
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
