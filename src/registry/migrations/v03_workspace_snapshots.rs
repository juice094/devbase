use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let snapshots_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='workspace_snapshots'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !snapshots_exists {
        conn.execute(
            "CREATE TABLE workspace_snapshots (
                repo_id TEXT PRIMARY KEY,
                file_hash TEXT NOT NULL,
                checked_at TEXT NOT NULL,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
    }
    conn.execute("PRAGMA user_version = 3", [])?;
    Ok(())
}
