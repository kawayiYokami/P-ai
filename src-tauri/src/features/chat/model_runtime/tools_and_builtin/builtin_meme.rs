#[derive(Debug, Clone, Deserialize, Serialize)]
struct MemeToolArgs {
    name: String,
    category: String,
    path: String,
}

#[derive(Debug, Clone)]
struct BuiltinMemeTool {
    app_state: AppState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PersistedMemeSegment {
    Text {
        text: String,
    },
    Meme {
        name: String,
        category: String,
        mime: String,
        #[serde(rename = "relativePath")]
        relative_path: String,
        #[serde(rename = "bytesBase64")]
        bytes_base64: String,
    },
}

#[derive(Debug, Clone)]
struct MemeAssetCandidate {
    name: String,
    mime: String,
    absolute_path: PathBuf,
    relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemeDHashDuplicateMatch {
    name: String,
    category: String,
    relative_path: String,
    distance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemeDetectedImageFormat {
    mime: String,
    ext: String,
    source: &'static str,
}

const MEME_DHASH_INDEX_FILE_NAME: &str = "image_dhash_index.json";
const MEME_DHASH_DISTANCE_THRESHOLD: u32 = 8;

fn meme_workspace_root(state: &AppState) -> PathBuf {
    configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone())
        .join(".meme")
}

fn meme_dhash_index_path(state: &AppState) -> PathBuf {
    meme_workspace_root(state).join(MEME_DHASH_INDEX_FILE_NAME)
}

fn meme_label_is_valid(raw: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('.')
        && !trimmed.contains(':')
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && !trimmed.chars().any(char::is_whitespace)
        && !trimmed
            .chars()
            .any(|ch| matches!(ch, '<' | '>' | '"' | '|' | '?' | '*'))
        && !trimmed.chars().any(char::is_control)
}

fn ensure_valid_meme_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if meme_label_is_valid(trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!(
            "素材名非法：`{}`。素材名不能为空，且不能包含空白、冒号、路径分隔符或 Windows 非法文件名字符。",
            raw
        ))
    }
}

fn ensure_valid_meme_category(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if meme_label_is_valid(trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!(
            "表情分类名非法：`{}`。分类名不能为空，且不能包含空白、冒号、路径分隔符或 Windows 非法文件名字符。",
            raw
        ))
    }
}

fn meme_asset_name_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("meme");
    stem
        .split_once("__")
        .map(|(head, _)| head.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(stem)
        .to_string()
}

fn meme_available_assets(
    state: &AppState,
) -> Result<std::collections::BTreeMap<String, Vec<MemeAssetCandidate>>, String> {
    let root = meme_workspace_root(state);
    if !root.exists() {
        return Ok(std::collections::BTreeMap::new());
    }
    let read_dir = std::fs::read_dir(&root)
        .map_err(|err| format!("读取表情目录失败: path={}, err={err}", root.display()))?;
    let mut grouped = std::collections::BTreeMap::<String, Vec<MemeAssetCandidate>>::new();
    for entry in read_dir {
        let entry = entry.map_err(|err| format!("读取表情目录项失败: {err}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let category = entry
            .file_name()
            .to_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("表情目录名非法: {}", path.display()))?
            .to_string();
        let child_dir = std::fs::read_dir(&path)
            .map_err(|err| format!("读取表情子目录失败: path={}, err={err}", path.display()))?;
        let mut variants = Vec::<MemeAssetCandidate>::new();
        for child in child_dir {
            let child = child.map_err(|err| format!("读取表情文件失败: {err}"))?;
            let child_path = child.path();
            if !child_path.is_file() {
                continue;
            }
            let detected = match meme_detect_image_format_from_path(&child_path) {
                Ok(value) => value,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[表情贴纸] 跳过无法识别真实格式的素材: path={}, err={err}",
                        child_path.display()
                    ));
                    continue;
                }
            };
            let relative_path = workspace_relative_path(state, &child_path);
            variants.push(MemeAssetCandidate {
                name: meme_asset_name_from_path(&child_path),
                mime: detected.mime,
                absolute_path: child_path,
                relative_path,
            });
        }
        if !variants.is_empty() {
            variants.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
            grouped.insert(category, variants);
        }
    }
    Ok(grouped)
}

