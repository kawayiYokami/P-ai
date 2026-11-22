use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::now_iso;
use super::runtime_log_debug;
use super::runtime_log_error;
use super::runtime_log_warn;
use super::terminal_workspace_path_from_conversation;
use super::AppState;
use super::Conversation;

pub(crate) const USER_MESSAGE_GIT_GHOST_SNAPSHOT_KEY: &str = "mainWorkspaceGitGhostSnapshot";
const GIT_GHOST_SNAPSHOT_TIMEOUT_SECS: u64 = 20;
const GIT_GHOST_SNAPSHOT_ENABLED: bool = false;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserMessageGitGhostSnapshotRecord {
    pub(crate) conversation_id: String,
    pub(crate) user_message_id: String,
    pub(crate) main_workspace_path: String,
    pub(crate) ghost_commit_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) preexisting_untracked_files: Vec<String>,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

fn git_ghost_snapshot_author_env() -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("GIT_AUTHOR_NAME"), OsString::from("pai")),
        (
            OsString::from("GIT_AUTHOR_EMAIL"),
            OsString::from("pai@local"),
        ),
        (OsString::from("GIT_COMMITTER_NAME"), OsString::from("pai")),
        (
            OsString::from("GIT_COMMITTER_EMAIL"),
            OsString::from("pai@local"),
        ),
    ]
}

async fn git_run_output(
    cwd: &Path,
    args: &[OsString],
    extra_env: Option<&[(OsString, OsString)]>,
) -> Result<std::process::Output, String> {
    let cwd = cwd.to_path_buf();
    let args = args.to_vec();
    let extra_env = extra_env.map(|items| items.to_vec());
    let cwd_for_error = cwd.clone();
    let args_for_error = args.clone();
    let join = tokio::time::timeout(
        Duration::from_secs(GIT_GHOST_SNAPSHOT_TIMEOUT_SECS),
        tauri::async_runtime::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("git");
            cmd.current_dir(&cwd);
            cmd.args(&args);
            if let Some(items) = extra_env.as_ref() {
                for (key, value) in items {
                    cmd.env(key, value);
                }
            }
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt as _;
                // Git 是控制台程序，后台快照不能让 GUI 应用弹出控制台窗口。
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            cmd.output().map_err(|err| {
                format!(
                    "执行 git 命令失败: cwd={} args={:?} err={}",
                    cwd.display(),
                    args,
                    err
                )
            })
        }),
    )
    .await
    .map_err(|_| {
        format!(
            "执行 git 命令超时: cwd={} args={:?} timeout_secs={}",
            cwd_for_error.display(),
            args_for_error,
            GIT_GHOST_SNAPSHOT_TIMEOUT_SECS
        )
    })?;

    match join {
        Ok(result) => result,
        Err(err) => Err(format!(
            "执行 git 命令任务失败: cwd={} args={:?} err={}",
            cwd_for_error.display(),
            args_for_error,
            err
        )),
    }
}

async fn git_run_stdout_trimmed(
    cwd: &Path,
    args: &[OsString],
    extra_env: Option<&[(OsString, OsString)]>,
) -> Result<String, String> {
    let output = git_run_output(cwd, args, extra_env).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "git 命令失败: cwd={} args={:?} stderr={}",
            cwd.display(),
            args,
            stderr
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn git_run_status_ok(
    cwd: &Path,
    args: &[OsString],
    extra_env: Option<&[(OsString, OsString)]>,
) -> Result<(), String> {
    let output = git_run_output(cwd, args, extra_env).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "git 命令失败: cwd={} args={:?} stderr={}",
            cwd.display(),
            args,
            stderr
        ));
    }
    Ok(())
}

async fn git_repo_root_for_workspace_path(workspace_path: &Path) -> Result<PathBuf, String> {
    let root = git_run_stdout_trimmed(
        workspace_path,
        &[
            OsString::from("rev-parse"),
            OsString::from("--show-toplevel"),
        ],
        None,
    )
    .await?;
    let path = PathBuf::from(root);
    let canonical = path.canonicalize().map_err(|err| {
        format!(
            "规范化 Git 仓库根目录失败: path={} err={}",
            path.display(),
            err
        )
    })?;
    Ok(canonical)
}

