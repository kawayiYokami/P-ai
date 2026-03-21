const APPLY_PATCH_APPROVAL_TIMEOUT_REASON: &str = "apply_patch_approval_timeout";

#[derive(Debug, Clone)]
enum ApplyPatchSafetyCheck {
    AutoApprove,
    AskUser { existing_paths: Vec<PathBuf> },
    Reject { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPatchToolArgs {
    input: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ApplyPatchHunk {
    lines: Vec<ApplyPatchLine>,
    end_of_file: bool,
}

#[derive(Debug, Clone)]
struct ApplyPatchUpdate {
    from: String,
    to: Option<String>,
    hunks: Vec<ApplyPatchHunk>,
}

#[derive(Debug, Clone)]
enum ApplyPatchOp {
    Add { path: String, lines: Vec<String> },
    Delete { path: String },
    Update(ApplyPatchUpdate),
}

#[derive(Debug, Clone)]
enum ApplyPatchLine {
    Context(String),
    Remove(String),
    Add(String),
}

#[derive(Debug, Clone)]
enum ApplyPatchResolvedOp {
    Add {
        path: PathBuf,
        content: String,
    },
    Delete {
        path: PathBuf,
    },
    Update {
        from: PathBuf,
        to: Option<PathBuf>,
        hunks: Vec<ApplyPatchHunk>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ApplyPatchBackupKind {
    Add,
    Delete,
    Update,
    MoveUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPatchBackupEntry {
    kind: ApplyPatchBackupKind,
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected_current_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    backup_blob_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPatchBackupRecord {
    record_id: String,
    session_id: String,
    cwd: String,
    fingerprint: String,
    created_at: String,
    entries: Vec<ApplyPatchBackupEntry>,
}

fn apply_patch_tool_description() -> String {
    "Apply structured patch to files in current workspace root.".to_string()
}

fn apply_patch_temp_root(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path).join("temp").join("apply_patch")
}

fn apply_patch_temp_records_dir(data_path: &PathBuf) -> PathBuf {
    apply_patch_temp_root(data_path).join("records")
}

fn apply_patch_temp_blobs_dir(data_path: &PathBuf) -> PathBuf {
    apply_patch_temp_root(data_path).join("blobs")
}

fn apply_patch_fingerprint(session_id: &str, cwd: &Path, input: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, session_id.as_bytes());
    sha2::Digest::update(&mut hasher, b"\n");
    sha2::Digest::update(&mut hasher, cwd.to_string_lossy().as_bytes());
    sha2::Digest::update(&mut hasher, b"\n");
    sha2::Digest::update(&mut hasher, input.as_bytes());
    sha2::Digest::finalize(hasher)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn apply_patch_record_path(data_path: &PathBuf, record_id: &str) -> PathBuf {
    apply_patch_temp_records_dir(data_path).join(format!("{record_id}.json"))
}

fn apply_patch_blob_path(data_path: &PathBuf, blob_file: &str) -> PathBuf {
    apply_patch_temp_blobs_dir(data_path).join(blob_file)
}

fn apply_patch_prepare_backup_record(
    data_path: &PathBuf,
    session_id: &str,
    cwd: &Path,
    input: &str,
    ops: &[ApplyPatchResolvedOp],
) -> Result<ApplyPatchBackupRecord, String> {
    std::fs::create_dir_all(apply_patch_temp_records_dir(data_path))
        .map_err(|err| format!("创建 apply_patch 记录目录失败：{err}"))?;
    std::fs::create_dir_all(apply_patch_temp_blobs_dir(data_path))
        .map_err(|err| format!("创建 apply_patch 备份目录失败：{err}"))?;

    let record_id = Uuid::new_v4().to_string();
    let mut entries = Vec::<ApplyPatchBackupEntry>::new();
    for op in ops {
        match op {
            ApplyPatchResolvedOp::Add { path, content } => {
                entries.push(ApplyPatchBackupEntry {
                    kind: ApplyPatchBackupKind::Add,
                    path: path.to_string_lossy().to_string(),
                    from_path: None,
                    to_path: None,
                    expected_current_content: Some(content.clone()),
                    backup_blob_file: None,
                });
            }
            ApplyPatchResolvedOp::Delete { path } => {
                let raw = std::fs::read(path).map_err(|_| {
                    format!("Delete File 失败，文件不存在：{}", path.to_string_lossy())
                })?;
                let metadata = std::fs::metadata(path).map_err(|_| {
                    format!("Delete File 失败，文件不存在：{}", path.to_string_lossy())
                })?;
                if !metadata.is_file() {
                    return Err(format!("Delete File 失败，目标不是文件：{}", path.to_string_lossy()));
                }
                let blob_file = format!("{}.bin", Uuid::new_v4());
                std::fs::write(apply_patch_blob_path(data_path, &blob_file), raw)
                    .map_err(|err| format!("写入删除备份失败（{}）：{err}", path.to_string_lossy()))?;
                entries.push(ApplyPatchBackupEntry {
                    kind: ApplyPatchBackupKind::Delete,
                    path: path.to_string_lossy().to_string(),
                    from_path: None,
                    to_path: None,
                    expected_current_content: None,
                    backup_blob_file: Some(blob_file),
                });
            }
            ApplyPatchResolvedOp::Update { from, to, hunks } => {
                let raw = std::fs::read(from).map_err(|_| {
                    format!("Update File 失败，文件不存在：{}", from.to_string_lossy())
                })?;
                let old_content = String::from_utf8(raw.clone()).map_err(|_| {
                    format!("Update File 失败，文件不是 UTF-8 文本：{}", from.to_string_lossy())
                })?;
                let new_content = apply_patch_apply_hunks(&old_content, hunks)?;
                let blob_file = format!("{}.bin", Uuid::new_v4());
                std::fs::write(apply_patch_blob_path(data_path, &blob_file), raw)
                    .map_err(|err| format!("写入修改备份失败（{}）：{err}", from.to_string_lossy()))?;
                entries.push(ApplyPatchBackupEntry {
                    kind: if to.is_some() {
                        ApplyPatchBackupKind::MoveUpdate
                    } else {
                        ApplyPatchBackupKind::Update
                    },
                    path: to.as_ref().unwrap_or(from).to_string_lossy().to_string(),
                    from_path: to.as_ref().map(|_| from.to_string_lossy().to_string()),
                    to_path: to.as_ref().map(|dest| dest.to_string_lossy().to_string()),
                    expected_current_content: Some(new_content),
                    backup_blob_file: Some(blob_file),
                });
            }
        }
    }

    Ok(ApplyPatchBackupRecord {
        record_id,
        session_id: session_id.to_string(),
        cwd: cwd.to_string_lossy().to_string(),
        fingerprint: apply_patch_fingerprint(session_id, cwd, input),
        created_at: now_iso(),
        entries,
    })
}

fn apply_patch_store_backup_record(
    state: &AppState,
    record: &ApplyPatchBackupRecord,
) -> Result<PathBuf, String> {
    let path = apply_patch_record_path(&state.data_path, &record.record_id);
    let body = serde_json::to_vec_pretty(record)
        .map_err(|err| format!("序列化 apply_patch 恢复记录失败：{err}"))?;
    std::fs::write(&path, body).map_err(|err| {
        format!("写入 apply_patch 恢复记录失败（{}）：{err}", path.to_string_lossy())
    })?;
    Ok(path)
}

fn apply_patch_cleanup_backup_record_by_value(
    data_path: &PathBuf,
    record: &ApplyPatchBackupRecord,
) -> Result<(), String> {
    for entry in &record.entries {
        if let Some(blob_file) = entry.backup_blob_file.as_deref() {
            let blob_path = apply_patch_blob_path(data_path, blob_file);
            if blob_path.exists() {
                std::fs::remove_file(&blob_path).map_err(|err| {
                    format!("清理 apply_patch 备份文件失败（{}）：{err}", blob_path.to_string_lossy())
                })?;
            }
        }
    }
    let record_path = apply_patch_record_path(data_path, &record.record_id);
    if record_path.exists() {
        std::fs::remove_file(&record_path).map_err(|err| {
            format!("清理 apply_patch 恢复记录失败（{}）：{err}", record_path.to_string_lossy())
        })?;
    }
    Ok(())
}

fn apply_patch_read_backup_record(path: &Path) -> Result<ApplyPatchBackupRecord, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("读取 apply_patch 恢复记录失败（{}）：{err}", path.to_string_lossy()))?;
    serde_json::from_str::<ApplyPatchBackupRecord>(&raw)
        .map_err(|err| format!("解析 apply_patch 恢复记录失败（{}）：{err}", path.to_string_lossy()))
}

fn apply_patch_take_latest_backup_record(
    data_path: &PathBuf,
    fingerprint: &str,
) -> Result<Option<(PathBuf, ApplyPatchBackupRecord)>, String> {
    let dir = apply_patch_temp_records_dir(data_path);
    if !dir.exists() {
        return Ok(None);
    }
    let mut matches = Vec::<(PathBuf, ApplyPatchBackupRecord)>::new();
    let entries = std::fs::read_dir(&dir)
        .map_err(|err| format!("读取 apply_patch 记录目录失败（{}）：{err}", dir.to_string_lossy()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let record = apply_patch_read_backup_record(&path)?;
        if record.fingerprint == fingerprint {
            matches.push((path, record));
        }
    }
    matches.sort_by(|left, right| {
        left.1
            .created_at
            .cmp(&right.1.created_at)
            .then_with(|| left.1.record_id.cmp(&right.1.record_id))
    });
    Ok(matches.pop())
}

fn apply_patch_read_utf8_file(path: &Path) -> Result<String, String> {
    let raw = std::fs::read(path)
        .map_err(|err| format!("读取文件失败（{}）：{err}", terminal_path_for_user(path)))?;
    String::from_utf8(raw)
        .map_err(|_| format!("文件不是 UTF-8 文本：{}", terminal_path_for_user(path)))
}

fn apply_patch_write_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("创建目录失败（{}）：{err}", terminal_path_for_user(parent)))?;
    }
    Ok(())
}

fn apply_patch_restore_backup_record(
    data_path: &PathBuf,
    record: &ApplyPatchBackupRecord,
) -> Result<usize, String> {
    let mut restored = 0usize;
    for entry in record.entries.iter().rev() {
        match entry.kind {
            ApplyPatchBackupKind::Add => {
                let path = PathBuf::from(&entry.path);
                if !path.exists() {
                    return Err(format!("撤回失败：目标文件不存在 {}", terminal_path_for_user(&path)));
                }
                let current = apply_patch_read_utf8_file(&path)?;
                let expected = entry.expected_current_content.as_deref().unwrap_or_default();
                if current != expected {
                    return Err(format!(
                        "撤回失败：文件已变更，无法安全删除 {}",
                        terminal_path_for_user(&path)
                    ));
                }
                std::fs::remove_file(&path).map_err(|err| {
                    format!("撤回失败：删除文件失败 {}: {err}", terminal_path_for_user(&path))
                })?;
                restored = restored.saturating_add(1);
            }
            ApplyPatchBackupKind::Delete => {
                let path = PathBuf::from(&entry.path);
                if path.exists() {
                    return Err(format!(
                        "撤回失败：删除前文件恢复目标已存在 {}",
                        terminal_path_for_user(&path)
                    ));
                }
                let blob_file = entry.backup_blob_file.as_deref().ok_or_else(|| {
                    format!("撤回失败：删除备份缺少 blob 记录 {}", terminal_path_for_user(&path))
                })?;
                let raw = std::fs::read(apply_patch_blob_path(data_path, blob_file)).map_err(|err| {
                    format!("撤回失败：读取删除备份失败 {}: {err}", terminal_path_for_user(&path))
                })?;
                apply_patch_write_parent_dir(&path)?;
                std::fs::write(&path, raw).map_err(|err| {
                    format!("撤回失败：恢复文件失败 {}: {err}", terminal_path_for_user(&path))
                })?;
                restored = restored.saturating_add(1);
            }
            ApplyPatchBackupKind::Update => {
                let path = PathBuf::from(&entry.path);
                if !path.exists() {
                    return Err(format!("撤回失败：目标文件不存在 {}", terminal_path_for_user(&path)));
                }
                let current = apply_patch_read_utf8_file(&path)?;
                let expected = entry.expected_current_content.as_deref().unwrap_or_default();
                if current != expected {
                    return Err(format!(
                        "撤回失败：文件已变更，无法安全恢复 {}",
                        terminal_path_for_user(&path)
                    ));
                }
                let blob_file = entry.backup_blob_file.as_deref().ok_or_else(|| {
                    format!("撤回失败：修改备份缺少 blob 记录 {}", terminal_path_for_user(&path))
                })?;
                let raw = std::fs::read(apply_patch_blob_path(data_path, blob_file)).map_err(|err| {
                    format!("撤回失败：读取修改备份失败 {}: {err}", terminal_path_for_user(&path))
                })?;
                std::fs::write(&path, raw).map_err(|err| {
                    format!("撤回失败：恢复文件失败 {}: {err}", terminal_path_for_user(&path))
                })?;
                restored = restored.saturating_add(1);
            }
            ApplyPatchBackupKind::MoveUpdate => {
                let from_path = PathBuf::from(entry.from_path.as_deref().unwrap_or_default());
                let to_path = PathBuf::from(entry.to_path.as_deref().unwrap_or_default());
                if !to_path.exists() {
                    return Err(format!(
                        "撤回失败：移动后的目标文件不存在 {}",
                        terminal_path_for_user(&to_path)
                    ));
                }
                if from_path.exists()
                    && terminal_normalize_for_access_check(&from_path)
                        != terminal_normalize_for_access_check(&to_path)
                {
                    return Err(format!(
                        "撤回失败：原始路径已存在，无法安全恢复 {}",
                        terminal_path_for_user(&from_path)
                    ));
                }
                let current = apply_patch_read_utf8_file(&to_path)?;
                let expected = entry.expected_current_content.as_deref().unwrap_or_default();
                if current != expected {
                    return Err(format!(
                        "撤回失败：文件已变更，无法安全恢复 {}",
                        terminal_path_for_user(&to_path)
                    ));
                }
                let blob_file = entry.backup_blob_file.as_deref().ok_or_else(|| {
                    format!("撤回失败：移动备份缺少 blob 记录 {}", terminal_path_for_user(&to_path))
                })?;
                let raw = std::fs::read(apply_patch_blob_path(data_path, blob_file)).map_err(|err| {
                    format!("撤回失败：读取移动备份失败 {}: {err}", terminal_path_for_user(&to_path))
                })?;
                apply_patch_write_parent_dir(&from_path)?;
                std::fs::write(&from_path, raw).map_err(|err| {
                    format!("撤回失败：恢复原始文件失败 {}: {err}", terminal_path_for_user(&from_path))
                })?;
                std::fs::remove_file(&to_path).map_err(|err| {
                    format!("撤回失败：删除移动后的文件失败 {}: {err}", terminal_path_for_user(&to_path))
                })?;
                restored = restored.saturating_add(1);
            }
        }
    }
    Ok(restored)
}

fn clear_apply_patch_temp(data_path: &PathBuf) -> Result<(usize, usize), String> {
    let records_dir = apply_patch_temp_records_dir(data_path);
    let blobs_dir = apply_patch_temp_blobs_dir(data_path);
    let mut removed_records = 0usize;
    let mut removed_blobs = 0usize;
    for dir in [&records_dir, &blobs_dir] {
        std::fs::create_dir_all(dir)
            .map_err(|err| format!("创建 apply_patch temp 目录失败（{}）：{err}", dir.to_string_lossy()))?;
    }
    if let Ok(entries) = std::fs::read_dir(&records_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|err| {
                    format!("清理 apply_patch 记录失败（{}）：{err}", path.to_string_lossy())
                })?;
                removed_records = removed_records.saturating_add(1);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(&blobs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|err| {
                    format!("清理 apply_patch 备份失败（{}）：{err}", path.to_string_lossy())
                })?;
                removed_blobs = removed_blobs.saturating_add(1);
            }
        }
    }
    Ok((removed_records, removed_blobs))
}

