// 商店安装：把远端条目落到本地工作区。
// MCP 写入 servers/ + policies/（默认关闭）；Skill 写入 skills/{name}/ 并落盘启用状态（默认关闭）。

/// 慢路径扫 frontmatter 时每批并发读多少个 SKILL.md。
const SKILLS_SH_FRONTMATTER_SCAN_BATCH: usize = 24;
/// 慢路径最多扫多少个 SKILL.md：`mattpocock/skills` 有 39 个，一次截断会让排在后面的技能永远装不上。
/// 分批命中即停，扫满这个总量才放弃。
const SKILLS_SH_FRONTMATTER_SCAN_MAX: usize = 192;

fn catalog_entry_local_id(source: &str, entry_id: &str) -> String {
    let raw = format!("{source}-{entry_id}");
    sanitize_mcp_server_id_for_filename(&raw)
}

/// 在指定源内定位条目：用条目标识做检索，再按精确 id 匹配。
async fn find_catalog_entry(
    state: &AppState,
    source: &str,
    entry_id: &str,
) -> Result<CatalogEntry, String> {
    let candidates = [
        entry_id.to_string(),
        entry_id.rsplit('/').next().unwrap_or(entry_id).to_string(),
    ];
    for candidate in candidates {
        if candidate.trim().is_empty() {
            continue;
        }
        let page = load_catalog_page(
            state,
            &CatalogSearchInput {
                source: source.to_string(),
                query: candidate.clone(),
                page: 1,
                page_size: 50,
            },
            Some(source),
        )
        .await?;
        if let Some(found) = page.entries.iter().find(|entry| entry.id == entry_id) {
            return Ok(found.clone());
        }
    }
    Err(format!("在来源 {source} 中未找到条目：{entry_id}"))
}

/// GitHub 目录地址 -> 目录内 SKILL.md 的 raw 地址。
/// 两种形态：
/// - 仓库根 `https://github.com/{owner}/{repo}`：SKILL.md 就在根目录（`AMap-Web/amap-lbs-skill` 这类实测有）。
/// - 目录 `https://github.com/{owner}/{repo}/tree/{branch}/{path}`：取该目录下的 SKILL.md。
/// 两种都拼 `HEAD` 当 ref：raw 认 HEAD（实测 200），且不受默认分支改名影响；
/// 拼字面的分支名会在仓库把 main 改成 master 那天整批 404。
fn github_tree_url_to_skill_raw(url: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let mut parts = rest.splitn(3, '/');
    let owner = parts.next().filter(|part| !part.is_empty())?;
    let repo = parts.next().filter(|part| !part.is_empty())?;
    let tail = parts.next().unwrap_or_default();
    // 仓库根地址：没有后半段，SKILL.md 落在根上。
    // 注意根目录没有 SKILL.md 的仓库也会被判成可装，实际只能在安装时 404——列表阶段拿不到仓库树。
    if tail.is_empty() {
        return Some(format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/SKILL.md"
        ));
    }
    let rest = tail.strip_prefix("tree/")?;
    // 分支名丢掉不用：只保留目录路径。
    let (_, path) = rest.split_once('/')?;
    if path.trim().is_empty() {
        return None;
    }
    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/{}/SKILL.md",
        path.trim_end_matches('/')
    ))
}

/// 条目来源能否拉到 SKILL.md。
/// 与安装逻辑同口径：GitHub 仓库根与目录地址可映射为 SKILL.md，
/// 或以 `.md` / raw 形式直接指向文件；站点页面抓到的都是 HTML，视为不可装。
fn skill_source_is_installable(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() {
        return false;
    }
    if github_tree_url_to_skill_raw(url).is_some() {
        return true;
    }
    let lower = url.to_lowercase();
    lower.ends_with(".md")
        || lower.contains("raw.githubusercontent.com")
        || lower.contains("/raw/")
}

async fn fetch_catalog_text(state: &AppState, url: &str) -> Result<String, String> {
    let resp = state
        .shared_http_client
        .get(url)
        .header("Accept", "text/plain, text/markdown, */*")
        .send()
        .await
        .map_err(|err| format!("请求 {url} 失败：{err}"))?;
    if !resp.status().is_success() {
        return Err(format!("请求 {url} 失败：HTTP {}", resp.status()));
    }
    resp.text()
        .await
        .map_err(|err| format!("读取 {url} 响应失败：{err}"))
}

