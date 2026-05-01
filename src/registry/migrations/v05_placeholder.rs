use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    conn.execute("PRAGMA user_version = 5", [])?;
    Ok(())
}
