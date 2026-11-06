#[derive(Debug, Clone)]
enum ApplyPatchSafetyCheck {
    AutoApprove,
    Reject { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplyPatchToolArgs {
    operations: Vec<ApplyPatchToolOpArgs>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WriteFileToolArgs {
    path: String,
    content: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeleteFileToolArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateFileToolArgs {
    path: String,
    #[serde(alias = "oldString")]
    old_string: String,
    #[serde(alias = "newString")]
    new_string: String,
    #[serde(default, alias = "replaceAll")]
    replace_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MoveFileToolArgs {
    path: String,
    to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplyPatchToolOpArgs {
    action: String,
    path: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default, alias = "oldString")]
    old_string: Option<String>,
    #[serde(default, alias = "newString")]
    new_string: Option<String>,
    #[serde(default, alias = "replaceAll")]
    replace_all: Option<bool>,
    #[serde(default)]
    to: Option<String>,
}

#[derive(Debug, Clone)]
enum ApplyPatchOp {
    Add { path: String, content: String },
    Overwrite { path: String, content: String },
    Delete { path: String },
    Update { path: String, old_string: String, new_string: String, replace_all: bool },
    Move { path: String, to: String },
}

#[derive(Debug, Clone)]
enum ApplyPatchResolvedOp {
    Add { path: PathBuf, content: String },
    Overwrite { path: PathBuf, content: String },
    Delete { path: PathBuf },
    Update { path: PathBuf, old_string: String, new_string: String, replace_all: bool },
    Move { from: PathBuf, to: PathBuf },
}

#[derive(Debug, Clone)]
struct ApplyPatchExecutionFailure {
    index: usize,
    op: String,
    path: Option<String>,
    message: String,
}

#[derive(Debug, Clone)]
struct ApplyPatchExecutionOutcome {
    changed: Vec<Value>,
    failure: Option<ApplyPatchExecutionFailure>,
}

#[derive(Debug, Clone)]
struct ApplyPatchUpdateResult {
    content: String,
    match_line_ranges: Vec<(usize, usize)>,
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

#[cfg(test)]
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
        entries.push(apply_patch_prepare_backup_entry(data_path, op)?);
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

fn apply_patch_empty_backup_record(
    data_path: &PathBuf,
    session_id: &str,
    cwd: &Path,
    input: &str,
) -> Result<ApplyPatchBackupRecord, String> {
    std::fs::create_dir_all(apply_patch_temp_records_dir(data_path))
        .map_err(|err| format!("创建 apply_patch 记录目录失败：{err}"))?;
    std::fs::create_dir_all(apply_patch_temp_blobs_dir(data_path))
        .map_err(|err| format!("创建 apply_patch 备份目录失败：{err}"))?;
    Ok(ApplyPatchBackupRecord {
        record_id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        cwd: cwd.to_string_lossy().to_string(),
        fingerprint: apply_patch_fingerprint(session_id, cwd, input),
        created_at: now_iso(),
        entries: Vec::new(),
    })
}

fn apply_patch_prepare_backup_entry(
    data_path: &PathBuf,
    op: &ApplyPatchResolvedOp,
) -> Result<ApplyPatchBackupEntry, String> {
    match op {
        ApplyPatchResolvedOp::Add { path, content } => Ok(ApplyPatchBackupEntry {
            kind: ApplyPatchBackupKind::Add,
            path: path.to_string_lossy().to_string(),
            from_path: None,
            to_path: None,
            // 撤回时用 apply_patch_read_text_file 读回当前内容，它读到的是归一化后的 LF 视图。
            // expected 必须同口径，否则内容只是行尾不同也会被记成「被外部修改过」。
            expected_current_content: Some(normalize_line_endings(content).0),
            backup_blob_file: None,
        }),
        ApplyPatchResolvedOp::Delete { path } => {
            let raw = std::fs::read(path)
                .map_err(|_| format!("Delete File 失败，文件不存在：{}", path.to_string_lossy()))?;
            let metadata = std::fs::metadata(path)
                .map_err(|_| format!("Delete File 失败，文件不存在：{}", path.to_string_lossy()))?;
            if !metadata.is_file() {
                return Err(format!("Delete File 失败，目标不是文件：{}", path.to_string_lossy()));
            }
            let blob_file = format!("{}.bin", Uuid::new_v4());
            std::fs::write(apply_patch_blob_path(data_path, &blob_file), raw)
                .map_err(|err| format!("写入删除备份失败（{}）：{err}", path.to_string_lossy()))?;
            Ok(ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::Delete,
                path: path.to_string_lossy().to_string(),
                from_path: None,
                to_path: None,
                expected_current_content: None,
                backup_blob_file: Some(blob_file),
            })
        }
        ApplyPatchResolvedOp::Overwrite { path, content } => {
            let raw = std::fs::read(path)
                .map_err(|_| format!("Write File 失败，文件不存在：{}", path.to_string_lossy()))?;
            let blob_file = format!("{}.bin", Uuid::new_v4());
            std::fs::write(apply_patch_blob_path(data_path, &blob_file), raw)
                .map_err(|err| format!("写入覆盖备份失败（{}）：{err}", path.to_string_lossy()))?;
            Ok(ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::Update,
                path: path.to_string_lossy().to_string(),
                from_path: None,
                to_path: None,
                // 同 Add：撤回比较走归一化视图，写盘时才按原行尾还原。
                expected_current_content: Some(normalize_line_endings(content).0),
                backup_blob_file: Some(blob_file),
            })
        }
        ApplyPatchResolvedOp::Move { from, to } => {
            let raw = std::fs::read(from)
                .map_err(|_| format!("Move 操作失败，文件不存在：{}", from.to_string_lossy()))?;
            let expected = decode_text_file_bytes(&raw).ok().map(|decoded| decoded.text);
            let blob_file = format!("{}.bin", Uuid::new_v4());
            std::fs::write(apply_patch_blob_path(data_path, &blob_file), raw)
                .map_err(|err| format!("写入移动备份失败（{}）：{err}", from.to_string_lossy()))?;
            Ok(ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::MoveUpdate,
                path: to.to_string_lossy().to_string(),
                from_path: Some(from.to_string_lossy().to_string()),
                to_path: Some(to.to_string_lossy().to_string()),
                expected_current_content: expected,
                backup_blob_file: Some(blob_file),
            })
        }
        ApplyPatchResolvedOp::Update { path, old_string, new_string, replace_all } => {
            let raw = std::fs::read(path)
                .map_err(|_| format!("Update 操作失败，文件不存在：{}", path.to_string_lossy()))?;
            let old_content = decode_text_file_bytes(&raw)
                .map_err(|err| format!("Update 操作失败，{}：{}", err, path.to_string_lossy()))?
                .text;
            let new_content = apply_patch_apply_update(
                &old_content,
                old_string,
                new_string,
                *replace_all,
            )?;
            let blob_file = format!("{}.bin", Uuid::new_v4());
            std::fs::write(apply_patch_blob_path(data_path, &blob_file), raw)
                .map_err(|err| format!("写入修改备份失败（{}）：{err}", path.to_string_lossy()))?;
            Ok(ApplyPatchBackupEntry {
                kind: ApplyPatchBackupKind::Update,
                path: path.to_string_lossy().to_string(),
                from_path: None,
                to_path: None,
                expected_current_content: Some(new_content.content),
                backup_blob_file: Some(blob_file),
            })
        }
    }
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

fn apply_patch_cleanup_backup_entry(
    data_path: &PathBuf,
    entry: &ApplyPatchBackupEntry,
) -> Result<(), String> {
    if let Some(blob_file) = entry.backup_blob_file.as_deref() {
        let blob_path = apply_patch_blob_path(data_path, blob_file);
        if blob_path.exists() {
            std::fs::remove_file(&blob_path).map_err(|err| {
                format!("清理 apply_patch 备份文件失败（{}）：{err}", blob_path.to_string_lossy())
            })?;
        }
    }
    Ok(())
}

fn apply_patch_read_backup_record(path: &Path) -> Result<ApplyPatchBackupRecord, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("读取 apply_patch 恢复记录失败（{}）：{err}", path.to_string_lossy()))?;
    serde_json::from_str::<ApplyPatchBackupRecord>(&raw)
        .map_err(|err| format!("解析 apply_patch 恢复记录失败（{}）：{err}", path.to_string_lossy()))
}

fn apply_patch_read_text_file(path: &Path) -> Result<String, String> {
    decode_text_file_from_path(path).map(|decoded| decoded.text)
}

fn apply_patch_write_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("创建目录失败（{}）：{err}", terminal_path_for_user(parent)))?;
    }
    Ok(())
}

