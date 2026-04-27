const ACTIVE_PLAN_STATUS_IN_PROGRESS: &str = "in_progress";
const ACTIVE_PLAN_STATUS_COMPLETED: &str = "completed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivePlanRecord {
    plan_id: String,
    source_message_id: String,
    status: String,
    context: String,
    created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completion_text: Option<String>,
}

fn encode_active_plan_record(record: &ActivePlanRecord) -> Result<String, String> {
    serde_json::to_string(record)
        .map(|value| format!("{value}\n"))
        .map_err(|err| format!("序列化执行中计划失败: {err}"))
}

fn read_active_plan_records(path: &PathBuf) -> Result<Vec<ActivePlanRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|err| {
        format!(
            "读取执行中计划文件失败，path={}，error={err}",
            path.display()
        )
    })?;
    let mut records = Vec::<ActivePlanRecord>::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<ActivePlanRecord>(trimmed).map_err(|err| {
            format!(
                "解析执行中计划失败，path={}，line={}，error={err}",
                path.display(),
                index + 1
            )
        })?;
        records.push(record);
    }
    Ok(records)
}

fn write_active_plan_records(path: &PathBuf, records: &[ActivePlanRecord]) -> Result<(), String> {
    let mut content = String::new();
    for record in records {
        content.push_str(&encode_active_plan_record(record)?);
    }
    write_message_store_text_atomic(path, "jsonl.tmp", &content, "执行中计划")
}

fn append_active_plan_record(path: &PathBuf, record: &ActivePlanRecord) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "创建执行中计划目录失败，path={}，error={err}",
                parent.display()
            )
        })?;
    }
    let line = encode_active_plan_record(record)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            format!(
                "打开执行中计划文件失败，path={}，error={err}",
                path.display()
            )
        })?;
    use std::io::Write as _;
    file.write_all(line.as_bytes()).map_err(|err| {
        format!(
            "追加执行中计划失败，path={}，error={err}",
            path.display()
        )
    })
}

fn active_plan_records_in_progress(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Vec<ActivePlanRecord>, String> {
    let paths = message_store_paths(data_path, conversation_id)?;
    Ok(read_active_plan_records(&paths.active_plans_file)?
        .into_iter()
        .filter(|record| record.status.trim() == ACTIVE_PLAN_STATUS_IN_PROGRESS)
        .collect())
}

pub(super) fn active_plan_append_in_progress(
    data_path: &PathBuf,
    conversation_id: &str,
    source_message_id: &str,
    context: &str,
) -> Result<(), String> {
    let paths = message_store_paths(data_path, conversation_id)?;
    let record = ActivePlanRecord {
        plan_id: Uuid::new_v4().to_string(),
        source_message_id: source_message_id.trim().to_string(),
        status: ACTIVE_PLAN_STATUS_IN_PROGRESS.to_string(),
        context: context.trim().to_string(),
        created_at: now_iso(),
        completed_at: None,
        completion_text: None,
    };
    if record.source_message_id.is_empty() {
        return Err("sourceMessageId 为空，无法写入执行中计划。".to_string());
    }
    if record.context.is_empty() {
        return Err("计划内容为空，无法写入执行中计划。".to_string());
    }
    append_active_plan_record(&paths.active_plans_file, &record)?;
    Ok(())
}

pub(super) fn active_plan_complete_latest_in_progress(
    data_path: &PathBuf,
    conversation_id: &str,
    completion_text: Option<&str>,
) -> Result<bool, String> {
    let paths = message_store_paths(data_path, conversation_id)?;
    let mut records = read_active_plan_records(&paths.active_plans_file)?;
    let Some(index) = records
        .iter()
        .rposition(|record| record.status.trim() == ACTIVE_PLAN_STATUS_IN_PROGRESS)
    else {
        return Ok(false);
    };
    records[index].status = ACTIVE_PLAN_STATUS_COMPLETED.to_string();
    records[index].completed_at = Some(now_iso());
    records[index].completion_text = completion_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    write_active_plan_records(&paths.active_plans_file, &records)?;
    Ok(true)
}

pub(super) fn active_plan_prompt_block(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Option<String>, String> {
    let records = active_plan_records_in_progress(data_path, conversation_id)?;
    if records.is_empty() {
        return Ok(None);
    }
    let mut lines = Vec::<String>::new();
    lines.push("<active_plans>".to_string());
    lines.push("以下为用户已同意且正在执行的计划。它们必须持续纳入上下文；完成后调用 plan(action=complete) 结束最新进行中计划。".to_string());
    for (index, record) in records.iter().enumerate() {
        lines.push(format!("<active_plan index=\"{}\">", index + 1));
        lines.push(record.context.trim().to_string());
        lines.push("</active_plan>".to_string());
    }
    lines.push("</active_plans>".to_string());
    Ok(Some(lines.join("\n")))
}