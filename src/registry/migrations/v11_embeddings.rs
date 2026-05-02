use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let ce_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_embeddings'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !ce_exists {
        conn.execute(
            "CREATE TABLE code_embeddings (
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL DEFAULT '',
                symbol_name TEXT NOT NULL,
                embedding BLOB NOT NULL,
                generated_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, file_path, symbol_name)
            )",
            [],
        )?;
    }
    conn.execute("PRAGMA user_version = 11", [])?;
    Ok(())
}
