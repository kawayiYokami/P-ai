// 能力商店条目详情：按条目 id 回填简介。
//
// 为什么单开一个接口：skills.sh 的检索与热门榜都不带简介，简介只在详情页 HTML 的
// `<meta name="description">` 里。把这一步塞进列表的返回路径，一页 30 条就要串行抓 30 个页面，
// 列表得等全部抓完才回来——用户看到的就是「点了没反应」。
// 这里改成列表先返回、前端渲染完再拿 id 来换简介，抓取量与等待都被从列表里摘出去了。

/// 一次最多换多少条简介。列表一页 30 条，60 留一倍余量。
const CATALOG_ABOUT_MAX_IDS: usize = 60;
/// 同时在飞的请求数。单条详情页 80KB 上下，6 个并发够用，别把对方站点打疼。
const CATALOG_ABOUT_CONCURRENCY: usize = 6;
/// 单条请求的超时。没有它，某个连接卡住会把整批拖到天荒地老。
const CATALOG_ABOUT_TIMEOUT_SECS: u64 = 8;
/// 简介缓存文件名，落在 `{app_root}/temp/` 下。
const CATALOG_ABOUT_FILE: &str = "catalog-about.json";

/// skills.sh 条目 id（`owner/repo/skillId`）-> 详情页地址；形态不符时返回空串。
fn skills_sh_detail_url(entry_id: &str) -> String {
    let mut parts = entry_id.trim().splitn(3, '/');
    let owner = parts.next().unwrap_or_default().trim();
    let repo = parts.next().unwrap_or_default().trim();
    let skill_id = parts.next().unwrap_or_default().trim();
    if owner.is_empty() || repo.is_empty() || skill_id.is_empty() {
        return String::new();
    }
    format!("https://www.skills.sh/{owner}/{repo}/{skill_id}")
}

/// 从详情页 HTML 里取简介：`name="description"` 优先，没有就退 `og:description`。
/// 两个都没有时给空串——不猜正文。
fn skills_sh_meta_description(html: &str) -> String {
    let named = catalog_meta_content(html, "name=\"description\"");
    let named = if named.is_empty() {
        catalog_meta_content(html, "name='description'")
    } else {
        named
    };
    let picked = if named.is_empty() {
        let og = catalog_meta_content(html, "property=\"og:description\"");
        if og.is_empty() {
            catalog_meta_content(html, "property='og:description'")
        } else {
            og
        }
    } else {
        named
    };
    catalog_decode_entities(&picked)
}

/// 在 `<meta ...>` 里找指定属性，取出同一标签内的 `content` 值。
/// 属性顺序在真实页面里两种都出现过（`name` 在前、`content` 在前），所以按标签切而不是按顺序切。
fn catalog_meta_content(html: &str, attribute: &str) -> String {
    let mut rest = html;
    while let Some(index) = rest.find("<meta") {
        let after = &rest[index..];
        let end = after.find('>').map(|offset| offset + 1).unwrap_or(after.len());
        let tag = &after[..end];
        if tag.contains(attribute) {
            if let Some(value) = catalog_tag_attribute(tag, "content") {
                return value;
            }
        }
        rest = &after[end..];
    }
    String::new()
}

/// 取标签内某个属性的值，两种引号都认。
fn catalog_tag_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!("{name}={quote}");
        if let Some(start) = tag.find(&pattern) {
            let tail = &tag[start + pattern.len()..];
            if let Some(end) = tail.find(quote) {
                return Some(tail[..end].to_string());
            }
        }
    }
    None
}

/// 还原 meta 里常见的 HTML 实体。描述文案里的引号、& 号都是实体，不还原会把它们原样显示出来。
fn catalog_decode_entities(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices();
    while let Some((index, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let tail = &raw[index..];
        let Some(semicolon) = tail.find(';') else {
            out.push(ch);
            continue;
        };
        // 实体名不会很长，避免把一整段文本当成实体去搜分号。
        if semicolon > 10 {
            out.push(ch);
            continue;
        }
        let entity = &tail[1..semicolon];
        let decoded = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse::<u32>().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(value) => {
                out.push(value);
                // 跳过实体剩余部分（含分号）。
                for _ in 0..semicolon {
                    chars.next();
                }
            }
            None => out.push(ch),
        }
    }
    out
}