fn apply_patch_split_lines(input: &str) -> Vec<String> {
    input
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

fn apply_patch_parse(input: &str) -> Result<Vec<ApplyPatchOp>, String> {
    let lines = apply_patch_split_lines(input);
    if lines.is_empty() {
        return Err("补丁为空。".to_string());
    }
    if lines.first().map(|v| v.trim()) != Some("*** Begin Patch") {
        return Err("补丁必须以 `*** Begin Patch` 开始。".to_string());
    }
    let mut idx = 1usize;
    let mut ops = Vec::<ApplyPatchOp>::new();
    while idx < lines.len() {
        let line = lines[idx].trim_end();
        if line == "*** End Patch" {
            if idx + 1 != lines.len() && !lines[idx + 1..].iter().all(|v| v.trim().is_empty()) {
                return Err("`*** End Patch` 后不允许有额外内容。".to_string());
            }
            return Ok(ops);
        }
        if line.is_empty() {
            idx += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            let path = path.trim().to_string();
            idx += 1;
            let mut add_lines = Vec::<String>::new();
            while idx < lines.len() {
                let current = lines[idx].as_str();
                if current.starts_with("*** ") {
                    break;
                }
                let Some(payload) = current.strip_prefix('+') else {
                    return Err(format!("Add File 仅允许 `+` 行，第 {} 行非法。", idx + 1));
                };
                add_lines.push(payload.to_string());
                idx += 1;
            }
            if add_lines.is_empty() {
                return Err(format!("Add File `{path}` 至少需要一行 `+` 内容。"));
            }
            ops.push(ApplyPatchOp::Add {
                path,
                lines: add_lines,
            });
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            ops.push(ApplyPatchOp::Delete {
                path: path.trim().to_string(),
            });
            idx += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            let from = path.trim().to_string();
            idx += 1;
            let mut to = None::<String>;
            if idx < lines.len() && lines[idx].starts_with("*** Move to: ") {
                to = lines[idx]
                    .strip_prefix("*** Move to: ")
                    .map(|v| v.trim().to_string());
                idx += 1;
            }
            let mut hunks = Vec::<ApplyPatchHunk>::new();
            while idx < lines.len() {
                let current = lines[idx].as_str();
                if current == "*** End Patch"
                    || current.starts_with("*** Add File: ")
                    || current.starts_with("*** Delete File: ")
                    || current.starts_with("*** Update File: ")
                {
                    break;
                }
                if !current.starts_with("@@") {
                    return Err(format!(
                        "Update File `{from}` hunk 头非法，第 {} 行：`{}`",
                        idx + 1,
                        current
                    ));
                }
                idx += 1;
                let mut hunk_lines = Vec::<ApplyPatchLine>::new();
                let mut end_of_file = false;
                while idx < lines.len() {
                    let hunk_line = lines[idx].as_str();
                    if hunk_line == "*** End of File" {
                        end_of_file = true;
                        idx += 1;
                        break;
                    }
                    if hunk_line.starts_with("@@")
                        || hunk_line == "*** End Patch"
                        || hunk_line.starts_with("*** Add File: ")
                        || hunk_line.starts_with("*** Delete File: ")
                        || hunk_line.starts_with("*** Update File: ")
                    {
                        break;
                    }
                    let mut chars = hunk_line.chars();
                    let Some(prefix) = chars.next() else {
                        return Err(format!("空 hunk 行非法（第 {} 行）。", idx + 1));
                    };
                    let payload = chars.collect::<String>();
                    match prefix {
                        ' ' => hunk_lines.push(ApplyPatchLine::Context(payload)),
                        '-' => hunk_lines.push(ApplyPatchLine::Remove(payload)),
                        '+' => hunk_lines.push(ApplyPatchLine::Add(payload)),
                        _ => {
                            return Err(format!(
                                "hunk 行前缀必须是空格/+/-，第 {} 行：`{}`",
                                idx + 1,
                                hunk_line
                            ));
                        }
                    }
                    idx += 1;
                }
                if hunk_lines.is_empty() {
                    return Err(format!("Update File `{from}` 存在空 hunk。"));
                }
                hunks.push(ApplyPatchHunk {
                    lines: hunk_lines,
                    end_of_file,
                });
            }
            ops.push(ApplyPatchOp::Update(ApplyPatchUpdate { from, to, hunks }));
            continue;
        }
        return Err(format!("未知补丁头：`{line}`"));
    }
    Err("补丁缺少 `*** End Patch`。".to_string())
}

#[cfg(target_os = "windows")]
fn apply_patch_has_windows_drive_prefix(path: &str) -> bool {
    terminal_has_windows_drive_prefix(path)
}

#[cfg(not(target_os = "windows"))]
fn apply_patch_has_windows_drive_prefix(_path: &str) -> bool {
    false
}

fn apply_patch_resolve_path(base: &Path, raw: &str) -> Result<PathBuf, String> {
    let normalized = normalize_terminal_path_input_for_current_platform(raw.trim());
    if normalized.is_empty() {
        return Err("补丁路径为空。".to_string());
    }
    if PathBuf::from(&normalized).is_absolute() || apply_patch_has_windows_drive_prefix(&normalized)
    {
        return Err(format!("补丁路径必须是相对路径：`{raw}`"));
    }
    let joined = base.join(&normalized);
    let safe = terminal_normalize_for_access_check(&joined);
    if !path_is_within(base, &safe) {
        return Err(format!("补丁路径越界：`{raw}`"));
    }
    Ok(safe)
}

fn apply_patch_resolve_ops(base: &Path, ops: Vec<ApplyPatchOp>) -> Result<Vec<ApplyPatchResolvedOp>, String> {
    let mut out = Vec::<ApplyPatchResolvedOp>::new();
    for op in ops {
        match op {
            ApplyPatchOp::Add { path, lines } => out.push(ApplyPatchResolvedOp::Add {
                path: apply_patch_resolve_path(base, &path)?,
                content: lines.join("\n"),
            }),
            ApplyPatchOp::Delete { path } => out.push(ApplyPatchResolvedOp::Delete {
                path: apply_patch_resolve_path(base, &path)?,
            }),
            ApplyPatchOp::Update(update) => {
                let from = apply_patch_resolve_path(base, &update.from)?;
                let to = match update.to {
                    Some(raw) => Some(apply_patch_resolve_path(base, &raw)?),
                    None => None,
                };
                out.push(ApplyPatchResolvedOp::Update {
                    from,
                    to,
                    hunks: update.hunks,
                });
            }
        }
    }
    Ok(out)
}

fn apply_patch_collect_existing_paths(ops: &[ApplyPatchResolvedOp]) -> Vec<PathBuf> {
    let mut out = Vec::<PathBuf>::new();
    for op in ops {
        match op {
            ApplyPatchResolvedOp::Add { .. } => {}
            ApplyPatchResolvedOp::Delete { path } => out.push(path.clone()),
            ApplyPatchResolvedOp::Update { from, to, .. } => {
                out.push(from.clone());
                if let Some(next) = to {
                    out.push(next.clone());
                }
            }
        }
    }
    terminal_dedup_paths(out)
}

fn apply_patch_assess_safety(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
    ops: &[ApplyPatchResolvedOp],
) -> ApplyPatchSafetyCheck {
    if ops.is_empty() {
        return ApplyPatchSafetyCheck::Reject {
            reason: "empty patch".to_string(),
        };
    }
    if !terminal_cwd_in_agent_default_workspace(state, cwd) {
        let existing = apply_patch_collect_existing_paths(ops)
            .into_iter()
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        if !existing.is_empty() {
            return ApplyPatchSafetyCheck::AskUser {
                existing_paths: existing,
            };
        }
    }
    if !terminal_session_has_locked_root(state, session_id)
        && !terminal_cwd_in_agent_default_workspace(state, cwd)
    {
        return ApplyPatchSafetyCheck::AskUser {
            existing_paths: Vec::new(),
        };
    }
    ApplyPatchSafetyCheck::AutoApprove
}

fn apply_patch_split_file_lines(content: &str) -> (Vec<String>, bool) {
    let trailing_newline = content.ends_with('\n');
    let mut lines = content
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();
    if trailing_newline && lines.last().map(|v| v.is_empty()).unwrap_or(false) {
        let _ = lines.pop();
    }
    (lines, trailing_newline)
}

fn apply_patch_find_subsequence(source: &[String], needle: &[String]) -> Option<usize> {
    if needle.is_empty() {
        return Some(source.len());
    }
    source
        .windows(needle.len())
        .position(|window| window.iter().zip(needle).all(|(a, b)| a == b))
}

fn apply_patch_render_lines(lines: &[String], trailing_newline: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

fn apply_patch_apply_hunks(content: &str, hunks: &[ApplyPatchHunk]) -> Result<String, String> {
    let (mut lines, mut trailing_newline) = apply_patch_split_file_lines(content);
    for hunk in hunks {
        let mut old_seq = Vec::<String>::new();
        let mut new_seq = Vec::<String>::new();
        for line in &hunk.lines {
            match line {
                ApplyPatchLine::Context(v) => {
                    old_seq.push(v.clone());
                    new_seq.push(v.clone());
                }
                ApplyPatchLine::Remove(v) => old_seq.push(v.clone()),
                ApplyPatchLine::Add(v) => new_seq.push(v.clone()),
            }
        }
        let Some(start) = apply_patch_find_subsequence(&lines, &old_seq) else {
            return Err("hunk 上下文不匹配，无法应用补丁。".to_string());
        };
        let end = start + old_seq.len();
        lines.splice(start..end, new_seq);
        if hunk.end_of_file {
            trailing_newline = false;
        }
    }
    Ok(apply_patch_render_lines(&lines, trailing_newline))
}

async fn apply_patch_execute_ops(ops: &[ApplyPatchResolvedOp]) -> Result<Vec<Value>, String> {
    let mut changed = Vec::<Value>::new();
    for op in ops {
        match op {
            ApplyPatchResolvedOp::Add { path, content } => {
                if path.exists() {
                    return Err(format!("Add File 失败，文件已存在：{}", path.to_string_lossy()));
                }
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|err| format!("创建目录失败（{}）：{err}", parent.to_string_lossy()))?;
                }
                tokio::fs::write(path, content.as_bytes())
                    .await
                    .map_err(|err| format!("写入文件失败（{}）：{err}", path.to_string_lossy()))?;
                changed.push(serde_json::json!({
                    "op": "add",
                    "path": terminal_path_for_user(path),
                }));
            }
            ApplyPatchResolvedOp::Delete { path } => {
                let metadata = tokio::fs::metadata(path)
                    .await
                    .map_err(|_| format!("Delete File 失败，文件不存在：{}", path.to_string_lossy()))?;
                if !metadata.is_file() {
                    return Err(format!("Delete File 失败，目标不是文件：{}", path.to_string_lossy()));
                }
                tokio::fs::remove_file(path)
                    .await
                    .map_err(|err| format!("删除文件失败（{}）：{err}", path.to_string_lossy()))?;
                changed.push(serde_json::json!({
                    "op": "delete",
                    "path": terminal_path_for_user(path),
                }));
            }
            ApplyPatchResolvedOp::Update { from, to, hunks } => {
                let raw = tokio::fs::read(from)
                    .await
                    .map_err(|_| format!("Update File 失败，文件不存在：{}", from.to_string_lossy()))?;
                let old_content = String::from_utf8(raw)
                    .map_err(|_| format!("Update File 失败，文件不是 UTF-8 文本：{}", from.to_string_lossy()))?;
                let new_content = apply_patch_apply_hunks(&old_content, hunks)?;
                tokio::fs::write(from, new_content.as_bytes())
                    .await
                    .map_err(|err| format!("更新文件失败（{}）：{err}", from.to_string_lossy()))?;
                if let Some(dest) = to {
                    if let Some(parent) = dest.parent() {
                        tokio::fs::create_dir_all(parent)
                            .await
                            .map_err(|err| format!("创建重命名目录失败（{}）：{err}", parent.to_string_lossy()))?;
                    }
                    if dest.exists()
                        && terminal_normalize_for_access_check(dest)
                            != terminal_normalize_for_access_check(from)
                    {
                        return Err(format!("重命名目标已存在：{}", dest.to_string_lossy()));
                    }
                    tokio::fs::rename(from, dest)
                        .await
                        .map_err(|err| format!("重命名失败（{} -> {}）：{err}", from.to_string_lossy(), dest.to_string_lossy()))?;
                    changed.push(serde_json::json!({
                        "op": "update_move",
                        "from": terminal_path_for_user(from),
                        "to": terminal_path_for_user(dest),
                    }));
                } else {
                    changed.push(serde_json::json!({
                        "op": "update",
                        "path": terminal_path_for_user(from),
                    }));
                }
            }
        }
    }
    Ok(changed)
}

