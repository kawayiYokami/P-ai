#[cfg(test)]
mod memory_store_tests {
    use super::*;

    fn temp_data_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("easy_call_ai_tests")
            .join(format!("{}_{}", name, Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp dir");
        root.join("app_data.json")
    }

    #[test]
    fn memory_store_crud_and_import_should_work() {
        let data_path = temp_data_path("memory_store_crud");

        let drafts = vec![MemoryDraftInput {
            memory_type: "knowledge".to_string(),
            judgment: "Alice likes rust".to_string(),
            reasoning: "用户偏好".to_string(),
            tags: vec!["alice".to_string(), "rust".to_string()],
            owner_agent_id: None,
        }];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("upsert drafts");
        assert_eq!(saved.len(), 1);
        assert!(saved[0].saved);
        assert_eq!(total, 1);

        let memories = memory_store_list_memories(&data_path).expect("list memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].tags, vec!["alice".to_string(), "rust".to_string()]);

        let stats = memory_store_import_memories(&data_path, &vec![MemoryEntry {
            id: String::new(),
            memory_type: "knowledge".to_string(),
            judgment: "Alice likes rust".to_string(),
            reasoning: "".to_string(),
            tags: vec!["backend".to_string(), "rust".to_string()],
            owner_agent_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
        }])
        .expect("import memories");
        assert_eq!(stats.imported_count, 1);
        assert_eq!(stats.total_count, 1);

        let memories_after = memory_store_list_memories(&data_path).expect("list memories after");
        assert_eq!(memories_after.len(), 1);
        assert!(memories_after[0].tags.contains(&"backend".to_string()));
    }

    #[test]
    fn memory_store_delete_should_remove_record_and_fts() {
        let data_path = temp_data_path("memory_store_delete");
        let drafts = vec![MemoryDraftInput {
            memory_type: "knowledge".to_string(),
            judgment: "删除测试样本".to_string(),
            reasoning: "delete".to_string(),
            tags: vec!["删除".to_string(), "样本".to_string()],
            owner_agent_id: None,
        }];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 1);
        let memory_id = saved[0].id.clone().expect("saved id");

        memory_store_delete_memory(&data_path, &memory_id).expect("delete memory");

        let memories = memory_store_list_memories(&data_path).expect("list memories");
        assert!(memories.is_empty());

        let conn = memory_store_open(&data_path).expect("open conn");
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM memory_fts WHERE item_id=?1",
                params![memory_id],
                |row| row.get(0),
            )
            .expect("count memory fts");
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn provider_sync_should_rebuild_and_switch() {
        let data_path = temp_data_path("provider_sync");
        let _ = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Use sqlite for truth source".to_string(),
                reasoning: "".to_string(),
                tags: vec!["sqlite".to_string()],
                owner_agent_id: None,
            }],
        )
        .expect("seed memory");

        let report = memory_store_sync_provider_index(
            &data_path,
            "openai_text_embedding_3_large",
            "text-embedding-3-large",
            16,
            |texts| {
                Ok(texts
                    .iter()
                    .map(|text| vec![text.len() as f32, 1.0, 2.0])
                    .collect::<Vec<_>>())
            },
        )
        .expect("sync provider");

        assert_eq!(report.status, "synced");
        assert_eq!(report.new_provider_id, "openai_text_embedding_3_large");
        assert_eq!(report.added, 1);

        let conn = memory_store_open(&data_path).expect("open conn");
        let active = memory_store_get_runtime_state(&conn, KB_STATE_ACTIVE_INDEX_PROVIDER_ID)
            .expect("runtime state")
            .expect("active provider");
        assert_eq!(active, "openai_text_embedding_3_large");
    }

    #[test]
    fn provider_sync_should_support_noop_and_delete_diff() {
        let data_path = temp_data_path("provider_sync_diff");
        let (saved, _) = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "Document A".to_string(),
                reasoning: "".to_string(),
                tags: vec!["a".to_string()],
                owner_agent_id: None,
            }],
        )
        .expect("seed memory");
        let first_id = saved[0].id.clone().expect("saved id");

        let report1 = memory_store_sync_provider_index(
            &data_path,
            "provider_x",
            "model-x",
            8,
            |texts| Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect::<Vec<_>>()),
        )
        .expect("first sync");
        assert_eq!(report1.added, 1);

        let report2 = memory_store_sync_provider_index(
            &data_path,
            "provider_x",
            "model-x",
            8,
            |texts| Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect::<Vec<_>>()),
        )
        .expect("second sync");
        assert_eq!(report2.status, "no_op");

        let conn = memory_store_open(&data_path).expect("open conn");
        conn.execute(
            "DELETE FROM memory_record WHERE id=?1",
            params![first_id],
        )
        .expect("delete memory record");
        conn.execute("DELETE FROM memory_fts", []).expect("clear memory fts");

        let report3 = memory_store_sync_provider_index(
            &data_path,
            "provider_y",
            "model-y",
            8,
            |texts| Ok(texts.iter().map(|_| vec![0.4, 0.5, 0.6]).collect::<Vec<_>>()),
        )
        .expect("third sync");
        assert_eq!(report3.deleted, 0);
        assert_eq!(report3.added, 0);
    }

    #[test]
    fn rebuild_health_backup_restore_should_work() {
        let data_path = temp_data_path("rebuild_health_backup");
        let _ = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "skill".to_string(),
                judgment: "Backup target memory".to_string(),
                reasoning: "".to_string(),
                tags: vec!["backup".to_string()],
                owner_agent_id: None,
            }],
        )
        .expect("seed memory");

        let rebuild = memory_store_rebuild_indexes(&data_path).expect("rebuild indexes");
        assert_eq!(rebuild.memory_rows, rebuild.memory_fts_rows);

        let health = memory_store_health_check(&data_path, false).expect("health check");
        assert_eq!(health.status, "ok");

        let backup_path = data_path
            .parent()
            .expect("parent")
            .join("memory_store_backup.db");
        let backup = memory_store_backup_db(&data_path, &backup_path).expect("backup db");
        assert!(backup.bytes > 0);

        let restore = memory_store_restore_db(&data_path, &backup_path).expect("restore db");
        assert!(restore.bytes > 0);
    }

    #[test]
    fn bm25_search_should_return_ranked_results() {
        let data_path = temp_data_path("bm25_ranked");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "用户偏好深色主题的编辑器风格".to_string(),
                reasoning: "UI偏好".to_string(),
                tags: vec!["风格".to_string(), "编辑器".to_string(), "偏好".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "skill".to_string(),
                judgment: "写代码时风格偏简洁，不喜欢过度抽象".to_string(),
                reasoning: "编码习惯".to_string(),
                tags: vec!["风格".to_string(), "代码".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "今天讨论了项目架构的风格问题".to_string(),
                reasoning: "事件记录".to_string(),
                tags: vec!["架构".to_string(), "风格".to_string()],
                owner_agent_id: None,
            },
        ];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 3);
        assert!(saved.iter().all(|s| s.saved));

        let hits = memory_store_search_fts_bm25(&data_path, "风格", 10).expect("search");
        assert!(!hits.is_empty());
        assert!(hits.len() <= 3);
        assert!(hits.iter().all(|(_, s)| s.is_finite()));
    }

    #[test]
    fn bm25_search_should_produce_non_zero_and_non_binary_scores() {
        let data_path = temp_data_path("bm25_score_independent");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Rust Rust Rust 适合做高性能后端架构".to_string(),
                reasoning: "高频词样本".to_string(),
                tags: vec!["偏好".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Rust 也常用于工具链开发".to_string(),
                reasoning: "低频词样本".to_string(),
                tags: vec!["习惯".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "今天讨论的是数据库迁移方案".to_string(),
                reasoning: "无关样本".to_string(),
                tags: vec!["会议".to_string()],
                owner_agent_id: None,
            },
        ];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 3);
        assert!(saved.iter().all(|s| s.saved));

        let hits = memory_store_search_fts_bm25(&data_path, "Rust", 10).expect("search");
        assert!(
            hits.len() >= 2,
            "expected at least 2 bm25 hits, got {}",
            hits.len()
        );

        let abs_scores = hits.iter().map(|(_, s)| s.abs()).collect::<Vec<_>>();
        assert!(
            abs_scores.iter().all(|s| s.is_finite()),
            "bm25 contains non-finite values: {abs_scores:?}"
        );
        assert!(
            abs_scores.iter().any(|s| *s > 0.0),
            "bm25 all zero: {abs_scores:?}"
        );

        let unique = abs_scores
            .iter()
            .map(|s| format!("{s:.9}"))
            .collect::<std::collections::HashSet<_>>();
        assert!(
            unique.len() >= 2,
            "bm25 appears binary/discrete for this sample: {abs_scores:?}"
        );
    }

    #[test]
    fn bm25_should_hit_chinese_exact_fragment() {
        let data_path = temp_data_path("bm25_chinese_fragment");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "我最喜欢的角色是遥酱，她的语气很温柔。".to_string(),
                reasoning: "".to_string(),
                tags: vec!["遥酱".to_string(), "偏好".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "今天复习了Rust生命周期".to_string(),
                reasoning: "".to_string(),
                tags: vec!["rust".to_string()],
                owner_agent_id: None,
            },
        ];
        let (saved, total) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        assert_eq!(total, 2);
        assert!(saved.iter().all(|s| s.saved));

        let hits = memory_store_search_fts_bm25(&data_path, "遥酱", 10).expect("search");
        assert!(
            !hits.is_empty(),
            "expected bm25 hit for exact chinese fragment, got empty"
        );
        assert!(
            hits.iter().any(|(_, score)| score.abs() > 0.0),
            "expected non-zero bm25 score for chinese fragment, got {hits:?}"
        );
    }

    #[test]
    fn task_memory_type_should_be_rejected() {
        let data_path = temp_data_path("task_type_reject");
        let result = memory_store_upsert_drafts(
            &data_path,
            &vec![MemoryDraftInput {
                memory_type: "task".to_string(),
                judgment: "Do something".to_string(),
                reasoning: "todo".to_string(),
                tags: vec!["todo".to_string()],
                owner_agent_id: None,
            }],
        );
        assert!(result.is_err());
    }

    #[test]
    fn archive_feedback_should_boost_useful_and_penalize_t1_only() {
        let data_path = temp_data_path("archive_feedback");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "A".to_string(),
                reasoning: "".to_string(),
                tags: vec!["a".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "B".to_string(),
                reasoning: "".to_string(),
                tags: vec!["b".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "C".to_string(),
                reasoning: "".to_string(),
                tags: vec!["c".to_string()],
                owner_agent_id: None,
            },
        ];
        let (saved, _) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed");
        let a_id = saved[0].id.clone().expect("id a");
        let b_id = saved[1].id.clone().expect("id b");
        let c_id = saved[2].id.clone().expect("id c");

        let conn = memory_store_open(&data_path).expect("open");
        conn.execute(
            "UPDATE memory_record SET strength=2, useful_score=5.0 WHERE id=?1",
            params![a_id.clone()],
        )
        .expect("set a");
        conn.execute(
            "UPDATE memory_record SET strength=2, useful_score=1.0 WHERE id=?1",
            params![b_id.clone()],
        )
        .expect("set b");
        conn.execute(
            "UPDATE memory_record SET strength=2, useful_score=11.0 WHERE id=?1",
            params![c_id.clone()],
        )
        .expect("set c");

        let report = memory_store_apply_archive_feedback(
            &data_path,
            &vec![a_id.clone(), b_id.clone(), c_id.clone()],
            &vec![b_id.clone()],
        )
        .expect("apply feedback");
        assert_eq!(report.useful_accepted_count, 1);
        assert_eq!(report.penalized_count, 1);

        let conn = memory_store_open(&data_path).expect("open verify");
        let a_strength: i64 = conn
            .query_row(
                "SELECT strength FROM memory_record WHERE id=?1",
                params![a_id],
                |row| row.get(0),
            )
            .expect("read a strength");
        let b_state: (i64, i64, f64) = conn
            .query_row(
                "SELECT strength, useful_count, useful_score FROM memory_record WHERE id=?1",
                params![b_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read b state");
        let c_strength: i64 = conn
            .query_row(
                "SELECT strength FROM memory_record WHERE id=?1",
                params![c_id],
                |row| row.get(0),
            )
            .expect("read c strength");
        assert_eq!(a_strength, 1, "T1 recalled-but-useless should be penalized");
        assert_eq!(b_state.0, 3, "useful memory should gain strength");
        assert_eq!(b_state.1, 1, "useful_count should increase");
        assert!(b_state.2 > 3.0, "useful_score should increase");
        assert_eq!(c_strength, 2, "T2 recalled-but-useless should remain unchanged");
    }
}

