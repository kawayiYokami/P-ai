#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MemeToolArgs {
    emotion: String,
    path: String,
}

#[derive(Debug, Clone)]
struct BuiltinMemeTool {
    app_state: AppState,
    /// 表情库归属人格：写入与读取都落在该人格的私有目录下。
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PersistedMemeSegment {
    Text {
        text: String,
    },
    Meme {
        name: String,
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
const MEME_NORMALIZED_MAX_EDGE: u32 = 300;
const MEME_NORMALIZED_WEBP_QUALITY: f32 = 85.0;
const MEME_NORMALIZED_MIME: &str = "image/webp";
const MEME_NORMALIZED_EXT: &str = "webp";
/// GIF 原样保留入库时的体积上限，超过则拒绝收编
const MEME_GIF_MAX_BYTES: u64 = 5 * 1024 * 1024;

struct MemeNormalizedImage {
    bytes: Vec<u8>,
    frame_count: usize,
    width: u32,
    height: u32,
    ext: String,
    mime: String,
}

fn meme_workspace_root(state: &AppState) -> PathBuf {
    configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone())
        .join(".meme")
}

/// 人格 id 作为表情库目录名时的清洗：只保留 ASCII 字母数字与 `-` `_`，其余替换为下划线。
fn meme_agent_dir_name(agent_id: &str) -> String {
    let trimmed = agent_id.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

/// 人格私有表情库根目录。人格 id 为空或清洗后为空时返回 None，调用方按空库处理。
fn meme_agent_root(state: &AppState, agent_id: &str) -> Option<PathBuf> {
    let dir_name = meme_agent_dir_name(agent_id);
    if dir_name.is_empty() {
        return None;
    }
    Some(meme_workspace_root(state).join(dir_name))
}

fn meme_dhash_index_path(state: &AppState, agent_id: &str) -> Option<PathBuf> {
    meme_agent_root(state, agent_id).map(|root| root.join(MEME_DHASH_INDEX_FILE_NAME))
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
            "表情名非法：`{}`。表情名不能为空，且不能包含空白、冒号、路径分隔符或 Windows 非法文件名字符。",
            raw
        ))
    }
}

fn meme_asset_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("meme")
        .to_string()
}

fn meme_is_dhash_index_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(MEME_DHASH_INDEX_FILE_NAME))
        .unwrap_or(false)
}

fn meme_available_assets(
    state: &AppState,
    agent_id: &str,
) -> Result<std::collections::BTreeMap<String, MemeAssetCandidate>, String> {
    let Some(root) = meme_agent_root(state, agent_id) else {
        return Ok(std::collections::BTreeMap::new());
    };
    if !root.exists() {
        return Ok(std::collections::BTreeMap::new());
    }
    let entries = std::fs::read_dir(&root)
        .map_err(|err| format!("读取表情目录失败: path={}, err={err}", root.display()))?;
    let mut candidates = Vec::<MemeAssetCandidate>::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("读取表情目录项失败: {err}"))?;
        let path = entry.path();
        if !path.is_file() || meme_is_dhash_index_file(&path) {
            continue;
        }
        let meme_name = meme_asset_name_from_path(&path);
        if !meme_label_is_valid(&meme_name) {
            continue;
        }
        let detected = match meme_detect_image_format_from_path(&path) {
            Ok(value) => value,
            Err(err) => {
                runtime_log_warn(format!(
                    "[表情贴纸] 跳过无法识别真实格式的素材: path={}, err={err}",
                    path.display()
                ));
                continue;
            }
        };
        let relative_path = workspace_relative_path(state, &path);
        candidates.push(MemeAssetCandidate {
            name: meme_name,
            mime: detected.mime,
            absolute_path: path,
            relative_path,
        });
    }
    // 一名一图：同名文件按相对路径字典序取第一个，其余按重复跳过。
    candidates.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut grouped = std::collections::BTreeMap::<String, MemeAssetCandidate>::new();
    for candidate in candidates {
        if let Some(existing) = grouped.get(&candidate.name) {
            runtime_log_warn(format!(
                "[表情贴纸] 同名表情文件重复，已跳过: kept={}, skipped={}",
                existing.relative_path, candidate.relative_path
            ));
            continue;
        }
        grouped.insert(candidate.name.clone(), candidate);
    }
    Ok(grouped)
}

