// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // Schema v34: Agent Memory vector storage
    // Design principle: devbase does not generate embeddings; it only stores
    // and retrieves vectors produced by external providers (Ollama, OpenAI, etc.).
    // This keeps devbase as a "Local Context Compiler" / database layer,
    // not an LLM runtime.

    conn.execute("ALTER TABLE agent_memories ADD COLUMN embedding BLOB", [])?;
    conn.execute("ALTER TABLE agent_memories ADD COLUMN embedding_model TEXT", [])?;
    conn.execute("ALTER TABLE agent_memories ADD COLUMN indexed_at DATETIME", [])?;

    // Partial index: only index rows that actually have embeddings.
    // Keeps query plans efficient when most memories are text-only.
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_memories_embedding ON agent_memories(context_id, indexed_at) WHERE embedding IS NOT NULL",
        [],
    )?;

    conn.execute("PRAGMA user_version = 34", [])?;
    Ok(())
}