/// 恢复备份记录中的所有文件。
/// 无论文件当前状态如何都强制恢复，返回 (恢复文件数, 有非LLM修改被覆盖的文件列表)。
fn apply_patch_restore_backup_record(
    data_path: &PathBuf,
    record: &ApplyPatchBackupRecord,
) -> Result<(usize, Vec<String>), String> {
    let mut restored = 0usize;
    let mut overwritten_files = Vec::<String>::new();
    for entry in record.entries.iter().rev() {
        match entry.kind {
            ApplyPatchBackupKind::Add => {
                // Add 操作的撤回 = 删除该文件
                let path = PathBuf::from(&entry.path);
                if !path.exists() {
                    // 文件已不存在，视为已撤回，跳过
                    continue;
                }
                let expected = entry.expected_current_content.as_deref().unwrap_or_default();
                if let Ok(current) = apply_patch_read_text_file(&path) {
                    if current != expected {
                        overwritten_files.push(terminal_path_for_user(&path));
                    }
                }
                if let Err(err) = std::fs::remove_file(&path) {
                    runtime_log_warn(format!(
                        "[apply_patch撤回] 删除文件失败（跳过）: path={}, error={}",
                        terminal_path_for_user(&path), err
                    ));
                    continue;
                }
                restored = restored.saturating_add(1);
            }
            ApplyPatchBackupKind::Delete => {
                // Delete 操作的撤回 = 从 blob 恢复文件
                let path = PathBuf::from(&entry.path);
                let blob_file = match entry.backup_blob_file.as_deref() {
                    Some(f) => f,
                    None => continue,
                };
                let raw = match std::fs::read(apply_patch_blob_path(data_path, blob_file)) {
                    Ok(data) => data,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[apply_patch撤回] 读取删除备份失败（跳过）: path={}, error={}",
                            terminal_path_for_user(&path), err
                        ));
                        continue;
                    }
                };
                if path.exists() {
                    // 文件已存在（用户或其他操作重新创建了），强制覆盖但记录
                    overwritten_files.push(terminal_path_for_user(&path));
                }
                let _ = apply_patch_write_parent_dir(&path);
                if let Err(err) = std::fs::write(&path, raw) {
                    runtime_log_warn(format!(
                        "[apply_patch撤回] 恢复文件失败（跳过）: path={}, error={}",
                        terminal_path_for_user(&path), err
                    ));
                    continue;
                }
                restored = restored.saturating_add(1);
            }
            ApplyPatchBackupKind::Update => {
                // Update 操作的撤回 = 从 blob 恢复原始内容
                let path = PathBuf::from(&entry.path);
                let blob_file = match entry.backup_blob_file.as_deref() {
                    Some(f) => f,
                    None => continue,
                };
                let raw = match std::fs::read(apply_patch_blob_path(data_path, blob_file)) {
                    Ok(data) => data,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[apply_patch撤回] 读取修改备份失败（跳过）: path={}, error={}",
                            terminal_path_for_user(&path), err
                        ));
                        continue;
                    }
                };
                if path.exists() {
                    let expected = entry.expected_current_content.as_deref().unwrap_or_default();
                    if let Ok(current) = apply_patch_read_text_file(&path) {
                        if current != expected {
                            overwritten_files.push(terminal_path_for_user(&path));
                        }
                    }
                }
                let _ = apply_patch_write_parent_dir(&path);
                if let Err(err) = std::fs::write(&path, raw) {
                    runtime_log_warn(format!(
                        "[apply_patch撤回] 恢复文件失败（跳过）: path={}, error={}",
                        terminal_path_for_user(&path), err
                    ));
                    continue;
                }
                restored = restored.saturating_add(1);
            }
            ApplyPatchBackupKind::MoveUpdate => {
                // MoveUpdate 操作的撤回 = 从 blob 恢复到原始路径，删除移动后的路径
                let from_path = PathBuf::from(entry.from_path.as_deref().unwrap_or_default());
                let to_path = PathBuf::from(entry.to_path.as_deref().unwrap_or_default());
                let blob_file = match entry.backup_blob_file.as_deref() {
                    Some(f) => f,
                    None => continue,
                };
                let raw = match std::fs::read(apply_patch_blob_path(data_path, blob_file)) {
                    Ok(data) => data,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[apply_patch撤回] 读取移动备份失败（跳过）: path={}, error={}",
                            terminal_path_for_user(&to_path), err
                        ));
                        continue;
                    }
                };
                // 检查移动后的文件是否有非 LLM 修改
                if to_path.exists() {
                    let expected = entry.expected_current_content.as_deref().unwrap_or_default();
                    if let Ok(current) = apply_patch_read_text_file(&to_path) {
                        if current != expected {
                            overwritten_files.push(terminal_path_for_user(&to_path));
                        }
                    }
                }
                // 检查原始路径是否已被占用
                if from_path.exists()
                    && terminal_normalize_for_access_check(&from_path)
                        != terminal_normalize_for_access_check(&to_path)
                {
                    overwritten_files.push(terminal_path_for_user(&from_path));
                }
                let _ = apply_patch_write_parent_dir(&from_path);
                if let Err(err) = std::fs::write(&from_path, raw) {
                    runtime_log_warn(format!(
                        "[apply_patch撤回] 恢复原始文件失败（跳过）: path={}, error={}",
                        terminal_path_for_user(&from_path), err
                    ));
                    continue;
                }
                // 删除移动后的文件（如果和原始路径不同）
                if terminal_normalize_for_access_check(&from_path)
                    != terminal_normalize_for_access_check(&to_path)
                    && to_path.exists()
                {
                    let _ = std::fs::remove_file(&to_path);
                }
                restored = restored.saturating_add(1);
            }
        }
    }
    Ok((restored, overwritten_files))
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

fn apply_patch_tool_args_to_raw_json(args: &ApplyPatchToolArgs) -> Result<String, String> {
    serde_json::to_string(args).map_err(|err| format!("apply_patch 参数序列化失败：{err}"))
}

fn apply_patch_json_example() -> &'static str {
    r#"{"operations":[{"action":"update","path":"src/example.ts","old_string":"before","new_string":"after"}]}"#
}

fn apply_patch_ops_from_tool_args(args: ApplyPatchToolArgs) -> Result<Vec<ApplyPatchOp>, String> {
    if args.operations.is_empty() {
        return Err(apply_patch_format_error(
            "apply_patch 操作列表为空。至少需要一项操作。",
        ));
    }
    let mut ops = Vec::<ApplyPatchOp>::new();
    for (i, op) in args.operations.into_iter().enumerate() {
        match op.action.as_str() {
            "add" => {
                let Some(content) = op.content else {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (add) 缺少 "content" 字段。add 操作必须提供文件内容。\n最小 add 示例：{}"#,
                        i,
                        apply_patch_operation_example("add")
                    )));
                };
                ops.push(ApplyPatchOp::Add { path: op.path, content });
            }
            "overwrite" => {
                let Some(content) = op.content else {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (overwrite) 缺少 "content" 字段。overwrite 操作必须提供完整文件内容。\n最小 overwrite 示例：{}"#,
                        i,
                        apply_patch_operation_example("overwrite")
                    )));
                };
                ops.push(ApplyPatchOp::Overwrite { path: op.path, content });
            }
            "delete" => {
                if op.old_string.is_some() || op.new_string.is_some() || op.replace_all.is_some() {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (delete) 只会删除整个文件，不支持 "old_string"、"new_string" 或 "replace_all"。如果你想删除文件中的部分内容，请改用 update，并让 new_string 设为空字符串。\n最小 delete 示例：{}\n最小 update 示例：{}"#,
                        i,
                        apply_patch_operation_example("delete"),
                        apply_patch_operation_example("update")
                    )));
                }
                ops.push(ApplyPatchOp::Delete { path: op.path });
            }
            "update" => {
                let Some(old_string) = op.old_string else {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (update) 缺少 "old_string" 字段。\n最小 update 示例：{}"#,
                        i,
                        apply_patch_operation_example("update")
                    )));
                };
                let Some(new_string) = op.new_string else {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (update) 缺少 "new_string" 字段。\n最小 update 示例：{}"#,
                        i,
                        apply_patch_operation_example("update")
                    )));
                };
                if old_string.is_empty() {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (update) 的 "old_string" 不能为空。请提供要替换的原文片段。\n最小 update 示例：{}"#,
                        i,
                        apply_patch_operation_example("update")
                    )));
                }
                // 补丁内容来自模型，模型看到的是 read_file 归一化后的 LF 视图；但它也可能
                // 从 terminal 输出、git show 里复制到带 \r 的原文。这里统一到同一套视图，
                // 否则文件内容里一格 \r 都不剩，任何带 \r 的 old_string 必然匹配失败，
                // 而失败提示还会指出「最相似的就是这一段」，把模型逼进原地重试。
                // new_string 同样归一化：写回时由目标文件的行尾统一还原，不会丢信息。
                let old_string = normalize_line_endings(&old_string).0;
                let new_string = normalize_line_endings(&new_string).0;
                if old_string == new_string {
                    return Err(apply_patch_format_error(format!(
                        "apply_patch 操作[{}] (update) old_string 和 new_string 完全相同。update 必须真的修改内容。\n最小 update 示例：{}",
                        i,
                        apply_patch_operation_example("update")
                    )));
                }
                ops.push(ApplyPatchOp::Update {
                    path: op.path,
                    old_string,
                    new_string,
                    replace_all: op.replace_all.unwrap_or(false),
                });
            }
            "move" => {
                let Some(to) = op.to else {
                    return Err(apply_patch_format_error(format!(
                        r#"apply_patch 操作[{}] (move) 缺少 "to" 字段。\n最小 move 示例：{}"#,
                        i,
                        apply_patch_operation_example("move")
                    )));
                };
                ops.push(ApplyPatchOp::Move { path: op.path, to });
            }
            other => {
                return Err(apply_patch_format_error(format!(
                    r#"apply_patch 操作[{}] 的 action "{}" 无效。必须是 "add"、"overwrite"、"update"、"delete" 或 "move"。\n可参考最小 update 示例：{}"#,
                    i,
                    other,
                    apply_patch_operation_example("update")
                )));
            }
        }
    }
    Ok(ops)
}

fn apply_patch_operation_example(action: &str) -> &'static str {
    match action {
        "add" => r#"{"action":"add","path":"src/new.ts","content":"export const value = 1;\n"}"#,
        "overwrite" => r#"{"action":"overwrite","path":"src/existing.ts","content":"export const value = 2;\n"}"#,
        "update" => r#"{"action":"update","path":"src/example.ts","old_string":"before","new_string":"after","replace_all":false}"#,
        "delete" => r#"{"action":"delete","path":"src/old.ts"}"#,
        "move" => r#"{"action":"move","path":"src/old.ts","to":"src/new.ts"}"#,
        _ => r#"{"action":"update","path":"src/example.ts","old_string":"before","new_string":"after"}"#,
    }
}

