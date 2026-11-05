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
            "search", "playwright", "--source", "modelscope-skill", "--page", "2",
        ]));
        assert_eq!(
            parsed.positional,
            vec!["search".to_string(), "playwright".to_string()]
        );
        assert_eq!(parsed.flag("source"), Some("modelscope-skill"));
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
        assert!(!store_command_is_readonly(&to_args(&["install", "skills-sh", "a/b"])));
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
    fn modelscope_required_env_should_read_schema_required_list() {
        // 空 schema：无环境变量，且不得把 schema 顶层键名当变量名。
        let empty = serde_json::json!({
            "EnvSchema": { "properties": {}, "required": [], "type": "object" }
        });
        assert!(modelscope_required_env_names(&empty).is_empty());

        // 字段为 null 的畸形 schema：同样按无变量处理。
        let nullish = serde_json::json!({
            "EnvSchema": { "properties": null, "required": null, "type": "" }
        });
        assert!(modelscope_required_env_names(&nullish).is_empty());

        // 正常 schema：required 数组里才是变量名。
        let real = serde_json::json!({
            "EnvSchema": {
                "properties": {
                    "AMAP_MAPS_API_KEY": { "type": "string" },
                    "OPTIONAL_ONE": { "type": "string" }
                },
                "required": ["AMAP_MAPS_API_KEY"],
                "type": "object"
            }
        });
        assert_eq!(
            modelscope_required_env_names(&real),
            vec!["AMAP_MAPS_API_KEY".to_string()]
        );

        // 整条 entry 的空 schema 不得产生任何 required_env。
        let entry = modelscope_mcp_entry(&serde_json::json!({
            "Path": "@modelcontextprotocol",
            "Name": "fetch",
            "EnvSchema": { "properties": {}, "required": [], "type": "object" },
            "ServerConfig": [{ "mcpServers": { "fetch": { "command": "uvx" } } }]
        }));
        assert!(entry.required_env.is_empty(), "空 schema 不应渲染环境变量表单");
        assert!(entry.install_ready);
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
        assert!(load_skill_enabled_map_at_root(&configured_workspace_root_path(&state).expect("workspace root")).expect("load empty map").is_empty());

        write_skill_enabled(&state, "demo-skill", false).expect("write disabled");
        let map = load_skill_enabled_map_at_root(&configured_workspace_root_path(&state).expect("workspace root")).expect("load map");
        assert_eq!(map.get("demo-skill"), Some(&false));

        write_skill_enabled(&state, "demo-skill", true).expect("write enabled");
        let map = load_skill_enabled_map_at_root(&configured_workspace_root_path(&state).expect("workspace root")).expect("reload map");
        assert_eq!(map.get("demo-skill"), Some(&true));

        let _ = fs::remove_dir_all(state.llm_workspace_path.parent().unwrap_or(&state.llm_workspace_path));
    }

    #[test]
    fn skill_kind_sources_should_pair_modelscope_with_skills_sh() {
        let sources = catalog_sources_for_kind(CATALOG_KIND_SKILL);
        let ids = sources.iter().map(|item| item.id.as_str()).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![CATALOG_SOURCE_MODELSCOPE_SKILL, CATALOG_SOURCE_SKILLS_SH],
            "Skill 分支两个源，顺序固定"
        );
        assert_eq!(
            catalog_kind_for_source(CATALOG_SOURCE_SKILLS_SH),
            CATALOG_KIND_SKILL
        );
    }

    #[test]
    fn mcp_kind_sources_should_pair_modelscope_with_official_registry() {
        let sources = catalog_sources_for_kind(CATALOG_KIND_MCP);
        let ids = sources.iter().map(|item| item.id.as_str()).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![CATALOG_SOURCE_MODELSCOPE_MCP, CATALOG_SOURCE_OFFICIAL],
            "MCP 分支两个源，顺序固定"
        );
        assert_eq!(
            catalog_kind_for_source(CATALOG_SOURCE_OFFICIAL),
            CATALOG_KIND_MCP
        );
    }

    #[test]
    fn skills_sh_trending_should_read_escaped_array_and_drop_dirty_rows() {
        // 页面把列表嵌在 JS 字符串里：键名前后的引号带一层反斜杠转义。
        let escaped = r#"initialSkills\":[{\"source\":\"obra/superpowers\",\"skillId\":\"using-git-worktrees\",\"name\":\"Using Git Worktrees\",\"installs\":1234},{\"source\":\"not-a-repo\",\"skillId\":\"junk\",\"name\":\"Junk\",\"installs\":9}]"#;
        let entries = skills_sh_trending_entries(escaped);
        assert_eq!(entries.len(), 1, "不是 owner/repo 形态的条目要丢掉");
        assert_eq!(entries[0].id, "obra/superpowers/using-git-worktrees");
        assert_eq!(entries[0].popularity, 1234);

        // 不带转义的形态同样要认。
        let plain = r#"initialSkills":[{"source":"obra/superpowers","skillId":"using-git-worktrees","name":"Using Git Worktrees","installs":7}]"#;
        let entries = skills_sh_trending_entries(plain);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].popularity, 7);
    }

    #[test]
    fn skills_sh_search_should_map_fields() {
        let payload = serde_json::json!({
            "skills": [
                {"source": "obra/superpowers", "skillId": "using-git-worktrees", "name": "Using Git Worktrees", "installs": 1234},
                {"source": "单段", "skillId": "broken", "name": "Broken", "installs": 1}
            ]
        });
        let items = payload["skills"].as_array().cloned().unwrap_or_default();
        let entries = skills_sh_entries(&items);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.id, "obra/superpowers/using-git-worktrees");
        assert_eq!(entry.source, CATALOG_SOURCE_SKILLS_SH);
        assert_eq!(entry.kind, CATALOG_KIND_SKILL);
        assert_eq!(entry.popularity, 1234);
        assert_eq!(entry.icon, "https://github.com/obra.png?size=96");
        assert!(entry.install_ready);
        assert!(entry.description.is_empty(), "检索结果没有描述，留空");
    }

    #[test]
    fn skills_sh_search_should_return_empty_when_nothing_found() {
        assert!(skills_sh_entries(&[]).is_empty());
        assert!(
            skills_sh_trending_entries("页面里没有那个键").is_empty(),
            "找不到列表时返回空"
        );
    }

    #[test]
    fn skills_sh_request_should_report_failure() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let state = catalog_test_state("skills-sh-unreachable");
        let err = rt
            .block_on(skills_sh_get(
                &state,
                "http://127.0.0.1:1/nope",
                "application/json",
            ))
            .expect_err("连不上的地址应当报错");
        assert!(
            err.contains("请求 skills.sh 失败") || err.contains("skills.sh 返回"),
            "错误要说清是请求失败：{err}"
        );
        let _ = fs::remove_dir_all(
            state
                .llm_workspace_path
                .parent()
                .unwrap_or(&state.llm_workspace_path),
        );
    }

    #[test]
    fn skills_sh_pick_paths_should_find_skill_md_and_text_siblings() {
        let paths = vec![
            "README.md".to_string(),
            "skills/using-git-worktrees/SKILL.md".to_string(),
            "skills/using-git-worktrees/scripts/setup.sh".to_string(),
            "skills/using-git-worktrees/preview.png".to_string(),
            "skills/other/SKILL.md".to_string(),
        ];
        let (skill_md, dir_prefix, siblings) =
            skills_sh_pick_skill_paths(&paths, "using-git-worktrees").expect("应能定位 SKILL.md");
        assert_eq!(skill_md, "skills/using-git-worktrees/SKILL.md");
        assert_eq!(
            dir_prefix, "skills/using-git-worktrees/",
            "目录基准应是 SKILL.md 所在目录，不是 skillId"
        );
        assert_eq!(
            siblings,
            vec!["skills/using-git-worktrees/scripts/setup.sh".to_string()],
            "同目录文本文件带走，图片跳过，别的技能不碰"
        );
    }

    #[test]
    fn skills_sh_pick_paths_should_report_missing_skill_md() {
        let paths = vec!["README.md".to_string()];
        let err = skills_sh_pick_skill_paths(&paths, "missing").expect_err("找不到要报错");
        assert!(
            err.contains("missing/SKILL.md"),
            "错误要说清缺哪个文件：{err}"
        );
    }

    #[test]
    fn official_registry_icon_should_prefer_own_icon_then_github_owner_then_empty() {
        let with_icon = serde_json::json!({
            "server": {
                "name": "io.github.foo/bar",
                "icons": [{"src": "ftp://x/y.png"}, {"src": "https://cdn.example.com/bar.png"}],
                "repository": {"url": "https://github.com/foo/bar"}
            }
        });
        let entry = official_registry_entry(&with_icon);
        assert_eq!(
            entry.icon, "https://cdn.example.com/bar.png",
            "只取 icons 里第一个 https 地址"
        );
        assert_eq!(entry.author, "foo");

        let owner_only = serde_json::json!({
            "server": {
                "name": "io.github.baz/qux",
                "repository": {"url": "https://github.com/baz/qux"}
            }
        });
        let entry = official_registry_entry(&owner_only);
        assert_eq!(
            entry.icon, "https://github.com/baz.png?size=96",
            "没有自带图标时用 GitHub owner 头像"
        );
        assert_eq!(entry.author, "baz");

        let neither = serde_json::json!({"server": {"name": "acme.dev/tool"}});
        let entry = official_registry_entry(&neither);
        assert_eq!(entry.icon, "", "推不出 GitHub owner 时不编造图标");
        assert_eq!(entry.author, "dev.acme", "命名空间倒序拼成域名形态");
    }

    #[test]
    fn catalog_icon_mime_should_recognize_known_magic_numbers() {
        assert_eq!(
            catalog_icon_mime_from_bytes(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1]),
            Some("image/png")
        );
        assert_eq!(
            catalog_icon_mime_from_bytes(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("image/jpeg")
        );
        assert_eq!(catalog_icon_mime_from_bytes(b"GIF89a...."), Some("image/gif"));
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBP");
        assert_eq!(catalog_icon_mime_from_bytes(&webp), Some("image/webp"));
        assert_eq!(
            catalog_icon_mime_from_bytes(&[0x00, 0x00, 0x01, 0x00, 1]),
            Some("image/x-icon")
        );
        assert_eq!(
            catalog_icon_mime_from_bytes(b"<svg xmlns=\"x\"></svg>"),
            Some("image/svg+xml")
        );
        assert_eq!(
            catalog_icon_mime_from_bytes(b"<svg><script>alert(1)</script></svg>"),
            None,
            "带脚本的 SVG 不收"
        );
        assert_eq!(
            catalog_icon_mime_from_bytes(b"<!DOCTYPE html><html></html>"),
            None,
            "不是图片"
        );
    }

    #[test]
    fn catalog_icon_url_should_reject_non_public_or_non_https() {
        assert!(catalog_icon_url_is_public_https(
            "https://github.com/obra.png?size=96"
        ));
        assert!(
            !catalog_icon_url_is_public_https("http://github.com/obra.png"),
            "只允许 https"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://user:pass@github.com/a.png"),
            "拒绝带账号密码"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://github.com:8443/a.png"),
            "拒绝非 443 端口"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://127.0.0.1/a.png"),
            "拒绝回环地址"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://10.0.0.5/a.png"),
            "拒绝内网地址"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://localhost/a.png"),
            "拒绝本机名"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://foo.internal/a.png"),
            "拒绝内网域名"
        );
    }

    #[test]
    fn catalog_icon_fetch_should_refuse_address_outside_allowlist() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let state = catalog_test_state("catalog-icon-allowlist");
        let address = "https://github.com/obra-unlisted.png?size=96";

        // 没登记过的地址：格式再合法也不能取，而且不报错（前端据此退首字母）。
        let denied = rt.block_on(catalog_icon_data_url_inner(&state, address));
        assert_eq!(denied.expect("不该报错"), "", "未登记地址应返回空");
        assert!(!catalog_icon_address_is_allowed(address));

        // 登记之后放行判定通过。
        let mut entry = skill_entry("icon-probe");
        entry.icon = address.to_string();
        catalog_remember_icon_addresses(&[entry]);
        assert!(catalog_icon_address_is_allowed(address));

        let _ = fs::remove_dir_all(
            state
                .llm_workspace_path
                .parent()
                .unwrap_or(&state.llm_workspace_path),
        );
    }

    #[test]
    fn skills_sh_frontmatter_name_should_read_yaml_name() {
        let content = "---\nname: vercel-react-best-practices\ndescription: 何时使用\n---\n\n# 正文\n";
        assert_eq!(
            skills_sh_frontmatter_name(content).as_deref(),
            Some("vercel-react-best-practices")
        );
        assert_eq!(skills_sh_frontmatter_name("# 没有 frontmatter"), None);
        assert_eq!(skills_sh_frontmatter_name("---\ndescription: 没名字\n---\n"), None);
    }

    #[test]
    fn skills_sh_should_locate_skill_by_frontmatter_name_when_dir_differs() {
        // 热门榜第一页的真实形状：skillId 是 frontmatter 的 name，目录名不一样。
        let paths = vec![
            "README.md".to_string(),
            "skills/react-best-practices/SKILL.md".to_string(),
            "skills/react-best-practices/rules/hooks.md".to_string(),
            "skills/other/SKILL.md".to_string(),
        ];
        // 快路径按目录名拼前缀，这里必然落空：这正是要补慢路径的原因。
        assert!(skills_sh_pick_skill_paths(&paths, "vercel-react-best-practices").is_err());

        let candidates = vec![
            (
                "skills/other/SKILL.md".to_string(),
                Some("other-skill".to_string()),
            ),
            (
                "skills/react-best-practices/SKILL.md".to_string(),
                Some("vercel-react-best-practices".to_string()),
            ),
        ];
        let hit = skills_sh_select_by_name(&candidates, "vercel-react-best-practices")
            .expect("按 frontmatter name 命中");
        assert_eq!(hit, "skills/react-best-practices/SKILL.md");

        let (dir_prefix, siblings) = skills_sh_sibling_paths(&paths, &hit);
        assert_eq!(dir_prefix, "skills/react-best-practices/");
        assert_eq!(
            siblings,
            vec!["skills/react-best-practices/rules/hooks.md".to_string()],
            "兄弟文件按 SKILL.md 所在目录取，不碰别的技能"
        );
    }

    #[test]
    fn skills_sh_relative_path_should_put_skill_md_at_skill_dir_root() {
        // 命中的 SKILL.md 在子目录时，落点必须是 {dir}/SKILL.md，
        // 不能变成 {dir}/skills/xxx/SKILL.md（那样 parse_skill_file 会读不到）。
        assert_eq!(
            skills_sh_relative_path(
                "skills/react-best-practices/SKILL.md",
                "skills/react-best-practices/"
            ),
            "SKILL.md"
        );
        assert_eq!(
            skills_sh_relative_path(
                "skills/react-best-practices/rules/hooks.md",
                "skills/react-best-practices/"
            ),
            "rules/hooks.md"
        );
    }

    #[test]
    fn skills_sh_sibling_paths_should_scope_to_root_when_skill_md_at_repo_root() {
        let paths = vec![
            "SKILL.md".to_string(),
            "README.md".to_string(),
            "notes/todo.md".to_string(),
            "logo.png".to_string(),
        ];
        let (dir_prefix, siblings) = skills_sh_sibling_paths(&paths, "SKILL.md");
        assert_eq!(dir_prefix, "", "仓库根 SKILL.md 的目录前缀是空串");
        assert_eq!(
            siblings,
            vec!["README.md".to_string()],
            "只取仓库根下的文本文件，不能把整棵树都拖下来"
        );
        assert_eq!(
            skills_sh_relative_path("SKILL.md", &dir_prefix),
            "SKILL.md",
            "仓库根的 SKILL.md 落点仍是技能目录根下的 SKILL.md"
        );
    }

    #[test]
    fn skills_sh_short_queries_should_share_one_cache_key() {
        // 短查询共用首页那一份缓存：键必须归一，否则每敲一个字就重拉一次 934KB 首页。
        assert_eq!(catalog_cache_query(CATALOG_SOURCE_SKILLS_SH, ""), "");
        assert_eq!(catalog_cache_query(CATALOG_SOURCE_SKILLS_SH, "r"), "");
        assert_eq!(
            catalog_cache_file_name(
                CATALOG_SOURCE_SKILLS_SH,
                &catalog_cache_query(CATALOG_SOURCE_SKILLS_SH, "r"),
                1,
                30
            ),
            catalog_cache_file_name(
                CATALOG_SOURCE_SKILLS_SH,
                &catalog_cache_query(CATALOG_SOURCE_SKILLS_SH, ""),
                1,
                30
            ),
            "空查询与单字查询要落到同一个缓存文件"
        );
        // 两字符以上走检索接口，各自一份缓存，不能被归一掉。
        assert_eq!(catalog_cache_query(CATALOG_SOURCE_SKILLS_SH, "re"), "re");
        // 别的来源行为不动。
        assert_eq!(
            catalog_cache_query(CATALOG_SOURCE_MODELSCOPE_SKILL, "r"),
            "r"
        );
    }

    #[test]
    fn official_registry_should_keep_only_latest_active_versions() {
        let item = |is_latest: bool, status: &str| {
            serde_json::json!({
                "server": {"name": "ac.inference.sh/mcp"},
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "status": status,
                        "isLatest": is_latest
                    }
                }
            })
        };
        assert!(
            !official_registry_item_is_latest_active(&item(false, "active")),
            "非最新版本要滤掉，否则同一个 MCP 一页出现两三次"
        );
        assert!(
            !official_registry_item_is_latest_active(&item(true, "deprecated")),
            "非 active 状态一起滤掉"
        );
        assert!(official_registry_item_is_latest_active(&item(true, "active")));
        // 接口换形态、不带 _meta 时放行，避免整页被滤空。
        assert!(official_registry_item_is_latest_active(
            &serde_json::json!({"server": {"name": "x/y"}})
        ));
    }

    #[test]
    fn catalog_icon_redirect_targets_should_stay_public_https() {
        // GitHub 头像 302 到 avatars.githubusercontent.com，这一跳必须放行，否则头像全拿不到。
        assert!(catalog_icon_url_is_public_https(
            "https://avatars.githubusercontent.com/u/14985020?s=96&v=4"
        ));
        // 跳转目标按同一套规则校验：链路本地地址与内网不能靠一串 302 绕过。
        assert!(
            !catalog_icon_url_is_public_https("http://169.254.169.254/latest/meta-data/"),
            "链路本地地址要拒"
        );
        assert!(
            !catalog_icon_url_is_public_https("https://169.254.169.254/a.png"),
            "链路本地地址换 https 也要拒"
        );
    }
}
