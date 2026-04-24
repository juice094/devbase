# devbase Embedding Provider

A lightweight external embedding generator for [devbase](https://github.com/juice094/devbase). It reads function symbols from devbase's SQLite registry, generates vector embeddings via [Ollama](https://ollama.com), and stores them back for semantic search.

## Architecture

```
┌─────────────────┐     /api/embeddings      ┌──────────────────┐
│  embedding-     │ ───────────────────────> │     Ollama       │
│   provider      │    (nomic-embed-text)    │  (local LLM)     │
│   (this tool)   │ <─────────────────────── │                  │
└────────┬────────┘        float[] vectors   └──────────────────┘
         │
         │ SQLite INSERT/UPDATE
         ▼
┌──────────────────┐
│   registry.db    │
│ code_embeddings  │  <-- consumed by devbase MCP tools:
│   (BLOB f32)     │      devkit_semantic_search
└──────────────────┘      devkit_embedding_search
```

devbase follows an **"outboard brain"** design: the Rust core handles storage, indexing, and query-time similarity computation, while embedding generation is delegated to external providers (Ollama, llama.cpp, ONNX, remote APIs, etc.). This keeps the core small and lets you swap models without recompiling.

## Installation

```bash
# 1. Clone or navigate into this directory
cd tools/embedding-provider

# 2. Install dependencies
pip install -r requirements.txt

# 3. Ensure Ollama is running locally and the model is pulled
ollama pull nomic-embed-text
```

### Optional: TOML config

Copy `config.example.toml` to `config.toml` if you want persistent defaults (e.g., a remote Ollama instance).

## Usage

```bash
# Basic run
python index.py --repo-id myrepo

# Custom model / remote Ollama
python index.py --repo-id myrepo \
  --model nomic-embed-text \
  --ollama-url http://localhost:11434 \
  --batch-size 32

# Re-generate embeddings even if they already exist
python index.py --repo-id myrepo --force

# Override registry path (auto-detected by default)
python index.py --repo-id myrepo --registry-db /path/to/registry.db
```

### Registry auto-detection

| OS      | Default path                                              |
|---------|-----------------------------------------------------------|
| Windows | `%LOCALAPPDATA%\devbase\registry.db`                      |
| Linux   | `~/.local/share/devbase/registry.db` (fallback: `~/.config/...`) |
| macOS   | `~/.local/share/devbase/registry.db` (fallback: `~/.config/...`) |

## How it works

1. **Read** all rows from `code_symbols` where `symbol_type = 'function'` and `repo_id = <repo_id>`.
2. **Skip** symbols already present in `code_embeddings` unless `--force` is passed.
3. **Format** each symbol as:
   ```
   {name} in {file_path}: {signature}
   ```
4. **Request** embeddings from Ollama (`POST /api/embeddings`) one symbol at a time (MVP).
5. **Store** vectors as little-endian `f32` BLOBs in `code_embeddings`, using `ON CONFLICT DO UPDATE` upsert semantics.

## Notes

- The embedding BLOB format **must** match devbase's Rust serializer (`f32` little-endian). This script uses Python's `struct.pack('<f', ...)` to guarantee compatibility.
- `generated_at` is written as RFC 3339 / ISO 8601 UTC (`2024-01-15T10:30:00Z`).
- If Ollama is unreachable, the script prints a clear error and exits with code `1`.
