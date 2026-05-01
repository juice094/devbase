use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v17: Workflow Engine — workflow definitions and execution tracking
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflows (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            version         TEXT NOT NULL,
            description     TEXT,
            definition_yaml TEXT NOT NULL,
            status          TEXT DEFAULT 'draft',
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflow_executions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            workflow_id     TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            inputs_json     TEXT,
            status          TEXT NOT NULL,
            current_step    TEXT,
            started_at      TEXT NOT NULL,
            finished_at     TEXT,
            duration_ms     INTEGER
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflow_execs_wf ON workflow_executions(workflow_id)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 17", [])?;
    Ok(())
}