fn apply_patch_timeout_blocked_result(
    session_id: &str,
    cwd: &Path,
    existing_paths: &[PathBuf],
) -> Value {
    serde_json::json!({
        "ok": false,
        "approved": false,
        "blockedReason": APPLY_PATCH_APPROVAL_TIMEOUT_REASON,
        "message": "审核超时：用户工具区修改需要本机确认。",
        "sessionId": session_id,
        "cwd": terminal_path_for_user(cwd),
        "existingPaths": existing_paths
            .iter()
            .map(|path| terminal_path_for_user(path))
            .collect::<Vec<_>>(),
    })
}

async fn builtin_apply_patch(
    state: &AppState,
    session_id: &str,
    input: &str,
) -> Result<Value, String> {
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let cwd = resolve_terminal_cwd(state, &normalized_session, None)?;
    let parsed = apply_patch_parse(input)?;
    let resolved = apply_patch_resolve_ops(&cwd, parsed)?;

    let safety = apply_patch_assess_safety(state, &normalized_session, &cwd, &resolved);
    match safety {
        ApplyPatchSafetyCheck::Reject { reason } => {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "rejected",
                "message": reason,
                "sessionId": normalized_session,
                "cwd": terminal_path_for_user(&cwd),
            }));
        }
        ApplyPatchSafetyCheck::AskUser { existing_paths } => {
            let mut lines = vec![
                "该补丁将在用户工具区执行，是否批准本次修改？".to_string(),
                format!("会话: {}", normalized_session),
                format!("工作目录: {}", cwd.to_string_lossy()),
                "命中已有文件：".to_string(),
            ];
            if existing_paths.is_empty() {
                lines.push("- 未识别到已存在文件，但该区域仍需确认。".to_string());
            } else {
                for path in existing_paths.iter().take(8) {
                    lines.push(format!("- {}", path.to_string_lossy()));
                }
            }
            let approved = match terminal_request_user_approval(
                state,
                "补丁执行审批",
                &lines.join("\n"),
                &normalized_session,
                "apply_patch_workspace_write",
                Some(&cwd),
                None,
                None,
                Some("用户工具区修改需要审批"),
                &existing_paths,
            )
            .await
            {
                Ok(v) => v,
                Err(err) if terminal_is_approval_timeout_error(&err) => {
                    return Ok(apply_patch_timeout_blocked_result(
                        &normalized_session,
                        &cwd,
                        &existing_paths,
                    ));
                }
                Err(err) => return Err(err),
            };
            if !approved {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "user_denied_apply_patch",
                    "message": "用户拒绝了本次补丁执行。",
                    "sessionId": normalized_session,
                    "cwd": terminal_path_for_user(&cwd),
                }));
            }
        }
        ApplyPatchSafetyCheck::AutoApprove => {}
    }

    let backup_record = apply_patch_prepare_backup_record(
        &state.data_path,
        &normalized_session,
        &cwd,
        input,
        &resolved,
    )?;
    let started = std::time::Instant::now();
    let changed = match apply_patch_execute_ops(&resolved).await {
        Ok(value) => value,
        Err(err) => {
            let _ = apply_patch_cleanup_backup_record_by_value(&state.data_path, &backup_record);
            return Err(err);
        }
    };
    let record_path = match apply_patch_store_backup_record(state, &backup_record) {
        Ok(path) => path,
        Err(err) => {
            let _ = apply_patch_cleanup_backup_record_by_value(&state.data_path, &backup_record);
            return Err(err);
        }
    };
    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    eprintln!(
        "[补丁执行] 完成 task=apply_patch session={} changed={} elapsed_ms={} record_id={}",
        normalized_session,
        changed.len(),
        elapsed_ms,
        backup_record.record_id
    );
    Ok(serde_json::json!({
        "ok": true,
        "approved": true,
        "sessionId": normalized_session,
        "cwd": terminal_path_for_user(&cwd),
        "changed": changed,
        "changedCount": changed.len(),
        "elapsedMs": elapsed_ms,
        "backupRecordId": backup_record.record_id,
        "backupFingerprint": backup_record.fingerprint,
        "backupRecordPath": terminal_path_for_user(&record_path),
    }))
}