fn skill_dir_name_from_entry(entry: &CatalogEntry) -> String {
    let raw = entry
        .id
        .rsplit('/')
        .next()
        .unwrap_or(entry.id.as_str())
        .trim();
    let sanitized = sanitize_mcp_server_id_for_filename(raw);
    if sanitized.is_empty() {
        "imported-skill".to_string()
    } else {
        sanitized
    }
}

/// 商店安装 Skill 时写入的相对路径。
/// 目录名必须与 `skill_dir_name_from_entry` 的清洗结果一致，
/// 否则 id 里含 `.`、空格等字符时，这里会指向一个并未落盘的目录。
fn skill_installed_skill_md_rel_path(dir_name: &str) -> String {
    format!("skills/{dir_name}/SKILL.md")
}

/// 把环境变量值写进 `mcpServers` 定义。
fn apply_env_overrides(definition_json: &str, env_values: &std::collections::HashMap<String, String>) -> String {
    if env_values.is_empty() {
        return definition_json.to_string();
    }
    let Ok(mut value) = serde_json::from_str::<Value>(definition_json) else {
        return definition_json.to_string();
    };
    if let Some(servers) = value.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        for (_, server) in servers.iter_mut() {
            if !server.is_object() {
                continue;
            }
            let env = server
                .as_object_mut()
                .and_then(|obj| obj.entry("env").or_insert_with(|| serde_json::json!({})).as_object_mut());
            if let Some(env) = env {
                for (key, val) in env_values {
                    env.insert(key.clone(), Value::String(val.clone()));
                }
            }
        }
    }
    serde_json::to_string(&value).unwrap_or_else(|_| definition_json.to_string())
}

fn write_catalog_mcp_files(
    state: &AppState,
    local_id: &str,
    name: &str,
    definition_json: &str,
) -> Result<Vec<String>, String> {
    ensure_workspace_mcp_layout_at_root(&ensure_workspace_root_ready(
        &configured_workspace_root_path(state)?,
    )?)?;
    let servers_dir = llm_workspace_mcp_servers_dir(state)?;
    let policies_dir = llm_workspace_mcp_policies_dir(state)?;
    fs::create_dir_all(&servers_dir)
        .map_err(|err| format!("创建 MCP servers 目录失败：{err}"))?;
    fs::create_dir_all(&policies_dir)
        .map_err(|err| format!("创建 MCP policies 目录失败：{err}"))?;

    let file_stem = sanitize_mcp_server_id_for_filename(local_id);
    let server_path = servers_dir.join(format!("{file_stem}.json"));
    let payload = serde_json::json!({
        "name": name,
        "definitionJson": definition_json,
    });
    let raw = serde_json::to_vec_pretty(&payload)
        .map_err(|err| format!("序列化 MCP 定义失败：{err}"))?;
    fs::write(&server_path, raw)
        .map_err(|err| format!("写入 MCP 定义失败（{}）：{err}", server_path.display()))?;

    let policy_path = policies_dir.join(format!("{file_stem}.json"));
    // 商店安装的条目默认关闭，与内置清单保持一致。
    let policy = serde_json::json!({
        "serverId": file_stem,
        "enabled": false,
        "tools": [],
    });
    let raw = serde_json::to_vec_pretty(&policy)
        .map_err(|err| format!("序列化 MCP 策略失败：{err}"))?;
    fs::write(&policy_path, raw)
        .map_err(|err| format!("写入 MCP 策略失败（{}）：{err}", policy_path.display()))?;

    Ok(vec![
        format!("mcp/servers/{file_stem}.json"),
        format!("mcp/policies/{file_stem}.json"),
    ])
}