fn apply_patch_format_error(message: impl AsRef<str>) -> String {
    format!(
        "{}\n\napply_patch 只支持 JSON 格式，不支持标准 git diff / unified diff。\n顶层格式示例：\n{}",
        message.as_ref(),
        apply_patch_json_example()
    )
}

fn apply_patch_preview_text(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
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
    let candidate = PathBuf::from(&normalized);
    let joined = if candidate.is_absolute() || apply_patch_has_windows_drive_prefix(&normalized) {
        candidate
    } else {
        base.join(candidate)
    };
    Ok(terminal_normalize_for_access_check(&joined))
}

fn apply_patch_resolve_ops(base: &Path, ops: Vec<ApplyPatchOp>) -> Result<Vec<ApplyPatchResolvedOp>, String> {
    let mut out = Vec::<ApplyPatchResolvedOp>::new();
    for op in ops {
        match op {
            ApplyPatchOp::Add { path, content } => out.push(ApplyPatchResolvedOp::Add {
                path: apply_patch_resolve_path(base, &path)?,
                content,
            }),
            ApplyPatchOp::Delete { path } => out.push(ApplyPatchResolvedOp::Delete {
                path: apply_patch_resolve_path(base, &path)?,
            }),
            ApplyPatchOp::Overwrite { path, content } => out.push(ApplyPatchResolvedOp::Overwrite {
                path: apply_patch_resolve_path(base, &path)?,
                content,
            }),
            ApplyPatchOp::Update { path, old_string, new_string, replace_all } => {
                let path = apply_patch_resolve_path(base, &path)?;
                out.push(ApplyPatchResolvedOp::Update { path, old_string, new_string, replace_all });
            }
            ApplyPatchOp::Move { path, to } => {
                let from = apply_patch_resolve_path(base, &path)?;
                let to = apply_patch_resolve_path(base, &to)?;
                out.push(ApplyPatchResolvedOp::Move { from, to });
            }
        }
    }
    Ok(out)
}


fn apply_patch_assess_safety(
    state: &AppState,
    _session_id: &str,
    _cwd: &Path,
    ops: &[ApplyPatchResolvedOp],
) -> Result<ApplyPatchSafetyCheck, String> {
    if ops.is_empty() {
        return Ok(ApplyPatchSafetyCheck::Reject {
            reason: "empty patch".to_string(),
        });
    }
    let mut target_paths = Vec::<PathBuf>::new();
    for op in ops {
        match op {
            ApplyPatchResolvedOp::Add { path, .. }
            | ApplyPatchResolvedOp::Overwrite { path, .. }
            | ApplyPatchResolvedOp::Delete { path }
            | ApplyPatchResolvedOp::Update { path, .. } => target_paths.push(path.clone()),
            ApplyPatchResolvedOp::Move { from, to } => {
                target_paths.push(from.clone());
                target_paths.push(to.clone());
            }
        }
    }
    let target_paths = terminal_dedup_paths(target_paths);
    if target_paths.is_empty() {
        return Ok(ApplyPatchSafetyCheck::Reject {
            reason: "empty patch".to_string(),
        });
    }
    let mut accesses = Vec::<String>::new();
    for path in &target_paths {
        let Some(workspace) = terminal_match_workspace_for_session_target(state, _session_id, path)
            .unwrap_or(None)
        else {
            runtime_log_warn(format!(
                "[补丁拦截] 写入目标未命中本会话权限白名单，session={}，target={}",
                _session_id,
                terminal_path_for_user(path),
            ));
            return Ok(ApplyPatchSafetyCheck::Reject {
                reason: format!(
                    "补丁路径未命中本会话可写范围：{}",
                    terminal_path_for_user(&path)
                ),
            });
        };
        accesses.push(workspace.access);
    }
    let effective_access = terminal_strictest_workspace_access(&accesses);
    if effective_access == SHELL_WORKSPACE_ACCESS_READ_ONLY {
        return Ok(ApplyPatchSafetyCheck::Reject {
            reason: "当前目录权限为只读，禁止执行补丁。".to_string(),
        });
    }
    Ok(ApplyPatchSafetyCheck::AutoApprove)
}

/// update 匹配失败的成因。刻意不带面向模型的文案——
/// 构造文案要做全文候选定位，可能很贵，只应在确认彻底失败后做一次。
enum ApplyPatchUpdateMiss {
    EmptyOldString,
    NotFound,
    Unchanged,
    Ambiguous { count: usize, ranges: String },
}