fn git_repo_prefix_for_workspace(repo_root: &Path, workspace_path: &Path) -> Option<PathBuf> {
    workspace_path
        .strip_prefix(repo_root)
        .ok()
        .map(PathBuf::from)
        .filter(|value| {
            let text = value.to_string_lossy().trim().to_string();
            !text.is_empty() && text != "."
        })
}

fn git_scope_pathspec(prefix: Option<&Path>) -> OsString {
    prefix
        .map(|value| OsString::from(value.as_os_str()))
        .unwrap_or_else(|| OsString::from("."))
}

async fn git_ls_untracked_files(
    repo_root: &Path,
    prefix: Option<&Path>,
) -> Result<Vec<String>, String> {
    let args = vec![
        OsString::from("ls-files"),
        OsString::from("--others"),
        OsString::from("--exclude-standard"),
        OsString::from("-z"),
        OsString::from("--"),
        git_scope_pathspec(prefix),
    ];
    let output = git_run_output(repo_root, &args, None).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "读取 Git 未跟踪文件失败: repo_root={} stderr={}",
            repo_root.display(),
            stderr
        ));
    }
    let mut items = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
        .map(|item| String::from_utf8_lossy(item).to_string())
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    Ok(items)
}

pub(crate) fn write_git_snapshot_record_into_provider_meta(
    provider_meta: &mut Option<Value>,
    record: &UserMessageGitGhostSnapshotRecord,
) -> Result<(), String> {
    let value = serde_json::to_value(record)
        .map_err(|err| format!("序列化 Git 幽灵快照记录失败: {err}"))?;
    let base = provider_meta
        .take()
        .unwrap_or_else(|| serde_json::json!({}));
    let mut object = match base {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    object.insert(USER_MESSAGE_GIT_GHOST_SNAPSHOT_KEY.to_string(), value);
    *provider_meta = Some(Value::Object(object));
    Ok(())
}

pub(crate) fn read_git_snapshot_record_from_provider_meta(
    provider_meta: Option<&Value>,
) -> Option<UserMessageGitGhostSnapshotRecord> {
    provider_meta
        .and_then(Value::as_object)
        .and_then(|map| map.get(USER_MESSAGE_GIT_GHOST_SNAPSHOT_KEY))
        .and_then(|value| serde_json::from_value::<UserMessageGitGhostSnapshotRecord>(value.clone()).ok())
}

pub(crate) async fn create_main_workspace_git_ghost_snapshot_record(
    state: &AppState,
    conversation: &Conversation,
    user_message_id: &str,
) -> Option<UserMessageGitGhostSnapshotRecord> {
    if !GIT_GHOST_SNAPSHOT_ENABLED {
        runtime_log_warn(format!(
            "[系统] 跳过 原因=feature_disabled"
        ));
        return None;
    }
    let workspace_path = terminal_workspace_path_from_conversation(state, conversation)?;
    Some(
        create_git_ghost_snapshot_record_for_workspace(
            &conversation.id,
            user_message_id,
            &workspace_path,
        )
        .await,
    )
}

async fn create_git_ghost_snapshot_record_for_workspace(
    conversation_id: &str,
    user_message_id: &str,
    workspace_path: &Path,
) -> UserMessageGitGhostSnapshotRecord {
    let created_at = now_iso();
    let mut record = UserMessageGitGhostSnapshotRecord {
        conversation_id: conversation_id.to_string(),
        user_message_id: user_message_id.to_string(),
        main_workspace_path: workspace_path.to_string_lossy().to_string(),
        ghost_commit_id: None,
        created_at,
        status: "skipped".to_string(),
        preexisting_untracked_files: Vec::new(),
        error: None,
    };
    let repo_root = match git_repo_root_for_workspace_path(workspace_path).await {
        Ok(value) => value,
        Err(err) => {
            record.error = Some(err);
            runtime_log_warn(format!(
                "[系统] 跳过 工作区={} 原因=not_git_repo",
                workspace_path.display()
            ));
            return record;
        }
    };
    let prefix = git_repo_prefix_for_workspace(&repo_root, workspace_path);
    let preexisting_untracked_files = match git_ls_untracked_files(&repo_root, prefix.as_deref()).await {
        Ok(value) => value,
        Err(err) => {
            record.status = "failed".to_string();
            record.error = Some(err);
            runtime_log_error(format!(
                "[系统] 失败 工作区={} 阶段=list_untracked 错误={}",
                workspace_path.display(),
                record.error.as_deref().unwrap_or_default()
            ));
            return record;
        }
    };
    record.preexisting_untracked_files = preexisting_untracked_files;

    let temp_index_path = std::env::temp_dir().join(format!(
        "pai-git-ghost-index-{}.tmp",
        Uuid::new_v4()
    ));
    let env_pairs = vec![(
        OsString::from("GIT_INDEX_FILE"),
        OsString::from(temp_index_path.as_os_str()),
    )];
    let parent = git_run_stdout_trimmed(
        &repo_root,
        &[
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from("HEAD"),
        ],
        None,
    )
    .await
    .ok();
    if let Some(parent_sha) = parent.as_deref() {
        if let Err(err) = git_run_status_ok(
            &repo_root,
            &[OsString::from("read-tree"), OsString::from(parent_sha)],
            Some(env_pairs.as_slice()),
        )
        .await
        {
            let _ = fs::remove_file(&temp_index_path);
            record.status = "failed".to_string();
            record.error = Some(err);
            runtime_log_error(format!(
                "[系统] 失败 工作区={} 阶段=read_tree 错误={}",
                workspace_path.display(),
                record.error.as_deref().unwrap_or_default()
            ));
            return record;
        }
    }
    let add_args = vec![
        OsString::from("add"),
        OsString::from("--all"),
        OsString::from("--"),
        git_scope_pathspec(prefix.as_deref()),
    ];
    if let Err(err) = git_run_status_ok(&repo_root, &add_args, Some(env_pairs.as_slice())).await {
        let _ = fs::remove_file(&temp_index_path);
        record.status = "failed".to_string();
        record.error = Some(err);
        runtime_log_error(format!(
            "[系统] 失败 工作区={} 阶段=add_all 错误={}",
            workspace_path.display(),
            record.error.as_deref().unwrap_or_default()
        ));
        return record;
    }
    let tree_id = match git_run_stdout_trimmed(
        &repo_root,
        &[OsString::from("write-tree")],
        Some(env_pairs.as_slice()),
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            let _ = fs::remove_file(&temp_index_path);
            record.status = "failed".to_string();
            record.error = Some(err);
            runtime_log_error(format!(
                "[系统] 失败 工作区={} 阶段=write_tree 错误={}",
                workspace_path.display(),
                record.error.as_deref().unwrap_or_default()
            ));
            return record;
        }
    };
    let mut commit_env = env_pairs.clone();
    commit_env.extend(git_ghost_snapshot_author_env());
    let mut commit_args = vec![OsString::from("commit-tree"), OsString::from(tree_id)];
    if let Some(parent_sha) = parent.as_deref() {
        commit_args.push(OsString::from("-p"));
        commit_args.push(OsString::from(parent_sha));
    }
    commit_args.push(OsString::from("-m"));
    commit_args.push(OsString::from(format!(
        "pai ghost snapshot {}",
        user_message_id
    )));
    let commit_id = match git_run_stdout_trimmed(
        &repo_root,
        &commit_args,
        Some(commit_env.as_slice()),
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            let _ = fs::remove_file(&temp_index_path);
            record.status = "failed".to_string();
            record.error = Some(err);
            runtime_log_error(format!(
                "[系统] 失败 工作区={} 阶段=commit_tree 错误={}",
                workspace_path.display(),
                record.error.as_deref().unwrap_or_default()
            ));
            return record;
        }
    };
    let _ = fs::remove_file(&temp_index_path);
    record.status = "created".to_string();
    record.ghost_commit_id = Some(commit_id.clone());
    runtime_log_debug(format!(
        "[系统] 完成 工作区={} 提交={}",
        workspace_path.display(),
        commit_id
    ));
    record
}

