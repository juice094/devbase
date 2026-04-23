use super::*;

#[cfg(test)]
impl WorkspaceRegistry {
    /// Create an in-memory SQLite connection with the full current schema.
    /// This is faster than file-based tests and leaves no artifacts.
    pub fn init_in_memory() -> anyhow::Result<rusqlite::Connection> {
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_DDL)?;
        Ok(conn)
    }
}

#[cfg(test)]
const SCHEMA_DDL: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS repos (
    id TEXT PRIMARY KEY,
    local_path TEXT NOT NULL,
    language TEXT,
    discovered_at TEXT NOT NULL,
    workspace_type TEXT DEFAULT 'git',
    data_tier TEXT DEFAULT 'private',
    last_synced_at TEXT,
    stars INTEGER
);

CREATE TABLE IF NOT EXISTS repo_tags (
    repo_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (repo_id, tag),
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_repo_tags_tag ON repo_tags(tag);

CREATE TABLE IF NOT EXISTS repo_remotes (
    repo_id TEXT NOT NULL,
    remote_name TEXT NOT NULL,
    upstream_url TEXT,
    default_branch TEXT,
    last_sync TEXT,
    PRIMARY KEY (repo_id, remote_name),
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS repo_health (
    repo_id TEXT PRIMARY KEY,
    status TEXT,
    ahead INTEGER DEFAULT 0,
    behind INTEGER DEFAULT 0,
    checked_at TEXT,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS repo_stars_cache (
    repo_id TEXT PRIMARY KEY,
    stars INTEGER,
    fetched_at TEXT,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS repo_stars_history (
    repo_id TEXT,
    stars INTEGER,
    fetched_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_stars_history_repo ON repo_stars_history(repo_id, fetched_at);

CREATE TABLE IF NOT EXISTS repo_summaries (
    repo_id TEXT PRIMARY KEY,
    summary TEXT,
    keywords TEXT,
    generated_at TEXT,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS repo_modules (
    repo_id TEXT,
    module_name TEXT,
    module_type TEXT,
    module_path TEXT,
    PRIMARY KEY (repo_id, module_name)
);

CREATE TABLE IF NOT EXISTS repo_relations (
    from_repo_id TEXT NOT NULL,
    to_repo_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    confidence REAL DEFAULT 0.0,
    discovered_at TEXT NOT NULL,
    PRIMARY KEY (from_repo_id, to_repo_id, relation_type),
    FOREIGN KEY (from_repo_id) REFERENCES repos(id) ON DELETE CASCADE,
    FOREIGN KEY (to_repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ai_discoveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT,
    discovery_type TEXT,
    description TEXT,
    confidence REAL DEFAULT 0.0,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS repo_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT NOT NULL,
    note_text TEXT NOT NULL,
    author TEXT DEFAULT 'ai',
    timestamp TEXT NOT NULL,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS papers (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    authors TEXT,
    venue TEXT,
    year INTEGER,
    pdf_path TEXT,
    bibtex TEXT,
    tags TEXT,
    added_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_papers_venue ON papers(venue);
CREATE INDEX IF NOT EXISTS idx_papers_year ON papers(year);

CREATE TABLE IF NOT EXISTS experiments (
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
);
CREATE INDEX IF NOT EXISTS idx_experiments_repo ON experiments(repo_id);
CREATE INDEX IF NOT EXISTS idx_experiments_paper ON experiments(paper_id);

CREATE TABLE IF NOT EXISTS workspace_snapshots (
    repo_id TEXT PRIMARY KEY,
    file_hash TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS oplog (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation TEXT NOT NULL,
    repo_id TEXT,
    details TEXT,
    status TEXT NOT NULL,
    timestamp TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_oplog_operation ON oplog(operation);
CREATE INDEX IF NOT EXISTS idx_oplog_timestamp ON oplog(timestamp);

CREATE TABLE IF NOT EXISTS repo_code_metrics (
    repo_id TEXT PRIMARY KEY,
    total_lines INTEGER,
    source_lines INTEGER,
    test_lines INTEGER,
    comment_lines INTEGER,
    file_count INTEGER,
    language_breakdown TEXT,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS vault_notes (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    title TEXT,
    frontmatter TEXT,
    tags TEXT,
    outgoing_links TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vault_notes_tags ON vault_notes(tags);

CREATE TABLE IF NOT EXISTS vault_repo_links (
    vault_id TEXT NOT NULL,
    repo_id TEXT NOT NULL,
    PRIMARY KEY (vault_id, repo_id)
);
"#;