/// 只负责匹配与替换：不构造任何面向模型的文案。
///
/// 入参内容已是解码后的 LF 视图（见 `text_codec` 的行尾归一化），所以这里
/// 不需要为 CRLF/LF 差异留任何后路——两套行尾写法不会同时出现在比较的两侧。
fn apply_patch_update_core(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<ApplyPatchUpdateResult, ApplyPatchUpdateMiss> {
    if old_string.is_empty() {
        return Err(ApplyPatchUpdateMiss::EmptyOldString);
    }
    if !content.contains(old_string) {
        return Err(ApplyPatchUpdateMiss::NotFound);
    }
    if !replace_all {
        let count = content.matches(old_string).count();
        if count > 1 {
            return Err(ApplyPatchUpdateMiss::Ambiguous {
                count,
                ranges: apply_patch_format_ranges_limited(
                    apply_patch_exact_match_ranges(content, old_string),
                    12,
                ),
            });
        }
    }
    let match_line_ranges = apply_patch_exact_match_ranges(content, old_string);
    let result = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    if result == content {
        return Err(ApplyPatchUpdateMiss::Unchanged);
    }
    Ok(ApplyPatchUpdateResult {
        content: result,
        match_line_ranges,
    })
}

/// 把一次失败升级成面向模型的完整文案。`NotFound` 分支要做全文候选定位，
/// 是这里唯一可能昂贵的分支——调用方必须保证只在确实要返回错误时调用。
fn apply_patch_update_miss_error(
    content: &str,
    old_string: &str,
    miss: ApplyPatchUpdateMiss,
) -> String {
    let old_preview = apply_patch_preview_text(old_string, 300);
    match miss {
        ApplyPatchUpdateMiss::EmptyOldString => format!(
            "update 失败：old_string 不能为空。请提供要替换的原文片段。\n最小 update 示例：{}",
            apply_patch_operation_example("update")
        ),
        ApplyPatchUpdateMiss::Unchanged => {
            "apply_patch update 失败：替换后文件内容未变化。".to_string()
        }
        ApplyPatchUpdateMiss::Ambiguous { count, ranges } => format!(
            "apply_patch update 失败：old_string 在文件中出现了 {} 次，但 replace_all 为 false。\n命中行范围：{}\n请改用以下两种方式之一：\n1. 扩大 old_string，上下多带几行稳定上下文，使其只命中 1 处。\n2. 如果你确实要全部替换，再设置 replace_all: true。\n最小 update 示例：{}\nold_string 预览（前 300 字符）：\n{}",
            count,
            ranges,
            apply_patch_operation_example("update"),
            old_preview
        ),
        ApplyPatchUpdateMiss::NotFound => {
            let similar = apply_patch_similar_line_ranges(content, old_string);
            let similar_hint = if similar.is_empty() {
                "最相似候选行范围：未找到可用候选。".to_string()
            } else {
                let rows = similar
                    .into_iter()
                    .map(|(start, end, summary)| {
                        format!(
                            "- {}，摘要：{}",
                            apply_patch_format_line_range((start, end)),
                            summary
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("最相似候选行范围：\n{rows}")
            };
            format!(
                "apply_patch update 失败：old_string 在文件中未找到。\n{}\n请先重新读取目标文件，直接复制文件中的原文作为 old_string。\n如果你原本想提交标准 git diff，请改成 JSON update 操作。\n最小 update 示例：{}\nold_string 预览（前 300 字符）：\n{}",
                similar_hint,
                apply_patch_operation_example("update"),
                old_preview
            )
        }
    }
}

/// 单次尝试并即刻给出文案。
fn apply_patch_apply_update(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<ApplyPatchUpdateResult, String> {
    apply_patch_update_core(content, old_string, new_string, replace_all)
        .map_err(|miss| apply_patch_update_miss_error(content, old_string, miss))
}

fn apply_patch_line_number_at_byte(content: &str, offset: usize) -> usize {
    content
        .as_bytes()
        .get(..offset.min(content.len()))
        .unwrap_or_default()
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        .saturating_add(1)
}

fn apply_patch_match_line_range(content: &str, start: usize, len: usize) -> (usize, usize) {
    let end = start.saturating_add(len).min(content.len());
    let adjusted_end = if end > start && content.as_bytes().get(end.saturating_sub(1)) == Some(&b'\n') {
        end.saturating_sub(1)
    } else {
        end
    };
    (
        apply_patch_line_number_at_byte(content, start),
        apply_patch_line_number_at_byte(content, adjusted_end),
    )
}

fn apply_patch_format_line_range((start, end): (usize, usize)) -> String {
    if start == end {
        format!("line {start}")
    } else {
        format!("lines {start}-{end}")
    }
}

fn apply_patch_exact_match_ranges(content: &str, needle: &str) -> Vec<(usize, usize)> {
    content
        .match_indices(needle)
        .map(|(start, _)| apply_patch_match_line_range(content, start, needle.len()))
        .collect()
}

fn apply_patch_format_ranges_limited(ranges: Vec<(usize, usize)>, limit: usize) -> String {
    let total = ranges.len();
    let mut out = ranges
        .into_iter()
        .take(limit)
        .map(apply_patch_format_line_range)
        .collect::<Vec<_>>()
        .join(", ");
    if total > limit {
        out.push_str(&format!(" ... 另有 {} 处", total - limit));
    }
    out
}

fn apply_patch_compact_line_summary(value: &str, limit: usize) -> String {
    let compact = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return "<空行>".to_string();
    }
    apply_patch_preview_text(&compact, limit)
}

fn apply_patch_similar_line_ranges(
    content: &str,
    old_string: &str,
) -> Vec<(usize, usize, String)> {
    // 第一层按整行投票定位；投不出来再退到行内模糊比较。
    // 两层都失败就承认没有候选，而不是随便指一处。
    apply_patch_locate_candidate_by_line_votes(content, old_string)
        .or_else(|| apply_patch_locate_candidate_by_line_similarity(content, old_string))
        .map(|(start, end, preview)| {
            vec![(start, end, apply_patch_compact_line_summary(&preview, 80))]
        })
        .unwrap_or_default()
}

/// 归一化一整行：去掉全部空白并转小写。
///
/// 缩进、行尾空白、大小写是手抄原文时最常见的三类偏差，整行归一化后它们不再影响定位。
fn apply_patch_line_key(line: &str) -> String {
    line.chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// 只有含字母或数字的行才参与投票。
///
/// `}`、`);`、空行在文件里大量重复，让它们投票只会把候选摊平，反而掩盖真正的线索。
fn apply_patch_line_is_distinct(key: &str) -> bool {
    key.chars().any(char::is_alphanumeric)
}

/// 单行 Levenshtein。只用于比较两行文本，不参与整块窗口比对——
/// 前者是 行长²，后者是 整块长度²，差着几个数量级。
fn apply_patch_line_levenshtein(left: &str, right: &str) -> usize {
    let left_chars = left.chars().collect::<Vec<_>>();
    let right_chars = right.chars().collect::<Vec<_>>();
    if left_chars.is_empty() {
        return right_chars.len();
    }
    if right_chars.is_empty() {
        return left_chars.len();
    }
    let mut prev = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut curr = vec![0usize; right_chars.len() + 1];
    for (i, left_ch) in left_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, right_ch) in right_chars.iter().enumerate() {
            let cost = usize::from(left_ch != right_ch);
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[right_chars.len()]
}

/// 第二层兜底：整行一票都投不出来时（典型是手抄时某一行差了一两个字），
/// 改用单行模糊比较——拿 old_string 里第一条有内容的行当锚点，逐行算行内编辑距离。
///
/// 代价是 文件行数 × 行长²，而不是旧实现的 窗口数 × 整块长度²。短行文件一万行在
/// 百毫秒量级；行长上百字符时这一项会明显放大，但只在投票完全失败时才会走到，
/// 且不会像旧实现那样随文件行数与块长度的平方一起爆掉。
fn apply_patch_locate_candidate_by_line_similarity(
    content: &str,
    old_string: &str,
) -> Option<(usize, usize, String)> {
    // 单行相似度低于这个值不值得给候选：差一两个字是抄写偏差，差一半就是另一行了。
    const MIN_LINE_SIMILARITY: f64 = 0.6;
    // 最佳行必须比次佳行领先这么多，否则说明文件里有多行同样相近，指出哪一行都是猜。
    const MIN_LINE_LEAD: f64 = 0.1;

    let lines = content.lines().collect::<Vec<_>>();
    let search = old_string.lines().collect::<Vec<_>>();

    // 模糊层只负责补齐「绝大部分行精确命中、个别行差一两个字符」的情况。
    // 多行 old_string 里只有一行对得上时，那一行会成为锚点，把任意短行指成候选——
    // 这种猜测比不给候选更糟（会引导模型改坏别处），所以先要求精确命中已覆盖三分之二。
    // 单行的 old_string 没有第二份证据可用，锚点即全部线索，不必卡这道门槛。
    let exact_keys = lines
        .iter()
        .map(|line| apply_patch_line_key(line))
        .collect::<std::collections::HashSet<_>>();
    let mut wanted = 0usize;
    let mut exact_hits = 0usize;
    for line in &search {
        let key = apply_patch_line_key(line);
        if !apply_patch_line_is_distinct(&key) {
            continue;
        }
        wanted += 1;
        if exact_keys.contains(&key) {
            exact_hits += 1;
        }
    }
    if wanted == 0 {
        return None;
    }
    if wanted > 1 && exact_hits * 3 < wanted * 2 {
        return None;
    }

    let (anchor_offset, anchor_key) = search
        .iter()
        .enumerate()
        .map(|(offset, line)| (offset, apply_patch_line_key(line)))
        .find(|(_, key)| apply_patch_line_is_distinct(key))?;
    let anchor_len = anchor_key.chars().count();
    if anchor_len == 0 {
        return None;
    }

    let mut ranked = Vec::<(f64, usize)>::new();
    for (index, line) in lines.iter().enumerate() {
        let key = apply_patch_line_key(line);
        if !apply_patch_line_is_distinct(&key) {
            continue;
        }
        let len = key.chars().count();
        // 长度差一倍以上不必算编辑距离
        if len * 2 < anchor_len || anchor_len * 2 < len {
            continue;
        }
        let distance = apply_patch_line_levenshtein(&key, &anchor_key);
        let score = 1.0 - distance as f64 / len.max(anchor_len) as f64;
        if score >= MIN_LINE_SIMILARITY {
            ranked.push((score, index));
        }
    }
    ranked.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });
    let (best, index) = *ranked.first()?;
    if ranked
        .get(1)
        .map(|(second, _)| best - second < MIN_LINE_LEAD)
        .unwrap_or(false)
    {
        return None;
    }
    let start = index.saturating_sub(anchor_offset);
    let end = start.saturating_add(search.len()).min(lines.len());
    Some((start + 1, end, lines[start..end].join("\n")))
}

/// 在 old_string 未能精确匹配时，定位文件里最可能对应的一段，供错误提示使用。
///
/// 这里曾按「三种行数窗口 × 每一个起点」逐个计算整段编辑距离，代价是
/// 窗口数 × 窗口长度²：一千行文件配三百行更新要十几分钟，且对 5000 行以上的文件
/// 直接放弃、不给任何提示。改为行级投票后——整行归一化后建「行 -> 行号」索引，
/// old_string 的每一行去索引里查并给候选起点投票——复杂度是
/// O(文件行数 + old_string 行数)，与块长度无关，一万行文件也在十毫秒内。
///
/// 只产出唯一最优候选：调用方只需要一句「最可能是哪几行」，给多个反而干扰模型。
fn apply_patch_locate_candidate_by_line_votes(
    content: &str,
    old_string: &str,
) -> Option<(usize, usize, String)> {
    // 有效行里至少三分之二要投到同一个起点。定在一半会让「只有一半行对得上」的输入
    // 也拿到候选——那种情况实质上等于在猜，而猜错位置会让模型改坏别的地方。
    const MIN_VOTE_NUMERATOR: usize = 2;
    const MIN_VOTE_DENOMINATOR: usize = 3;
    // 最高票还要明显领先次高票：文件里到处是相似内容时票数会被摊平，
    // 此时指出任何位置都是瞎猜，宁可回落到兜底文案。
    const LEAD_MULTIPLIER: usize = 3;
    const LEAD_GAP_NUMERATOR: usize = 3;
    const LEAD_GAP_DENOMINATOR: usize = 10;

    let lines = content.lines().collect::<Vec<_>>();
    let search = old_string.lines().collect::<Vec<_>>();
    if lines.is_empty() || search.is_empty() {
        return None;
    }

    let mut positions: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        let key = apply_patch_line_key(line);
        if !apply_patch_line_is_distinct(&key) {
            continue;
        }
        positions.entry(key).or_default().push(index);
    }
    if positions.is_empty() {
        return None;
    }

    let mut votes: std::collections::HashMap<isize, usize> = std::collections::HashMap::new();
    let mut wanted = 0usize;
    for (offset, line) in search.iter().enumerate() {
        let key = apply_patch_line_key(line);
        if !apply_patch_line_is_distinct(&key) {
            continue;
        }
        wanted += 1;
        if let Some(hits) = positions.get(&key) {
            for &index in hits {
                *votes.entry(index as isize - offset as isize).or_insert(0) += 1;
            }
        }
    }
    if wanted == 0 {
        return None;
    }

    // 同票时取更靠前的起点，保证同一输入永远得到同一结果。
    let mut best: Option<(isize, usize)> = None;
    for (start, count) in &votes {
        let better = match best {
            None => true,
            Some((best_start, best_count)) => {
                *count > best_count || (*count == best_count && *start < best_start)
            }
        };
        if better {
            best = Some((*start, *count));
        }
    }
    let (start, best_count) = best?;
    if start < 0 || best_count == 0 {
        return None;
    }
    let second = votes
        .iter()
        .filter(|(candidate, _)| **candidate != start)
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(0);

    if best_count * MIN_VOTE_DENOMINATOR < wanted * MIN_VOTE_NUMERATOR {
        return None;
    }
    let lead_ok = second == 0
        || best_count >= second.saturating_mul(LEAD_MULTIPLIER)
        || (best_count - second) * LEAD_GAP_DENOMINATOR >= wanted * LEAD_GAP_NUMERATOR;
    if !lead_ok {
        return None;
    }

    let start = start as usize;
    let end = start.saturating_add(search.len()).min(lines.len());
    Some((start + 1, end, lines[start..end].join("\n")))
}

fn apply_patch_op_name(op: &ApplyPatchResolvedOp) -> &'static str {
    match op {
        ApplyPatchResolvedOp::Add { .. } => "add",
        ApplyPatchResolvedOp::Overwrite { .. } => "overwrite",
        ApplyPatchResolvedOp::Delete { .. } => "delete",
        ApplyPatchResolvedOp::Update { .. } => "update",
        ApplyPatchResolvedOp::Move { .. } => "move",
    }
}

fn apply_patch_op_path(op: &ApplyPatchResolvedOp) -> Option<String> {
    match op {
        ApplyPatchResolvedOp::Add { path, .. }
        | ApplyPatchResolvedOp::Overwrite { path, .. }
        | ApplyPatchResolvedOp::Delete { path }
        | ApplyPatchResolvedOp::Update { path, .. } => Some(terminal_path_for_user(path)),
        ApplyPatchResolvedOp::Move { from, .. } => Some(terminal_path_for_user(from)),
    }
}

async fn apply_patch_execute_single_op(op: &ApplyPatchResolvedOp) -> Result<Value, String> {
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
            Ok(serde_json::json!({
                "op": "add",
                "path": terminal_path_for_user(path),
            }))
        }
        ApplyPatchResolvedOp::Delete { path } => {
            let metadata = tokio::fs::metadata(path)
                .await
                .map_err(|_| format!("Delete File 失败，文件不存在：{}", path.to_string_lossy()))?;
            if !metadata.is_file() {
                return Err(format!("Delete File 失败，目标不是文件：{}", path.to_string_lossy()));
            }
            trash::delete(path)
                .map_err(|err| format!("删除文件失败（{}）：{err}", path.to_string_lossy()))?;
            Ok(serde_json::json!({
                "op": "delete",
                "path": terminal_path_for_user(path),
            }))
        }
        ApplyPatchResolvedOp::Overwrite { path, content } => {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|err| format!("创建目录失败（{}）：{err}", parent.to_string_lossy()))?;
            }
            // 覆盖已有 CRLF 文件时沿用它的行尾，否则整份文件会被模型带来的 LF 刷成 LF，
            // 产生一整篇全是改动的 diff。文件不存在（新建）时按 LF。
            let line_ending = match tokio::fs::read(path).await {
                Ok(raw) => decode_text_file_bytes(&raw)
                    .map(|decoded| decoded.line_ending)
                    .unwrap_or(TextLineEnding::Lf),
                Err(_) => TextLineEnding::Lf,
            };
            let body = line_ending.restore(content);
            tokio::fs::write(path, body.as_bytes())
                .await
                .map_err(|err| format!("写入文件失败（{}）：{err}", path.to_string_lossy()))?;
            Ok(serde_json::json!({
                "op": "overwrite",
                "path": terminal_path_for_user(path),
                "bytes": body.len(),
            }))
        }
        ApplyPatchResolvedOp::Move { from, to } => {
            let raw_move = tokio::fs::read(from)
                .await
                .map_err(|_| format!("Move 操作失败，文件不存在：{}", from.to_string_lossy()))?;
            let _ = decode_text_file_bytes(&raw_move)
                .map_err(|err| format!("Move 操作失败，{}：{}", err, from.to_string_lossy()))?;
            if let Some(parent) = to.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|err| format!("创建目录失败（{}）：{err}", parent.to_string_lossy()))?;
            }
            if to.exists()
                && terminal_normalize_for_access_check(to)
                    != terminal_normalize_for_access_check(from)
            {
                return Err(format!("Move 失败，目标已存在：{}", to.to_string_lossy()));
            }
            tokio::fs::rename(from, to).await.map_err(|err| {
                format!(
                    "Move 失败（{} -> {}）：{err}",
                    from.to_string_lossy(),
                    to.to_string_lossy()
                )
            })?;
            Ok(serde_json::json!({
                "op": "move",
                "from": terminal_path_for_user(from),
                "to": terminal_path_for_user(to),
            }))
        }
        ApplyPatchResolvedOp::Update { path, old_string, new_string, replace_all } => {
            let raw = tokio::fs::read(path)
                .await
                .map_err(|_| format!("Update 操作失败，文件不存在：{}", path.to_string_lossy()))?;
            let decoded = decode_text_file_bytes(&raw)
                .map_err(|err| format!("Update 操作失败，{}：{}", err, path.to_string_lossy()))?;
            let update_result = apply_patch_apply_update(
                &decoded.text,
                old_string,
                new_string,
                *replace_all,
            )?;
            let encoded = decoded
                .encode_like_original(&update_result.content)
                .map_err(|err| format!("Update 操作失败，{}：{}", err, path.to_string_lossy()))?;
            tokio::fs::write(path, encoded)
                .await
                .map_err(|err| format!("更新文件失败（{}）：{err}", path.to_string_lossy()))?;
            Ok(serde_json::json!({
                "op": "update",
                "path": terminal_path_for_user(path),
                "matchCount": update_result.match_line_ranges.len(),
                "lineRanges": update_result
                    .match_line_ranges
                    .iter()
                    .map(|(start, end)| serde_json::json!({ "start": start, "end": end }))
                    .collect::<Vec<_>>(),
            }))
        }
    }
}

