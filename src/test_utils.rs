use crate::registry::{RepoEntry, WorkspaceRegistry};
use chrono::Utc;
use std::path::PathBuf;

/// Create an in-memory SQLite connection with the full devbase schema.
pub fn temp_db() -> rusqlite::Connection {
    WorkspaceRegistry::init_in_memory().expect("failed to create in-memory db")
}

/// Build a minimal RepoEntry fixture for tests.
pub fn fixture_repo(id: &str, local_path: &str) -> RepoEntry {
    RepoEntry {
        id: id.to_string(),
        local_path: PathBuf::from(local_path),
        tags: vec![],
        discovered_at: Utc::now(),
        language: None,
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: vec![],
    }
}

/// Build a RepoEntry fixture with tags.
pub fn fixture_repo_with_tags(id: &str, local_path: &str, tags: &[&str]) -> RepoEntry {
    RepoEntry {
        id: id.to_string(),
        local_path: PathBuf::from(local_path),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        discovered_at: Utc::now(),
        language: None,
        workspace_type: "git".to_string(),
        data_tier: "private".to_string(),
        last_synced_at: None,
        stars: None,
        remotes: vec![],
    }
}