fn meme_available_names(state: &AppState, agent_id: &str) -> Result<Vec<String>, String> {
    Ok(meme_available_assets(state, agent_id)?
        .into_keys()
        .collect::<Vec<_>>())
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

fn meme_resized_dimensions(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_edge || longest == 0 {
        return (width.max(1), height.max(1));
    }
    let new_width = ((width as u64 * max_edge as u64 + longest as u64 / 2) / longest as u64)
        .max(1) as u32;
    let new_height = ((height as u64 * max_edge as u64 + longest as u64 / 2) / longest as u64)
        .max(1) as u32;
    (new_width, new_height)
}

fn meme_resize_rgba_frame(image: image::RgbaImage) -> image::RgbaImage {
    let (width, height) = image.dimensions();
    let (new_width, new_height) =
        meme_resized_dimensions(width, height, MEME_NORMALIZED_MAX_EDGE);
    if (new_width, new_height) == (width, height) {
        image
    } else {
        image::imageops::resize(
            &image,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        )
    }
}

fn meme_encode_static_webp(image: image::RgbaImage) -> Result<MemeNormalizedImage, String> {
    let width = image.width();
    let height = image.height();
    let encoded = webp::Encoder::from_rgba(image.as_raw(), width, height)
        .encode_simple(false, MEME_NORMALIZED_WEBP_QUALITY)
        .map_err(|err| format!("编码 WebP 贴纸失败: {err:?}"))?;
    Ok(MemeNormalizedImage {
        bytes: encoded.to_vec(),
        frame_count: 1,
        width,
        height,
        ext: MEME_NORMALIZED_EXT.to_string(),
        mime: MEME_NORMALIZED_MIME.to_string(),
    })
}

fn meme_encode_animated_webp(
    frames: Vec<(image::RgbaImage, u32)>,
) -> Result<MemeNormalizedImage, String> {
    let Some((first, _)) = frames.first() else {
        return Err("动图没有可用帧".to_string());
    };
    let width = first.width();
    let height = first.height();
    let mut config = webp::WebPConfig::new()
        .map_err(|err| format!("初始化 WebP 动图编码配置失败: {err:?}"))?;
    config.lossless = 0;
    config.quality = MEME_NORMALIZED_WEBP_QUALITY;
    config.method = 6;
    config.alpha_compression = 1;
    let mut encoder = webp::AnimEncoder::new(width, height, &config);
    encoder.set_loop_count(0);
    let mut timestamp_ms = 0i32;
    for (frame, duration_ms) in &frames {
        if frame.width() != width || frame.height() != height {
            return Err("动图帧尺寸不一致，无法编码 WebP".to_string());
        }
        let frame = webp::AnimFrame::from_rgba(frame.as_raw(), width, height, timestamp_ms);
        encoder.add_frame(frame);
        let duration = (*duration_ms).max(20).min(i32::MAX as u32) as i32;
        timestamp_ms = timestamp_ms.saturating_add(duration);
    }
    let encoded = encoder
        .try_encode()
        .map_err(|err| format!("编码 WebP 动图贴纸失败: {err:?}"))?;
    Ok(MemeNormalizedImage {
        bytes: encoded.to_vec(),
        frame_count: frames.len(),
        width,
        height,
        ext: MEME_NORMALIZED_EXT.to_string(),
        mime: MEME_NORMALIZED_MIME.to_string(),
    })
}

fn meme_decode_animation_frames(
    raw: &[u8],
    detected: &MemeDetectedImageFormat,
) -> Result<Option<Vec<(image::RgbaImage, u32)>>, String> {
    use image::AnimationDecoder;

    let cursor = std::io::Cursor::new(raw);
    let frames = match detected.ext.as_str() {
        "gif" => {
            let decoder = image::codecs::gif::GifDecoder::new(cursor)
                .map_err(|err| format!("解码 GIF 贴纸失败: {err}"))?;
            decoder
                .into_frames()
                .collect_frames()
                .map_err(|err| format!("读取 GIF 贴纸帧失败: {err}"))?
        }
        "webp" => {
            let decoder = image::codecs::webp::WebPDecoder::new(cursor)
                .map_err(|err| format!("解码 WebP 贴纸失败: {err}"))?;
            decoder
                .into_frames()
                .collect_frames()
                .map_err(|err| format!("读取 WebP 贴纸帧失败: {err}"))?
        }
        _ => return Ok(None),
    };
    if frames.len() <= 1 {
        return Ok(None);
    }
    let mut out = Vec::<(image::RgbaImage, u32)>::new();
    for frame in frames {
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let duration = if denominator == 0 {
            40
        } else {
            ((numerator as u64 + denominator as u64 / 2) / denominator as u64)
                .max(20)
                .min(i32::MAX as u64) as u32
        };
        out.push((meme_resize_rgba_frame(frame.into_buffer()), duration));
    }
    Ok(Some(out))
}

/// 解析 GIF 的尺寸与帧数，不修改原始字节（用于原样保留入库时的元数据）
fn meme_inspect_gif_frames(raw: &[u8]) -> Result<(u32, u32, usize), String> {
    use image::AnimationDecoder;
    let cursor = std::io::Cursor::new(raw);
    let decoder = image::codecs::gif::GifDecoder::new(cursor)
        .map_err(|err| format!("解码 GIF 贴纸失败: {err}"))?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|err| format!("读取 GIF 贴纸帧失败: {err}"))?;
    let first = frames
        .first()
        .ok_or_else(|| "GIF 贴纸没有可用帧".to_string())?;
    let buffer = first.buffer();
    Ok((buffer.width(), buffer.height(), frames.len()))
}

