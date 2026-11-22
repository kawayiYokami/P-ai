#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteUtf8TextFileInput {
    path: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteBase64FileInput {
    path: String,
    bytes_base64: String,
}

fn normalize_export_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    if std::path::Path::new(trimmed)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("导出路径不允许包含上级目录跳转".to_string());
    }
    Ok(PathBuf::from(trimmed))
}

fn ensure_share_export_parent_dir(path: &PathBuf) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("创建导出目录失败: {err}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn write_utf8_text_file_to_path(input: WriteUtf8TextFileInput) -> Result<(), String> {
    let path = normalize_export_path(&input.path)?;
    let text = input.text;
    let byte_len = text.as_bytes().len();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let started_at = std::time::Instant::now();
        ensure_share_export_parent_dir(&path)?;
        std::fs::write(&path, text.as_bytes())
            .map_err(|err| format!("写入文本导出文件失败 path={}: {err}", path.display()))?;
        runtime_log_info(format!(
            "[系统] 文本文件写入完成 文件={} 字节={} 耗时={}",
            path.display(),
            byte_len,
            started_at.elapsed().as_millis()
        ));
        Ok(())
    })
    .await
    .map_err(|err| format!("写入文本导出文件任务失败: {err}"))?
}

#[tauri::command]
async fn write_base64_file_to_path(input: WriteBase64FileInput) -> Result<(), String> {
    let path = normalize_export_path(&input.path)?;
    let path_display = path.display().to_string();
    let bytes = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("解码导出文件内容失败 path={}: {err}", path_display))?;
    runtime_log_info(format!(
        "[系统] 开始写入二进制文件 文件={} 字节={}",
        path_display,
        bytes.len()
    ));
    let write_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let started_at = std::time::Instant::now();
        ensure_share_export_parent_dir(&write_path)?;
        std::fs::write(&write_path, bytes)
            .map_err(|err| format!("写入二进制导出文件失败 path={}: {err}", write_path.display()))?;
        runtime_log_info(format!(
            "[系统] 二进制文件写入完成 文件={} 耗时={}",
            write_path.display(),
            started_at.elapsed().as_millis()
        ));
        Ok(())
    })
    .await
    .map_err(|err| format!("写入二进制导出文件任务失败 path={}: {err}", path_display))?
}
