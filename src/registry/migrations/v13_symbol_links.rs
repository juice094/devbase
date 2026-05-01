use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v13: explicit symbol-to-symbol knowledge links
    conn.execute(
        "CREATE TABLE IF NOT EXISTS code_symbol_links (
            source_repo TEXT NOT NULL,
            source_symbol TEXT NOT NULL,
            target_repo TEXT NOT NULL,
            target_symbol TEXT NOT NULL,
            link_type TEXT NOT NULL,
            strength REAL NOT NULL DEFAULT 0.0,
            created_at TEXT NOT NULL,
            PRIMARY KEY (source_repo, source_symbol, target_repo, target_symbol, link_type)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_symbol_links_source ON code_symbol_links(source_repo, source_symbol)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_symbol_links_target ON code_symbol_links(target_repo, target_symbol)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 13", [])?;
    Ok(())
}
