use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // Step 1: Create new table with 3D primary key (repo_id, file_path, symbol_name)
    // Note: FK to repos is omitted because repos may have been dropped (v21).
    conn.execute(
        "CREATE TABLE code_embeddings_new (
            repo_id      TEXT NOT NULL,
            file_path    TEXT NOT NULL,
            symbol_name  TEXT NOT NULL,
            embedding    BLOB NOT NULL,
            generated_at TEXT NOT NULL,
            PRIMARY KEY (repo_id, file_path, symbol_name)
        )",
        [],
    )?;

    // Step 2: Migrate data from old table.
    // file_path is backfilled as empty string; subsequent incremental indexes
    // will populate the correct file_path via the new 3D primary key.
    conn.execute(
        "INSERT INTO code_embeddings_new (repo_id, file_path, symbol_name, embedding, generated_at)
         SELECT repo_id, '', symbol_name, embedding, generated_at
         FROM code_embeddings",
        [],
    )?;

    // Step 3: Drop old table and rename
    conn.execute("DROP TABLE code_embeddings", [])?;
    conn.execute("ALTER TABLE code_embeddings_new RENAME TO code_embeddings", [])?;

    // Step 4: Also create the compensation log table for Sprint B (Saga pattern)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS index_compensation_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_id TEXT NOT NULL,
            phase TEXT NOT NULL,
            tantivy_doc_id TEXT,
            created_at DATETIME DEFAULT current_timestamp
        )",
        [],
    )?;

    conn.execute("PRAGMA user_version = 28", [])?;
    Ok(())
}
