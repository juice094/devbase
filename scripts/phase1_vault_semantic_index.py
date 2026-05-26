#!/usr/bin/env python3
"""
Phase 1 Prototype: Vault Semantic Indexing via Local Ollama + sqlite-vec

Pipeline:
    1. Scan Vault for Markdown files
    2. Parse frontmatter + body
    3. Chunk by heading hierarchy
    4. Generate embeddings via Ollama /api/embed
    5. Store in sqlite-vec
    6. Semantic search interface

Usage:
    python phase1_vault_semantic_index.py index    # Full rebuild
    python phase1_vault_semantic_index.py search "本地模型知识库设计"  # Query
    python phase1_vault_semantic_index.py rag "本地模型知识库设计"    # RAG answer
    python phase1_vault_semantic_index.py stats    # Show index stats
"""

import sqlite3
import sqlite_vec
import yaml
import re
import json
import urllib.request
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, asdict

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

VAULT_DIR = Path("C:/Users/22414/Documents/Obsidian Vault")
DB_PATH = Path("C:/Users/22414/.devbase/vault_semantic_index.db")
OLLAMA_URL = "http://localhost:11434/api/embed"
GENERATE_URL = "http://localhost:11434/api/generate"
# bge-m3: 1024-dim, multilingual (Chinese optimized), ~1.2GB
# nomic-embed-text: 768-dim, English optimized, ~274MB — kept as fallback
EMBED_MODEL = "bge-m3"
GENERATE_MODEL = "qwen2.5:7b"
EMBED_DIM = 1024
BATCH_SIZE = 16

# Files/dirs to skip
SKIP_PATTERNS = [
    r"\.obsidian",
    r"\.trash",
    r"99-Archive/\.trash-待清理",
    r"workspace",
    r"student-era",
    r"devbase-knowledge",
    r"dev",
    r"clarity",
    r"dotfiles",
    r"syncthing-rust",
    r"skills-dev",
]

# ---------------------------------------------------------------------------
# Data structures
# ---------------------------------------------------------------------------

@dataclass
class Chunk:
    file_path: str
    chunk_index: int
    chunk_type: str        # 'heading' | 'paragraph' | 'code' | 'table' | 'frontmatter'
    heading_path: str      # H2/H3 breadcrumb, e.g. "## 架构定位 / ### 技术选型"
    content: str
    tags: str              # JSON array from frontmatter
    date: Optional[str]
    indexed_at: str

# ---------------------------------------------------------------------------
# Markdown parsing
# ---------------------------------------------------------------------------

def parse_frontmatter(text: str) -> Tuple[Dict, str]:
    """Extract YAML frontmatter and return (metadata, body)."""
    if text.startswith("---"):
        parts = text.split("---", 2)
        if len(parts) >= 3:
            try:
                meta = yaml.safe_load(parts[1]) or {}
                return meta, parts[2].strip()
            except yaml.YAMLError:
                pass
    return {}, text


