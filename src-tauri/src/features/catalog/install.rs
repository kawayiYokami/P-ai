// 商店安装：把远端条目落到本地工作区。
// MCP 写入 servers/ + policies/（默认关闭）；Skill 写入 skills/{name}/ 并落盘启用状态（默认关闭）。

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
fn github_tree_url_to_skill_raw(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://github.com/")?;
    let (owner, rest) = rest.split_once('/')?;
    let (repo, rest) = rest.split_once('/')?;
    let rest = rest.strip_prefix("tree/")?;
    let (branch, path) = rest.split_once('/')?;
    if path.trim().is_empty() {
        return None;
    }
    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{}/SKILL.md",
        path.trim_end_matches('/')
    ))
}

/// 条目来源能否拉到 SKILL.md。
/// 与安装逻辑同口径：GitHub 目录地址可映射为目录内 SKILL.md，
/// 或以 `.md` / raw 形式直接指向文件；仓库根地址、站点页面抓到的都是 HTML，
/// 一律视为不可装（安装时会因缺 frontmatter 失败）。
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
    // Smithery 的列表不带启动配置，安装时再取详情。
    let definition_json = if entry.definition_json.trim().is_empty() {
        if entry.detail_url.trim().is_empty() {
            return Err(format!("条目「{}」缺少可用的启动配置。", entry.name));
        }
        let resp = state
            .shared_http_client
            .get(entry.detail_url.trim())
            .send()
            .await
            .map_err(|err| format!("请求条目详情失败：{err}"))?;
        if !resp.status().is_success() {
            return Err(format!("请求条目详情失败：HTTP {}", resp.status()));
        }
        let detail = resp
            .json::<Value>()
            .await
            .map_err(|err| format!("解析条目详情失败：{err}"))?;
        // 需要密钥的条目在安装前明确提示，避免装出一个连不上的服务。
        let required = smithery_required_env_from_detail(&detail);
        let missing: Vec<String> = required
            .into_iter()
            .filter(|name: &String| {
                env_values
                    .get(name.as_str())
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
            })
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "条目「{}」需要先提供环境变量：{}。请在安装时填写后重试。",
                entry.name,
                missing.join("、")
            ));
        }
        let definition = smithery_definition_from_detail(&detail, &entry.name)
            .ok_or_else(|| format!("条目「{}」没有可用的远端连接地址。", entry.name))?;
        serde_json::to_string(&definition).unwrap_or_default()
    } else {
        entry.definition_json.clone()
    };

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

/// ClawHub 压缩包内的平台附加文件：不是 skill 正文，落盘时跳过。
const CLAWHUB_ARCHIVE_SKIP_FILES: [&str; 2] = ["_meta.json", "skill-card.md"];

/// 只跳过包根的附加文件；skill 自带 reference 目录里的同名文件不受影响。
fn is_clawhub_archive_skip_file(relative_path: &std::path::Path) -> bool {
    let at_archive_root = relative_path
        .parent()
        .map(|parent| parent.as_os_str().is_empty())
        .unwrap_or(true);
    at_archive_root
        && relative_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|name| CLAWHUB_ARCHIVE_SKIP_FILES.contains(&name))
            .unwrap_or(false)
}

/// ClawHub 条目 id 形如 `owner/slug`；只有 slug 时 owner 为空。
fn clawhub_split_ref(entry_id: &str) -> (String, String) {
    match entry_id.split_once('/') {
        Some((owner, slug)) => (owner.trim().to_string(), slug.trim().to_string()),
        None => (String::new(), entry_id.trim().to_string()),
    }
}

/// 由 install 入参的 id 直接构造 ClawHub 条目。
/// 浏览来源的条目只带裸 slug、且列表接口不返回发布者，无法用精确 id 在检索结果里定位，
/// 所以这里不再做二次检索，发布者解析与重名判定统一交给 `resolve_clawhub_publisher`。
fn clawhub_entry_from_ref(entry_id: &str) -> CatalogEntry {
    let (_, slug) = clawhub_split_ref(entry_id);
    let name = if slug.is_empty() {
        entry_id.to_string()
    } else {
        slug
    };
    CatalogEntry {
        id: entry_id.to_string(),
        name,
        description: String::new(),
        kind: CATALOG_KIND_SKILL.to_string(),
        source: CATALOG_SOURCE_CLAWHUB.to_string(),
        categories: Vec::new(),
        popularity: 0,
        author: String::new(),
        homepage: String::new(),
        icon: String::new(),
        transport: String::new(),
        definition_json: String::new(),
        required_env: Vec::new(),
        tools: Vec::new(),
        install_ready: true,
        detail_url: String::new(),
        installed: false,
        enabled: false,
        local_id: String::new(),
    }
}

