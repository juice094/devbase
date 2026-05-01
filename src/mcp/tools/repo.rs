use crate::mcp::McpTool;
use crate::mcp::clients::{HealthClient, KnowledgeClient, ScanClient, SyncClient};
use crate::registry::RepoEntry;
use crate::repository::health::HealthRepository;
use crate::repository::repo::RepoRepository;
use crate::repository::workspace::WorkspaceRepository;
use crate::storage::AppContext;
use anyhow::Context;

#[derive(Clone)]
pub struct DevkitScanTool;

impl McpTool for DevkitScanTool {
    fn name(&self) -> &'static str {
        "devkit_scan"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Scan a directory to discover Git repositories and non-Git workspaces (e.g., openclaw, generic projects marked by SOUL.md or .devbase files).

Use this when the user wants to:
- Discover repositories in a directory for the first time
- Add newly cloned or downloaded projects to the devbase workspace
- Find ZIP-snapshot folders (named with -main/-master suffix) that need Git migration

Do NOT use this for:
- Listing already-registered repos (use devkit_query_repos instead)
- Checking repo status (use devkit_health instead)
- Searching across repos (use devkit_query_repos or devkit_natural_language_query instead)

Parameters:
- path: Directory to scan (absolute or relative). Defaults to current directory.
- register: If true, discovered repos are persisted to the devbase SQLite registry. If false, returns a preview only.

Returns: JSON array of discovered repos with id, path, language, source_type, and whether registration succeeded."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory to scan",
                        "default": "."
                    },
                    "register": {
                        "type": "boolean",
                        "description": "Register discovered repos into the database",
                        "default": false
                    }
                },
                "required": ["path"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;
        let register = args.get("register").and_then(|v| v.as_bool()).unwrap_or(false);
        ctx.scan_directory(path, register).await
    }
}
#[derive(Clone)]
pub struct DevkitHealthTool;

impl McpTool for DevkitHealthTool {
    fn name(&self) -> &'static str {
        "devkit_health"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Check the health status of all registered repositories in the devbase workspace. This is a read-only diagnostic tool.

Use this when the user wants to:
- Get an overview of all tracked repos and their Git status
- Identify repos that are dirty (uncommitted changes), ahead (local commits not pushed), behind (remote commits not pulled), or diverged
- Check environment prerequisites (Rust, Go, Node.js, CMake versions)
- Find repos that need attention before a sync

Do NOT use this for:
- Pulling or pushing changes (use devkit_sync instead)
- Searching repos by language or tag (use devkit_query_repos instead)
- Scanning new directories (use devkit_scan instead)

Parameters:
- detail: If true, returns per-repo Git status (dirty/ahead/behind/diverged), last sync time, and file count. If false, returns a summary only.

Returns: JSON object with workspace summary and per-repo health records. Each repo includes: id, path, language, tags, git_status (dirty/ahead/behind/diverged/up_to_date), last_synced_at, file_count, and health score."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "detail": {
                        "type": "boolean",
                        "description": "Show detailed per-repo status",
                        "default": false
                    }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let detail = args.get("detail").and_then(|v| v.as_bool()).unwrap_or(false);
        ctx.check_health(detail).await
    }
}
#[derive(Clone)]
pub struct DevkitSyncTool;

