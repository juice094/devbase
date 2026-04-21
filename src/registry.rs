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
    pub workspace_type: String,
    pub data_tier: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub stars: Option<u64>,
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
pub struct VaultNote {
    pub id: String,
    pub path: String,
    pub title: Option<String>,
    pub content: String,
    pub frontmatter: Option<String>,
    pub tags: Vec<String>,
    pub outgoing_links: Vec<String>,
    pub linked_repo: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperEntry {
    pub id: String,
    pub title: String,
    pub authors: Option<String>,
    pub venue: Option<String>,
    pub year: Option<i32>,
    pub pdf_path: Option<String>,
    pub bibtex: Option<String>,
    pub tags: Vec<String>,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentEntry {
    pub id: String,
    pub repo_id: Option<String>,
    pub paper_id: Option<String>,
    pub config_json: Option<String>,
    pub result_path: Option<String>,
    pub git_commit: Option<String>,
    pub syncthing_folder_id: Option<String>,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub repo_id: String,
    pub file_hash: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OplogEntry {
    pub id: Option<i64>,
    pub operation: String,
    pub repo_id: Option<String>,
    pub details: Option<String>,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CodeMetrics {
    pub total_lines: usize,
    pub source_lines: usize,
    pub test_lines: usize,
    pub comment_lines: usize,
    pub file_count: usize,
    pub language_breakdown: serde_json::Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
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

mod core;
mod health;
mod knowledge;
mod metrics;
pub mod repos_toml;
mod workspace;

#[cfg(test)]
mod tests;