fn remove_git_snapshot_new_untracked_files(
    repo_root: &Path,
    workspace_path: &Path,
    snapshot: &UserMessageGitGhostSnapshotRecord,
    current_untracked_files: Vec<String>,
) -> Result<(), String> {
    let preserved = snapshot
        .preexisting_untracked_files
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for relative in current_untracked_files {
        if preserved.contains(&relative) {
            continue;
        }
        let absolute = repo_root.join(&relative);
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(value) => value,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(format!(
                    "读取新增未跟踪路径元信息失败: path={} err={}",
                    absolute.display(),
                    err
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            fs::remove_file(&absolute).map_err(|err| {
                format!(
                    "删除新增未跟踪符号链接失败: symlink={} err={}",
                    absolute.display(),
                    err
                )
            })?;
        } else if metadata.is_file() {
            fs::remove_file(&absolute).map_err(|err| {
                format!(
                    "删除新增未跟踪文件失败: file={} err={}",
                    absolute.display(),
                    err
                )
            })?;
        } else if metadata.is_dir() {
            fs::remove_dir_all(&absolute).map_err(|err| {
                format!(
                    "删除新增未跟踪目录失败: dir={} err={}",
                    absolute.display(),
                    err
                )
            })?;
        }
        let mut current = absolute.parent().map(PathBuf::from);
        while let Some(dir) = current {
            if dir == workspace_path || dir == repo_root {
                break;
            }
            let is_empty = dir
                .read_dir()
                .map(|mut iter| iter.next().is_none())
                .unwrap_or(false);
            if !is_empty {
                break;
            }
            let parent = dir.parent().map(PathBuf::from);
            let _ = fs::remove_dir(&dir);
            current = parent;
        }
    }
    Ok(())
}

pub(crate) async fn restore_main_workspace_from_git_ghost_snapshot(
    snapshot: &UserMessageGitGhostSnapshotRecord,
) -> Result<(), String> {
    if !GIT_GHOST_SNAPSHOT_ENABLED {
        runtime_log_warn(format!(
            "[系统] 跳过恢复 原因=feature_disabled"
        ));
        return Ok(());
    }
    let commit_id = snapshot
        .ghost_commit_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Git 幽灵快照记录缺少 commit_id。".to_string())?;
    let workspace_path = PathBuf::from(snapshot.main_workspace_path.trim());
    let repo_root = git_repo_root_for_workspace_path(&workspace_path).await?;
    let prefix = git_repo_prefix_for_workspace(&repo_root, &workspace_path);
    let current_untracked_files = git_ls_untracked_files(&repo_root, prefix.as_deref()).await?;
    let restore_args = vec![
        OsString::from("restore"),
        OsString::from("--source"),
        OsString::from(commit_id),
        OsString::from("--worktree"),
        OsString::from("--"),
        git_scope_pathspec(prefix.as_deref()),
    ];
    git_run_status_ok(&repo_root, &restore_args, None).await?;
    remove_git_snapshot_new_untracked_files(
        &repo_root,
        &workspace_path,
        snapshot,
        current_untracked_files,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn run_git_test_command(cwd: &Path, args: &[OsString]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run git command");
        assert!(
            output.status.success(),
            "git command failed: cwd={} args={:?} stderr={}",
            cwd.display(),
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn git_snapshot_record_should_roundtrip_in_provider_meta() {
        let record = UserMessageGitGhostSnapshotRecord {
            conversation_id: "conv-a".to_string(),
            user_message_id: "user-a".to_string(),
            main_workspace_path: "E:/repo".to_string(),
            ghost_commit_id: Some("abc123".to_string()),
            created_at: "2026-04-20T10:00:00Z".to_string(),
            status: "created".to_string(),
            preexisting_untracked_files: vec!["tmp.txt".to_string()],
            error: None,
        };
        let mut provider_meta = None;
        write_git_snapshot_record_into_provider_meta(&mut provider_meta, &record)
            .expect("write snapshot record");
        let restored = read_git_snapshot_record_from_provider_meta(provider_meta.as_ref())
            .expect("read snapshot record");
        assert_eq!(restored.conversation_id, record.conversation_id);
        assert_eq!(restored.user_message_id, record.user_message_id);
        assert_eq!(restored.ghost_commit_id, record.ghost_commit_id);
        assert_eq!(
            restored.preexisting_untracked_files,
            record.preexisting_untracked_files
        );
    }

    #[test]
    fn create_git_ghost_snapshot_should_skip_non_git_workspace() {
        let root =
            std::env::temp_dir().join(format!("easy-call-ai-git-skip-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp workspace");
        let record = tauri::async_runtime::block_on(create_git_ghost_snapshot_record_for_workspace(
            "conv-a",
            "user-a",
            &root,
        ));
        assert_eq!(record.status, "skipped");
        assert!(record.ghost_commit_id.is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn git_ghost_snapshot_should_restore_workspace_and_remove_new_untracked_files() {
        if !git_available() {
            return;
        }
        let root =
            std::env::temp_dir().join(format!("easy-call-ai-git-ghost-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp repo");
        run_git_test_command(&root, &[OsString::from("init")]);
        run_git_test_command(
            &root,
            &[
                OsString::from("config"),
                OsString::from("user.name"),
                OsString::from("pai"),
            ],
        );
        run_git_test_command(
            &root,
            &[
                OsString::from("config"),
                OsString::from("user.email"),
                OsString::from("pai@local"),
            ],
        );
        fs::write(root.join("tracked.txt"), "before\n").expect("write tracked before");
        fs::write(root.join("keep-untracked.txt"), "keep\n").expect("write keep file");
        run_git_test_command(
            &root,
            &[OsString::from("add"), OsString::from("tracked.txt")],
        );
        run_git_test_command(
            &root,
            &[
                OsString::from("commit"),
                OsString::from("-m"),
                OsString::from("init"),
            ],
        );

        let record = tauri::async_runtime::block_on(create_git_ghost_snapshot_record_for_workspace(
            "conv-a",
            "user-a",
            &root,
        ));
        assert_eq!(record.status, "created");
        assert!(record.ghost_commit_id.is_some());
        assert_eq!(
            record.preexisting_untracked_files,
            vec!["keep-untracked.txt".to_string()]
        );

        fs::write(root.join("tracked.txt"), "after\n").expect("write tracked after");
        fs::write(root.join("new-untracked.txt"), "remove me\n")
            .expect("write new untracked");

        tauri::async_runtime::block_on(restore_main_workspace_from_git_ghost_snapshot(&record))
            .expect("restore snapshot");

        if GIT_GHOST_SNAPSHOT_ENABLED {
            assert_eq!(
                fs::read_to_string(root.join("tracked.txt")).expect("read tracked"),
                "before\n"
            );
            assert_eq!(
                fs::read_to_string(root.join("keep-untracked.txt")).expect("read keep file"),
                "keep\n"
            );
            assert!(
                !root.join("new-untracked.txt").exists(),
                "new untracked file should be removed"
            );
        } else {
            assert_eq!(
                fs::read_to_string(root.join("tracked.txt")).expect("read tracked"),
                "after\n"
            );
            assert_eq!(
                fs::read_to_string(root.join("keep-untracked.txt")).expect("read keep file"),
                "keep\n"
            );
            assert!(
                root.join("new-untracked.txt").exists(),
                "feature disabled should skip removing new untracked file"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }
}
