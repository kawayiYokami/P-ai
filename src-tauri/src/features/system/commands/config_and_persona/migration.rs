const MIGRATION_SCHEMA_VERSION: u32 = 1;
const MIGRATION_MANIFEST_FILE_NAME: &str = "manifest.json";
const MIGRATION_PAYLOAD_FILE_NAME: &str = "payload.json";
const MIGRATION_PASSWORD_REQUIRED_CODE: &str = "MIGRATION_PASSWORD_REQUIRED";
const MIGRATION_PASSWORD_REQUIRED_MESSAGE: &str = "该迁移包需要密码，请先输入解压密码。";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationCommandError {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

impl MigrationCommandError {
    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    fn coded(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some(code.into()),
        }
    }
}

impl From<String> for MigrationCommandError {
    fn from(value: String) -> Self {
        Self::message(value)
    }
}

impl From<&str> for MigrationCommandError {
    fn from(value: &str) -> Self {
        Self::message(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationManifest {
    schema_version: u32,
    app_version: String,
    exported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationBlobFile {
    key: String,
    relative_path: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationPayload {
    config: AppConfig,
    runtime_data: AppData,
    memories: Vec<MemoryEntry>,
    oauth_files: Vec<MigrationBlobFile>,
    avatar_files: Vec<MigrationBlobFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportConfigMigrationPackageInput {
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportConfigMigrationPackageResult {
    path: String,
    provider_count: usize,
    api_config_count: usize,
    memory_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImportConfigMigrationPackageInput {
    password: String,
    package_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImportConfigMigrationPackageResult {
    preview_id: String,
    package_version: String,
    memory_added_count: usize,
    memory_merged_count: usize,
    provider_added_count: usize,
    provider_updated_count: usize,
    api_config_added_count: usize,
    api_config_updated_count: usize,
    oauth_file_count: usize,
    avatar_file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyImportConfigMigrationPackageInput {
    preview_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyImportConfigMigrationPackageResult {
    imported_memory_count: usize,
    created_memory_count: usize,
    merged_memory_count: usize,
    provider_added_count: usize,
    provider_updated_count: usize,
    api_config_added_count: usize,
    api_config_updated_count: usize,
    backup_dir: String,
}

fn validate_migration_password(password: &str) -> Result<(), String> {
    let trimmed = password.trim();
    if trimmed.is_empty() {
        return Err("迁移密码不能为空".to_string());
    }
    if trimmed.chars().count() < 6 {
        return Err("迁移密码不能少于 6 位".to_string());
    }
    Ok(())
}

fn validate_export_migration_password(password: &str) -> Result<(), String> {
    validate_migration_password(password)
}

fn migration_temp_root(state: &AppState) -> PathBuf {
    app_root_from_data_path(&state.data_path)
        .join("temp")
        .join("migration")
}

fn migration_preview_dir(state: &AppState, preview_id: &str) -> PathBuf {
    migration_temp_root(state).join(preview_id)
}

fn migration_backup_dir(state: &AppState) -> PathBuf {
    app_layout_backups_dir(&state.data_path)
        .join("migration")
        .join(now_iso().replace(':', "-"))
}

async fn migration_save_path(app: &AppHandle) -> Result<PathBuf, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();
    app.dialog()
        .file()
        .add_filter("P-AI Migration", &["zip"])
        .save_file(move |file| {
            let path = file.and_then(|item| item.as_path().map(ToOwned::to_owned));
            let _ = tx.send(path);
        });
    rx.await
        .map_err(|err| format!("等待导出文件选择结果失败: {err}"))?
        .ok_or_else(|| "导出已取消".to_string())
}

async fn migration_import_path(app: &AppHandle) -> Result<PathBuf, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();
    app.dialog()
        .file()
        .add_filter("P-AI Migration", &["zip"])
        .pick_file(move |file| {
            let path = file.and_then(|item| item.as_path().map(ToOwned::to_owned));
            let _ = tx.send(path);
        });
    rx.await
        .map_err(|err| format!("等待导入文件选择结果失败: {err}"))?
        .ok_or_else(|| "导入已取消".to_string())
}

async fn resolve_migration_import_path(
    app: &AppHandle,
    input: &PreviewImportConfigMigrationPackageInput,
) -> Result<PathBuf, String> {
    let provided = input.package_path.as_deref().unwrap_or("").trim();
    if !provided.is_empty() {
        return Ok(PathBuf::from(provided));
    }
    migration_import_path(app).await
}

fn zip_error_requires_password(err: &zip::result::ZipError) -> bool {
    let text = err.to_string().to_ascii_lowercase();
    text.contains("password")
        || text.contains("encrypted")
        || text.contains("aes")
        || text.contains("unsupported archive: aes")
}

fn sanitize_migration_zip_entry_path(name: &str) -> Result<PathBuf, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("迁移包中存在空文件路径。".to_string());
    }

    let mut safe = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            std::path::Component::Normal(part) => safe.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(format!("迁移包包含非法路径（禁止 ..）: {trimmed}"));
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(format!("迁移包包含非法绝对路径: {trimmed}"));
            }
        }
    }

    if safe.as_os_str().is_empty() {
        return Err(format!("迁移包包含非法路径: {trimmed}"));
    }

    Ok(safe)
}

fn encode_base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(input.trim())
        .map_err(|err| format!("解码迁移文件失败: {err}"))
}

fn copy_dir_recursive_if_exists(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(src) {
        let entry = entry.map_err(|err| format!("遍历目录失败: {err}"))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .map_err(|err| format!("计算相对路径失败: {err}"))?;
        let target = dst.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|err| format!("创建目录失败 ({}): {err}", target.display()))?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("创建父目录失败 ({}): {err}", parent.display()))?;
            }
            fs::copy(path, &target).map_err(|err| {
                format!(
                    "复制文件失败 ({} -> {}): {err}",
                    path.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn copy_file_if_exists(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建父目录失败 ({}): {err}", parent.display()))?;
    }
    fs::copy(src, dst).map_err(|err| {
        format!(
            "复制文件失败 ({} -> {}): {err}",
            src.display(),
            dst.display()
        )
    })?;
    Ok(())
}

fn backup_current_migration_targets(state: &AppState) -> Result<PathBuf, String> {
    let backup_dir = migration_backup_dir(state);
    fs::create_dir_all(&backup_dir)
        .map_err(|err| format!("创建迁移备份目录失败 ({}): {err}", backup_dir.display()))?;

    copy_file_if_exists(&state.config_path, &backup_dir.join("app_config.toml"))?;
    copy_file_if_exists(
        &app_layout_agents_path(&state.data_path),
        &backup_dir.join("layout").join("config").join("agents.json"),
    )?;
    copy_file_if_exists(
        &app_layout_runtime_state_path(&state.data_path),
        &backup_dir
            .join("layout")
            .join("state")
            .join("runtime_state.json"),
    )?;
    copy_file_if_exists(
        &app_layout_chat_index_path(&state.data_path),
        &backup_dir.join("layout").join("chat").join("chat_index.json"),
    )?;
    copy_file_if_exists(
        &memory_store_db_path(&state.data_path),
        &backup_dir.join("memory").join("memory.db"),
    )?;
    copy_dir_recursive_if_exists(&avatar_storage_dir(state)?, &backup_dir.join("avatars"))?;
    copy_dir_recursive_if_exists(
        &codex_auth_storage_root()?,
        &backup_dir.join("auth").join("codex"),
    )?;

    Ok(backup_dir)
}

fn collect_avatar_files_from_agents(
    state: &AppState,
    agents: &mut [AgentProfile],
) -> Result<Vec<MigrationBlobFile>, String> {
    let avatars_root = avatar_storage_dir(state)?;
    let canonical_root = std::fs::canonicalize(&avatars_root).unwrap_or(avatars_root.clone());
    let mut files = Vec::<MigrationBlobFile>::new();

    for agent in agents.iter_mut() {
        let Some(avatar_path) = agent.avatar_path.as_ref() else {
            continue;
        };
        let path = PathBuf::from(avatar_path);
        let canonical = match std::fs::canonicalize(&path) {
            Ok(value) => value,
            Err(_) => {
                agent.avatar_path = None;
                agent.avatar_updated_at = None;
                continue;
            }
        };
        if !canonical.starts_with(&canonical_root) {
            agent.avatar_path = None;
            agent.avatar_updated_at = None;
            continue;
        }
        let bytes = std::fs::read(&canonical)
            .map_err(|err| format!("读取头像文件失败 ({}): {err}", canonical.display()))?;
        files.push(MigrationBlobFile {
            key: agent.id.clone(),
            relative_path: format!("avatars/{}.webp", sanitize_avatar_key(&agent.id)),
            bytes_base64: encode_base64(&bytes),
        });
        agent.avatar_path = None;
    }

    Ok(files)
}

fn collect_oauth_files_from_providers(config: &AppConfig) -> Result<Vec<MigrationBlobFile>, String> {
    let mut files = Vec::<MigrationBlobFile>::new();
    let mut seen = std::collections::HashSet::<String>::new();

    for provider in &config.api_providers {
        if provider.request_format != RequestFormat::Codex {
            continue;
        }

        let managed_path = managed_codex_auth_path(&provider.id)?;
        if managed_path.exists() && seen.insert(managed_path.to_string_lossy().to_string()) {
            files.push(MigrationBlobFile {
                key: format!("managed:{}", provider.id),
                relative_path: format!("oauth/managed/{}.json", provider.id),
                bytes_base64: encode_base64(
                    &std::fs::read(&managed_path).map_err(|err| {
                        format!(
                            "读取托管授权文件失败 ({}): {err}",
                            managed_path.display()
                        )
                    })?,
                ),
            });
        }
    }

    Ok(files)
}

fn build_export_payload(state: &AppState) -> Result<MigrationPayload, String> {
    let mut config = state_read_config_cached(state)?;
    let mut runtime_data = state_read_app_data_cached(state)?;
    let memories = memory_store_list_memories(&state.data_path)?;

    config.shell_workspaces = Vec::new();
    for channel in &mut config.remote_im_channels {
        channel.credentials = serde_json::Value::Object(serde_json::Map::new());
    }

    let avatar_files = collect_avatar_files_from_agents(state, &mut runtime_data.agents)?;
    let oauth_files = collect_oauth_files_from_providers(&config)?;

    runtime_data.main_conversation_id = None;
    runtime_data.conversations.clear();
    runtime_data.archived_conversations.clear();
    runtime_data.image_text_cache.clear();
    runtime_data.pdf_text_cache.clear();
    runtime_data.pdf_image_cache.clear();
    runtime_data.remote_im_contacts.clear();

    Ok(MigrationPayload {
        config,
        runtime_data,
        memories,
        oauth_files,
        avatar_files,
    })
}

fn write_migration_package(
    path: &Path,
    password: &str,
    manifest: &MigrationManifest,
    payload: &MigrationPayload,
) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|err| format!("创建迁移包失败 ({}): {err}", path.display()))?;
    let mut writer = zip::ZipWriter::new(file);

    writer
        .start_file(
            MIGRATION_MANIFEST_FILE_NAME,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .with_aes_encryption(zip::AesMode::Aes256, password),
        )
        .map_err(|err| format!("写入迁移清单失败: {err}"))?;
    writer
        .write_all(
            &serde_json::to_vec_pretty(manifest)
                .map_err(|err| format!("序列化迁移清单失败: {err}"))?,
        )
        .map_err(|err| format!("写入迁移清单内容失败: {err}"))?;

    writer
        .start_file(
            MIGRATION_PAYLOAD_FILE_NAME,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .with_aes_encryption(zip::AesMode::Aes256, password),
        )
        .map_err(|err| format!("写入迁移数据失败: {err}"))?;
    writer
        .write_all(
            &serde_json::to_vec_pretty(payload)
                .map_err(|err| format!("序列化迁移数据失败: {err}"))?,
        )
        .map_err(|err| format!("写入迁移数据内容失败: {err}"))?;

    writer.finish().map_err(|err| format!("完成迁移包写入失败: {err}"))?;
    Ok(())
}

fn unzip_migration_package_to_dir(
    package_path: &Path,
    password: &str,
    target_dir: &Path,
) -> Result<(), MigrationCommandError> {
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir).map_err(|err| {
            format!("清理旧迁移临时目录失败 ({}): {err}", target_dir.display())
        })?;
    }
    std::fs::create_dir_all(target_dir).map_err(|err| {
        format!("创建迁移临时目录失败 ({}): {err}", target_dir.display())
    })?;