/// 解析条目要安装的发布者。
/// 条目 id 已带 owner 时直接采用；只有裸 slug 时查详情补全——
/// ClawHub 的 slug 可以重名，缺少发布者就无法唯一定位，此时列出候选交给用户选择。
async fn resolve_clawhub_publisher(
    state: &AppState,
    entry: &CatalogEntry,
) -> Result<(String, String), String> {
    let (owner, slug) = clawhub_split_ref(&entry.id);
    if slug.is_empty() {
        return Err("条目缺少 slug，无法安装。".to_string());
    }
    if !owner.is_empty() {
        return Ok((owner, slug));
    }
    let url = format!(
        "https://clawhub.ai/api/v1/skills/{}",
        urlencoding_encode(&slug)
    );
    let resp = state
        .shared_http_client
        .get(&url)
        .send()
        .await
        .map_err(|err| format!("查询 ClawHub 条目详情失败：{err}"))?;
    let status = resp.status();
    // 先取文本，避免错误体（409 带候选清单、其它纯文本错误）不是 JSON 时先抛解析错。
    let body = resp
        .text()
        .await
        .map_err(|err| format!("读取 ClawHub 条目详情失败：{err}"))?;
    let payload = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
    if status.as_u16() == 409 {
        let candidates = payload
            .get("matches")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        let handle = json_str(item, "ownerHandle");
                        let slug = json_str(item, "slug");
                        if handle.is_empty() {
                            slug
                        } else {
                            format!("@{handle}/{slug}")
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return Err(format!(
            "条目「{}」的名字有多个发布者，无法确定要安装哪一个，请用搜索框搜索后从结果中选择。候选：{}",
            entry.name,
            candidates.join("、")
        ));
    }
    if !status.is_success() {
        let detail = {
            let message = json_str(&payload, "message");
            if message.trim().is_empty() {
                body.chars().take(200).collect::<String>()
            } else {
                message
            }
        };
        return Err(format!("查询 ClawHub 条目详情失败：{status} | {detail}"));
    }
    if payload.is_null() {
        return Err(format!(
            "解析 ClawHub 条目详情失败：响应不是合法 JSON（{} 字节）。",
            body.len()
        ));
    }
    let resolved = payload
        .get("owner")
        .map(|owner| json_str(owner, "handle"))
        .unwrap_or_default();
    if resolved.trim().is_empty() {
        return Err(format!("条目「{}」缺少发布者信息，无法安装。", entry.name));
    }
    Ok((resolved, slug))
}

/// ClawHub 安装：按 slug 取 zip 包，解压到 `skills/{dir}/`。
/// 先把压缩包解到同级临时目录并完成校验，再整体替换目标目录，
/// 避免中途失败在 `skills/` 里留下半截或新旧混合的目录。
async fn install_clawhub_skill(
    state: &AppState,
    entry: &CatalogEntry,
) -> Result<CatalogInstallResult, String> {
    let (owner, slug) = resolve_clawhub_publisher(state, entry).await?;
    let url = clawhub_download_url(&owner, &slug);
    let resp = state
        .shared_http_client
        .get(&url)
        .send()
        .await
        .map_err(|err| format!("下载 ClawHub 条目失败：{err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let snippet = resp
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>();
        return Err(format!("下载 ClawHub 条目失败：{status} | {snippet}"));
    }
    // 该端点对由 GitHub 托管的条目返回 JSON 交接信息，而不是压缩包。
    let is_zip = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("zip"))
        .unwrap_or(false);
    if !is_zip {
        return Err(format!(
            "条目「{}」由外部仓库托管，当前版本暂不支持安装。",
            entry.name
        ));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|err| format!("读取 ClawHub 压缩包失败：{err}"))?;

    let workspace_root = configured_workspace_root_path(state)?;
    ensure_workspace_skills_layout_at_root(&workspace_root)?;
    let skills_root = workspace_root.join("skills");
    let dir_name = skill_dir_name_from_entry(entry);
    let dir = skills_root.join(&dir_name);
    // 先解到同级临时目录，校验全部通过后再整体替换目标目录。
    let staging = skills_root.join(format!(
        ".tmp-clawhub-{dir_name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    ));

    let written_files = match extract_clawhub_archive(&bytes, &staging, entry) {
        Ok(count) => count,
        Err(err) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
    };
    if written_files == 0 {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("条目「{}」的压缩包没有可落盘的文件。", entry.name));
    }
    let skill_md = staging.join("SKILL.md");
    if !skill_md.is_file() {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("条目「{}」的压缩包缺少 SKILL.md。", entry.name));
    }
    let skill_name = match parse_skill_file(&skill_md) {
        Ok((name, _, _)) => name,
        Err(err) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("解析条目「{}」的 SKILL.md 失败：{err}", entry.name));
        }
    };
    if dir.exists() {
        if let Err(err) = fs::remove_dir_all(&dir) {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("清理旧的 Skill 目录失败（{}）：{err}", dir.display()));
        }
    }
    if let Err(err) = fs::rename(&staging, &dir) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("落盘 Skill 目录失败（{}）：{err}", dir.display()));
    }
    // 商店安装的 Skill 默认关闭，由用户自行启用。
    write_skill_enabled(state, &skill_name, false)?;
    reload_workspace(state).await?;
    runtime_log_info(format!(
        "[能力商店] 已安装 Skill：name={}，source={}，dir={dir_name}，文件数={written_files}",
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

/// 把 ClawHub 压缩包解到指定目录，返回落盘文件数。
/// 跳过后端平台附加文件，并用 `enclosed_name` 拒绝越出包根的路径。
fn extract_clawhub_archive(
    bytes: &[u8],
    dir: &std::path::Path,
    entry: &CatalogEntry,
) -> Result<usize, String> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|err| format!("解析 ClawHub 压缩包失败：{err}"))?;
    if archive.is_empty() {
        return Err(format!("条目「{}」的压缩包为空。", entry.name));
    }
    let mut written_files = 0usize;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("读取 ClawHub 压缩包条目失败：{err}"))?;
        // enclosed_name 已拒绝越出包根的相对路径。
        let Some(relative) = file.enclosed_name() else {
            return Err(format!(
                "条目「{}」的压缩包存在不安全路径：{}",
                entry.name,
                file.name()
            ));
        };
        if is_clawhub_archive_skip_file(&relative) {
            continue;
        }
        let output = dir.join(&relative);
        if file.is_dir() {
            fs::create_dir_all(&output)
                .map_err(|err| format!("创建 Skill 目录失败（{}）：{err}", output.display()))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("创建 Skill 目录失败（{}）：{err}", parent.display()))?;
        }
        let mut body = Vec::<u8>::new();
        std::io::Read::read_to_end(&mut file, &mut body)
            .map_err(|err| format!("读取 Skill 文件内容失败：{err}"))?;
        fs::write(&output, body)
            .map_err(|err| format!("写入 Skill 文件失败（{}）：{err}", output.display()))?;
        written_files += 1;
    }
    Ok(written_files)
}

async fn install_catalog_skill(
    state: &AppState,
    entry: &CatalogEntry,
) -> Result<CatalogInstallResult, String> {
    if entry.source == CATALOG_SOURCE_CLAWHUB {
        return install_clawhub_skill(state, entry).await;
    }
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
    // ClawHub 浏览条目只带裸 slug，用精确 id 检索无法命中（检索结果 id 一律带发布者），
    // 因此直接按 id 构造条目，交给安装链路自行解析发布者。
    let entry = if source == CATALOG_SOURCE_CLAWHUB {
        clawhub_entry_from_ref(entry_id)
    } else {
        find_catalog_entry(state, source, entry_id).await?
    };
    if catalog_kind_for_source(source) == CATALOG_KIND_SKILL {
        install_catalog_skill(state, &entry).await
    } else {
        install_catalog_mcp(state, &entry, &input.env_values).await
    }
}
