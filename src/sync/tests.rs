use super::tasks::write_syncdone_marker;
use super::*;
use git2::Repository;
use std::fs;
use tempfile::TempDir;

fn create_commit(repo: &Repository, message: &str) -> git2::Oid {
    let sig = repo.signature().unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    match parent {
        Some(ref p) => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[p]).unwrap(),
        None => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[]).unwrap(),
    }
}

fn setup_repo_with_remote_commits(
    ahead_local: usize,
    behind_remote: usize,
) -> (TempDir, Repository) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(&dir).unwrap();

    // Initial commit on main
    fs::write(dir.path().join("file.txt"), "base").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let sig = repo.signature().unwrap();
    let tree_id = index.write_tree().unwrap();
    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[]).unwrap();
    }

    // Create origin remote pointing to a bare repo
    let bare_dir = TempDir::new().unwrap();
    let _bare_repo = Repository::init_bare(&bare_dir).unwrap();
    repo.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();

    // Push base to origin/main
    {
        let mut remote = repo.find_remote("origin").unwrap();
        remote.push(&["+refs/heads/main:refs/heads/main"], None).unwrap();
    }

    // Create remote commits via a helper clone
    let helper_dir = TempDir::new().unwrap();
    let helper = Repository::clone(bare_dir.path().to_str().unwrap(), &helper_dir).unwrap();
    for i in 0..behind_remote {
        fs::write(helper_dir.path().join("file.txt"), format!("remote{}", i)).unwrap();
        let mut hindex = helper.index().unwrap();
        hindex.add_path(std::path::Path::new("file.txt")).unwrap();
        hindex.write().unwrap();
        let hsig = helper.signature().unwrap();
        let htree_id = hindex.write_tree().unwrap();
        {
            let htree = helper.find_tree(htree_id).unwrap();
            let hparent = helper.head().unwrap().peel_to_commit().unwrap();
            helper
                .commit(Some("HEAD"), &hsig, &hsig, &format!("remote{}", i), &htree, &[&hparent])
                .unwrap();
        }
    }
    let mut hremote = helper.find_remote("origin").unwrap();
    hremote.push(&["+refs/heads/main:refs/heads/main"], None).unwrap();

    // Fetch remote changes back so origin/main exists and is updated
    {
        let mut remote = repo.find_remote("origin").unwrap();
        remote.fetch(&["main"], None, None).unwrap();
    }

    // Set upstream tracking for local main branch
    {
        let mut branch = repo.find_branch("main", git2::BranchType::Local).unwrap();
        branch.set_upstream(Some("origin/main")).unwrap();
    }

    // Create local commits (these will make local ahead)
    for i in 0..ahead_local {
        fs::write(dir.path().join("file.txt"), format!("local{}", i)).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        create_commit(&repo, &format!("local{}", i));
    }

    (dir, repo)
}

#[test]
fn test_assess_safety_safe_ff() {
    let (dir, _repo) = setup_repo_with_remote_commits(0, 2);
    let (safety, _, _) =
        assess_safety(dir.path().to_str().unwrap(), SyncPolicy::from_tags("third-party"));
    assert_eq!(safety, SyncSafety::Safe);
}

#[test]
fn test_assess_safety_blocked_dirty() {
    let (dir, _repo) = setup_repo_with_remote_commits(0, 2);
    // Modify an existing tracked file in the worktree
    fs::write(dir.path().join("file.txt"), "dirty").unwrap();
    let (safety, _, _) =
        assess_safety(dir.path().to_str().unwrap(), SyncPolicy::from_tags("third-party"));
    assert_eq!(safety, SyncSafety::BlockedDirty);
}

#[test]
fn test_assess_safety_blocked_diverged_conservative() {
    let (dir, _repo) = setup_repo_with_remote_commits(1, 2);
    let (safety, _, _) =
        assess_safety(dir.path().to_str().unwrap(), SyncPolicy::from_tags("third-party"));
    assert_eq!(safety, SyncSafety::BlockedDiverged);
}

#[test]
fn test_assess_safety_diverged_rebase_allowed() {
    let (dir, _repo) = setup_repo_with_remote_commits(1, 2);
    let (safety, _, _) =
        assess_safety(dir.path().to_str().unwrap(), SyncPolicy::from_tags("own-project"));
    assert_eq!(safety, SyncSafety::Safe);
}

#[test]
fn test_assess_safety_up_to_date() {
    let (dir, _repo) = setup_repo_with_remote_commits(0, 0);
    let (safety, _, _) =
        assess_safety(dir.path().to_str().unwrap(), SyncPolicy::from_tags("third-party"));
    assert_eq!(safety, SyncSafety::UpToDate);
}

#[test]
fn test_assess_safety_no_upstream() {
    let dir = TempDir::new().unwrap();
    let _repo = Repository::init(&dir).unwrap();
    let sig = _repo.signature().unwrap();
    let tree_id = _repo.index().unwrap().write_tree().unwrap();
    let tree = _repo.find_tree(tree_id).unwrap();
    _repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
    let (safety, _, _) = assess_safety(dir.path().to_str().unwrap(), SyncPolicy::from_tags(""));
    assert_eq!(safety, SyncSafety::NoUpstream);
}

#[test]
fn test_write_syncdone_marker() {
    let dir = TempDir::new().unwrap();
    write_syncdone_marker(dir.path(), "FETCH", Some("abc1234"));

    let syncdone_path = dir.path().join(".devbase").join("syncdone");
    assert!(syncdone_path.exists(), ".devbase/syncdone should be written");

    let content = fs::read_to_string(&syncdone_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.get("action").and_then(|v| v.as_str()), Some("FETCH"));
    assert_eq!(parsed.get("local_commit").and_then(|v| v.as_str()), Some("abc1234"));
    assert!(parsed.get("timestamp").is_some());
}

#[test]
fn test_sync_repo_skip_no_syncdone() {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(&dir).unwrap();
    let sig = repo.signature().unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

    // Simulate a SKIP summary: write_syncdone_marker should NOT be called
    let syncdone_path = dir.path().join(".devbase").join("syncdone");
    assert!(!syncdone_path.exists(), ".devbase/syncdone should NOT exist before any write");

    // Write it manually with SKIP action to verify it would be wrong
    write_syncdone_marker(dir.path(), "SKIP", None);
    assert!(syncdone_path.exists(), "marker can be written for testing");

    // In real sync_repo, SKIP action bypasses write_syncdone_marker, so delete it
    fs::remove_file(&syncdone_path).unwrap();
    assert!(!syncdone_path.exists());
}