async fn install_catalog_mcp(
    state: &AppState,
    entry: &CatalogEntry,
    env_values: &std::collections::HashMap<String, String>,
) -> Result<CatalogInstallResult, String> {
    // 现役两个来源都把启动配置随列表一起给出；缺配置的条目在列表阶段已是 install_ready = false，
    // 安装按钮点不动，这里只作兜底，不再回取详情。
    if entry.definition_json.trim().is_empty() {
        return Err(format!("条目「{}」缺少可用的启动配置。", entry.name));
    }
    let definition_json = entry.definition_json.clone();

    let definition_json = apply_env_overrides(&definition_json, env_values);
    let local_id = catalog_entry_local_id(&entry.source, &entry.id);
    let written = write_catalog_mcp_files(state, &local_id, &entry.name, &definition_json)?;
    reload_workspace(state).await?;
    runtime_log_info(format!(
        "[能力商店] 已安装 MCP：name={}，source={}，local_id={local_id}",
        entry.name, entry.source
    ));
    Ok(CatalogInstallResult {
        kind: CATALOG_KIND_MCP.to_string(),
        local_id,
        enabled: false,
        written_paths: written,
    })
}

// ==================== skills.sh 安装 ====================

const GITHUB_TREE_API: &str = "https://api.github.com/repos";

/// skills.sh 条目 id -> `(owner, repo, skillId)`；不是三段形态返回 None。
fn skills_sh_split_entry_id(entry_id: &str) -> Option<(String, String, String)> {
    let mut parts = entry_id.trim().rsplitn(3, '/');
    let skill_id = parts.next()?.trim();
    let repo = parts.next()?.trim();
    let owner = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || skill_id.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string(), skill_id.to_string()))
}

/// 包内不该按文本落盘的二进制文件。
fn skills_sh_skip_binary_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".zip", ".woff", ".woff2",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// 以 SKILL.md 实际所在目录为基准，取同目录的文本引用文件。
/// 目录基准只能从命中的 SKILL.md 反推：skill 目录常常不在仓库根，
/// 而 skills.sh 的 skillId 是 frontmatter 的 name，未必等于目录名。
fn skills_sh_sibling_paths(paths: &[String], skill_md: &str) -> (String, Vec<String>) {
    let dir_prefix = skill_md
        .strip_suffix("SKILL.md")
        .unwrap_or(skill_md)
        .to_string();
    let mut siblings = Vec::new();
    for path in paths {
        if path == skill_md || !path.starts_with(&dir_prefix) {
            continue;
        }
        // SKILL.md 落在仓库根时 dir_prefix 是空串，此时只算根下的文件，
        // 否则整棵树都会被当成「同目录」一起拖下来。
        if dir_prefix.is_empty() && path.contains('/') {
            continue;
        }
        if skills_sh_skip_binary_path(path) {
            continue;
        }
        siblings.push(path.clone());
    }
    (dir_prefix, siblings)
}

/// 落盘相对路径一律相对 SKILL.md 所在目录，保证 SKILL.md 落在技能目录根上。
fn skills_sh_relative_path(path: &str, dir_prefix: &str) -> String {
    path.strip_prefix(dir_prefix).unwrap_or(path).to_string()
}

