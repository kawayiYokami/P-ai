#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RemoteImChannelPrivateState {
    #[serde(default = "remote_im_channel_private_state_schema_version")]
    schema_version: u32,
    #[serde(default)]
    channel_id: String,
    #[serde(default)]
    platform: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    base_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    account_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    user_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    sync_buf: String,
    #[serde(default, skip_serializing_if = "remote_im_string_map_is_empty")]
    context_tokens: std::collections::HashMap<String, String>,
    #[serde(default)]
    updated_at: String,
}

fn remote_im_channel_private_state_schema_version() -> u32 {
    1
}

fn remote_im_string_map_is_empty(
    value: &std::collections::HashMap<String, String>,
) -> bool {
    value.is_empty()
}

fn remote_im_platform_store_key(platform: &RemoteImPlatform) -> &'static str {
    match platform {
        RemoteImPlatform::Feishu => "feishu",
        RemoteImPlatform::Dingtalk => "dingtalk",
        RemoteImPlatform::OnebotV11 => "onebot_v11",
        RemoteImPlatform::WeixinOc => "weixin_oc",
        RemoteImPlatform::QqOfficial => "qq_official",
    }
}

fn remote_im_channel_state_file_stem(platform: &RemoteImPlatform, channel_id: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(remote_im_platform_store_key(platform).as_bytes());
    hasher.update(b"\n");
    hasher.update(channel_id.trim().as_bytes());
    bytes_to_lower_hex(hasher.finalize())
}

fn remote_im_channel_state_path(
    state: &AppState,
    platform: &RemoteImPlatform,
    channel_id: &str,
) -> PathBuf {
    app_root_from_data_path(&state.data_path)
        .join("remote-im")
        .join("channels")
        .join(remote_im_platform_store_key(platform))
        .join(format!(
            "{}.json",
            remote_im_channel_state_file_stem(platform, channel_id)
        ))
}

fn remote_im_channel_state_lock_key(platform: &RemoteImPlatform, channel_id: &str) -> String {
    format!(
        "{}:{}",
        remote_im_platform_store_key(platform),
        remote_im_channel_state_file_stem(platform, channel_id)
    )
}

fn remote_im_channel_state_write_lock(
    state: &AppState,
    platform: &RemoteImPlatform,
    channel_id: &str,
) -> Result<Arc<Mutex<()>>, String> {
    let key = remote_im_channel_state_lock_key(platform, channel_id);
    let mut locks = state
        .remote_im_channel_state_write_locks
        .lock()
        .map_err(|_| "锁定远程 IM 渠道状态写入锁失败".to_string())?;
    Ok(locks
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

fn remote_im_read_channel_private_state(
    state: &AppState,
    platform: &RemoteImPlatform,
    channel_id: &str,
) -> Result<RemoteImChannelPrivateState, String> {
    let path = remote_im_channel_state_path(state, platform, channel_id);
    if !path.exists() {
        return Ok(RemoteImChannelPrivateState {
            schema_version: remote_im_channel_private_state_schema_version(),
            channel_id: channel_id.trim().to_string(),
            platform: remote_im_platform_store_key(platform).to_string(),
            ..Default::default()
        });
    }
    let raw = fs::read_to_string(&path).map_err(|err| {
        format!(
            "读取远程 IM 渠道私有状态失败: platform={}, channel_id={}, path={}, err={}",
            remote_im_platform_store_key(platform),
            channel_id,
            path.display(),
            err
        )
    })?;
    let mut out = serde_json::from_str::<RemoteImChannelPrivateState>(&raw).map_err(|err| {
        format!(
            "解析远程 IM 渠道私有状态失败: platform={}, channel_id={}, path={}, err={}",
            remote_im_platform_store_key(platform),
            channel_id,
            path.display(),
            err
        )
    })?;
    out.schema_version = remote_im_channel_private_state_schema_version();
    out.channel_id = channel_id.trim().to_string();
    out.platform = remote_im_platform_store_key(platform).to_string();
    Ok(out)
}

fn remote_im_write_channel_private_state(
    state: &AppState,
    platform: &RemoteImPlatform,
    channel_id: &str,
    value: &RemoteImChannelPrivateState,
) -> Result<(), String> {
    let lock = remote_im_channel_state_write_lock(state, platform, channel_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| "锁定远程 IM 渠道状态文件失败".to_string())?;
    let path = remote_im_channel_state_path(state, platform, channel_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "创建远程 IM 渠道状态目录失败: platform={}, channel_id={}, path={}, err={}",
                remote_im_platform_store_key(platform),
                channel_id,
                parent.display(),
                err
            )
        })?;
    }
    let mut next = value.clone();
    next.schema_version = remote_im_channel_private_state_schema_version();
    next.channel_id = channel_id.trim().to_string();
    next.platform = remote_im_platform_store_key(platform).to_string();
    next.updated_at = now_iso();

    let body = serde_json::to_vec_pretty(&next).map_err(|err| {
        format!(
            "序列化远程 IM 渠道私有状态失败: platform={}, channel_id={}, err={}",
            remote_im_platform_store_key(platform),
            channel_id,
            err
        )
    })?;
    let tmp = path.with_extension(format!("json.tmp-{}", Uuid::new_v4()));
    {
        let mut file = fs::File::create(&tmp).map_err(|err| {
            format!(
                "创建远程 IM 渠道状态临时文件失败: platform={}, channel_id={}, path={}, err={}",
                remote_im_platform_store_key(platform),
                channel_id,
                tmp.display(),
                err
            )
        })?;
        std::io::Write::write_all(&mut file, &body).map_err(|err| {
            format!(
                "写入远程 IM 渠道状态临时文件失败: platform={}, channel_id={}, path={}, err={}",
                remote_im_platform_store_key(platform),
                channel_id,
                tmp.display(),
                err
            )
        })?;
        std::io::Write::flush(&mut file).map_err(|err| {
            format!(
                "刷新远程 IM 渠道状态临时文件失败: platform={}, channel_id={}, path={}, err={}",
                remote_im_platform_store_key(platform),
                channel_id,
                tmp.display(),
                err
            )
        })?;
    }
    fs::rename(&tmp, &path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        format!(
            "替换远程 IM 渠道状态文件失败: platform={}, channel_id={}, path={}, err={}",
            remote_im_platform_store_key(platform),
            channel_id,
            path.display(),
            err
        )
    })?;
    Ok(())
}

