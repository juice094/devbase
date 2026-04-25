use super::*;
use std::path::PathBuf;

impl WorkspaceRegistry {
    pub fn db_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
        let db_dir = data_dir.join("devbase");
        std::fs::create_dir_all(&db_dir)?;
        Ok(db_dir.join("registry.db"))
    }

    /// Workspace root directory where vault notes, assets, and repo manifests live.
    pub fn workspace_dir() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
        let ws = data_dir.join("devbase").join("workspace");
        std::fs::create_dir_all(&ws)?;
        std::fs::create_dir_all(ws.join("vault"))?;
        std::fs::create_dir_all(ws.join("assets"))?;

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

        // v11: code embeddings for semantic vector search
        conn.execute(
            "CREATE TABLE IF NOT EXISTS code_embeddings (
                repo_id TEXT NOT NULL,
                symbol_name TEXT NOT NULL,
                embedding BLOB NOT NULL,
                generated_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, symbol_name),
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Schema versioning for future migrations
        let user_version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        const CURRENT_SCHEMA_VERSION: i32 = 15;
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
        if user_version < 7 {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS vault_notes (
                    id TEXT PRIMARY KEY,
                    path TEXT NOT NULL,
                    title TEXT,
                    frontmatter TEXT,
                    tags TEXT,
                    outgoing_links TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_vault_notes_tags ON vault_notes(tags)",
                [],
            )?;
            conn.execute("PRAGMA user_version = 7", [])?;
        }
        if user_version < 8 {
            // Wave 9-3: drop content column from vault_notes (filesystem-first)
            let has_content: bool = conn
                .query_row(
                    "SELECT 1 FROM pragma_table_info('vault_notes') WHERE name = 'content'",
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if has_content {
                conn.execute(
                    "CREATE TABLE vault_notes_v2 (
                        id TEXT PRIMARY KEY,
                        path TEXT NOT NULL,
                        title TEXT,
                        frontmatter TEXT,
                        tags TEXT,
                        outgoing_links TEXT,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    )",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO vault_notes_v2 (id, path, title, frontmatter, tags, outgoing_links, created_at, updated_at)
                     SELECT id, path, title, frontmatter, tags, outgoing_links, created_at, updated_at FROM vault_notes",
                    [],
                )?;
                conn.execute("DROP TABLE vault_notes", [])?;
                conn.execute("ALTER TABLE vault_notes_v2 RENAME TO vault_notes", [])?;
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_vault_notes_tags ON vault_notes(tags)",
                    [],
                )?;
            }
            conn.execute("PRAGMA user_version = 8", [])?;
        }
        if user_version < 9 {
            // v9: semantic code symbols — already created above via CREATE TABLE IF NOT EXISTS
            conn.execute("PRAGMA user_version = 9", [])?;
        }
        if user_version < 10 {
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
        }
        if user_version < 11 {
            let ce_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_embeddings'",
                    [],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !ce_exists {
                conn.execute(
                    "CREATE TABLE code_embeddings (
                        repo_id TEXT NOT NULL,
                        symbol_name TEXT NOT NULL,
                        embedding BLOB NOT NULL,
                        generated_at TEXT NOT NULL,
                        PRIMARY KEY (repo_id, symbol_name),
                        FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
                    )",
                    [],
                )?;
            }
            conn.execute("PRAGMA user_version = 11", [])?;
        }
        if user_version < 12 {
            let cols: Vec<String> = {
                let mut stmt = conn.prepare("PRAGMA table_info(oplog)")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                rows.filter_map(Result::ok).collect()
            };
            if !cols.iter().any(|c| c == "event_type") {
                conn.execute("ALTER TABLE oplog ADD COLUMN event_type TEXT", [])?;
            }
            if !cols.iter().any(|c| c == "duration_ms") {
                conn.execute("ALTER TABLE oplog ADD COLUMN duration_ms INTEGER", [])?;
            }
            if !cols.iter().any(|c| c == "event_version") {
                conn.execute("ALTER TABLE oplog ADD COLUMN event_version INTEGER DEFAULT 1", [])?;
            }
            conn.execute(
                "UPDATE oplog SET event_type = CASE operation WHEN 'health' THEN 'health_check' ELSE operation END WHERE event_type IS NULL",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_oplog_event_type ON oplog(event_type)",
                [],
            )?;
            conn.execute("CREATE INDEX IF NOT EXISTS idx_oplog_repo ON oplog(repo_id)", [])?;
            conn.execute("PRAGMA user_version = 12", [])?;
        }
        if user_version < 13 {
            // v13: explicit symbol-to-symbol knowledge links
            conn.execute(
                "CREATE TABLE IF NOT EXISTS code_symbol_links (
                    source_repo TEXT NOT NULL,
                    source_symbol TEXT NOT NULL,
                    target_repo TEXT NOT NULL,
                    target_symbol TEXT NOT NULL,
                    link_type TEXT NOT NULL,
                    strength REAL NOT NULL DEFAULT 0.0,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (source_repo, source_symbol, target_repo, target_symbol, link_type)
                )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_symbol_links_source ON code_symbol_links(source_repo, source_symbol)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_symbol_links_target ON code_symbol_links(target_repo, target_symbol)",
                [],
            )?;
            conn.execute("PRAGMA user_version = 13", [])?;
        }
        if user_version < 14 {
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
        }
        if user_version < 15 {
            let cols: Vec<String> = {
                let mut stmt = conn.prepare("PRAGMA table_info(skills)")?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
                rows.filter_map(Result::ok).collect()
            };
            if !cols.iter().any(|c| c == "dependencies") {
                conn.execute("ALTER TABLE skills ADD COLUMN dependencies TEXT", [])?;
            }
            conn.execute("PRAGMA user_version = 15", [])?;
        }

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

        Ok(conn)
    }
}
