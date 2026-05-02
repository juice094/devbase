use std::path::Path;

pub struct ChangedFiles {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// Get current HEAD commit hash (short form is fine, full OID string).
pub fn current_head_hash(repo_path: &Path) -> anyhow::Result<Option<String>> {
    let repo = git2::Repository::open(repo_path)?;
    let head = repo.head()?;
    Ok(head.target().map(|oid| oid.to_string()))
}

/// Diff between last indexed commit and current HEAD, plus uncommitted working-directory changes.
/// If `last_hash` is None, returns all tracked files as "added" (first-time index).
pub fn diff_since(
    repo_path: &Path,
    last_hash: Option<&str>,
) -> anyhow::Result<ChangedFiles> {
    let repo = git2::Repository::open(repo_path)?;
    let mut changed = ChangedFiles {
        added: Vec::new(),
        modified: Vec::new(),
        deleted: Vec::new(),
    };

    // 1. Diff between last indexed commit and HEAD
    let mut opts = git2::DiffOptions::new();
    let diff_commits = if let Some(old) = last_hash {
        let old_tree = repo.revparse_single(old)?.peel_to_tree()?;
        let head_tree = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_tree(Some(&old_tree), Some(&head_tree), Some(&mut opts))?
    } else {
        let head_tree = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_tree(None, Some(&head_tree), Some(&mut opts))?
    };
    collect_diff(&diff_commits, &mut changed);

    // 2. Diff between index and working directory (uncommitted changes)
    let mut opts2 = git2::DiffOptions::new();
    if let Ok(diff_workdir) = repo.diff_index_to_workdir(None, Some(&mut opts2)) {
        collect_diff(&diff_workdir, &mut changed);
    }

    // Deduplicate
    changed.added.sort_unstable();
    changed.added.dedup();
    changed.modified.sort_unstable();
    changed.modified.dedup();
    changed.deleted.sort_unstable();
    changed.deleted.dedup();

    Ok(changed)
}

fn collect_diff(diff: &git2::Diff, changed: &mut ChangedFiles) {
    let _ = diff.foreach(
        &mut |delta, _| {
            let old = delta.old_file().path().and_then(|p| p.to_str());
            let new = delta.new_file().path().and_then(|p| p.to_str());
            match delta.status() {
                git2::Delta::Added | git2::Delta::Untracked => {
                    if let Some(p) = new { changed.added.push(p.to_string()); }
                }
                git2::Delta::Modified | git2::Delta::Renamed => {
                    if let Some(p) = new { changed.modified.push(p.to_string()); }
                }
                git2::Delta::Deleted => {
                    if let Some(p) = old { changed.deleted.push(p.to_string()); }
                }
                _ => {}
            }
            true
        },
        None, None, None,
    );
}
