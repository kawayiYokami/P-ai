// ==================== 人格表情库管理 ====================
//
// 表情库按人格隔离存放在 `<助理空间>/.meme/<agent-id>/` 下，一名一图、全扁平。
// 本模块只负责人格页表情管理器的读写入口，表情解析仍走 builtin_meme 的既有逻辑。

/// 表情库里的一个表情项，一个名字对应一张图。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentMemeItemView {
    /// 表情名，即 `:名字:` 里的名字。
    name: String,
    /// 文件名，用于展示与定位。
    file_name: String,
    /// 相对助理空间的路径，删除与定位都按它走。
    relative_path: String,
    /// 供前端直接渲染的 data URL。
    data_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMemeListInput {
    agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMemeAddInput {
    agent_id: String,
    file_name: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMemeDeleteInput {
    agent_id: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMemeRenameInput {
    agent_id: String,
    name: String,
    new_name: String,
}

fn agent_meme_root_or_err(state: &AppState, agent_id: &str) -> Result<PathBuf, String> {
    meme_agent_root(state, agent_id).ok_or_else(|| "人格 id 为空，无法定位表情库".to_string())
}

fn meme_data_url(path: &Path, mime: &str) -> Result<String, String> {
    let raw = std::fs::read(path)
        .map_err(|err| format!("读取表情文件失败: path={}, err={err}", path.display()))?;
    Ok(format!("data:{mime};base64,{}", B64.encode(raw)))
}

/// 用文件名推导表情名：去掉扩展名，非法文件名字符替换为下划线，清洗后为空时退回 `meme`。
fn meme_name_from_file_name(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map(|(head, _)| head)
        .unwrap_or(file_name);
    let cleaned = stem
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_whitespace()
                || ch.is_control()
                || matches!(ch, ':' | '/' | '\\' | '<' | '>' | '"' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();
    let trimmed = cleaned.trim_matches('_').trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "meme".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 重名时按 `-2`、`-3` 顺序找一个可用名字，保证「一名一图」不被破坏。
fn next_available_meme_name(
    grouped: &std::collections::BTreeMap<String, MemeAssetCandidate>,
    base: &str,
) -> String {
    if !grouped.contains_key(base) {
        return base.to_string();
    }
    for index in 2..=u32::MAX {
        let candidate = format!("{base}-{index}");
        if !grouped.contains_key(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", Uuid::new_v4())
}

fn meme_item_view_from_candidate(candidate: &MemeAssetCandidate) -> Result<AgentMemeItemView, String> {
    Ok(AgentMemeItemView {
        name: candidate.name.clone(),
        file_name: candidate
            .absolute_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        relative_path: candidate.relative_path.clone(),
        data_url: meme_data_url(&candidate.absolute_path, &candidate.mime)?,
    })
}

fn list_agent_memes_inner(
    input: AgentMemeListInput,
    state: &AppState,
) -> Result<Vec<AgentMemeItemView>, String> {
    let grouped = meme_available_assets(state, &input.agent_id)?;
    let mut items = Vec::with_capacity(grouped.len());
    for candidate in grouped.values() {
        match meme_item_view_from_candidate(candidate) {
            Ok(item) => items.push(item),
            Err(err) => runtime_log_warn(format!(
                "[表情贴纸] 表情管理器读取素材失败，已跳过: relative_path={}, err={err}",
                candidate.relative_path
            )),
        }
    }
    Ok(items)
}

fn add_agent_meme_inner(
    input: AgentMemeAddInput,
    state: &AppState,
) -> Result<AgentMemeItemView, String> {
    let root = agent_meme_root_or_err(state, &input.agent_id)?;
    std::fs::create_dir_all(&root).map_err(|err| {
        format!(
            "创建表情目录失败: path={}, err={err}",
            root.display()
        )
    })?;

    let grouped = meme_available_assets(state, &input.agent_id)?;
    let emotion = next_available_meme_name(&grouped, &meme_name_from_file_name(&input.file_name));

    let raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("解码表情图片失败: {err}"))?;
    if raw.is_empty() {
        return Err("表情图片内容为空".to_string());
    }

    // 复用既有归一化流程：先落到系统临时目录，再按标准流程入库。
    let temp_path = std::env::temp_dir().join(format!(
        "pai-meme-upload-{}.{}",
        Uuid::new_v4(),
        input
            .file_name
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .filter(|ext| !ext.trim().is_empty())
            .unwrap_or("png")
    ));
    std::fs::write(&temp_path, &raw)
        .map_err(|err| format!("写入临时表情文件失败: path={}, err={err}", temp_path.display()))?;

    let prepared = meme_prepare_image_asset(&temp_path);
    let _ = std::fs::remove_file(&temp_path);
    let normalized = prepared?;

    let target_path =
        meme_target_path_for_new_asset(state, &input.agent_id, &emotion, &normalized.ext)?;
    std::fs::write(&target_path, &normalized.bytes).map_err(|err| {
        format!(
            "保存表情失败: path={}, err={err}",
            target_path.display()
        )
    })?;

    let relative_path = workspace_relative_path(state, &target_path);
    if let Ok(hash) = compute_meme_dhash_hex(&target_path) {
        let mut dhash_index = meme_load_dhash_index(state, &input.agent_id);
        dhash_index.insert(relative_path.clone(), hash);
        if let Err(err) = meme_persist_dhash_index(state, &input.agent_id, &dhash_index) {
            runtime_log_warn(format!(
                "[表情贴纸] 表情管理器入库后持久化 dHash 索引失败: {err}"
            ));
        }
    }

    runtime_log_info(format!(
        "[表情贴纸] 表情管理器添加完成: agent_id={}, name={}, path={}",
        input.agent_id,
        emotion,
        target_path.display()
    ));

    Ok(AgentMemeItemView {
        name: emotion,
        file_name: target_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        relative_path,
        data_url: meme_data_url(&target_path, &normalized.mime)?,
    })
}

fn delete_agent_meme_inner(input: AgentMemeDeleteInput, state: &AppState) -> Result<(), String> {
    meme_delete_asset(state, &input.agent_id, &input.name)
}

fn rename_agent_meme_inner(input: AgentMemeRenameInput, state: &AppState) -> Result<(), String> {
    meme_rename_asset(state, &input.agent_id, &input.name, &input.new_name)
}

#[tauri::command]
fn list_agent_memes(
    input: AgentMemeListInput,
    state: State<'_, AppState>,
) -> Result<Vec<AgentMemeItemView>, String> {
    list_agent_memes_inner(input, state.inner())
}

#[tauri::command]
fn add_agent_meme(
    input: AgentMemeAddInput,
    state: State<'_, AppState>,
) -> Result<AgentMemeItemView, String> {
    add_agent_meme_inner(input, state.inner())
}

#[tauri::command]
fn delete_agent_meme(
    input: AgentMemeDeleteInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_agent_meme_inner(input, state.inner())
}

#[tauri::command]
fn rename_agent_meme(
    input: AgentMemeRenameInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    rename_agent_meme_inner(input, state.inner())
}

#[cfg(test)]
mod meme_library_tests {
    use super::*;
    use crate::builtin_meme_tests::meme_test_state;

    fn png_base64(width: u32, height: u32, seed: u8) -> String {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([
                (x as u8).wrapping_mul(seed).wrapping_add(seed),
                (y as u8).wrapping_mul(seed).wrapping_add(7),
                seed,
            ])
        });
        let mut buffer = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut buffer, image::ImageFormat::Png)
            .expect("encode png");
        B64.encode(buffer.into_inner())
    }

    fn add(state: &AppState, agent_id: &str, name: &str, seed: u8) -> AgentMemeItemView {
        add_agent_meme_inner(
            AgentMemeAddInput {
                agent_id: agent_id.to_string(),
                file_name: format!("{name}.png"),
                bytes_base64: png_base64(40, 40, seed),
            },
            state,
        )
        .expect("add meme")
    }

    fn list(state: &AppState, agent_id: &str) -> Vec<AgentMemeItemView> {
        list_agent_memes_inner(
            AgentMemeListInput {
                agent_id: agent_id.to_string(),
            },
            state,
        )
        .expect("list memes")
    }

    #[test]
    fn add_then_list_should_expose_single_image_with_data_url() {
        let state = meme_test_state();
        let item = add(&state, "agent-a", "坏笑", 11);

        assert_eq!(item.name, "坏笑");
        assert!(item.file_name.ends_with(".webp"));
        assert!(item.data_url.starts_with("data:image/"));

        let items = list(&state, "agent-a");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "坏笑");
    }

    #[test]
    fn adding_same_file_name_twice_should_auto_suffix() {
        let state = meme_test_state();
        add(&state, "agent-a", "坏笑", 21);
        let second = add(&state, "agent-a", "坏笑", 22);

        assert_eq!(second.name, "坏笑-2");
        assert_eq!(list(&state, "agent-a").len(), 2);
    }

    #[test]
    fn file_name_should_be_sanitized_into_meme_name() {
        let state = meme_test_state();
        let item = add_agent_meme_inner(
            AgentMemeAddInput {
                agent_id: "agent-a".to_string(),
                file_name: "my sticker:1.png".to_string(),
                bytes_base64: png_base64(40, 40, 82),
            },
            &state,
        )
        .expect("add meme");

        assert_eq!(item.name, "my_sticker_1");
    }

    #[test]
    fn rename_should_move_single_image() {
        let state = meme_test_state();
        add(&state, "agent-a", "坏笑", 31);

        rename_agent_meme_inner(
            AgentMemeRenameInput {
                agent_id: "agent-a".to_string(),
                name: "坏笑".to_string(),
                new_name: "开心".to_string(),
            },
            &state,
        )
        .expect("rename meme");

        let items = list(&state, "agent-a");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "开心");
        assert!(items[0].file_name.starts_with("开心"));
    }

    #[test]
    fn rename_should_reject_existing_target_name() {
        let state = meme_test_state();
        add(&state, "agent-a", "坏笑", 41);
        add(&state, "agent-a", "无语", 42);

        let err = rename_agent_meme_inner(
            AgentMemeRenameInput {
                agent_id: "agent-a".to_string(),
                name: "坏笑".to_string(),
                new_name: "无语".to_string(),
            },
            &state,
        )
        .expect_err("rename to existing name should fail");
        assert!(err.contains("已存在"));
    }

    #[test]
    fn delete_should_remove_image() {
        let state = meme_test_state();
        add(&state, "agent-a", "坏笑", 51);

        delete_agent_meme_inner(
            AgentMemeDeleteInput {
                agent_id: "agent-a".to_string(),
                name: "坏笑".to_string(),
            },
            &state,
        )
        .expect("delete meme");

        assert!(list(&state, "agent-a").is_empty());
    }

    #[test]
    fn delete_unknown_name_should_fail() {
        let state = meme_test_state();
        let err = delete_agent_meme_inner(
            AgentMemeDeleteInput {
                agent_id: "agent-a".to_string(),
                name: "不存在".to_string(),
            },
            &state,
        )
        .expect_err("delete unknown meme should fail");
        assert!(err.contains("未找到"));
    }

    #[test]
    fn rename_and_delete_should_keep_dhash_index_in_sync() {
        let state = meme_test_state();
        add(&state, "agent-a", "坏笑", 91);
        let old_path = list(&state, "agent-a")[0].relative_path.clone();
        assert!(meme_load_dhash_index(&state, "agent-a").contains_key(&old_path));

        rename_agent_meme_inner(
            AgentMemeRenameInput {
                agent_id: "agent-a".to_string(),
                name: "坏笑".to_string(),
                new_name: "开心".to_string(),
            },
            &state,
        )
        .expect("rename meme");

        let index_after_rename = meme_load_dhash_index(&state, "agent-a");
        assert!(!index_after_rename.contains_key(&old_path));
        let renamed = list(&state, "agent-a");
        assert!(index_after_rename.contains_key(&renamed[0].relative_path));

        delete_agent_meme_inner(
            AgentMemeDeleteInput {
                agent_id: "agent-a".to_string(),
                name: "开心".to_string(),
            },
            &state,
        )
        .expect("delete meme");

        assert!(meme_load_dhash_index(&state, "agent-a").is_empty());
    }

    #[test]
    fn library_should_stay_isolated_between_agents() {
        let state = meme_test_state();
        add(&state, "agent-a", "坏笑", 71);
        add(&state, "agent-b", "无语", 72);

        let a = list(&state, "agent-a");
        let b = list(&state, "agent-b");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, "坏笑");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].name, "无语");
    }

    #[test]
    fn empty_agent_id_should_yield_empty_library() {
        let state = meme_test_state();
        assert!(list_agent_memes_inner(
            AgentMemeListInput {
                agent_id: "   ".to_string(),
            },
            &state,
        )
        .expect("empty agent id should be safe")
        .is_empty());

        let err = add_agent_meme_inner(
            AgentMemeAddInput {
                agent_id: "   ".to_string(),
                file_name: "坏笑.png".to_string(),
                bytes_base64: png_base64(40, 40, 81),
            },
            &state,
        )
        .expect_err("empty agent id should not be writable");
        assert!(err.contains("人格 id"));
    }
}