    let reader = std::io::Cursor::new(
        std::fs::read(package_path)
            .map_err(|err| format!("读取迁移包失败 ({}): {err}", package_path.display()))?,
    );
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|err| format!("打开迁移包失败: {err}"))?;

    for index in 0..archive.len() {
        let mut file = if password.trim().is_empty() {
            match archive.by_index(index) {
                Ok(file) => file,
                Err(err) => {
                    if zip_error_requires_password(&err) {
                        return Err(MigrationCommandError::coded(
                            MIGRATION_PASSWORD_REQUIRED_CODE,
                            MIGRATION_PASSWORD_REQUIRED_MESSAGE,
                        ));
                    }
                    return Err(MigrationCommandError::message(format!(
                        "读取迁移包条目失败: {err}"
                    )));
                }
            }
        } else {
            archive
                .by_index_decrypt(index, password.as_bytes())
                .map_err(|err| format!("解密迁移包失败: {err}"))?
        };
        let relative_path = sanitize_migration_zip_entry_path(file.name())?;
        let output = target_dir.join(&relative_path);
        if file.is_dir() {
            std::fs::create_dir_all(&output)
                .map_err(|err| format!("创建迁移临时目录失败 ({}): {err}", output.display()))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("创建迁移临时目录失败 ({}): {err}", parent.display()))?;
        }
        let mut body = Vec::<u8>::new();
        std::io::Read::read_to_end(&mut file, &mut body)
            .map_err(|err| format!("读取迁移包内容失败: {err}"))?;
        std::fs::write(&output, body)
            .map_err(|err| format!("写入迁移临时文件失败 ({}): {err}", output.display()))?;
    }

    Ok(())
}

