// SPDX-License-Identifier: MIT
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
    Box::new(CandleProvider)
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
        "ollama" => Box::new(OllamaProvider::new(base_url, _model, timeout_seconds)),
        _ => Box::new(CandleProvider),
    }
}

// ---------------------------------------------------------------------------
// OllamaProvider — local HTTP embedding via Ollama /api/embed
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// CandleProvider — pure-Rust local embedding via all-MiniLM-L6-v2
// ---------------------------------------------------------------------------

pub struct CandleProvider;

impl EmbeddingProvider for CandleProvider {
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let (model, tokenizer) = get_candle_resources()?;
        encode_with_candle(model, tokenizer, text)
    }
    fn encode_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let (model, tokenizer) = get_candle_resources()?;
        encode_batch_with_candle(model, tokenizer, texts)
    }
    fn name(&self) -> &'static str {
        "candle-all-MiniLM-L6-v2"
    }
}

fn get_candle_resources()
-> anyhow::Result<&'static (candle_transformers::models::bert::BertModel, tokenizers::Tokenizer)> {
    use std::sync::OnceLock;
    static RESOURCES: OnceLock<
        Result<(candle_transformers::models::bert::BertModel, tokenizers::Tokenizer), String>,
    > = OnceLock::new();
    match RESOURCES.get_or_init(|| init_candle_resources().map_err(|e| e.to_string())) {
        Ok(r) => Ok(r),
        Err(e) => Err(anyhow::anyhow!("CandleProvider init failed: {}", e)),
    }
}

fn init_candle_resources()
-> anyhow::Result<(candle_transformers::models::bert::BertModel, tokenizers::Tokenizer)> {
    use candle_core::Device;
    use candle_nn::VarBuilder;
    use candle_transformers::models::bert::{BertModel, Config};
    use hf_hub::api::sync::Api;
    use tokenizers::Tokenizer;

    let api = Api::new()?;
    let repo = api.model("sentence-transformers/all-MiniLM-L6-v2".to_string());

    let config_path = repo.get("config.json")?;
    let tokenizer_path = repo.get("tokenizer.json")?;
    let weights_path = repo.get("model.safetensors")?;

    let config: Config = serde_json::from_reader(std::fs::File::open(config_path)?)?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow::anyhow!(e))?;

    let device = Device::Cpu;
    let vb = unsafe {
        // SAFETY: model.safetensors is read-only after hf-hub download;
        // no other process modifies it. This is the standard candle loading pattern.
        VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
    };
    let model = BertModel::load(vb, &config)?;

    Ok((model, tokenizer))
}

fn encode_with_candle(
    model: &candle_transformers::models::bert::BertModel,
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
) -> anyhow::Result<Vec<f32>> {
    encode_batch_with_candle(model, tokenizer, &[text])
        .and_then(|mut v| v.pop().ok_or_else(|| anyhow::anyhow!("empty embedding batch")))
}

fn encode_batch_with_candle(
    model: &candle_transformers::models::bert::BertModel,
    tokenizer: &tokenizers::Tokenizer,
    texts: &[&str],
) -> anyhow::Result<Vec<Vec<f32>>> {
    use candle_core::Tensor;
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    // Batch tokenize
    let encodings = tokenizer.encode_batch(texts.to_vec(), true).map_err(|e| anyhow::anyhow!(e))?;

    // Find max length for padding
    let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

    // Build padded batch tensors
    let mut input_ids_vec = Vec::new();
    let mut attention_mask_vec = Vec::new();
    for encoding in &encodings {
        let ids = encoding.get_ids();
        let mask = encoding.get_attention_mask();
        let mut padded_ids = ids.to_vec();
        let mut padded_mask = mask.to_vec();
        padded_ids.resize(max_len, 0);
        padded_mask.resize(max_len, 0);
        input_ids_vec.extend(padded_ids);
        attention_mask_vec.extend(padded_mask);
    }

    let batch_size = texts.len();
    let input_ids = Tensor::new(input_ids_vec, &model.device)?.reshape((batch_size, max_len))?;
    let token_type_ids = input_ids.zeros_like()?;
    let attention_mask_t =
        Tensor::new(attention_mask_vec, &model.device)?.reshape((batch_size, max_len))?;

    // Single forward pass for the whole batch
    let output = model.forward(&input_ids, &token_type_ids, Some(&attention_mask_t))?;

    // Mean pooling + L2 normalize per sample
    let mask = attention_mask_t.to_dtype(candle_core::DType::F32)?.unsqueeze(2)?;
    let sum = output.broadcast_mul(&mask)?.sum(1)?;
    let count = mask.sum(1)?;
    let mean_pooled = sum.broadcast_div(&count)?;

    let norm = mean_pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
    let normalized = mean_pooled.broadcast_div(&norm)?;

    // Extract per-sample embeddings
    let mut results = Vec::with_capacity(batch_size);
    for i in 0..batch_size {
        let emb = normalized.get(i)?.squeeze(0)?.to_vec1()?;
        results.push(emb);
    }
    Ok(results)
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
    #[ignore = "requires HuggingFace model download"]
    fn test_candle_provider_encode() {
        let provider = CandleProvider;
        let emb = provider.encode("hello world").unwrap();
        assert_eq!(emb.len(), 384);
        // L2 norm should be ≈ 1.0 (sentence-transformers normalizes)
        let norm: f32 = emb.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "L2 norm = {}", norm);
    }

    #[test]
    #[ignore = "requires HuggingFace model download"]
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

        let candle_emb = CandleProvider.encode(text).unwrap();
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