/// 从 SKILL.md 文本里取 frontmatter 的 `name`。
/// skills.sh 的 skillId 就是这个字段，不等于仓库目录名。
fn skills_sh_frontmatter_name(content: &str) -> Option<String> {
    let mut lines = content.lines();
    let first = lines.next()?.trim_start_matches('\u{feff}').trim();
    if first != "---" {
        return None;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }
        let value = raw_value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// 在 `(路径, frontmatter name)` 候选里挑出 name 与 skillId 一致的那一条。
fn skills_sh_select_by_name(
    candidates: &[(String, Option<String>)],
    skill_id: &str,
) -> Option<String> {
    candidates
        .iter()
        .find(|(_, name)| name.as_deref() == Some(skill_id))
        .map(|(path, _)| path.clone())
}

/// 快路径：目录名恰好等于 skillId（`obra/superpowers` 这类）。
fn skills_sh_pick_skill_paths(
    paths: &[String],
    skill_id: &str,
) -> Result<(String, String, Vec<String>), String> {
    // 两种形态都要：仓库根下的 `{skillId}/SKILL.md` 与嵌套的 `skills/{skillId}/SKILL.md`。
    // 嵌套形态必须带前导 `/`：`ends_with` 是纯字节后缀比较，
    // 少了斜杠会让 `skills/pre-commit/SKILL.md` 命中 `commit`，静默装错技能。
    let root_form = format!("{skill_id}/SKILL.md");
    let nested_form = format!("/{skill_id}/SKILL.md");
    let Some(skill_md) = paths
        .iter()
        .find(|path| *path == &root_form || path.ends_with(&nested_form))
        .cloned()
    else {
        return Err(format!("仓库树里没有 {root_form}"));
    };
    let (dir_prefix, siblings) = skills_sh_sibling_paths(paths, &skill_md);
    Ok((skill_md, dir_prefix, siblings))
}

/// GitHub 匿名仓库树：定位 `{skillId}/SKILL.md`，
/// 返回 `(默认分支 sha, SKILL.md 路径, 同目录其余文本文件)`。
async fn github_skill_tree_files(
    state: &AppState,
    owner: &str,
    repo: &str,
    skill_id: &str,
) -> Result<(String, String, String, Vec<String>), String> {
    let url = format!("{GITHUB_TREE_API}/{owner}/{repo}/git/trees/HEAD?recursive=1");
    let resp = state
        .shared_http_client
        .get(&url)
        .header("User-Agent", SKILLS_SH_USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("请求 GitHub 仓库树失败（{owner}/{repo}）：{err}"))?;
    let status = resp.status();
    // 匿名接口每小时有额度：403 / 429 是限流，不能报成「skill 不存在」。
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(format!(
            "GitHub 接口限流，暂时装不了（{owner}/{repo}），请稍后再试"
        ));
    }
    if !status.is_success() {
        return Err(format!(
            "请求 GitHub 仓库树失败（{owner}/{repo}）：HTTP {status}"
        ));
    }
    let payload = resp
        .json::<Value>()
        .await
        .map_err(|err| format!("解析 GitHub 仓库树失败（{owner}/{repo}）：{err}"))?;
    let tree_sha = json_str(&payload, "sha");
    // 后续 raw 下载一律用这个 commit sha 当 ref。raw 也认 HEAD（实测三个样本均 200），
    // 用 sha 的理由不是 HEAD 不可用，而是它和这份仓库树严格对应：仓库在两次请求之间改默认分支
    // 或推进 HEAD，都不会让「按树定位到的路径」与「下载到的文件」错位。
    if tree_sha.trim().is_empty() {
        return Err(format!(
            "{owner}/{repo}：仓库树没有返回 commit sha，无法确定文件版本"
        ));
    }
    let reference = tree_sha.clone();
    let paths: Vec<String> = payload
        .get("tree")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|item| json_str(item, "type") == "blob")
                .map(|item| json_str(item, "path"))
                .filter(|path| !path.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // 快路径：目录名恰好等于 skillId（`obra/superpowers` 这类）。
    let (skill_md, dir_prefix, siblings) = match skills_sh_pick_skill_paths(&paths, skill_id) {
        Ok(picked) => picked,
        // 慢路径：skills.sh 的 skillId 是 SKILL.md 里的 frontmatter name，不等于目录名
        //（`vercel-labs/agent-skills` 的 vercel-react-best-practices 落在 skills/react-best-practices/）。
        // 仓库树里没有路径映射，只能把各 SKILL.md 的 name 读出来比对。
        Err(_) => {
            match skills_sh_find_skill_md_by_name(
                state,
                owner,
                repo,
                &reference,
                &paths,
                skill_id,
            )
            .await
            {
                Ok(skill_md) => {
                    let (dir_prefix, siblings) = skills_sh_sibling_paths(&paths, &skill_md);
                    (skill_md, dir_prefix, siblings)
                }
                Err(scanned) => {
                    let total = paths
                        .iter()
                        .filter(|path| path.ends_with("SKILL.md"))
                        .count();
                    return Err(skills_sh_not_found_message(
                        owner, repo, skill_id, scanned, total,
                    ));
                }
            }
        }
    };
    Ok((tree_sha, skill_md, dir_prefix, siblings))
}

/// 慢路径未命中时的报错文案。
/// 扫满上限时不能说「不存在」——后面还有没扫到的，只是不再扫了。
fn skills_sh_not_found_message(
    owner: &str,
    repo: &str,
    skill_id: &str,
    scanned: usize,
    total: usize,
) -> String {
    let truncated = if scanned < total {
        format!("，扫描上限为 {SKILLS_SH_FRONTMATTER_SCAN_MAX} 个")
    } else {
        String::new()
    };
    format!(
        "{owner}/{repo}：已扫描前 {scanned} 个 SKILL.md（共 {total} 个）{truncated}，未找到 name 为「{skill_id}」的条目"
    )
}

