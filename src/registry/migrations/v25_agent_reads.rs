use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v25: Behavioral context — agent symbol read tracking for relevance boosting.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_symbol_reads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_id TEXT NOT NULL,
            symbol_name TEXT NOT NULL,
            read_at TEXT NOT NULL,
            context TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_reads_symbol ON agent_symbol_reads(repo_id, symbol_name)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_reads_time ON agent_symbol_reads(read_at DESC)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 25", [])?;
    Ok(())
}