async fn apply_patch_execute_ops(
    data_path: &PathBuf,
    record: &mut ApplyPatchBackupRecord,
    ops: &[ApplyPatchResolvedOp],
) -> Result<ApplyPatchExecutionOutcome, String> {
    let mut changed = Vec::<Value>::new();
    for (index, op) in ops.iter().enumerate() {
        let entry = match apply_patch_prepare_backup_entry(data_path, op) {
            Ok(value) => value,
            Err(message) => {
                // prepare_backup_entry 返回 Err 有两种情况：
                // 1. 操作级失败（文件不存在等）：此时无 blob 被创建，changed 为空
                // 2. 系统级故障（写 blob I/O 失败）：部分 blob 可能已写入
                // 两种情况都返回 Ok(failure)，调用方根据 changed/entries 是否为空决定是否保留备份
                return Ok(ApplyPatchExecutionOutcome {
                    changed,
                    failure: Some(ApplyPatchExecutionFailure {
                        index,
                        op: apply_patch_op_name(op).to_string(),
                        path: apply_patch_op_path(op),
                        message,
                    }),
                });
            }
        };
        match apply_patch_execute_single_op(op).await {
            Ok(value) => {
                record.entries.push(entry);
                changed.push(value);
            }
            Err(message) => {
                let _ = apply_patch_cleanup_backup_entry(data_path, &entry);
                return Ok(ApplyPatchExecutionOutcome {
                    changed,
                    failure: Some(ApplyPatchExecutionFailure {
                        index,
                        op: apply_patch_op_name(op).to_string(),
                        path: apply_patch_op_path(op),
                        message,
                    }),
                });
            }
        }
    }
    Ok(ApplyPatchExecutionOutcome { changed, failure: None })
}

async fn builtin_apply_patch_with_name(
    state: &AppState,
    session_id: &str,
    args: ApplyPatchToolArgs,
) -> Result<Value, String> {
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let cwd = resolve_terminal_cwd(state, &normalized_session, None)?;
    let raw_input = apply_patch_tool_args_to_raw_json(&args)?;
    let parsed = apply_patch_ops_from_tool_args(args)?;
    let resolved = apply_patch_resolve_ops(&cwd, parsed)?;

    let safety = apply_patch_assess_safety(state, &normalized_session, &cwd, &resolved)?;
    if let ApplyPatchSafetyCheck::Reject { reason } = safety {
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "rejected",
            "message": reason,
            "cwd": terminal_path_for_user(&cwd),
        }));
    }

    let mut backup_record = apply_patch_empty_backup_record(
        &state.data_path,
        &normalized_session,
        &cwd,
        &raw_input,
    )?;
    let started = std::time::Instant::now();
    let outcome = match apply_patch_execute_ops(&state.data_path, &mut backup_record, &resolved).await {
        Ok(value) => value,
        Err(err) => {
            let _ = apply_patch_cleanup_backup_record_by_value(&state.data_path, &backup_record);
            return Err(err);
        }
    };
    let record_path = if backup_record.entries.is_empty() {
        None
    } else {
        match apply_patch_store_backup_record(state, &backup_record) {
            Ok(path) => Some(path),
            Err(err) => {
                let _ = apply_patch_cleanup_backup_record_by_value(&state.data_path, &backup_record);
                return Err(err);
            }
        }
    };
    runtime_log_info(format!(
        "[补丁执行] 备份记录状态，任务=apply_patch，session={}，entry_count={}，record_id={}，record_path={}，changed_count={}",
        normalized_session,
        backup_record.entries.len(),
        backup_record.record_id,
        record_path
            .as_ref()
            .map(|path| terminal_path_for_user(path))
            .unwrap_or_else(|| "(none)".to_string()),
        outcome.changed.len()
    ));
    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    if let Some(failure) = outcome.failure {
        runtime_log_error(format!(
            "[补丁执行] 失败 task=apply_patch session={} changed={} failed_index={} elapsed_ms={} record_id={}",
            normalized_session,
            outcome.changed.len(),
            failure.index,
            elapsed_ms,
            backup_record.record_id
        ));
        return Ok(serde_json::json!({
            "ok": false,
            "approved": true,
            "partial": !outcome.changed.is_empty(),
            "cwd": terminal_path_for_user(&cwd),
            "changed": outcome.changed,
            "failed": {
                "index": failure.index,
                "op": failure.op,
                "path": failure.path,
                "message": failure.message,
            },
            "backupRecordId": record_path.as_ref().map(|_| backup_record.record_id.clone()),
            "backupRecordPath": record_path.as_ref().map(|path| terminal_path_for_user(path)),
        }));
    }
    runtime_log_info(format!(
        "[补丁执行] 完成 task=apply_patch session={} changed={} elapsed_ms={} record_id={}",
        normalized_session,
        outcome.changed.len(),
        elapsed_ms,
        backup_record.record_id
    ));
    Ok(serde_json::json!({
        "ok": true,
        "approved": true,
        "cwd": terminal_path_for_user(&cwd),
        "changed": outcome.changed,
        "backupRecordId": record_path.as_ref().map(|_| backup_record.record_id.clone()),
        "backupRecordPath": record_path.as_ref().map(|path| terminal_path_for_user(path)),
    }))
}

