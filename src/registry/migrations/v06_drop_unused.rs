use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // Drop unused tables from earlier schema versions
    let _ = conn.execute("DROP TABLE IF EXISTS ai_queries", []);
    let _ = conn.execute("DROP TABLE IF EXISTS agri_observations", []);
    conn.execute("PRAGMA user_version = 6", [])?;
    Ok(())
}