def split_into_chunks(file_path: str, body: str, frontmatter: Dict) -> List[Chunk]:
    """Split Markdown body into semantic chunks by heading hierarchy."""
    chunks: List[Chunk] = []
    tags = json.dumps(frontmatter.get("tags", []), ensure_ascii=False)
    date = frontmatter.get("date") or frontmatter.get("date")

    # Chunk 0: frontmatter summary (if present)
    if frontmatter:
        summary = " ".join(f"{k}: {v}" for k, v in frontmatter.items()
                          if k in ("title", "project", "type", "tags", "description"))
        if summary:
            chunks.append(Chunk(
                file_path=file_path,
                chunk_index=0,
                chunk_type="frontmatter",
                heading_path="",
                content=summary,
                tags=tags,
                date=date,
                indexed_at=datetime.now().isoformat(),
            ))

    lines = body.splitlines()
    current_heading = ""
    current_lines: List[str] = []
    chunk_idx = len(chunks)

    def flush():
        nonlocal chunk_idx, current_lines
        if not current_lines:
            return
        content = "\n".join(current_lines).strip()
        if len(content) >= 20:  # Skip very short fragments
            chunks.append(Chunk(
                file_path=file_path,
                chunk_index=chunk_idx,
                chunk_type="paragraph",
                heading_path=current_heading,
                content=content,
                tags=tags,
                date=date,
                indexed_at=datetime.now().isoformat(),
            ))
            chunk_idx += 1
        current_lines = []

    in_code_block = False
    code_buffer: List[str] = []
    code_lang = ""

    for line in lines:
        # Code blocks
        if line.strip().startswith("```"):
            if in_code_block:
                # End code block
                code_buffer.append(line)
                code_content = "\n".join(code_buffer)
                if len(code_content) >= 30:
                    chunks.append(Chunk(
                        file_path=file_path,
                        chunk_index=chunk_idx,
                        chunk_type="code",
                        heading_path=current_heading,
                        content=f"[{code_lang}]\n{code_content}",
                        tags=tags,
                        date=date,
                        indexed_at=datetime.now().isoformat(),
                    ))
                    chunk_idx += 1
                code_buffer = []
                in_code_block = False
                code_lang = ""
            else:
                # Start code block
                flush()
                in_code_block = True
                code_lang = line.strip()[3:].strip()
                code_buffer.append(line)
            continue

        if in_code_block:
            code_buffer.append(line)
            continue

        # Headings: update breadcrumb, do NOT append heading line to chunk
        m = re.match(r"^(#{2,3})\s+(.+)$", line)
        if m:
            flush()
            level = len(m.group(1))
            title = m.group(2).strip()
            if level == 2:
                current_heading = f"## {title}"
            else:
                current_heading = f"{current_heading} / ### {title}"
            continue

        # Tables: collect as chunks, flush when too large
        if line.strip().startswith("|"):
            if current_lines and not current_lines[-1].strip().startswith("|"):
                flush()
            if current_lines and len("\n".join(current_lines)) + len(line) > 3000:
                flush()
            current_lines.append(line)
            continue

        # Empty line -> potential flush boundary
        if line.strip() == "":
            if current_lines and len("\n".join(current_lines)) > 400:
                flush()
            continue

        current_lines.append(line)

    flush()

    # Post-process: split oversized paragraphs
    final_chunks: List[Chunk] = []
    for c in chunks:
        if c.chunk_type == "paragraph" and len(c.content) > 800:
            # Split by sentences
            sentences = re.split(r'(?<=[。\.\?\!])\s+', c.content)
            half = len(sentences) // 2
            if half > 0:
                final_chunks.append(Chunk(
                    file_path=c.file_path, chunk_index=c.chunk_index,
                    chunk_type=c.chunk_type, heading_path=c.heading_path,
                    content=" ".join(sentences[:half]), tags=c.tags, date=c.date,
                    indexed_at=c.indexed_at,
                ))
                final_chunks.append(Chunk(
                    file_path=c.file_path, chunk_index=c.chunk_index + 1,
                    chunk_type=c.chunk_type, heading_path=c.heading_path,
                    content=" ".join(sentences[half:]), tags=c.tags, date=c.date,
                    indexed_at=c.indexed_at,
                ))
            else:
                final_chunks.append(c)
        else:
            final_chunks.append(c)

    # Re-index chunk_index
    for i, c in enumerate(final_chunks):
        c.chunk_index = i

    return final_chunks


# ---------------------------------------------------------------------------
# Ollama embedding
# ---------------------------------------------------------------------------

