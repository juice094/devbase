use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub remote_name: String,
    pub upstream_url: Option<String>,
    pub default_branch: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEntry {
    pub status: String,
    pub ahead: usize,
    pub behind: usize,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub id: String,
    pub local_path: PathBuf,
    pub tags: Vec<String>,
    pub discovered_at: DateTime<Utc>,
    pub language: Option<String>,
    pub remotes: Vec<RemoteEntry>,
}

impl RepoEntry {
    /// Return the 'origin' remote if present, otherwise the first remote.
    pub fn primary_remote(&self) -> Option<&RemoteEntry> {
        self.remotes
            .iter()
            .find(|r| r.remote_name == "origin")
            .or_else(|| self.remotes.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRegistry {
    pub version: String,
    pub entries: Vec<RepoEntry>,
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            entries: Vec::new(),
        }
    }
}

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
            let rows = stmt.query_map([], |row| Ok(row.get::<_, String>(1)?))?;
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

        // New normalized schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repos (
                id TEXT PRIMARY KEY,
                local_path TEXT NOT NULL,
                language TEXT,
                discovered_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS repo_tags (
                repo_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (repo_id, tag),
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_repo_tags_tag ON repo_tags(tag)",
            [],
        )?;

        // One-time migration: tags from repos CSV to repo_tags
        let repos_has_tags = {
            let mut stmt = conn.prepare("PRAGMA table_info(repos)")?;
            let rows = stmt.query_map([], |row| Ok(row.get::<_, String>(1)?))?;
            rows.filter_map(Result::ok).any(|name| name == "tags")
        };
        if repos_has_tags {
            {
                let mut stmt = conn.prepare("SELECT id, tags FROM repos WHERE tags IS NOT NULL AND tags != ''")?;
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
            "CREATE TABLE IF NOT EXISTS ai_queries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query_text TEXT NOT NULL,
                query_type TEXT,
                results_count INTEGER DEFAULT 0,
                top_result_ids TEXT,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;
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
                    let (id, local_path, upstream_url, default_branch, tags, last_sync, discovered_at, language) = row?;
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
                row.get::<_, Option<String>>(8)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, local_path, tags, language, discovered_at, remote_name, upstream_url, default_branch, last_sync) = row?;
            let local_path = PathBuf::from(local_path);
            let discovered_at = DateTime::parse_from_rfc3339(&discovered_at)?.with_timezone(&Utc);
            let tags: Vec<String> = tags
                .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default();
            let remote = remote_name.map(|name| RemoteEntry {
                remote_name: name,
                upstream_url,
                default_branch,
                last_sync: last_sync
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
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
                    rm.remote_name, rm.upstream_url, rm.default_branch, rm.last_sync
             FROM repos r
             LEFT JOIN repo_remotes rm ON r.id = rm.repo_id
             WHERE NOT EXISTS (
                 SELECT 1 FROM repo_summaries s WHERE s.repo_id = r.id
             ) OR EXISTS (
                 SELECT 1 FROM repo_summaries s WHERE s.repo_id = r.id AND s.generated_at < ?1
             )
             ORDER BY r.id, rm.remote_name"
        )?;
        Self::collect_repos_from_stmt(stmt, &[&threshold])
    }

    pub fn save_repo(conn: &mut rusqlite::Connection, repo: &RepoEntry) -> anyhow::Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO repos (id, local_path, language, discovered_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                &repo.id,
                repo.local_path.to_string_lossy().to_string(),
                repo.language.as_ref(),
                repo.discovered_at.to_rfc3339()
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

    pub fn save_health(
        conn: &rusqlite::Connection,
        repo_id: &str,
        health: &HealthEntry,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO repo_health (repo_id, status, ahead, behind, checked_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                repo_id,
                &health.status,
                health.ahead as i64,
                health.behind as i64,
                health.checked_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_health(
        conn: &rusqlite::Connection,
        repo_id: &str,
    ) -> anyhow::Result<Option<HealthEntry>> {
        let mut stmt =
            conn.prepare("SELECT status, ahead, behind, checked_at FROM repo_health WHERE repo_id = ?1")?;
        let mut rows = stmt.query([repo_id])?;
        if let Some(row) = rows.next()? {
            let status: String = row.get(0)?;
            let ahead: i64 = row.get(1)?;
            let behind: i64 = row.get(2)?;
            let checked_at: String = row.get(3)?;
            let checked_at = match DateTime::parse_from_rfc3339(&checked_at) {
                Ok(dt) => dt.with_timezone(&Utc),
                Err(_) => return Ok(None),
            };
            Ok(Some(HealthEntry {
                status,
                ahead: ahead as usize,
                behind: behind as usize,
                checked_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn save_summary(
        conn: &rusqlite::Connection,
        repo_id: &str,
        summary: &str,
        keywords: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO repo_summaries (repo_id, summary, keywords, generated_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![repo_id, summary, keywords, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn save_modules(
        conn: &mut rusqlite::Connection,
        repo_id: &str,
        modules: &[(String, String)],
    ) -> anyhow::Result<()> {
        let tx = conn.transaction()?;
        for (module_path, public_apis) in modules {
            tx.execute(
                "INSERT OR REPLACE INTO repo_modules (repo_id, module_path, public_apis, extracted_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![repo_id, module_path, public_apis, Utc::now().to_rfc3339()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn save_relation(
        conn: &rusqlite::Connection,
        from: &str,
        to: &str,
        rel_type: &str,
        confidence: f64,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO repo_relations (from_repo_id, to_repo_id, relation_type, confidence, discovered_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![from, to, rel_type, confidence, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn log_query(
        conn: &rusqlite::Connection,
        text: &str,
        qtype: &str,
        count: usize,
        top_ids: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT INTO ai_queries (query_text, query_type, results_count, top_result_ids, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![text, qtype, count as i64, top_ids, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn save_discovery(
        conn: &rusqlite::Connection,
        repo_id: Option<&str>,
        dtype: &str,
        desc: &str,
        confidence: f64,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT INTO ai_discoveries (repo_id, discovery_type, description, confidence, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![repo_id, dtype, desc, confidence, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn save_note(
        conn: &rusqlite::Connection,
        repo_id: &str,
        text: &str,
        author: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT INTO repo_notes (repo_id, note_text, author, timestamp) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![repo_id, text, author, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }
}
