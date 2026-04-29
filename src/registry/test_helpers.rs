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

    /// Seed a minimal test repo into the registry so that FK-dependent tables can be tested.
    pub fn seed_test_repo(conn: &mut rusqlite::Connection, id: &str) -> anyhow::Result<RepoEntry> {
        let repo = RepoEntry {
            id: id.to_string(),
            local_path: std::path::PathBuf::from(format!("/tmp/{}", id)),
            tags: vec![],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };
        Self::save_repo(conn, &repo)?;
        Ok(repo)
    }
}

#[cfg(test)]
const SCHEMA_DDL: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS repo_tags (
    repo_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (repo_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_repo_tags_tag ON repo_tags(tag);

CREATE TABLE IF NOT EXISTS repo_remotes (
    repo_id TEXT NOT NULL,
    remote_name TEXT NOT NULL,
    upstream_url TEXT,
    default_branch TEXT,
    last_sync TEXT,
    PRIMARY KEY (repo_id, remote_name)
);

CREATE TABLE IF NOT EXISTS repo_health (
    repo_id TEXT PRIMARY KEY,
    status TEXT,
    ahead INTEGER DEFAULT 0,
    behind INTEGER DEFAULT 0,
    checked_at TEXT
);

CREATE TABLE IF NOT EXISTS repo_stars_cache (
    repo_id TEXT PRIMARY KEY,
    stars INTEGER,
    fetched_at TEXT
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
    generated_at TEXT
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
    PRIMARY KEY (from_repo_id, to_repo_id, relation_type)
);

CREATE TABLE IF NOT EXISTS ai_discoveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT,
    discovery_type TEXT,
    description TEXT,
    confidence REAL DEFAULT 0.0,
    timestamp TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS repo_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT NOT NULL,
    note_text TEXT NOT NULL,
    author TEXT DEFAULT 'ai',
    timestamp TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS experiments (
    id TEXT PRIMARY KEY,
    repo_id TEXT,
    paper_id TEXT,
    config_json TEXT,
    result_path TEXT,
    git_commit TEXT,
    syncthing_folder_id TEXT,
    status TEXT,
    timestamp TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_experiments_repo ON experiments(repo_id);
CREATE INDEX IF NOT EXISTS idx_experiments_paper ON experiments(paper_id);

CREATE TABLE IF NOT EXISTS workspace_snapshots (
    repo_id TEXT PRIMARY KEY,
    file_hash TEXT NOT NULL,
    checked_at TEXT NOT NULL
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

-- v18: Known Limits (L3 risk layer)
CREATE TABLE IF NOT EXISTS known_limits (
    id              TEXT PRIMARY KEY,
    category        TEXT NOT NULL,
    description     TEXT NOT NULL,
    source          TEXT,
    severity        INTEGER,
    first_seen_at   TEXT NOT NULL,
    last_checked_at TEXT,
    mitigated       INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_known_limits_category ON known_limits(category);
CREATE INDEX IF NOT EXISTS idx_known_limits_mitigated ON known_limits(mitigated);

-- v19: Knowledge Meta (L4 metacognition layer)
CREATE TABLE IF NOT EXISTS knowledge_meta (
    id              TEXT PRIMARY KEY,
    target_level    INTEGER NOT NULL,
    target_id       TEXT NOT NULL,
    correction_type TEXT,
    correction_json TEXT,
    confidence      REAL DEFAULT 0.0,
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_knowledge_meta_target ON knowledge_meta(target_level, target_id);

CREATE TABLE IF NOT EXISTS code_symbols (
    repo_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    symbol_type TEXT NOT NULL,
    name TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    signature TEXT,
    PRIMARY KEY (repo_id, file_path, name)
);
CREATE INDEX IF NOT EXISTS idx_code_symbols_repo ON code_symbols(repo_id);
CREATE INDEX IF NOT EXISTS idx_code_symbols_name ON code_symbols(name);
CREATE INDEX IF NOT EXISTS idx_code_symbols_type ON code_symbols(symbol_type);

CREATE TABLE IF NOT EXISTS code_call_graph (
    repo_id TEXT NOT NULL,
    caller_file TEXT NOT NULL,
    caller_symbol TEXT NOT NULL,
    caller_line INTEGER,
    callee_name TEXT NOT NULL
);
CREATE INDEX idx_call_graph_repo ON code_call_graph(repo_id);
CREATE INDEX idx_call_graph_callee ON code_call_graph(callee_name);
CREATE INDEX idx_call_graph_caller ON code_call_graph(repo_id, caller_file, caller_symbol);

CREATE TABLE IF NOT EXISTS code_embeddings (
    repo_id TEXT NOT NULL,
    symbol_name TEXT NOT NULL,
    embedding BLOB NOT NULL,
    generated_at TEXT NOT NULL,
    PRIMARY KEY (repo_id, symbol_name)
);

CREATE TABLE IF NOT EXISTS code_symbol_links (
    source_repo TEXT NOT NULL,
    source_symbol TEXT NOT NULL,
    target_repo TEXT NOT NULL,
    target_symbol TEXT NOT NULL,
    link_type TEXT NOT NULL,
    strength REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (source_repo, source_symbol, target_repo, target_symbol, link_type)
);
CREATE INDEX IF NOT EXISTS idx_symbol_links_source ON code_symbol_links(source_repo, source_symbol);
CREATE INDEX IF NOT EXISTS idx_symbol_links_target ON code_symbol_links(target_repo, target_symbol);
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

    #[test]
    fn test_known_limits_table_exists() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='known_limits'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "known_limits table must exist in current schema");
    }

    #[test]
    fn test_knowledge_meta_table_exists() {
        let conn = WorkspaceRegistry::init_in_memory().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='knowledge_meta'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "knowledge_meta table must exist in current schema");
    }
}