def embed_batch(texts: List[str]) -> List[List[float]]:
    """Call Ollama /api/embed for a batch of texts."""
    # nomic-embed-text supports 8192 tokens; table-heavy content is token-inefficient
    MAX_CHARS = 6000
    trimmed = [t[:MAX_CHARS] if len(t) > MAX_CHARS else t for t in texts]
    body = json.dumps({
        "model": EMBED_MODEL,
        "input": trimmed,
    }).encode("utf-8")

    req = urllib.request.Request(
        OLLAMA_URL,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    with urllib.request.urlopen(req, timeout=120) as resp:
        data = json.loads(resp.read())
        embeddings = data.get("embeddings", [])
        if not embeddings:
            raise RuntimeError(f"Ollama returned no embeddings: {data}")
        return embeddings


def generate(prompt: str, model: str = GENERATE_MODEL, timeout: int = 300) -> str:
    """Call Ollama /api/generate and return the generated text."""
    body = json.dumps({
        "model": model,
        "prompt": prompt,
        "stream": False,
        "options": {
            "temperature": 0.3,
            "num_predict": 512,
        },
    }).encode("utf-8")

    req = urllib.request.Request(
        GENERATE_URL,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.loads(resp.read())
        response = data.get("response", "")
        if not response:
            raise RuntimeError(f"Ollama returned empty response: {data}")
        return response


# ---------------------------------------------------------------------------
# Database (sqlite-vec)
# ---------------------------------------------------------------------------

def init_db() -> sqlite3.Connection:
    """Create tables and virtual vector index."""
    DB_PATH.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(DB_PATH))
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)

    conn.execute("""
        CREATE TABLE IF NOT EXISTS vault_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            chunk_type TEXT,
            heading_path TEXT,
            content TEXT NOT NULL,
            tags TEXT,
            date TEXT,
            indexed_at TEXT NOT NULL,
            file_mtime REAL,
            UNIQUE(file_path, chunk_index)
        )
    """)

    conn.execute("""
        CREATE TABLE IF NOT EXISTS index_meta (
            key TEXT PRIMARY KEY,
            value TEXT
        )
    """)

    # sqlite-vec virtual table for vector search
    conn.execute(f"""
        CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(
            chunk_id INTEGER PRIMARY KEY,
            embedding FLOAT[{EMBED_DIM}]
        )
    """)

    conn.execute("CREATE INDEX IF NOT EXISTS idx_chunks_file ON vault_chunks(file_path)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_chunks_type ON vault_chunks(chunk_type)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_chunks_date ON vault_chunks(date)")

    return conn


def store_chunks(conn: sqlite3.Connection, chunks: List[Chunk], embeddings: List[List[float]]):
    """Upsert chunks and their embeddings."""
    assert len(chunks) == len(embeddings)

    for chunk, emb in zip(chunks, embeddings):
        # Upsert chunk
        conn.execute("""
            INSERT INTO vault_chunks (file_path, chunk_index, chunk_type, heading_path,
                                      content, tags, date, indexed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(file_path, chunk_index) DO UPDATE SET
                chunk_type = excluded.chunk_type,
                heading_path = excluded.heading_path,
                content = excluded.content,
                tags = excluded.tags,
                date = excluded.date,
                indexed_at = excluded.indexed_at
        """, (chunk.file_path, chunk.chunk_index, chunk.chunk_type,
              chunk.heading_path, chunk.content, chunk.tags,
              chunk.date, chunk.indexed_at))

        chunk_id = conn.execute(
            "SELECT id FROM vault_chunks WHERE file_path = ?1 AND chunk_index = ?2",
            (chunk.file_path, chunk.chunk_index)
        ).fetchone()[0]

        # sqlite-vec virtual table does not support UPSERT; use DELETE + INSERT
        conn.execute("DELETE FROM vec_index WHERE chunk_id = ?", (chunk_id,))
        conn.execute(
            "INSERT INTO vec_index (chunk_id, embedding) VALUES (?1, ?2)",
            (chunk_id, json.dumps(emb))
        )

    conn.commit()


