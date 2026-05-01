use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v18: Known Limits — L3 risk layer for tracking system constraints and hard vetoes
    conn.execute(
        "CREATE TABLE IF NOT EXISTS known_limits (
            id              TEXT PRIMARY KEY,
            category        TEXT NOT NULL,
            description     TEXT NOT NULL,
            source          TEXT,
            severity        INTEGER,
            first_seen_at   TEXT NOT NULL,
            last_checked_at TEXT,
            mitigated       INTEGER DEFAULT 0
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_known_limits_category ON known_limits(category)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_known_limits_mitigated ON known_limits(mitigated)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 18", [])?;
    Ok(())
}