fn meme_available_categories(state: &AppState) -> Result<Vec<String>, String> {
    Ok(meme_available_assets(state)?
        .into_keys()
        .collect::<Vec<_>>())
}

fn meme_categories_summary_text(state: &AppState) -> String {
    match meme_available_categories(state) {
        Ok(names) if !names.is_empty() => names.join("、"),
        Ok(_) => "（当前没有可用表情）".to_string(),
        Err(err) => format!("（读取失败：{}）", err),
    }
}

fn meme_magic_detect_image_format(raw: &[u8]) -> Option<MemeDetectedImageFormat> {
    if raw.len() >= 4 && raw[..4] == [0x89, b'P', b'N', b'G'] {
        return Some(MemeDetectedImageFormat {
            mime: "image/png".to_string(),
            ext: "png".to_string(),
            source: "magic",
        });
    }
    if raw.len() >= 3 && raw[0] == 0xFF && raw[1] == 0xD8 && raw[2] == 0xFF {
        return Some(MemeDetectedImageFormat {
            mime: "image/jpeg".to_string(),
            ext: "jpg".to_string(),
            source: "magic",
        });
    }
    if raw.len() >= 3 && &raw[..3] == b"GIF" {
        return Some(MemeDetectedImageFormat {
            mime: "image/gif".to_string(),
            ext: "gif".to_string(),
            source: "magic",
        });
    }
    if raw.len() >= 12 && &raw[..4] == b"RIFF" && &raw[8..12] == b"WEBP" {
        return Some(MemeDetectedImageFormat {
            mime: "image/webp".to_string(),
            ext: "webp".to_string(),
            source: "magic",
        });
    }
    None
}

fn meme_guess_detect_image_format(raw: &[u8]) -> Option<MemeDetectedImageFormat> {
    match image::guess_format(raw).ok()? {
        image::ImageFormat::Png => Some(MemeDetectedImageFormat {
            mime: "image/png".to_string(),
            ext: "png".to_string(),
            source: "guess_format",
        }),
        image::ImageFormat::Jpeg => Some(MemeDetectedImageFormat {
            mime: "image/jpeg".to_string(),
            ext: "jpg".to_string(),
            source: "guess_format",
        }),
        image::ImageFormat::Gif => Some(MemeDetectedImageFormat {
            mime: "image/gif".to_string(),
            ext: "gif".to_string(),
            source: "guess_format",
        }),
        image::ImageFormat::WebP => Some(MemeDetectedImageFormat {
            mime: "image/webp".to_string(),
            ext: "webp".to_string(),
            source: "guess_format",
        }),
        _ => None,
    }
}

fn meme_detect_image_format_from_bytes(raw: &[u8]) -> Option<MemeDetectedImageFormat> {
    meme_magic_detect_image_format(raw).or_else(|| meme_guess_detect_image_format(raw))
}

fn meme_detect_image_format_from_path(path: &Path) -> Result<MemeDetectedImageFormat, String> {
    let raw = std::fs::read(path)
        .map_err(|err| format!("读取表情源文件失败: path={}, err={err}", path.display()))?;
    meme_detect_image_format_from_bytes(&raw)
        .ok_or_else(|| format!("无法识别图片真实格式: {}", path.display()))
}

fn meme_decode_dynamic_image_from_path(path: &Path) -> Result<image::DynamicImage, String> {
    let raw = std::fs::read(path)
        .map_err(|err| format!("读取表情图文件失败: path={}, err={err}", path.display()))?;
    let Some(detected) = meme_detect_image_format_from_bytes(&raw) else {
        return Err(format!("无法识别表情图真实格式: {}", path.display()));
    };
    let format = match detected.ext.as_str() {
        "png" => image::ImageFormat::Png,
        "jpg" => image::ImageFormat::Jpeg,
        "gif" => image::ImageFormat::Gif,
        "webp" => image::ImageFormat::WebP,
        _ => {
            return Err(format!(
                "暂不支持用 `{}` 解码表情图: {}",
                detected.ext,
                path.display()
            ))
        }
    };
    image::load_from_memory_with_format(&raw, format).map_err(|err| {
        format!(
            "解码表情图失败: path={}, format={}, err={err}",
            path.display(),
            detected.ext
        )
    })
}

