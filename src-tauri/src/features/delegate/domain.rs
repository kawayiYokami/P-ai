const DELEGATE_DB_FILE_NAME: &str = "delegate_store.db";
const DELEGATE_STATUS_RUNNING: &str = "running";
const DELEGATE_STATUS_DELIVERED: &str = "delivered";
const DELEGATE_STATUS_COMPLETED: &str = "completed";
const DELEGATE_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DelegateEntry {
    delegate_id: String,
    kind: String,
    conversation_id: String,
    #[serde(default)]
    parent_delegate_id: Option<String>,
    source_agent_id: String,
    target_agent_id: String,
    title: String,
    #[serde(default)]
    why: String,
    #[serde(default)]
    goal: String,
    #[serde(default)]
    todo: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
    status: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    delivered_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

#[derive(Debug, Clone)]
struct DelegateCreateInput {
    kind: String,
    conversation_id: String,
    parent_delegate_id: Option<String>,
    source_agent_id: String,
    target_agent_id: String,
    title: String,
    why: String,
    goal: String,
    todo: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
}
