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

fn delegate_conversation_store_path(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<PathBuf, String> {
    let conversation_id = conversation_id.trim();
    validate_delegate_conversation_id(conversation_id)?;
    Ok(delegate_conversation_store_dir(data_path).join(format!("{conversation_id}.json")))
}

fn delegate_conversation_store_read(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Option<Conversation>, String> {
    let path = delegate_conversation_store_path(data_path, conversation_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let conversation = read_json_file::<Conversation>(&path, "delegate conversation file")?;
    if conversation.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE {
        return Err(format!(
            "委托会话文件类型不正确，conversation_id={}，conversation_kind={}",
            conversation.id, conversation.conversation_kind
        ));
    }
    if conversation.id.trim() != conversation_id.trim() {
        return Err(format!(
            "委托会话文件 ID 不匹配，requested={}，actual={}",
            conversation_id.trim(), conversation.id
        ));
    }
    Ok(Some(conversation))
}

fn delegate_conversation_store_write(
    data_path: &PathBuf,
    conversation: &Conversation,
) -> Result<(), String> {
    validate_delegate_conversation_id(&conversation.id)?;
    if conversation.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE {
        return Err(format!(
            "拒绝写入非委托会话，conversation_id={}，conversation_kind={}",
            conversation.id, conversation.conversation_kind
        ));
    }
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
    let path = delegate_conversation_store_path(data_path, &conversation.id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!("创建委托会话目录失败，path={}，error={err}", parent.display())
        })?;
    }
    write_json_file_atomic(&path, conversation, "delegate conversation file")
}

fn delegate_conversation_store_delete(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let path = delegate_conversation_store_path(data_path, conversation_id)?;
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|err| {
        format!(
            "删除委托会话文件失败，path={}，error={err}",
            path.display()
        )
    })?;
    Ok(true)
}

fn delegate_conversation_store_list(data_path: &PathBuf) -> Result<Vec<Conversation>, String> {
    let dir = delegate_conversation_store_dir(data_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut conversations = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|err| {
        format!("读取委托会话目录失败，path={}，error={err}", dir.display())
    })? {
        let entry = entry.map_err(|err| format!("读取委托会话目录项失败: {err}"))?;
        let path = entry.path();
        if !path.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let conversation = read_json_file::<Conversation>(&path, "delegate conversation file")?;
        if conversation.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE {
            conversations.push(conversation);
        }
    }
    Ok(conversations)
}
