use crate::registry::WorkspaceRegistry;
use chrono::{DateTime, Utc};
use git2::Repository;
use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone)]
enum Condition {
    Lang(String),
    Stale { op: String, days: i64 },
    Behind { op: String, count: i64 },
    Tag(String),
    Keyword(String),
}

fn parse_cmp_expr(value: &str) -> Option<(String, i64)> {
    if value.is_empty() {
        return None;
    }
    let first = value.chars().next().unwrap();
    if first == '>' || first == '<' || first == '=' {
        let num = value[1..].parse().ok()?;
        Some((first.to_string(), num))
    } else {
        let num = value.parse().ok()?;
        Some(("=".to_string(), num))
    }
}

fn parse_query(query_str: &str) -> Vec<Condition> {
    let mut conditions = Vec::new();
    for token in query_str.split_whitespace() {
        if let Some((key, rest)) = token.split_once(':') {
            match key {
                "lang" => conditions.push(Condition::Lang(rest.to_lowercase())),
                "stale" => {
                    if let Some((op, days)) = parse_cmp_expr(rest) {
                        conditions.push(Condition::Stale { op, days });
                    }
                }
                "behind" => {
                    if let Some((op, count)) = parse_cmp_expr(rest) {
                        conditions.push(Condition::Behind { op, count });
                    }
                }
                "tag" => conditions.push(Condition::Tag(rest.to_lowercase())),
                _ => conditions.push(Condition::Keyword(token.to_lowercase())),
            }
        } else {
            conditions.push(Condition::Keyword(token.to_lowercase()));
        }
    }
    conditions
}

fn detect_lang(path: &str, lang: &str) -> bool {
    let p = Path::new(path);
    match lang {
        "rust" => p.join("Cargo.toml").exists(),
        "go" => p.join("go.mod").exists(),
        "node" | "js" | "ts" | "javascript" | "typescript" => p.join("package.json").exists(),
        "python" => {
            p.join("pyproject.toml").exists()
                || p.join("requirements.txt").exists()
                || p.join("setup.py").exists()
        }
        "java" => {
            p.join("pom.xml").exists()
                || p.join("build.gradle").exists()
                || p.join("build.gradle.kts").exists()
        }
        "cpp" | "c++" => p.join("CMakeLists.txt").exists() || p.join("Makefile").exists(),
        _ => false,
    }
}

fn compute_behind(path: &str, default_branch: Option<&str>) -> anyhow::Result<Option<i32>> {
    let repo = Repository::open(path)?;

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    if !head.is_branch() {
        return Ok(None); // detached HEAD
    }
    let local_oid = match head.target_peel() {
        Some(oid) => oid,
        None => return Ok(None),
    };

    let branch = default_branch
        .map(|s| s.to_string())
        .or_else(|| {
            repo.find_remote("origin")
                .ok()
                .and_then(|r| r.default_branch().ok())
                .and_then(|b| b.as_str().map(|s| s.trim_start_matches("refs/heads/").to_string()))
        })
        .unwrap_or_else(|| "main".to_string());

    let remote_oid = repo
        .revparse_single(&format!("refs/remotes/origin/{}", branch))
        .ok()
        .map(|obj| obj.id())
        .or_else(|| {
            repo.revparse_single("origin/HEAD")
                .ok()
                .map(|obj| obj.id())
        });

    match remote_oid {
        Some(remote) => {
            let (_, behind) = repo.graph_ahead_behind(local_oid, remote)?;
            Ok(Some(behind as i32))
        }
        None => Ok(None),
    }
}

