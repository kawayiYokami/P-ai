const DELEGATE_CONVERSATIONS_DIR_NAME: &str = "delegate-conversations";

fn delegate_conversation_store_dir(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path).join(DELEGATE_CONVERSATIONS_DIR_NAME)
}

fn validate_delegate_conversation_id(conversation_id: &str) -> Result<(), String> {
    if conversation_id.trim().is_empty() {
        return Err("委托会话 ID 不能为空".to_string());
    }
    if conversation_id != conversation_id.trim() {
        return Err(format!(
            "委托会话 ID 不能包含首尾空白，conversation_id={conversation_id}"
        ));
    }
    if conversation_id.contains('/') || conversation_id.contains('\\') {
        return Err(format!(
            "委托会话 ID 不能包含路径分隔符，conversation_id={conversation_id}"
        ));
    }
    if conversation_id
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err(format!(
            "委托会话 ID 不能包含 Windows 文件名非法字符，conversation_id={conversation_id}"
        ));
    }
    if conversation_id.ends_with([' ', '.']) {
        return Err(format!(
            "委托会话 ID 不能以 Windows 不稳定文件名字符结尾，conversation_id={conversation_id}"
        ));
    }
    let mut components = std::path::Path::new(conversation_id).components();
    let Some(component) = components.next() else {
        return Err("委托会话 ID 不能为空".to_string());
    };
    if components.next().is_some() || !matches!(component, std::path::Component::Normal(_)) {
        return Err(format!(
            "委托会话 ID 不能包含路径组件，conversation_id={conversation_id}"
        ));
    }
    Ok(())
}

fn delegate_conversation_message_store_paths(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<message_store::MessageStorePaths, String> {
    let conversation_id = conversation_id.trim();
    validate_delegate_conversation_id(conversation_id)?;
    let store_dir = delegate_conversation_store_dir(data_path);
    message_store::message_store_paths_for_shard_dir(
        data_path,
        conversation_id,
        store_dir.join(conversation_id),
        store_dir.join(format!("{conversation_id}.json")),
    )
}

fn validate_delegate_conversation_record(
    conversation: &Conversation,
    requested_conversation_id: &str,
) -> Result<(), String> {
    if conversation.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE {
        return Err(format!(
            "委托会话文件类型不正确，conversation_id={}，conversation_kind={}",
            conversation.id, conversation.conversation_kind
        ));
    }
    if conversation.id.trim() != requested_conversation_id.trim() {
        return Err(format!(
            "委托会话文件 ID 不匹配，requested={}，actual={}",
            requested_conversation_id.trim(), conversation.id
        ));
    }
    Ok(())
}

fn validate_delegate_conversation_for_write(conversation: &Conversation) -> Result<(), String> {
    validate_delegate_conversation_id(&conversation.id)?;
    validate_delegate_conversation_record(conversation, &conversation.id)?;
    if conversation
        .root_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err(format!(
            "委托会话缺少 root_conversation_id，conversation_id={}",
            conversation.id
        ));
    }
    Ok(())
}

fn validate_delegate_conversation_meta(
    meta: &message_store::ConversationShardMeta,
    requested_conversation_id: &str,
) -> Result<(), String> {
    if meta.conversation_kind().trim() != CONVERSATION_KIND_DELEGATE {
        return Err(format!(
            "委托会话元数据类型不正确，conversation_id={}，conversation_kind={}",
            meta.id(),
            meta.conversation_kind()
        ));
    }
    if meta.id().trim() != requested_conversation_id.trim() {
        return Err(format!(
            "委托会话元数据 ID 不匹配，requested={}，actual={}",
            requested_conversation_id.trim(),
            meta.id()
        ));
    }
    Ok(())
}

fn delegate_conversation_store_read_meta(
    paths: &message_store::MessageStorePaths,
    conversation_id: &str,
) -> Result<Option<message_store::ConversationShardMeta>, String> {
    let Some(meta) = message_store::delegate_directory_store_read_meta(paths)? else {
        return Ok(None);
    };
    validate_delegate_conversation_meta(&meta, conversation_id)?;
    Ok(Some(meta))
}

