fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = dirs::data_local_dir().ok_or("no local data dir")?;
    let db_path = data_dir.join("devbase").join("registry.db");
    let conn = rusqlite::Connection::open(&db_path)?;
    let deleted = conn.execute(
        "DELETE FROM repos WHERE id LIKE 'Clarity_%' OR id LIKE 'clarity_backup%'",
        [],
    )?;
    println!("Deleted {} backup entries from devbase registry.", deleted);
    
    println!("\nRemaining registered repos:");
    let mut stmt = conn.prepare("SELECT id, local_path FROM repos")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, path) = row?;
        println!("  [{}] {}", id, path);
    }
    Ok(())
}
