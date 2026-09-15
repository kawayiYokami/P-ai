// 能力商店与内置清单的单元测试。
// 覆盖：源转换三传输、缓存 TTL 判定、安装辅助函数、Skill 来源可装性、Skill 启用状态缺省与显式关闭。

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
}
