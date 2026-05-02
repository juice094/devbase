use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS repo_index_state (
            repo_id TEXT PRIMARY KEY,
            last_commit_hash TEXT,
            indexed_at DATETIME DEFAULT current_timestamp
        )",
        [],
    )?;
    conn.execute("PRAGMA user_version = 27", [])?;
    Ok(())
}
