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
    fn github_url_mapping_should_reject_non_github_and_blob() {
        assert!(
            github_tree_url_to_skill_raw("https://gitlab.com/owner/repo/tree/main/demo").is_none()
        );
        assert!(github_tree_url_to_skill_raw("https://github.com/owner/repo/blob/main/x").is_none());
    }

    #[test]
    fn skill_source_installable_should_follow_the_mapping() {
        // GitHub 目录地址：可映射为目录内 SKILL.md。
        assert!(skill_source_is_installable(
            "https://github.com/owner/repo/tree/main/skills/demo"
        ));
        // 直接指向文件。
        assert!(skill_source_is_installable(
            "https://raw.githubusercontent.com/owner/repo/main/SKILL.md"
        ));
        assert!(skill_source_is_installable("https://example.com/demo/SKILL.md"));
        // 仓库根地址：根目录可能有 SKILL.md（AMap-Web/amap-lbs-skill 实测 200），按可装放行。
        assert!(skill_source_is_installable(
            "https://github.com/AMap-Web/amap-lbs-skill"
        ));
        // 站点页面仍然装不了：抓下来是 HTML。
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
    fn skills_sh_js_string_unescape_should_restore_exactly_one_layer() {
        assert_eq!(
            skills_sh_unescape_js_string(r#"{\"a\":\"b\"}"#),
            r#"{"a":"b"}"#
        );
        // 字面反斜杠是 `\\`，还原成一个；不能把紧随其后的引号一起吃掉。
        assert_eq!(skills_sh_unescape_js_string(r#"C:\\path\""#), "C:\\path\"");
        // 不属于这一层的转义（\n、\u）原样留给 JSON 解析器。
        assert_eq!(skills_sh_unescape_js_string(r#"a\nb"#), r#"a\nb"#);
        assert_eq!(skills_sh_unescape_js_string(r#"\u4e2d"#), r#"\u4e2d"#);
    }

    #[test]
    fn skills_sh_trending_should_read_escaped_array_and_drop_dirty_rows() {
        // 页面把列表嵌在 JS 字符串里：键名前后的引号带一层反斜杠转义。
        // 脏行取自实测：`uizze.sh`、`open.feishu.cn` 是站点把自己也列进去了（600 条里 31 条）。
        let escaped = r#"initialSkills\":[{\"source\":\"obra/superpowers\",\"skillId\":\"using-git-worktrees\",\"name\":\"Using Git Worktrees\",\"installs\":1234},{\"source\":\"uizze.sh\",\"skillId\":\"junk\",\"name\":\"Junk\",\"installs\":9},{\"source\":\"open.feishu.cn\",\"skillId\":\"junk2\",\"name\":\"Junk2\",\"installs\":8},{\"source\":\"a/b/c\",\"skillId\":\"junk3\",\"name\":\"Junk3\",\"installs\":7}]"#;
        let entries = skills_sh_trending_entries(escaped);
        assert_eq!(
            entries.len(),
            1,
            "不是 owner/repo 两段形态的条目要丢掉"
        );
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
    fn skills_sh_pick_paths_should_not_match_a_partial_directory_name() {
        // `ends_with` 是纯字节后缀比较：少了前导斜杠，`commit` 会命中 `pre-commit`，
        // 读到的是别人的正文、安装目录却叫 commit，全程不报错。
        let paths = vec![
            "skills/pre-commit/SKILL.md".to_string(),
            "skills/pre-commit/reference.md".to_string(),
        ];
        assert!(
            skills_sh_pick_skill_paths(&paths, "commit").is_err(),
            "commit 不能命中 pre-commit"
        );
        // 完整目录名仍然要命中，取到的是 pre-commit 自己的文件。
        let (skill_md, dir_prefix, siblings) =
            skills_sh_pick_skill_paths(&paths, "pre-commit").expect("完整目录名应命中");
        assert_eq!(skill_md, "skills/pre-commit/SKILL.md");
        assert_eq!(dir_prefix, "skills/pre-commit/");
        assert_eq!(siblings, vec!["skills/pre-commit/reference.md".to_string()]);
    }

    #[test]
    fn skills_sh_pick_paths_should_support_repo_root_form() {
        // 仓库根下的 `{skillId}/SKILL.md` 没有前导斜杠可依，必须靠整串相等命中。
        let paths = vec![
            "README.md".to_string(),
            "using-git-worktrees/SKILL.md".to_string(),
        ];
        let (skill_md, dir_prefix, _) =
            skills_sh_pick_skill_paths(&paths, "using-git-worktrees").expect("根目录形态应命中");
        assert_eq!(skill_md, "using-git-worktrees/SKILL.md");
        assert_eq!(dir_prefix, "using-git-worktrees/");
    }

    #[test]
    fn skills_sh_entry_should_drop_skill_id_with_slash() {
        // id 由三段拼成，skillId 含斜杠会变四段，安装时反解出错的仓库。
        let dirty = serde_json::json!({
            "source": "obra/superpowers",
            "skillId": "skills/using-git-worktrees",
            "name": "using-git-worktrees",
        });
        assert!(skills_sh_entry(&dirty).is_none(), "含斜杠的 skillId 直接丢掉");
        let clean = serde_json::json!({
            "source": "obra/superpowers",
            "skillId": "using-git-worktrees",
            "name": "using-git-worktrees",
        });
        let entry = skills_sh_entry(&clean).expect("正常条目应保留");
        assert_eq!(entry.id, "obra/superpowers/using-git-worktrees");
    }

    #[test]
    fn github_url_mapping_should_cover_repo_root_and_use_head() {
        let root = "https://raw.githubusercontent.com/AMap-Web/amap-lbs-skill/HEAD/SKILL.md";
        // 仓库根地址：根目录就有 SKILL.md（AMap-Web/amap-lbs-skill 实测 200）。
        assert_eq!(
            github_tree_url_to_skill_raw("https://github.com/AMap-Web/amap-lbs-skill"),
            Some(root.to_string())
        );
        // 尾斜杠不影响判定。
        assert_eq!(
            github_tree_url_to_skill_raw("https://github.com/AMap-Web/amap-lbs-skill/"),
            Some(root.to_string())
        );
        // tree 地址：字面分支名丢掉，统一拼 HEAD（main / master 都能落到默认分支）。
        assert_eq!(
            github_tree_url_to_skill_raw(
                "https://github.com/obra/superpowers/tree/master/skills/find-skills"
            ),
            Some(
                "https://raw.githubusercontent.com/obra/superpowers/HEAD/skills/find-skills/SKILL.md"
                    .to_string()
            )
        );
        // 深路径同样直接接在 HEAD 后面。
        assert_eq!(
            github_tree_url_to_skill_raw(
                "https://github.com/x/y/tree/main/plugins/skill-creator/skills/skill-creator"
            ),
            Some(
                "https://raw.githubusercontent.com/x/y/HEAD/plugins/skill-creator/skills/skill-creator/SKILL.md"
                    .to_string()
            )
        );
        // 站点页仍判不可装。
        assert_eq!(
            github_tree_url_to_skill_raw("https://modelscope.cn/studios"),
            None
        );
        assert!(!skill_source_is_installable("https://modelscope.cn/studios"));
        assert!(skill_source_is_installable(
            "https://github.com/AMap-Web/amap-lbs-skill"
        ));
    }

    #[test]
    fn skills_sh_scan_should_stop_after_first_batch_hit() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        // 60 个候选、目标落在第一批里：读满一批就该停手，不能把后两批也读掉。
        let paths = (0..60)
            .map(|index| format!("skills/s{index:02}/SKILL.md"))
            .collect::<Vec<_>>();
        let calls = std::cell::Cell::new(0usize);
        let result = rt.block_on(skills_sh_scan_skill_md_by_name(
            &paths,
            "target",
            |path: String| {
                calls.set(calls.get() + 1);
                let name = if path == "skills/s03/SKILL.md" {
                    Some("target".to_string())
                } else {
                    Some(format!("other:{path}"))
                };
                async move { (path, name) }
            },
        ));
        assert_eq!(result.as_deref(), Ok("skills/s03/SKILL.md"));
        assert_eq!(
            calls.get(),
            SKILLS_SH_FRONTMATTER_SCAN_BATCH,
            "第一批命中即停：只发一批请求，后两批不该发"
        );
    }

    #[test]
    fn skills_sh_scan_should_count_exactly_when_hitting_the_cap() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build runtime");
        // 220 个候选全不命中：扫到上限就停，返回的计数必须精确等于上限，
        // 不能因为最后一批被 remaining 截断而少算或多算。
        let paths = (0..220)
            .map(|index| format!("skills/s{index:03}/SKILL.md"))
            .collect::<Vec<_>>();
        let calls = std::cell::Cell::new(0usize);
        let result = rt.block_on(skills_sh_scan_skill_md_by_name(
            &paths,
            "target",
            |path: String| {
                calls.set(calls.get() + 1);
                async move { (path, Some("other".to_string())) }
            },
        ));
        assert_eq!(result, Err(SKILLS_SH_FRONTMATTER_SCAN_MAX));
        assert_eq!(
            calls.get(),
            SKILLS_SH_FRONTMATTER_SCAN_MAX,
            "触顶后不能再发请求"
        );
    }

    #[test]
    fn skills_sh_not_found_message_should_not_claim_absent_when_truncated() {
        // 扫满上限仍未命中：文案要说「已扫描前 N 个（共 M 个）」并点明上限，不能说「没有」。
        let truncated = skills_sh_not_found_message(
            "owner",
            "repo",
            "target",
            SKILLS_SH_FRONTMATTER_SCAN_MAX,
            220,
        );
        assert!(truncated.contains("已扫描前 192"), "{truncated}");
        assert!(truncated.contains("共 220"), "{truncated}");
        assert!(truncated.contains("扫描上限为 192"), "{truncated}");
        assert!(
            !truncated.contains("没有"),
            "后面还有没扫到的，不能说「没有」：{truncated}"
        );
        // 候选全部扫完仍未命中：这才是「确实找不到」，不该带上限提示。
        let complete = skills_sh_not_found_message("owner", "repo", "target", 9, 9);
        assert!(!complete.contains("扫描上限"), "{complete}");
        assert!(complete.contains("已扫描前 9 个"), "{complete}");
    }

    #[test]
    fn catalog_icon_cache_should_live_under_app_temp_not_config_dir() {
        let root = std::env::temp_dir().join(format!(
            "eca-catalog-icon-cache-{}",
            Uuid::new_v4()
        ));
        let config_dir = root.join("config");
        fs::create_dir_all(&config_dir).expect("create config dir");
        let state = AppState {
            config_path: config_dir.join("app_config.toml"),
            data_path: config_dir.join("config_mark"),
            llm_workspace_path: root.join("llm-workspace"),
            ..AppState::new().expect("create test app state")
        };

        let (bin, mime) = catalog_icon_cache_paths(&state, "https://github.com/obra.png?size=96");
        assert!(
            bin.starts_with(root.join("temp")),
            "图标缓存放在 app_root/temp 下：{}",
            bin.display()
        );
        assert!(
            !bin.starts_with(&config_dir),
            "不能写进配置文件所在的那层：{}",
            bin.display()
        );
        assert_eq!(bin.extension().and_then(|value| value.to_str()), Some("bin"));
        assert_eq!(
            mime.extension().and_then(|value| value.to_str()),
            Some("type")
        );
        assert_eq!(bin.file_stem(), mime.file_stem(), "两个文件用同一个地址哈希");

        // 地址不同 → 哈希不同，互不覆盖。
        let (other, _) = catalog_icon_cache_paths(&state, "https://github.com/other.png?size=96");
        assert_ne!(bin, other);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn official_registry_entry_should_match_real_samples() {
        // 实测样例一：自带 icons[0].src，仓库地址能推出 owner。
        let case = serde_json::json!({
            "server": {
                "name": "io.github.upstash/context7",
                "title": "Context7",
                "icons": [{"src": "https://raw.githubusercontent.com/upstash/context7/master/public/icon.png"}],
                "repository": {"url": "https://github.com/upstash/context7"},
                "remotes": [{"url": "https://mcp.context7.com/mcp"}]
            },
            "_meta": {"io.modelcontextprotocol.registry/official": {"status": "active", "isLatest": true}}
        });
        let entry = official_registry_entry(&case);
        assert_eq!(
            entry.icon,
            "https://raw.githubusercontent.com/upstash/context7/master/public/icon.png"
        );
        assert_eq!(entry.author, "upstash");
        assert_eq!(entry.id, "io.github.upstash/context7");
        assert_eq!(entry.transport, "streamable_http");

        // 实测样例二：icons 为 null，但仓库地址能推 owner，用头像而不是空白。
        let case = serde_json::json!({
            "server": {
                "name": "ai.smithery/renCosta2025-context7fork",
                "title": "Context7 Fork",
                "icons": null,
                "repository": {"url": "https://github.com/renCosta2025/context7-fork"}
            },
            "_meta": {"io.modelcontextprotocol.registry/official": {"status": "active", "isLatest": true}}
        });
        let entry = official_registry_entry(&case);
        assert_eq!(entry.icon, "https://github.com/renCosta2025.png?size=96");
        assert_eq!(entry.author, "renCosta2025");

        // 实测 30 条里 13 条既无图标也无 GitHub 归属：只能是空串，前端退首字母。
        // 不给它们编造默认图。
        let case = serde_json::json!({
            "server": {"name": "com.example/mcp", "title": "Example"},
            "_meta": {"io.modelcontextprotocol.registry/official": {"status": "active", "isLatest": true}}
        });
        let entry = official_registry_entry(&case);
        assert_eq!(entry.icon, "");
        assert_eq!(entry.author, "example.com");
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
    fn catalog_icon_allowlist_should_evict_oldest_instead_of_growing_forever() {
        let mut list = CatalogIconAllowlist::default();
        let limit = CATALOG_ICON_ALLOWED_LIMIT;
        for index in 0..(limit + 10) {
            list.remember(&format!("https://example.com/{index}.png"));
        }
        assert!(
            !list.contains("https://example.com/0.png"),
            "超出上限后最旧的地址要被淘汰，不能只增不减"
        );
        assert!(
            list.contains(&format!("https://example.com/{}.png", limit + 9)),
            "最新登记的地址必须还在"
        );
        assert_eq!(list.order.len(), limit, "上限就是上限");
        assert_eq!(list.set.len(), limit, "两个集合不能脱节");

        // 重复登记不占新位置，也不改变淘汰顺序。
        list.remember("https://example.com/10.png");
        assert_eq!(list.order.len(), limit);
        assert!(list.contains("https://example.com/10.png"));

        // 翻回上一页的场景：刚登记过的地址仍可取，不因为翻页被清空。
        list.remember(&format!("https://example.com/{}.png", limit + 9));
        assert!(list.contains(&format!("https://example.com/{}.png", limit + 9)));
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
    fn skills_sh_single_char_query_should_be_rejected_without_request() {
        // 单字查询不发请求也不写缓存：首页 93 万字节，敲一个删一个会反复触发。
        assert!(catalog_skills_sh_query_too_short(CATALOG_SOURCE_SKILLS_SH, "r"));
        assert!(catalog_skills_sh_query_too_short(CATALOG_SOURCE_SKILLS_SH, "记"));
        // 空查询是浏览态，必须放行去拉热门榜。
        assert!(!catalog_skills_sh_query_too_short(
            CATALOG_SOURCE_SKILLS_SH,
            ""
        ));
        // 两字符起走检索接口。
        assert!(!catalog_skills_sh_query_too_short(
            CATALOG_SOURCE_SKILLS_SH,
            "re"
        ));
        // 别的来源行为不动。
        assert!(!catalog_skills_sh_query_too_short(
            CATALOG_SOURCE_MODELSCOPE_SKILL,
            "r"
        ));
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
