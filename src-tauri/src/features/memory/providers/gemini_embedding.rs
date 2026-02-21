#[derive(Debug, Clone)]
struct GeminiEmbeddingProvider {
    base_url: String,
    api_key: String,
    model: String,
}

impl GeminiEmbeddingProvider {
    fn endpoint_url(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        let lower = base.to_ascii_lowercase();
        if lower.contains(":batchembedcontents") {
            base.to_string()
        } else if lower.contains("/v1beta") || lower.contains("/v1/") {
            memory_join_url(base, &format!("models/{}:batchEmbedContents", self.model))
        } else {
            memory_join_url(base, &format!("v1beta/models/{}:batchEmbedContents", self.model))
        }
    }
}

impl MemoryEmbeddingProvider for GeminiEmbeddingProvider {
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if self.base_url.trim().is_empty() {
            return Err("Gemini embedding base URL is empty.".to_string());
        }
        if self.api_key.trim().is_empty() {
            return Err("Gemini embedding API key is empty.".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("Gemini embedding model is empty.".to_string());
        }

        let requests = texts
            .iter()
            .map(|text| {
                serde_json::json!({
                    "model": format!("models/{}", self.model),
                    "content": { "parts": [{ "text": text }] }
                })
            })
            .collect::<Vec<_>>();
        let payload = serde_json::json!({ "requests": requests });
        let url = self.endpoint_url();
        let api_key = self.api_key.trim().to_string();

        memory_run_async(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(|err| format!("Build Gemini embedding HTTP client failed: {err}"))?;
            let resp = client
                .post(&url)
                .query(&[("key", api_key.clone())])
                .header(CONTENT_TYPE, "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|err| format!("Gemini embedding request failed ({url}): {err}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!(
                    "Gemini embedding failed ({url}): {status} | {}",
                    body.chars().take(300).collect::<String>()
                ));
            }
            let body = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|err| format!("Parse Gemini embedding response failed ({url}): {err}"))?;
            let rows = body
                .get("embeddings")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| "Gemini embedding response missing embeddings".to_string())?;
            let mut vectors = Vec::<Vec<f32>>::with_capacity(rows.len());
            for row in rows {
                let values = row
                    .get("values")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| "Gemini embedding row missing values".to_string())?;
                let mut vector = Vec::<f32>::with_capacity(values.len());
                for v in values {
                    let as_f64 = v
                        .as_f64()
                        .ok_or_else(|| "Gemini embedding element is not number".to_string())?;
                    vector.push(as_f64 as f32);
                }
                vectors.push(vector);
            }
            Ok(vectors)
        })
    }
}

