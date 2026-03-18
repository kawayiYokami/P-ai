
// ========== constants ==========
const MEMORY_DB_FILE_NAME: &str = "memory_store.db";
const KB_STATE_ACTIVE_INDEX_PROVIDER_ID: &str = "active_index_provider_id";
const KB_STATE_EMBEDDING_API_CONFIG_ID: &str = "embedding_api_config_id";
const KB_STATE_RERANK_API_CONFIG_ID: &str = "rerank_api_config_id";
const KB_STATE_REBUILD_STATUS: &str = "rebuild_status";
const KB_STATE_REBUILD_TRACE_ID: &str = "rebuild_trace_id";
const KB_STATE_REBUILD_DONE_BATCHES: &str = "rebuild_done_batches";
const KB_STATE_REBUILD_TOTAL_BATCHES: &str = "rebuild_total_batches";
const KB_STATE_REBUILD_ERROR: &str = "rebuild_error";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreImportStats {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreProviderSyncReport {
    status: String,
    old_provider_id: Option<String>,
    new_provider_id: String,
    deleted: usize,
    added: usize,
    batch_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreRebuildReport {
    memory_rows: usize,
    memory_fts_rows: usize,
    note_rows: usize,
    note_fts_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreHealthReport {
    status: String,
    memory_rows: usize,
    memory_fts_rows: usize,
    note_rows: usize,
    note_fts_rows: usize,
    orphan_memory_tag_rows: usize,
    orphan_note_tag_rows: usize,
    repaired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryStoreBackupResult {
    path: String,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct MemoryDraftInput {
    memory_type: String,
    judgment: String,
    reasoning: String,
    tags: Vec<String>,
    owner_agent_id: Option<String>,
}


