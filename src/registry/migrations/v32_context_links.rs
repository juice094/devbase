// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS context_entity_links (
            context_id TEXT NOT NULL,
            entity_id  TEXT NOT NULL,
            link_type  TEXT NOT NULL DEFAULT 'linked',
            created_at TEXT NOT NULL,
            PRIMARY KEY (context_id, entity_id, link_type)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_context_links_entity ON context_entity_links(entity_id)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 32", [])?;
    Ok(())
}
