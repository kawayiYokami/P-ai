impl ConversationService {
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
        let result = with_app_data_cached_ref(state, |data, _detail| {
            let conversation = data
                .conversations
                .iter()
                .find(|item| {
                    item.id == normalized_conversation_id
                        && item.summary.trim().is_empty()
                        && conversation_visible_in_foreground_lists(item)
                })
                .ok_or_else(|| {
                    format!(
                        "Unarchived conversation not found: {normalized_conversation_id}"
                    )
                })?;
            self.ensure_unarchived_foreground_conversation(
                conversation,
                normalized_conversation_id,
            )?;
            reader(conversation)
        })?;
        drop(guard);
        Ok(result)
    }

    fn get_or_ensure_cached_app_data(&self, state: &AppState) -> Result<AppData, String> {
        {
            let cached = state
                .cached_app_data
                .lock()
                .map_err(|err| format!("Failed to lock cached app data: {:?}", err))?;
            if let Some(data) = cached.as_ref() {
                return Ok(data.clone());
            }
        }

        let loaded = state_read_app_data_cached(state)?;
        let mut cached = state
            .cached_app_data
            .lock()
            .map_err(|err| format!("Failed to lock cached app data: {:?}", err))?;
        if cached.is_none() {
            *cached = Some(loaded);
        }
        cached
            .as_ref()
            .cloned()
            .ok_or_else(|| "Cached app data is unexpectedly missing".to_string())
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

    fn find_remote_im_contact_conversation_id_in_data(
        &self,
        data: &AppData,
        contact: &RemoteImContact,
    ) -> Option<String> {
        let key = remote_im_contact_conversation_key(contact);
        data.conversations
            .iter()
            .find(|conversation| {
                conversation.summary.trim().is_empty()
                    && conversation_is_remote_im_contact(conversation)
                    && conversation.root_conversation_id.as_deref().map(str::trim)
                        == Some(key.as_str())
            })
            .map(|conversation| conversation.id.clone())
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

    fn conversation_has_remote_im_platform_message_in_data(
        &self,
        data: &AppData,
        conversation_id: &str,
        channel_id: &str,
        remote_contact_type: &str,
        remote_contact_id: &str,
        platform_message_id: &str,
    ) -> bool {
        data.conversations.iter().any(|conversation| {
            conversation.id == conversation_id
                && conversation_has_remote_im_platform_message(
                    conversation,
                    channel_id,
                    remote_contact_type,
                    remote_contact_id,
                    platform_message_id,
                )
        })
    }

    fn resolve_prompt_preview_conversation(
        &self,
        data: &AppData,
        requested_conversation_id: Option<&str>,
        effective_agent_id: &str,
    ) -> Conversation {
        requested_conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|conversation_id| {
                data.conversations
                    .iter()
                    .find(|conversation| {
                        conversation.id == conversation_id
                            && conversation.summary.trim().is_empty()
                            && !conversation_is_delegate(conversation)
                    })
                    .cloned()
            })
            .or_else(|| {
                latest_active_conversation_index(data, "", effective_agent_id)
                    .and_then(|idx| data.conversations.get(idx).cloned())
            })
            .unwrap_or_else(|| Conversation {
                id: "preview".to_string(),
                title: "Preview".to_string(),
                agent_id: effective_agent_id.to_string(),
                department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
                bound_conversation_id: None,
                parent_conversation_id: None,
                child_conversation_ids: Vec::new(),
                fork_message_cursor: None,
                last_read_message_id: String::new(),
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                status: "active".to_string(),
                summary: String::new(),
                user_profile_snapshot: String::new(),
                shell_workspace_path: None,
                shell_workspaces: Vec::new(),
                archived_at: None,
                messages: Vec::new(),
                current_todos: Vec::new(),
                memory_recall_table: Vec::new(),
                plan_mode_enabled: false,
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
        let data = self.get_or_ensure_cached_app_data(state)?;
        let result = {
            let conversation = data
                .conversations
                .iter()
                .find(|item| {
                    item.id == normalized_conversation_id
                        && item.summary.trim().is_empty()
                        && conversation_visible_in_foreground_lists(item)
                })
                .ok_or_else(|| format!("Unarchived conversation not found: {normalized_conversation_id}"))?;
            self.ensure_unarchived_foreground_conversation(
                conversation,
                normalized_conversation_id,
            )?;
            reader(conversation)?
        };
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
        let data = state_read_app_data_cached(state)?;
        let normalized_agent_id = agent_id.trim();
        Ok(data
            .conversations
            .iter()
            .rev()
            .find(|item| {
                item.summary.trim().is_empty()
                    && !conversation_is_delegate(item)
                    && (normalized_agent_id.is_empty() || item.agent_id.trim() == normalized_agent_id)
            })
            .map(|conversation| conversation.id.clone()))
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
        let data = state_read_app_data_cached(state)?;
        let contact = self
            .find_remote_im_contact_by_conversation_in_data(&data, normalized_conversation_id)
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
