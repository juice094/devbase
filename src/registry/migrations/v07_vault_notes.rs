use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS vault_notes (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            title TEXT,
            frontmatter TEXT,
            tags TEXT,
            outgoing_links TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_vault_notes_tags ON vault_notes(tags)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 7", [])?;
    Ok(())
}