fn read_preview_payload(preview_dir: &Path) -> Result<(MigrationManifest, MigrationPayload), String> {
    let manifest = serde_json::from_str::<MigrationManifest>(
        &std::fs::read_to_string(preview_dir.join(MIGRATION_MANIFEST_FILE_NAME))
            .map_err(|err| format!("读取迁移清单失败: {err}"))?,
    )
    .map_err(|err| format!("解析迁移清单失败: {err}"))?;
    let payload = serde_json::from_str::<MigrationPayload>(
        &std::fs::read_to_string(preview_dir.join(MIGRATION_PAYLOAD_FILE_NAME))
            .map_err(|err| format!("读取迁移数据失败: {err}"))?,
    )
    .map_err(|err| format!("解析迁移数据失败: {err}"))?;
    Ok((manifest, payload))
}

fn assert_manifest_version(manifest: &MigrationManifest) -> Result<(), String> {
    if manifest.schema_version != MIGRATION_SCHEMA_VERSION {
        return Err(format!(
            "迁移包 schema 不匹配：expected={}, actual={}",
            MIGRATION_SCHEMA_VERSION, manifest.schema_version
        ));
    }
    let current_version = env!("CARGO_PKG_VERSION");
    if manifest.app_version.trim() != current_version {
        return Err(format!(
            "迁移包版本不匹配：当前版本为 {}，迁移包版本为 {}。仅允许同版本导入。",
            current_version, manifest.app_version
        ));
    }
    Ok(())
}

