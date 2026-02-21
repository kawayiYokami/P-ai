#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryProviderKind {
    OpenAIEmbedding,
    GeminiEmbedding,
    VllmRerank,
    DeterministicLocal,
}

#[derive(Debug, Clone)]
struct MemoryProviderApiConfig {
    base_url: String,
    api_key: String,
    model: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryRerankItem {
    index: usize,
    relevance_score: f64,
}

trait MemoryEmbeddingProvider: Send + Sync {
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

#[allow(dead_code)]
trait MemoryRerankProvider: Send + Sync {
    fn rerank(
        &self,
        query: &str,
        documents: &[String],
        top_n: Option<usize>,
    ) -> Result<Vec<MemoryRerankItem>, String>;
}

fn memory_run_async<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("Create async runtime failed: {err}"))?;
    runtime.block_on(future)
}

fn memory_join_url(base_url: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim().trim_end_matches('/'),
        suffix.trim().trim_start_matches('/'),
    )
}
