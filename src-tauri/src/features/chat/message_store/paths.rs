pub(super) const MESSAGE_STORE_MANIFEST_FILE_NAME: &str = "manifest.json";
pub(super) const MESSAGE_STORE_META_FILE_NAME: &str = "meta.json";
pub(super) const MESSAGE_STORE_MESSAGES_FILE_NAME: &str = "messages.jsonl";
pub(super) const MESSAGE_STORE_ACTIVE_PLANS_FILE_NAME: &str = "active_plans.jsonl";
pub(super) const MESSAGE_STORE_INDEX_FILE_NAME: &str = "messages.idx.json";
pub(super) const MESSAGE_STORE_BLOCKS_DIR_NAME: &str = "blocks";
const MESSAGE_STORE_BLOBS_DIR_NAME: &str = "blobs";

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MessageStorePaths {
    data_path: PathBuf,
    conversation_id: String,
    legacy_conversation_file: PathBuf,
    shard_dir: PathBuf,
    manifest_file: PathBuf,
    meta_file: PathBuf,
    messages_file: PathBuf,
    active_plans_file: PathBuf,
    index_file: PathBuf,
    blocks_dir: PathBuf,
    blobs_dir: PathBuf,
}

pub(super) fn message_store_paths(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<MessageStorePaths, String> {
    if conversation_id.trim().is_empty() {
        return Err("会话 ID 为空，无法构造消息存储路径".to_string());
    }
    if conversation_id != conversation_id.trim() {
        return Err(format!(
            "会话 ID 不能包含首尾空白，conversation_id={conversation_id}"
        ));
    }
    validate_message_store_conversation_id(conversation_id)?;
    let shard_dir = app_layout_chat_conversations_dir(data_path).join(conversation_id);
    message_store_paths_for_shard_dir(
        data_path,
        conversation_id,
        shard_dir,
        app_layout_chat_conversation_path(data_path, conversation_id),
    )
}

pub(super) fn message_store_paths_for_shard_dir(
    data_path: &PathBuf,
    conversation_id: &str,
    shard_dir: PathBuf,
    legacy_conversation_file: PathBuf,
) -> Result<MessageStorePaths, String> {
    if conversation_id.trim().is_empty() {
        return Err("会话 ID 为空，无法构造消息存储路径".to_string());
    }
    if conversation_id != conversation_id.trim() {
        return Err(format!(
            "会话 ID 不能包含首尾空白，conversation_id={conversation_id}"
        ));
    }
    validate_message_store_conversation_id(conversation_id)?;
    Ok(MessageStorePaths {
        data_path: data_path.clone(),
        conversation_id: conversation_id.to_string(),
        legacy_conversation_file,
        manifest_file: shard_dir.join(MESSAGE_STORE_MANIFEST_FILE_NAME),
        meta_file: shard_dir.join(MESSAGE_STORE_META_FILE_NAME),
        messages_file: shard_dir.join(MESSAGE_STORE_MESSAGES_FILE_NAME),
        active_plans_file: shard_dir.join(MESSAGE_STORE_ACTIVE_PLANS_FILE_NAME),
        index_file: shard_dir.join(MESSAGE_STORE_INDEX_FILE_NAME),
        blocks_dir: shard_dir.join(MESSAGE_STORE_BLOCKS_DIR_NAME),
        blobs_dir: shard_dir.join(MESSAGE_STORE_BLOBS_DIR_NAME),
        shard_dir,
    })
}

fn validate_message_store_conversation_id(conversation_id: &str) -> Result<(), String> {
    if conversation_id.contains('/') || conversation_id.contains('\\') {
        return Err(format!(
            "会话 ID 不能包含路径分隔符，conversation_id={conversation_id}"
        ));
    }
    if conversation_id
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err(format!(
            "会话 ID 不能包含 Windows 文件名非法字符，conversation_id={conversation_id}"
        ));
    }
    if conversation_id.ends_with([' ', '.']) {
        return Err(format!(
            "会话 ID 不能以 Windows 不稳定文件名字符结尾，conversation_id={conversation_id}"
        ));
    }
    let mut components = std::path::Path::new(conversation_id).components();
    let Some(component) = components.next() else {
        return Err("会话 ID 为空，无法构造消息存储路径".to_string());
    };
    if components.next().is_some() || !matches!(component, std::path::Component::Normal(_)) {
        return Err(format!(
            "会话 ID 不能包含路径组件，conversation_id={conversation_id}"
        ));
    }
    let upper = conversation_id.to_ascii_uppercase();
    let base_name = upper.split('.').next().unwrap_or(upper.as_str());
    let reserved = matches!(base_name, "CON" | "PRN" | "AUX" | "NUL")
        || base_name
            .strip_prefix("COM")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
        || base_name
            .strip_prefix("LPT")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value));
    if reserved {
        return Err(format!(
            "会话 ID 不能使用 Windows 保留文件名，conversation_id={conversation_id}"
        ));
    }
    Ok(())
}

fn path_modified_time(path: &PathBuf) -> Option<std::time::SystemTime> {
    fs::metadata(path).and_then(|metadata| metadata.modified()).ok()
}

pub(super) fn message_store_shard_modified_time(
    paths: &MessageStorePaths,
) -> Option<std::time::SystemTime> {
    path_modified_time(&chat_metadata_store_db_path(&paths.data_path))
}