fn preview_memory_import(
    state: &AppState,
    preview_dir: &Path,
    memories: &[MemoryEntry],
) -> Result<ImportMemoriesResult, String> {
    let temp_data_path = preview_dir.join("memory-preview").join("app_data.json");
    if let Some(parent) = temp_data_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("创建记忆预检目录失败 ({}): {err}", parent.display()))?;
    }

    let current_db = memory_store_db_path(&state.data_path);
    let temp_db = memory_store_db_path(&temp_data_path);
    if let Some(parent) = temp_db.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!("创建记忆预检数据库目录失败 ({}): {err}", parent.display())
        })?;
    }
    if current_db.exists() {
        std::fs::copy(&current_db, &temp_db).map_err(|err| {
            format!(
                "复制记忆数据库失败 ({} -> {}): {err}",
                current_db.display(),
                temp_db.display()
            )
        })?;
    }

    let stats = memory_store_import_memories(&temp_data_path, memories)?;
    Ok(ImportMemoriesResult {
        imported_count: stats.imported_count,
        created_count: stats.created_count,
        merged_count: stats.merged_count,
        total_count: stats.total_count,
    })
}

fn merge_unique_api_keys(current: &[String], imported: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::<String>::new();
    let mut merged = Vec::<String>::new();
    for item in current.iter().chain(imported.iter()) {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            merged.push(trimmed.to_string());
        }
    }
    merged
}

