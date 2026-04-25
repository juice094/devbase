use super::*;

#[cfg(test)]
impl WorkspaceRegistry {
    /// Create an in-memory SQLite connection with the full current schema.
    /// This is faster than file-based tests and leaves no artifacts.
    pub fn init_in_memory() -> anyhow::Result<rusqlite::Connection> {
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_DDL)?;
        conn.execute(
            &format!("PRAGMA user_version = {}", crate::registry::migrate::CURRENT_SCHEMA_VERSION),
            [],
        )?;
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
    timestamp TEXT NOT NULL,
    event_type TEXT,
    duration_ms INTEGER,
    event_version INTEGER DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_oplog_operation ON oplog(operation);
CREATE INDEX IF NOT EXISTS idx_oplog_timestamp ON oplog(timestamp);
CREATE INDEX IF NOT EXISTS idx_oplog_event_type ON oplog(event_type);
CREATE INDEX IF NOT EXISTS idx_oplog_repo ON oplog(repo_id);

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

CREATE TABLE IF NOT EXISTS skills (
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
    dependencies    TEXT,
    embedding       BLOB,
    installed_at    TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    last_used_at    TEXT,
    category        TEXT,
    success_rate    REAL,
    usage_count     INTEGER DEFAULT 0,
    rating          REAL
);
CREATE INDEX IF NOT EXISTS idx_skills_type ON skills(skill_type);

CREATE TABLE IF NOT EXISTS skill_executions (
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
);

-- v16: Unified Entity Model
CREATE TABLE IF NOT EXISTS entity_types (
    name            TEXT PRIMARY KEY,
    schema_json     TEXT NOT NULL,
    description     TEXT,
    created_at      TEXT NOT NULL
);
INSERT OR IGNORE INTO entity_types (name, schema_json, description, created_at) VALUES
    ('repo', '{"fields":[]}', 'Git repository', '2024-01-01T00:00:00Z'),
    ('skill', '{"fields":[]}', 'Executable Skill', '2024-01-01T00:00:00Z'),
    ('paper', '{"fields":[]}', 'Academic paper', '2024-01-01T00:00:00Z'),
    ('vault_note', '{"fields":[]}', 'Vault markdown note', '2024-01-01T00:00:00Z'),
    ('workflow', '{"fields":[]}', 'Workflow definition', '2024-01-01T00:00:00Z');

CREATE TABLE IF NOT EXISTS entities (
    id              TEXT PRIMARY KEY,
    entity_type     TEXT NOT NULL REFERENCES entity_types(name),
    name            TEXT NOT NULL,
    source_url      TEXT,
    local_path      TEXT,
    metadata        TEXT,
    content_hash    TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);
CREATE INDEX IF NOT EXISTS idx_entities_source ON entities(source_url);

CREATE TABLE IF NOT EXISTS relations (
    id              TEXT PRIMARY KEY,
    from_entity_id  TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity_id    TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation_type   TEXT NOT NULL,
    metadata        TEXT,
    confidence      REAL NOT NULL DEFAULT 1.0,
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_relations_type ON relations(relation_type);

-- v0.5.0 reserved: Workflow Engine
CREATE TABLE IF NOT EXISTS workflows (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    version         TEXT NOT NULL,
    description     TEXT,
    definition_yaml TEXT NOT NULL,
    status          TEXT DEFAULT 'draft',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workflow_executions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id     TEXT NOT NULL,
    inputs_json     TEXT,
    status          TEXT NOT NULL,
    current_step    TEXT,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    duration_ms     INTEGER
);
"#;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_schema_version() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        assert_eq!(version, crate::registry::migrate::CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_workflows_table_exists() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='workflows'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "workflows table must exist in current schema");
    }

    #[test]
    fn test_workflow_executions_table_exists() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='workflow_executions'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "workflow_executions table must exist in current schema");
    }
}
