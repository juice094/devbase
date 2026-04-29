use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create a devbase CLI command with an isolated data directory.
fn devbase_cmd() -> (Command, TempDir) {
    let tmp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("devbase").unwrap();
    cmd.env("DEVBASE_DATA_DIR", tmp.path());
    (cmd, tmp)
}

#[test]
fn test_version() {
    let (mut cmd, _tmp) = devbase_cmd();
    cmd.arg("--version");
    let expected = env!("CARGO_PKG_VERSION");
    cmd.assert().success().stdout(predicate::str::contains(expected));
}

#[test]
fn test_health_empty_registry() {
    let (mut cmd, _tmp) = devbase_cmd();
    cmd.arg("health");
    cmd.assert().success().stdout(predicate::str::contains("total_repos: 0"));
}

#[test]
fn test_limit_list_empty() {
    let (mut cmd, _tmp) = devbase_cmd();
    cmd.args(["limit", "list"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("No known limits found."));
}

#[test]
fn test_skill_list_empty() {
    let (mut cmd, _tmp) = devbase_cmd();
    cmd.args(["skill", "list"]);
    cmd.assert().success().stdout(predicate::str::contains("No skills found."));
}

#[test]
fn test_registry_backups_empty() {
    let (mut cmd, _tmp) = devbase_cmd();
    cmd.args(["registry", "backups"]);
    cmd.assert().success().stdout(predicate::str::contains("没有找到备份"));
}

#[test]
fn test_limit_add_and_list() {
    let tmp = TempDir::new().unwrap();
    let mut cmd_add = Command::cargo_bin("devbase").unwrap();
    cmd_add.env("DEVBASE_DATA_DIR", tmp.path());
    cmd_add.args([
        "limit",
        "add",
        "test-limit-1",
        "--category",
        "known-bug",
        "--description",
        "A test limit",
    ]);
    cmd_add.assert().success();

    let mut cmd_list = Command::cargo_bin("devbase").unwrap();
    cmd_list.env("DEVBASE_DATA_DIR", tmp.path());
    cmd_list.args(["limit", "list", "--json"]);
    cmd_list.assert().success().stdout(predicate::str::contains("test-limit-1"));
}

#[test]
fn test_scan_git_repo() {
    let (mut cmd, tmp) = devbase_cmd();

    // Create a temporary git repo
    let repo_dir = tmp.path().join("test-repo");
    fs::create_dir(&repo_dir).unwrap();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    let sig = repo.signature().unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

    cmd.args(["scan", repo_dir.to_str().unwrap(), "--register"]);
    cmd.assert().success().stdout(predicate::str::contains("test-repo"));
}

#[test]
fn test_skill_discover() {
    let (mut cmd, tmp) = devbase_cmd();

    // Create a temporary skill project
    let skill_dir = tmp.path().join("my-skill");
    fs::create_dir(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
id: my-skill
name: My Skill
version: 1.0.0
description: A test skill
---

# Usage

Run the skill.
"#,
    )
    .unwrap();

    cmd.args(["skill", "discover", skill_dir.to_str().unwrap(), "--json"]);
    cmd.assert().success().stdout(predicate::str::contains("my-skill"));
}

#[test]
fn test_backup_export() {
    let tmp = TempDir::new().unwrap();
    let mut cmd_add = Command::cargo_bin("devbase").unwrap();
    cmd_add.env("DEVBASE_DATA_DIR", tmp.path());
    cmd_add.args([
        "limit",
        "add",
        "test-limit-backup",
        "--category",
        "known-bug",
        "--description",
        "For backup test",
    ]);
    cmd_add.assert().success();

    let mut cmd_export = Command::cargo_bin("devbase").unwrap();
    cmd_export.env("DEVBASE_DATA_DIR", tmp.path());
    cmd_export.args(["registry", "export", "--format", "sqlite"]);
    cmd_export.assert().success();
}

/// Helper: create a minimal git repo with one commit.
fn init_git_repo(path: &std::path::Path) {
    let repo = git2::Repository::init(path).unwrap();
    let sig = repo.signature().unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
}

#[test]
fn test_sync_skips_unmanaged_repo() {
    let tmp = TempDir::new().unwrap();

    // Create and register a git repo
    let repo_dir = tmp.path().join("unmanaged-repo");
    fs::create_dir(&repo_dir).unwrap();
    init_git_repo(&repo_dir);

    let mut scan = Command::cargo_bin("devbase").unwrap();
    scan.env("DEVBASE_DATA_DIR", tmp.path());
    scan.args(["scan", repo_dir.to_str().unwrap(), "--register"]);
    scan.assert().success();

    // Sync dry-run should skip the unmanaged repo
    let mut sync = Command::cargo_bin("devbase").unwrap();
    sync.env("DEVBASE_DATA_DIR", tmp.path());
    sync.args(["sync", "--dry-run"]);
    sync.assert().success().stdout(predicate::str::contains("devbase tag"));
}

#[test]
fn test_tag_enables_sync() {
    let tmp = TempDir::new().unwrap();

    // Create and register a git repo
    let repo_dir = tmp.path().join("managed-repo");
    fs::create_dir(&repo_dir).unwrap();
    init_git_repo(&repo_dir);

    let mut scan = Command::cargo_bin("devbase").unwrap();
    scan.env("DEVBASE_DATA_DIR", tmp.path());
    scan.args(["scan", repo_dir.to_str().unwrap(), "--register"]);
    scan.assert().success();

    // Tag as managed
    let mut tag = Command::cargo_bin("devbase").unwrap();
    tag.env("DEVBASE_DATA_DIR", tmp.path());
    tag.args(["tag", "managed-repo", "managed"]);
    tag.assert().success();

    // Sync dry-run should now evaluate the repo
    let mut sync = Command::cargo_bin("devbase").unwrap();
    sync.env("DEVBASE_DATA_DIR", tmp.path());
    sync.args(["sync", "--dry-run"]);
    sync.assert().success().stdout(predicate::str::contains("managed-repo"));
}