fn merge_api_providers(
    current: &[ApiProviderConfig],
    imported: &[ApiProviderConfig],
) -> (Vec<ApiProviderConfig>, usize, usize) {
    let mut merged = current.to_vec();
    let mut added = 0usize;
    let mut updated = 0usize;

    for provider in imported {
        if let Some(index) = merged.iter().position(|item| item.id == provider.id) {
            let mut next = provider.clone();
            next.api_keys = merge_unique_api_keys(&merged[index].api_keys, &provider.api_keys);
            merged[index] = next;
            updated += 1;
        } else {
            merged.push(provider.clone());
            added += 1;
        }
    }

    (merged, added, updated)
}

fn merge_api_configs(
    current: &[ApiConfig],
    imported: &[ApiConfig],
) -> (Vec<ApiConfig>, usize, usize) {
    let mut merged = current.to_vec();
    let mut added = 0usize;
    let mut updated = 0usize;

    for item in imported {
        if let Some(index) = merged.iter().position(|config| config.id == item.id) {
            merged[index] = item.clone();
            updated += 1;
        } else {
            merged.push(item.clone());
            added += 1;
        }
    }

    (merged, added, updated)
}

fn merge_remote_im_channels_preserve_credentials(
    current: &[RemoteImChannelConfig],
    imported: &[RemoteImChannelConfig],
) -> Vec<RemoteImChannelConfig> {
    let credentials = current
        .iter()
        .map(|item| (item.id.clone(), item.credentials.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    imported
        .iter()
        .map(|item| {
            let mut next = item.clone();
            next.credentials = credentials
                .get(&item.id)
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
            next
        })
        .collect::<Vec<_>>()
}

fn write_avatar_files(
    state: &AppState,
    avatar_files: &[MigrationBlobFile],
) -> Result<std::collections::HashMap<String, String>, String> {
    let dir = avatar_storage_dir(state)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("创建头像目录失败 ({}): {err}", dir.display()))?;
    let mut result = std::collections::HashMap::<String, String>::new();
    for item in avatar_files {
        let path = dir.join(format!("{}.webp", sanitize_avatar_key(&item.key)));
        std::fs::write(&path, decode_base64(&item.bytes_base64)?)
            .map_err(|err| format!("写入头像文件失败 ({}): {err}", path.display()))?;
        result.insert(item.key.clone(), path.to_string_lossy().to_string());
    }
    Ok(result)
}

fn write_oauth_files(config: &AppConfig, oauth_files: &[MigrationBlobFile]) -> Result<(), String> {
    let file_map = oauth_files
        .iter()
        .map(|item| (item.key.clone(), item))
        .collect::<std::collections::HashMap<_, _>>();

    for provider in &config.api_providers {
        if provider.request_format != RequestFormat::Codex {
            continue;
        }

        if let Some(item) = file_map.get(&format!("managed:{}", provider.id)) {
            let path = managed_codex_auth_path(&provider.id)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    format!("创建托管授权目录失败 ({}): {err}", parent.display())
                })?;
            }
            std::fs::write(&path, decode_base64(&item.bytes_base64)?)
                .map_err(|err| format!("写入托管授权文件失败 ({}): {err}", path.display()))?;
        }
    }

    Ok(())
}

