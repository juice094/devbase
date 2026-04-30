//! Embedding storage protocol and similarity utilities.
//!
//! Devbase handles storage (SQLite BLOB), serialization, query-time
//! similarity computation, and local query embedding generation via
//! sentence-transformers (when available).
//!
//! ## Provider architecture (v0.14预留)
//! `EmbeddingProvider` trait abstracts the generation backend.
//! Current: `PythonProvider` (local Python + sentence-transformers).
//! Future: `CandleProvider` (pure-Rust, feature-gated, Sprint 14).

/// Provider trait for text-to-embedding generation.
/// Implemented by Python backend now; candle backend in v0.14.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for a single query string.
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// Provider name for diagnostics.
    fn name(&self) -> &'static str;
}

/// Production provider selector.
/// Returns the best available provider at runtime.
pub fn default_provider() -> Box<dyn EmbeddingProvider> {
    #[cfg(feature = "local-embedding")]
    {
        Box::new(CandleProvider)
    }
    #[cfg(not(feature = "local-embedding"))]
    {
        Box::new(PythonProvider)
    }
}

// ---------------------------------------------------------------------------
// CandleProvider — pure-Rust local embedding via all-MiniLM-L6-v2
// ---------------------------------------------------------------------------

#[cfg(feature = "local-embedding")]
pub struct CandleProvider;

#[cfg(feature = "local-embedding")]
impl EmbeddingProvider for CandleProvider {
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let (model, tokenizer) = get_candle_resources()?;
        encode_with_candle(model, tokenizer, text)
    }
    fn name(&self) -> &'static str {
        "candle-all-MiniLM-L6-v2"
    }
}

#[cfg(feature = "local-embedding")]
fn get_candle_resources() -> anyhow::Result<&'static (candle_transformers::models::bert::BertModel, tokenizers::Tokenizer)> {
    use std::sync::OnceLock;
    static RESOURCES: OnceLock<Result<(candle_transformers::models::bert::BertModel, tokenizers::Tokenizer), String>> = OnceLock::new();
    match RESOURCES.get_or_init(|| init_candle_resources().map_err(|e| e.to_string())) {
        Ok(r) => Ok(r),
        Err(e) => Err(anyhow::anyhow!("CandleProvider init failed: {}", e)),
    }
}

