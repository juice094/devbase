// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::clients::{RepoAnalyzer, SearchClient};
use crate::health::RepoAnalyzerImpl;
use crate::mcp::McpTool;
use crate::registry::RepoEntry;
use crate::repository::health::HealthRepository;
use crate::repository::repo::RepoRepository;
use crate::search::SearchClientImpl;
use crate::storage::AppContext;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitNaturalLanguageQueryTool;

impl McpTool for DevkitNaturalLanguageQueryTool {
    fn name(&self) -> &'static str {
        "devkit_natural_language_query"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query registered repositories using natural language instead of structured filters. The query is parsed into filter conditions (language, status, stars, tags) and executed against the registry.

Use this when the user asks in conversational form, such as:
- "Show me all dirty Rust projects"
- "Which repos have more than 100 stars?"
- "List third-party libraries that are behind upstream"

Do NOT use this for:
- Precise structured queries (use devkit_query_repos for exact filters)
- Full-text search across code (use devkit_index + search)
- Vault note searches (use devkit_vault_search instead)

Parameters:
- query: Natural language query string.

Returns: JSON array of matching repos with metadata, same format as devkit_query_repos."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural language query" }
                },
                "required": ["query"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing required argument: query")?;
        let query = query.to_string();

        let pool = ctx.pool();
        let index_path = ctx.storage.index_path()?;
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let repos = RepoRepository::new(&conn).list_repos(None)?;
            let searcher = SearchClientImpl;
            let analyzer = RepoAnalyzerImpl;
            let filtered =
                nl_filter_repos_at(&index_path, &query, &repos, &conn, &searcher, &analyzer)?;

            let results: Vec<serde_json::Value> = filtered
                .into_iter()
                .map(|repo| {
                    serde_json::json!({
                        "id": repo.id,
                        "path": repo.local_path,
                        "language": repo.language,
                        "tags": repo.tags,
                        "stars": repo.stars,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "count": results.len(),
                "query": query,
                "repos": results,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

fn apply_nl_filters<A: RepoAnalyzer>(
    repo: &RepoEntry,
    q: &str,
    stars_cond: Option<(char, u64)>,
    explicit_tag: Option<&str>,
    conn: &rusqlite::Connection,
    analyzer: &A,
) -> anyhow::Result<bool> {
    // Language filter: only apply if query explicitly mentions a language keyword
    let lang_keywords = [
        ("rust", "rust"),
        ("go", "go"),
        ("golang", "go"),
        ("python", "python"),
        ("typescript", "typescript"),
        ("ts", "typescript"),
        ("javascript", "javascript"),
        ("js", "javascript"),
        ("cpp", "c++"),
        ("c++", "c++"),
        ("java", "java"),
    ];
    for &(kw, expected) in &lang_keywords {
        if q.contains(kw) && repo.language.as_deref() != Some(expected) {
            return Ok(false);
        }
    }

    // Tag filter
    if let Some(tag) = explicit_tag
        && !repo.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    {
        return Ok(false);
    }

    // Stars filter
    if let Some((op, val)) = stars_cond {
        let stars = repo.stars.unwrap_or(0);
        let matches = match op {
            '>' => stars > val,
            '<' => stars < val,
            '=' => stars == val,
            _ => true,
        };
        if !matches {
            return Ok(false);
        }
    }

    // Status filters (need health data)
    if q.contains("dirty")
        || q.contains("behind")
        || q.contains("ahead")
        || q.contains("diverged")
        || q.contains("up to date")
        || q.contains("uptodate")
    {
        let (st, ah, bh) = match HealthRepository::new(conn).get_health(&repo.id)? {
            Some(h) => (h.status.clone(), h.ahead, h.behind),
            None => analyze_repo_for_repo(repo, analyzer)?,
        };
        let dirty = st == "dirty" || st == "changed";

        if q.contains("dirty") && !dirty {
            return Ok(false);
        }
        if q.contains("behind") && !q.contains("ahead") && bh == 0 {
            return Ok(false);
        }
        if q.contains("ahead") && !q.contains("behind") && ah == 0 {
            return Ok(false);
        }
        if q.contains("diverged") && (ah == 0 || bh == 0) {
            return Ok(false);
        }
        if (q.contains("up to date") || q.contains("uptodate")) && (dirty || ah > 0 || bh > 0) {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Filter repos using an explicit Tantivy index path, bypassing global storage backend.
pub fn nl_filter_repos_at<S: SearchClient, A: RepoAnalyzer>(
    index_path: &std::path::Path,
    query: &str,
    repos: &[RepoEntry],
    conn: &rusqlite::Connection,
    searcher: &S,
    analyzer: &A,
) -> anyhow::Result<Vec<RepoEntry>> {
    let q = query.to_lowercase();
    let stars_cond = parse_stars_condition(&q);
    let explicit_tag = extract_tag_from_query(&q);

    let has_structural_filter = stars_cond.is_some()
        || explicit_tag.is_some()
        || q.contains("dirty")
        || q.contains("behind")
        || q.contains("ahead")
        || q.contains("diverged")
        || q.contains("up to date")
        || q.contains("uptodate");

    // Try Tantivy search first if index is not empty
    let use_tantivy = match searcher.index_is_empty_at(index_path) {
        Ok(empty) => !empty,
        Err(e) => {
            tracing::warn!("Failed to check search index: {}", e);
            false
        }
    };

    if use_tantivy && !query.trim().is_empty() {
        let limit = repos.len().max(1000);
        match searcher.search_repos_at(index_path, query, limit) {
            Ok(search_results) => {
                let repo_map: std::collections::HashMap<_, _> =
                    repos.iter().map(|r| (r.id.clone(), r)).collect();
                let mut seen = std::collections::HashSet::new();
                let mut results = Vec::new();
                for (id, _score) in search_results {
                    if !seen.insert(id.clone()) {
                        continue;
                    }
                    if let Some(repo) = repo_map.get(&id)
                        && apply_nl_filters(
                            repo,
                            &q,
                            stars_cond,
                            explicit_tag.as_deref(),
                            conn,
                            analyzer,
                        )?
                    {
                        results.push((*repo).clone());
                    }
                }
                if !results.is_empty() {
                    return Ok(results);
                } else if has_structural_filter {
                    // Tantivy returned no matching current repos, but query has structural filters -> return empty
                    return Ok(Vec::new());
                }
                // Otherwise fall through to fallback logic
            }
            Err(e) => {
                tracing::warn!("Tantivy search failed, falling back: {}", e);
            }
        }
    }

    // Fallback: iterate all repos with hardcoded regex logic
    let mut results = Vec::new();
    for repo in repos {
        if apply_nl_filters(repo, &q, stars_cond, explicit_tag.as_deref(), conn, analyzer)? {
            results.push(repo.clone());
        }
    }
    Ok(results)
}

fn parse_stars_condition(query: &str) -> Option<(char, u64)> {
    let lower = query.to_lowercase();
    if !lower.contains("stars") && !lower.contains("star") {
        return None;
    }
    let digits: String = lower
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let num = digits.parse::<u64>().ok()?;

    if lower.contains(">") || lower.contains("more than") || lower.contains("over") {
        Some(('>', num))
    } else if lower.contains("<") || lower.contains("less than") || lower.contains("under") {
        Some(('<', num))
    } else {
        Some(('=', num))
    }
}

fn extract_tag_from_query(q: &str) -> Option<String> {
    if let Some(pos) = q.find("tag ") {
        let rest = &q[pos + 4..];
        rest.split_whitespace().next().map(|s| s.to_string())
    } else if let Some(pos) = q.find("with tag ") {
        let rest = &q[pos + 9..];
        rest.split_whitespace().next().map(|s| s.to_string())
    } else {
        None
    }
}

pub(crate) fn analyze_repo_for_repo<A: RepoAnalyzer>(
    repo: &RepoEntry,
    analyzer: &A,
) -> anyhow::Result<(String, usize, usize)> {
    let path = repo.local_path.to_string_lossy();
    let primary = repo.primary_remote();
    let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
    let default_branch = primary.and_then(|r| r.default_branch.as_deref());
    analyzer.analyze_repo(&path, upstream_url, default_branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stars_condition() {
        assert_eq!(parse_stars_condition("stars > 100"), Some(('>', 100)));
        assert_eq!(parse_stars_condition("more than 50 stars"), Some(('>', 50)));
        assert_eq!(parse_stars_condition("less than 10 stars"), Some(('<', 10)));
        assert_eq!(parse_stars_condition("stars 42"), Some(('=', 42)));
        assert_eq!(parse_stars_condition("just a query"), None);
    }

    #[test]
    fn test_extract_tag_from_query() {
        assert_eq!(extract_tag_from_query("show repos tag rust"), Some("rust".to_string()));
        assert_eq!(extract_tag_from_query("repos with tag python"), Some("python".to_string()));
        assert_eq!(extract_tag_from_query("show all repos"), None);
    }
}
