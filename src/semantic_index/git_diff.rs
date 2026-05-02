use std::path::Path;

pub struct ChangedFiles {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// Get current HEAD commit hash (short form is fine, full OID string).
pub fn current_head_hash(repo_path: &Path) -> anyhow::Result<Option<String>> {
    let repo = git2::Repository::open(repo_path)?;
    match repo.head() {
        Ok(head) => Ok(head.target().map(|oid| oid.to_string())),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => Ok(None),
        Err(e) => Err(e.into()),
    }
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
    opts2.include_untracked(true);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn init_repo(path: &Path) -> git2::Repository {
        let repo = git2::Repository::init(path).unwrap();
        // Configure git user for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        repo
    }

    fn commit_file(repo: &git2::Repository, path: &str, content: &str) -> String {
        let repo_path = repo.workdir().unwrap();
        let file_path = repo_path.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        drop(file);

        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.head().ok().and_then(|h| h.target()).and_then(|oid| repo.find_commit(oid).ok());
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        let commit_id = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("Add {}", path),
            &tree,
            &parents,
        ).unwrap();
        commit_id.to_string()
    }

    #[test]
    fn test_current_head_hash_empty_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let _repo = init_repo(tmp.path());
        // No commits yet
        assert!(current_head_hash(tmp.path()).unwrap().is_none());
    }

    #[test]
    fn test_current_head_hash_after_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let hash = commit_file(&repo, "src/main.rs", "fn main() {}");
        let head = current_head_hash(tmp.path()).unwrap();
        assert_eq!(head, Some(hash));
    }

    #[test]
    fn test_diff_since_none_first_index() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        commit_file(&repo, "src/main.rs", "fn main() {}");
        commit_file(&repo, "src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }");

        let changed = diff_since(tmp.path(), None).unwrap();
        assert_eq!(changed.added.len(), 2);
        assert!(changed.added.contains(&"src/main.rs".to_string()));
        assert!(changed.added.contains(&"src/lib.rs".to_string()));
        assert!(changed.modified.is_empty());
        assert!(changed.deleted.is_empty());
    }

    #[test]
    fn test_diff_since_with_last_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let first_hash = commit_file(&repo, "src/main.rs", "fn main() {}");
        let _second_hash = commit_file(&repo, "src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }");

        let changed = diff_since(tmp.path(), Some(&first_hash)).unwrap();
        assert_eq!(changed.added.len(), 1);
        assert!(changed.added.contains(&"src/lib.rs".to_string()));
        assert!(changed.modified.is_empty());
        assert!(changed.deleted.is_empty());
    }

    #[test]
    fn test_diff_since_workdir_modification() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let hash = commit_file(&repo, "src/main.rs", "fn main() {}");

        // Modify file in working directory without committing
        let file_path = tmp.path().join("src/main.rs");
        let mut file = std::fs::OpenOptions::new().write(true).truncate(true).open(&file_path).unwrap();
        file.write_all(b"fn main() { println!(\"hello\"); }").unwrap();
        drop(file);

        let changed = diff_since(tmp.path(), Some(&hash)).unwrap();
        assert!(changed.added.is_empty());
        assert_eq!(changed.modified.len(), 1);
        assert!(changed.modified.contains(&"src/main.rs".to_string()));
        assert!(changed.deleted.is_empty());
    }

    #[test]
    fn test_diff_since_workdir_untracked() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let hash = commit_file(&repo, "src/main.rs", "fn main() {}");

        // Create untracked file
        let file_path = tmp.path().join("src/new.rs");
        std::fs::write(&file_path, "pub fn new() {}").unwrap();

        let changed = diff_since(tmp.path(), Some(&hash)).unwrap();
        assert_eq!(changed.added.len(), 1);
        assert!(changed.added.contains(&"src/new.rs".to_string()));
        assert!(changed.modified.is_empty());
        assert!(changed.deleted.is_empty());
    }

    #[test]
    fn test_diff_since_no_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let hash = commit_file(&repo, "src/main.rs", "fn main() {}");

        let changed = diff_since(tmp.path(), Some(&hash)).unwrap();
        assert!(changed.added.is_empty());
        assert!(changed.modified.is_empty());
        assert!(changed.deleted.is_empty());
    }

    #[test]
    fn test_diff_since_deleted_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let first_hash = commit_file(&repo, "src/main.rs", "fn main() {}");
        let _second_hash = commit_file(&repo, "src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }");

        // Delete main.rs in a new commit
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new("src/main.rs")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.find_commit(repo.revparse_single("HEAD").unwrap().id()).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig, &sig,
            "Remove main.rs",
            &tree,
            &[&parent],
        ).unwrap();

        let changed = diff_since(tmp.path(), Some(&first_hash)).unwrap();
        // src/lib.rs was added between first_hash and HEAD (plus possibly other untracked files in workdir)
        assert!(changed.added.contains(&"src/lib.rs".to_string()));
        assert!(changed.modified.is_empty());
        assert!(changed.deleted.contains(&"src/main.rs".to_string()));
    }
}