def delete_file_chunks(conn: sqlite3.Connection, file_path: str):
    """Remove all chunks and embeddings for a given file."""
    rows = conn.execute(
        "SELECT id FROM vault_chunks WHERE file_path = ?", (file_path,)
    ).fetchall()
    for (chunk_id,) in rows:
        conn.execute("DELETE FROM vec_index WHERE chunk_id = ?", (chunk_id,))
    conn.execute("DELETE FROM vault_chunks WHERE file_path = ?", (file_path,))
    conn.commit()


# ---------------------------------------------------------------------------
# Indexing
# ---------------------------------------------------------------------------

def should_index(file_path: Path) -> bool:
    """Check if file should be indexed."""
    rel = file_path.relative_to(VAULT_DIR).as_posix()
    for pat in SKIP_PATTERNS:
        if re.search(pat, rel):
            return False
    return file_path.suffix == ".md"


def index_all():
    """Full rebuild of the semantic index."""
    conn = init_db()

    # Collect all markdown files
    files = [f for f in VAULT_DIR.rglob("*.md") if should_index(f)]
    print(f"[INFO] Found {len(files)} Markdown files to index")

    # Clear existing index for full rebuild — drop vec_index to avoid
    # sqlite-vec HNSW shadow-state bugs on DELETE+INSERT cycles.
    conn.execute("DROP TABLE IF EXISTS vec_index")
    conn.execute("DELETE FROM vault_chunks")
    conn.execute(f"""
        CREATE VIRTUAL TABLE vec_index USING vec0(
            chunk_id INTEGER PRIMARY KEY,
            embedding FLOAT[{EMBED_DIM}]
        )
    """)
    conn.commit()

    total_chunks = 0
    for i, file_path in enumerate(files, 1):
        rel_path = file_path.relative_to(VAULT_DIR).as_posix()
        print(f"[{i}/{len(files)}] {rel_path}")

        try:
            text = file_path.read_text(encoding="utf-8")
        except Exception as e:
            print(f"  [WARN] Read failed: {e}")
            continue

        frontmatter, body = parse_frontmatter(text)
        chunks = split_into_chunks(rel_path, body, frontmatter)
        if not chunks:
            continue

        # Generate embeddings in batches
        texts = [c.content for c in chunks]
        embeddings: List[List[float]] = []
        for batch_start in range(0, len(texts), BATCH_SIZE):
            batch = texts[batch_start:batch_start + BATCH_SIZE]
            try:
                embs = embed_batch(batch)
                embeddings.extend(embs)
                print(f"  → embedded {len(batch)} chunks")
            except Exception as e:
                print(f"  [ERROR] Embedding failed: {e}")
                break

        if len(embeddings) == len(chunks):
            store_chunks(conn, chunks, embeddings)
            total_chunks += len(chunks)

    conn.execute(
        "INSERT OR REPLACE INTO index_meta (key, value) VALUES ('last_full_index', ?)",
        (datetime.now().isoformat(),)
    )
    conn.commit()
    conn.close()

    print(f"\n[INFO] Index complete: {total_chunks} chunks from {len(files)} files")
    print(f"[INFO] Database: {DB_PATH}")


# ---------------------------------------------------------------------------
# Search
# ---------------------------------------------------------------------------

def search(query: str, top_k: int = 5) -> List[Dict]:
    """Semantic search over the Vault index."""
    conn = init_db()

    # Embed query
    query_emb = embed_batch([query])[0]
    query_json = json.dumps(query_emb)

    # Vector search via sqlite-vec (k=top_k required for KNN)
    rows = conn.execute("""
        SELECT
            c.file_path,
            c.chunk_index,
            c.chunk_type,
            c.heading_path,
            c.content,
            c.tags,
            c.date,
            v.distance
        FROM vec_index v
        JOIN vault_chunks c ON v.chunk_id = c.id
        WHERE v.embedding MATCH ?1 AND k = ?2
        ORDER BY v.distance
    """, (query_json, top_k)).fetchall()

    results = []
    for row in rows:
        results.append({
            "file_path": row[0],
            "chunk_index": row[1],
            "chunk_type": row[2],
            "heading_path": row[3],
            "content": row[4][:300] + "..." if len(row[4]) > 300 else row[4],
            "tags": row[5],
            "date": row[6],
            "distance": row[7],
        })

    conn.close()
    return results


