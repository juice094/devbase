#!/usr/bin/env python3
"""Local embedding provider for devbase — no Ollama, no HTTP, just Python.

Uses sentence-transformers to compute embeddings directly.
Models are downloaded automatically on first use and cached locally.

Example:
    python local.py --repo-id devbase --model all-MiniLM-L6-v2
"""

from __future__ import annotations

import argparse
import os
import platform
import sqlite3
import struct
import sys
from datetime import datetime, timezone
from pathlib import Path

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None


def get_registry_path(override: str | None = None) -> str:
    if override:
        return override
    system = platform.system()
    if system == "Windows":
        local_appdata = os.environ.get("LOCALAPPDATA")
        if not local_appdata:
            raise RuntimeError("LOCALAPPDATA environment variable is not set")
        return os.path.join(local_appdata, "devbase", "registry.db")
    home = os.path.expanduser("~")
    candidates = [
        os.path.join(home, ".local", "share", "devbase", "registry.db"),
        os.path.join(home, ".config", "devbase", "registry.db"),
    ]
    for path in candidates:
        if os.path.exists(path):
            return path
    return candidates[0]


def load_config(path: str = "config.toml") -> dict:
    if tomllib is None or not os.path.exists(path):
        return {}
    with open(path, "rb") as f:
        return tomllib.load(f)


def get_functions(conn: sqlite3.Connection, repo_id: str) -> list[dict]:
    cursor = conn.execute(
        """
        SELECT file_path, name, signature
        FROM code_symbols
        WHERE repo_id = ? AND symbol_type = 'function'
        """,
        (repo_id,),
    )
    rows = []
    for file_path, name, signature in cursor:
        rows.append({
            "file_path": file_path,
            "name": name,
            "signature": signature or "",
        })
    return rows


def get_existing_embeddings(conn: sqlite3.Connection, repo_id: str) -> set[str]:
    cursor = conn.execute(
        "SELECT symbol_name FROM code_embeddings WHERE repo_id = ?",
        (repo_id,),
    )
    return {row[0] for row in cursor}


def format_prompt(symbol: dict) -> str:
    return f"{symbol['name']} in {symbol['file_path']}: {symbol['signature']}"


def embedding_to_bytes(embedding: list[float]) -> bytes:
    return b"".join(struct.pack("<f", x) for x in embedding)


def store_embeddings(
    conn: sqlite3.Connection,
    repo_id: str,
    items: list[tuple[str, bytes]],
) -> None:
    now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    with conn:
        for symbol_name, blob in items:
            conn.execute(
                """
                INSERT INTO code_embeddings (repo_id, symbol_name, embedding, generated_at)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(repo_id, symbol_name) DO UPDATE SET
                    embedding = excluded.embedding,
                    generated_at = excluded.generated_at
                """,
                (repo_id, symbol_name, blob, now),
            )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate and store code embeddings for devbase using local sentence-transformers."
    )
    parser.add_argument("--repo-id", required=True, help="Repository ID in devbase")
    parser.add_argument(
        "--model",
        default="all-MiniLM-L6-v2",
        help="Sentence-transformers model name (default: all-MiniLM-L6-v2)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=256,
        help="Symbols per batch for encoding (default: 256)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Re-generate embeddings for symbols that already have them",
    )
    parser.add_argument("--registry-db", help="Override path to registry.db")
    parser.add_argument(
        "--device",
        default="auto",
        help="Device for inference: auto, cpu, cuda (default: auto)",
    )
    args = parser.parse_args()

    config = load_config()
    model_name = args.model or config.get("embedding", {}).get("model", "all-MiniLM-L6-v2")
    batch_size = args.batch_size or config.get("embedding", {}).get("batch_size", 256)
    registry_db = args.registry_db or config.get("registry", {}).get("db_path")

    try:
        db_path = get_registry_path(registry_db)
    except RuntimeError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    if not os.path.exists(db_path):
        print(f"Error: Registry database not found at {db_path}", file=sys.stderr)
        return 1

    # Lazy import so we fail fast with a helpful message if not installed
    try:
        from sentence_transformers import SentenceTransformer
    except ImportError:
        print(
            "Error: sentence-transformers is not installed.\n"
            "Run: pip install sentence-transformers",
            file=sys.stderr,
        )
        return 1

    device = None if args.device == "auto" else args.device
    print(f"Loading model '{model_name}' (device={device or 'auto-detect'})...")
    kwargs = {}
    if device is not None:
        kwargs["device"] = device
    model = SentenceTransformer(model_name, **kwargs)
    dim = model.get_embedding_dimension()
    print(f"Model loaded. Embedding dimension: {dim}")

    conn = sqlite3.connect(db_path)

    symbols = get_functions(conn, args.repo_id)
    if not symbols:
        print(f"No function symbols found for repo '{args.repo_id}'.")
        return 0

    existing = set()
    if not args.force:
        existing = get_existing_embeddings(conn, args.repo_id)

    to_process = [s for s in symbols if s["name"] not in existing]
    skipped = len(symbols) - len(to_process)

    if skipped:
        print(f"Skipping {skipped} symbol(s) already embedded (use --force to override).")
    if not to_process:
        print("All symbols already have embeddings. Nothing to do.")
        return 0

    total = len(to_process)
    print(f"Embedding {total} function symbol(s) for repo '{args.repo_id}'...")
    print(f"Batch size: {batch_size} | Device: {args.device}")

    # Prepare prompts
    prompts = [format_prompt(s) for s in to_process]

    stored = 0
    errors = 0

    # Batch encode with progress
    for batch_start in range(0, total, batch_size):
        batch_end = min(batch_start + batch_size, total)
        batch_prompts = prompts[batch_start:batch_end]
        batch_symbols = to_process[batch_start:batch_end]

        try:
            # encode returns numpy array of shape (batch_size, dim)
            embeddings = model.encode(batch_prompts, convert_to_numpy=True, show_progress_bar=False)
            batch_items = []
            for sym, vec in zip(batch_symbols, embeddings):
                blob = embedding_to_bytes(vec.tolist())
                batch_items.append((sym["name"], blob))
            store_embeddings(conn, args.repo_id, batch_items)
            stored += len(batch_items)
        except Exception as e:
            print(f"\nError encoding batch {batch_start}-{batch_end}: {e}", file=sys.stderr)
            errors += len(batch_symbols)

        if (batch_end % 500 == 0) or batch_end == total:
            print(f"  Progress: {batch_end}/{total} ({stored} stored, {errors} errors)")

    conn.close()

    print(f"\nDone. {stored} embedding(s) stored, {errors} error(s), {skipped} skipped.")
    return 1 if errors > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
