//! Local Rust embedding provider for devbase.
//!
//! Loads a GGUF model directly via llama.cpp (through `embellama`) and generates
//! f32 embeddings for code symbols stored in the devbase SQLite registry.
//!
//! No Ollama server required — pure local inference with optional CUDA.
//!
//! ## Prerequisites
//! - CMake 3.14+
//! - Visual Studio 2022 Build Tools (or full VS) with "Desktop development with C++"
//! - CUDA Toolkit 12.x (optional, only if `--features cuda` is used)
//!
//! ## Build
//! ```powershell
//! # CPU only
//! cargo build --release
//!
//! # With CUDA acceleration
//! cargo build --release --features cuda
//! ```
//!
//! ## Run
//! ```powershell
//! # Auto-discover model from Desktop\model
//! .\target\release\embedding-provider-rs --repo-id claude-code-rust
//!
//! # Explicit model path
//! .\target\release\embedding-provider-rs `
//!     --model-path "C:\Users\22414\Desktop\model\Qwen2.5-7B-Instruct.Q4_K_M.gguf" `
//!     --repo-id claude-code-rust
//! ```

use std::path::PathBuf;

use clap::Parser;
use embellama::{EngineConfig, EmbeddingEngine, ModelConfig, NormalizationMode};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "embedding-provider-rs")]
#[command(about = "Local Rust embedding provider for devbase (GGUF → SQLite)")]
struct Args {
    /// Repository ID to generate embeddings for
    #[arg(long)]
    repo_id: String,

    /// Path to GGUF model file. If omitted, auto-discovers from Desktop\model.
    #[arg(long)]
    model_path: Option<PathBuf>,

    /// Batch size for embedding generation
    #[arg(long, default_value_t = 16)]
    batch_size: usize,

    /// Skip symbols that already have embeddings
    #[arg(long, default_value_t = false)]
    skip_existing: bool,

    /// Registry database path. Defaults to devbase's standard location.
    #[arg(long)]
    db_path: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // 1. Resolve model path
    let model_path = resolve_model_path(args.model_path)?;
    info!("Using model: {}", model_path.display());

    // 2. Load embedding engine
    let model_config = ModelConfig::builder()
        .with_model_path(model_path.to_string_lossy().as_ref())
        .with_model_name("local-embedding")
        .with_normalization_mode(NormalizationMode::L2)
        .build()?;

    let engine_config = EngineConfig::builder()
        .with_model_config(model_config)
        .build()?;

    let engine = EmbeddingEngine::new(engine_config)?;
    info!("Embedding engine loaded successfully");

    // 3. Connect to devbase registry
    let db_path = args.db_path.unwrap_or_else(|| {
        dirs::data_local_dir()
            .expect("Could not find local data dir")
            .join("devbase")
            .join("registry.db")
    });
    info!("Registry DB: {}", db_path.display());

    let mut conn = rusqlite::Connection::open(&db_path)?;

    // 4. Read function symbols
    let symbols = read_symbols(&conn, &args.repo_id, args.skip_existing)?;
    if symbols.is_empty() {
        info!("No symbols to process for repo '{}'", args.repo_id);
        return Ok(());
    }
    info!("Found {} function symbols to embed", symbols.len());

    // 5. Generate & store embeddings in batches
    let mut total = 0usize;
    for (idx, chunk) in symbols.chunks(args.batch_size).enumerate() {
        let texts: Vec<String> = chunk
            .iter()
            .map(|(name, file, sig)| {
                let sig_text = sig.as_deref().unwrap_or(name);
                format!("{} in {}: {}", name, file, sig_text)
            })
            .collect();

        let embeddings = engine.embed_batch(None, &texts)?;
        if embeddings.len() != chunk.len() {
            warn!(
                "Batch {}: expected {} embeddings, got {}. Skipping batch.",
                idx,
                chunk.len(),
                embeddings.len()
            );
            continue;
        }

        let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);
        let mut pairs: Vec<(String, Vec<f32>)> = Vec::with_capacity(chunk.len());
        for ((name, _file, _sig), emb) in chunk.iter().zip(embeddings.iter()) {
            let vec: Vec<f32> = emb.iter().map(|&v| v).collect();
            pairs.push((name.clone(), vec));
        }

        save_embeddings(&mut conn, &args.repo_id, &pairs)?;
        total += chunk.len();
        info!("Batch {}/{}: {} embeddings stored (dim={})",
            idx + 1,
            (symbols.len() + args.batch_size - 1) / args.batch_size,
            chunk.len(),
            dim
        );
    }

    info!("Done! {} embeddings stored for '{}'", total, args.repo_id);
    Ok(())
}

/// Auto-discover GGUF model from known locations.
fn resolve_model_path(explicit: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!("Specified model path does not exist: {}", p.display());
    }

    let candidates = [
        PathBuf::from(r"C:\Users\22414\Desktop\model\Qwen2.5-7B-Instruct.Q4_K_M.gguf"),
        PathBuf::from(r"C:\Users\22414\Desktop\model\Qwen2.5-14B-Instruct.Q4_K_M.gguf"),
    ];

    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }

    anyhow::bail!(
        "Could not auto-discover GGUF model. Please specify --model-path. \
         Searched: {:?}",
        candidates
    )
}

/// Read function symbols from code_symbols table.
fn read_symbols(
    conn: &rusqlite::Connection,
    repo_id: &str,
    skip_existing: bool,
) -> anyhow::Result<Vec<(String, String, Option<String>)>> {
    let existing: std::collections::HashSet<String> = if skip_existing {
        let mut stmt = conn.prepare(
            "SELECT symbol_name FROM code_embeddings WHERE repo_id = ?1"
        )?;
        let rows = stmt.query_map([repo_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<std::collections::HashSet<_>, _>>()?
    } else {
        std::collections::HashSet::new()
    };

    let mut stmt = conn.prepare(
        "SELECT name, file_path, signature FROM code_symbols
         WHERE repo_id = ?1 AND symbol_type = 'function'"
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut symbols = Vec::new();
    for row in rows {
        let (name, file, sig) = row?;
        if skip_existing && existing.contains(&name) {
            continue;
        }
        symbols.push((name, file, sig));
    }
    Ok(symbols)
}

/// Save embeddings to code_embeddings table (little-endian f32 BLOB).
fn save_embeddings(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    pairs: &[(String, Vec<f32>)],
) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    let now = chrono::Utc::now().to_rfc3339();
    for (symbol_name, vec) in pairs {
        let blob: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
        tx.execute(
            "INSERT INTO code_embeddings (repo_id, symbol_name, embedding, generated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(repo_id, symbol_name) DO UPDATE SET
                 embedding = excluded.embedding,
                 generated_at = excluded.generated_at",
            rusqlite::params![repo_id, symbol_name, blob, &now],
        )?;
    }
    tx.commit()?;
    Ok(())
}
