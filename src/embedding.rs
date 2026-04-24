//! Local embedding generation for semantic code search.
//!
//! Supports Ollama (default) and OpenAI-compatible embedding APIs.
//! Embeddings are stored as f32 vectors in SQLite `code_embeddings` table.

use serde::Deserialize;
use tracing::warn;

/// Generate embeddings for a batch of texts via Ollama HTTP API.
///
/// Returns a Vec of (text_index, embedding_vector) pairs.
/// If the API is unavailable or disabled, returns an empty Vec.
pub async fn generate_embeddings_ollama(
    texts: &[String],
    model: &str,
    base_url: &str,
    timeout_secs: u64,
) -> Vec<Vec<f32>> {
    if texts.is_empty() {
        return Vec::new();
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to build HTTP client for embedding: {}", e);
            return Vec::new();
        }
    };

    let url = format!("{}/api/embeddings", base_url.trim_end_matches('/'));
    let mut results = Vec::with_capacity(texts.len());

    for text in texts {
        let body = serde_json::json!({
            "model": model,
            "prompt": text,
        });

        match client.post(&url).json(&body).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!("Ollama embedding API returned {}: {}", resp.status(), text);
                    continue;
                }
                match resp.json::<OllamaEmbeddingResponse>().await {
                    Ok(data) => {
                        results.push(data.embedding);
                    }
                    Err(e) => {
                        warn!("Failed to parse Ollama embedding response: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Ollama embedding request failed: {}", e);
                continue;
            }
        }
    }

    results
}

/// Generate embeddings for a batch of texts via OpenAI-compatible API.
pub async fn generate_embeddings_openai(
    texts: &[String],
    model: &str,
    api_key: &str,
    base_url: &str,
    timeout_secs: u64,
) -> Vec<Vec<f32>> {
    if texts.is_empty() {
        return Vec::new();
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to build HTTP client for embedding: {}", e);
            return Vec::new();
        }
    };

    let url = format!("{}/embeddings", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "input": texts,
    });

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                warn!("OpenAI embedding API returned {}", resp.status());
                return Vec::new();
            }
            match resp.json::<OpenAIEmbeddingResponse>().await {
                Ok(data) => data.data.into_iter().map(|d| d.embedding).collect(),
                Err(e) => {
                    warn!("Failed to parse OpenAI embedding response: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            warn!("OpenAI embedding request failed: {}", e);
            Vec::new()
        }
    }
}

/// Convenience wrapper: generate embeddings using the configured provider.
pub async fn generate_embeddings(
    texts: &[String],
    config: &crate::config::EmbeddingConfig,
) -> Vec<Vec<f32>> {
    if !config.enabled {
        return Vec::new();
    }

    match config.provider.as_str() {
        "ollama" => {
            generate_embeddings_ollama(
                texts,
                &config.model,
                &config.base_url,
                config.timeout_seconds,
            )
            .await
        }
        "openai" => {
            // For OpenAI we need an API key; fallback to no embeddings if missing
            warn!("OpenAI embedding provider requires API key in config; skipping");
            Vec::new()
        }
        _ => {
            warn!("Unknown embedding provider: {}; skipping", config.provider);
            Vec::new()
        }
    }
}

/// Cosine similarity between two f32 vectors.
/// Returns a value in [-1.0, 1.0]. Higher = more similar.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Serialize an f32 vector to bytes for SQLite BLOB storage.
pub fn embedding_to_bytes(emb: &[f32]) -> Vec<u8> {
    emb.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize bytes from SQLite BLOB back to f32 vector.
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ------------------------------------------------------------------
// API response types
// ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_embedding_roundtrip() {
        let emb = vec![1.5, -2.25, 3.0, 0.0];
        let bytes = embedding_to_bytes(&emb);
        let recovered = bytes_to_embedding(&bytes);
        assert_eq!(emb, recovered);
    }
}
