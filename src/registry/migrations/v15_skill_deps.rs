use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(skills)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect()
    };
    if !cols.iter().any(|c| c == "dependencies") {
        conn.execute("ALTER TABLE skills ADD COLUMN dependencies TEXT", [])?;
    }
    conn.execute("PRAGMA user_version = 15", [])?;
    Ok(())
}