fn compute_meme_dhash_hex(image_path: &Path) -> Result<String, String> {
    let image = meme_decode_dynamic_image_from_path(image_path)?;
    let grayscale = image.to_luma8();
    let resized = image::imageops::resize(
        &grayscale,
        9,
        8,
        image::imageops::FilterType::Lanczos3,
    );
    let mut hash = 0u64;
    let mut bit_index = 0u32;
    for row in 0..8 {
        for col in 0..8 {
            let left = resized.get_pixel(col, row)[0];
            let right = resized.get_pixel(col + 1, row)[0];
            if left > right {
                hash |= 1u64 << bit_index;
            }
            bit_index += 1;
        }
    }
    Ok(format!("{hash:016x}"))
}

fn meme_hamming_distance(left: &str, right: &str) -> Option<u32> {
    let left = u64::from_str_radix(left, 16).ok()?;
    let right = u64::from_str_radix(right, 16).ok()?;
    Some((left ^ right).count_ones())
}

fn meme_load_dhash_index(state: &AppState) -> std::collections::BTreeMap<String, String> {
    let index_path = meme_dhash_index_path(state);
    if !index_path.exists() {
        return std::collections::BTreeMap::new();
    }
    match std::fs::read_to_string(&index_path) {
        Ok(raw) => match serde_json::from_str::<std::collections::BTreeMap<String, String>>(&raw) {
            Ok(index) => index,
            Err(err) => {
                runtime_log_warn(format!(
                    "[表情贴纸] 读取 dHash 索引失败，已按空索引处理: path={}, err={err}",
                    index_path.display()
                ));
                std::collections::BTreeMap::new()
            }
        },
        Err(err) => {
            runtime_log_warn(format!(
                "[表情贴纸] 读取 dHash 索引文件失败，已按空索引处理: path={}, err={err}",
                index_path.display()
            ));
            std::collections::BTreeMap::new()
        }
    }
}

fn meme_persist_dhash_index(
    state: &AppState,
    index: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let index_path = meme_dhash_index_path(state);
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "创建表情哈希索引目录失败: path={}, err={err}",
                parent.display()
            )
        })?;
    }
    std::fs::write(
        &index_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(index)
                .map_err(|err| format!("序列化表情哈希索引失败: {err}"))?
        ),
    )
    .map_err(|err| format!("写入表情哈希索引失败: path={}, err={err}", index_path.display()))
}