#[cfg(test)]
mod apply_patch_tool_tests {
    use super::*;

    fn make_temp_data_path(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create config dir");
        root.join("config").join("app_data.json")
    }

    #[test]
    fn parse_should_support_add_delete_update_and_move() {
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+hello\n*** Delete File: b.txt\n*** Update File: c.txt\n*** Move to: d.txt\n@@\n-old\n+new\n*** End Patch";
        let ops = apply_patch_parse(patch).expect("parse");
        assert_eq!(ops.len(), 3);
        match &ops[2] {
            ApplyPatchOp::Update(update) => {
                assert_eq!(update.from, "c.txt");
                assert_eq!(update.to.as_deref(), Some("d.txt"));
                assert_eq!(update.hunks.len(), 1);
            }
            _ => panic!("expected update op"),
        }
    }

    #[test]
    fn apply_hunks_should_replace_lines() {
        let content = "a\nb\nc\n";
        let hunks = vec![ApplyPatchHunk {
            lines: vec![
                ApplyPatchLine::Context("a".to_string()),
                ApplyPatchLine::Remove("b".to_string()),
                ApplyPatchLine::Add("B".to_string()),
                ApplyPatchLine::Context("c".to_string()),
            ],
            end_of_file: false,
        }];
        let updated = apply_patch_apply_hunks(content, &hunks).expect("apply");
        assert_eq!(updated, "a\nB\nc\n");
    }