pub(super) fn write_message_store_text_atomic(
    path: &PathBuf,
    tmp_extension: &str,
    content: &str,
    label: &str,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "创建{label}目录失败，path={}，error={err}",
                parent.display()
            )
        })?;
    }
    let tmp_path = path.with_extension(format!("{tmp_extension}.{}", Uuid::new_v4()));
    fs::write(&tmp_path, content).map_err(|err| {
        format!(
            "写入{label}临时文件失败，path={}，error={err}",
            tmp_path.display()
        )
    })?;
    replace_message_store_file_atomic(&tmp_path, path, label)
}

/// 字节原子写（V4 压缩块用）：写临时文件后原子替换，与文本版同一套替换语义
pub(super) fn write_message_store_bytes_atomic(
    path: &PathBuf,
    tmp_extension: &str,
    content: &[u8],
    label: &str,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "创建{label}目录失败，path={}，error={err}",
                parent.display()
            )
        })?;
    }
    let tmp_path = path.with_extension(format!("{tmp_extension}.{}", Uuid::new_v4()));
    fs::write(&tmp_path, content).map_err(|err| {
        format!(
            "写入{label}临时文件失败，path={}，error={err}",
            tmp_path.display()
        )
    })?;
    replace_message_store_file_atomic(&tmp_path, path, label)
}

pub(super) fn replace_message_store_file_atomic(
    tmp_path: &PathBuf,
    path: &PathBuf,
    label: &str,
) -> Result<(), String> {
    if let Err(replace_err) = replace_message_store_file_atomic_inner(tmp_path, path) {
        if let Err(copy_err) = fs::copy(tmp_path, path) {
            let _ = fs::remove_file(tmp_path);
            return Err(format!(
                "替换{label}失败，tmp={}，target={}，replace_error={replace_err}，copy_error={copy_err}",
                tmp_path.display(),
                path.display()
            ));
        }
        fs::remove_file(tmp_path).map_err(|cleanup_err| {
            format!(
                "清理{label}临时文件失败，tmp={}，replace_error={replace_err}，cleanup_error={cleanup_err}",
                tmp_path.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn replace_message_store_file_atomic_inner(
    tmp_path: &PathBuf,
    path: &PathBuf,
) -> Result<(), std::io::Error> {
    let tmp_wide = path_to_windows_wide_null(tmp_path);
    let path_wide = path_to_windows_wide_null(path);
    let flags = windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
        | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH;
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            tmp_wide.as_ptr(),
            path_wide.as_ptr(),
            flags,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_message_store_file_atomic_inner(
    tmp_path: &PathBuf,
    path: &PathBuf,
) -> Result<(), std::io::Error> {
    fs::rename(tmp_path, path)
}

#[cfg(target_os = "windows")]
fn path_to_windows_wide_null(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod message_store_atomic_write_tests {
    use super::*;

    #[test]
    fn message_store_paths_should_reject_path_like_conversation_id() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-paths-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");

        for invalid_id in [
            "../escape",
            "nested/id",
            r"nested\id",
            "bad:name",
            "bad*name",
            "bad\u{7}name",
            "CON",
            "con.txt",
            "com1",
            "LPT1",
            "name.",
            "name ",
            " name",
            ".",
            "",
        ] {
            let err = message_store_paths(&data_path, invalid_id)
                .expect_err("path-like conversation id should fail");
            assert!(err.contains("会话 ID"));
        }

        let paths = message_store_paths(&data_path, "conversation-safe").expect("safe id");
        assert_eq!(paths.conversation_id, "conversation-safe");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_atomic_write_should_create_parent_and_overwrite_target() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-atomic-{}",
            Uuid::new_v4()
        ));
        let target = root.join("nested").join("messages.jsonl");

        write_message_store_text_atomic(&target, "jsonl.tmp", "first", "测试文件")
            .expect("write first");
        write_message_store_text_atomic(&target, "jsonl.tmp", "second", "测试文件")
            .expect("overwrite second");

        assert_eq!(fs::read_to_string(&target).expect("read target"), "second");
        let remaining = fs::read_dir(target.parent().expect("target parent"))
            .expect("read parent")
            .map(|entry| entry.expect("entry").path())
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec![target.clone()]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_atomic_write_should_use_distinct_temp_files_for_parallel_writers() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-atomic-parallel-{}",
            Uuid::new_v4()
        ));
        let target = root.join("nested").join("messages.jsonl");
        let succeeded = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handles = Vec::new();

        for idx in 0..8 {
            let target = target.clone();
            let succeeded = succeeded.clone();
            handles.push(std::thread::spawn(move || {
                // 八个线程同时替换同一个目标：Windows 上个别写者会撞上文件被占用而失败，
                // 这是平台语义，不是本测试要验的东西。这里要验的是每个写者用各自的临时文件、
                // 失败也不留残骸、成功的那个能完整落盘。
                if write_message_store_text_atomic(
                    &target,
                    "jsonl.tmp",
                    &format!("writer-{idx}"),
                    "并发测试文件",
                )
                .is_ok()
                {
                    succeeded.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }));
        }
        for handle in handles {
            handle.join().expect("join writer");
        }

        assert!(
            succeeded.load(std::sync::atomic::Ordering::SeqCst) >= 1,
            "至少应有一个写者成功"
        );
        let final_content = fs::read_to_string(&target).expect("read target");
        assert!(final_content.starts_with("writer-"));
        let remaining = fs::read_dir(target.parent().expect("target parent"))
            .expect("read parent")
            .map(|entry| entry.expect("entry").path())
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec![target.clone()], "失败或成功的写者都不许留下临时文件");
        let _ = fs::remove_dir_all(root);
    }
}
