impl ConversationService {
    fn resolve_delegate_result_target_conversation(
        &self,
        state: &AppState,
        root_conversation_id: &str,
    ) -> Result<DelegateResultTargetConversationResolution, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let config = read_config(&state.config_path)?;
        let assistant_agent_id = assistant_department_agent_id(&config)
            .ok_or_else(|| "未找到助理部门委任人".to_string())?;
        let department_id = department_for_agent_id(&config, &assistant_agent_id)
            .map(|item| item.id.clone())
            .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
        let target_conversation_id = if state_read_conversation_cached(state, root_conversation_id)
            .ok()
            .filter(|conversation| {
                conversation.summary.trim().is_empty() && !conversation_is_delegate(conversation)
            })
            .is_some()
        {
            root_conversation_id.trim().to_string()
        } else if let Some(conversation_id) =
            self.resolve_latest_foreground_conversation_id(state, &assistant_agent_id)?
        {
            eprintln!(
                "[委托线程] 原始会话不可用，委托结果回退到前台会话: requested_conversation_id={}, fallback_conversation_id={}",
                root_conversation_id,
                conversation_id
            );
            conversation_id
        } else {
            let conversation = build_conversation_record(
                "",
                &assistant_agent_id,
                &department_id,
                "",
                CONVERSATION_KIND_CHAT,
                None,
                None,
            );
            let conversation_id = conversation.id.clone();
            state_schedule_conversation_persist(state, &conversation, true)?;
            let mut runtime = state_read_runtime_state_cached(state)?;
            runtime.main_conversation_id = Some(conversation_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
            eprintln!(
                "[委托线程] 原始会话不可用，委托结果新建主会话: requested_conversation_id={}, fallback_conversation_id={}",
                root_conversation_id,
                conversation_id
            );
            conversation_id
        };
        drop(guard);
        Ok(DelegateResultTargetConversationResolution {
            department_id,
            agent_id: assistant_agent_id,
            target_conversation_id,
        })
    }

    fn resolve_delegate_context(
        &self,
        app_state: &AppState,
        source_agent_id: &str,
        source_conversation_id: Option<&str>,
        target_department_id: &str,
    ) -> Result<DelegateContextResolution, String> {
        let guard = app_state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let mut config = read_config(&app_state.config_path)?;
        let mut agents = state_read_agents_cached(app_state)?;
        merge_private_organization_into_runtime(
            &app_state.data_path,
            &mut config,
            &mut agents,
        )?;
        let source_department = department_for_agent_id(&config, source_agent_id)
            .cloned()
            .ok_or_else(|| format!("未找到发起部门，agentId={source_agent_id}"))?;
        let target_department = department_by_id(&config, target_department_id)
            .cloned()
            .ok_or_else(|| format!("目标部门不存在，departmentId={target_department_id}"))?;
        let target_agent_id = target_department
            .agent_ids
            .iter()
            .find(|id| !id.trim().is_empty())
            .cloned()
            .ok_or_else(|| format!("目标部门没有可用委任人，departmentId={target_department_id}"))?;
        if !agents
            .iter()
            .any(|agent| agent.id == target_agent_id && !agent.is_built_in_user)
        {
            drop(guard);
            return Err(format!("目标委任人不存在，agentId={target_agent_id}"));
        }
        let thread_context = if let Some(conversation_id) = source_conversation_id {
            delegate_runtime_thread_get(app_state, conversation_id)?
        } else {
            None
        };
        let source_conversation_id = if let Some(thread) = thread_context.as_ref() {
            thread.root_conversation_id.clone()
        } else {
            let requested_conversation_id = source_conversation_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "主代理缺少当前会话 ID，无法发起委托".to_string())?;
            state_read_conversation_cached(app_state, requested_conversation_id)
                .ok()
                .filter(|conversation| {
                    conversation.summary.trim().is_empty()
                        && !conversation_is_delegate(conversation)
                })
                .map(|conversation| conversation.id)
                .ok_or_else(|| format!("未找到指定主会话，conversationId={requested_conversation_id}"))?
        };
        drop(guard);
        Ok(DelegateContextResolution {
            config,
            agents,
            source_department,
            target_department,
            target_agent_id,
            source_conversation_id,
            thread_context,
        })
    }


    fn resolve_prompt_prepare_conversation_from_data_read_only(
        &self,
        data: &AppData,
        data_path: &PathBuf,
        runtime_conversation_id: Option<&str>,
        runtime_conversation: &Conversation,
        selected_api: &ApiConfig,
        effective_agent_id: &str,
        requested_conversation_id: Option<&str>,
    ) -> Result<Option<PromptPrepareConversationResolution>, String> {
        let mut cloned = data.clone();
        self.resolve_prompt_prepare_conversation_core(
            &mut cloned,
            data_path,
            runtime_conversation_id,
            runtime_conversation,
            selected_api,
            effective_agent_id,
            requested_conversation_id,
            true,
        )
    }

    fn resolve_prompt_prepare_conversation_core(
        &self,
        data: &mut AppData,
        data_path: &PathBuf,
        runtime_conversation_id: Option<&str>,
        runtime_conversation: &Conversation,
        selected_api: &ApiConfig,
        effective_agent_id: &str,
        requested_conversation_id: Option<&str>,
        read_only: bool,
    ) -> Result<Option<PromptPrepareConversationResolution>, String> {
        let Some((idx, is_runtime_conversation)) = resolve_prompt_prepare_target(
            data,
            data_path,
            runtime_conversation_id,
            selected_api,
            effective_agent_id,
            requested_conversation_id,
            read_only,
        )? else {
            return Ok(None);
        };

        if idx.is_some() && !read_only {
            for conversation in &mut data.conversations {
                if conversation_is_delegate(conversation) || !conversation.summary.trim().is_empty()
                {
                    continue;
                }
                conversation.status = "active".to_string();
            }
        }

        let conversation_before = if let Some(actual_idx) = idx {
            data.conversations
                .get(actual_idx)
                .cloned()
                .ok_or_else(|| "前台会话索引无效".to_string())?
        } else {
            runtime_conversation.clone()
        };
        Ok(Some(build_prompt_prepare_resolution(
            data,
            &conversation_before,
            selected_api,
            is_runtime_conversation,
        )))
    }

}