fn build_imported_config(
    current: &AppConfig,
    imported: &AppConfig,
) -> (AppConfig, usize, usize, usize, usize) {
    let (api_providers, provider_added_count, provider_updated_count) =
        merge_api_providers(&current.api_providers, &imported.api_providers);
    let (api_configs, api_config_added_count, api_config_updated_count) =
        merge_api_configs(&current.api_configs, &imported.api_configs);

    let mut final_config = imported.clone();
    final_config.shell_workspaces = current.shell_workspaces.clone();
    final_config.api_providers = api_providers;
    final_config.api_configs = api_configs;
    final_config.remote_im_channels =
        merge_remote_im_channels_preserve_credentials(&current.remote_im_channels, &imported.remote_im_channels);
    normalize_app_config(&mut final_config);

    (
        final_config,
        provider_added_count,
        provider_updated_count,
        api_config_added_count,
        api_config_updated_count,
    )
}

fn build_imported_runtime(
    current: &AppData,
    imported: &AppData,
    avatar_path_map: &std::collections::HashMap<String, String>,
) -> AppData {
    let mut final_data = current.clone();
    final_data.agents = imported.agents.clone();
    for agent in &mut final_data.agents {
        agent.avatar_path = avatar_path_map.get(&agent.id).cloned();
    }
    final_data.assistant_department_agent_id = imported.assistant_department_agent_id.clone();
    final_data.user_alias = imported.user_alias.clone();
    final_data.response_style_id = imported.response_style_id.clone();
    final_data.pdf_read_mode = imported.pdf_read_mode.clone();
    final_data.background_voice_screenshot_keywords =
        imported.background_voice_screenshot_keywords.clone();
    final_data.background_voice_screenshot_mode =
        imported.background_voice_screenshot_mode.clone();
    final_data.instruction_presets = imported.instruction_presets.clone();
    final_data.pinned_conversation_ids = imported.pinned_conversation_ids.clone();
    final_data
}

#[tauri::command]
async fn export_config_migration_package(
    input: ExportConfigMigrationPackageInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExportConfigMigrationPackageResult, MigrationCommandError> {
    validate_export_migration_password(&input.password)?;
    let total_started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[迁移包导出] 开始 task=export_config_migration_package trigger=tauri_command password_present={} password_len={}",
        !input.password.trim().is_empty(),
        input.password.chars().count()
    ));
    let path = migration_save_path(&app).await?;
    let _guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("获取会话锁失败: {err}"))?;

    let payload_started_at = std::time::Instant::now();
    let payload = build_export_payload(state.inner())?;
    let payload_elapsed_ms = payload_started_at.elapsed().as_millis();
    runtime_log_info(format!(
        "[迁移包导出] 完成 task=export_config_migration_package trigger=tauri_command stage=build_export_payload provider_count={} api_config_count={} memory_count={} duration_ms={}",
        payload.config.api_providers.len(),
        payload.config.api_configs.len(),
        payload.memories.len(),
        payload_elapsed_ms
    ));
    let manifest = MigrationManifest {
        schema_version: MIGRATION_SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: now_iso(),
    };
    write_migration_package(&path, input.password.trim(), &manifest, &payload)?;
    runtime_log_info(format!(
        "[迁移包导出] 完成 task=export_config_migration_package trigger=tauri_command stage=write_migration_package path={} provider_count={} api_config_count={} memory_count={} total_duration_ms={}",
        path.to_string_lossy(),
        payload.config.api_providers.len(),
        payload.config.api_configs.len(),
        payload.memories.len(),
        total_started_at.elapsed().as_millis()
    ));

    Ok(ExportConfigMigrationPackageResult {
        path: path.to_string_lossy().to_string(),
        provider_count: payload.config.api_providers.len(),
        api_config_count: payload.config.api_configs.len(),
        memory_count: payload.memories.len(),
    })
}