    #[test]
    fn resolve_path_should_reject_escape() {
        let base = std::env::temp_dir().join("eca-apply-patch-tests");
        let _ = std::fs::create_dir_all(&base);
        let result = apply_patch_resolve_path(&base, "../escape.txt");
        assert!(result.is_err());
    }

    #[test]
    fn backup_record_should_capture_delete_update_and_move() {
        let data_path = make_temp_data_path("apply-patch-backup");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let cwd = cwd.canonicalize().expect("canonical cwd");
        std::fs::write(cwd.join("delete.txt"), b"\x00\x01delete").expect("seed delete");
        std::fs::write(cwd.join("update.txt"), "old\n").expect("seed update");
        std::fs::write(cwd.join("move.txt"), "before\nold\n").expect("seed move");
        let patch = "*** Begin Patch\n*** Delete File: delete.txt\n*** Update File: update.txt\n@@\n-old\n+new\n*** Update File: move.txt\n*** Move to: moved.txt\n@@\n before\n-old\n+new\n*** End Patch";
        let ops = apply_patch_resolve_ops(&cwd, apply_patch_parse(patch).expect("parse")).expect("resolve");
        let record = apply_patch_prepare_backup_record(&data_path, "s1", &cwd, patch, &ops)
            .expect("prepare");
        assert_eq!(record.entries.len(), 3);
        assert!(record.entries.iter().any(|entry| entry.kind == ApplyPatchBackupKind::Delete));
        assert!(record.entries.iter().any(|entry| entry.kind == ApplyPatchBackupKind::Update));
        assert!(record.entries.iter().any(|entry| entry.kind == ApplyPatchBackupKind::MoveUpdate));
    }

