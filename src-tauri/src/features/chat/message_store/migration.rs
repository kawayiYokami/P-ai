#[derive(Debug, Clone, PartialEq, Eq)]
struct MessageStoreMigrationPlan {
    conversation_id: String,
    source_message_count: usize,
    source_last_message_id: String,
    target_store_kind: MessageStoreKind,
    dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MessageStoreMigrationOutcome {
    pub(super) conversation_id: String,
    pub(super) manifest: MessageStoreManifest,
    pub(super) wrote_files: bool,
}

fn plan_message_store_jsonl_snapshot_migration(
    conversation: &Conversation,
    dry_run: bool,
) -> Result<MessageStoreMigrationPlan, String> {
    let conversation_id = conversation.id.trim();
    if conversation_id.is_empty() {
        return Err("会话 ID 为空，无法规划消息迁移".to_string());
    }
    Ok(MessageStoreMigrationPlan {
        conversation_id: conversation_id.to_string(),
        source_message_count: conversation.messages.len(),
        source_last_message_id: conversation
            .messages
            .last()
            .map(|message| message.id.trim().to_string())
            .unwrap_or_default(),
        target_store_kind: MessageStoreKind::JsonlSnapshot,
        dry_run,
    })
}

fn build_jsonl_snapshot_migration_artifacts(
    conversation: &Conversation,
) -> Result<(String, MessageStoreManifest, MessageStoreIndexFile), String> {
    let plan = plan_message_store_jsonl_snapshot_migration(conversation, false)?;
    let content = encode_jsonl_snapshot_messages(&conversation.messages)?;
    let report = verify_jsonl_snapshot_content(
        &content,
        plan.source_message_count,
        &plan.source_last_message_id,
    )?;
    let manifest = MessageStoreManifest::jsonl_snapshot_building(conversation)
        .jsonl_snapshot_ready(content.as_bytes().len() as u64, 1);
    Ok((content, manifest, report.index))
}

pub(super) fn run_jsonl_snapshot_migration(
    paths: &MessageStorePaths,
    conversation: &Conversation,
    dry_run: bool,
) -> Result<MessageStoreMigrationOutcome, String> {
    let plan = plan_message_store_jsonl_snapshot_migration(conversation, dry_run)?;
    let building_manifest = MessageStoreManifest::jsonl_snapshot_building(conversation);
    if dry_run {
        return Ok(MessageStoreMigrationOutcome {
            conversation_id: plan.conversation_id,
            manifest: building_manifest,
            wrote_files: false,
        });
    }

    write_message_store_manifest_atomic(&paths.manifest_file, &building_manifest)?;
    let write = write_jsonl_snapshot_directory_shard(paths, conversation)?;

    Ok(MessageStoreMigrationOutcome {
        conversation_id: plan.conversation_id,
        manifest: write.manifest,
        wrote_files: true,
    })
}

fn read_message_store_manifest_for_resume(
    paths: &MessageStorePaths,
) -> Result<Option<MessageStoreManifest>, String> {
    if !paths.manifest_file.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&paths.manifest_file).map_err(|err| {
        format!(
            "读取消息存储 manifest 失败，path={}，error={err}",
            paths.manifest_file.display()
        )
    })?;
    match serde_json::from_str::<MessageStoreManifest>(&raw) {
        Ok(manifest) => {
            manifest.validate_version(&paths.manifest_file)?;
            Ok(Some(manifest))
        }
        Err(err) => {
            let backup_path = backup_corrupt_message_store_manifest(paths, &raw)?;
            eprintln!(
                "[消息存储迁移] manifest 解析失败，已备份并执行恢复重建：conversation_id={}，path={}，backup={}，error={}",
                paths.conversation_id,
                paths.manifest_file.display(),
                backup_path.display(),
                err
            );
            Ok(None)
        }
    }
}

