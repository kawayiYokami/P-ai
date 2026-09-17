// 能力商店与内置清单的单元测试。
// 覆盖：源转换三传输、缓存 TTL 判定、安装辅助函数、Skill 来源可装性、Skill 启用状态缺省与显式关闭、store 命令参数与来源分组。

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use uuid::Uuid;

    fn catalog_test_state(label: &str) -> AppState {
        let root = std::env::temp_dir().join(format!(
            "eca-catalog-test-{label}-{}",
            Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        let state = AppState::new().expect("create test app state");
        AppState {
            config_path: root.join("app_config.toml"),
            data_path: root.join("config_mark"),
            llm_workspace_path: root.join("llm-workspace"),
            ..state
        }
    }

    #[test]
    fn store_args_should_split_flags_from_positional() {
        let to_args = |raw: &[&str]| raw.iter().map(|value| value.to_string()).collect::<Vec<_>>();

        let parsed = parse_store_args(&to_args(&[
            "search", "playwright", "--source", "clawhub", "--page", "2",
        ]));
        assert_eq!(
            parsed.positional,
            vec!["search".to_string(), "playwright".to_string()]
        );
        assert_eq!(parsed.flag("source"), Some("clawhub"));
        assert_eq!(parsed.flag("page"), Some("2"));
        assert_eq!(parsed.flag("kind"), None);

        let parsed = parse_store_args(&to_args(&["ls", "--kind"]));
        assert_eq!(parsed.positional, vec!["ls".to_string()]);
        assert_eq!(parsed.flag("kind"), None);
    }

    #[test]
    fn store_command_should_treat_install_as_write() {
        let to_args = |raw: &[&str]| raw.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        assert!(store_command_is_readonly(&to_args(&[])));
        assert!(store_command_is_readonly(&to_args(&["ls"])));
        assert!(store_command_is_readonly(&to_args(&["search", "pdf"])));
        assert!(!store_command_is_readonly(&to_args(&["install", "clawhub", "a/b"])));
    }

    #[test]
    fn store_sources_should_split_by_kind_and_default_to_skill() {
        let all = store_sources_for_kind(None).expect("list all sources");
        assert!(all.iter().any(|item| item.kind == CATALOG_KIND_SKILL));
        assert!(all.iter().any(|item| item.kind == CATALOG_KIND_MCP));

        let skills = store_sources_for_kind(Some(CATALOG_KIND_SKILL)).expect("list skill sources");
        assert!(!skills.is_empty());
        assert!(skills.iter().all(|item| item.kind == CATALOG_KIND_SKILL));
        assert!(store_sources_for_kind(Some("bogus")).is_err());

        assert_eq!(
            store_default_source(None).expect("default source"),
            CATALOG_SOURCE_MODELSCOPE_SKILL.to_string()
        );
        assert_eq!(
            store_default_source(Some(CATALOG_KIND_MCP)).expect("mcp default source"),
            CATALOG_SOURCE_MODELSCOPE_MCP.to_string()
        );
    }

    #[test]
    fn modelscope_pick_definition_should_cover_three_transports() {
        let stdio = serde_json::json!({
            "ServerConfig": [{ "mcpServers": { "demo": { "command": "npx" } } }]
        });
        let streamable = serde_json::json!({
            "StreamableHTTPServerConfig": [{ "mcpServers": { "demo": { "url": "https://x/mcp" } } }]
        });
        let sse = serde_json::json!({
            "SSEServerConfig": [{ "mcpServers": { "demo": { "url": "https://x/sse" } } }]
        });

        assert_eq!(modelscope_pick_definition(&stdio).map(|v| v.0), Some("stdio".to_string()));
        assert_eq!(
            modelscope_pick_definition(&streamable).map(|v| v.0),
            Some("streamable_http".to_string())
        );
        assert_eq!(modelscope_pick_definition(&sse).map(|v| v.0), Some("sse".to_string()));
        assert!(modelscope_pick_definition(&serde_json::json!({})).is_none());
    }

    #[test]
    fn modelscope_pick_definition_should_prefer_stdio_when_multiple_present() {
        let item = serde_json::json!({
            "ServerConfig": [{ "mcpServers": { "demo": { "command": "npx" } } }],
            "StreamableHTTPServerConfig": [{ "mcpServers": { "demo": { "url": "https://x/mcp" } } }]
        });
        assert_eq!(modelscope_pick_definition(&item).map(|v| v.0), Some("stdio".to_string()));
    }

    #[test]
    fn remote_catalog_cache_staleness_should_respect_ttl_boundary() {
        let now = chrono::Utc::now().timestamp_millis();
        let fresh = RemoteCatalogCacheFile {
            updated_at: String::new(),
            fetched_at_ms: now - 1000,
            payload: Value::Null,
        };
        let stale = RemoteCatalogCacheFile {
            updated_at: String::new(),
            fetched_at_ms: now - CATALOG_CACHE_MAX_AGE_MS - 60_000,
            payload: Value::Null,
        };
        assert!(!remote_catalog_cache_is_stale(&fresh, CATALOG_CACHE_MAX_AGE_MS));
        assert!(remote_catalog_cache_is_stale(&stale, CATALOG_CACHE_MAX_AGE_MS));
    }

    #[test]
    fn github_tree_url_should_map_to_skill_raw() {
        let converted = github_tree_url_to_skill_raw("https://github.com/owner/repo/tree/main/skills/demo");
        assert_eq!(
            converted.as_deref(),
            Some("https://raw.githubusercontent.com/owner/repo/main/skills/demo/SKILL.md")
        );
        assert!(github_tree_url_to_skill_raw("https://gitlab.com/owner/repo/tree/main/demo").is_none());
        assert!(github_tree_url_to_skill_raw("https://github.com/owner/repo/blob/main/x").is_none());
    }

    #[test]
    fn skill_source_installable_should_require_a_fetchable_markdown() {
        // GitHub 目录地址：可映射为目录内 SKILL.md。
        assert!(skill_source_is_installable(
            "https://github.com/owner/repo/tree/main/skills/demo"
        ));
        // 直接指向文件。
        assert!(skill_source_is_installable(
            "https://raw.githubusercontent.com/owner/repo/main/SKILL.md"
        ));
        assert!(skill_source_is_installable("https://example.com/demo/SKILL.md"));
        // 仓库根地址抓下来是 HTML，装不了。
        assert!(!skill_source_is_installable("https://github.com/AMap-Web/amap-lbs-skill"));
        // 站点页面同理。
        assert!(!skill_source_is_installable("https://modelscope.cn/studios"));
        // 非 GitHub 的目录页也无法静态映射。
        assert!(!skill_source_is_installable(
            "https://gitlab.com/owner/repo/tree/main/skills/demo"
        ));
        assert!(!skill_source_is_installable(""));
    }

    #[tokio::test]
    async fn catalog_cache_should_report_whether_data_came_from_local_cache() {
        let state = catalog_test_state("cache-hit");
        let file = "catalog_cache_outcome_test.json";

        // 无缓存 + 拉取成功 → 新数据。
        let (_, hit) = ensure_remote_catalog_cache(
            &state,
            file,
            CATALOG_CACHE_MAX_AGE_MS,
            "测试",
            || async { Ok(serde_json::json!({ "v": 1 })) },
        )
        .await
        .expect("首次拉取应成功");
        assert!(!hit, "首次拉取不应标记为取自本地缓存");

        // 缓存未过期 → 命中，且不再触发拉取。
        let fetched = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fetched_probe = fetched.clone();
        let (cache, hit) = ensure_remote_catalog_cache(
            &state,
            file,
            CATALOG_CACHE_MAX_AGE_MS,
            "测试",
            move || {
                let flag = fetched_probe.clone();
                async move {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(serde_json::json!({ "v": 2 }))
                }
            },
        )
        .await
        .expect("新鲜缓存应直接命中");
        assert!(hit, "新鲜缓存应标记为取自本地缓存");
        assert!(
            !fetched.load(std::sync::atomic::Ordering::SeqCst),
            "新鲜缓存不得触发重新拉取"
        );
        assert_eq!(cache.payload["v"].as_i64(), Some(1));

        // 缓存过期 + 拉取失败 → 回退旧缓存，数据仍取自本地缓存。
        // TTL 取负值保证刚写入的缓存也判定为过期。
        let (cache, hit) = ensure_remote_catalog_cache(
            &state,
            file,
            -1,
            "测试",
            || async { Err("拉取失败".to_string()) },
        )
        .await
        .expect("拉取失败应回退旧缓存");
        assert!(hit, "回退旧缓存应标记为取自本地缓存");
        assert_eq!(cache.payload["v"].as_i64(), Some(1));

        let _ = fs::remove_dir_all(state.config_path.parent().unwrap_or(&state.config_path));
    }

    #[test]
    fn skill_install_written_path_should_follow_sanitized_dir_name() {
        // id 末段含 `.`，落盘目录名会被清洗成 demo_skill。
        let entry = skill_entry("owner/demo.skill");
        let dir_name = skill_dir_name_from_entry(&entry);
        assert_eq!(dir_name, "demo_skill", "落盘目录名应经清洗");

        let rel = skill_installed_skill_md_rel_path(&dir_name);
        assert_eq!(rel, "skills/demo_skill/SKILL.md");
        // 若沿用 id 的原始末段，路径会指向一个并未落盘的目录。
        assert_ne!(
            rel,
            skill_installed_skill_md_rel_path("demo.skill"),
            "写入路径必须与清洗后的目录名一致"
        );
    }

    #[test]
    fn apply_env_overrides_should_inject_values_into_definition() {
        let definition = serde_json::json!({
            "mcpServers": {
                "demo": { "command": "npx", "env": { "KEY": "" } }
            }
        })
        .to_string();
        let mut values = std::collections::HashMap::new();
        values.insert("KEY".to_string(), "secret".to_string());
        let updated = apply_env_overrides(&definition, &values);
        let parsed: Value = serde_json::from_str(&updated).expect("parse updated definition");
        assert_eq!(
            parsed["mcpServers"]["demo"]["env"]["KEY"].as_str(),
            Some("secret")
        );
        // 空映射应原样返回。
        assert_eq!(
            apply_env_overrides(&definition, &std::collections::HashMap::new()),
            definition
        );
    }

    #[test]
    fn smithery_detail_should_expose_definition_and_required_env() {
        let detail = serde_json::json!({
            "qualifiedName": "@demo/server",
            "connections": [{
                "deploymentUrl": "https://demo.example.com/mcp",
                "configSchema": { "required": ["API_KEY", "TOKEN"] }
            }]
        });
        let definition = smithery_definition_from_detail(&detail, "fallback").expect("definition");
        assert_eq!(
            definition["mcpServers"]["@demo-server"]["url"].as_str(),
            Some("https://demo.example.com/mcp")
        );
        let mut required = smithery_required_env_from_detail(&detail);
        required.sort();
        assert_eq!(required, vec!["API_KEY".to_string(), "TOKEN".to_string()]);
    }

    fn skill_entry(id: &str) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            kind: CATALOG_KIND_SKILL.to_string(),
            source: CATALOG_SOURCE_MODELSCOPE_SKILL.to_string(),
            categories: Vec::new(),
            popularity: 0,
            author: String::new(),
            homepage: String::new(),
            icon: String::new(),
            transport: String::new(),
            definition_json: String::new(),
            required_env: Vec::new(),
            tools: Vec::new(),
            detail_url: String::new(),
            install_ready: true,
            installed: false,
            enabled: false,
            local_id: String::new(),
        }
    }

    #[test]
    fn mark_catalog_install_state_should_flag_installed_skill_only() {
        let state = catalog_test_state("install-state");
        let skills_root = llm_workspace_skills_root(&state).expect("skills root");
        let dir = skills_root.join("community-skill");
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: 社区技能\ndescription: d\n---\n\nbody\n",
        )
        .expect("write skill md");

        let mut entries = vec![
            skill_entry("owner/community-skill"),
            skill_entry("owner/ghost-skill"),
        ];
        mark_catalog_install_state(&state, CATALOG_SOURCE_MODELSCOPE_SKILL, &mut entries);

        assert!(entries[0].installed, "目录已存在应标记为已安装");
        assert_eq!(entries[0].local_id, "社区技能", "本地标识应为 frontmatter 技能名");
        assert!(entries[0].enabled, "未显式关闭时缺省为启用");
        assert!(!entries[1].installed, "目录不存在不应标记为已安装");
        assert!(entries[1].local_id.is_empty());

        let _ = fs::remove_dir_all(state.llm_workspace_path.parent().unwrap_or(&state.llm_workspace_path));
    }

    #[test]
    fn skill_enabled_policy_should_default_enabled_then_persist_explicit_disable() {
        let state = catalog_test_state("skill-policy");
        let dir = llm_workspace_skill_policies_dir(&state).expect("skill policies dir");
        // 目录不存在时，没有任何显式状态，调用方按缺省启用处理。
        assert!(!dir.exists());
        assert!(load_skill_enabled_map(&state).expect("load empty map").is_empty());

        write_skill_enabled(&state, "demo-skill", false).expect("write disabled");
        let map = load_skill_enabled_map(&state).expect("load map");
        assert_eq!(map.get("demo-skill"), Some(&false));

        write_skill_enabled(&state, "demo-skill", true).expect("write enabled");
        let map = load_skill_enabled_map(&state).expect("reload map");
        assert_eq!(map.get("demo-skill"), Some(&true));

        let _ = fs::remove_dir_all(state.llm_workspace_path.parent().unwrap_or(&state.llm_workspace_path));
    }

    #[test]
    fn skill_kind_sources_should_include_clawhub_alongside_modelscope() {
        let sources = catalog_sources_for_kind(CATALOG_KIND_SKILL);
        let ids = sources.iter().map(|item| item.id.as_str()).collect::<Vec<_>>();
        assert_eq!(sources.len(), 2);
        assert!(ids.contains(&CATALOG_SOURCE_MODELSCOPE_SKILL));
        assert!(ids.contains(&CATALOG_SOURCE_CLAWHUB));
        assert_eq!(
            catalog_kind_for_source(CATALOG_SOURCE_CLAWHUB),
            CATALOG_KIND_SKILL
        );
    }

    #[test]
    fn mcp_kind_sources_should_stay_at_three_without_clawhub() {
        let sources = catalog_sources_for_kind(CATALOG_KIND_MCP);
        let ids = sources.iter().map(|item| item.id.as_str()).collect::<Vec<_>>();
        assert_eq!(sources.len(), 3);
        assert!(!ids.contains(&CATALOG_SOURCE_CLAWHUB));
        assert_eq!(
            catalog_kind_for_source(CATALOG_SOURCE_CLAWHUB),
            CATALOG_KIND_SKILL,
            "ClawHub 只属于 Skill 分支"
        );
    }

    #[test]
    fn clawhub_list_entry_should_map_core_fields() {
        let item = serde_json::json!({
            "slug": "linux-disk-triage",
            "displayName": "Linux 磁盘告警治理",
            "summary": "磁盘清理与扩容分析",
            "topics": ["devops", "disk"],
            "stats": { "installs": 120, "downloads": 300 },
            "metadata": null
        });
        let entry = clawhub_list_entry(&item);
        assert_eq!(entry.id, "linux-disk-triage");
        assert_eq!(entry.name, "Linux 磁盘告警治理");
        assert_eq!(entry.description, "磁盘清理与扩容分析");
        assert_eq!(entry.categories, vec!["devops".to_string(), "disk".to_string()]);
        assert_eq!(entry.popularity, 120, "热度优先取安装数");
        assert_eq!(entry.kind, CATALOG_KIND_SKILL);
        assert_eq!(entry.source, CATALOG_SOURCE_CLAWHUB);
        assert!(entry.install_ready);
        assert!(entry.detail_url.contains("slug=linux-disk-triage"));
    }

    #[test]
    fn clawhub_list_entry_should_fall_back_to_downloads_and_keep_author_blank() {
        let item = serde_json::json!({
            "slug": "demo",
            "stats": { "installs": 0, "downloads": 42 }
        });
        let entry = clawhub_list_entry(&item);
        assert_eq!(entry.name, "demo", "无展示名时回落到 slug");
        assert_eq!(entry.popularity, 42, "无安装数时回落到下载数");
        assert!(entry.author.is_empty(), "浏览端点不返回作者");
    }

    #[test]
    fn clawhub_search_entry_should_join_owner_and_canonical_url() {
        let item = serde_json::json!({
            "slug": "linux-disk-triage",
            "displayName": "Linux",
            "summary": "x",
            "downloads": 7,
            "ownerHandle": "peterliu-512",
            "canonicalUrl": "/peterliu-512/skills/linux-disk-triage"
        });
        let entry = clawhub_search_entry(&item);
        assert_eq!(entry.id, "peterliu-512/linux-disk-triage", "检索态用 owner/slug 唯一标识");
        assert_eq!(entry.author, "peterliu-512");
        assert_eq!(entry.popularity, 7);
        assert_eq!(
            entry.homepage,
            "https://clawhub.ai/peterliu-512/skills/linux-disk-triage"
        );
        assert!(entry.detail_url.contains("ownerHandle=peterliu-512"));
    }

    #[test]
    fn clawhub_search_entry_should_fall_back_when_canonical_missing() {
        let item = serde_json::json!({ "slug": "demo" });
        let entry = clawhub_search_entry(&item);
        assert_eq!(entry.id, "demo", "无发布者时退回裸 slug");
        assert_eq!(entry.homepage, "https://clawhub.ai/skills/demo");
    }

    #[test]
    fn clawhub_split_ref_should_separate_owner_from_slug() {
        assert_eq!(
            clawhub_split_ref("peterliu-512/linux-disk-triage"),
            ("peterliu-512".to_string(), "linux-disk-triage".to_string())
        );
        assert_eq!(
            clawhub_split_ref("linux-disk-triage"),
            (String::new(), "linux-disk-triage".to_string())
        );
    }

    #[test]
    fn clawhub_download_url_should_only_carry_owner_when_known() {
        assert_eq!(
            clawhub_download_url("peterliu-512", "linux-disk-triage"),
            "https://clawhub.ai/api/v1/download?slug=linux-disk-triage&ownerHandle=peterliu-512"
        );
        assert_eq!(
            clawhub_download_url("", "linux-disk-triage"),
            "https://clawhub.ai/api/v1/download?slug=linux-disk-triage"
        );
    }

    #[test]
    fn clawhub_entries_from_should_read_both_items_and_results() {
        let list = serde_json::json!({ "items": [{ "slug": "a" }], "nextCursor": null });
        assert_eq!(clawhub_entries_from(&list, clawhub_list_entry).len(), 1);
        let search = serde_json::json!({ "results": [{ "slug": "a" }, { "slug": "b" }] });
        assert_eq!(clawhub_entries_from(&search, clawhub_search_entry).len(), 2);
        assert!(clawhub_entries_from(&serde_json::json!({}), clawhub_list_entry).is_empty());
    }

    /// 浏览态条目只带裸 slug，安装入口必须能直接据此构造条目，
    /// 不能再走「精确 id 检索匹配」（那条路对裸 slug 永不命中）。
    #[test]
    fn clawhub_entry_from_ref_should_build_from_id_without_search() {
        let browse = clawhub_entry_from_ref("linux-disk-triage");
        assert_eq!(browse.id, "linux-disk-triage");
        assert_eq!(browse.name, "linux-disk-triage");
        assert_eq!(browse.source, CATALOG_SOURCE_CLAWHUB);
        assert_eq!(browse.kind, CATALOG_KIND_SKILL);
        assert!(browse.install_ready);

        let owned = clawhub_entry_from_ref("acme/linux-disk-triage");
        assert_eq!(owned.id, "acme/linux-disk-triage");
        assert_eq!(owned.name, "linux-disk-triage", "展示名取 slug 段");
    }

    #[test]
    fn clawhub_archive_skip_file_should_only_match_archive_root() {
        assert!(is_clawhub_archive_skip_file(std::path::Path::new("_meta.json")));
        assert!(is_clawhub_archive_skip_file(std::path::Path::new("skill-card.md")));
        assert!(!is_clawhub_archive_skip_file(std::path::Path::new("SKILL.md")));
        assert!(
            !is_clawhub_archive_skip_file(std::path::Path::new("references/_meta.json")),
            "只跳过包根的平台附加文件"
        );
    }

    /// ClawHub 公开接口的联网契约测试：浏览 / 检索 / 安装三段。
    /// 默认忽略，需要网络时手动执行：
    /// `cargo test clawhub_live_contract -- --ignored`
    #[test]
    #[ignore = "需要网络：验证 ClawHub 公开接口契约"]
    fn clawhub_live_contract_should_browse_search_and_install() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        rt.block_on(async {
            let state = catalog_test_state("clawhub-live");

            let browse = fetch_clawhub_browse_page(&state, 1, 5)
                .await
                .expect("browse ClawHub");
            assert!(!browse.entries.is_empty(), "浏览应返回条目");
            assert!(browse.total > 0, "浏览应折算出自有下一页的标记");
            assert!(
                browse.entries.iter().all(|entry| !entry.id.is_empty() && entry.install_ready),
                "每个条目都应有 slug 且可安装"
            );

            let search = fetch_clawhub_search_page(&state, "disk", 5)
                .await
                .expect("search ClawHub");
            assert!(!search.entries.is_empty(), "检索应返回条目");

            let target = search
                .entries
                .first()
                .expect("取一个检索结果")
                .clone();
            assert!(
                target.id.contains('/'),
                "检索条目应带发布者，形如 owner/slug：{}",
                target.id
            );
            // 走真实安装入口：直接调 install_clawhub_skill 会绕过条目定位，掩盖安装派发层的问题。
            let result = install_catalog_entry_inner(
                &state,
                &CatalogInstallInput {
                    source: CATALOG_SOURCE_CLAWHUB.to_string(),
                    entry_id: target.id.clone(),
                    env_values: std::collections::HashMap::new(),
                },
            )
            .await
            .expect("install ClawHub skill");
            assert_eq!(result.kind, CATALOG_KIND_SKILL);
            assert!(!result.local_id.is_empty());
            assert!(!result.enabled, "商店安装的 Skill 默认关闭");

            let skill_dir = state.llm_workspace_path.join("skills").join(&result.local_id);
            let skill_md = skill_dir.join("SKILL.md");
            assert!(skill_md.is_file(), "SKILL.md 应落盘：{}", skill_md.display());
            let content = fs::read_to_string(&skill_md).expect("read SKILL.md");
            assert!(
                content.trim_start().starts_with("---"),
                "落盘内容应是带 frontmatter 的 SKILL.md 正文"
            );
            assert!(
                !skill_dir.join("_meta.json").exists(),
                "平台附加文件不应落盘"
            );

            // 浏览条目不带发布者：安装入口必须能直接接住裸 slug，
            // 并在重名时给出可操作的候选提示，而不是笼统报「未找到条目」。
            let browse_target = browse.entries.first().expect("取一个浏览条目").clone();
            let browse_outcome = install_catalog_entry_inner(
                &state,
                &CatalogInstallInput {
                    source: CATALOG_SOURCE_CLAWHUB.to_string(),
                    entry_id: browse_target.id.clone(),
                    env_values: std::collections::HashMap::new(),
                },
            )
            .await;
            if let Err(err) = browse_outcome {
                assert!(
                    !err.contains("未找到条目"),
                    "浏览态安装不应因 id 匹配失败：{err}"
                );
                assert!(
                    err.contains("多个发布者"),
                    "浏览态条目的失败应指出重名并给出候选，实际：{err}"
                );
            }

            let _ = fs::remove_dir_all(
                state.llm_workspace_path.parent().unwrap_or(&state.llm_workspace_path),
            );
        });
    }
}
