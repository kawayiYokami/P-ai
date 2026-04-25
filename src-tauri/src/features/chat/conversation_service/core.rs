impl ConversationService {
    fn conversation_has_active_chat_view(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> bool {
        let target = conversation_id.trim();
        if target.is_empty() {
            return false;
        }
        state
            .active_chat_view_bindings
            .lock()
            .map(|bindings| {
                bindings.values().any(|binding| {
                    let bound = binding.conversation_id.trim();
                    !bound.is_empty() && bound != "*" && bound == target
                })
            })
            .unwrap_or(false)
    }

    fn increment_conversation_unread_count_if_background(
        &self,
        state: &AppState,
        conversation: &mut Conversation,
        count: usize,
    ) {
        if count == 0 {
            return;
        }
        if self.conversation_has_active_chat_view(state, &conversation.id) {
            clear_conversation_unread_count(conversation);
            return;
        }
        increment_conversation_unread_count(conversation, count);
    }

    fn remote_im_runtime_state_should_cache_blocks(
        &self,
        runtime_state: &RemoteImContactRuntimeState,
    ) -> bool {
        runtime_state.presence_state == RemoteImPresenceState::Present
            || runtime_state.work_state == RemoteImWorkState::Busy
            || runtime_state.has_pending
    }

    fn collect_block_cache_whitelist_conversation_ids(
        &self,
        state: &AppState,
    ) -> Result<std::collections::HashSet<String>, String> {
        let mut ids = std::collections::HashSet::<String>::new();
        if let Ok(bindings) = state.active_chat_view_bindings.lock() {
            for binding in bindings.values() {
                let conversation_id = binding.conversation_id.trim();
                if !conversation_id.is_empty() {
                    ids.insert(conversation_id.to_string());
                }
            }
        }
        let active_contact_ids = state
            .remote_im_contact_runtime_states
            .lock()
            .map(|runtime_states| {
                runtime_states
                    .iter()
                    .filter(|(_, runtime_state)| {
                        self.remote_im_runtime_state_should_cache_blocks(runtime_state)
                    })
                    .map(|(contact_id, _)| contact_id.trim().to_string())
                    .filter(|contact_id| !contact_id.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !active_contact_ids.is_empty() {
            let contact_ids = active_contact_ids
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let runtime = state_read_runtime_state_cached(state)?;
            let mut unresolved_contact_ids = std::collections::HashSet::<String>::new();
            for contact in runtime
                .remote_im_contacts
                .iter()
                .filter(|contact| contact_ids.contains(contact.id.trim()))
            {
                if let Some(bound_conversation_id) = contact
                    .bound_conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    ids.insert(bound_conversation_id.to_string());
                } else {
                    unresolved_contact_ids.insert(contact.id.trim().to_string());
                }
            }
            if !unresolved_contact_ids.is_empty() {
                let chat_index = state_read_chat_index_cached(state)?;
                let conversation_key_map = runtime
                    .remote_im_contacts
                    .iter()
                    .filter(|contact| unresolved_contact_ids.contains(contact.id.trim()))
                    .map(|contact| {
                        (
                            remote_im_contact_conversation_key(contact),
                            contact.id.trim().to_string(),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>();
                let mapped_ids = chat_index
                    .conversations
                    .iter()
                    .filter_map(|item| {
                        let conversation =
                            match state_read_conversation_cached(state, item.id.as_str()) {
                                Ok(conversation) => conversation,
                                Err(err) => {
                                    eprintln!(
                                        "[会话索引读取] 状态=失败，任务=collect_block_cache_whitelist_conversation_ids，conversation_id={}，error={}",
                                        item.id, err
                                    );
                                    return None;
                                }
                            };
                        let root_key = conversation
                            .root_conversation_id
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())?;
                        if !conversation.summary.trim().is_empty()
                            || !conversation_is_remote_im_contact(&conversation)
                            || !conversation_key_map.contains_key(root_key)
                        {
                            return None;
                        }
                        Some(conversation.id)
                    })
                    .collect::<Vec<_>>();
                ids.extend(mapped_ids);
            }
        }
        Ok(ids)
    }

    fn retain_message_store_block_cache_whitelist(
        &self,
        state: &AppState,
    ) -> Result<(), String> {
        let conversation_ids = self.collect_block_cache_whitelist_conversation_ids(state)?;
        let mut allowed_paths = std::collections::HashSet::<PathBuf>::new();
        for conversation_id in conversation_ids {
            let paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
            if let Some(block_paths) =
                message_store::read_ready_message_store_latest_block_paths(&paths, 2)?
            {
                allowed_paths.extend(block_paths);
            }
        }
        message_store::retain_message_store_block_file_cache_paths(&allowed_paths);
        Ok(())
    }

    fn with_unarchived_conversation_by_id_fast<T>(
        &self,
        state: &AppState,
        conversation_id: &str,
        reader: impl FnOnce(&Conversation) -> Result<T, String>,
    ) -> Result<T, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let conversation = state_read_conversation_cached(state, normalized_conversation_id)
            .map_err(|err| {
                format!(
                    "Unarchived conversation not found: {normalized_conversation_id}: {err}"
                )
            })?;
        self.ensure_unarchived_foreground_conversation(&conversation, normalized_conversation_id)?;
        let result = reader(&conversation)?;
        drop(guard);
        Ok(result)
    }

    fn ensure_unarchived_foreground_conversation(
        &self,
        conversation: &Conversation,
        conversation_id: &str,
    ) -> Result<(), String> {
        if !conversation.summary.trim().is_empty()
            || !conversation_visible_in_foreground_lists(conversation)
        {
            return Err(format!(
                "Unarchived conversation not found: {}",
                conversation_id.trim()
            ));
        }
        Ok(())
    }

    fn find_remote_im_contact_by_conversation_in_data<'a>(
        &self,
        data: &'a AppData,
        conversation_id: &str,
    ) -> Option<&'a RemoteImContact> {
        let conversation = data
            .conversations
            .iter()
            .find(|item| item.id == conversation_id)?;
        let contact_conversation_key = conversation
            .root_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(key) = contact_conversation_key {
            return data
                .remote_im_contacts
                .iter()
                .find(|contact| remote_im_contact_conversation_key(contact) == key);
        }
        data.remote_im_contacts.iter().find(|contact| {
            contact
                .bound_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                == Some(conversation_id)
        })
    }

    fn with_unarchived_conversation_by_id<T>(
        &self,
        state: &AppState,
        conversation_id: &str,
        reader: impl FnOnce(&Conversation) -> Result<T, String>,
    ) -> Result<T, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let conversation = state_read_conversation_cached(state, normalized_conversation_id)
            .map_err(|err| {
                format!(
                    "Unarchived conversation not found: {normalized_conversation_id}: {err}"
                )
            })?;
        self.ensure_unarchived_foreground_conversation(&conversation, normalized_conversation_id)?;
        let result = reader(&conversation)?;
        drop(guard);
        Ok(result)
    }

    fn read_persisted_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Conversation, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        state_read_conversation_cached(state, normalized_conversation_id)
    }

    fn try_read_persisted_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Option<Conversation>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(None);
        }
        match self.read_persisted_conversation(state, normalized_conversation_id) {
            Ok(conversation) => Ok(Some(conversation)),
            Err(err) if err.contains("not found") || err.contains("不存在") => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn try_read_unarchived_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Option<Conversation>, String> {
        Ok(self
            .try_read_persisted_conversation(state, conversation_id)?
            .filter(|conversation| conversation.summary.trim().is_empty()))
    }

    fn resolve_latest_foreground_conversation_id(
        &self,
        state: &AppState,
        agent_id: &str,
    ) -> Result<Option<String>, String> {
        let normalized_agent_id = agent_id.trim();
        if normalized_agent_id.is_empty() {
            let runtime = state_read_runtime_state_cached(state)?;
            if let Some(main_conversation_id) = runtime
                .main_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if let Some(conversation) =
                    self.try_read_unarchived_conversation(state, main_conversation_id)?
                {
                    if conversation_visible_in_foreground_lists(&conversation) {
                        return Ok(Some(conversation.id));
                    }
                }
            }
        }
        let chat_index = state_read_chat_index_cached(state)?;
        Ok(chat_index
            .conversations
            .iter()
            .rev()
            .find_map(|item| {
                if !item.summary.trim().is_empty() {
                    return None;
                }
                let conversation = state_read_conversation_cached(state, &item.id).ok()?;
                if !conversation_visible_in_foreground_lists(&conversation) {
                    return None;
                }
                if !normalized_agent_id.is_empty()
                    && conversation.agent_id.trim() != normalized_agent_id
                {
                    return None;
                }
                Some(conversation.id)
            }))
    }

    fn update_persisted_conversation_shell_workspace(
        &self,
        state: &AppState,
        conversation_id: &str,
        shell_workspace_path: Option<Option<String>>,
        shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
    ) -> Result<Conversation, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("指定会话不存在：".to_string());
        }
        let mut conversation = self
            .read_persisted_conversation(state, normalized_conversation_id)
            .map_err(|_| format!("指定会话不存在：{normalized_conversation_id}"))?;
        let original_path = conversation.shell_workspace_path.clone();
        let original_workspaces = conversation.shell_workspaces.clone();
        if let Some(value) = shell_workspace_path {
            conversation.shell_workspace_path = value;
        }
        if let Some(value) = shell_workspaces {
            conversation.shell_workspaces = value;
        }
        if conversation
            .shell_workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
            && terminal_workspace_path_from_conversation(state, &conversation).is_none()
        {
            conversation.shell_workspace_path = None;
        }
        if conversation.shell_workspace_path == original_path
            && conversation.shell_workspaces == original_workspaces
        {
            return Ok(conversation);
        }
        self.persist_conversation_with_chat_index(state, &conversation)?;
        Ok(conversation)
    }

    fn read_remote_im_contact_session_context(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<(Conversation, RemoteImChannelConfig, RemoteImContact), String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("联系人专用工具缺少 conversation_id，无法定位当前联系人".to_string());
        }
        let config = state_read_config_cached(state)?;
        let conversation = self
            .read_persisted_conversation(state, normalized_conversation_id)
            .map_err(|_| format!("当前会话不存在: conversation_id={normalized_conversation_id}"))?;
        if !conversation.summary.trim().is_empty() {
            return Err(format!("当前会话不存在: conversation_id={normalized_conversation_id}"));
        }
        if !conversation_is_remote_im_contact(&conversation) {
            return Err("联系人专用工具仅可用于联系人会话".to_string());
        }
        let runtime = state_read_runtime_state_cached(state)?;
        let contact_conversation_key = conversation
            .root_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let contact = runtime
            .remote_im_contacts
            .iter()
            .find(|contact| {
                if let Some(key) = contact_conversation_key {
                    remote_im_contact_conversation_key(contact) == key
                } else {
                    contact
                        .bound_conversation_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        == Some(normalized_conversation_id)
                }
            })
            .cloned()
            .ok_or_else(|| format!("未找到当前会话绑定的联系人: conversation_id={normalized_conversation_id}"))?;
        let channel = remote_im_channel_by_id(&config, &contact.channel_id)
            .cloned()
            .ok_or_else(|| format!("远程 IM 渠道不存在: {}", contact.channel_id))?;
        if !channel.enabled {
            return Err(format!("远程 IM 渠道未启用: {}", contact.channel_id));
        }
        Ok((conversation, channel, contact))
    }

    fn is_remote_im_contact_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<bool, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(false);
        }
        match self.read_persisted_conversation(state, normalized_conversation_id) {
            Ok(conversation) => Ok(
                conversation.summary.trim().is_empty()
                    && conversation_is_remote_im_contact(&conversation),
            ),
            Err(err) if err.contains("not found") => Ok(false),
            Err(err) => Err(err),
        }
    }

}