fn meme_sync_dhash_index(
    state: &AppState,
    grouped: &std::collections::BTreeMap<String, Vec<MemeAssetCandidate>>,
) -> std::collections::BTreeMap<String, String> {
    let mut index = meme_load_dhash_index(state);
    let mut changed = false;
    let expected = grouped
        .values()
        .flat_map(|items| items.iter())
        .map(|candidate| {
            (
                candidate.relative_path.clone(),
                candidate.absolute_path.clone(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let stale_keys = index
        .keys()
        .filter(|relative_path| !expected.contains_key(*relative_path))
        .cloned()
        .collect::<Vec<_>>();
    for stale in stale_keys {
        index.remove(&stale);
        changed = true;
    }

    for (relative_path, absolute_path) in &expected {
        if index.contains_key(relative_path) {
            continue;
        }
        match compute_meme_dhash_hex(absolute_path) {
            Ok(hash) => {
                index.insert(relative_path.clone(), hash);
                changed = true;
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[表情贴纸] 重建 dHash 索引时跳过损坏素材: relative_path={}, err={err}",
                    relative_path
                ));
            }
        }
    }

    if changed {
        if let Err(err) = meme_persist_dhash_index(state, &index) {
            runtime_log_warn(format!("[表情贴纸] 持久化 dHash 索引失败: {err}"));
        }
    }
    index
}

fn meme_find_duplicate_in_assets(
    source_hash: &str,
    grouped: &std::collections::BTreeMap<String, Vec<MemeAssetCandidate>>,
    dhash_index: &std::collections::BTreeMap<String, String>,
) -> Option<MemeDHashDuplicateMatch> {
    let mut best_match = None::<MemeDHashDuplicateMatch>;
    for (category, items) in grouped {
        for candidate in items {
            let Some(existing_hash) = dhash_index.get(&candidate.relative_path) else {
                continue;
            };
            let Some(distance) = meme_hamming_distance(source_hash, existing_hash) else {
                runtime_log_warn(format!(
                    "[表情贴纸] dHash 索引项非法，已跳过: relative_path={}",
                    candidate.relative_path
                ));
                continue;
            };
            if distance > MEME_DHASH_DISTANCE_THRESHOLD {
                continue;
            }
            let current = MemeDHashDuplicateMatch {
                name: candidate.name.clone(),
                category: category.clone(),
                relative_path: candidate.relative_path.clone(),
                distance,
            };
            let should_replace = best_match
                .as_ref()
                .map(|best| current.distance < best.distance)
                .unwrap_or(true);
            if should_replace {
                best_match = Some(current);
            }
        }
    }
    best_match
}

fn meme_prompt_rule_block(state: Option<&AppState>) -> Option<String> {
    let state = state?;
    let categories = meme_available_categories(state).ok()?;
    if categories.is_empty() {
        return None;
    }
    Some(prompt_xml_block(
        "meme sticker rule",
        format!(
            "当前可用表情分类：{}。\n如果你要发送贴纸，请直接在回答正文中写 `:分类名:`，例如 `:happy:`。\n不要编造不存在的分类名，也不要调用工具查询列表。\n当你需要把当前看到的图片收进贴纸库时，才调用 `meme` 工具。`category` 应优先使用常见用途分类，如 happy、sad、angry、awkward、speechless、cry、shy、surprised、like、dislike。",
            categories.join("、")
        ),
    ))
}

fn choose_meme_variant_index(seed_source: &str, meme_category: &str, token_index: usize, count: usize) -> usize {
    use std::hash::{Hash, Hasher};

    if count <= 1 {
        return 0;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed_source.hash(&mut hasher);
    meme_category.hash(&mut hasher);
    token_index.hash(&mut hasher);
    (hasher.finish() as usize) % count
}

fn meme_token_is_valid(category: &str) -> bool {
    let trimmed = category.trim();
    !trimmed.is_empty()
        && !trimmed.chars().any(char::is_whitespace)
        && !trimmed.contains(':')
        && !trimmed.chars().any(char::is_control)
}

fn resolve_text_to_persisted_meme_segments(
    state: &AppState,
    text: &str,
    seed_source: &str,
) -> Result<Option<Vec<PersistedMemeSegment>>, String> {
    let grouped = meme_available_assets(state)?;
    if grouped.is_empty() || text.trim().is_empty() {
        return Ok(None);
    }
    let mut segments = Vec::<PersistedMemeSegment>::new();
    let mut cursor = 0usize;
    let mut token_index = 0usize;
    let mut matched_any = false;

    while cursor < text.len() {
        let Some(start_rel) = text[cursor..].find(':') else {
            break;
        };
        let start = cursor + start_rel;
        let Some(end_rel) = text[start + 1..].find(':') else {
            break;
        };
        let end = start + 1 + end_rel;
        let category = &text[start + 1..end];
        if !meme_token_is_valid(category) {
            cursor = start + 1;
            continue;
        }
        let Some(candidates) = grouped.get(category) else {
            cursor = start + 1;
            continue;
        };
        if start > cursor {
            segments.push(PersistedMemeSegment::Text {
                text: text[cursor..start].to_string(),
            });
        }
        let chosen = &candidates[choose_meme_variant_index(
            seed_source,
            category,
            token_index,
            candidates.len(),
        )];
        let raw = std::fs::read(&chosen.absolute_path).map_err(|err| {
            format!(
                "读取表情文件失败: path={}, err={err}",
                chosen.absolute_path.display()
            )
        })?;
        segments.push(PersistedMemeSegment::Meme {
            name: chosen.name.clone(),
            category: category.to_string(),
            mime: chosen.mime.clone(),
            relative_path: chosen.relative_path.clone(),
            bytes_base64: B64.encode(raw),
        });
        matched_any = true;
        token_index += 1;
        cursor = end + 1;
    }

    if !matched_any {
        return Ok(None);
    }
    if cursor < text.len() {
        segments.push(PersistedMemeSegment::Text {
            text: text[cursor..].to_string(),
        });
    }
    Ok(Some(segments))
}

fn provider_meta_meme_segments(meta: Option<&Value>) -> Option<Vec<PersistedMemeSegment>> {
    let raw = meta?.as_object()?.get("memeSegments")?.clone();
    serde_json::from_value::<Vec<PersistedMemeSegment>>(raw).ok()
}

fn persist_meme_segments_into_provider_meta(
    provider_meta: &mut Option<Value>,
    segments: Option<&[PersistedMemeSegment]>,
) {
    let Some(segments) = segments else {
        return;
    };
    if !segments.iter().any(|segment| matches!(segment, PersistedMemeSegment::Meme { .. })) {
        return;
    }
    let mut meta = provider_meta
        .take()
        .unwrap_or_else(|| serde_json::json!({}));
    if !meta.is_object() {
        meta = serde_json::json!({});
    }
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("memeSegments".to_string(), serde_json::json!(segments));
    }
    *provider_meta = Some(meta);
}

fn meme_segments_to_remote_im_content_items(
    segments: &[PersistedMemeSegment],
) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for segment in segments {
        match segment {
            PersistedMemeSegment::Text { text } => {
                if !text.is_empty() {
                    out.push(serde_json::json!({
                        "type": "text",
                        "text": text,
                    }));
                }
            }
            PersistedMemeSegment::Meme {
                mime,
                relative_path,
                bytes_base64,
                ..
            } => {
                let file_name = std::path::Path::new(relative_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("meme")
                    .to_string();
                out.push(serde_json::json!({
                    "type": "image",
                    "mime": mime,
                    "name": file_name,
                    "bytesBase64": bytes_base64,
                }));
            }
        }
    }
    out
}

fn meme_segments_from_remote_im_text(
    state: &AppState,
    text: &str,
    seed_source: &str,
) -> Result<Option<Vec<PersistedMemeSegment>>, String> {
    resolve_text_to_persisted_meme_segments(state, text, seed_source)
}

fn meme_resolve_source_path(state: &AppState, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("path 不能为空".to_string());
    }
    let direct = PathBuf::from(trimmed);
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    let candidate = if direct.is_absolute() {
        direct
    } else {
        workspace_root.join(direct)
    };
    let metadata = std::fs::metadata(&candidate)
        .map_err(|_| format!("表情源文件不存在: {}", candidate.display()))?;
    if !metadata.is_file() {
        return Err(format!("表情源路径不是文件: {}", candidate.display()));
    }
    Ok(candidate)
}

impl RuntimeToolMetadata for BuiltinMemeTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        let categories = meme_categories_summary_text(&self.app_state);
        ProviderToolDefinition::new(
            "meme",
            format!(
                "把当前看到的图片偷进助理私人目录 `.meme` 贴纸库。当前可用表情分类：{}。不要用本工具查询列表；如果需要在回答中使用贴纸，直接输出 `:分类名:`。只有在需要新增贴纸库存时才调用本工具。",
                categories
            ),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "素材名，仅用于保存和管理这张图本身，不影响聊天里使用的 `:category:`。不能为空，且不能包含空格、冒号或路径分隔符。"
                    },
                    "category": {
                        "type": "string",
                        "description": "贴纸分类名，也就是聊天里使用的 `:category:`。请优先使用常见用途分类，如 happy、sad、angry、awkward、speechless、cry、shy、surprised、like、dislike。"
                    },
                    "path": {
                        "type": "string",
                        "description": "图片文件路径。可以是工作区相对路径，也可以是绝对路径。"
                    }
                },
                "required": ["name", "category", "path"]
            }),
        )
    }
}

