// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
// RE-EXPORT ONLY — 实现已迁移至 devbase-embedding crate.
// 禁止在本文件中添加新代码。

#[cfg(feature = "embedding")]
pub use devbase_embedding::*;

#[cfg(feature = "embedding")]
static CONFIG_PROVIDER: std::sync::OnceLock<Box<dyn EmbeddingProvider>> =
    std::sync::OnceLock::new();

/// Generate a query embedding, respecting the user's embedding backend configuration.
/// Falls back to the default Candle provider if config cannot be loaded.
#[cfg(feature = "embedding")]
pub fn generate_query_embedding(text: &str) -> anyhow::Result<Vec<f32>> {
    let provider = CONFIG_PROVIDER.get_or_init(|| {
        crate::config::Config::load()
            .ok()
            .map(|c| {
                create_provider(
                    &c.embedding.provider,
                    &c.embedding.model,
                    &c.embedding.base_url,
                    c.embedding.timeout_seconds,
                )
            })
            .unwrap_or_else(default_provider)
    });
    provider.encode(text)
}

#[cfg(not(feature = "embedding"))]
pub fn generate_query_embedding(_text: &str) -> anyhow::Result<Vec<f32>> {
    anyhow::bail!("Embedding support is disabled. Enable the 'embedding' feature.")
}

#[cfg(not(feature = "embedding"))]
pub fn embedding_to_bytes(emb: &[f32]) -> Vec<u8> {
    emb.iter().flat_map(|f| f.to_le_bytes()).collect()
}

#[cfg(not(feature = "embedding"))]
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

#[cfg(not(feature = "embedding"))]
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
