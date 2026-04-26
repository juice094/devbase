//! Embedding storage protocol and similarity utilities.
//!
//! Devbase handles storage (SQLite BLOB), serialization, query-time
//! similarity computation, and local query embedding generation via
//! sentence-transformers (when available).

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

/// Generate an embedding vector for a natural-language query using
/// sentence-transformers (all-MiniLM-L6-v2) via a local Python interpreter.
/// Falls back to an error if no Python with sentence-transformers is available.
pub fn generate_query_embedding(query: &str) -> anyhow::Result<Vec<f32>> {
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
        let emb = generate_query_embedding("hello world").unwrap();
        assert!(!emb.is_empty());
        // all-MiniLM-L6-v2 produces 384-dim vectors
        assert_eq!(emb.len(), 384);
    }
}
