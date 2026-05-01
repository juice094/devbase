use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v26: Denormalize repo-specific fields from entities.metadata JSON into standalone columns.
    // This eliminates json_extract drift and enables NOT NULL constraints for repos.
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(entities)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect()
    };
    if !cols.contains(&"language".to_string()) {
        conn.execute("ALTER TABLE entities ADD COLUMN language TEXT", [])?;
    }
    if !cols.contains(&"discovered_at".to_string()) {
        conn.execute("ALTER TABLE entities ADD COLUMN discovered_at TEXT", [])?;
    }
    if !cols.contains(&"workspace_type".to_string()) {
        conn.execute("ALTER TABLE entities ADD COLUMN workspace_type TEXT DEFAULT 'git'", [])?;
    }
    if !cols.contains(&"data_tier".to_string()) {
        conn.execute("ALTER TABLE entities ADD COLUMN data_tier TEXT DEFAULT 'private'", [])?;
    }
    if !cols.contains(&"last_synced_at".to_string()) {
        conn.execute("ALTER TABLE entities ADD COLUMN last_synced_at TEXT", [])?;
    }
    if !cols.contains(&"stars".to_string()) {
        conn.execute("ALTER TABLE entities ADD COLUMN stars INTEGER", [])?;
    }
    // Migrate existing repo data from metadata JSON into new columns.
    conn.execute(
        "UPDATE entities SET
            language = json_extract(metadata, '$.language'),
            discovered_at = COALESCE(json_extract(metadata, '$.discovered_at'), datetime('now')),
            workspace_type = COALESCE(json_extract(metadata, '$.workspace_type'), 'git'),
            data_tier = COALESCE(json_extract(metadata, '$.data_tier'), 'private'),
            last_synced_at = json_extract(metadata, '$.last_synced_at'),
            stars = json_extract(metadata, '$.stars')
         WHERE entity_type = 'repo'",
        [],
    )?;
    conn.execute("PRAGMA user_version = 26", [])?;
    Ok(())
}
