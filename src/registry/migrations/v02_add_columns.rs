use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let cols = {
        let mut stmt = conn.prepare("PRAGMA table_info(repos)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect::<Vec<_>>()
    };
    if !cols.iter().any(|c| c == "workspace_type") {
        conn.execute("ALTER TABLE repos ADD COLUMN workspace_type TEXT DEFAULT 'git'", [])?;
    }
    if !cols.iter().any(|c| c == "data_tier") {
        conn.execute("ALTER TABLE repos ADD COLUMN data_tier TEXT DEFAULT 'private'", [])?;
    }
    if !cols.iter().any(|c| c == "last_synced_at") {
        conn.execute("ALTER TABLE repos ADD COLUMN last_synced_at TEXT", [])?;
    }
    conn.execute("PRAGMA user_version = 2", [])?;
    Ok(())
}