/// 判断 GIF 源文件是否超过体积上限
fn meme_check_gif_size_limit(detected_ext: &str, source_path: &Path) -> Result<(), String> {
    if detected_ext != "gif" {
        return Ok(());
    }
    let size = std::fs::metadata(source_path)
        .map_err(|err| format!("读取表情源文件大小失败: path={}, err={err}", source_path.display()))?
        .len();
    if size > MEME_GIF_MAX_BYTES {
        return Err("图片过大，无法作为表情".to_string());
    }
    Ok(())
}

/// 准备入库素材：GIF 原样保留（动画与帧时序不丢失），其余格式归一化为 WebP
fn meme_prepare_image_asset(path: &Path) -> Result<MemeNormalizedImage, String> {
    let raw = std::fs::read(path)
        .map_err(|err| format!("读取表情源文件失败: path={}, err={err}", path.display()))?;
    let detected = meme_detect_image_format_from_bytes(&raw)
        .ok_or_else(|| format!("无法识别图片真实格式: {}", path.display()))?;
    if detected.ext == "gif" {
        let (width, height, frame_count) = meme_inspect_gif_frames(&raw)?;
        return Ok(MemeNormalizedImage {
            bytes: raw,
            frame_count,
            width,
            height,
            ext: "gif".to_string(),
            mime: "image/gif".to_string(),
        });
    }
    if let Some(frames) = meme_decode_animation_frames(&raw, &detected)? {
        return meme_encode_animated_webp(frames);
    }
    let image = image::load_from_memory_with_format(
        &raw,
        match detected.ext.as_str() {
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
        },
    )
    .map_err(|err| {
        format!(
            "解码表情图失败: path={}, format={}, err={err}",
            path.display(),
            detected.ext
        )
    })?;
    meme_encode_static_webp(meme_resize_rgba_frame(image.to_rgba8()))
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

fn meme_load_dhash_index(
    state: &AppState,
    agent_id: &str,
) -> std::collections::BTreeMap<String, String> {
    let Some(index_path) = meme_dhash_index_path(state, agent_id) else {
        return std::collections::BTreeMap::new();
    };
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
    agent_id: &str,
    index: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let Some(index_path) = meme_dhash_index_path(state, agent_id) else {
        return Ok(());
    };
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
    agent_id: &str,
    grouped: &std::collections::BTreeMap<String, MemeAssetCandidate>,
) -> std::collections::BTreeMap<String, String> {
    let mut index = meme_load_dhash_index(state, agent_id);
    let mut changed = false;
    let expected = grouped
        .values()
        .map(|candidate| {
            (
                candidate.relative_path.clone(),
                candidate.absolute_path.clone(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

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
        if let Err(err) = meme_persist_dhash_index(state, agent_id, &index) {
            runtime_log_warn(format!("[表情贴纸] 持久化 dHash 索引失败: {err}"));
        }
    }
    index
}

fn meme_find_duplicate_in_assets(
    source_hash: &str,
    grouped: &std::collections::BTreeMap<String, MemeAssetCandidate>,
    dhash_index: &std::collections::BTreeMap<String, String>,
) -> Option<MemeDHashDuplicateMatch> {
    let mut best_match = None::<MemeDHashDuplicateMatch>;
    let names_by_path = grouped
        .iter()
        .map(|(name, candidate)| (candidate.relative_path.as_str(), name.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (relative_path, existing_hash) in dhash_index {
        let Some(distance) = meme_hamming_distance(source_hash, existing_hash) else {
            runtime_log_warn(format!(
                "[表情贴纸] dHash 索引项非法，已跳过: relative_path={}",
                relative_path
            ));
            continue;
        };
        if distance > MEME_DHASH_DISTANCE_THRESHOLD {
            continue;
        }
        let name = match names_by_path.get(relative_path.as_str()) {
            Some(name) => (*name).to_string(),
            None => meme_asset_name_from_path(Path::new(relative_path)),
        };
        let current = MemeDHashDuplicateMatch {
            name,
            relative_path: relative_path.clone(),
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
    best_match
}

fn meme_prompt_rule_block(state: Option<&AppState>, agent_id: &str) -> Option<String> {
    let state = state?;
    let names = meme_available_names(state, agent_id).ok()?;
    if names.is_empty() {
        return None;
    }
    Some(prompt_xml_block(
        "meme sticker rule",
        format!(
            "## 可用 meme\n\
             - {}\n\n\
             ## 发送贴纸\n\
             - 直接在回答正文中写 `:meme名:`。\n\
             - 示例：`:坏笑:`\n\
             - 只能使用上方已经存在的 meme 名。\n\n\
             ## 不要这样做\n\
             - 不要编造不存在的 meme 名。\n\
             - 不要调用工具查询 meme 列表。\n\n\
             ## 何时调用工具\n\
             - 只有需要把当前看到的图片收入贴纸库时，才调用 `meme` 工具。\n\
             - `emotion` 是贴纸名，也是后续可使用的 `:meme名:`。",
            names.join("、")
        ),
    ))
}

fn meme_token_is_valid(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && !trimmed.chars().any(char::is_whitespace)
        && !trimmed.contains(':')
        && !trimmed.chars().any(char::is_control)
}

fn resolve_text_to_persisted_meme_segments(
    state: &AppState,
    agent_id: &str,
    text: &str,
) -> Result<Option<Vec<PersistedMemeSegment>>, String> {
    let grouped = meme_available_assets(state, agent_id)?;
    if grouped.is_empty() || text.trim().is_empty() {
        return Ok(None);
    }
    let mut segments = Vec::<PersistedMemeSegment>::new();
    let mut scan_cursor = 0usize;
    let mut text_cursor = 0usize;
    let mut matched_any = false;

    while scan_cursor < text.len() {
        let Some(start_rel) = text[scan_cursor..].find(':') else {
            break;
        };
        let start = scan_cursor + start_rel;
        let Some(end_rel) = text[start + 1..].find(':') else {
            break;
        };
        let end = start + 1 + end_rel;
        let name = &text[start + 1..end];
        if !meme_token_is_valid(name) {
            scan_cursor = start + 1;
            continue;
        }
        let Some(chosen) = grouped.get(name) else {
            scan_cursor = start + 1;
            continue;
        };
        if start > text_cursor {
            segments.push(PersistedMemeSegment::Text {
                text: text[text_cursor..start].to_string(),
            });
        }
        let raw = std::fs::read(&chosen.absolute_path).map_err(|err| {
            format!(
                "读取表情文件失败: path={}, err={err}",
                chosen.absolute_path.display()
            )
        })?;
        segments.push(PersistedMemeSegment::Meme {
            name: chosen.name.clone(),
            mime: chosen.mime.clone(),
            relative_path: chosen.relative_path.clone(),
            bytes_base64: B64.encode(raw),
        });
        matched_any = true;
        scan_cursor = end + 1;
        text_cursor = end + 1;
    }

    if !matched_any {
        return Ok(None);
    }
    if text_cursor < text.len() {
        segments.push(PersistedMemeSegment::Text {
            text: text[text_cursor..].to_string(),
        });
    }
    Ok(Some(segments))
}

fn build_meme_annotations(
    state: &AppState,
    agent_id: &str,
    text: &str,
) -> Result<Vec<MemeAnnotation>, String> {
    let grouped = meme_available_assets(state, agent_id)?;
    if grouped.is_empty() || text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut annotations = Vec::<MemeAnnotation>::new();
    let mut scan_cursor = 0usize;

    while scan_cursor < text.len() {
        let Some(start_rel) = text[scan_cursor..].find(':') else {
            break;
        };
        let start = scan_cursor + start_rel;
        let Some(end_rel) = text[start + 1..].find(':') else {
            break;
        };
        let end = start + 1 + end_rel;
        let name = &text[start + 1..end];
        if !meme_token_is_valid(name) {
            scan_cursor = start + 1;
            continue;
        }
        let Some(chosen) = grouped.get(name) else {
            scan_cursor = start + 1;
            continue;
        };
        annotations.push(MemeAnnotation {
            meme: format!(":{}:", name),
            path: chosen.absolute_path.to_string_lossy().to_string(),
        });
        scan_cursor = end + 1;
    }

    Ok(annotations)
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

fn meme_target_path_for_new_asset(
    state: &AppState,
    agent_id: &str,
    emotion: &str,
    ext: &str,
) -> Result<PathBuf, String> {
    let root = meme_agent_root(state, agent_id)
        .ok_or_else(|| "表情库归属人格为空，无法写入表情".to_string())?;
    std::fs::create_dir_all(&root).map_err(|err| {
        format!(
            "创建表情目录失败: path={}, err={err}",
            root.display()
        )
    })?;
    Ok(root.join(format!("{emotion}.{ext}")))
}

impl RuntimeToolMetadata for BuiltinMemeTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            "meme",
            "把当前看到的图片收入助理空间（Assistant Space）下的 `.meme` 贴纸库。不要用本工具查询列表；如果需要在回答中使用贴纸，直接输出 `:meme名:`，例如 `:坏笑:`。只有在需要新增贴纸库存时才调用本工具。",
            serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "emotion": {
                        "type": "string",
                        "description": "贴纸名，也是聊天里使用的 `:meme名:`。例如 emotion 为 `坏笑`，聊天时使用 `:坏笑:`。不能为空，且不能包含空格、冒号或路径分隔符。"
                    },
                    "path": {
                        "type": "string",
                        "description": "图片文件路径。可以是工作区相对路径，也可以是绝对路径。"
                    }
                },
                "required": ["emotion", "path"]
            }),
        )
    }
}

impl RuntimeValueTool for BuiltinMemeTool {
    const NAME: &'static str = "meme";
    type Args = MemeToolArgs;
    type Error = ToolInvokeError;

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let emotion = ensure_valid_meme_name(&args.emotion).map_err(ToolInvokeError::from)?;
            let source_path =
                meme_resolve_source_path(&self.app_state, &args.path).map_err(ToolInvokeError::from)?;
            let detected =
                meme_detect_image_format_from_path(&source_path).map_err(ToolInvokeError::from)?;
            let source_hash = compute_meme_dhash_hex(&source_path).map_err(ToolInvokeError::from)?;
            let grouped = meme_available_assets(&self.app_state, &self.agent_id).map_err(ToolInvokeError::from)?;
            let mut dhash_index = meme_sync_dhash_index(&self.app_state, &self.agent_id, &grouped);
            if let Some(duplicate) =
                meme_find_duplicate_in_assets(&source_hash, &grouped, &dhash_index)
            {
                runtime_log_info(format!(
                    "[表情贴纸] 重复素材 跳过: source={}, matched={}, distance={}",
                    source_path.display(),
                    duplicate.relative_path,
                    duplicate.distance
                ));
                let duplicate_mime = grouped
                    .get(&duplicate.name)
                    .map(|item| item.mime.clone())
                    .unwrap_or_else(|| MEME_NORMALIZED_MIME.to_string());
                return Ok(serde_json::json!({
                    "ok": true,
                    "action": "duplicate_skipped",
                    "duplicate": true,
                    "name": duplicate.name,
                    "mime": duplicate_mime,
                    "relativePath": duplicate.relative_path,
                    "distance": duplicate.distance,
                }));
            }
            // 一名一图：同名已存在时显式拒绝，避免静默覆盖用户已有表情。
            if let Some(existing) = grouped.get(&emotion) {
                runtime_log_info(format!(
                    "[表情贴纸] 同名表情已存在，拒绝覆盖: name={}, existing={}",
                    emotion, existing.relative_path
                ));
                return Ok(serde_json::json!({
                    "ok": false,
                    "action": "name_exists",
                    "name": emotion,
                    "relativePath": existing.relative_path,
                    "message": format!("表情名 `:{emotion}:` 已存在，请换一个名字，或先让用户删掉旧表情。"),
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
            meme_check_gif_size_limit(&detected.ext, &source_path).map_err(ToolInvokeError::from)?;
            // 读文件 + 解码 + 编码是同步 IO 与 CPU 密集操作，移到 blocking 线程池，
            // 避免占住 async 工作线程阻塞其他并发任务（流式回复/工具调用）。
            let prepare_source = source_path.clone();
            let normalized = tokio::task::spawn_blocking(move || {
                meme_prepare_image_asset(&prepare_source)
            })
            .await
            .map_err(|err| {
                ToolInvokeError::from(format!("表情素材准备任务失败: err={err}"))
            })?
            .map_err(ToolInvokeError::from)?;
            let target_path = meme_target_path_for_new_asset(
                &self.app_state,
                &self.agent_id,
                &emotion,
                &normalized.ext,
            )
            .map_err(ToolInvokeError::from)?;
            let output_size = normalized.bytes.len();
            let write_target = target_path.clone();
            let write_bytes = normalized.bytes;
            tokio::task::spawn_blocking(move || {
                std::fs::write(&write_target, &write_bytes)
            })
            .await
            .map_err(|err| {
                ToolInvokeError::from(format!("表情保存任务失败: err={err}"))
            })?
            .map_err(|err| {
                ToolInvokeError::from(format!(
                    "保存表情失败: from={}, to={}, err={err}",
                    source_path.display(),
                    target_path.display()
                ))
            })?;
            let preserved = normalized.ext == "gif";
            runtime_log_info(format!(
                "[表情贴纸] {} 完成: source={}, target={}, original_mime={}, target_mime={}, width={}, height={}, frame_count={}, output_size={}",
                if preserved { "原样保留入库" } else { "归一化入库" },
                source_path.display(),
                target_path.display(),
                detected.mime,
                normalized.mime,
                normalized.width,
                normalized.height,
                normalized.frame_count,
                output_size
            ));
            let relative_path = workspace_relative_path(&self.app_state, &target_path);
            dhash_index.insert(relative_path.clone(), source_hash);
            if let Err(err) = meme_persist_dhash_index(&self.app_state, &self.agent_id, &dhash_index) {
                runtime_log_warn(format!(
                    "[表情贴纸] 新素材入库后持久化 dHash 索引失败: relative_path={}, err={err}",
                    relative_path
                ));
            }
            Ok(serde_json::json!({
                "ok": true,
                "action": "steal",
                "name": emotion,
                "emotion": emotion,
                "mime": normalized.mime,
                "relativePath": relative_path,
                "width": normalized.width,
                "height": normalized.height,
                "frameCount": normalized.frame_count,
            }))
        })
    }
}

#[cfg(test)]
pub(crate) mod builtin_meme_tests {
    use super::*;

    #[test]
    fn meme_available_assets_should_isolate_between_agents() {
        let state = meme_test_state();
        let root_a = meme_agent_root(&state, "agent-a").expect("root a");
        let root_b = meme_agent_root(&state, "agent-b").expect("root b");
        write_test_png(&root_a.join("坏笑.webp"));
        write_test_png(&root_b.join("无语.webp"));

        let a = meme_available_assets(&state, "agent-a").expect("scan a");
        let b = meme_available_assets(&state, "agent-b").expect("scan b");

        assert!(a.contains_key("坏笑"));
        assert!(!a.contains_key("无语"));
        assert!(b.contains_key("无语"));
        assert!(!b.contains_key("坏笑"));
    }

    #[test]
    fn meme_agent_dir_name_should_sanitize_and_reject_empty() {
        assert_eq!(meme_agent_dir_name("agent-a"), "agent-a");
        assert_eq!(meme_agent_dir_name("a/b:c d"), "a_b_c_d");
        assert_eq!(meme_agent_dir_name("   "), "");
        assert!(meme_agent_root(&meme_test_state(), "  ").is_none());
    }

    #[test]
    fn meme_available_assets_should_be_empty_for_unknown_agent() {
        let state = meme_test_state();
        write_test_png(&meme_agent_root(&state, "agent-a").expect("root a").join("坏笑.webp"));

        let empty = meme_available_assets(&state, "agent-b").expect("scan b");
        assert!(empty.is_empty());
    }

    #[test]
    fn meme_prompt_rule_block_should_be_none_for_empty_library() {
        let state = meme_test_state();
        write_test_png(&meme_agent_root(&state, "agent-a").expect("root a").join("坏笑.webp"));

        assert!(meme_prompt_rule_block(Some(&state), "agent-b").is_none());
        assert!(meme_prompt_rule_block(Some(&state), "agent-a").is_some());
    }

    #[test]
    fn meme_name_validator_should_reject_invalid_names() {
        assert!(ensure_valid_meme_name("happy").is_ok());
        assert!(ensure_valid_meme_name("开心").is_ok());
        assert!(ensure_valid_meme_name("bad name").is_err());
        assert!(ensure_valid_meme_name("bad:name").is_err());
        assert!(ensure_valid_meme_name("../bad").is_err());
    }

    #[test]
    fn meme_tool_args_should_only_accept_emotion_and_path() {
        let parsed = serde_json::from_value::<MemeToolArgs>(serde_json::json!({
            "emotion": "坏笑",
            "path": "source.png",
        }))
        .expect("parse new args");
        assert_eq!(parsed.emotion, "坏笑");
        assert_eq!(parsed.path, "source.png");

        assert!(serde_json::from_value::<MemeToolArgs>(serde_json::json!({
            "name": "坏笑",
            "category": "坏笑",
            "path": "source.png",
        }))
        .is_err());
    }

    #[test]
    fn resolve_text_to_persisted_meme_segments_should_preserve_text_before_invalid_colons() {
        let state = meme_test_state();
        let existing = test_meme_root(&state).join("坏笑粉毛.webp");
        write_test_png(&existing);
        let text = "| 指标 | 数值 |\n|:---|:---|\n| MA5 | 1373 |\n收尾:坏笑粉毛:";

        let segments = resolve_text_to_persisted_meme_segments(&state, TEST_AGENT_ID, text)
            .expect("resolve meme segments")
            .expect("segments");

        assert_eq!(
            segments.first(),
            Some(&PersistedMemeSegment::Text {
                text: "| 指标 | 数值 |\n|:---|:---|\n| MA5 | 1373 |\n收尾".to_string(),
            }),
        );
        assert!(matches!(
            segments.get(1),
            Some(PersistedMemeSegment::Meme { name, .. }) if name == "坏笑粉毛"
        ));
    }

    #[test]
    fn build_meme_annotations_should_map_token_to_asset_path() {
        let state = meme_test_state();
        let existing = test_meme_root(&state).join("坏笑.webp");
        write_test_png(&existing);

        let annotations = build_meme_annotations(&state, TEST_AGENT_ID, "你好 :坏笑: 收到")
            .expect("build meme annotations");

        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].meme, ":坏笑:");
        assert!(annotations[0].path.ends_with("坏笑.webp"));
    }

    #[test]
    fn meme_available_assets_should_scan_flat_root_files() {
        let state = meme_test_state();
        write_test_png(&test_meme_root(&state).join("坏笑.webp"));
        write_test_png(&test_meme_root(&state).join("无语.webp"));
        // 分类子目录已废弃，不再参与扫描
        write_test_png(&test_meme_root(&state).join("坏笑").join("坏笑(2).webp"));

        let grouped = meme_available_assets(&state, TEST_AGENT_ID).expect("scan meme assets");

        assert_eq!(grouped.len(), 2);
        assert_eq!(
            grouped.get("坏笑").map(|item| item.relative_path.as_str()),
            Some(".meme/agent-meme-test/坏笑.webp"),
        );
        assert_eq!(
            grouped.get("无语").map(|item| item.relative_path.as_str()),
            Some(".meme/agent-meme-test/无语.webp"),
        );
    }

    #[test]
    fn meme_available_assets_should_keep_first_on_duplicate_name() {
        let state = meme_test_state();
        let root = test_meme_root(&state);
        write_test_png(&root.join("坏笑.webp"));
        write_test_png(&root.join("坏笑.gif"));

        let grouped = meme_available_assets(&state, TEST_AGENT_ID).expect("scan meme assets");

        assert_eq!(
            grouped.get("坏笑").map(|item| item.relative_path.as_str()),
            Some(".meme/agent-meme-test/坏笑.gif"),
        );
    }

    #[test]
    fn persisted_meme_segment_should_tolerate_legacy_category_field() {
        let raw = serde_json::json!({
            "type": "meme",
            "name": "坏笑",
            "category": "坏笑",
            "mime": "image/webp",
            "relativePath": ".meme/agent-meme-test/坏笑.webp",
            "bytesBase64": "aGVsbG8=",
        });

        let segment = serde_json::from_value::<PersistedMemeSegment>(raw)
            .expect("deserialize legacy meme segment");

        assert!(matches!(segment, PersistedMemeSegment::Meme { name, .. } if name == "坏笑"));
    }

    #[test]
    fn meme_target_path_should_stay_flat_under_agent_root() {
        let state = meme_test_state();
        let target = meme_target_path_for_new_asset(&state, TEST_AGENT_ID, "坏笑", MEME_NORMALIZED_EXT)
            .expect("target path");

        assert_eq!(target, test_meme_root(&state).join("坏笑.webp"));
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

    pub(crate) fn meme_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-meme-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("config_mark"),
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
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
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
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            conversation_project_skills_cache: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn write_test_png(path: &Path) {
        write_test_png_with_size(path, 32, 32);
    }

    const TEST_AGENT_ID: &str = "agent-meme-test";

    fn test_meme_root(state: &AppState) -> PathBuf {
        meme_agent_root(state, TEST_AGENT_ID).expect("test meme root")
    }

    fn write_test_png_with_size(path: &Path, width: u32, height: u32) {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
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

    fn write_test_gif(path: &Path, frames: u8) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create gif parent");
        }
        let file = std::fs::File::create(path).expect("create gif file");
        let mut encoder = image::codecs::gif::GifEncoder::new(file);
        for i in 0..frames {
            let frame =
                image::RgbImage::from_pixel(48, 48, image::Rgb([i * 40, 0, 255 - i * 40]));
            encoder
                .encode(&frame, 48, 48, image::ExtendedColorType::Rgb8)
                .expect("encode gif frame");
        }
    }

    #[test]
    fn meme_prepare_image_asset_should_resize_static_image() {
        let state = meme_test_state();
        let source = state.llm_workspace_path.join("downloads").join("large.png");
        write_test_png_with_size(&source, 640, 320);

        let normalized = meme_prepare_image_asset(&source).expect("normalize image");
        let features = webp::BitstreamFeatures::new(&normalized.bytes).expect("webp features");

        assert_eq!(normalized.frame_count, 1);
        assert_eq!(normalized.ext, "webp");
        assert_eq!(normalized.mime, "image/webp");
        assert_eq!(features.width().max(features.height()), MEME_NORMALIZED_MAX_EDGE);
        assert!(!features.has_animation());
    }

    #[test]
    fn meme_prepare_image_asset_should_preserve_gif_as_is() {
        let state = meme_test_state();
        let source = state.llm_workspace_path.join("downloads").join("anim.gif");
        write_test_gif(&source, 3);

        let normalized = meme_prepare_image_asset(&source).expect("preserve gif");
        let raw = std::fs::read(&source).expect("read source");

        // 字节原样保留，不经过任何转换
        assert_eq!(normalized.bytes, raw);
        assert_eq!(normalized.ext, "gif");
        assert_eq!(normalized.mime, "image/gif");
        assert_eq!(normalized.frame_count, 3);
    }

    #[test]
    fn meme_gif_size_limit_should_reject_oversized_gif() {
        let state = meme_test_state();
        let source = state.llm_workspace_path.join("downloads").join("big.gif");
        if let Some(parent) = source.parent() {
            std::fs::create_dir_all(parent).expect("create source parent");
        }
        std::fs::write(&source, vec![0u8; (5 * 1024 * 1024 + 1) as usize]).expect("write big gif");

        let err = meme_check_gif_size_limit("gif", &source).expect_err("should reject");
        assert!(err.contains("图片过大"));

        // 非 GIF 不受大小限制
        assert!(meme_check_gif_size_limit("png", &source).is_ok());
    }

    #[test]
    fn meme_target_path_should_keep_gif_suffix() {
        let state = meme_test_state();
        let target = meme_target_path_for_new_asset(&state, TEST_AGENT_ID, "小猪", "gif")
            .expect("target path");

        assert_eq!(target, test_meme_root(&state).join("小猪.gif"));
    }

    #[test]
    fn meme_encode_animated_webp_should_preserve_frames() {
        let frames = vec![
            (image::RgbaImage::from_pixel(80, 60, image::Rgba([255, 0, 0, 255])), 30),
            (image::RgbaImage::from_pixel(80, 60, image::Rgba([0, 255, 0, 255])), 40),
        ];

        let normalized = meme_encode_animated_webp(frames).expect("encode animated webp");
        let features = webp::BitstreamFeatures::new(&normalized.bytes).expect("webp features");

        assert_eq!(normalized.frame_count, 2);
        assert_eq!((features.width(), features.height()), (80, 60));
        assert!(features.has_animation());
    }

    #[test]
    fn meme_dhash_duplicate_lookup_should_match_identical_images() {
        let state = meme_test_state();
        let existing = test_meme_root(&state).join("happy.webp");
        let source = state.llm_workspace_path.join("downloads").join("candidate.png");
        write_test_png(&existing);
        write_test_png(&source);

        let grouped = meme_available_assets(&state, TEST_AGENT_ID).expect("scan meme assets");
        let index = meme_sync_dhash_index(&state, TEST_AGENT_ID, &grouped);
        let source_hash = compute_meme_dhash_hex(&source).expect("compute source dhash");
        let duplicate =
            meme_find_duplicate_in_assets(&source_hash, &grouped, &index).expect("duplicate");

        assert_eq!(duplicate.name, "happy");
        assert_eq!(duplicate.relative_path, ".meme/agent-meme-test/happy.webp");
        assert_eq!(duplicate.distance, 0);
        assert!(meme_dhash_index_path(&state, TEST_AGENT_ID)
            .expect("dhash index path")
            .exists());
    }

    #[test]
    fn meme_dhash_sync_should_preserve_orphan_paths() {
        let state = meme_test_state();
        let existing = test_meme_root(&state).join("坏笑.webp");
        write_test_png(&existing);
        let mut old_index = std::collections::BTreeMap::new();
        old_index.insert(".meme/已删除.webp".to_string(), "0000000000000000".to_string());
        meme_persist_dhash_index(&state, TEST_AGENT_ID, &old_index).expect("persist old index");

        let grouped = meme_available_assets(&state, TEST_AGENT_ID).expect("scan meme assets");
        let synced = meme_sync_dhash_index(&state, TEST_AGENT_ID, &grouped);

        assert_eq!(
            synced.get(".meme/已删除.webp").map(String::as_str),
            Some("0000000000000000"),
        );
        assert!(synced.contains_key(".meme/agent-meme-test/坏笑.webp"));
    }

    #[test]
    fn meme_dhash_duplicate_lookup_should_match_orphan_hash() {
        let state = meme_test_state();
        let source = state.llm_workspace_path.join("downloads").join("candidate.png");
        write_test_png(&source);
        let source_hash = compute_meme_dhash_hex(&source).expect("compute source dhash");
        let mut dhash_index = std::collections::BTreeMap::new();
        dhash_index.insert(".meme/已删除.webp".to_string(), source_hash.clone());
        let grouped = meme_available_assets(&state, TEST_AGENT_ID).expect("scan meme assets");

        let duplicate =
            meme_find_duplicate_in_assets(&source_hash, &grouped, &dhash_index).expect("duplicate");

        assert_eq!(duplicate.name, "已删除");
        assert_eq!(duplicate.relative_path, ".meme/已删除.webp");
        assert_eq!(duplicate.distance, 0);
    }
}
