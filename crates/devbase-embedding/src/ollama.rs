// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use crate::EmbeddingProvider;

pub struct OllamaProvider {
    base_url: String,
    model: String,
    timeout_seconds: u64,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str, timeout_seconds: u64) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            timeout_seconds,
        }
    }

    fn embed_inner(&self, inputs: Vec<&str>) -> anyhow::Result<Vec<Vec<f32>>> {
        let url = format!("{}/api/embed", self.base_url);
        let body = if inputs.len() == 1 {
            serde_json::json!({
                "model": self.model,
                "input": inputs[0],
            })
        } else {
            serde_json::json!({
                "model": self.model,
                "input": inputs,
            })
        };

        let resp: serde_json::Value = ureq::post(&url)
            .set("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(self.timeout_seconds))
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("Ollama API request failed: {}", e))?
            .into_json()
            .map_err(|e| anyhow::anyhow!("Ollama API JSON parse error: {}", e))?;

        let embeddings = resp
            .get("embeddings")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Ollama response missing embeddings: {}", resp))?;

        let mut results = Vec::with_capacity(embeddings.len());
        for emb in embeddings {
            let vec: Vec<f32> = emb
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid embedding array in Ollama response"))?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            results.push(vec);
        }
        Ok(results)
    }
}

impl EmbeddingProvider for OllamaProvider {
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.embed_inner(vec![text])?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("empty embedding result from Ollama"))
    }

    fn encode_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        self.embed_inner(texts.to_vec())
    }

    fn name(&self) -> &'static str {
        "ollama"
    }
}
