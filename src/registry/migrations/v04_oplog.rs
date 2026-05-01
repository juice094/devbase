use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let oplog_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='oplog'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !oplog_exists {
        conn.execute(
            "CREATE TABLE oplog (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                operation TEXT NOT NULL,
                repo_id TEXT,
                details TEXT,
                status TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX idx_oplog_operation ON oplog(operation)", [])?;
        conn.execute("CREATE INDEX idx_oplog_timestamp ON oplog(timestamp)", [])?;
    }
    conn.execute("PRAGMA user_version = 4", [])?;
    Ok(())
}