async fn builtin_write_file(
    state: &AppState,
    session_id: &str,
    args: WriteFileToolArgs,
) -> Result<Value, String> {
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    let cwd = resolve_terminal_cwd(state, &normalized_session, None)?;
    let target_path = apply_patch_resolve_path(&cwd, &args.path)?;
    if target_path.exists() && !args.overwrite {
        return Err(format!(
            "write 失败，文件已存在：{}。若要整份替换已有内容，请设置 overwrite 为 true。",
            terminal_path_for_user(&target_path)
        ));
    }
    let action = if target_path.exists() { "overwrite" } else { "add" }.to_string();
    let content = Some(args.content);
    let (old_string, new_string) = (None, None);
    builtin_apply_patch_with_name(
        state,
        session_id,
        ApplyPatchToolArgs {
            operations: vec![ApplyPatchToolOpArgs {
                action,
                path: args.path,
                content,
                old_string,
                new_string,
                replace_all: None,
                to: None,
            }],
        },
    )
    .await
}

async fn builtin_delete_file(
    state: &AppState,
    session_id: &str,
    args: DeleteFileToolArgs,
) -> Result<Value, String> {
    builtin_apply_patch_with_name(
        state,
        session_id,
        ApplyPatchToolArgs {
            operations: vec![ApplyPatchToolOpArgs {
                action: "delete".to_string(),
                path: args.path,
                content: None,
                old_string: None,
                new_string: None,
                replace_all: None,
                to: None,
            }],
        },
    )
    .await
}

async fn builtin_update_file(
    state: &AppState,
    session_id: &str,
    args: UpdateFileToolArgs,
) -> Result<Value, String> {
    builtin_apply_patch_with_name(
        state,
        session_id,
        ApplyPatchToolArgs {
            operations: vec![ApplyPatchToolOpArgs {
                action: "update".to_string(),
                path: args.path,
                content: None,
                old_string: Some(args.old_string),
                new_string: Some(args.new_string),
                replace_all: args.replace_all,
                to: None,
            }],
        },
    )
    .await
}

async fn builtin_move_file(
    state: &AppState,
    session_id: &str,
    args: MoveFileToolArgs,
) -> Result<Value, String> {
    builtin_apply_patch_with_name(
        state,
        session_id,
        ApplyPatchToolArgs {
            operations: vec![ApplyPatchToolOpArgs {
                action: "move".to_string(),
                path: args.path,
                content: None,
                old_string: None,
                new_string: None,
                replace_all: None,
                to: Some(args.to),
            }],
        },
    )
    .await
}

#[cfg(test)]
mod apply_patch_tool_tests {
    use super::*;

    // ------------------------------------------------------------------
    // old_string 未命中时的候选定位：必须又快，且不能瞎猜。
    // 旧实现按窗口逐个计算整段编辑距离，一千行文件配三百行更新要十几分钟，
    // 五千行以上则直接不返回任何候选；这组测试把这两件事同时钉住。
    // ------------------------------------------------------------------

    fn similarity_fixture_lines(count: usize) -> Vec<String> {
        (0..count)
            .map(|index| match index % 5 {
                0 => format!("const ITEM_{index}: usize = {index};"),
                1 => format!("    self.handle_{index}(&mut ctx)"),
                2 => format!("// 第 {index} 段：滚动区高度按行数乘以 1.4 倍行高估算"),
                3 => format!("  color: #fb{}3c;", index % 90 + 10),
                _ => format!("  pub fn slot_{index}() -> Result<()> {{"),
            })
            .collect()
    }

