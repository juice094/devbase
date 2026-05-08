# devbase-embedding

Pure-local text embedding generation with pluggable backends: Candle (all-MiniLM-L6-v2) and Ollama. Zero external API dependencies.

## Why use it

For projects that need local embeddings without pulling in Python. Provides a unified `EmbeddingProvider` trait with configurable backend switching.

## Alternatives

- `text-embeddings-inference` (HuggingFace): more powerful but requires Docker/Python
- `fastembed-rs`: pure Rust but limited model selection
