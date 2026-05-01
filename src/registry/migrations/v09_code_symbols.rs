use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v9: semantic code symbols — already created above via CREATE TABLE IF NOT EXISTS
    conn.execute("PRAGMA user_version = 9", [])?;
    Ok(())
}
