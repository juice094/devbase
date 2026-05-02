use rusqlite::Connection;

pub fn run(conn: &Connection) -> anyhow::Result<()> {
    // v21: Drop repos table — entities is the single source of truth.
    // Recreate all child tables without FK constraints to repos.
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
        "repo_tags",
        "CREATE TABLE repo_tags (repo_id TEXT NOT NULL, tag TEXT NOT NULL, PRIMARY KEY (repo_id, tag))",
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_repo_tags_tag ON repo_tags(tag)", [])?;

    rebuild_table_without_fk(
        conn,
        "repo_remotes",
        "CREATE TABLE repo_remotes (repo_id TEXT NOT NULL, remote_name TEXT NOT NULL, upstream_url TEXT, default_branch TEXT, last_sync TEXT, PRIMARY KEY (repo_id, remote_name))",
    )?;

    rebuild_table_without_fk(
        conn,
        "repo_health",
        "CREATE TABLE repo_health (repo_id TEXT PRIMARY KEY, status TEXT, ahead INTEGER DEFAULT 0, behind INTEGER DEFAULT 0, checked_at TEXT)",
    )?;

    rebuild_table_without_fk(
        conn,
        "repo_stars_cache",
        "CREATE TABLE repo_stars_cache (repo_id TEXT PRIMARY KEY, stars INTEGER, fetched_at TEXT)",
    )?;

    rebuild_table_without_fk(
        conn,
        "repo_summaries",
        "CREATE TABLE repo_summaries (repo_id TEXT PRIMARY KEY, summary TEXT, keywords TEXT, generated_at TEXT)",
    )?;

    rebuild_table_without_fk(
        conn,
        "repo_relations",
        "CREATE TABLE repo_relations (from_repo_id TEXT NOT NULL, to_repo_id TEXT NOT NULL, relation_type TEXT NOT NULL, confidence REAL DEFAULT 0.0, discovered_at TEXT NOT NULL, PRIMARY KEY (from_repo_id, to_repo_id, relation_type))",
    )?;

    rebuild_table_without_fk(
        conn,
        "ai_discoveries",
        "CREATE TABLE ai_discoveries (id INTEGER PRIMARY KEY AUTOINCREMENT, repo_id TEXT, discovery_type TEXT, description TEXT, confidence REAL DEFAULT 0.0, timestamp TEXT NOT NULL)",
    )?;

    rebuild_table_without_fk(
        conn,
        "repo_notes",
        "CREATE TABLE repo_notes (id INTEGER PRIMARY KEY AUTOINCREMENT, repo_id TEXT NOT NULL, note_text TEXT NOT NULL, author TEXT DEFAULT 'ai', timestamp TEXT NOT NULL)",
    )?;

    rebuild_table_without_fk(
        conn,
        "experiments",
        "CREATE TABLE experiments (id TEXT PRIMARY KEY, repo_id TEXT, paper_id TEXT, config_json TEXT, result_path TEXT, git_commit TEXT, syncthing_folder_id TEXT, status TEXT, timestamp TEXT NOT NULL, FOREIGN KEY (paper_id) REFERENCES papers(id) ON DELETE SET NULL)",
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
        "workspace_snapshots",
        "CREATE TABLE workspace_snapshots (repo_id TEXT PRIMARY KEY, file_hash TEXT NOT NULL, checked_at TEXT NOT NULL)",
    )?;

    rebuild_table_without_fk(
        conn,
        "code_embeddings",
        "CREATE TABLE code_embeddings (repo_id TEXT NOT NULL, file_path TEXT NOT NULL DEFAULT '', symbol_name TEXT NOT NULL, embedding BLOB NOT NULL, generated_at TEXT NOT NULL, PRIMARY KEY (repo_id, file_path, symbol_name))",
    )?;

    conn.execute("DROP TABLE IF EXISTS repos", [])?;
    conn.execute("PRAGMA user_version = 21", [])?;
    Ok(())
}
