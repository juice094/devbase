use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v19: Knowledge Meta — L4 metacognition layer for human corrections and cross-session consistency
    conn.execute(
        "CREATE TABLE IF NOT EXISTS knowledge_meta (
            id              TEXT PRIMARY KEY,
            target_level    INTEGER NOT NULL,
            target_id       TEXT NOT NULL,
            correction_type TEXT,
            correction_json TEXT,
            confidence      REAL DEFAULT 0.0,
            created_at      TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_knowledge_meta_target ON knowledge_meta(target_level, target_id)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 19", [])?;
    Ok(())
}