fn delegate_conversation_store_read(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Option<Conversation>, String> {
    let conversation_id = conversation_id.trim();
    let paths = delegate_conversation_message_store_paths(data_path, conversation_id)?;
    match message_store::delegate_directory_store_read_conversation(&paths) {
        Ok(Some(conversation)) => {
            validate_delegate_conversation_record(&conversation, conversation_id)?;
            Ok(Some(conversation))
        }
        Ok(None) => Ok(None),
        Err(err) => Err(err),
    }
}

fn delegate_conversation_store_write(
    data_path: &PathBuf,
    conversation: &Conversation,
) -> Result<(), String> {
    validate_delegate_conversation_for_write(conversation)?;
    if conversation.messages.is_empty() {
        runtime_log_info(format!(
            "[委托会话] 跳过，任务=写入空委托会话，conversation_id={}",
            conversation.id
        ));
        return Ok(());
    }
    fs::create_dir_all(delegate_conversation_store_dir(data_path)).map_err(|err| {
        format!(
            "创建委托会话目录失败，path={}，error={err}",
            delegate_conversation_store_dir(data_path).display()
        )
    })?;
    let paths = delegate_conversation_message_store_paths(data_path, &conversation.id)?;
    message_store::delegate_directory_store_write_if_changed(&paths, conversation)?;
    delegate_snapshot_store_sync_from_conversation(data_path, conversation)?;
    Ok(())
}

fn delegate_conversation_store_delete(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let paths = delegate_conversation_message_store_paths(data_path, conversation_id)?;
    let deleted = message_store::delegate_directory_store_delete(&paths)?;
    let deleted_snapshot = delegate_snapshot_cache_delete(data_path, conversation_id)?;
    Ok(deleted || deleted_snapshot)
}

fn delegate_conversation_store_collect_ids(
    data_path: &PathBuf,
) -> Result<std::collections::BTreeSet<String>, String> {
    let dir = delegate_conversation_store_dir(data_path);
    if !dir.exists() {
        return Ok(std::collections::BTreeSet::new());
    }
    let mut ids = std::collections::BTreeSet::<String>::new();
    for entry in fs::read_dir(&dir).map_err(|err| {
        format!("读取委托会话目录失败，path={}，error={err}", dir.display())
    })? {
        let entry = entry.map_err(|err| format!("读取委托会话目录项失败: {err}"))?;
        let path = entry.path();
        // 正常业务路径只认当前目录型正文仓库；旧平面 JSON 只能由迁移/清理服务处理，绝不能在这里补读。
        if !path.is_dir() {
            continue;
        }
        let conversation_id = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if conversation_id.is_empty() || validate_delegate_conversation_id(&conversation_id).is_err()
        {
            continue;
        }
        ids.insert(conversation_id);
    }
    Ok(ids)
}

fn delegate_conversation_store_list(data_path: &PathBuf) -> Result<Vec<Conversation>, String> {
    let mut conversations = Vec::new();
    for conversation_id in delegate_conversation_store_collect_ids(data_path)? {
        if let Some(conversation) = delegate_conversation_store_read(data_path, &conversation_id)? {
            conversations.push(conversation);
        }
    }
    Ok(conversations)
}

fn delegate_conversation_store_read_block_page(
    data_path: &PathBuf,
    conversation_id: &str,
    requested_block_id: Option<u32>,
) -> Result<Option<message_store::MessageStoreBlockPage>, String> {
    let conversation_id = conversation_id.trim();
    let paths = delegate_conversation_message_store_paths(data_path, conversation_id)?;
    if delegate_conversation_store_read_meta(&paths, conversation_id)?.is_none() {
        return Ok(None);
    }
    message_store::delegate_directory_store_read_block_page(&paths, requested_block_id)
}

#[cfg(test)]
mod delegate_conversation_store_tests {
    use super::*;

    fn test_delegate_entry(data_path: &PathBuf) -> DelegateEntry {
        delegate_store_create_delegate(
            data_path,
            &DelegateCreateInput {
                kind: "delegate".to_string(),
                conversation_id: "root-conversation".to_string(),
                parent_delegate_id: None,
                source_agent_id: "source-agent".to_string(),
                target_agent_id: DEFAULT_AGENT_ID.to_string(),
                title: "委托会话".to_string(),
                why: "测试委托会话存储".to_string(),
                goal: "验证委托会话存储".to_string(),
                todo: "写入并读取会话".to_string(),
                notify_assistant_when_done: false,
                call_stack: Vec::new(),
            },
        )
        .expect("create delegate record")
    }

    fn test_message(id: &str, role: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: format!("2026-06-08T00:00:0{}Z", id.len()),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: format!("message {id}"),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        }
    }

    fn test_delegate_conversation(conversation_id: &str) -> Conversation {
        Conversation {
            id: conversation_id.to_string(),
            title: "委托会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_DELEGATE.to_string(),
            root_conversation_id: Some("root-conversation".to_string()),
            delegate_id: Some(conversation_id.to_string()),
            created_at: "2026-06-08T00:00:00Z".to_string(),
            updated_at: "2026-06-08T00:00:02Z".to_string(),
            last_user_at: Some("2026-06-08T00:00:01Z".to_string()),
            last_assistant_at: Some("2026-06-08T00:00:02Z".to_string()),
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![test_message("m1", "user"), test_message("m2", "assistant")],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    #[test]
    fn delegate_conversation_store_should_write_and_read_directory_store() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-store-write-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let entry = test_delegate_entry(&data_path);
        let conversation = test_delegate_conversation(&entry.delegate_id);

        delegate_conversation_store_write(&data_path, &conversation).expect("write delegate");
        let page = delegate_conversation_store_read_block_page(&data_path, &conversation.id, None)
            .expect("read block page")
            .expect("page exists");

        assert_eq!(page.messages.len(), 2);
        assert_eq!(page.blocks.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delegate_conversation_store_delete_should_remove_directory_store() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-delegate-store-delete-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let entry = test_delegate_entry(&data_path);
        let conversation = test_delegate_conversation(&entry.delegate_id);
        delegate_conversation_store_write(&data_path, &conversation).expect("write delegate");
        let shard_dir = delegate_conversation_store_dir(&data_path).join(&conversation.id);

        let deleted =
            delegate_conversation_store_delete(&data_path, &conversation.id).expect("delete");

        assert!(deleted);
        assert!(!shard_dir.exists());
        let _ = fs::remove_dir_all(root);
    }
}