    #[test]
    fn clear_apply_patch_temp_should_remove_records_and_blobs() {
        let data_path = make_temp_data_path("apply-patch-clear");
        let records_dir = apply_patch_temp_records_dir(&data_path);
        let blobs_dir = apply_patch_temp_blobs_dir(&data_path);
        std::fs::create_dir_all(&records_dir).expect("create records dir");
        std::fs::create_dir_all(&blobs_dir).expect("create blobs dir");
        std::fs::write(records_dir.join("a.json"), "{}").expect("seed record");
        std::fs::write(blobs_dir.join("b.bin"), "x").expect("seed blob");

        let (records, blobs) = clear_apply_patch_temp(&data_path).expect("clear temp");
        assert_eq!(records, 1);
        assert_eq!(blobs, 1);
        assert_eq!(std::fs::read_dir(&records_dir).expect("read records").count(), 0);
        assert_eq!(std::fs::read_dir(&blobs_dir).expect("read blobs").count(), 0);
    }

    #[test]
    fn backup_record_should_restore_added_file_by_deleting_it() {
        let data_path = make_temp_data_path("apply-patch-restore-add");
        let path = app_root_from_data_path(&data_path).join("added.txt");
        std::fs::write(&path, "hello").expect("seed add");
        let record = ApplyPatchBackupRecord {
            record_id: Uuid::new_v4().to_string(),
            session_id: "s1".to_string(),
            cwd: app_root_from_data_path(&data_path).to_string_lossy().to_string(),
            fingerprint: "fp".to_string(),
            created_at: now_iso(),
            entries: vec![ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::Add,
                path: path.to_string_lossy().to_string(),
                from_path: None,
                to_path: None,
                expected_current_content: Some("hello".to_string()),
                backup_blob_file: None,
            }],
        };
        let restored = apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(restored, 1);
        assert!(!path.exists());
    }

    #[test]
    fn backup_record_should_restore_updated_file_content() {
        let data_path = make_temp_data_path("apply-patch-restore-update");
        let path = app_root_from_data_path(&data_path).join("update.txt");
        std::fs::write(&path, "new\n").expect("seed update");
        std::fs::create_dir_all(apply_patch_temp_blobs_dir(&data_path)).expect("create blobs");
        std::fs::write(apply_patch_blob_path(&data_path, "update.bin"), "old\n")
            .expect("write blob");
        let record = ApplyPatchBackupRecord {
            record_id: Uuid::new_v4().to_string(),
            session_id: "s1".to_string(),
            cwd: app_root_from_data_path(&data_path).to_string_lossy().to_string(),
            fingerprint: "fp".to_string(),
            created_at: now_iso(),
            entries: vec![ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::Update,
                path: path.to_string_lossy().to_string(),
                from_path: None,
                to_path: None,
                expected_current_content: Some("new\n".to_string()),
                backup_blob_file: Some("update.bin".to_string()),
            }],
        };
        let restored = apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(restored, 1);
        assert_eq!(std::fs::read_to_string(&path).expect("read restored"), "old\n");
    }

    #[test]
    fn take_latest_backup_record_should_pick_newest_match() {
        let data_path = make_temp_data_path("apply-patch-record-pick");
        let records_dir = apply_patch_temp_records_dir(&data_path);
        std::fs::create_dir_all(&records_dir).expect("create records");
        let older = ApplyPatchBackupRecord {
            record_id: "older".to_string(),
            session_id: "s1".to_string(),
            cwd: "c".to_string(),
            fingerprint: "fp".to_string(),
            created_at: "2026-03-21T10:00:00Z".to_string(),
            entries: Vec::new(),
        };
        let newer = ApplyPatchBackupRecord {
            record_id: "newer".to_string(),
            session_id: "s1".to_string(),
            cwd: "c".to_string(),
            fingerprint: "fp".to_string(),
            created_at: "2026-03-21T10:00:01Z".to_string(),
            entries: Vec::new(),
        };
        std::fs::write(
            apply_patch_record_path(&data_path, &older.record_id),
            serde_json::to_vec(&older).expect("serialize older"),
        )
        .expect("write older");
        std::fs::write(
            apply_patch_record_path(&data_path, &newer.record_id),
            serde_json::to_vec(&newer).expect("serialize newer"),
        )
        .expect("write newer");

        let hit = apply_patch_take_latest_backup_record(&data_path, "fp")
            .expect("take record")
            .expect("matched record");
        assert_eq!(hit.1.record_id, "newer");
    }
}
