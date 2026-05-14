// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! Vault note history — Git-based revision tracking for Markdown notes.
//!
//! Requires the vault directory to be a Git repository. If not, returns empty
//! results gracefully.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub commit: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub message: String,
    pub insertions: usize,
    pub deletions: usize,
}

/// Get the commit history for a specific note file.
///
/// Returns entries from oldest to newest so that the caller can easily
/// compute adjacent diffs.
pub fn note_history(vault_dir: &Path, note_path: &str) -> anyhow::Result<Vec<HistoryEntry>> {
    let repo = match git2::Repository::open(vault_dir) {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()),
    };

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)?;

    let path = std::path::Path::new(note_path);
    let mut history = Vec::new();
    let mut last_relevant_tree: Option<git2::Tree> = None;

    fn count_lines(tree: &git2::Tree, path: &std::path::Path, repo: &git2::Repository) -> usize {
        tree.get_path(path)
            .ok()
            .and_then(|e| repo.find_blob(e.id()).ok())
            .map(|b| std::str::from_utf8(b.content()).unwrap_or("").lines().count())
            .unwrap_or(0)
    }

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let changed = if let Some(ref parent_tree) = last_relevant_tree {
            let mut opts = git2::DiffOptions::new();
            opts.pathspec(note_path);
            let diff = repo.diff_tree_to_tree(Some(parent_tree), Some(&tree), Some(&mut opts))?;
            diff.deltas().count() > 0
        } else {
            tree.get_path(path).is_ok()
        };

        if changed {
            let (insertions, deletions) =
                if let Some(ref parent_tree) = last_relevant_tree {
                    let old_lines = count_lines(parent_tree, path, &repo);
                    let new_lines = count_lines(&tree, path, &repo);
                    if new_lines >= old_lines {
                        (new_lines - old_lines, 0)
                    } else {
                        (0, old_lines - new_lines)
                    }
                } else {
                    (0, 0)
                };

            history.push(HistoryEntry {
                commit: oid.to_string(),
                author: commit.author().name().unwrap_or("").to_string(),
                email: commit.author().email().unwrap_or("").to_string(),
                timestamp: commit.time().seconds(),
                message: commit.message().unwrap_or("").trim().to_string(),
                insertions,
                deletions,
            });
            last_relevant_tree = Some(tree);
        }
    }

    Ok(history)
}

/// Get a text diff between two commits for a specific note.
pub fn note_diff(
    vault_dir: &Path,
    note_path: &str,
    old_commit: &str,
    new_commit: &str,
) -> anyhow::Result<String> {
    let repo = git2::Repository::open(vault_dir)?;
    let old = repo.revparse_single(old_commit)?.peel_to_tree()?;
    let new = repo.revparse_single(new_commit)?.peel_to_tree()?;

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(note_path);

    let diff = repo.diff_tree_to_tree(Some(&old), Some(&new), Some(&mut opts))?;

    let mut buf = Vec::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        buf.extend_from_slice(line.content());
        true
    })?;

    Ok(String::from_utf8_lossy(&buf).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_vault_git(tmp: &std::path::Path) -> git2::Repository {
        std::fs::create_dir_all(tmp).unwrap();
        let repo = git2::Repository::init(tmp).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        {
            let mut index = repo.index().unwrap();
            let file = tmp.join("note.md");
            std::fs::write(&file, "Hello world\n").unwrap();
            index.add_path(std::path::Path::new("note.md")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();
        }
        repo
    }

    #[test]
    fn test_note_history_basic() {
        let tmp = std::env::temp_dir().join(format!("devbase_vh_{}", std::process::id()));
        let repo = init_vault_git(&tmp);
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        // Second commit
        {
            let mut index = repo.index().unwrap();
            std::fs::write(tmp.join("note.md"), "Hello world\nMore lines\n").unwrap();
            index.add_path(std::path::Path::new("note.md")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add more lines",
                &tree,
                &[&parent],
            )
            .unwrap();
        }

        let history = note_history(&tmp, "note.md").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].message, "Initial commit");
        assert_eq!(history[1].message, "Add more lines");
        assert!(history[1].insertions > 0);
    }

    #[test]
    fn test_note_history_not_git() {
        let tmp = std::env::temp_dir().join(format!("devbase_vh_ng_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let history = note_history(&tmp, "note.md").unwrap();
        assert!(history.is_empty());
    }
}
