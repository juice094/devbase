use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v14: Skill Runtime — skill registry and execution tracking
    conn.execute(
        "CREATE TABLE IF NOT EXISTS skills (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            version         TEXT NOT NULL,
            description     TEXT NOT NULL,
            author          TEXT,
            tags            TEXT,
            entry_script    TEXT,
            skill_type      TEXT NOT NULL DEFAULT 'custom',
            local_path      TEXT NOT NULL,
            inputs_schema   TEXT,
            outputs_schema  TEXT,
            embedding       BLOB,
            installed_at    TEXT NOT NULL,
            updated_at      TEXT NOT NULL,
            last_used_at    TEXT
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_skills_type ON skills(skill_type)", [])?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS skill_executions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_id        TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            args            TEXT,
            status          TEXT NOT NULL,
            stdout          TEXT,
            stderr          TEXT,
            exit_code       INTEGER,
            started_at      TEXT NOT NULL,
            finished_at     TEXT,
            duration_ms     INTEGER
        )",
        [],
    )?;
    conn.execute("PRAGMA user_version = 14", [])?;
    Ok(())
}