fn eval_condition(
    cond: &Condition,
    id: &str,
    path: &str,
    tags: &str,
    last_sync: Option<&str>,
    behind: Option<i32>,
) -> Option<String> {
    match cond {
        Condition::Lang(lang) => {
            if detect_lang(path, lang) {
                Some(format!("lang={}", lang))
            } else {
                None
            }
        }
        Condition::Stale { op, days } => {
            let diff_days = last_sync
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_days());

            let matched = match op.as_str() {
                ">" => diff_days.map(|d| d > *days).unwrap_or(true), // never synced = very stale
                "<" => diff_days.map(|d| d < *days).unwrap_or(false),
                "=" => diff_days.map(|d| d == *days).unwrap_or(false),
                _ => false,
            };

            if matched {
                Some(format!("stale{}{}d", op, diff_days.unwrap_or(-1)))
            } else {
                None
            }
        }
        Condition::Behind { op, count } => {
            let matched = match op.as_str() {
                ">" => behind.map(|b| b > *count as i32).unwrap_or(false),
                "<" => behind.map(|b| b < *count as i32).unwrap_or(false),
                "=" => behind.map(|b| b == *count as i32).unwrap_or(false),
                _ => false,
            };
            if matched {
                Some(format!("behind{}{}", op, behind.unwrap_or(0)))
            } else {
                None
            }
        }
        Condition::Tag(tag) => {
            if tags.to_lowercase().split(',').any(|t| t.trim() == tag) {
                Some(format!("tag={}", tag))
            } else {
                None
            }
        }
        Condition::Keyword(kw) => {
            let haystack = format!("{} {} {}", id, path, tags).to_lowercase();
            if haystack.contains(kw) {
                Some(format!("keyword={}", kw))
            } else {
                None
            }
        }
    }
}

pub async fn run_json(query_str: &str) -> anyhow::Result<serde_json::Value> {
    let conn = WorkspaceRegistry::init_db()?;
    let conditions = parse_query(query_str);

    let repos = WorkspaceRegistry::list_repos(&conn)?;

    let mut count = 0;
    let mut results: Vec<serde_json::Value> = Vec::new();

    for repo in repos {
        let primary = repo.primary_remote();
        let upstream_url = primary.and_then(|r| r.upstream_url.clone());
        let default_branch = primary.and_then(|r| r.default_branch.clone());
        let last_sync = primary.and_then(|r| r.last_sync.map(|dt| dt.to_rfc3339()));
        let tags = repo.tags.join(",");

        const CACHE_TTL_SECS: i64 = 300;

        let needs_behind = conditions.iter().any(|c| matches!(c, Condition::Behind { .. }));
        let behind = if needs_behind {
            let cached = WorkspaceRegistry::get_health(&conn, &repo.id).ok().flatten();
            if let Some(health) = cached {
                let elapsed = Utc::now().signed_duration_since(health.checked_at).num_seconds();
                if elapsed < CACHE_TTL_SECS {
                    Some(health.behind as i32)
                } else {
                    match compute_behind(repo.local_path.to_string_lossy().as_ref(), default_branch.as_deref()) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!("Failed to open repo {} at {}: {}", repo.id, repo.local_path.display(), e);
                            None
                        }
                    }
                }
            } else {
                match compute_behind(repo.local_path.to_string_lossy().as_ref(), default_branch.as_deref()) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("Failed to open repo {} at {}: {}", repo.id, repo.local_path.display(), e);
                        None
                    }
                }
            }
        } else {
            None
        };

        let mut reasons = Vec::new();
        let mut matched = true;
        for cond in &conditions {
            if let Some(reason) = eval_condition(cond, &repo.id, repo.local_path.to_string_lossy().as_ref(), &tags, last_sync.as_deref(), behind) {
                reasons.push(reason);
            } else {
                matched = false;
                break;
            }
        }

        if matched {
            count += 1;
            results.push(serde_json::json!({
                "id": repo.id,
                "local_path": repo.local_path,
                "upstream_url": upstream_url,
                "tags": tags,
                "default_branch": default_branch,
                "last_sync": last_sync,
                "match_reasons": reasons
            }));
        }
    }

    info!("Query executed: {}", query_str);
    Ok(serde_json::json!({
        "success": true,
        "count": count,
        "expression": query_str,
        "results": results
    }))
}

pub async fn run(query_str: &str) -> anyhow::Result<()> {
    let result = run_json(query_str).await?;
    let count = result["count"].as_u64().unwrap_or(0) as usize;

    if count == 0 {
        println!("No repositories matched '{}'", query_str);
    } else {
        println!("\nFound {} result(s).", count);
        for item in result["results"].as_array().unwrap_or(&vec![]) {
            let id = item["id"].as_str().unwrap_or("");
            let path = item["local_path"].as_str().unwrap_or("");
            let tags = item["tags"].as_str().unwrap_or("");
            let reasons = item["match_reasons"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            println!("  [{}] {} (tags: {})  [match: {}]", id, path, tags, reasons);
        }
    }

    Ok(())
}
