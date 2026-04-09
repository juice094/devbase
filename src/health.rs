use crate::registry::{HealthEntry, WorkspaceRegistry};
use chrono::Utc;
use git2::Repository;
use tracing::info;

pub async fn run_json(detail: bool) -> anyhow::Result<serde_json::Value> {
    let (total_repos, dirty_repos, behind_upstream, no_upstream_count, repo_details) = {
        let conn = WorkspaceRegistry::init_db()?;
        let repos = WorkspaceRegistry::list_repos(&conn)?;

        let mut total_repos: usize = 0;
        let mut dirty_repos: usize = 0;
        let mut behind_upstream: usize = 0;
        let mut no_upstream_count: usize = 0;
        let mut repo_details: Vec<serde_json::Value> = Vec::new();

        const CACHE_TTL_SECS: i64 = 300;

        for repo in repos {
            total_repos += 1;
            let primary = repo.primary_remote();
            let upstream_url = primary.and_then(|r| r.upstream_url.clone());
            let default_branch = primary.and_then(|r| r.default_branch.clone());

            let (status, ahead, behind) = match WorkspaceRegistry::get_health(&conn, &repo.id) {
                Ok(Some(health)) => {
                    let elapsed = Utc::now().signed_duration_since(health.checked_at).num_seconds();
                    if elapsed < CACHE_TTL_SECS {
                        (health.status, health.ahead, health.behind)
                    } else {
                        let (status, ahead, behind) =
                            analyze_repo(repo.local_path.to_string_lossy().as_ref(), upstream_url.as_deref(), default_branch.as_deref());
                        let new_health = HealthEntry {
                            status: status.clone(),
                            ahead,
                            behind,
                            checked_at: Utc::now(),
                        };
                        if let Err(e) = WorkspaceRegistry::save_health(&conn, &repo.id, &new_health) {
                            tracing::warn!("Failed to save health for {}: {}", repo.id, e);
                        }
                        (status, ahead, behind)
                    }
                }
                _ => {
                    let (status, ahead, behind) =
                        analyze_repo(repo.local_path.to_string_lossy().as_ref(), upstream_url.as_deref(), default_branch.as_deref());
                    let new_health = HealthEntry {
                        status: status.clone(),
                        ahead,
                        behind,
                        checked_at: Utc::now(),
                    };
                    if let Err(e) = WorkspaceRegistry::save_health(&conn, &repo.id, &new_health) {
                        tracing::warn!("Failed to save health for {}: {}", repo.id, e);
                    }
                    (status, ahead, behind)
                }
            };

            match status.as_str() {
                "dirty" => dirty_repos += 1,
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
                "behind": behind
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
    Ok(serde_json::json!({
        "success": true,
        "summary": summary,
        "environment": environment,
        "repos": if detail { repo_details } else { vec![] }
    }))
}

pub async fn run(detail: bool) -> anyhow::Result<()> {
    let result = run_json(detail).await?;

    let summary = result["summary"].as_object().unwrap();
    println!("Summary:");
    println!("  total_repos: {}", summary["total_repos"].as_u64().unwrap_or(0));
    println!("  dirty_repos: {}", summary["dirty_repos"].as_u64().unwrap_or(0));
    println!("  behind_upstream: {}", summary["behind_upstream"].as_u64().unwrap_or(0));
    println!("  no_upstream: {}", summary["no_upstream"].as_u64().unwrap_or(0));

    let env = result["environment"].as_object().unwrap();
    println!("\nEnvironment:");
    println!("  rustc: {}", env["rustc"].as_str().unwrap_or("not installed"));
    println!("  cargo: {}", env["cargo"].as_str().unwrap_or("not installed"));
    println!("  node: {}", env["node"].as_str().unwrap_or("not installed"));
    println!("  go: {}", env["go"].as_str().unwrap_or("not installed"));
    println!("  cmake: {}", env["cmake"].as_str().unwrap_or("not installed"));

    if detail {
        let repos = result["repos"].as_array().unwrap();
        if !repos.is_empty() {
            println!("\nRepos:");
            for repo in repos {
                let id = repo["id"].as_str().unwrap_or("");
                let path = repo["local_path"].as_str().unwrap_or("");
                let upstream = repo["upstream_url"].as_str().unwrap_or("none");
                let branch = repo["default_branch"].as_str().unwrap_or("unknown");
                let status = repo["status"].as_str().unwrap_or("");
                let ahead = repo["ahead"].as_u64().unwrap_or(0);
                let behind = repo["behind"].as_u64().unwrap_or(0);
                println!(
                    "  [{}] status={} | ahead={} | behind={} | path={} | upstream={} | branch={}",
                    id, status, ahead, behind, path, upstream, branch
                );
            }
        }
    }

    Ok(())
}

fn analyze_repo(path: &str, upstream_url: Option<&str>, default_branch: Option<&str>) -> (String, usize, usize) {
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

fn calc_ahead_behind(repo: &Repository, default_branch: Option<&str>) -> anyhow::Result<(usize, usize)> {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok((0, 0)),
    };

    let local_oid = match head.target() {
        Some(oid) => oid,
        None => return Ok((0, 0)),
    };

    let upstream_names: Vec<String> = match git2::Branch::wrap(head).upstream() {
        Ok(up) => {
            match up.name() {
                Ok(Some(name)) => vec![name.to_string()],
                _ => vec!["origin/HEAD".to_string()],
            }
        }
        Err(_) => {
            let branch = default_branch.unwrap_or("HEAD");
            vec![format!("origin/{}", branch), "origin/HEAD".to_string()]
        }
    };

    let upstream_ref = upstream_names
        .iter()
        .find_map(|name| repo.find_reference(name).ok());

    let upstream_ref = match upstream_ref {
        Some(r) => r,
        None => return Ok((0, 0)),
    };

    let remote_oid = match upstream_ref.target() {
        Some(oid) => oid,
        None => return Ok((0, 0)),
    };

    repo.graph_ahead_behind(local_oid, remote_oid)
        .map_err(|e| anyhow::anyhow!(e))
}

async fn get_tool_version(cmd: &str, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .ok()?;

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
        None => "not installed".to_string(),
    }
}