# ---------------------------------------------------------------------------
# RAG (Retrieval-Augmented Generation)
# ---------------------------------------------------------------------------

def rag(query: str, top_k: int = 5) -> str:
    """Search vault chunks and generate an answer via local LLM."""
    print(f"[RAG] Retrieving context for: '{query}'")
    results = search(query, top_k=top_k)
    if not results:
        return "[RAG] No relevant chunks found in the vault index."

    # Build context from retrieved chunks
    context_parts = []
    for i, r in enumerate(results, 1):
        heading = f"Heading: {r['heading_path']}\n" if r['heading_path'] else ""
        context_parts.append(
            f"[{i}] Source: {r['file_path']}\n"
            f"{heading}"
            f"Content: {r['content']}\n"
        )
    context = "\n".join(context_parts)

    prompt = (
        "You are a helpful assistant with access to a personal knowledge base.\n"
        "Answer the user's question based ONLY on the provided context.\n"
        "If the context does not contain enough information, say so clearly.\n"
        "Keep your answer concise and in the same language as the question.\n\n"
        f"--- Context ---\n{context}\n--- End Context ---\n\n"
        f"Question: {query}\n\nAnswer:"
    )

    print(f"[RAG] Generating answer with {GENERATE_MODEL} ...")
    answer = generate(prompt)
    return answer


# ---------------------------------------------------------------------------
# Stats
# ---------------------------------------------------------------------------

def show_stats():
    conn = init_db()
    file_count = conn.execute("SELECT COUNT(DISTINCT file_path) FROM vault_chunks").fetchone()[0]
    chunk_count = conn.execute("SELECT COUNT(*) FROM vault_chunks").fetchone()[0]
    embed_count = conn.execute("SELECT COUNT(*) FROM vec_index").fetchone()[0]
    last_index = conn.execute(
        "SELECT value FROM index_meta WHERE key = 'last_full_index'"
    ).fetchone()

    print("=== Vault Semantic Index Stats ===")
    print(f"Files indexed:     {file_count}")
    print(f"Chunks stored:     {chunk_count}")
    print(f"Embeddings stored: {embed_count}")
    print(f"Last full index:   {last_index[0] if last_index else 'N/A'}")
    print(f"Database path:     {DB_PATH}")
    print(f"Embed model:       {EMBED_MODEL} ({EMBED_DIM}d)")

    # Chunk type distribution
    print("\nChunk type distribution:")
    for row in conn.execute(
        "SELECT chunk_type, COUNT(*) FROM vault_chunks GROUP BY chunk_type"
    ):
        print(f"  {row[0] or 'unknown'}: {row[1]}")

    conn.close()


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    import sys
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    cmd = sys.argv[1]

    if cmd == "index":
        index_all()
    elif cmd == "search":
        query = sys.argv[2] if len(sys.argv) > 2 else input("Query: ")
        results = search(query)
        print(f"\nTop {len(results)} results for: '{query}'\n")
        for i, r in enumerate(results, 1):
            print(f"--- [{i}] {r['file_path']} (dist={r['distance']:.4f}) ---")
            if r['heading_path']:
                print(f"Heading: {r['heading_path']}")
            print(f"Content: {r['content']}\n")
    elif cmd == "rag":
        query = sys.argv[2] if len(sys.argv) > 2 else input("Query: ")
        answer = rag(query)
        print(f"\n=== RAG Answer ===\n{answer}\n")
    elif cmd == "stats":
        show_stats()
    else:
        print(__doc__)
        sys.exit(1)


if __name__ == "__main__":
    main()