impl McpTool for DevkitSyncTool {
    fn name(&self) -> &'static str {
        "devkit_sync"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Synchronize registered repositories with their upstream remotes by pulling and/or pushing commits according to each repo's inferred SyncPolicy (Mirror / Conservative / Rebase / Merge, determined by tags).

This is a WRITE operation. By default it runs in dry-run mode (no files are modified) for safety.

⚠️ SECURITY: This tool modifies Git state (pull/push/rebase/merge). Managed-gate skips untagged repos automatically. Set DEVBASE_MCP_ENABLE_DESTRUCTIVE=1 if this tool is unavailable.

Use this when the user wants to:
- Update local repos to match their remotes (git pull)
- Push local commits to remotes (git push)
- Preview what a sync would do before executing it
- Batch-sync multiple repos filtered by tags

Do NOT use this for:
- Checking repo status without modifying anything (use devkit_health instead)
- Scanning or registering new repos (use devkit_scan instead)
- Repos with dirty working directories — these are automatically skipped for safety
- Repos with diverged histories under Conservative policy — these are also skipped

Parameters:
- dry_run: Defaults to true. When true, previews the sync plan without modifying any files. Set to false to execute.
- filter_tags: Comma-separated tags to limit which repos are synced (e.g., "third-party,reference").

Returns: JSON object with per-repo sync results including: repo_id, action (pull/push/skipped), status (success/conflict/error), and safety_reason if skipped."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dry_run": {
                        "type": "boolean",
                        "description": "Preview mode: do not modify any files",
                        "default": true
                    },
                    "filter_tags": {
                        "type": "string",
                        "description": "Comma-separated tags to filter repos",
                        "default": ""
                    }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        crate::mcp::check_destructive_enabled()?;
        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true);
        let filter_tags = args.get("filter_tags").and_then(|v| v.as_str());
        let filter_tags_vec = filter_tags.map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
        });
        SyncClient::sync_repos(ctx, dry_run, filter_tags_vec).await
    }
}
#[derive(Clone)]
pub struct DevkitIndexTool;

