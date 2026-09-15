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
    Ok(CatalogInstallResult {
        kind: CATALOG_KIND_SKILL.to_string(),
        local_id: dir_name,
        enabled: false,
        written_paths: vec![format!("skills/{}/SKILL.md", entry.id.rsplit('/').next().unwrap_or(""))],
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
    let entry = find_catalog_entry(state, source, entry_id).await?;
    if catalog_kind_for_source(source) == CATALOG_KIND_SKILL {
        install_catalog_skill(state, &entry).await
    } else {
        install_catalog_mcp(state, &entry, &input.env_values).await
    }
}
