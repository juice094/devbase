// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//! Embedding storage protocol and similarity utilities.
//!
//! Devbase handles storage (SQLite BLOB), serialization, query-time
//! similarity computation, and local query embedding generation via
//! candle (all-MiniLM-L6-v2, pure Rust).
//!
//! ## Provider architecture
//! `EmbeddingProvider` trait abstracts the generation backend.
//! Current: `CandleProvider` (pure-Rust local inference).

pub mod candle;
pub mod ollama;

/// Provider trait for text-to-embedding generation.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for a single query string.
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// Generate embeddings for a batch of texts.
    /// Default implementation falls back to sequential single encoding.
    fn encode_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.encode(t)).collect()
    }

    /// Provider name for diagnostics.
    fn name(&self) -> &'static str;
}

/// Production provider selector.
/// Returns the best available provider at runtime.
pub fn default_provider() -> Box<dyn EmbeddingProvider> {
    Box::new(candle::CandleProvider)
}

/// Create a provider from configuration parameters.
/// `backend`: "candle" | "ollama"
/// `model`: model name (for Ollama, e.g. "all-minilm")
/// `base_url`: Ollama base URL (e.g. "http://localhost:11434")
/// `timeout_seconds`: HTTP timeout for Ollama
pub fn create_provider(
    backend: &str,
    _model: &str,
    base_url: &str,
    timeout_seconds: u64,
) -> Box<dyn EmbeddingProvider> {
    match backend {
        "ollama" => Box::new(ollama::OllamaProvider::new(base_url, _model, timeout_seconds)),
        _ => Box::new(candle::CandleProvider),
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

    #[test]
    fn test_default_provider_routes_correctly() {
        let provider = default_provider();
        assert_eq!(provider.name(), "candle-all-MiniLM-L6-v2");
    }

    #[test]
    fn test_candle_provider_encode() {
        let provider = candle::CandleProvider;
        let emb = provider.encode("hello world").unwrap();
        assert_eq!(emb.len(), 384);
        // L2 norm should be ≈ 1.0 (sentence-transformers normalizes)
        let norm: f32 = emb.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "L2 norm = {}", norm);
    }

    #[test]
    fn test_provider_trait_consistency() {
        let provider = default_provider();
        let emb = provider.encode("hello world").unwrap();
        assert!(!emb.is_empty());
        // all-MiniLM-L6-v2 produces 384-dim vectors
        assert_eq!(emb.len(), 384);
    }

    /// Run Python sentence-transformers and compare against candle output.
    /// Ignored by default because it requires a Python environment with
    /// `sentence-transformers` installed.
    #[test]
    #[ignore = "requires Python with sentence-transformers installed"]
    fn test_candle_python_cosine_similarity() {
        let text = "The quick brown fox jumps over the lazy dog.";

        let candle_emb = candle::CandleProvider.encode(text).unwrap();
        let python_emb = generate_python_embedding(text).unwrap();

        assert_eq!(
            candle_emb.len(),
            python_emb.len(),
            "dimension mismatch: candle={}, python={}",
            candle_emb.len(),
            python_emb.len()
        );

        let sim = cosine_similarity(&candle_emb, &python_emb);
        assert!(sim > 0.999, "candle vs python cosine similarity = {} (expected > 0.999)", sim);
    }

    /// Helper: call Python sentence-transformers for a single text.
    fn generate_python_embedding(text: &str) -> anyhow::Result<Vec<f32>> {
        let candidates: Vec<std::path::PathBuf> = ["python", "python3"]
            .iter()
            .map(std::path::PathBuf::from)
            .filter(|p| {
                std::process::Command::new(p)
                    .arg("-c")
                    .arg("import sentence_transformers")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            })
            .collect();

        let script = format!(
            r#"import os; os.environ['HF_HUB_OFFLINE']='1'; from sentence_transformers import SentenceTransformer; import struct; model = SentenceTransformer('all-MiniLM-L6-v2'); emb = model.encode('{}', convert_to_numpy=True); print(''.join(struct.pack('<f', float(x)).hex() for x in emb.tolist()))"#,
            text.replace('\\', "\\\\").replace('\'', "\\'")
        );

        let mut last_err = String::new();
        for python in &candidates {
            let output = std::process::Command::new(python).args(["-c", &script]).output();
            match output {
                Ok(out) if out.status.success() => {
                    let hex_str = String::from_utf8(out.stdout)?.trim().to_string();
                    let mut embedding = Vec::new();
                    for chunk in hex_str.as_bytes().chunks_exact(8) {
                        let chunk_str = std::str::from_utf8(chunk)?;
                        let bytes = u32::from_str_radix(chunk_str, 16)?;
                        embedding.push(f32::from_le_bytes(bytes.to_le_bytes()));
                    }
                    return Ok(embedding);
                }
                Ok(out) => {
                    last_err = format!(
                        "{} failed: {}",
                        python.display(),
                        String::from_utf8_lossy(&out.stderr)
                    )
                }
                Err(e) => last_err = format!("{} error: {}", python.display(), e),
            }
        }
        Err(anyhow::anyhow!(
            "Python sentence-transformers not available (tried {} candidates). Last: {}",
            candidates.len(),
            last_err
        ))
    }

    #[test]
    fn test_create_provider_candle() {
        let provider = create_provider("candle", "", "", 0);
        assert_eq!(provider.name(), "candle-all-MiniLM-L6-v2");
    }

    #[test]
    fn test_create_provider_ollama() {
        let provider = create_provider("ollama", "all-minilm", "http://localhost:11434", 30);
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_create_provider_unknown_defaults_to_candle() {
        let provider = create_provider("unknown", "", "", 0);
        assert_eq!(provider.name(), "candle-all-MiniLM-L6-v2");
    }
}
