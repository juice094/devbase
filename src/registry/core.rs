use super::*;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

impl WorkspaceRegistry {
    pub fn db_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
        let db_dir = data_dir.join("devbase");
        std::fs::create_dir_all(&db_dir)?;
        Ok(db_dir.join("registry.db"))
    }

    pub fn init_db() -> anyhow::Result<rusqlite::Connection> {
        let path = Self::db_path()?;
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;

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
                PRIMARY KEY (repo_id, module_path),
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
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

        // Schema versioning for future migrations
        let user_version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        const CURRENT_SCHEMA_VERSION: i32 = 6;
        if user_version < CURRENT_SCHEMA_VERSION
            && path.exists()
            && let Err(e) = crate::backup::auto_backup_before_migration(&path)
        {
            tracing::warn!("Failed to auto-backup registry before migration: {}", e);
        }
        if user_version < 1 {
            conn.execute("PRAGMA user_version = 1", [])?;
        }
        if user_version < 2 {
            let cols = {
                let mut stmt = conn.prepare("PRAGMA table_info(repos)")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                rows.filter_map(Result::ok).collect::<Vec<_>>()
            };
            if !cols.iter().any(|c| c == "workspace_type") {
                conn.execute("ALTER TABLE repos ADD COLUMN workspace_type TEXT DEFAULT 'git'", [])?;
            }
            if !cols.iter().any(|c| c == "data_tier") {
                conn.execute("ALTER TABLE repos ADD COLUMN data_tier TEXT DEFAULT 'private'", [])?;
            }
            if !cols.iter().any(|c| c == "last_synced_at") {
                conn.execute("ALTER TABLE repos ADD COLUMN last_synced_at TEXT", [])?;
            }
            conn.execute("PRAGMA user_version = 2", [])?;
        }
        if user_version < 3 {
            let snapshots_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='workspace_snapshots'",
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !snapshots_exists {
                conn.execute(
                    "CREATE TABLE workspace_snapshots (
                        repo_id TEXT PRIMARY KEY,
                        file_hash TEXT NOT NULL,
                        checked_at TEXT NOT NULL,
                        FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
                    )",
                    [],
                )?;
            }
            conn.execute("PRAGMA user_version = 3", [])?;
        }
        if user_version < 4 {
            let oplog_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='oplog'",
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !oplog_exists {
                conn.execute(
                    "CREATE TABLE oplog (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        operation TEXT NOT NULL,
                        repo_id TEXT,
                        details TEXT,
                        status TEXT NOT NULL,
                        timestamp TEXT NOT NULL
                    )",
                    [],
                )?;
                conn.execute("CREATE INDEX idx_oplog_operation ON oplog(operation)", [])?;
                conn.execute("CREATE INDEX idx_oplog_timestamp ON oplog(timestamp)", [])?;
            }
            conn.execute("PRAGMA user_version = 4", [])?;
        }
        if user_version < 5 {
            conn.execute("PRAGMA user_version = 5", [])?;
        }
        if user_version < 6 {
            // Drop unused tables from earlier schema versions
            let _ = conn.execute("DROP TABLE IF EXISTS ai_queries", []);
            let _ = conn.execute("DROP TABLE IF EXISTS agri_observations", []);
            conn.execute("PRAGMA user_version = 6", [])?;
        }

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

        Ok(conn)
    }

    fn collect_repos_from_stmt(
        mut stmt: rusqlite::Statement<'_>,
        params: &[&dyn rusqlite::ToSql],
    ) -> anyhow::Result<Vec<RepoEntry>> {
        let rows = stmt.query_map(params, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (
                id,
                local_path,
                tags,
                language,
                discovered_at,
                workspace_type,
                data_tier,
                last_synced_at,
                stars,
                remote_name,
                upstream_url,
                default_branch,
                last_sync,
            ) = row?;
            let local_path = PathBuf::from(local_path);
            let discovered_at = DateTime::parse_from_rfc3339(&discovered_at)?.with_timezone(&Utc);
            let tags: Vec<String> = tags
                .map(|s| {
                    s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
                })
                .unwrap_or_default();
            let workspace_type = workspace_type.unwrap_or_else(|| "git".to_string());
            let data_tier = data_tier.unwrap_or_else(|| "private".to_string());
            let last_synced_at = last_synced_at.and_then(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            });
            let stars = stars.map(|s| s as u64);
            let remote = remote_name.map(|name| RemoteEntry {
                remote_name: name,
                upstream_url,
                default_branch,
                last_sync: last_sync.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
                }),
            });
            if let Some(entry) = entries.last_mut().filter(|e: &&mut RepoEntry| e.id == id) {
                if let Some(r) = remote {
                    entry.remotes.push(r);
                }
            } else {
                let mut remotes = Vec::new();
                if let Some(r) = remote {
                    remotes.push(r);
                }
                entries.push(RepoEntry {
                    id,
                    local_path,
                    tags,
                    language,
                    workspace_type,
                    data_tier,
                    last_synced_at,
                    stars,
                    discovered_at,
                    remotes,
                });
            }
        }
        Ok(entries)
    }

    pub fn list_repos(conn: &rusqlite::Connection) -> anyhow::Result<Vec<RepoEntry>> {
        let stmt = conn.prepare(
            "SELECT r.id, r.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = r.id) as tags, r.language, r.discovered_at,
                    r.workspace_type, r.data_tier, r.last_synced_at, r.stars,
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM repos r
             LEFT JOIN repo_remotes rm ON r.id = rm.repo_id
             ORDER BY r.id, rm.remote_name"
        )?;
        Self::collect_repos_from_stmt(stmt, &[])
    }

    pub fn list_repos_stale_health(
        conn: &rusqlite::Connection,
        threshold: &str,
    ) -> anyhow::Result<Vec<RepoEntry>> {
        let stmt = conn.prepare(
            "SELECT r.id, r.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = r.id) as tags, r.language, r.discovered_at,
                    r.workspace_type, r.data_tier, r.last_synced_at, r.stars,
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM repos r
             LEFT JOIN repo_remotes rm ON r.id = rm.repo_id
             WHERE NOT EXISTS (
                 SELECT 1 FROM repo_health h WHERE h.repo_id = r.id
             ) OR EXISTS (
                 SELECT 1 FROM repo_health h WHERE h.repo_id = r.id AND h.checked_at < ?1
             )
             ORDER BY r.id, rm.remote_name"
        )?;
        Self::collect_repos_from_stmt(stmt, &[&threshold])
    }

    pub fn list_repos_need_index(
        conn: &rusqlite::Connection,
        threshold: &str,
    ) -> anyhow::Result<Vec<RepoEntry>> {
        let stmt = conn.prepare(
            "SELECT r.id, r.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = r.id) as tags, r.language, r.discovered_at,
                    r.workspace_type, r.data_tier, r.last_synced_at, r.stars,
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM repos r
             LEFT JOIN repo_remotes rm ON r.id = rm.repo_id
             WHERE NOT EXISTS (
                 SELECT 1 FROM repo_summaries s WHERE s.repo_id = r.id
             ) OR EXISTS (
                 SELECT 1 FROM repo_summaries s WHERE s.repo_id = r.id AND s.generated_at < ?1
             ) OR r.language IS NULL
             ORDER BY r.id, rm.remote_name"
        )?;
        Self::collect_repos_from_stmt(stmt, &[&threshold])
    }

    pub fn save_repo(conn: &mut rusqlite::Connection, repo: &RepoEntry) -> anyhow::Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO repos (id, local_path, language, discovered_at, workspace_type, data_tier, last_synced_at, stars) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &repo.id,
                repo.local_path.to_string_lossy().to_string(),
                repo.language.as_ref(),
                repo.discovered_at.to_rfc3339(),
                &repo.workspace_type,
                &repo.data_tier,
                repo.last_synced_at.map(|dt| dt.to_rfc3339()),
                repo.stars.map(|s| s as i64)
            ],
        )?;
        tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1", [&repo.id])?;
        for tag in &repo.tags {
            tx.execute(
                "INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                rusqlite::params![&repo.id, tag],
            )?;
        }
        tx.execute("DELETE FROM repo_remotes WHERE repo_id = ?1", [&repo.id])?;
        for remote in &repo.remotes {
            tx.execute(
                "INSERT INTO repo_remotes (repo_id, remote_name, upstream_url, default_branch, last_sync) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    &repo.id,
                    &remote.remote_name,
                    remote.upstream_url.as_ref(),
                    remote.default_branch.as_ref(),
                    remote.last_sync.map(|dt| dt.to_rfc3339())
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_repo_language(
        conn: &rusqlite::Connection,
        repo_id: &str,
        language: Option<&str>,
    ) -> anyhow::Result<()> {
        conn.execute(
            "UPDATE repos SET language = ?1 WHERE id = ?2",
            rusqlite::params![language, repo_id],
        )?;
        Ok(())
    }

    pub fn update_repo_tier(
        conn: &rusqlite::Connection,
        repo_id: &str,
        tier: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "UPDATE repos SET data_tier = ?1 WHERE id = ?2",
            rusqlite::params![tier, repo_id],
        )?;
        Ok(())
    }

    pub fn update_repo_workspace_type(
        conn: &rusqlite::Connection,
        repo_id: &str,
        workspace_type: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "UPDATE repos SET workspace_type = ?1 WHERE id = ?2",
            rusqlite::params![workspace_type, repo_id],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn update_repo_last_synced_at(
        conn: &rusqlite::Connection,
        repo_id: &str,
        timestamp: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        conn.execute(
            "UPDATE repos SET last_synced_at = ?1 WHERE id = ?2",
            rusqlite::params![timestamp.to_rfc3339(), repo_id],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn list_workspaces_by_tier(
        conn: &rusqlite::Connection,
        tier: &str,
    ) -> anyhow::Result<Vec<RepoEntry>> {
        let stmt = conn.prepare(
            "SELECT r.id, r.local_path, (SELECT group_concat(tag, ',') FROM repo_tags WHERE repo_id = r.id) as tags, r.language, r.discovered_at,
                    r.workspace_type, r.data_tier, r.last_synced_at, r.stars,
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM repos r
             LEFT JOIN repo_remotes rm ON r.id = rm.repo_id
             WHERE r.data_tier = ?1
             ORDER BY r.id, rm.remote_name"
        )?;
        Self::collect_repos_from_stmt(stmt, &[&tier])
    }
}
