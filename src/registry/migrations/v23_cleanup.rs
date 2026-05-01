use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v23: Remove repo_modules_legacy which has a stale FK to repos(id).
    // save_modules now writes to repo_modules (entity-model aligned, no FK).
    let _ = conn.execute("DROP TABLE IF EXISTS repo_modules_legacy", []);
    conn.execute("PRAGMA user_version = 23", [])?;
    Ok(())
}
