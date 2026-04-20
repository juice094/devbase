use crate::registry::{HealthEntry, OplogEntry, WorkspaceRegistry, WorkspaceSnapshot};
use chrono::Utc;
use git2::Repository;
use std::path::Path;
use tracing::info;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    ".tmp",
    ".cache",
    ".bun",
    ".cargo",
    ".rustup",
];

pub fn compute_workspace_hash(root: &Path) -> anyhow::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut files = Vec::new();
    if root.is_dir() {
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.split('/').any(|part| IGNORED_DIRS.contains(&part)) {
                continue;
            }
            files.push(rel_str);
        }
    }
    files.sort();
    for rel in files {
        hasher.update(rel.as_bytes());
        let full = root.join(&rel);
        if let Ok(bytes) = std::fs::read(&full) {
            hasher.update(&bytes);
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub async fn run_json(
    detail: bool,
    limit: usize,
    page: usize,
    ttl_seconds: i64,
) -> anyhow::Result<serde_json::Value> {
    let (total_repos, dirty_repos, behind_upstream, no_upstream_count, repo_details) = {
        let conn = WorkspaceRegistry::init_db()?;
        let repos = WorkspaceRegistry::list_repos(&conn)?;

        let mut total_repos: usize = 0;
        let mut dirty_repos: usize = 0;
        let mut behind_upstream: usize = 0;
        let mut no_upstream_count: usize = 0;
        let mut repo_details: Vec<serde_json::Value> = Vec::new();

        for repo in repos {
            total_repos += 1;
            let primary = repo.primary_remote();
            let upstream_url = primary.and_then(|r| r.upstream_url.clone());
            let default_branch = primary.and_then(|r| r.default_branch.clone());

            let (status, ahead, behind) = if repo.workspace_type == "git" {
                match WorkspaceRegistry::get_health(&conn, &repo.id) {
                    Ok(Some(health)) => {
                        let elapsed =
                            Utc::now().signed_duration_since(health.checked_at).num_seconds();
                        if elapsed < ttl_seconds {
                            (health.status, health.ahead, health.behind)
                        } else {
                            let (status, ahead, behind) = analyze_repo(
                                repo.local_path.to_string_lossy().as_ref(),
                                upstream_url.as_deref(),
                                default_branch.as_deref(),
                            );
                            let new_health = HealthEntry {
                                status: status.clone(),
                                ahead,
                                behind,
                                checked_at: Utc::now(),
                            };
                            if let Err(e) =
                                WorkspaceRegistry::save_health(&conn, &repo.id, &new_health)
                            {
                                tracing::warn!("Failed to save health for {}: {}", repo.id, e);
                            }
                            (status, ahead, behind)
                        }
                    }
                    _ => {
                        let (status, ahead, behind) = analyze_repo(
                            repo.local_path.to_string_lossy().as_ref(),
                            upstream_url.as_deref(),
                            default_branch.as_deref(),
                        );
                        let new_health = HealthEntry {
                            status: status.clone(),
                            ahead,
                            behind,
                            checked_at: Utc::now(),
                        };
                        if let Err(e) = WorkspaceRegistry::save_health(&conn, &repo.id, &new_health)
                        {
                            tracing::warn!("Failed to save health for {}: {}", repo.id, e);
                        }
                        (status, ahead, behind)
                    }
                }
            } else {
                // Non-git workspace: use file-hash snapshot
                let current_hash = match compute_workspace_hash(&repo.local_path) {
                    Ok(h) => h,
                    Err(_) => {
                        repo_details.push(serde_json::json!({
                            "id": repo.id,
                            "local_path": repo.local_path,
                            "upstream_url": upstream_url,
                            "default_branch": default_branch,
                            "status": "error",
                            "ahead": 0,
                            "behind": 0,
                            "workspace_type": repo.workspace_type,
                            "data_tier": repo.data_tier
                        }));
                        continue;
                    }
                };
                let status = match WorkspaceRegistry::get_latest_workspace_snapshot(&conn, &repo.id)
                {
                    Ok(Some(prev)) if prev.file_hash == current_hash => "ok".to_string(),
                    _ => {
                        let snapshot = WorkspaceSnapshot {
                            repo_id: repo.id.clone(),
                            file_hash: current_hash,
                            checked_at: Utc::now(),
                        };
                        if let Err(e) = WorkspaceRegistry::save_workspace_snapshot(&conn, &snapshot)
                        {
                            tracing::warn!(
                                "Failed to save workspace snapshot for {}: {}",
                                repo.id,
                                e
                            );
                        }
                        "changed".to_string()
                    }
                };
                (status, 0, 0)
            };

            match status.as_str() {
                "dirty" | "changed" => dirty_repos += 1,
                "behind" => behind_upstream += 1,
                "no_upstream" => no_upstream_count += 1,
                _ => {}
            }

            repo_details.push(serde_json::json!({
                "id": repo.id,
                "local_path": repo.local_path,
                "upstream_url": upstream_url,
                "default_branch": default_branch,
                "status": status,
                "ahead": ahead,
                "behind": behind,
                "workspace_type": repo.workspace_type,
                "data_tier": repo.data_tier
            }));
        }

        (total_repos, dirty_repos, behind_upstream, no_upstream_count, repo_details)
    };

    let environment = serde_json::json!({
        "rustc": get_tool_version("rustc", &["--version"]).await.map(|s| fmt_version(Some(s))),
        "cargo": get_tool_version("cargo", &["--version"]).await.map(|s| fmt_version(Some(s))),
        "node": get_tool_version("node", &["--version"]).await.map(|s| fmt_version(Some(s))),
        "go": get_tool_version("go", &["version"]).await.map(|s| fmt_version(Some(s))),
        "cmake": get_tool_version("cmake", &["--version"]).await.map(|s| fmt_version(Some(s))),
    });

    let summary = serde_json::json!({
        "total_repos": total_repos,
        "dirty_repos": dirty_repos,
        "behind_upstream": behind_upstream,
        "no_upstream": no_upstream_count
    });

    info!("Health check completed");

    // Log to oplog
    if let Ok(conn) = WorkspaceRegistry::init_db() {
        let _ = WorkspaceRegistry::save_oplog(
            &conn,
            &OplogEntry {
                id: None,
                operation: "health".to_string(),
                repo_id: None,
                details: Some(format!(
                    "repos={}, dirty={}, behind={}",
                    total_repos, dirty_repos, behind_upstream
                )),
                status: "success".to_string(),
                timestamp: Utc::now(),
            },
        );
    }

    let total_repos_detail = repo_details.len();
    let paged_repos = if detail && limit > 0 {
        let start = (page.saturating_sub(1)) * limit;
        repo_details.into_iter().skip(start).take(limit).collect()
    } else {
        repo_details
    };

    Ok(serde_json::json!({
        "success": true,
        "summary": summary,
        "environment": environment,
        "pagination": if limit > 0 {
            serde_json::json!({
                "total": total_repos_detail,
                "page": page,
                "limit": limit,
                "has_more": total_repos_detail > page * limit
            })
        } else {
            serde_json::Value::Null
        },
        "repos": if detail { paged_repos } else { vec![] }
    }))
}

pub async fn run(detail: bool, limit: usize, page: usize, ttl_seconds: i64) -> anyhow::Result<()> {
    let result = run_json(detail, limit, page, ttl_seconds).await?;

    let summary = result["summary"].as_object().unwrap();
    println!("{}:", crate::i18n::current().log.health_summary);
    println!("  total_repos: {}", summary["total_repos"].as_u64().unwrap_or(0));
    println!("  dirty_repos: {}", summary["dirty_repos"].as_u64().unwrap_or(0));
    println!("  behind_upstream: {}", summary["behind_upstream"].as_u64().unwrap_or(0));
    println!("  no_upstream: {}", summary["no_upstream"].as_u64().unwrap_or(0));

    let env = result["environment"].as_object().unwrap();
    println!("\n{}:", crate::i18n::current().log.health_environment);
    println!(
        "  rustc: {}",
        env["rustc"].as_str().unwrap_or(crate::i18n::current().log.not_installed)
    );
    println!(
        "  cargo: {}",
        env["cargo"].as_str().unwrap_or(crate::i18n::current().log.not_installed)
    );
    println!(
        "  node: {}",
        env["node"].as_str().unwrap_or(crate::i18n::current().log.not_installed)
    );
    println!(
        "  go: {}",
        env["go"].as_str().unwrap_or(crate::i18n::current().log.not_installed)
    );
    println!(
        "  cmake: {}",
        env["cmake"].as_str().unwrap_or(crate::i18n::current().log.not_installed)
    );

    if detail {
        let repos = result["repos"].as_array().unwrap();
        if !repos.is_empty() {
            if let Some(pagination) = result.get("pagination") {
                if pagination != &serde_json::Value::Null {
                    let total = pagination["total"].as_u64().unwrap_or(0);
                    let page_num = pagination["page"].as_u64().unwrap_or(1);
                    let limit_val = pagination["limit"].as_u64().unwrap_or(0);
                    let has_more = pagination["has_more"].as_bool().unwrap_or(false);
                    println!(
                        "\n{} (page {} of ~{}, limit={}):",
                        crate::i18n::current().log.health_repos,
                        page_num,
                        (total as f64 / limit_val as f64).ceil() as u64,
                        limit_val
                    );
                    if has_more {
                        println!("  (more results available, use --page {})", page_num + 1);
                    }
                } else {
                    println!("\n{}:", crate::i18n::current().log.health_repos);
                }
            } else {
                println!("\n{}:", crate::i18n::current().log.health_repos);
            }
            for repo in repos {
                let id = repo["id"].as_str().unwrap_or("");
                let path = repo["local_path"].as_str().unwrap_or("");
                let upstream = repo["upstream_url"].as_str().unwrap_or("none");
                let branch = repo["default_branch"].as_str().unwrap_or("unknown");
                let status = repo["status"].as_str().unwrap_or("");
                let ahead = repo["ahead"].as_u64().unwrap_or(0);
                let behind = repo["behind"].as_u64().unwrap_or(0);
                let workspace_type = repo["workspace_type"].as_str().unwrap_or("git");
                let data_tier = repo["data_tier"].as_str().unwrap_or("private");
                println!(
                    "  [{}] status={} | ahead={} | behind={} | tier={} | type={} | path={} | upstream={} | branch={}",
                    id, status, ahead, behind, data_tier, workspace_type, path, upstream, branch
                );
            }
        }
    }

    Ok(())
}

pub fn analyze_repo(
    path: &str,
    upstream_url: Option<&str>,
    default_branch: Option<&str>,
) -> (String, usize, usize) {
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(_) => return ("error".to_string(), 0, 0),
    };

    let dirty = match repo.statuses(None) {
        Ok(statuses) => statuses.iter().count() > 0,
        Err(_) => false,
    };

    if upstream_url.map(|u| u.trim().is_empty()).unwrap_or(true) {
        if dirty {
            return ("dirty".to_string(), 0, 0);
        }
        return ("no_upstream".to_string(), 0, 0);
    }

    // Check for detached HEAD
    let is_detached = match repo.head() {
        Ok(head) => head.target().is_none(),
        Err(_) => true,
    };

    if is_detached {
        return ("detached".to_string(), 0, 0);
    }

    let (ahead, behind) = match calc_ahead_behind(&repo, default_branch) {
        Ok(ab) => ab,
        Err(_) => return ("ok".to_string(), 0, 0),
    };

    let status = if dirty {
        "dirty"
    } else if ahead > 0 && behind > 0 {
        "diverged"
    } else if ahead > 0 {
        "ahead"
    } else if behind > 0 {
        "behind"
    } else {
        "ok"
    };

    (status.to_string(), ahead, behind)
}

fn calc_ahead_behind(
    repo: &Repository,
    default_branch: Option<&str>,
) -> anyhow::Result<(usize, usize)> {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok((0, 0)),
    };

    let local_oid = match head.target() {
        Some(oid) => oid,
        None => return Ok((0, 0)),
    };

    let upstream_names: Vec<String> = match git2::Branch::wrap(head).upstream() {
        Ok(up) => match up.name() {
            Ok(Some(name)) => vec![name.to_string()],
            _ => vec!["origin/HEAD".to_string()],
        },
        Err(_) => {
            let branch = default_branch.unwrap_or("HEAD");
            vec![format!("origin/{}", branch), "origin/HEAD".to_string()]
        }
    };

    let upstream_ref = upstream_names.iter().find_map(|name| repo.find_reference(name).ok());

    let upstream_ref = match upstream_ref {
        Some(r) => r,
        None => return Ok((0, 0)),
    };

    let remote_oid = match upstream_ref.target() {
        Some(oid) => oid,
        None => return Ok((0, 0)),
    };

    repo.graph_ahead_behind(local_oid, remote_oid).map_err(|e| anyhow::anyhow!(e))
}

