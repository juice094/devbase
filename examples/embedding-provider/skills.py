#!/usr/bin/env python3
"""Generate and store semantic embeddings for devbase skills.

Uses sentence-transformers to compute embeddings directly from skill descriptions.
Embeddings are stored as little-endian f32 BLOBs in the skills table.
"""

from __future__ import annotations

import os
import platform
import sqlite3
import struct
import sys


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


def get_skills_without_embeddings(conn: sqlite3.Connection) -> list[tuple[str, str]]:
    cursor = conn.execute(
        "SELECT id, description FROM skills WHERE embedding IS NULL OR LENGTH(embedding) = 0"
    )
    return [(row[0], row[1]) for row in cursor]


def embedding_to_bytes(embedding: list[float]) -> bytes:
    return b"".join(struct.pack("<f", x) for x in embedding)


def main() -> int:
    model_name = "all-MiniLM-L6-v2"

    try:
        db_path = get_registry_path()
    except RuntimeError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    if not os.path.exists(db_path):
        print(f"Error: Registry database not found at {db_path}", file=sys.stderr)
        return 1

    try:
        from sentence_transformers import SentenceTransformer
    except ImportError:
        print(
            "Error: sentence-transformers is not installed.\n"
            "Run: pip install sentence-transformers",
            file=sys.stderr,
        )
        return 1

    print(f"Loading model '{model_name}'...")
    model = SentenceTransformer(model_name)
    dim = model.get_sentence_embedding_dimension()
    print(f"Model loaded. Embedding dimension: {dim}")

    conn = sqlite3.connect(db_path)

    skills = get_skills_without_embeddings(conn)
    if not skills:
        print("No skills without embeddings found.")
        conn.close()
        return 0

    print(f"Embedding {len(skills)} skill(s)...")

    descriptions = [description for _, description in skills]
    embeddings = model.encode(descriptions, convert_to_numpy=True, show_progress_bar=False)

    updated = 0
    with conn:
        for (skill_id, _), vec in zip(skills, embeddings):
            blob = embedding_to_bytes(vec.tolist())
            conn.execute(
                "UPDATE skills SET embedding = ? WHERE id = ?",
                (blob, skill_id),
            )
            updated += 1
            print(f"  Embedded: {skill_id}")

    conn.close()
    print(f"\nDone. {updated} skill embedding(s) stored.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