fn remote_im_patch_channel_private_state<F>(
    state: &AppState,
    platform: &RemoteImPlatform,
    channel_id: &str,
    patch: F,
) -> Result<RemoteImChannelPrivateState, String>
where
    F: FnOnce(&mut RemoteImChannelPrivateState),
{
    let mut current = remote_im_read_channel_private_state(state, platform, channel_id)?;
    patch(&mut current);
    remote_im_write_channel_private_state(state, platform, channel_id, &current)?;
    Ok(current)
}

fn remote_im_delete_channel_private_state(
    state: &AppState,
    platform: &RemoteImPlatform,
    channel_id: &str,
) -> Result<(), String> {
    let lock = remote_im_channel_state_write_lock(state, platform, channel_id)?;
    let _guard = lock
        .lock()
        .map_err(|_| "锁定远程 IM 渠道状态文件失败".to_string())?;
    let path = remote_im_channel_state_path(state, platform, channel_id);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|err| {
        format!(
            "删除远程 IM 渠道私有状态失败: platform={}, channel_id={}, path={}, err={}",
            remote_im_platform_store_key(platform),
            channel_id,
            path.display(),
            err
        )
    })
}

fn remote_im_take_string_field(obj: &mut serde_json::Map<String, Value>, key: &str) -> String {
    obj.remove(key)
        .and_then(|value| value.as_str().map(str::trim).map(ToString::to_string))
        .unwrap_or_default()
}

fn remote_im_merge_non_empty(target: &mut String, value: String) -> bool {
    let value = value.trim();
    if value.is_empty() || !target.trim().is_empty() {
        return false;
    }
    *target = value.to_string();
    true
}

fn remote_im_channel_private_state_from_legacy_credentials(
    platform: &RemoteImPlatform,
    credentials: &mut Value,
) -> Option<RemoteImChannelPrivateState> {
    let obj = credentials.as_object_mut()?;
    let mut out = RemoteImChannelPrivateState::default();
    let mut changed = false;

    match platform {
        RemoteImPlatform::WeixinOc => {
            changed |= remote_im_merge_non_empty(&mut out.token, remote_im_take_string_field(obj, "token"));
            changed |= remote_im_merge_non_empty(&mut out.account_id, remote_im_take_string_field(obj, "accountId"));
            changed |= remote_im_merge_non_empty(&mut out.user_id, remote_im_take_string_field(obj, "userId"));
            changed |= remote_im_merge_non_empty(&mut out.sync_buf, remote_im_take_string_field(obj, "syncBuf"));
        }
        RemoteImPlatform::Dingtalk
        | RemoteImPlatform::Feishu
        | RemoteImPlatform::OnebotV11
        | RemoteImPlatform::QqOfficial => {}
    }

    changed.then_some(out)
}

