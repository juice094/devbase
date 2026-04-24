#!/usr/bin/env python3
"""External embedding provider for devbase.

Reads function symbols from devbase's SQLite registry, generates embeddings
via Ollama, and stores them back as little-endian f32 BLOBs.
"""

from __future__ import annotations

import argparse
import os
import platform
import sqlite3
import struct
import sys
from datetime import datetime, timezone

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None

import requests


def get_registry_path(override: str | None = None) -> str:
    if override:
        return override
    system = platform.system()
    if system == "Windows":
        local_appdata = os.environ.get("LOCALAPPDATA")
        if not local_appdata:
            raise RuntimeError("LOCALAPPDATA environment variable is not set")
        return os.path.join(local_appdata, "devbase", "registry.db")
    # Linux / macOS
    home = os.path.expanduser("~")
    candidates = [
        os.path.join(home, ".local", "share", "devbase", "registry.db"),
        os.path.join(home, ".config", "devbase", "registry.db"),
    ]
    for path in candidates:
        if os.path.exists(path):
            return path
    # Default to ~/.local/share if neither exists
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


def fetch_embedding(
    session: requests.Session,
    url: str,
    model: str,
    prompt: str,
    timeout: int,
) -> list[float]:
    resp = session.post(
        f"{url}/api/embeddings",
        json={"model": model, "prompt": prompt},
        timeout=timeout,
    )
    resp.raise_for_status()
    data = resp.json()
    embedding = data.get("embedding")
    if embedding is None:
        raise RuntimeError(f"Ollama response missing 'embedding' field: {data}")
    return embedding


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
        description="Generate and store code embeddings for devbase via Ollama."
    )
    parser.add_argument("--repo-id", required=True, help="Repository ID in devbase")
    parser.add_argument("--model", default="nomic-embed-text", help="Ollama embedding model")
    parser.add_argument(
        "--ollama-url", default="http://localhost:11434", help="Ollama base URL"
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=32,
        help="Symbols per batch (progress granularity)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Re-generate embeddings for symbols that already have them",
    )
    parser.add_argument("--registry-db", help="Override path to registry.db")
    parser.add_argument(
        "--timeout",
        type=int,
        default=120,
        help="HTTP timeout per Ollama request (seconds)",
    )
    args = parser.parse_args()

    config = load_config()

    # Config file overrides defaults, CLI overrides everything
    ollama_url = args.ollama_url or config.get("ollama", {}).get("url", "http://localhost:11434")
    model = args.model or config.get("ollama", {}).get("model", "nomic-embed-text")
    batch_size = args.batch_size or config.get("embedding", {}).get("batch_size", 32)
    timeout = args.timeout or config.get("embedding", {}).get("request_timeout", 120)
    registry_db = args.registry_db or config.get("registry", {}).get("db_path")

    try:
        db_path = get_registry_path(registry_db)
    except RuntimeError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    if not os.path.exists(db_path):
        print(f"Error: Registry database not found at {db_path}", file=sys.stderr)
        return 1

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

    print(f"Embedding {len(to_process)} function symbol(s) for repo '{args.repo_id}'...")
    print(f"Model: {model} | Ollama: {ollama_url} | Batch size: {batch_size}")

    session = requests.Session()
    try:
        # Quick health check
        resp = session.get(f"{ollama_url}/api/tags", timeout=10)
        resp.raise_for_status()
    except requests.exceptions.ConnectionError:
        print(
            f"Error: Cannot connect to Ollama at {ollama_url}. Is it running?",
            file=sys.stderr,
        )
        return 1
    except requests.exceptions.RequestException as e:
        print(f"Error: Ollama health check failed: {e}", file=sys.stderr)
        return 1

    total = len(to_process)
    stored = 0
    errors = 0
    batch_buffer: list[tuple[str, bytes]] = []

    for i, symbol in enumerate(to_process, 1):
        prompt = format_prompt(symbol)
        try:
            embedding = fetch_embedding(session, ollama_url, model, prompt, timeout)
            blob = embedding_to_bytes(embedding)
            batch_buffer.append((symbol["name"], blob))
        except requests.exceptions.RequestException as e:
            print(f"\nError embedding '{symbol['name']}': {e}", file=sys.stderr)
            errors += 1
            continue
        except Exception as e:
            print(f"\nError processing '{symbol['name']}': {e}", file=sys.stderr)
            errors += 1
            continue

        if len(batch_buffer) >= batch_size or i == total:
            try:
                store_embeddings(conn, args.repo_id, batch_buffer)
                stored += len(batch_buffer)
            except sqlite3.Error as e:
                print(f"\nDatabase error storing batch: {e}", file=sys.stderr)
                errors += len(batch_buffer)
            batch_buffer.clear()

        if i % 10 == 0 or i == total:
            print(f"  Progress: {i}/{total} ({stored} stored, {errors} errors)")

    conn.close()

    print(f"\nDone. {stored} embedding(s) stored, {errors} error(s), {skipped} skipped.")
    return 1 if errors > 0 else 0


if __name__ == "__main__":
    sys.exit(main())
