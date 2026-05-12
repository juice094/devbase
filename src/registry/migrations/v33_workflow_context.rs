// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    let cols: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(workflow_executions)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.filter_map(Result::ok).collect()
    };
    if !cols.iter().any(|c| c == "context_id") {
        conn.execute("ALTER TABLE workflow_executions ADD COLUMN context_id TEXT", [])?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflow_execs_context ON workflow_executions(context_id)",
        [],
    )?;
    conn.execute("PRAGMA user_version = 33", [])?;
    Ok(())
}