impl RuntimeJsonTool for BuiltinMemeTool {
    const NAME: &'static str = "meme";
    type Args = MemeToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeJsonValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let name = ensure_valid_meme_name(&args.name).map_err(ToolInvokeError::from)?;
            let category = ensure_valid_meme_category(&args.category).map_err(ToolInvokeError::from)?;
            let source_path =
                meme_resolve_source_path(&self.app_state, &args.path).map_err(ToolInvokeError::from)?;
            let detected =
                meme_detect_image_format_from_path(&source_path).map_err(ToolInvokeError::from)?;
            let mime = detected.mime.clone();
            let source_hash = compute_meme_dhash_hex(&source_path).map_err(ToolInvokeError::from)?;
            let grouped = meme_available_assets(&self.app_state).map_err(ToolInvokeError::from)?;
            let mut dhash_index = meme_sync_dhash_index(&self.app_state, &grouped);
            if let Some(duplicate) =
                meme_find_duplicate_in_assets(&source_hash, &grouped, &dhash_index)
            {
                runtime_log_info(format!(
                    "[表情贴纸] 重复素材 跳过: source={}, matched={}, distance={}",
                    source_path.display(),
                    duplicate.relative_path,
                    duplicate.distance
                ));
                let variant_count = grouped
                    .get(&duplicate.category)
                    .map(|items| items.len())
                    .unwrap_or(1);
                return Ok(serde_json::json!({
                    "ok": true,
                    "action": "duplicate_skipped",
                    "duplicate": true,
                    "name": duplicate.name,
                    "category": duplicate.category,
                    "mime": mime,
                    "relativePath": duplicate.relative_path,
                    "variantCount": variant_count,
                    "distance": duplicate.distance,
                }));
            }
            let original_ext = source_path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase());
            if original_ext
                .as_deref()
                .map(|value| value != detected.ext.as_str())
                .unwrap_or(true)
            {
                runtime_log_info(format!(
                    "[表情贴纸] 自动修正图片真实格式 完成: source={}, original_ext={}, detected_ext={}, detector={}",
                    source_path.display(),
                    original_ext.as_deref().unwrap_or("（无扩展名）"),
                    detected.ext,
                    detected.source
                ));
            }
            let target_dir = meme_workspace_root(&self.app_state).join(&category);
            std::fs::create_dir_all(&target_dir).map_err(|err| {
                ToolInvokeError::from(format!(
                    "创建表情目录失败: path={}, err={err}",
                    target_dir.display()
                ))
            })?;
            let target_path = target_dir.join(format!(
                "{}__{}.{}",
                name,
                Uuid::new_v4().simple(),
                detected.ext
            ));
            std::fs::copy(&source_path, &target_path).map_err(|err| {
                ToolInvokeError::from(format!(
                    "保存表情失败: from={}, to={}, err={err}",
                    source_path.display(),
                    target_path.display()
                ))
            })?;
            let relative_path = workspace_relative_path(&self.app_state, &target_path);
            dhash_index.insert(relative_path.clone(), source_hash);
            if let Err(err) = meme_persist_dhash_index(&self.app_state, &dhash_index) {
                runtime_log_warn(format!(
                    "[表情贴纸] 新素材入库后持久化 dHash 索引失败: relative_path={}, err={err}",
                    relative_path
                ));
            }
            let variants = meme_available_assets(&self.app_state)
                .map_err(ToolInvokeError::from)?
                .get(&category)
                .map(|items| items.len())
                .unwrap_or(1);
            Ok(serde_json::json!({
                "ok": true,
                "action": "steal",
                "name": name,
                "category": category,
                "mime": mime,
                "relativePath": relative_path,
                "variantCount": variants,
            }))
        })
    }
}