    #[test]
    fn similar_line_ranges_should_locate_candidate_within_budget() {
        // 一万行文件、三百行更新。旧实现在这个规模上要跑几十分钟，而且超过 5000 行
        // 时直接返回空候选，模型什么提示都拿不到。
        let lines = similarity_fixture_lines(10_000);
        let content = lines.join("\n");
        let start = 4_200usize;
        let mut search = lines[start..start + 300].to_vec();
        search[7] = format!("{} // 改过一行", search[7]);
        let old_string = search.join("\n");

        let started = std::time::Instant::now();
        let found = apply_patch_similar_line_ranges(&content, &old_string);
        let elapsed = started.elapsed();

        assert_eq!(found.len(), 1, "一万行文件也应当给出唯一候选");
        assert_eq!(found[0].0, start + 1, "候选起点应当命中真实位置");
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "定位耗时 {elapsed:?} 超出预算：行级投票不应当随文件规模退化"
        );
    }

    #[test]
    fn similar_line_ranges_should_ignore_indentation_and_case() {
        // 缩进、行尾空白、大小写是手抄原文最常见的三类偏差，都不该妨碍定位。
        let lines = similarity_fixture_lines(400);
        let content = lines.join("\n");
        let start = 120usize;
        let search = lines[start..start + 60]
            .iter()
            .map(|line| format!("    {}   ", line.to_uppercase()))
            .collect::<Vec<_>>()
            .join("\n");

        let found = apply_patch_similar_line_ranges(&content, &search);
        assert_eq!(found.len(), 1, "空白与大小写差异不应妨碍定位");
        assert_eq!(found[0].0, start + 1, "起点应当命中真实位置");
    }

    #[test]
    fn similar_line_ranges_should_not_guess_on_unrelated_content() {
        let content = similarity_fixture_lines(600).join("\n");
        let unrelated = (0..80)
            .map(|index| format!("zzz unrelated payload {index} qqq"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            apply_patch_similar_line_ranges(&content, &unrelated).is_empty(),
            "完全无关的内容不应给出候选"
        );
    }

    #[test]
    fn similar_line_ranges_should_not_guess_when_every_line_is_identical() {
        // 整份文件全是同一行时票数会被摊平；此时指出任何位置都属于瞎猜。
        let content = "  return true;\n".repeat(800);
        let old_string = "  return true;\n  return false;\n  return true;";

        assert!(
            apply_patch_similar_line_ranges(&content, old_string).is_empty(),
            "到处都像的时候应当拒绝给候选，而不是随便指一处"
        );
    }

    #[test]
    fn crlf_file_should_be_edited_through_a_single_lf_view() {
        // 生产链路：磁盘上是 CRLF → 解码成 LF → 匹配一次 → 编码回 CRLF。
        // 这替代了曾经「先按原样匹配、失败后归一化重试」的两趟写法：
        // 失败那趟会先扫全文找候选，1000 行文件上实测一次 185 秒，重试成功后被丢掉。
        let decoded =
            decode_text_file_bytes("a\r\nold\r\nvalue\r\nc\r\n".as_bytes()).expect("decode crlf");
        assert_eq!(decoded.text, "a\nold\nvalue\nc\n");

        // 模型读到的是 LF，写回来的 old_string 也是 LF。
        let updated = apply_patch_apply_update(&decoded.text, "old\nvalue", "new\nvalue", false)
            .expect("LF old_string 应当直接在归一化后的内容上命中");
        assert_eq!(updated.match_line_ranges, vec![(2, 3)]);

        let written = decoded
            .encode_like_original(&updated.content)
            .expect("encode crlf");
        assert_eq!(written, b"a\r\nnew\r\nvalue\r\nc\r\n");
    }

    fn absolute_user_path(path: &Path) -> String {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    fn make_temp_data_path(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create config dir");
        root.join("config").join("config_mark")
    }

    #[test]
    fn tool_args_should_accept_snake_case_update_fields() {
        let args: ApplyPatchToolArgs = serde_json::from_str(
            r#"{"operations":[{"action":"update","path":"a.txt","old_string":"before","new_string":"after","replace_all":true}]}"#,
        )
        .expect("parse snake case args");
        let ops = apply_patch_ops_from_tool_args(args).expect("convert snake case args");
        let ApplyPatchOp::Update { old_string, new_string, replace_all, .. } = &ops[0] else {
            panic!("expected update op");
        };
        assert_eq!(old_string, "before");
        assert_eq!(new_string, "after");
        assert!(*replace_all);
    }

    #[test]
    fn tool_args_should_reject_empty_old_string_in_update() {
        let args: ApplyPatchToolArgs = serde_json::from_str(
            r#"{"operations":[{"action":"update","path":"a.txt","old_string":"","new_string":"after"}]}"#,
        )
        .expect("parse args");
        let err = apply_patch_ops_from_tool_args(args).expect_err("empty old_string should be rejected");
        assert!(err.contains("old_string"));
        assert!(err.contains("不能为空"));
    }

    #[test]
    fn tool_args_should_parse_overwrite_action() {
        let args: ApplyPatchToolArgs = serde_json::from_str(
            r#"{"operations":[{"action":"overwrite","path":"a.txt","content":"after"}]}"#,
        )
        .expect("parse args");
        let ops = apply_patch_ops_from_tool_args(args).expect("convert overwrite");
        let ApplyPatchOp::Overwrite { path, content } = &ops[0] else {
            panic!("expected overwrite op");
        };
        assert_eq!(path, "a.txt");
        assert_eq!(content, "after");
    }

    #[test]
    fn tool_args_should_normalize_crlf_in_update_strings() {
        // old_string 可能来自 terminal 输出、git show 或用户粘贴，带 \r；
        // 而文件内容是归一化后的 LF 视图。不在入口统一，这类 old_string 必然匹配失败。
        let args = serde_json::from_str(
            r#"{"operations":[{"action":"update","path":"a.txt","oldString":"a\r\nb","newString":"x\r\ny"}]}"#,
        )
        .expect("parse crlf args");
        let ops = apply_patch_ops_from_tool_args(args).expect("convert crlf args");
        let ApplyPatchOp::Update { old_string, new_string, .. } = &ops[0] else {
            panic!("expected update op");
        };
        assert_eq!(old_string, "a\nb");
        assert_eq!(new_string, "x\ny");
    }

    #[test]
    fn tool_args_should_accept_camel_case_update_fields() {
        let args: ApplyPatchToolArgs = serde_json::from_str(
            r#"{"operations":[{"action":"update","path":"a.txt","oldString":"before","newString":"after","replaceAll":true}]}"#,
        )
        .expect("parse camel case args");
        let ops = apply_patch_ops_from_tool_args(args).expect("convert camel case args");
        let ApplyPatchOp::Update { old_string, new_string, replace_all, .. } = &ops[0] else {
            panic!("expected update op");
        };
        assert_eq!(old_string, "before");
        assert_eq!(new_string, "after");
        assert!(*replace_all);
    }

    #[test]
    fn write_tool_args_should_default_overwrite_to_false() {
        let args: WriteFileToolArgs =
            serde_json::from_str(r#"{"path":"a.txt","content":"hello"}"#).expect("parse write args");
        assert!(!args.overwrite);
    }

    #[test]
    fn write_tool_args_should_accept_overwrite_true() {
        let args: WriteFileToolArgs =
            serde_json::from_str(r#"{"path":"a.txt","content":"hello","overwrite":true}"#)
                .expect("parse write args");
        assert!(args.overwrite);
    }

    #[test]
    fn delete_with_old_string_should_report_use_update_for_content_removal() {
        let args: ApplyPatchToolArgs = serde_json::from_str(
            r#"{"operations":[{"action":"delete","path":"a.txt","old_string":"before"}]}"#,
        )
        .expect("parse delete args");
        let err = apply_patch_ops_from_tool_args(args).expect_err("delete with old_string should fail");
        assert!(err.contains("delete) 只会删除整个文件"));
        assert!(err.contains("如果你想删除文件中的部分内容，请改用 update"));
    }

    #[test]
    fn apply_update_should_replace_single_occurrence() {
        let updated = apply_patch_apply_update("a\nb\nc\n", "b", "B", false).expect("apply");
        assert_eq!(updated.content, "a\nB\nc\n");
        assert_eq!(updated.match_line_ranges, vec![(2, 2)]);
    }

    #[test]
    fn apply_update_should_reject_empty_old_string() {
        let err = apply_patch_apply_update("old\ncontent\n", "", "new content\n", false)
            .expect_err("empty old_string should be rejected");
        assert!(err.contains("old_string 不能为空"));
        assert!(err.contains("请提供要替换的原文片段"));
        assert!(err.contains("最小 update 示例"));
    }

    #[test]
    fn apply_update_should_explain_not_found() {
        let err = apply_patch_apply_update("a\nb\nc\n", "b\nmissing", "x", false).expect_err("not found should fail");
        assert!(err.contains("old_string 在文件中未找到"));
        assert!(err.contains("最相似候选行范围"));
        // 文件里没有足够接近的窗口时明确说明没有候选，不再给一个伪造的 line 1。
        assert!(err.contains("未找到可用候选"));
        assert!(err.contains("b"));
        assert!(err.contains("先重新读取目标文件"));
        assert!(err.contains("最小 update 示例"));
    }

    #[test]
    fn apply_update_should_suggest_candidate_when_single_char_differs() {
        let content = "第一行\nMULTI第一行\n第三行\n";
        let err = apply_patch_apply_update(content, "MULTI第1行", "x", false)
            .expect_err("差一个字的原文同样应判定为未找到");
        assert!(err.contains("最相似候选行范围"));
        assert!(!err.contains("未找到可用候选"), "差一个字时不应落到兜底文案：{err}");
        assert!(err.contains("line 2"), "应给出真实候选行号：{err}");
    }

    #[test]
    fn apply_update_should_not_invent_candidate_for_unrelated_text() {
        let content = "alpha\nbeta\ngamma\n";
        let err = apply_patch_apply_update(content, "完全无关的内容XYZ", "x", false)
            .expect_err("无关内容应判定为未找到");
        assert!(err.contains("未找到可用候选"));
        assert!(!err.contains("line 1，未找到可用候选"), "兜底文案不应带假行号：{err}");
    }

    #[test]
    fn apply_update_should_still_fail_when_replace_all_true_but_old_string_missing() {
        let err = apply_patch_apply_update("a\nb\nc\n", "missing", "x", true)
            .expect_err("missing old_string should still fail");
        assert!(err.contains("old_string 在文件中未找到"));
    }

    #[test]
    fn apply_update_should_truncate_long_old_string_preview_in_error() {
        let old_string = "x".repeat(400);
        let err = apply_patch_apply_update("short", &old_string, "new", false)
            .expect_err("long old_string preview should fail");
        let preview = err
            .split("old_string 预览（前 300 字符）：\n")
            .nth(1)
            .expect("preview should exist");
        assert_eq!(preview.chars().count(), 300);
    }

    #[test]
    fn apply_update_should_reject_non_unique_without_replace_all() {
        let err = apply_patch_apply_update("target\nkeep\ntarget\nkeep\n", "target", "changed", false)
            .expect_err("ambiguous match should fail");
        assert!(err.contains("replace_all"));
        assert!(err.contains("2 次"));
        assert!(err.contains("命中行范围：line 1, line 3"));
        assert!(err.contains("扩大 old_string"));
        assert!(err.contains("全部替换"));
    }

    #[test]
    fn apply_update_should_limit_many_match_ranges() {
        let content = (0..20).map(|_| "target\n").collect::<String>();
        let err = apply_patch_apply_update(&content, "target", "changed", false)
            .expect_err("ambiguous match should fail");
        assert!(err.contains("出现了 20 次"));
        assert!(err.contains("另有 8 处"));
        assert!(!err.contains("line 20"));
    }

    #[test]
    fn apply_update_should_replace_all() {
        let updated = apply_patch_apply_update("target\nkeep\ntarget\nkeep\n", "target", "changed", true)
            .expect("replace_all apply");
        assert_eq!(updated.content, "changed\nkeep\nchanged\nkeep\n");
        assert_eq!(updated.match_line_ranges, vec![(1, 1), (3, 3)]);
    }

    #[tokio::test]
    async fn execute_ops_should_keep_prior_success_and_return_failed_operation() {
        let data_path = make_temp_data_path("apply-patch-partial");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let first = cwd.join("first.txt");
        let second = cwd.join("second.txt");
        std::fs::write(&first, "old\n").expect("seed first");
        std::fs::write(&second, "still old\n").expect("seed second");

        let ops = vec![
            ApplyPatchResolvedOp::Update {
                path: first.clone(),
                old_string: "old".to_string(),
                new_string: "new".to_string(),
                replace_all: false,
            },
            ApplyPatchResolvedOp::Update {
                path: second.clone(),
                old_string: "missing".to_string(),
                new_string: "new".to_string(),
                replace_all: false,
            },
        ];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute should return structured outcome");

        assert_eq!(std::fs::read_to_string(&first).expect("read first"), "new\n");
        assert_eq!(std::fs::read_to_string(&second).expect("read second"), "still old\n");
        assert_eq!(outcome.changed.len(), 1);
        // 行范围只报一份：lineRanges 是唯一的来源，不再额外给 lineStart / lineEnd。
        assert!(outcome.changed[0].get("lineStart").is_none());
        assert!(outcome.changed[0].get("lineEnd").is_none());
        assert_eq!(outcome.changed[0]["lineRanges"][0]["start"], 1);
        assert_eq!(outcome.changed[0]["lineRanges"][0]["end"], 1);
        assert_eq!(record.entries.len(), 1);
        let failure = outcome.failure.expect("second op should fail");
        assert_eq!(failure.index, 1);
        assert_eq!(failure.op, "update");
        assert!(failure.message.contains("old_string 在文件中未找到"));
        assert!(failure.message.contains("最相似候选行范围"));
    }

    #[tokio::test]
    async fn execute_ops_should_update_gbk_file_without_converting_to_utf8() {
        let data_path = make_temp_data_path("apply-patch-gbk");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let file = cwd.join("gbk.txt");
        std::fs::write(&file, [0xd6, 0xd0, 0xce, 0xc4, b'\n', b'o', b'l', b'd', b'\n'])
            .expect("seed gbk file");
        let ops = vec![ApplyPatchResolvedOp::Update {
            path: file.clone(),
            old_string: "old".to_string(),
            new_string: "new".to_string(),
            replace_all: false,
        }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute");

        assert!(outcome.failure.is_none());
        assert_eq!(
            std::fs::read(&file).expect("read updated gbk"),
            vec![0xd6, 0xd0, 0xce, 0xc4, b'\n', b'n', b'e', b'w', b'\n']
        );
    }

    #[tokio::test]
    async fn execute_ops_should_reject_empty_old_string_and_leave_file_untouched() {
        let data_path = make_temp_data_path("apply-patch-reject-empty-old");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let file = cwd.join("overwrite.txt");
        std::fs::write(&file, "old\ncontent\n").expect("seed file");
        let ops = vec![ApplyPatchResolvedOp::Update {
            path: file.clone(),
            old_string: String::new(),
            new_string: "new\ncontent\n".to_string(),
            replace_all: false,
        }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute");

        let failure = outcome.failure.expect("empty old_string should fail");
        assert_eq!(failure.op, "update");
        assert!(failure.message.contains("old_string 不能为空"));
        assert!(outcome.changed.is_empty());
        assert_eq!(
            std::fs::read_to_string(&file).expect("read untouched"),
            "old\ncontent\n"
        );
        assert!(record.entries.is_empty(), "失败时不应产生备份条目");
    }

    #[tokio::test]
    async fn execute_ops_should_overwrite_file_and_backup_previous_content() {
        let data_path = make_temp_data_path("apply-patch-overwrite-op");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let file = cwd.join("overwrite.txt");
        std::fs::write(&file, "old\ncontent\n").expect("seed file");
        let ops = vec![ApplyPatchResolvedOp::Overwrite {
            path: file.clone(),
            content: "new\ncontent\n".to_string(),
        }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute");

        assert!(outcome.failure.is_none());
        assert_eq!(outcome.changed[0]["op"], "overwrite");
        assert_eq!(
            std::fs::read_to_string(&file).expect("read overwritten"),
            "new\ncontent\n"
        );
        assert_eq!(record.entries.len(), 1);
        assert_eq!(record.entries[0].kind, ApplyPatchBackupKind::Update);
        assert_eq!(
            record.entries[0].expected_current_content.as_deref(),
            Some("new\ncontent\n")
        );

        apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(
            std::fs::read_to_string(&file).expect("read restored"),
            "old\ncontent\n"
        );
    }

    #[tokio::test]
    async fn execute_ops_should_keep_crlf_when_overwriting_existing_file() {
        // 覆盖一个 CRLF 文件时，模型给的 content 是 LF。若照原样写下去，
        // 整份文件的行尾会被刷成 LF，diff 上等于每一行都改了。
        let data_path = make_temp_data_path("apply-patch-overwrite-crlf");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let file = cwd.join("crlf.txt");
        std::fs::write(&file, b"old\r\ncontent\r\n").expect("seed crlf file");
        let ops = vec![ApplyPatchResolvedOp::Overwrite {
            path: file.clone(),
            content: "new\ncontent\n".to_string(),
        }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute overwrite");

        assert_eq!(
            std::fs::read(&file).expect("read overwritten"),
            b"new\r\ncontent\r\n"
        );
    }

    #[tokio::test]
    async fn execute_ops_should_update_crlf_file_with_lf_old_string() {
        // 事故现场：ActivityTimeline.vue 是 CRLF，模型给的 old_string 是 LF。
        // 这条钉住整条生产链路：解码归一化 → 单次匹配 → 写回还原 CRLF → 撤回还原原字节。
        let data_path = make_temp_data_path("apply-patch-update-crlf");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let file = cwd.join("timeline.vue");
        std::fs::write(&file, b"<template>\r\n  <div>old</div>\r\n</template>\r\n")
            .expect("seed crlf file");
        let ops = vec![ApplyPatchResolvedOp::Update {
            path: file.clone(),
            // 必须跨行：单行 old_string 不含 \n，CRLF/LF 差异根本不会暴露出来。
            old_string: "<template>\n  <div>old</div>".to_string(),
            new_string: "<template>\n  <div>new</div>".to_string(),
            replace_all: false,
        }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute update");
        if let Some(failure) = &outcome.failure {
            panic!("LF 写法的 old_string 应当直接命中 CRLF 文件：{}", failure.message);
        }
        assert_eq!(
            std::fs::read(&file).expect("read updated"),
            b"<template>\r\n  <div>new</div>\r\n</template>\r\n",
            "没被触碰的行也必须保持 CRLF"
        );

        apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(
            std::fs::read(&file).expect("read restored"),
            b"<template>\r\n  <div>old</div>\r\n</template>\r\n"
        );
    }

    #[tokio::test]
    async fn execute_ops_should_move_file_and_restore_original_path() {
        let data_path = make_temp_data_path("apply-patch-move-op");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let from = cwd.join("from.txt");
        let to = cwd.join("nested/to.txt");
        std::fs::write(&from, "content\n").expect("seed file");
        let ops = vec![ApplyPatchResolvedOp::Move {
            from: from.clone(),
            to: to.clone(),
        }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute");

        assert!(outcome.failure.is_none());
        assert_eq!(outcome.changed[0]["op"], "move");
        assert!(!from.exists(), "原路径应已移走");
        assert_eq!(std::fs::read_to_string(&to).expect("read moved"), "content\n");
        assert_eq!(record.entries.len(), 1);
        assert_eq!(record.entries[0].kind, ApplyPatchBackupKind::MoveUpdate);

        apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert!(from.exists(), "撤回后原路径应恢复");
        assert_eq!(
            std::fs::read_to_string(&from).expect("read restored"),
            "content\n"
        );
        assert!(!to.exists(), "撤回后目标路径应移除");
    }

    #[tokio::test]
    async fn execute_ops_should_reject_move_when_target_exists() {
        let data_path = make_temp_data_path("apply-patch-move-exists");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let from = cwd.join("from.txt");
        let to = cwd.join("to.txt");
        std::fs::write(&from, "from\n").expect("seed from");
        std::fs::write(&to, "existing\n").expect("seed to");
        let ops = vec![ApplyPatchResolvedOp::Move { from, to: to.clone() }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute");

        let failure = outcome.failure.expect("target exists should fail");
        assert_eq!(failure.op, "move");
        assert!(failure.message.contains("目标已存在"));
        assert_eq!(std::fs::read_to_string(&to).expect("read target"), "existing\n");
    }

    #[tokio::test]
    async fn execute_ops_should_delete_file_via_trash_and_keep_blob_backup() {
        let data_path = make_temp_data_path("apply-patch-trash");
        let cwd = app_root_from_data_path(&data_path).join("workspace");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let file = cwd.join("trash.txt");
        std::fs::write(&file, "to be trashed\n").expect("seed file");
        let ops = vec![ApplyPatchResolvedOp::Delete { path: file.clone() }];
        let mut record = apply_patch_empty_backup_record(&data_path, "s1", &cwd, "raw")
            .expect("empty record");

        let outcome = apply_patch_execute_ops(&data_path, &mut record, &ops)
            .await
            .expect("execute");

        assert!(outcome.failure.is_none());
        assert_eq!(outcome.changed.len(), 1);
        assert_eq!(outcome.changed[0]["op"], "delete");
        assert!(!file.exists(), "原文件应已移出原位");
        assert_eq!(record.entries.len(), 1);
        assert_eq!(record.entries[0].kind, ApplyPatchBackupKind::Delete);
        let blob_file = record.entries[0]
            .backup_blob_file
            .as_deref()
            .expect("delete entry should have blob");
        let blob_path = apply_patch_blob_path(&data_path, blob_file);
        assert!(blob_path.exists(), "删除前备份 blob 应存在");
        assert_eq!(
            std::fs::read(&blob_path).expect("read blob"),
            b"to be trashed\n"
        );
    }

    #[test]
    fn resolve_path_should_allow_relative_path_from_cwd() {
        let base = std::env::temp_dir().join(format!("eca-apply-patch-tests-{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&base);
        let result = apply_patch_resolve_path(&base, "relative.txt");
        assert_eq!(
            result.expect("resolve"),
            terminal_normalize_for_access_check(&base.join("relative.txt"))
        );
    }

    #[test]
    fn resolve_path_should_allow_absolute_path_outside_workspace() {
        let base = std::env::temp_dir().join(format!("eca-apply-patch-base-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("eca-apply-patch-outside-{}.txt", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&base);
        let result = apply_patch_resolve_path(&base, &outside.to_string_lossy());
        assert_eq!(result.expect("resolve"), terminal_normalize_for_access_check(&outside));
    }

    #[test]
    fn resolve_ops_should_resolve_move_into_move_op() {
        let cwd = std::env::temp_dir().join(format!("eca-apply-patch-move-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&cwd).expect("create cwd");
        let ops = vec![ApplyPatchOp::Move {
            path: "from.txt".to_string(),
            to: "nested/to.txt".to_string(),
        }];
        let resolved = apply_patch_resolve_ops(&cwd, ops).expect("resolve move");
        match &resolved[0] {
            ApplyPatchResolvedOp::Move { from, to } => {
                assert_eq!(
                    from,
                    &terminal_normalize_for_access_check(&cwd.join("from.txt"))
                );
                assert_eq!(
                    to,
                    &terminal_normalize_for_access_check(&cwd.join("nested/to.txt"))
                );
            }
            _ => panic!("expected move to resolve as move op"),
        }
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
        let delete_path = absolute_user_path(&cwd.join("delete.txt"));
        let update_path = absolute_user_path(&cwd.join("update.txt"));
        let move_from_path = absolute_user_path(&cwd.join("move.txt"));
        let move_to_path = cwd.join("moved.txt").to_string_lossy().to_string();
        let input = serde_json::json!({
            "operations": [
                {"action": "delete", "path": delete_path},
                {"action": "update", "path": update_path, "old_string": "old", "new_string": "new"},
                {"action": "move", "path": move_from_path, "to": move_to_path}
            ]
        })
        .to_string();
        let args: ApplyPatchToolArgs = serde_json::from_str(&input).expect("parse tool args");
        let ops = apply_patch_resolve_ops(&cwd, apply_patch_ops_from_tool_args(args).expect("convert")).expect("resolve");
        let record = apply_patch_prepare_backup_record(&data_path, "s1", &cwd, &input, &ops)
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
        let (restored, overwritten) = apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(restored, 1);
        assert!(overwritten.is_empty());
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
        let (restored, overwritten) = apply_patch_restore_backup_record(&data_path, &record).expect("restore");
        assert_eq!(restored, 1);
        assert!(overwritten.is_empty());
        assert_eq!(std::fs::read_to_string(&path).expect("read restored"), "old\n");
    }
}
