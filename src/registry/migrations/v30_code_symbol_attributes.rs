// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // On fresh DBs, code_symbols hasn't been created yet (it comes later in init_db_at).
    // Create it here idempotently so ALTER TABLE succeeds.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS code_symbols (
            repo_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            symbol_type TEXT NOT NULL,
            name TEXT NOT NULL,
            line_start INTEGER,
            line_end INTEGER,
            signature TEXT,
            attributes TEXT,
            PRIMARY KEY (repo_id, file_path, name)
        )",
        [],
    )?;
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(code_symbols)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect()
    };
    if !cols.iter().any(|c| c == "attributes") {
        conn.execute("ALTER TABLE code_symbols ADD COLUMN attributes TEXT", [])?;
    }
    conn.execute("PRAGMA user_version = 30", [])?;
    Ok(())
}
