use jieba_rs::Jieba;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::QueryParser;
use tantivy::schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED};
use tantivy::tokenizer::{SimpleTokenizer, TextAnalyzer};
use tantivy::{doc, Index};

fn tokenize_cn(jieba: &Jieba, text: &str) -> String {
    jieba
        .cut(text, false)
        .into_iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_case(case_name: &str, docs: &[&str], queries: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let mut jieba = Jieba::new();
    jieba.add_word("遥酱", None, None);

    let mut schema_builder = Schema::builder();
    let indexing = TextFieldIndexing::default()
        .set_tokenizer("zh_ws")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(indexing)
        .set_stored();
    let content = schema_builder.add_text_field("content", text_options);
    let raw = schema_builder.add_text_field("raw", STORED);
    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema);
    index.tokenizers().register("zh_ws", TextAnalyzer::from(SimpleTokenizer::default()));
    let mut writer = index.writer(20_000_000)?;

    for d in docs {
        let toks = tokenize_cn(&jieba, d);
        writer.add_document(doc!(content => toks, raw => d.to_string()))?;
    }
    writer.commit()?;

    let reader = index.reader()?;
    let searcher = reader.searcher();
    println!("\n=== CASE: {case_name} | docs={} ===", docs.len());
    let qp = QueryParser::for_index(&index, vec![content]);
    for query_text in queries {
        let query_tokens = tokenize_cn(&jieba, query_text);
        let query = qp.parse_query(&query_tokens)?;
        let hit_count = searcher.search(&query, &Count)?;
        let hits = searcher.search(&query, &TopDocs::with_limit(3))?;
        println!("query_raw: {query_text}");
        println!("query_tokens: {query_tokens}");
        println!("hit_count: {hit_count}");
        for (rank, (score, addr)) in hits.iter().enumerate() {
            let d: tantivy::schema::TantivyDocument = searcher.doc(*addr)?;
            let raw_text = d
                .get_first(raw)
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "<missing>".to_string());
            println!("#{} score={:.6} text={}", rank + 1, score, raw_text);
        }
        if let Some((score, _)) = hits.first() {
            println!("top1_score: {:.6}\n", score);
        } else {
            println!("top1_score: <none>\n");
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queries = vec![
        "火锅底料 配方 调味",      // 无关
        "前端 组件 开发",          // 有点关系
        "遥酱擅长前端开发，能独立实现复杂组件功能", // 100%吻合
    ];

    let docs_small = vec![
        "遥酱擅长前端开发，能独立实现复杂组件功能",
        "前端工程实践与组件抽象能力很强",
        "今天讨论的是数据库迁移方案",
        "遥酱擅长前端开发，尤其是复杂组件",
    ];
    run_case("small", &docs_small, &queries)?;

    let mut docs_large = docs_small.clone();
    docs_large.extend(vec![
        "后端服务采用Rust实现，强调稳定性",
        "今天讨论的是数据库迁移方案与回滚策略",
        "前端开发需要关注性能和可维护性",
        "组件化设计让复杂页面更易维护",
        "团队在研究向量检索和重排策略",
        "遥酱在前端工程中偏好可测试架构",
        "遥酱擅长前端开发，能独立实现复杂组件功能",
        "前端组件复用率提升后交付效率更高",
        "数据库索引优化对检索延迟影响明显",
        "工程实践中日志可观测性非常重要",
        "这里是一条完全无关的句子",
        "又一条无关内容用于增加语料规模",
    ]);
    run_case("large", &docs_large, &queries)?;
    Ok(())
}
