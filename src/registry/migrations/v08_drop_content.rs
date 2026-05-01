use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // Wave 9-3: drop content column from vault_notes (filesystem-first)
    let has_content: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('vault_notes') WHERE name = 'content'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if has_content {
        conn.execute(
            "CREATE TABLE vault_notes_v2 (
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
            "INSERT INTO vault_notes_v2 (id, path, title, frontmatter, tags, outgoing_links, created_at, updated_at)
             SELECT id, path, title, frontmatter, tags, outgoing_links, created_at, updated_at FROM vault_notes",
            [],
        )?;
        conn.execute("DROP TABLE vault_notes", [])?;
        conn.execute("ALTER TABLE vault_notes_v2 RENAME TO vault_notes", [])?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vault_notes_tags ON vault_notes(tags)",
            [],
        )?;
    }
    conn.execute("PRAGMA user_version = 8", [])?;
    Ok(())
}