fn remote_im_merge_private_state(
    current: &mut RemoteImChannelPrivateState,
    legacy: RemoteImChannelPrivateState,
) {
    let _ = remote_im_merge_non_empty(&mut current.token, legacy.token);
    let _ = remote_im_merge_non_empty(&mut current.base_url, legacy.base_url);
    let _ = remote_im_merge_non_empty(&mut current.account_id, legacy.account_id);
    let _ = remote_im_merge_non_empty(&mut current.user_id, legacy.user_id);
    let _ = remote_im_merge_non_empty(&mut current.sync_buf, legacy.sync_buf);
    for (user_id, context_token) in legacy.context_tokens {
        let normalized_user_id = user_id.trim();
        let normalized_context_token = context_token.trim();
        if normalized_user_id.is_empty() || normalized_context_token.is_empty() {
            continue;
        }
        current
            .context_tokens
            .entry(normalized_user_id.to_string())
            .or_insert_with(|| normalized_context_token.to_string());
    }
}

fn remote_im_migrate_channel_private_states(
    state: &AppState,
    config: &mut AppConfig,
) -> Result<bool, String> {
    let mut changed = false;
    for channel in &mut config.remote_im_channels {
        let Some(legacy) =
            remote_im_channel_private_state_from_legacy_credentials(&channel.platform, &mut channel.credentials)
        else {
            continue;
        };
        let mut current =
            remote_im_read_channel_private_state(state, &channel.platform, &channel.id)?;
        remote_im_merge_private_state(&mut current, legacy);
        remote_im_write_channel_private_state(state, &channel.platform, &channel.id, &current)?;
        changed = true;
    }
    Ok(changed)
}

fn remote_im_effective_credentials(
    state: &AppState,
    channel: &RemoteImChannelConfig,
) -> Result<Value, String> {
    let mut out = channel.credentials.clone();
    if !out.is_object() {
        out = serde_json::json!({});
    }
    let private = remote_im_read_channel_private_state(state, &channel.platform, &channel.id)?;
    let obj = out
        .as_object_mut()
        .ok_or_else(|| "远程 IM credentials 不是对象".to_string())?;
    if !private.token.trim().is_empty() {
        obj.insert("token".to_string(), Value::String(private.token));
    }
    if !private.base_url.trim().is_empty() {
        obj.insert("baseUrl".to_string(), Value::String(private.base_url));
    }
    if !private.account_id.trim().is_empty() {
        obj.insert("accountId".to_string(), Value::String(private.account_id));
    }
    if !private.user_id.trim().is_empty() {
        obj.insert("userId".to_string(), Value::String(private.user_id));
    }
    if !private.sync_buf.trim().is_empty() {
        obj.insert("syncBuf".to_string(), Value::String(private.sync_buf));
    }
    Ok(out)
}

fn remote_im_channel_with_effective_credentials(
    state: &AppState,
    channel: &RemoteImChannelConfig,
) -> Result<RemoteImChannelConfig, String> {
    let mut out = channel.clone();
    out.credentials = remote_im_effective_credentials(state, channel)?;
    Ok(out)
}

#[cfg(test)]
mod remote_im_channel_store_tests {
    use super::*;

    #[test]
    fn weixin_legacy_migration_strips_private_fields_only() {
        let mut credentials = serde_json::json!({
            "baseUrl": "https://example.test",
            "token": "secret-token",
            "accountId": "bot-1",
            "userId": "user-1",
            "syncBuf": "cursor-1"
        });

        let private = remote_im_channel_private_state_from_legacy_credentials(
            &RemoteImPlatform::WeixinOc,
            &mut credentials,
        )
        .expect("private state");

        assert_eq!(private.token, "secret-token");
        assert_eq!(private.sync_buf, "cursor-1");
        assert_eq!(credentials.get("baseUrl").and_then(Value::as_str), Some("https://example.test"));
        assert!(credentials.get("token").is_none());
        assert!(credentials.get("syncBuf").is_none());
    }

    #[test]
    fn merge_private_state_keeps_existing_context_token_and_adds_new_one() {
        let mut current = RemoteImChannelPrivateState {
            context_tokens: std::collections::HashMap::from([(
                "user-1".to_string(),
                "token-a".to_string(),
            )]),
            ..Default::default()
        };
        let legacy = RemoteImChannelPrivateState {
            context_tokens: std::collections::HashMap::from([
                ("user-1".to_string(), "token-b".to_string()),
                ("user-2".to_string(), "token-c".to_string()),
            ]),
            ..Default::default()
        };

        remote_im_merge_private_state(&mut current, legacy);

        assert_eq!(
            current.context_tokens.get("user-1").map(String::as_str),
            Some("token-a")
        );
        assert_eq!(
            current.context_tokens.get("user-2").map(String::as_str),
            Some("token-c")
        );
    }

    #[test]
    fn channel_state_file_stem_does_not_contain_raw_channel_id() {
        let raw = "../evil/channel";
        let stem = remote_im_channel_state_file_stem(&RemoteImPlatform::WeixinOc, raw);

        assert_eq!(stem.len(), 64);
        assert!(stem.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(!stem.contains("evil"));
        assert!(!stem.contains('/'));
        assert!(!stem.contains('\\'));
    }
}