fn backup_corrupt_message_store_manifest(
    paths: &MessageStorePaths,
    raw: &str,
) -> Result<PathBuf, String> {
    let backup_path = paths
        .manifest_file
        .with_extension(format!("json.corrupt.{}", Uuid::new_v4()));
    fs::write(&backup_path, raw).map_err(|err| {
        format!(
            "备份损坏消息存储 manifest 失败，conversation_id={}，backup={}，error={err}",
            paths.conversation_id,
            backup_path.display()
        )
    })?;
    Ok(backup_path)
}

pub(super) fn resume_jsonl_snapshot_migration(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreMigrationOutcome, String> {
    let Some(manifest) = read_message_store_manifest_for_resume(paths)? else {
        return run_jsonl_snapshot_migration(paths, conversation, false);
    };
    if manifest.should_read_jsonl() {
        match validate_ready_message_store_snapshot_integrity(paths, &manifest) {
            Ok(()) => {
                return Ok(MessageStoreMigrationOutcome {
                    conversation_id: conversation.id.trim().to_string(),
                    manifest,
                    wrote_files: false,
                });
            }
            Err(err) => {
                eprintln!(
                    "[消息存储迁移] ready 快照校验失败，执行恢复重建：conversation_id={}，error={}",
                    paths.conversation_id, err
                );
            }
        }
        return run_jsonl_snapshot_migration(paths, conversation, false);
    }
    if manifest.is_ready_directory_store() {
        return Ok(MessageStoreMigrationOutcome {
            conversation_id: conversation.id.trim().to_string(),
            manifest,
            wrote_files: false,
        });
    }
    match manifest.migration_state {
        MessageStoreMigrationState::Ready => Ok(MessageStoreMigrationOutcome {
            conversation_id: conversation.id.trim().to_string(),
            manifest,
            wrote_files: false,
        }),
        MessageStoreMigrationState::Building
        | MessageStoreMigrationState::Failed
        | MessageStoreMigrationState::Rollback
        | MessageStoreMigrationState::None => {
            run_jsonl_snapshot_migration(paths, conversation, false)
        }
    }
}

pub(super) fn rollback_jsonl_snapshot_migration(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreManifest, String> {
    let mut manifest = MessageStoreManifest::conversation_json_now(conversation);
    manifest.migration_state = MessageStoreMigrationState::Rollback;
    write_message_store_manifest_atomic(&paths.manifest_file, &manifest)?;
    Ok(manifest)
}

pub(super) fn rollback_message_store_manifest(
    paths: &MessageStorePaths,
    mut manifest: MessageStoreManifest,
) -> Result<MessageStoreManifest, String> {
    manifest.message_store_kind = MessageStoreKind::ConversationJson;
    manifest.migration_state = MessageStoreMigrationState::Rollback;
    manifest.messages_jsonl_bytes = 0;
    manifest.messages_index_revision = 0;
    manifest.updated_at = now_iso();
    write_message_store_manifest_atomic(&paths.manifest_file, &manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod message_store_tests {
    use super::*;

    fn test_message(id: &str, role: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: format!("2026-04-24T00:00:0{}Z", id.len()),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: format!("message {id}"),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn test_compaction_message(id: &str, kind: &str) -> ChatMessage {
        let mut message = test_message(id, "assistant");
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": kind
            }
        }));
        message
    }

    fn test_conversation(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-a".to_string(),
            title: "会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            archived_at: None,
            messages,
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    #[test]
    fn message_store_migration_plan_should_be_dry_run_without_artifacts() {
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let plan = plan_message_store_jsonl_snapshot_migration(&conversation, true)
            .expect("plan migration");

        assert_eq!(plan.conversation_id, "conversation-a");
        assert_eq!(plan.source_message_count, 1);
        assert_eq!(plan.source_last_message_id, "m1");
        assert_eq!(plan.target_store_kind, MessageStoreKind::JsonlSnapshot);
        assert!(plan.dry_run);
    }

    #[test]
    fn message_store_jsonl_verification_should_detect_compaction_kinds() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
            test_compaction_message("c2", "summary_context_seed"),
        ]);

        let (content, manifest, index) =
            build_jsonl_snapshot_migration_artifacts(&conversation).expect("build artifacts");

        assert!(manifest.should_read_jsonl());
        assert_eq!(manifest.source_message_count, 4);
        assert_eq!(manifest.last_message_id, "c2");
        assert_eq!(index.items.len(), 4);
        assert_eq!(index.items.iter().filter(|item| item.compaction_kind.is_some()).count(), 2);
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn message_store_jsonl_verification_should_reject_stale_last_message() {
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let content = encode_jsonl_snapshot_messages(&conversation.messages).expect("encode");
        let err = verify_jsonl_snapshot_content(&content, 1, "m2")
            .expect_err("stale last message should fail");

        assert!(err.contains("最后一条消息不一致"));
    }

    #[test]
    fn message_store_manifest_should_not_read_stale_jsonl_without_ready_state() {
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);

        assert!(!manifest.should_read_jsonl());
        assert!(!manifest.is_ready_directory_store());
        assert!(manifest
            .stale_jsonl_reason()
            .expect("stale reason")
            .contains("未处于 ready JSONL 状态"));
    }

    #[test]
    fn message_store_manifest_should_distinguish_supported_readiness_from_reserved_ready_store() {
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        manifest.message_store_kind = MessageStoreKind::JsonlEventLog;
        manifest.migration_state = MessageStoreMigrationState::Ready;

        assert!(manifest.is_ready_directory_store());
        assert!(!manifest.should_read_jsonl());
        assert!(manifest
            .stale_jsonl_reason()
            .expect("unsupported ready reason")
            .contains("暂不支持读取"));
    }

    #[test]
    fn message_store_paths_should_extend_existing_chat_conversation_layout() {
        let data_path = PathBuf::from("E:/app/data/app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");

        assert!(paths
            .legacy_conversation_file
            .to_string_lossy()
            .ends_with("chat\\conversations\\conversation-a.json")
            || paths
                .legacy_conversation_file
                .to_string_lossy()
                .ends_with("chat/conversations/conversation-a.json"));
        assert!(paths
            .messages_file
            .to_string_lossy()
            .contains("chat"));
        assert!(paths
            .messages_file
            .to_string_lossy()
            .ends_with("conversation-a\\messages.jsonl")
            || paths
                .messages_file
                .to_string_lossy()
                .ends_with("conversation-a/messages.jsonl"));
    }

    #[test]
    fn message_store_manifest_should_round_trip_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-manifest-{}",
            Uuid::new_v4()
        ));
        let manifest_path = root.join("chat").join("conversations").join("conversation-a").join("manifest.json");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation)
            .jsonl_snapshot_ready(128, 2);

        write_message_store_manifest_atomic(&manifest_path, &manifest).expect("write manifest");
        let loaded = read_message_store_manifest(&manifest_path)
            .expect("read manifest")
            .expect("manifest exists");

        assert_eq!(loaded, manifest);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_manifest_should_reject_unsupported_version() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-manifest-version-{}",
            Uuid::new_v4()
        ));
        let manifest_path = root
            .join("chat")
            .join("conversations")
            .join("conversation-a")
            .join("manifest.json");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        manifest.version = MESSAGE_STORE_MANIFEST_VERSION + 1;

        write_message_store_manifest_atomic(&manifest_path, &manifest).expect("write manifest");
        let err = read_message_store_manifest(&manifest_path)
            .expect_err("unsupported manifest version should fail");

        assert!(err.contains("manifest 版本不支持"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_run_migration_should_write_manifest_jsonl_and_index() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-run-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "summary_context_seed"),
        ]);

        let outcome = run_jsonl_snapshot_migration(&paths, &conversation, false)
            .expect("run migration");
        let manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");

        assert!(outcome.wrote_files);
        assert!(manifest.should_read_jsonl());
        assert!(paths.meta_file.exists());
        assert!(!paths.messages_file.exists());
        assert!(paths.blocks_dir.exists());
        assert!(paths.index_file.exists());
        let meta = read_conversation_shard_meta(&paths.meta_file).expect("read meta");
        assert_eq!(meta.id, conversation.id);
        let block_zero = paths.blocks_dir.join("000000.jsonl");
        let block_one = paths.blocks_dir.join("000001.jsonl");
        let report_zero = verify_jsonl_snapshot_file(&block_zero, 1, "m1").expect("verify block 0");
        let report_one = verify_jsonl_snapshot_file(&block_one, 1, "c1").expect("verify block 1");
        assert_eq!(report_zero.compaction_count, 0);
        assert_eq!(report_one.compaction_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_dry_run_should_not_write_files() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-dry-run-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);

        let outcome = run_jsonl_snapshot_migration(&paths, &conversation, true)
            .expect("dry run migration");

        assert!(!outcome.wrote_files);
        assert!(!paths.manifest_file.exists());
        assert!(!paths.messages_file.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_resume_should_rebuild_from_building_manifest() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-resume-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let building = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        write_message_store_manifest_atomic(&paths.manifest_file, &building).expect("write building");

        let outcome = resume_jsonl_snapshot_migration(&paths, &conversation)
            .expect("resume migration");

        assert!(outcome.wrote_files);
        assert!(outcome.manifest.should_read_jsonl());
        assert!(!paths.messages_file.exists());
        assert!(paths.blocks_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_resume_should_rebuild_broken_ready_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-resume-broken-ready-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        fs::write(paths.blocks_dir.join("000000.jsonl"), "").expect("break ready snapshot");

        let outcome = resume_jsonl_snapshot_migration(&paths, &conversation)
            .expect("resume broken ready snapshot");
        let report = verify_jsonl_snapshot_file(&paths.blocks_dir.join("000000.jsonl"), 1, "m1")
            .expect("verify rebuilt snapshot");
        let backups = fs::read_dir(&paths.shard_dir)
            .expect("read shard dir")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("manifest.json.corrupt."))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        assert!(outcome.wrote_files);
        assert!(outcome.manifest.should_read_jsonl());
        assert_eq!(report.message_count, 1);
        assert!(backups.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_resume_should_rebuild_corrupt_manifest() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-resume-corrupt-manifest-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        write_message_store_text_atomic(
            &paths.manifest_file,
            "json.tmp",
            "{broken manifest",
            "损坏 manifest 测试",
        )
        .expect("write corrupt manifest");

        let outcome = resume_jsonl_snapshot_migration(&paths, &conversation)
            .expect("resume corrupt manifest");
        let report = verify_jsonl_snapshot_file(&paths.blocks_dir.join("000000.jsonl"), 1, "m1")
            .expect("verify rebuilt snapshot");

        assert!(outcome.wrote_files);
        assert!(outcome.manifest.should_read_jsonl());
        assert_eq!(report.message_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_resume_should_reject_unsupported_manifest_version() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-resume-future-manifest-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        manifest.version = MESSAGE_STORE_MANIFEST_VERSION + 1;
        write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
            .expect("write future manifest");

        let err = resume_jsonl_snapshot_migration(&paths, &conversation)
            .expect_err("future manifest should fail");

        assert!(err.contains("manifest 版本不支持"));
        assert!(!paths.messages_file.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_resume_should_not_overwrite_reserved_ready_store() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-resume-reserved-ready-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        manifest.message_store_kind = MessageStoreKind::JsonlEventLog;
        manifest.migration_state = MessageStoreMigrationState::Ready;
        write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
            .expect("write reserved ready manifest");

        let outcome = resume_jsonl_snapshot_migration(&paths, &conversation)
            .expect("resume reserved ready store");

        assert!(!outcome.wrote_files);
        assert_eq!(outcome.manifest.message_store_kind, MessageStoreKind::JsonlEventLog);
        assert!(!paths.messages_file.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_rollback_should_mark_manifest_without_deleting_diagnostics() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-rollback-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");

        let manifest = rollback_jsonl_snapshot_migration(&paths, &conversation)
            .expect("rollback migration");

        assert_eq!(manifest.migration_state, MessageStoreMigrationState::Rollback);
        assert!(!manifest.should_read_jsonl());
        assert!(!paths.messages_file.exists());
        assert!(paths.blocks_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_rollback_manifest_should_not_require_conversation_messages() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-rollback-manifest-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let manifest = read_message_store_manifest_for_paths(&paths)
            .expect("read manifest")
            .expect("manifest exists");

        let rollback =
            rollback_message_store_manifest(&paths, manifest).expect("rollback manifest");

        assert_eq!(rollback.migration_state, MessageStoreMigrationState::Rollback);
        assert_eq!(rollback.message_store_kind, MessageStoreKind::ConversationJson);
        assert_eq!(rollback.source_message_count, 1);
        assert_eq!(rollback.last_message_id, "m1");
        assert_eq!(rollback.messages_jsonl_bytes, 0);
        assert!(!paths.messages_file.exists());
        assert!(paths.blocks_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_conversation_json_should_match_before_after_limit_semantics() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_message("m4", "assistant"),
        ]);
        let store = ConversationJsonMessageStore::new(&conversation);

        let before = store.read_messages_before("m4", 2).expect("before page");
        let after = store.read_messages_after("m1", 2).expect("after page");

        assert_eq!(before.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["m2", "m3"]);
        assert!(before.has_more);
        assert_eq!(after.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["m2", "m3"]);
        assert!(after.has_more);
    }

    #[test]
    fn message_store_compaction_segment_should_use_compaction_boundaries() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m3", "user"),
            test_compaction_message("c2", "summary_context_seed"),
            test_message("m4", "assistant"),
        ]);
        let store = ConversationJsonMessageStore::new(&conversation);

        let current = store.read_current_compaction_segment().expect("current segment");
        let previous = store
            .read_compaction_segment_before(current.boundary_message_id.as_deref().expect("boundary"))
            .expect("previous segment");

        assert_eq!(
            current.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c2", "m4"]
        );
        assert_eq!(current.boundary_message_id.as_deref(), Some("c2"));
        assert_eq!(current.previous_boundary_message_id.as_deref(), Some("c1"));
        assert!(current.has_previous_segment);
        assert_eq!(
            previous.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c1", "m3"]
        );
        assert_eq!(previous.boundary_message_id.as_deref(), Some("c1"));
        assert!(previous.has_previous_segment);
    }

    #[test]
    fn message_store_compaction_segment_without_boundary_should_return_whole_conversation() {
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
        ]);
        let store = ConversationJsonMessageStore::new(&conversation);

        let current = store.read_current_compaction_segment().expect("current segment");

        assert_eq!(
            current.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
        assert_eq!(current.boundary_message_id, None);
        assert!(!current.has_previous_segment);
    }

    #[test]
    fn message_store_jsonl_verification_should_read_fixture_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-jsonl-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create temp dir");
        let messages_path = root.join("messages.jsonl");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
        ]);
        let content = encode_jsonl_snapshot_messages(&conversation.messages).expect("encode");
        fs::write(&messages_path, content).expect("write fixture");

        let report = verify_jsonl_snapshot_file(&messages_path, 3, "m2").expect("verify fixture");
        let rebuilt = rebuild_jsonl_snapshot_index_from_file(&messages_path).expect("rebuild index");

        assert_eq!(report.compaction_count, 1);
        assert_eq!(rebuilt.items.len(), 3);
        assert_eq!(rebuilt.items[1].compaction_kind.as_deref(), Some("context_compaction"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_verification_should_reject_half_line_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-half-line-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create temp dir");
        let messages_path = root.join("messages.jsonl");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        let mut content = encode_jsonl_snapshot_messages(&conversation.messages).expect("encode");
        content.push_str("{\"kind\":\"message\"");
        fs::write(&messages_path, content).expect("write fixture");

        let err = verify_jsonl_snapshot_file(&messages_path, 2, "")
            .expect_err("half line should fail");

        assert!(err.contains("offset=") || err.contains("半行"));
        let _ = fs::remove_dir_all(root);
    }
}