impl McpTool for DevkitIndexTool {
    fn name(&self) -> &'static str {
        "devkit_index"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Build or refresh the Tantivy full-text search index for repository summaries, README extracts, and module structures. This makes repos searchable via devkit_query and devkit_natural_language_query.

Use this when the user wants to:
- Make newly registered repos searchable
- Update the search index after significant code changes
- Enable full-text search across repo documentation

Do NOT use this for:
- Registering new repos (use devkit_scan instead)
- Querying repos directly (use devkit_query_repos or devkit_natural_language_query instead)
- Getting code metrics (use devkit_code_metrics instead)

Parameters:
- path: Specific repo path to index. If omitted, all registered repos are re-indexed.

Returns: JSON with indexed count and error count."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Specific path to index; if omitted, index all registered repos",
                        "default": ""
                    }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        KnowledgeClient::run_index(ctx, path)
    }
}
pub fn parse_github_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("http://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}
#[derive(Clone)]
pub struct DevkitQueryReposTool;

impl McpTool for DevkitQueryReposTool {
    fn name(&self) -> &'static str {
        "devkit_query_repos"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the devbase registry for registered repositories using structured filters. This is the primary read-only tool for repository discovery and filtering.

Use this when the user wants to:
- List repos by programming language (e.g., "show all Rust projects")
- Find repos with specific tags (e.g., "production", "third-party", "agri:crop:rice")
- Filter by Git status (dirty, ahead, behind, diverged, up_to_date)
- Get paginated repo listings with metadata

Do NOT use this for:
- Natural language queries like "show me big projects" (use devkit_natural_language_query instead)
- Full-text search across repo contents (use devkit_index + search instead)
- Checking detailed health diagnostics (use devkit_health instead)
- Writing or modifying repos (use devkit_sync or devkit_scan instead)

Parameters:
- language: Filter by programming language (e.g., "rust", "go", "python"). Empty string = all languages.
- tag: Filter by tag. Empty string = all tags.
- status: Filter by Git status enum: "dirty", "ahead", "behind", "diverged", "up_to_date", or "" (all).
- limit: Maximum results to return. Default 50.

Returns: JSON array of repo objects. Each includes: id, local_path, language, tags, stars, upstream_url, git_status (dirty/ahead/behind/diverged/up_to_date), and last_synced_at."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "language": { "type": "string", "description": "Filter by programming language (e.g., 'rust', 'go', 'python')", "default": "" },
                    "tag": { "type": "string", "description": "Filter by tag", "default": "" },
                    "status": { "type": "string", "enum": ["dirty", "ahead", "behind", "diverged", "up_to_date", ""], "description": "Filter by Git status", "default": "" },
                    "limit": { "type": "integer", "description": "Max results", "default": 50 }
                }
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let language = args.get("language").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tag = args.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let status = args.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as usize;

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let repos = RepoRepository::new(&conn).list_repos(None)?;

            let mut results = Vec::new();
            for repo in repos {
                // Filter by language (case-insensitive)
                if !language.is_empty() {
                    match &repo.language {
                        Some(lang) if lang.eq_ignore_ascii_case(&language) => {}
                        _ => continue,
                    }
                }

                // Filter by tag (case-insensitive)
                if !tag.is_empty() && !repo.tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                    continue;
                }

                // Gather status
                let (ahead, behind, dirty) = if repo.workspace_type == "git" {
                    let (st, ah, bh) = match HealthRepository::new(&conn).get_health(&repo.id)? {
                        Some(health) => (health.status.clone(), health.ahead, health.behind),
                        None => {
                            let path = repo.local_path.to_string_lossy();
                            let primary = repo.primary_remote();
                            let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
                            let default_branch = primary.and_then(|r| r.default_branch.as_deref());
                            crate::health::analyze_repo(&path, upstream_url, default_branch)
                        }
                    };
                    let dirty = st == "dirty" || st == "changed";
                    (ah, bh, dirty)
                } else {
                    let dirty = match crate::health::compute_workspace_hash(&repo.local_path) {
                        Ok(current_hash) => {
                            match WorkspaceRepository::new(&conn).get_latest_snapshot(&repo.id)? {
                                Some(prev) => prev.file_hash != current_hash,
                                None => true,
                            }
                        }
                        Err(_) => false,
                    };
                    (0, 0, dirty)
                };

                // Filter by conceptual status
                if !status.is_empty() {
                    let matches = match status.as_str() {
                        "dirty" => dirty,
                        "ahead" => !dirty && ahead > 0 && behind == 0,
                        "behind" => !dirty && behind > 0 && ahead == 0,
                        "diverged" => !dirty && ahead > 0 && behind > 0,
                        "up_to_date" => !dirty && ahead == 0 && behind == 0,
                        _ => true,
                    };
                    if !matches {
                        continue;
                    }
                }

                results.push(serde_json::json!({
                    "id": repo.id,
                    "path": repo.local_path,
                    "language": repo.language,
                    "tags": repo.tags,
                    "status": {
                        "dirty": dirty,
                        "ahead": ahead,
                        "behind": behind,
                    },
                    "stars": repo.stars,
                }));

                if limit > 0 && results.len() >= limit {
                    break;
                }
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "count": results.len(),
                "repos": results,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
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
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let repos = RepoRepository::new(&conn).list_repos(None)?;
            let filtered = nl_filter_repos(&query, &repos, &conn)?;

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
fn apply_nl_filters(
    repo: &RepoEntry,
    q: &str,
    stars_cond: Option<(char, u64)>,
    explicit_tag: Option<&str>,
    conn: &rusqlite::Connection,
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
            None => {
                let path = repo.local_path.to_string_lossy();
                let primary = repo.primary_remote();
                let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
                let default_branch = primary.and_then(|r| r.default_branch.as_deref());
                crate::health::analyze_repo(&path, upstream_url, default_branch)
            }
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
pub(crate) fn nl_filter_repos(
    query: &str,
    repos: &[RepoEntry],
    conn: &rusqlite::Connection,
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
    let use_tantivy = match crate::search::index_is_empty() {
        Ok(empty) => !empty,
        Err(e) => {
            tracing::warn!("Failed to check search index: {}", e);
            false
        }
    };

    if use_tantivy && !query.trim().is_empty() {
        let limit = repos.len().max(1000);
        match crate::search::search_repos(query, limit) {
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
                        && apply_nl_filters(repo, &q, stars_cond, explicit_tag.as_deref(), conn)?
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
        if apply_nl_filters(repo, &q, stars_cond, explicit_tag.as_deref(), conn)? {
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_repo_https() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo"),
            Some(("owner".to_string(), "repo".to_string()))
        );
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo.git"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_parse_github_repo_ssh() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_parse_github_repo_invalid() {
        assert_eq!(parse_github_repo("https://gitlab.com/owner/repo"), None);
        assert_eq!(parse_github_repo("not-a-url"), None);
    }

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
