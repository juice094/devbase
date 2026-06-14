// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use super::nl_query::analyze_repo_for_repo;
use crate::clients::RepoAnalyzer;
use crate::health::RepoAnalyzerImpl;
use crate::mcp::McpTool;
use crate::repository::health::HealthRepository;
use crate::repository::repo::RepoRepository;
use crate::repository::workspace::WorkspaceRepository;
use crate::storage::AppContext;

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
        let analyzer = RepoAnalyzerImpl;
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
                        None => analyze_repo_for_repo(&repo, &analyzer)?,
                    };
                    let dirty = st == "dirty" || st == "changed";
                    (ah, bh, dirty)
                } else {
                    let path_str = repo.local_path.to_string_lossy();
                    let dirty = match analyzer.compute_workspace_hash(&path_str) {
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