#[tauri::command]
async fn preview_import_config_migration_package(
    input: PreviewImportConfigMigrationPackageInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PreviewImportConfigMigrationPackageResult, MigrationCommandError> {
    let package_path = resolve_migration_import_path(&app, &input).await?;
    let preview_id = Uuid::new_v4().to_string();
    let preview_dir = migration_preview_dir(state.inner(), &preview_id);

    unzip_migration_package_to_dir(&package_path, input.password.trim(), &preview_dir)?;
    let (manifest, payload) = read_preview_payload(&preview_dir)?;
    assert_manifest_version(&manifest)?;

    let current_config = state_read_config_cached(&state)?;
    let memory_preview = preview_memory_import(state.inner(), &preview_dir, &payload.memories)?;
    let (_, provider_added_count, provider_updated_count) =
        merge_api_providers(&current_config.api_providers, &payload.config.api_providers);
    let (_, api_config_added_count, api_config_updated_count) =
        merge_api_configs(&current_config.api_configs, &payload.config.api_configs);

    state
        .migration_preview_dirs
        .lock()
        .map_err(|err| format!("锁定迁移预检目录失败: {err}"))?
        .insert(preview_id.clone(), preview_dir.to_string_lossy().to_string());

    Ok(PreviewImportConfigMigrationPackageResult {
        preview_id,
        package_version: manifest.app_version,
        memory_added_count: memory_preview.created_count,
        memory_merged_count: memory_preview.merged_count,
        provider_added_count,
        provider_updated_count,
        api_config_added_count,
        api_config_updated_count,
        oauth_file_count: payload.oauth_files.len(),
        avatar_file_count: payload.avatar_files.len(),
    })
}

#[tauri::command]
fn apply_import_config_migration_package(
    input: ApplyImportConfigMigrationPackageInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApplyImportConfigMigrationPackageResult, MigrationCommandError> {
    let preview_dir = state
        .migration_preview_dirs
        .lock()
        .map_err(|err| format!("锁定迁移预检目录失败: {err}"))?
        .remove(input.preview_id.trim())
        .ok_or_else(|| "迁移预检已失效，请重新选择迁移包。".to_string())?;
    let preview_dir = PathBuf::from(preview_dir);

    let _guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("获取会话锁失败: {err}"))?;

    let (manifest, payload) = read_preview_payload(&preview_dir)?;
    assert_manifest_version(&manifest)?;

    let backup_dir = backup_current_migration_targets(state.inner())?;
    let current_config = state_read_config_cached(&state)?;
    let current_data = state_read_app_data_cached(&state)?;
    let (
        final_config,
        provider_added_count,
        provider_updated_count,
        api_config_added_count,
        api_config_updated_count,
    ) = build_imported_config(&current_config, &payload.config);
    let avatar_path_map = write_avatar_files(state.inner(), &payload.avatar_files)?;
    write_oauth_files(&final_config, &payload.oauth_files)?;
    let final_data = build_imported_runtime(&current_data, &payload.runtime_data, &avatar_path_map);
    let memory_stats = memory_store_import_memories(&state.data_path, &payload.memories)?;

    state_write_config_cached(&state, &final_config)?;
    state_write_agents_cached(&state, &final_data.agents)?;
    state_write_runtime_state_cached(&state, &build_runtime_state_file(&final_data))?;

    if let Err(err) = std::fs::remove_dir_all(&preview_dir) {
        runtime_log_warn(format!(
            "[迁移包导入] 失败 task=apply_import_config_migration_package stage=remove_preview_dir path={} err={:?}",
            preview_dir.display(),
            err
        ));
    }

    let result = ApplyImportConfigMigrationPackageResult {
        imported_memory_count: memory_stats.imported_count,
        created_memory_count: memory_stats.created_count,
        merged_memory_count: memory_stats.merged_count,
        provider_added_count,
        provider_updated_count,
        api_config_added_count,
        api_config_updated_count,
        backup_dir: backup_dir.to_string_lossy().to_string(),
    };

    let app_handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        app_handle.restart();
    });

    Ok(result)
}
