impl ConversationServiceV2 {
    fn resolve_delegate_context(
        &self,
        app_state: &AppState,
        source_agent_id: &str,
        source_conversation_id: Option<&str>,
        target_agent_id: &str,
    ) -> Result<DelegateContextResolution, String> {
        let guard = app_state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let runtime_snapshot = load_runtime_organization_snapshot(app_state)?;
        let requested_source_conversation_id = source_conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let thread_context = if let Some(conversation_id) = requested_source_conversation_id {
            delegate_runtime_thread_get(app_state, conversation_id)?
        } else {
            None
        };
        let source_conversation = if let Some(thread) = thread_context.as_ref() {
            Some(thread.conversation.clone())
        } else if let Some(conversation_id) = requested_source_conversation_id {
            Some(
                self.get_conversation_meta(app_state, conversation_id)
                    .ok()
                    .filter(|conversation_meta| {
                        self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                            && conversation_meta.conversation_kind.trim()
                                != CONVERSATION_KIND_DELEGATE
                    })
                    .map(|conversation_meta| {
                        self.build_conversation_record_from_meta_view(&conversation_meta)
                    })
                    .ok_or_else(|| {
                        format!("未找到指定来源会话，conversationId={conversation_id}")
                    })?,
            )
        } else {
            None
        };
        let source_agent_id = source_conversation
            .as_ref()
            .map(|conversation| conversation.agent_id.trim().to_string())
            .filter(|agent_id| !agent_id.is_empty())
            .unwrap_or_else(|| source_agent_id.trim().to_string());
        let source_agent = runtime_available_agent(&runtime_snapshot, &source_agent_id)
            .cloned()
            .ok_or_else(|| format!("未找到发起人格，agentId={source_agent_id}"))?;
        let target_agent_id = target_agent_id.trim();
        if target_agent_id.is_empty() {
            drop(guard);
            return Err("缺少目标人格，无法发起委托".to_string());
        }
        let target_agent = runtime_resolve_agent_ref(&runtime_snapshot, target_agent_id)
            .map_err(|err| format!("目标人格解析失败：{err}"))?
            .clone();
        if !runtime_agent_is_available(&target_agent) {
            drop(guard);
            return Err(format!("目标人格不可用，agentId={}", target_agent.id));
        }
        let source_conversation_id = if let Some(thread) = thread_context.as_ref() {
            thread.root_conversation_id.clone()
        } else {
            source_conversation
                .as_ref()
                .map(|conversation| conversation.id.clone())
                .ok_or_else(|| "主代理缺少当前会话 ID，无法发起委托".to_string())?
        };
        drop(guard);
        Ok(DelegateContextResolution {
            config: runtime_snapshot.config,
            agents: runtime_snapshot.agents,
            source_agent,
            target_agent,
            source_conversation_id,
            thread_context,
        })
    }

    fn resolve_delegate_result_target_conversation(
        &self,
        state: &AppState,
        root_conversation_id: &str,
    ) -> Result<DelegateResultTargetConversationResolution, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let assistant_agent_id = state_service_get_assistant_agent_id(state)?;
        let normalized_root_conversation_id = root_conversation_id.trim();
        let mut conversation_to_persist = None::<Conversation>;
        let target_conversation_id =
            if task_conversation_id_is_system_notification(normalized_root_conversation_id) {
                if let Some(conversation_meta) = self
                    .get_conversation_meta(state, SYSTEM_NOTIFICATION_CONVERSATION_ID)
                    .ok()
                    .filter(|conversation_meta| {
                        self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                            && conversation_meta.visible_in_foreground_lists
                            && self.conversation_meta_is_system_notification_meta_view(
                                conversation_meta,
                            )
                    })
                {
                    conversation_meta.id
                } else {
                    let conversation = build_system_notification_conversation_record();
                    let conversation_id = conversation.id.clone();
                    conversation_to_persist = Some(conversation);
                    conversation_id
                }
            } else if self
                .get_conversation_meta(state, normalized_root_conversation_id)
                .ok()
                .filter(|conversation_meta| {
                    self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                        && conversation_meta.conversation_kind.trim()
                            != CONVERSATION_KIND_DELEGATE
                        && conversation_meta.conversation_kind.trim()
                            != CONVERSATION_KIND_SYSTEM_NOTIFICATION
                })
                .is_some()
            {
                normalized_root_conversation_id.to_string()
            } else {
                return Err(format!(
                    "委托绑定会话不存在，无法写回结果，conversationId={normalized_root_conversation_id}"
                ));
            };
        drop(guard);
        if let Some(conversation) = conversation_to_persist {
            state_schedule_conversation_persist(state, &conversation)?;
        }
        Ok(DelegateResultTargetConversationResolution {
            agent_id: assistant_agent_id,
            target_conversation_id,
        })
    }

}

