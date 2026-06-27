// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
// Schema v37: Memory Graph Foundation
// - memory_relations table for typed edges between agent_memories
// - Enriched agent_memories columns (importance, decay, access tracking, chunking, quality)
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // ── memory_relations: typed knowledge-graph edges between memories ──
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_relations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_memory_id INTEGER NOT NULL,
            to_memory_id INTEGER NOT NULL,
            relation_type TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 1.0,
            evidence TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (from_memory_id) REFERENCES agent_memories(id) ON DELETE CASCADE,
            FOREIGN KEY (to_memory_id) REFERENCES agent_memories(id) ON DELETE CASCADE,
            UNIQUE(from_memory_id, to_memory_id, relation_type)
        );

        CREATE INDEX IF NOT EXISTS idx_memory_relations_from ON memory_relations(from_memory_id);
        CREATE INDEX IF NOT EXISTS idx_memory_relations_to ON memory_relations(to_memory_id);
        CREATE INDEX IF NOT EXISTS idx_memory_relations_type ON memory_relations(relation_type);

        -- Memory lifecycle: importance (0-1), decay factor, access tracking
        ALTER TABLE agent_memories ADD COLUMN importance REAL NOT NULL DEFAULT 0.5;
        ALTER TABLE agent_memories ADD COLUMN decay_factor REAL NOT NULL DEFAULT 0.0;
        ALTER TABLE agent_memories ADD COLUMN last_accessed_at TEXT;
        ALTER TABLE agent_memories ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE agent_memories ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

        -- Chunking support for large memories
        ALTER TABLE agent_memories ADD COLUMN chunk_index INTEGER;
        ALTER TABLE agent_memories ADD COLUMN parent_memory_id INTEGER;
        ALTER TABLE agent_memories ADD COLUMN token_count INTEGER;

        -- Quality and archival
        ALTER TABLE agent_memories ADD COLUMN quality_score REAL;
        ALTER TABLE agent_memories ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0;

        -- Index for efficient decay computation: (context_id, last_accessed_at, importance)
        CREATE INDEX IF NOT EXISTS idx_agent_memories_decay
            ON agent_memories(context_id, is_archived, importance, last_accessed_at);
        ",
    )?;

    conn.execute("PRAGMA user_version = 37", [])?;
    Ok(())
}
