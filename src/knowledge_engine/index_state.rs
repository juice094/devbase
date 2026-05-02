use crate::registry::RepoEntry;
use serde::Serialize;

/// High-level index freshness state for a repository.
/// Used by `devbase status` and MCP `devkit_status` to let Agents decide
/// whether re-indexing is needed without triggering a full index run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum IndexState {
    /// last_hash == HEAD and workdir is clean.
    Fresh,
    /// last_hash != HEAD or workdir has changes.
    Stale {
        added: Vec<String>,
        modified: Vec<String>,
        deleted: Vec<String>,
    },
    /// No prior index state (first-time index).
    Missing,
    /// Non-Git repo or read error.
    Unknown { reason: String },
}

impl IndexState {
    pub fn is_fresh(&self) -> bool {
        matches!(self, IndexState::Fresh)
    }

    pub fn changed_files_count(&self) -> usize {
        match self {
            IndexState::Stale { added, modified, deleted } => added.len() + modified.len() + deleted.len(),
            _ => 0,
        }
    }
}

/// Determine the index state of a single repository.
/// This is a read-only operation — it never modifies the database or the index.
pub fn get_repo_index_state(
    conn: &rusqlite::Connection,
    repo: &RepoEntry,
) -> IndexState {
    use tracing::warn;

    // 1. Ensure repo has a HEAD commit
    let _current_hash = match crate::semantic_index::git_diff::current_head_hash(&repo.local_path) {
        Ok(Some(h)) => h,
        Ok(None) => {
            return IndexState::Unknown {
                reason: "no HEAD commit (unborn branch)".into(),
            };
        }
        Err(e) => {
            return IndexState::Unknown {
                reason: format!("failed to read HEAD: {}", e),
            };
        }
    };

    // 2. Check for prior index state
    let last_hash = match get_last_indexed_hash(conn, &repo.id) {
        Ok(Some(h)) => h,
        Ok(None) => return IndexState::Missing,
        Err(e) => {
            warn!("Failed to read last indexed hash for {}: {}", repo.id, e);
            return IndexState::Unknown {
                reason: format!("failed to read repo_index_state: {}", e),
            };
        }
    };

    // 3. Diff since last indexed commit
    let changed = match crate::semantic_index::git_diff::diff_since(&repo.local_path, Some(&last_hash)) {
        Ok(c) => c,
        Err(e) => {
            return IndexState::Unknown {
                reason: format!("git diff failed: {}", e),
            };
        }
    };

    let total = changed.added.len() + changed.modified.len() + changed.deleted.len();
    if total == 0 {
        IndexState::Fresh
    } else {
        IndexState::Stale {
            added: changed.added,
            modified: changed.modified,
            deleted: changed.deleted,
        }
    }
}

fn get_last_indexed_hash(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Option<String>> {
    let row: Result<Option<String>, _> = conn.query_row(
        "SELECT last_commit_hash FROM repo_index_state WHERE repo_id = ?1",
        [repo_id],
        |row| row.get(0),
    );
    Ok(row.unwrap_or(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_state_variants_serialize() {
        let fresh = IndexState::Fresh;
        let json = serde_json::to_string(&fresh).unwrap();
        assert!(json.contains("\"state\":\"fresh\""));

        let stale = IndexState::Stale {
            added: vec!["a.rs".into()],
            modified: vec!["b.rs".into()],
            deleted: vec![],
        };
        let json = serde_json::to_string(&stale).unwrap();
        assert!(json.contains("\"state\":\"stale\""));
        assert!(json.contains("\"added\":["));

        let missing = IndexState::Missing;
        let json = serde_json::to_string(&missing).unwrap();
        assert!(json.contains("\"state\":\"missing\""));

        let unknown = IndexState::Unknown { reason: "x".into() };
        let json = serde_json::to_string(&unknown).unwrap();
        assert!(json.contains("\"state\":\"unknown\""));
        assert!(json.contains("\"reason\":\"x\""));
    }
}
