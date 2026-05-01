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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OplogEventType {
    Scan,
    Sync,
    Index,
    HealthCheck,
    KnownLimit,
}

impl OplogEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OplogEventType::Scan => "scan",
            OplogEventType::Sync => "sync",
            OplogEventType::Index => "index",
            OplogEventType::HealthCheck => "health_check",
            OplogEventType::KnownLimit => "known_limit",
        }
    }
}

impl std::str::FromStr for OplogEventType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "scan" => Ok(OplogEventType::Scan),
            "sync" => Ok(OplogEventType::Sync),
            "index" => Ok(OplogEventType::Index),
            "health_check" => Ok(OplogEventType::HealthCheck),
            "health" => Ok(OplogEventType::HealthCheck),
            "known_limit" => Ok(OplogEventType::KnownLimit),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OplogEntry {
    pub id: Option<i64>,
    pub event_type: OplogEventType,
    pub repo_id: Option<String>,
    pub details: Option<String>,
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub event_version: i32,
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

pub mod entity;
pub mod relation;

// Backward-compatible re-exports (migrated to entity.rs in v0.15).
pub use entity::{
    ENTITY_TYPE_PAPER, ENTITY_TYPE_REPO, ENTITY_TYPE_SKILL, ENTITY_TYPE_VAULT_NOTE,
    ENTITY_TYPE_WORKFLOW, upsert_entity,
};

pub mod health;
pub mod knowledge;
pub mod knowledge_meta;
pub mod known_limits;
pub mod links;
pub mod metrics;
mod migrate;
pub mod repo;
pub mod repos_toml;
pub mod vault;
pub mod call_graph;
pub mod workspace;

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests;
