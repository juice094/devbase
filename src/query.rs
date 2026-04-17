use crate::registry::WorkspaceRegistry;
use chrono::{DateTime, Utc};
use git2::Repository;
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone)]
enum Condition {
    Lang(String),
    Stale { op: String, days: i64 },
    Behind { op: String, count: i64 },
    Tag(String),
    Note(String),
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
                "note" => conditions.push(Condition::Note(rest.to_lowercase())),
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
    repo: &crate::registry::RepoEntry,
    cond: &Condition,
    last_sync: Option<&str>,
    behind: Option<i32>,
    notes: &HashMap<String, Vec<String>>,
) -> Option<String> {
    match cond {
        Condition::Lang(lang) => {
            if detect_lang(repo.local_path.to_string_lossy().as_ref(), lang) {
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
            if repo.tags.iter().any(|t| t.to_lowercase() == *tag) {
                Some(format!("tag={}", tag))
            } else {
                None
            }
        }
        Condition::Note(note) => {
            if notes.get(&repo.id).map(|vec| vec.iter().any(|n| n.to_lowercase().contains(note))).unwrap_or(false) {
                Some(format!("note={}", note))
            } else {
                None
            }
        }
        Condition::Keyword(kw) => {
            let haystack = format!("{} {} {}", repo.id, repo.local_path.to_string_lossy(), repo.tags.join(",")).to_lowercase();
            if haystack.contains(kw) {
                Some(format!("keyword={}", kw))
            } else {
                None
            }
        }
    }
}

pub async fn run_json(query_str: &str, limit: usize, page: usize, config: &crate::config::Config) -> anyhow::Result<serde_json::Value> {
    let conn = WorkspaceRegistry::init_db()?;

    // Handle semantic: prefix queries directly against repo_summaries
    if let Some(rest) = query_str.strip_prefix("semantic:") {
        let keywords: Vec<&str> = rest.split_whitespace().collect();
        if keywords.is_empty() {
            return Ok(serde_json::json!({
                "success": true,
                "count": 0,
                "expression": query_str,
                "results": []
            }));
        }

        let clauses: Vec<String> = keywords.iter().map(|_| "(s.summary LIKE ? OR s.keywords LIKE ?)".to_string()).collect();
        let sql = format!(
            "SELECT r.id, r.local_path, s.summary, s.keywords FROM repo_summaries s JOIN repos r ON r.id = s.repo_id WHERE {}",
            clauses.join(" OR ")
        );

        let likes: Vec<String> = keywords.iter().map(|k| format!("%{}%", k)).collect();
        let mut param_refs: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for like in &likes {
            param_refs.push(like);
            param_refs.push(like);
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut count = 0;
        let mut results: Vec<serde_json::Value> = Vec::new();
        for row in rows {
            let (id, local_path, summary, keywords) = row?;
            count += 1;
            results.push(serde_json::json!({
                "id": id,
                "local_path": local_path,
                "summary": summary,
                "keywords": keywords,
                "match_reasons": ["semantic"]
            }));
        }

        let top_ids = results.iter().take(10).map(|r| r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect::<Vec<_>>().join(",");
        let _ = WorkspaceRegistry::log_query(&conn, query_str, "semantic", count, &top_ids);
        return Ok(serde_json::json!({
            "success": true,
            "count": count,
            "expression": query_str,
            "results": results
        }));
    }

    // Handle paper: prefix queries
    if query_str.starts_with("paper:") {
        let rest = &query_str["paper:".len()..];
        let papers = if let Some((field, value)) = rest.split_once(':') {
            match field {
                "venue" => WorkspaceRegistry::find_papers_by_venue(&conn, value)?,
                _ => {
                    let mut all = WorkspaceRegistry::list_papers(&conn)?;
                    let v = value.to_lowercase();
                    all.retain(|p| {
                        p.venue.as_ref().map(|x| x.to_lowercase() == v).unwrap_or(false)
                            || p.year.map(|y| y.to_string() == v).unwrap_or(false)
                            || p.tags.iter().any(|t| t.to_lowercase() == v)
                    });
                    all
                }
            }
        } else {
            // paper:iclr  -> treat as venue search
            let venue = rest;
            WorkspaceRegistry::find_papers_by_venue(&conn, venue)?
        };
        let count = papers.len();
        let results: Vec<serde_json::Value> = papers.into_iter().map(|p| serde_json::json!({
            "id": p.id,
            "title": p.title,
            "venue": p.venue,
            "year": p.year,
            "pdf_path": p.pdf_path,
            "tags": p.tags.join(","),
            "match_reasons": ["paper"]
        })).collect();
        let top_ids = results.iter().take(10).map(|r| r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect::<Vec<_>>().join(",");
        let _ = WorkspaceRegistry::log_query(&conn, query_str, "paper", count, &top_ids);
        return Ok(serde_json::json!({
            "success": true,
            "count": count,
            "expression": query_str,
            "results": results
        }));
    }

    // Handle experiment: prefix queries
    if query_str.starts_with("experiment:") {
        let rest = &query_str["experiment:".len()..];
        let exps = if let Some((field, value)) = rest.split_once(':') {
            match field {
                "repo" => WorkspaceRegistry::find_experiments_by_repo(&conn, value)?,
                _ => {
                    let mut all = WorkspaceRegistry::list_experiments(&conn)?;
                    let v = value.to_lowercase();
                    all.retain(|e| {
                        e.status.to_lowercase() == v
                            || e.paper_id.as_ref().map(|x| x.to_lowercase() == v).unwrap_or(false)
                    });
                    all
                }
            }
        } else {
            WorkspaceRegistry::list_experiments(&conn)?
        };
        let count = exps.len();
        let results: Vec<serde_json::Value> = exps.into_iter().map(|e| serde_json::json!({
            "id": e.id,
            "repo_id": e.repo_id,
            "paper_id": e.paper_id,
            "status": e.status,
            "syncthing_folder_id": e.syncthing_folder_id,
            "timestamp": e.timestamp.to_rfc3339(),
            "match_reasons": ["experiment"]
        })).collect();
        let top_ids = results.iter().take(10).map(|r| r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect::<Vec<_>>().join(",");
        let _ = WorkspaceRegistry::log_query(&conn, query_str, "experiment", count, &top_ids);
        return Ok(serde_json::json!({
            "success": true,
            "count": count,
            "expression": query_str,
            "results": results
        }));
    }

    let conditions = parse_query(query_str);

    let repos = WorkspaceRegistry::list_repos(&conn)?;

    let needs_notes = conditions.iter().any(|c| matches!(c, Condition::Note(..)));
    let mut notes_map: HashMap<String, Vec<String>> = HashMap::new();
    if needs_notes {
        let mut stmt = conn.prepare("SELECT repo_id, note_text FROM repo_notes")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (repo_id, note_text) = row?;
            notes_map.entry(repo_id).or_default().push(note_text);
        }
    }

    let mut count = 0;
    let mut results: Vec<serde_json::Value> = Vec::new();

    for repo in repos {
        let primary = repo.primary_remote();
        let upstream_url = primary.and_then(|r| r.upstream_url.clone());
        let default_branch = primary.and_then(|r| r.default_branch.clone());
        let last_sync = primary.and_then(|r| r.last_sync.map(|dt| dt.to_rfc3339()));
        let needs_behind = conditions.iter().any(|c| matches!(c, Condition::Behind { .. }));
        let behind = if needs_behind {
            let cached = WorkspaceRegistry::get_health(&conn, &repo.id).ok().flatten();
            if let Some(health) = cached {
                let elapsed = Utc::now().signed_duration_since(health.checked_at).num_seconds();
                if elapsed < config.cache.ttl_seconds {
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
            if let Some(reason) = eval_condition(&repo, cond, last_sync.as_deref(), behind, &notes_map) {
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
                "tags": repo.tags.join(","),
                "default_branch": default_branch,
                "last_sync": last_sync,
                "match_reasons": reasons
            }));
        }
    }

    let total_results = results.len();
    let paged_results = if limit > 0 {
        let start = (page.saturating_sub(1)) * limit;
        results.into_iter().skip(start).take(limit).collect()
    } else {
        results
    };

    let top_ids = paged_results.iter().take(10).map(|r| r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect::<Vec<_>>().join(",");
    let query_type = if query_str.starts_with("semantic:") { "semantic" } else { "structured" };
    let _ = WorkspaceRegistry::log_query(&conn, query_str, query_type, total_results, &top_ids);

    info!("Query executed: {}", query_str);
    Ok(serde_json::json!({
        "success": true,
        "count": total_results,
        "expression": query_str,
        "pagination": if limit > 0 {
            serde_json::json!({
                "total": total_results,
                "page": page,
                "limit": limit,
                "has_more": total_results > page * limit
            })
        } else {
            serde_json::Value::Null
        },
        "results": paged_results
    }))
}

pub async fn run(query_str: &str, limit: usize, page: usize, config: &crate::config::Config) -> anyhow::Result<()> {
    let result = run_json(query_str, limit, page, config).await?;
    let count = result["count"].as_u64().unwrap_or(0) as usize;

    if count == 0 {
        println!("No repositories matched '{}'", query_str);
    } else {
        if let Some(pagination) = result.get("pagination") {
            if pagination != &serde_json::Value::Null {
                let total = pagination["total"].as_u64().unwrap_or(0);
                let page_num = pagination["page"].as_u64().unwrap_or(1);
                let limit_val = pagination["limit"].as_u64().unwrap_or(0);
                let has_more = pagination["has_more"].as_bool().unwrap_or(false);
                println!("\nFound {} result(s) (page {} of ~{}, limit={}).", total, page_num, (total as f64 / limit_val as f64).ceil() as u64, limit_val);
                if has_more {
                    println!("(more results available, use --page {})", page_num + 1);
                }
            } else {
                println!("\nFound {} result(s).", count);
            }
        } else {
            println!("\nFound {} result(s).", count);
        }
        for item in result["results"].as_array().unwrap_or(&vec![]) {
            let id = item["id"].as_str().unwrap_or("");
            if let Some(summary) = item["summary"].as_str() {
                let keywords = item["keywords"].as_str().unwrap_or("");
                println!("  [{}] {} (keywords: {})", id, summary, keywords);
            } else {
                let path = item["local_path"].as_str().unwrap_or("");
                let tags = item["tags"].as_str().unwrap_or("");
                let reasons = item["match_reasons"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                    .unwrap_or_default();
                println!("  [{}] {} (tags: {})  [match: {}]", id, path, tags, reasons);
            }
        }
    }

    Ok(())
}