fn catalog_about_cache_path(state: &AppState) -> std::path::PathBuf {
    app_root_from_data_path(&state.data_path)
        .join("temp")
        .join(CATALOG_ABOUT_FILE)
}

fn catalog_about_cache_read(state: &AppState) -> std::collections::HashMap<String, String> {
    let path = catalog_about_cache_path(state);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Default::default();
    };
    serde_json::from_str::<std::collections::HashMap<String, String>>(&raw).unwrap_or_default()
}

fn catalog_about_cache_write(
    state: &AppState,
    map: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let path = catalog_about_cache_path(state);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("创建简介缓存目录失败（{}）：{err}", parent.display()))?;
    }
    let raw = serde_json::to_vec_pretty(map)
        .map_err(|err| format!("序列化简介缓存失败：{err}"))?;
    std::fs::write(&path, raw)
        .map_err(|err| format!("写入简介缓存失败（{}）：{err}", path.display()))
}

/// 抓一条详情页，取它的 meta 简介。取不到返回空串，不当作错误——
/// 简介是锦上添花的东西，抓不到就退回卡片上的占位，别让整批失败。
async fn fetch_skills_sh_description(state: &AppState, entry_id: &str) -> String {
    let url = skills_sh_detail_url(entry_id);
    if url.is_empty() {
        return String::new();
    }
    let request = state
        .shared_http_client
        .get(&url)
        .header("Accept", "text/html")
        .send();
    let Ok(Ok(resp)) = tokio::time::timeout(
        std::time::Duration::from_secs(CATALOG_ABOUT_TIMEOUT_SECS),
        request,
    )
    .await
    else {
        return String::new();
    };
    if !resp.status().is_success() {
        return String::new();
    }
    // 整页 80KB 上下，直接收下来再从 head 里取 meta；单条只发一次请求。
    let body = match tokio::time::timeout(
        std::time::Duration::from_secs(CATALOG_ABOUT_TIMEOUT_SECS),
        resp.text(),
    )
    .await
    {
        Ok(Ok(text)) => text,
        _ => return String::new(),
    };
    skills_sh_meta_description(&body)
}

/// 按 id 批量取简介：缓存命中的直接给，剩下的并发抓。
async fn catalog_skill_descriptions_inner(
    state: &AppState,
    ids: &[String],
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut wanted = Vec::<String>::new();
    let mut cache = catalog_about_cache_read(state);
    for id in ids {
        let id = id.trim();
        if id.is_empty() || wanted.iter().any(|hit| hit == id) {
            continue;
        }
        if !cache.contains_key(id) {
            wanted.push(id.to_string());
        }
        if wanted.len() >= CATALOG_ABOUT_MAX_IDS {
            break;
        }
    }

    let mut fetched = std::collections::HashMap::<String, String>::new();
    if !wanted.is_empty() {
        use futures_util::StreamExt as _;
        let results = futures_util::stream::iter(wanted.iter().cloned())
            .map(|id| async move {
                let description = fetch_skills_sh_description(state, &id).await;
                (id, description)
            })
            .buffer_unordered(CATALOG_ABOUT_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        for (id, description) in results {
            if description.trim().is_empty() {
                continue;
            }
            fetched.insert(id.clone(), description.clone());
            cache.insert(id, description);
        }
        if !fetched.is_empty() {
            // 写缓存失败不影响这次返回，下次再写。
            let _ = catalog_about_cache_write(state, &cache);
        }
    }

    // 结果按请求的 id 顺序发回：前端拿到就能直接按 id 回填，不用自己排。
    let mut out = std::collections::HashMap::<String, String>::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        if let Some(description) = fetched.get(id).or_else(|| cache.get(id)) {
            out.insert(id.to_string(), description.clone());
        }
    }
    Ok(out)
}
