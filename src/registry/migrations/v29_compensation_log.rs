use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS orphan_tantivy_docs (
            repo_id TEXT PRIMARY KEY,
            detected_at DATETIME DEFAULT current_timestamp
        )",
        [],
    )?;
    conn.execute("PRAGMA user_version = 29", [])?;
    Ok(())
}
