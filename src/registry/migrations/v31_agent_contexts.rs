// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_contexts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            intent TEXT,
            status TEXT DEFAULT 'active',
            created_at DATETIME DEFAULT current_timestamp,
            updated_at DATETIME DEFAULT current_timestamp
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_memories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            context_id TEXT NOT NULL,
            memory_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at DATETIME DEFAULT current_timestamp,
            FOREIGN KEY (context_id) REFERENCES agent_contexts(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_memories_context ON agent_memories(context_id)",
        [],
    )?;

    conn.execute("PRAGMA user_version = 31", [])?;
    Ok(())
}
