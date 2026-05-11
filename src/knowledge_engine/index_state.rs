// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
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
            IndexState::Stale { added, modified, deleted } => {
                added.len() + modified.len() + deleted.len()
            }
            _ => 0,
        }
    }
}

/// Determine the index state of a single repository.
/// This is a read-only operation — it never modifies the database or the index.
pub fn get_repo_index_state(conn: &rusqlite::Connection, repo: &RepoEntry) -> IndexState {
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
    let changed =
        match crate::semantic_index::git_diff::diff_since(&repo.local_path, Some(&last_hash)) {
            Ok(c) => c,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("revspec") || msg.contains("not found") {
                    // Stale hash (e.g. after rebase) — clear it to trigger full re-index
                    let _ =
                        conn.execute("DELETE FROM repo_index_state WHERE repo_id = ?1", [&repo.id]);
                    return IndexState::Missing;
                }
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

    #[test]
    fn test_index_state_is_fresh_and_changed_count() {
        assert!(IndexState::Fresh.is_fresh());
        assert!(!IndexState::Missing.is_fresh());
        assert!(!IndexState::Unknown { reason: "err".into() }.is_fresh());

        let stale = IndexState::Stale {
            added: vec!["a.rs".into()],
            modified: vec!["b.rs".into(), "c.rs".into()],
            deleted: vec![],
        };
        assert_eq!(stale.changed_files_count(), 3);
        assert_eq!(IndexState::Fresh.changed_files_count(), 0);
        assert_eq!(IndexState::Missing.changed_files_count(), 0);
    }

    #[test]
    fn test_get_repo_index_state_missing() -> anyhow::Result<()> {
        use crate::registry::test_helpers::WorkspaceRegistry;
        let conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("repo");
        std::fs::create_dir(&path)?;
        let repo = git2::Repository::init(&path)?;
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])?;

        let entry = RepoEntry {
            id: "missing-repo".to_string(),
            local_path: path,
            tags: vec![],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };

        let state = get_repo_index_state(&conn, &entry);
        assert!(matches!(state, IndexState::Missing));
        Ok(())
    }

    #[test]
    fn test_get_repo_index_state_fresh() -> anyhow::Result<()> {
        use crate::registry::test_helpers::WorkspaceRegistry;
        let conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("repo");
        std::fs::create_dir(&path)?;
        let repo = git2::Repository::init(&path)?;
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])?;

        conn.execute(
            "INSERT INTO repo_index_state (repo_id, last_commit_hash, indexed_at) VALUES (?1, ?2, datetime('now'))",
            ["fresh-repo", &oid.to_string()],
        )?;

        let entry = RepoEntry {
            id: "fresh-repo".to_string(),
            local_path: path,
            tags: vec![],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };

        let state = get_repo_index_state(&conn, &entry);
        assert!(matches!(state, IndexState::Fresh));
        Ok(())
    }

    #[test]
    fn test_get_repo_index_state_stale() -> anyhow::Result<()> {
        use crate::registry::test_helpers::WorkspaceRegistry;
        let conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("repo");
        std::fs::create_dir(&path)?;
        let repo = git2::Repository::init(&path)?;
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        let sig = repo.signature().unwrap();

        // First commit
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let old_oid = repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])?;
        drop(tree);

        // Save the first commit hash as last indexed
        conn.execute(
            "INSERT INTO repo_index_state (repo_id, last_commit_hash, indexed_at) VALUES (?1, ?2, datetime('now'))",
            ["stale-repo", &old_oid.to_string()],
        )?;

        // Second commit so HEAD moves forward and diff_since detects changes
        let tree_id2 = {
            let mut index = repo.index().unwrap();
            // Add a dummy file to create a new tree
            let blob_oid = repo.blob(b"hello")?;
            index.add_frombuffer(
                &git2::IndexEntry {
                    ctime: git2::IndexTime::new(0, 0),
                    mtime: git2::IndexTime::new(0, 0),
                    dev: 0,
                    ino: 0,
                    mode: 0o100644,
                    uid: 0,
                    gid: 0,
                    file_size: 5,
                    id: blob_oid,
                    flags: 0,
                    flags_extended: 0,
                    path: b"file.txt".to_vec(),
                },
                b"hello",
            )?;
            index.write_tree().unwrap()
        };
        let parent = repo.find_commit(old_oid)?;
        let tree2 = repo.find_tree(tree_id2).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "second", &tree2, &[&parent])?;
        drop(tree2);

        let entry = RepoEntry {
            id: "stale-repo".to_string(),
            local_path: path,
            tags: vec![],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };

        let state = get_repo_index_state(&conn, &entry);
        assert!(matches!(state, IndexState::Stale { .. }), "expected Stale, got {:?}", state);
        Ok(())
    }

    #[test]
    fn test_get_repo_index_state_unknown_not_git() -> anyhow::Result<()> {
        use crate::registry::test_helpers::WorkspaceRegistry;
        let conn = WorkspaceRegistry::init_in_memory()?;
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("not-git");
        std::fs::create_dir(&path)?;

        let entry = RepoEntry {
            id: "unknown-repo".to_string(),
            local_path: path,
            tags: vec![],
            language: Some("rust".to_string()),
            discovered_at: chrono::Utc::now(),
            workspace_type: "git".to_string(),
            data_tier: "private".to_string(),
            last_synced_at: None,
            stars: None,
            remotes: vec![],
        };

        let state = get_repo_index_state(&conn, &entry);
        assert!(matches!(state, IndexState::Unknown { .. }), "expected Unknown, got {:?}", state);
        Ok(())
    }
}