async fn get_tool_version(cmd: &str, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new(cmd).args(args).output().await.ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let line = raw.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    Some(line.to_string())
}

fn fmt_version(raw: Option<String>) -> String {
    match raw {
        Some(s) => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                match parts[0] {
                    "rustc" | "cargo" => parts.get(1).unwrap_or(&"unknown").to_string(),
                    "cmake" | "version" => parts.get(2).unwrap_or(&"unknown").to_string(),
                    _ => {
                        if parts[0] == "go" && parts.len() >= 3 {
                            parts[2].to_string()
                        } else {
                            s
                        }
                    }
                }
            } else {
                s
            }
        }
        None => crate::i18n::current().log.not_installed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_workspace_hash_empty_dir() {
        let tmp = std::env::temp_dir().join(format!("devbase_health_empty_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let hash1 = compute_workspace_hash(&tmp).unwrap();
        let hash2 = compute_workspace_hash(&tmp).unwrap();
        assert_eq!(hash1, hash2, "same empty dir should produce same hash");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn test_compute_workspace_hash_changes_with_content() {
        let tmp =
            std::env::temp_dir().join(format!("devbase_health_content_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("a.txt"), "hello").unwrap();
        let hash1 = compute_workspace_hash(&tmp).unwrap();

        std::fs::write(tmp.join("a.txt"), "world").unwrap();
        let hash2 = compute_workspace_hash(&tmp).unwrap();

        assert_ne!(hash1, hash2, "changing file content should change hash");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn test_compute_workspace_hash_ignores_dirs() {
        let tmp =
            std::env::temp_dir().join(format!("devbase_health_ignore_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::create_dir_all(tmp.join(".git")).unwrap();
        std::fs::write(tmp.join(".git").join("config"), "x").unwrap();
        std::fs::write(tmp.join("real.txt"), "y").unwrap();

        let hash = compute_workspace_hash(&tmp).unwrap();
        // If .git was ignored, hash should be based only on real.txt
        let tmp2 =
            std::env::temp_dir().join(format!("devbase_health_ignore2_{}", std::process::id()));
        std::fs::create_dir_all(&tmp2).unwrap();
        std::fs::write(tmp2.join("real.txt"), "y").unwrap();
        let hash2 = compute_workspace_hash(&tmp2).unwrap();

        assert_eq!(hash, hash2, ".git contents should be ignored");
        std::fs::remove_dir_all(&tmp).unwrap();
        std::fs::remove_dir_all(&tmp2).unwrap();
    }

    #[test]
    fn test_fmt_version_rustc() {
        assert_eq!(fmt_version(Some("rustc 1.70.0".to_string())), "1.70.0");
    }

    #[test]
    fn test_fmt_version_cargo() {
        assert_eq!(fmt_version(Some("cargo 1.70.0".to_string())), "1.70.0");
    }

    #[test]
    fn test_fmt_version_cmake() {
        assert_eq!(fmt_version(Some("cmake version 3.26".to_string())), "3.26");
    }

    #[test]
    fn test_fmt_version_go() {
        assert_eq!(fmt_version(Some("go version go1.20".to_string())), "go1.20");
    }

    #[test]
    fn test_fmt_version_unknown() {
        assert_eq!(fmt_version(Some("foo bar".to_string())), "foo bar");
    }

    #[test]
    fn test_fmt_version_single_word() {
        assert_eq!(fmt_version(Some("v1.0".to_string())), "v1.0");
    }
}
