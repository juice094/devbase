use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v10: code call graph for "who calls X" queries
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_call_graph'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !exists {
        conn.execute(
            "CREATE TABLE code_call_graph (
                repo_id TEXT NOT NULL,
                caller_file TEXT NOT NULL,
                caller_symbol TEXT NOT NULL,
                caller_line INTEGER,
                callee_name TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX idx_call_graph_repo ON code_call_graph(repo_id)", [])?;
        conn.execute(
            "CREATE INDEX idx_call_graph_callee ON code_call_graph(callee_name)",
            [],
        )?;
        conn.execute("CREATE INDEX idx_call_graph_caller ON code_call_graph(repo_id, caller_file, caller_symbol)", [])?;
    }
    conn.execute("PRAGMA user_version = 10", [])?;
    Ok(())
}
