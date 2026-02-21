#[derive(Debug, Clone)]
struct OpenAIEmbeddingProvider {
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAIEmbeddingProvider {
    fn endpoint_url(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        let lower = base.to_ascii_lowercase();
        if lower.ends_with("/embeddings") {
            base.to_string()
        } else if lower.ends_with("/v1") {
            memory_join_url(base, "embeddings")
        } else {
            memory_join_url(base, "v1/embeddings")
        }
    }
}

impl MemoryEmbeddingProvider for OpenAIEmbeddingProvider {
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if self.base_url.trim().is_empty() {
            return Err("OpenAI embedding base URL is empty.".to_string());
        }
        if self.api_key.trim().is_empty() {
            return Err("OpenAI embedding API key is empty.".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("OpenAI embedding model is empty.".to_string());
        }

        let input_value = if texts.len() == 1 {
            serde_json::Value::String(texts[0].clone())
        } else {
            serde_json::json!(texts)
        };
        let payload = serde_json::json!({
            "model": self.model,
            "input": input_value,
        });
        let url = self.endpoint_url();
        let api_key = self.api_key.trim().to_string();

        memory_run_async(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|err| format!("Build OpenAI embedding HTTP client failed: {err}"))?;
            let resp = client
                .post(&url)
                .header(AUTHORIZATION, format!("Bearer {api_key}"))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|err| format!("OpenAI embedding request failed ({url}): {err}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!(
                    "OpenAI embedding failed ({url}): {status} | {}",
                    body.chars().take(300).collect::<String>()
                ));
            }
            let body = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|err| format!("Parse OpenAI embedding response failed ({url}): {err}"))?;
            let data = body
                .get("data")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| "OpenAI embedding response missing data array".to_string())?;
            let mut vectors = Vec::<Vec<f32>>::with_capacity(data.len());
            for row in data {
                let emb = row
                    .get("embedding")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| "OpenAI embedding row missing embedding".to_string())?;
                let mut vector = Vec::<f32>::with_capacity(emb.len());
                for v in emb {
                    let as_f64 = v
                        .as_f64()
                        .ok_or_else(|| "OpenAI embedding element is not number".to_string())?;
                    vector.push(as_f64 as f32);
                }
                vectors.push(vector);
            }
            Ok(vectors)
        })
    }
}