#[cfg(feature = "local-embedding")]
fn init_candle_resources() -> anyhow::Result<(candle_transformers::models::bert::BertModel, tokenizers::Tokenizer)> {
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

#[cfg(feature = "local-embedding")]
fn encode_with_candle(
    model: &candle_transformers::models::bert::BertModel,
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
) -> anyhow::Result<Vec<f32>> {
    use candle_core::Tensor;

    let encoding = tokenizer.encode(text, true).map_err(|e| anyhow::anyhow!(e))?;
    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();

    let input_ids = Tensor::new(input_ids, &model.device)?.unsqueeze(0)?;
    let token_type_ids = input_ids.zeros_like()?;
    let attention_mask_t = Tensor::new(attention_mask, &model.device)?.unsqueeze(0)?;

    let output = model.forward(&input_ids, &token_type_ids, Some(&attention_mask_t))?;

    // Mean pooling: average over non-padding tokens
    let mask = attention_mask_t
        .to_dtype(candle_core::DType::F32)?
        .unsqueeze(2)?;
    let sum = output.broadcast_mul(&mask)?.sum(1)?;
    let count = mask.sum(1)?;
    let mean_pooled = sum.broadcast_div(&count)?;

    // L2 normalize (sentence-transformers default)
    let norm = mean_pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
    let normalized = mean_pooled.broadcast_div(&norm)?;

    Ok(normalized.squeeze(0)?.to_vec1()?)
}

/// Python-based provider using local sentence-transformers.
pub struct PythonProvider;

impl EmbeddingProvider for PythonProvider {
    fn encode(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        generate_query_embedding_python(text)
    }
    fn name(&self) -> &'static str {
        "python-sentence-transformers"
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

/// Convenience wrapper that uses the default provider.
/// Kept for backward compatibility with existing call sites.
pub fn generate_query_embedding(query: &str) -> anyhow::Result<Vec<f32>> {
    default_provider().encode(query)
}

fn generate_query_embedding_python(query: &str) -> anyhow::Result<Vec<f32>> {
    // TODO(veto-audit-2026-04-26): RF-7 路径隐私 + 可移植性 — 硬编码开发者个人环境路径，生产环境不可用。
    // 修复: 移除硬编码路径，仅保留 PATH 探测（python/python3/py）。v0.14 candle 本地 embedding 落地后删除此函数。
    let candidates: Vec<std::path::PathBuf> = [
        std::path::PathBuf::from(
            "C:\\Users\\22414\\AppData\\Roaming\\uv\\tools\\pip\\Scripts\\python.exe",
        ),
        std::path::PathBuf::from("python"),
        std::path::PathBuf::from("python3"),
        std::path::PathBuf::from("py"),
    ]
    .into_iter()
    .filter(|p| {
        if p == &std::path::PathBuf::from("python")
            || p == &std::path::PathBuf::from("python3")
            || p == &std::path::PathBuf::from("py")
        {
            std::process::Command::new(p)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            p.exists()
        }
    })
    .collect();

    let script = format!(
        r#"import os; os.environ['HF_HUB_OFFLINE']='1'; from sentence_transformers import SentenceTransformer; import struct; model = SentenceTransformer('all-MiniLM-L6-v2'); emb = model.encode('{}', convert_to_numpy=True); print(''.join(struct.pack('<f', float(x)).hex() for x in emb.tolist()))"#,
        query.replace('\\', "\\\\").replace('\'', "\\'")
    );

    let mut last_err = String::new();
    for python in &candidates {
        let output = std::process::Command::new(python).args(["-c", &script]).output();
        match output {
            Ok(out) if out.status.success() => {
                let hex_str = String::from_utf8(out.stdout)?.trim().to_string();
                let mut embedding = Vec::new();
                for chunk in hex_str.as_bytes().chunks(8) {
                    let bytes = u32::from_str_radix(std::str::from_utf8(chunk)?, 16)?;
                    embedding.push(f32::from_le_bytes(bytes.to_le_bytes()));
                }
                return Ok(embedding);
            }
            Ok(out) => {
                last_err =
                    format!("{} failed: {}", python.display(), String::from_utf8_lossy(&out.stderr))
            }
            Err(e) => last_err = format!("{} error: {}", python.display(), e),
        }
    }
    Err(anyhow::anyhow!(
        "Embedding provider failed (tried {} candidates). Last: {}",
        candidates.len(),
        last_err
    ))
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
    #[ignore = "requires local Python with sentence-transformers"]
    fn test_generate_query_embedding_smoke() {
        let provider = PythonProvider;
        let emb = provider.encode("hello world").unwrap();
        assert_eq!(emb.len(), 384);
    }

    #[test]
    fn test_default_provider_routes_correctly() {
        let provider = default_provider();
        #[cfg(feature = "local-embedding")]
        assert_eq!(provider.name(), "candle-all-MiniLM-L6-v2");
        #[cfg(not(feature = "local-embedding"))]
        assert_eq!(provider.name(), "python-sentence-transformers");
    }

    #[test]
    #[ignore = "requires hf-hub model download (~90MB)"]
    #[cfg(feature = "local-embedding")]
    fn test_candle_provider_encode() {
        let provider = CandleProvider;
        let emb = provider.encode("hello world").unwrap();
        assert_eq!(emb.len(), 384);
        // L2 norm should be ≈ 1.0 (sentence-transformers normalizes)
        let norm: f32 = emb.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "L2 norm = {}", norm);
    }

    #[test]
    #[ignore = "requires local Python with sentence-transformers"]
    fn test_provider_trait_consistency() {
        let provider = default_provider();
        let emb = provider.encode("hello world").unwrap();
        assert!(!emb.is_empty());
        // all-MiniLM-L6-v2 produces 384-dim vectors
        assert_eq!(emb.len(), 384);
    }
}
