use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v22: Drop vault_notes, papers, workflows — entities is sole source of truth.
    // Rebuild dependent tables to remove FK constraints first.
    fn rebuild_table_without_fk(
        conn: &rusqlite::Connection,
        name: &str,
        ddl: &str,
    ) -> anyhow::Result<()> {
        let old = format!("{}_legacy", name);
        let _ = conn.execute(&format!("DROP TABLE IF EXISTS {}", old), []);
        conn.execute(&format!("ALTER TABLE {} RENAME TO {}", name, old), [])?;
        conn.execute(ddl, [])?;
        conn.execute(&format!("INSERT INTO {} SELECT * FROM {}", name, old), [])?;
        conn.execute(&format!("DROP TABLE {}", old), [])?;
        Ok(())
    }

    rebuild_table_without_fk(
        conn,
        "experiments",
        "CREATE TABLE experiments (id TEXT PRIMARY KEY, repo_id TEXT, paper_id TEXT, config_json TEXT, result_path TEXT, git_commit TEXT, syncthing_folder_id TEXT, status TEXT, timestamp TEXT NOT NULL)",
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_experiments_repo ON experiments(repo_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_experiments_paper ON experiments(paper_id)",
        [],
    )?;

    rebuild_table_without_fk(
        conn,
        "workflow_executions",
        "CREATE TABLE workflow_executions (id INTEGER PRIMARY KEY AUTOINCREMENT, workflow_id TEXT NOT NULL, inputs_json TEXT, status TEXT NOT NULL, current_step TEXT, started_at TEXT NOT NULL, finished_at TEXT, duration_ms INTEGER)",
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_workflow_execs_wf ON workflow_executions(workflow_id)", [])?;

    conn.execute("DROP TABLE IF EXISTS vault_notes", [])?;
    conn.execute("DROP TABLE IF EXISTS papers", [])?;
    conn.execute("DROP TABLE IF EXISTS workflows", [])?;
    conn.execute("PRAGMA user_version = 22", [])?;
    Ok(())
}
