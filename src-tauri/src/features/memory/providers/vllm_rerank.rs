#[allow(dead_code)]
#[derive(Debug, Clone)]
struct VllmRerankProvider {
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl VllmRerankProvider {
    #[allow(dead_code)]
    fn endpoint_url(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        let lower = base.to_ascii_lowercase();
        if lower.ends_with("/v1/rerank") || lower.ends_with("/rerank") {
            base.to_string()
        } else if lower.ends_with("/v1") {
            memory_join_url(base, "rerank")
        } else {
            memory_join_url(base, "v1/rerank")
        }
    }
}

impl MemoryRerankProvider for VllmRerankProvider {
    fn rerank(
        &self,
        query: &str,
        documents: &[String],
        top_n: Option<usize>,
    ) -> Result<Vec<MemoryRerankItem>, String> {
        if self.base_url.trim().is_empty() {
            return Err("vLLM rerank base URL is empty.".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("vLLM rerank model is empty.".to_string());
        }
        if query.trim().is_empty() {
            return Err("vLLM rerank query is empty.".to_string());
        }
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let url = self.endpoint_url();
        let payload = serde_json::json!({
            "query": query,
            "documents": documents,
            "model": self.model,
            "top_n": top_n.unwrap_or(documents.len()),
        });
        let auth = self.api_key.clone();

        memory_run_async(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|err| format!("Build vLLM rerank HTTP client failed: {err}"))?;
            let mut req = client
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .json(&payload);
            if let Some(token) = auth.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                req = req.header(AUTHORIZATION, format!("Bearer {token}"));
            }
            let resp = req
                .send()
                .await
                .map_err(|err| format!("vLLM rerank request failed ({url}): {err}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!(
                    "vLLM rerank failed ({url}): {status} | {}",
                    body.chars().take(300).collect::<String>()
                ));
            }
            let body = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|err| format!("Parse vLLM rerank response failed ({url}): {err}"))?;

            let rows = body
                .get("data")
                .and_then(serde_json::Value::as_array)
                .or_else(|| body.get("results").and_then(serde_json::Value::as_array))
                .ok_or_else(|| "vLLM rerank response missing data/results".to_string())?;
            let mut out = Vec::<MemoryRerankItem>::new();
            for row in rows {
                let index = row
                    .get("index")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| "vLLM rerank row missing index".to_string())?;
                let score = row
                    .get("relevance_score")
                    .or_else(|| row.get("score"))
                    .and_then(serde_json::Value::as_f64)
                    .ok_or_else(|| "vLLM rerank row missing relevance_score".to_string())?;
                out.push(MemoryRerankItem {
                    index: index as usize,
                    relevance_score: score,
                });
            }
            Ok(out)
        })
    }
}