fn resolve_prompt_prepare_target(
    data: &mut AppData,
    data_path: &PathBuf,
    runtime_conversation_id: Option<&str>,
    selected_api: &ApiConfig,
    effective_agent_id: &str,
    requested_conversation_id: Option<&str>,
    read_only: bool,
) -> Result<Option<(Option<usize>, bool)>, String> {
    let requested_conversation_idx = requested_conversation_id.and_then(|conversation_id| {
        data.conversations
            .iter()
            .position(|item| item.id == conversation_id && item.summary.trim().is_empty())
    });
    let is_runtime_conversation = requested_conversation_id.is_some()
        && requested_conversation_idx.is_none()
        && runtime_conversation_id.is_some();
    let idx = if let Some(requested_idx) = requested_conversation_idx {
        Some(requested_idx)
    } else if is_runtime_conversation {
        None
    } else if let Some(conversation_id) = requested_conversation_id {
        if read_only {
            return Ok(None);
        }
        Some(
            data.conversations
                .iter()
                .position(|item| item.id == conversation_id && item.summary.trim().is_empty())
                .ok_or_else(|| format!("指定会话不存在或不可用：{conversation_id}"))?,
        )
    } else if read_only {
        active_foreground_conversation_index_read_only(data, effective_agent_id)
    } else {
        Some(ensure_active_foreground_conversation_index_atomic(
            data,
            data_path,
            &selected_api.id,
            effective_agent_id,
        ))
    };
    Ok(Some((idx, is_runtime_conversation)))
}

fn build_prompt_prepare_resolution(
    data: &AppData,
    conversation_before: &Conversation,
    selected_api: &ApiConfig,
    is_runtime_conversation: bool,
) -> PromptPrepareConversationResolution {
    let is_remote_im_contact_conversation = conversation_is_remote_im_contact(conversation_before);
    let remote_im_contact_processing_mode = if is_remote_im_contact_conversation {
        remote_im_find_contact_by_conversation(data, &conversation_before.id)
            .map(|contact| normalize_contact_processing_mode(&contact.processing_mode))
            .unwrap_or_else(|| "continuous".to_string())
    } else {
        "continuous".to_string()
    };
    PromptPrepareConversationResolution {
        conversation_before: build_prompt_prepare_conversation_before(
            conversation_before,
            is_remote_im_contact_conversation,
            &remote_im_contact_processing_mode,
        ),
        last_archive_summary: prompt_prepare_last_archive_summary(
            data,
            conversation_before,
            is_runtime_conversation,
        ),
        is_remote_im_contact_conversation,
        remote_im_contact_processing_mode,
        response_style_id: data.response_style_id.clone(),
        user_name: user_persona_name(data),
        user_intro: user_persona_intro(data),
        enable_pdf_images: data.pdf_read_mode == "image" && selected_api.enable_image,
        is_runtime_conversation,
    }
}

fn build_prompt_prepare_conversation_before(
    conversation_before: &Conversation,
    is_remote_im_contact_conversation: bool,
    remote_im_contact_processing_mode: &str,
) -> Conversation {
    if is_remote_im_contact_conversation && remote_im_contact_processing_mode == "qa" {
        let trimmed = remote_im_trim_conversation_for_qa_mode(conversation_before);
        eprintln!(
            "[远程IM] 问答模式裁剪会话上下文: conversation_id={}, original_messages={}, trimmed_messages={}",
            conversation_before.id,
            conversation_before.messages.len(),
            trimmed.messages.len()
        );
        return trimmed;
    }
    conversation_before.clone()
}

fn prompt_prepare_last_archive_summary(
    data: &AppData,
    conversation_before: &Conversation,
    is_runtime_conversation: bool,
) -> Option<String> {
    if is_runtime_conversation || conversation_is_delegate(conversation_before) {
        return None;
    }
    data.conversations
        .iter()
        .rev()
        .find(|conversation| !conversation_is_delegate(conversation) && !conversation.summary.trim().is_empty())
        .map(|conversation| conversation.summary.clone())
}