/// 慢路径：分批并发读候选 SKILL.md 的 frontmatter，命中即停。
/// 返回 `Ok(SKILL.md 路径)`；`Err(已扫描数量)` 表示扫到上限仍未命中。
/// 单个仓库的 SKILL.md 通常十来条，一批并发就读完；`mattpocock/skills` 这类 39 条的仓库靠分批覆盖，
/// 不能按原始顺序截断——排在后面的技能会永远装不上。
///
/// 抓取点用闭包参数化：分批推进、`scanned` 累加、上限截断都是纯算术，不该被 `AppState` 与网络锁住，
/// 测试里传一个不取网的闭包就能钉住这三条。
async fn skills_sh_find_skill_md_by_name(
    state: &AppState,
    owner: &str,
    repo: &str,
    reference: &str,
    paths: &[String],
    skill_id: &str,
) -> Result<String, usize> {
    skills_sh_scan_skill_md_by_name(paths, skill_id, |path| async move {
        let url = format!("https://raw.githubusercontent.com/{owner}/{repo}/{reference}/{path}");
        let name = fetch_catalog_text(state, &url)
            .await
            .ok()
            .and_then(|content| skills_sh_frontmatter_name(&content));
        (path, name)
    })
    .await
}

async fn skills_sh_scan_skill_md_by_name<F, Fut>(
    paths: &[String],
    skill_id: &str,
    fetch: F,
) -> Result<String, usize>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = (String, Option<String>)>,
{
    let candidates = paths
        .iter()
        .filter(|path| path.ends_with("SKILL.md"))
        .collect::<Vec<_>>();
    let mut scanned = 0usize;
    for batch in candidates.chunks(SKILLS_SH_FRONTMATTER_SCAN_BATCH) {
        let remaining = SKILLS_SH_FRONTMATTER_SCAN_MAX.saturating_sub(scanned);
        if remaining == 0 {
            break;
        }
        let batch = &batch[..batch.len().min(remaining)];
        let requests = batch
            .iter()
            .map(|path| fetch((*path).clone()))
            .collect::<Vec<_>>();
        let resolved = futures_util::future::join_all(requests).await;
        if let Some(hit) = skills_sh_select_by_name(&resolved, skill_id) {
            return Ok(hit);
        }
        scanned += batch.len();
    }
    Err(scanned)
}

async fn install_skills_sh_skill(
    state: &AppState,
    entry_id: &str,
) -> Result<CatalogInstallResult, String> {
    let (owner, repo, skill_id) = skills_sh_split_entry_id(entry_id)
        .ok_or_else(|| format!("条目标识不是 owner/repo/skillId 形态：{entry_id}"))?;
    let (tree_sha, skill_md_path, dir_prefix, sibling_paths) =
        github_skill_tree_files(state, &owner, &repo, &skill_id).await?;
    // 仓库树已经保证这里是 commit sha，不再有别的 ref 兜底。
    let reference = tree_sha;

    let workspace_root = configured_workspace_root_path(state)?;
    ensure_workspace_skills_layout_at_root(&workspace_root)?;
    let skills_root = workspace_root.join("skills");
    let sanitized = sanitize_mcp_server_id_for_filename(&skill_id);
    let dir_name = if sanitized.is_empty() {
        "imported-skill".to_string()
    } else {
        sanitized
    };
    let dir = skills_root.join(&dir_name);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("创建 Skill 目录失败（{}）：{err}", dir.display()))?;

    let mut targets = vec![skill_md_path.clone()];
    targets.extend(sibling_paths);
    let mut written_paths = Vec::new();
    for path in &targets {
        let raw_url = format!("https://raw.githubusercontent.com/{owner}/{repo}/{reference}/{path}");
        let content = fetch_catalog_text(state, &raw_url).await?;
        let relative = skills_sh_relative_path(path, &dir_prefix);
        let target = dir.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("创建 Skill 目录失败（{}）：{err}", parent.display()))?;
        }
        fs::write(&target, &content)
            .map_err(|err| format!("写入 Skill 文件失败（{}）：{err}", target.display()))?;
        written_paths.push(format!("skills/{dir_name}/{relative}"));
    }

    let skill_md = dir.join("SKILL.md");
    let skill_name = parse_skill_file(&skill_md)
        .map(|(name, _, _)| name)
        .unwrap_or_else(|_| skill_id.clone());
    // 商店安装的 Skill 默认关闭，由用户自行启用。
    write_skill_enabled(state, &skill_name, false)?;
    reload_workspace(state).await?;
    runtime_log_info(format!(
        "[能力商店] 已安装 Skill：name={skill_name}，source={}，repo={owner}/{repo}/{skill_id}，dir={dir_name}",
        CATALOG_SOURCE_SKILLS_SH
    ));
    Ok(CatalogInstallResult {
        kind: CATALOG_KIND_SKILL.to_string(),
        local_id: dir_name,
        enabled: false,
        written_paths,
    })
}

