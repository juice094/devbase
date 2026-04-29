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
    cmd.assert().success().stdout(predicate::str::contains("0.11.1"));
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
