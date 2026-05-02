use super::*;
use crate::storage::StorageBackend;
use std::path::PathBuf;

pub const CURRENT_SCHEMA_VERSION: i32 = 29;

impl WorkspaceRegistry {
    pub fn db_path() -> anyhow::Result<PathBuf> {
        crate::storage::DefaultStorageBackend {}.db_path()
    }

    /// Workspace root directory where vault notes, assets, and repo manifests live.
    pub fn workspace_dir() -> anyhow::Result<PathBuf> {
        let ws = crate::storage::DefaultStorageBackend {}.workspace_dir()?;

        // P2-lite: create sample repos.toml if not exists
        let repos_toml = ws.join("repos.toml");
        if !repos_toml.exists() {
            let sample = r#"# Static repository configuration overrides.
# devbase auto-discovers repos, but you can declare tags/tier here.
#
# [[repo]]
# path = "devbase"
# tags = ["rust", "cli"]
# tier = "hot"
# workspace_type = "rust"
"#;
            let _ = std::fs::write(&repos_toml, sample);
        }

        Ok(ws)
    }

    pub fn init_db() -> anyhow::Result<rusqlite::Connection> {
        Self::init_db_at(&Self::db_path()?)
    }

    pub fn init_db_at(path: &std::path::Path) -> anyhow::Result<rusqlite::Connection> {
        let mut conn = rusqlite::Connection::open(path)?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        // Prevent TOCTOU races when multiple threads/processes open the same DB
        // concurrently (e.g. workflow executor's parallel step threads).
        conn.execute("BEGIN EXCLUSIVE", [])?;

        // Detect legacy schema: old repos table has upstream_url column
        let old_has_upstream = {
            let mut stmt = conn.prepare("PRAGMA table_info(repos)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.filter_map(Result::ok).any(|name| name == "upstream_url")
        };

        if old_has_upstream {
            let legacy_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='repos_legacy'",
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !legacy_exists {
                conn.execute("ALTER TABLE repos RENAME TO repos_legacy", [])?;
            }
        }

        // New normalized schema (v2: added workspace_type, data_tier, last_synced_at)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repos (
                id TEXT PRIMARY KEY,
                local_path TEXT NOT NULL,
                language TEXT,
                discovered_at TEXT NOT NULL,
                workspace_type TEXT DEFAULT 'git',
                data_tier TEXT DEFAULT 'private',
                last_synced_at TEXT,
                stars INTEGER
            )",
            [],
        )?;
        conn.execute("ALTER TABLE repos ADD COLUMN stars INTEGER", []).ok();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_tags (
                repo_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (repo_id, tag),
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_repo_tags_tag ON repo_tags(tag)", [])?;

        // One-time migration: tags from repos CSV to repo_tags
        let repos_has_tags = {
            let mut stmt = conn.prepare("PRAGMA table_info(repos)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.filter_map(Result::ok).any(|name| name == "tags")
        };
        if repos_has_tags {
            {
                let mut stmt = conn
                    .prepare("SELECT id, tags FROM repos WHERE tags IS NOT NULL AND tags != ''")?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (repo_id, tags_csv) = row?;
                    for tag in tags_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let _ = conn.execute(
                            "INSERT OR IGNORE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                            [&repo_id, tag],
                        );
                    }
                }
            }
            let _ = conn.execute("ALTER TABLE repos DROP COLUMN tags", []);
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_remotes (
                repo_id TEXT NOT NULL,
                remote_name TEXT NOT NULL,
                upstream_url TEXT,
                default_branch TEXT,
                last_sync TEXT,
                PRIMARY KEY (repo_id, remote_name),
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_health (
                repo_id TEXT PRIMARY KEY,
                status TEXT,
                ahead INTEGER DEFAULT 0,
                behind INTEGER DEFAULT 0,
                checked_at TEXT,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_stars_cache (
                repo_id TEXT PRIMARY KEY,
                stars INTEGER,
                fetched_at TEXT,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_stars_history (
                repo_id TEXT,
                stars INTEGER,
                fetched_at TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_stars_history_repo ON repo_stars_history(repo_id, fetched_at)",
            [],
        )?;

        // 共生知识库
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_summaries (
                repo_id TEXT PRIMARY KEY,
                summary TEXT,
                keywords TEXT,
                generated_at TEXT,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_modules (
                repo_id TEXT NOT NULL,
                module_path TEXT NOT NULL,
                public_apis TEXT,
                extracted_at TEXT,
                PRIMARY KEY (repo_id, module_path)
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_relations (
                from_repo_id TEXT NOT NULL,
                to_repo_id TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                confidence REAL DEFAULT 0.0,
                discovered_at TEXT NOT NULL,
                PRIMARY KEY (from_repo_id, to_repo_id, relation_type),
                FOREIGN KEY (from_repo_id) REFERENCES repos(id) ON DELETE CASCADE,
                FOREIGN KEY (to_repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 学习痕迹
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ai_discoveries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT,
                discovery_type TEXT,
                description TEXT,
                confidence REAL DEFAULT 0.0,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                note_text TEXT NOT NULL,
                author TEXT DEFAULT 'ai',
                timestamp TEXT NOT NULL,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Academic asset tracking
        conn.execute(
            "CREATE TABLE IF NOT EXISTS papers (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                authors TEXT,
                venue TEXT,
                year INTEGER,
                pdf_path TEXT,
                bibtex TEXT,
                tags TEXT,
                added_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_papers_venue ON papers(venue)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_papers_year ON papers(year)", [])?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS experiments (
                id TEXT PRIMARY KEY,
                repo_id TEXT,
                paper_id TEXT,
                config_json TEXT,
                result_path TEXT,
                git_commit TEXT,
                syncthing_folder_id TEXT,
                status TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE SET NULL,
                FOREIGN KEY (paper_id) REFERENCES papers(id) ON DELETE SET NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_experiments_repo ON experiments(repo_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_experiments_paper ON experiments(paper_id)",
            [],
        )?;

        // v3: workspace snapshots for non-git workspace change detection
        conn.execute(
            "CREATE TABLE IF NOT EXISTS workspace_snapshots (
                repo_id TEXT PRIMARY KEY,
                file_hash TEXT NOT NULL,
                checked_at TEXT NOT NULL,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // v4: operation log for tracking devbase actions
        conn.execute(
            "CREATE TABLE IF NOT EXISTS oplog (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                operation TEXT NOT NULL,
                repo_id TEXT,
                details TEXT,
                status TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_oplog_operation ON oplog(operation)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_oplog_timestamp ON oplog(timestamp)", [])?;

        // v11+v28: code embeddings for semantic vector search (3D PK since v28)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS code_embeddings (
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                embedding BLOB NOT NULL,
                generated_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, file_path, symbol_name),
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Schema versioning for future migrations
        run_migrations(&mut conn, path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS vault_repo_links (
                vault_id TEXT NOT NULL,
                repo_id TEXT NOT NULL,
                PRIMARY KEY (vault_id, repo_id)
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_code_metrics (
                repo_id TEXT PRIMARY KEY,
                total_lines INTEGER,
                source_lines INTEGER,
                test_lines INTEGER,
                comment_lines INTEGER,
                file_count INTEGER,
                language_breakdown TEXT,
                updated_at TEXT
            )",
            [],
        )?;

        // v9: semantic code symbols for AI-powered code queries
        conn.execute(
            "CREATE TABLE IF NOT EXISTS code_symbols (
                repo_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                symbol_type TEXT NOT NULL,
                name TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                signature TEXT,
                PRIMARY KEY (repo_id, file_path, name)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_code_symbols_repo ON code_symbols(repo_id)",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_code_symbols_name ON code_symbols(name)", [])?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_code_symbols_type ON code_symbols(symbol_type)",
            [],
        )?;

        // Migrate old repo_modules (used by knowledge_engine) to repo_modules_legacy if needed,
        // then create new repo_modules for cargo metadata indexing.
        let repo_modules_cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(repo_modules)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.filter_map(Result::ok).collect()
        };
        if repo_modules_cols.iter().any(|c| c == "module_path")
            && !repo_modules_cols.iter().any(|c| c == "module_name")
        {
            let _ = conn.execute("DROP TABLE IF EXISTS repo_modules_legacy", []);
            conn.execute("ALTER TABLE repo_modules RENAME TO repo_modules_legacy", [])?;
        }
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_modules (
                repo_id TEXT,
                module_name TEXT,
                module_type TEXT,
                module_path TEXT,
                PRIMARY KEY (repo_id, module_name)
            )",
            [],
        )?;

        // One-time migration from legacy table
        let legacy_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='repos_legacy'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if legacy_exists {
            let legacy_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM repos_legacy", [], |row| row.get(0))?;
            let remote_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM repo_remotes", [], |row| row.get(0))?;
            if legacy_count > 0 && remote_count == 0 {
                let mut stmt = conn.prepare(
                    "SELECT id, local_path, upstream_url, default_branch, tags, last_sync, discovered_at, language FROM repos_legacy"
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                })?;
                for row in rows {
                    let (
                        id,
                        local_path,
                        upstream_url,
                        default_branch,
                        tags,
                        last_sync,
                        discovered_at,
                        language,
                    ) = row?;
                    conn.execute(
                        "INSERT OR REPLACE INTO repos (id, local_path, language, discovered_at) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![&id, &local_path, language.as_deref(), &discovered_at],
                    )?;
                    if let Some(ref t) = tags {
                        for tag in t.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                            let _ = conn.execute(
                                "INSERT OR IGNORE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                                [&id, tag],
                            );
                        }
                    }
                    conn.execute(
                        "INSERT OR REPLACE INTO repo_remotes (repo_id, remote_name, upstream_url, default_branch, last_sync) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![&id, "origin", upstream_url.as_deref(), default_branch.as_deref(), last_sync.as_deref()],
                    )?;
                }
            }
        }

        // Ghost-table cleanup: repos is deprecated since v21.  It may be
        // recreated by the historical CREATE TABLE IF NOT EXISTS at the top
        // of this function, so drop it unconditionally at the end.
        let _ = conn.execute("DROP TABLE IF EXISTS repos", []);

        // Debug: explicit FK check before commit to surface any dangling constraints
        match conn.execute("PRAGMA foreign_key_check", []) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("FK check failed before COMMIT: {}", e);
                return Err(e.into());
            }
        }
        conn.execute("COMMIT", [])?;
        Ok(conn)
    }
}

fn run_migrations(conn: &mut rusqlite::Connection, path: &std::path::Path) -> anyhow::Result<()> {
    let user_version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if user_version > 0
        && user_version < CURRENT_SCHEMA_VERSION
        && path.exists()
        && let Err(e) = crate::backup::auto_backup_before_migration(path)
    {
        tracing::warn!("Failed to auto-backup registry before migration: {}", e);
    }
    crate::registry::migrations::run_all(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_path_format() {
        let path = WorkspaceRegistry::db_path().unwrap();
        let s = path.to_string_lossy();
        assert!(s.contains("devbase"), "db_path should contain 'devbase': {}", s);
        assert!(s.ends_with("registry.db"), "db_path should end with 'registry.db': {}", s);
    }

    #[test]
    fn test_workspace_dir_format() {
        let path = WorkspaceRegistry::workspace_dir().unwrap();
        let s = path.to_string_lossy();
        assert!(s.contains("devbase"), "workspace_dir should contain 'devbase': {}", s);
        assert!(s.ends_with("workspace"), "workspace_dir should end with 'workspace': {}", s);
    }
}