async fn install_catalog_skill(
    state: &AppState,
    entry: &CatalogEntry,
) -> Result<CatalogInstallResult, String> {
    let source_url = if entry.detail_url.trim().is_empty() {
        entry.homepage.trim().to_string()
    } else {
        entry.detail_url.trim().to_string()
    };
    if source_url.is_empty() {
        return Err(format!("条目「{}」缺少正文来源地址。", entry.name));
    }
    let raw_url = github_tree_url_to_skill_raw(&source_url).unwrap_or_else(|| source_url.clone());
    let content = fetch_catalog_text(state, &raw_url).await?;
    if content.trim().is_empty() {
        return Err(format!("条目「{}」的 SKILL.md 内容为空。", entry.name));
    }
    if !content.trim_start().starts_with("---") {
        return Err(format!(
            "条目「{}」取到的内容不是合法的 SKILL.md（缺少 frontmatter）。",
            entry.name
        ));
    }

    let skills_root = configured_workspace_root_path(state)?.join("skills");
    ensure_workspace_skills_layout_at_root(&configured_workspace_root_path(state)?)?;
    let dir_name = skill_dir_name_from_entry(entry);
    let dir = skills_root.join(&dir_name);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("创建 Skill 目录失败（{}）：{err}", dir.display()))?;
    let skill_md = dir.join("SKILL.md");
    fs::write(&skill_md, &content)
        .map_err(|err| format!("写入 SKILL.md 失败（{}）：{err}", skill_md.display()))?;

    let skill_name = parse_skill_file(&skill_md)
        .map(|(name, _, _)| name)
        .unwrap_or_else(|_| {
            entry
                .id
                .rsplit('/')
                .next()
                .unwrap_or(entry.id.as_str())
                .to_string()
        });
    // 商店安装的 Skill 默认关闭，由用户自行启用。
    write_skill_enabled(state, &skill_name, false)?;
    reload_workspace(state).await?;
    runtime_log_info(format!(
        "[能力商店] 已安装 Skill：name={}，source={}，dir={dir_name}",
        entry.name, entry.source
    ));
    let written_paths = vec![skill_installed_skill_md_rel_path(&dir_name)];
    Ok(CatalogInstallResult {
        kind: CATALOG_KIND_SKILL.to_string(),
        local_id: dir_name,
        enabled: false,
        written_paths,
    })
}

async fn install_catalog_entry_inner(
    state: &AppState,
    input: &CatalogInstallInput,
) -> Result<CatalogInstallResult, String> {
    let source = input.source.trim();
    if source.is_empty() {
        return Err("缺少商店来源。".to_string());
    }
    let entry_id = input.entry_id.trim();
    if entry_id.is_empty() {
        return Err("缺少条目标识。".to_string());
    }
    // skills.sh 的 skill 都在 GitHub 仓库目录里，id 已经带齐仓库与目录，安装时直接查树，
    // 不再回列表接口检索一次。
    if source == CATALOG_SOURCE_SKILLS_SH {
        return install_skills_sh_skill(state, entry_id).await;
    }
    let entry = find_catalog_entry(state, source, entry_id).await?;
    if catalog_kind_for_source(source) == CATALOG_KIND_SKILL {
        install_catalog_skill(state, &entry).await
    } else {
        install_catalog_mcp(state, &entry, &input.env_values).await
    }
}