#[cfg(test)]
mod builtin_meme_tests {
    use super::*;

    #[test]
    fn meme_name_validator_should_reject_invalid_names() {
        assert!(ensure_valid_meme_name("happy").is_ok());
        assert!(ensure_valid_meme_name("开心").is_ok());
        assert!(ensure_valid_meme_name("bad name").is_err());
        assert!(ensure_valid_meme_name("bad:name").is_err());
        assert!(ensure_valid_meme_name("../bad").is_err());
        assert!(ensure_valid_meme_category("happy").is_ok());
        assert!(ensure_valid_meme_category("bad name").is_err());
    }

    #[test]
    fn provider_meta_meme_segments_should_round_trip() {
        let segments = vec![
            PersistedMemeSegment::Text {
                text: "你好".to_string(),
            },
            PersistedMemeSegment::Meme {
                name: "贴纸A".to_string(),
                category: "happy".to_string(),
                mime: "image/png".to_string(),
                relative_path: ".meme/happy/贴纸A__1.png".to_string(),
                bytes_base64: "QUJD".to_string(),
            },
        ];
        let mut meta = None;
        persist_meme_segments_into_provider_meta(&mut meta, Some(&segments));
        let parsed = provider_meta_meme_segments(meta.as_ref()).expect("parse meme segments");
        assert_eq!(parsed, segments);
    }

