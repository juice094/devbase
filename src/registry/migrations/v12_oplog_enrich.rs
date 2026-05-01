use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(oplog)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect()
    };
    if !cols.iter().any(|c| c == "event_type") {
        conn.execute("ALTER TABLE oplog ADD COLUMN event_type TEXT", [])?;
    }
    if !cols.iter().any(|c| c == "duration_ms") {
        conn.execute("ALTER TABLE oplog ADD COLUMN duration_ms INTEGER", [])?;
    }
    if !cols.iter().any(|c| c == "event_version") {
        conn.execute("ALTER TABLE oplog ADD COLUMN event_version INTEGER DEFAULT 1", [])?;
    }
    conn.execute(
        "UPDATE oplog SET event_type = CASE operation WHEN 'health' THEN 'health_check' ELSE operation END WHERE event_type IS NULL",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_oplog_event_type ON oplog(event_type)",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_oplog_repo ON oplog(repo_id)", [])?;
    conn.execute("PRAGMA user_version = 12", [])?;
    Ok(())
}