    #[test]
    fn meme_detect_image_format_should_fix_gif_jpg_mismatch() {
        let gif_bytes = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";
        let root = std::env::temp_dir().join(format!("eca-meme-format-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        let path = root.join("wrong.jpg");
        std::fs::write(&path, gif_bytes).expect("write fake jpg gif bytes");

        let detected = meme_detect_image_format_from_path(&path).expect("detect actual format");
        assert_eq!(detected.mime, "image/gif");
        assert_eq!(detected.ext, "gif");

        let hash = compute_meme_dhash_hex(&path).expect("compute dhash from actual gif");
        assert!(!hash.is_empty());
    }

    fn meme_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-meme-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("app_data.json"),
            llm_workspace_path: root.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_runtime_state: Arc::new(Mutex::new(None)),
            cached_runtime_state_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_chat_index_mtime: Arc::new(Mutex::new(None)),
            cached_conversations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_chat_index_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            conversation_index_repair_gates: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    fn write_test_png(path: &Path) {
        let image = image::RgbImage::from_fn(32, 32, |x, y| {
            let r = ((x * 7 + y * 3) % 255) as u8;
            let g = ((x * 11 + y * 5) % 255) as u8;
            let b = ((x * 13 + y * 17) % 255) as u8;
            image::Rgb([r, g, b])
        });
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create image parent");
        }
        image.save(path).expect("save png");
    }

    #[test]
    fn meme_dhash_duplicate_lookup_should_match_identical_images() {
        let state = meme_test_state();
        let existing = meme_workspace_root(&state)
            .join("happy")
            .join("贴纸A__1.png");
        let source = state.llm_workspace_path.join("downloads").join("candidate.png");
        write_test_png(&existing);
        write_test_png(&source);

        let grouped = meme_available_assets(&state).expect("scan meme assets");
        let index = meme_sync_dhash_index(&state, &grouped);
        let source_hash = compute_meme_dhash_hex(&source).expect("compute source dhash");
        let duplicate =
            meme_find_duplicate_in_assets(&source_hash, &grouped, &index).expect("duplicate");

        assert_eq!(duplicate.category, "happy");
        assert_eq!(duplicate.name, "贴纸A");
        assert_eq!(duplicate.relative_path, ".meme/happy/贴纸A__1.png");
        assert_eq!(duplicate.distance, 0);
        assert!(meme_dhash_index_path(&state).exists());
    }
}
