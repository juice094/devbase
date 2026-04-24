use crate::mcp::McpTool;
use anyhow::Context;
use rusqlite::OptionalExtension;

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

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: path")?;
        let register = args.get("register").and_then(|v| v.as_bool()).unwrap_or(false);
        crate::scan::run_json(path, register).await
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

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let detail = args.get("detail").and_then(|v| v.as_bool()).unwrap_or(false);
        let config = crate::config::Config::load()?;
        crate::health::run_json(detail, 0, 1, config.cache.ttl_seconds).await
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

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true);
        let filter_tags = args.get("filter_tags").and_then(|v| v.as_str());
        crate::sync::run_json(dry_run, filter_tags, None).await
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

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let path_owned = path.to_string();
        let count =
            tokio::task::spawn_blocking(move || crate::knowledge_engine::run_index(&path_owned))
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
        Ok(serde_json::json!({ "success": true, "indexed": count, "errors": 0 }))
    }
}

#[derive(Clone)]
pub struct DevkitNoteTool;

impl McpTool for DevkitNoteTool {
    fn name(&self) -> &'static str {
        "devkit_note"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Attach a short text note to a registered repository in the devbase SQLite registry. This is a lightweight annotation tool, not a full document.

Use this when the user wants to:
- Record a quick observation about a repo (e.g., "needs dependency update")
- Mark a repo with temporary status information
- Log a one-line note from an AI assistant

Do NOT use this for:
- Writing long-form documentation (use devkit_vault_write instead)
- Creating structured knowledge base entries (use devkit_vault_write instead)
- Notes that need markdown formatting or backlinks (use devkit_vault_write instead)

Parameters:
- repo_id: Registered repository ID.
- text: Note content (plain text, max ~500 chars recommended).
- author: Optional author label (default: "ai").

Returns: JSON with success status."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "text": { "type": "string" },
                    "author": { "type": "string", "default": "ai" }
                },
                "required": ["repo_id", "text"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let text = args.get("text").and_then(|v| v.as_str()).context("text required")?;
        let author = args.get("author").and_then(|v| v.as_str()).unwrap_or("ai");
        let repo_id = repo_id.to_string();
        let text = text.to_string();
        let author = author.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            crate::registry::WorkspaceRegistry::save_note(&conn, &repo_id, &text, &author)?;
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitDigestTool;

impl McpTool for DevkitDigestTool {
    fn name(&self) -> &'static str {
        "devkit_digest"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Generate a daily summary digest of recent activity across the devbase workspace, including new repos, health changes, and sync events.

Use this when the user wants to:
- Get a morning briefing of workspace changes
- Review what happened across all repos in the last 24 hours
- Identify repos that need attention today

Do NOT use this for:
- Real-time status checks (use devkit_health instead)
- Specific repo queries (use devkit_query_repos instead)
- Searching the vault (use devkit_vault_search instead)

Parameters: None.

Returns: JSON with a plain-text digest string."#,
            "inputSchema": { "type": "object", "properties": {} }
        })
    }
    async fn invoke(&self, _args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        tokio::task::spawn_blocking(|| {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let config = crate::config::Config::load()?;
            let text = crate::digest::generate_daily_digest(&conn, &config)?;
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "digest": text }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitPaperIndexTool;

impl McpTool for DevkitPaperIndexTool {
    fn name(&self) -> &'static str {
        "devkit_paper_index"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Scan a directory for PDF academic papers, extract metadata (title, authors, year if available), and register them in the devbase knowledge base for citation and search.

Use this when the user wants to:
- Import a collection of PDF papers into the workspace
- Make papers searchable alongside code repos
- Build a personal research library

Do NOT use this for:
- Indexing code repositories (use devkit_scan instead)
- Reading paper content (use devkit_vault_read after indexing)
- Searching existing papers (use devkit_vault_search instead)

Parameters:
- path: Directory containing PDFs. Defaults to ~/papers.
- tags: Comma-separated tags to apply to all discovered papers.

Returns: JSON with discovered paper count and registration status."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory containing PDFs", "default": "~/papers" },
                    "tags": { "type": "string", "description": "Comma-separated tags to apply", "default": "" }
                }
            }
        })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("~/papers");
        let tags_str = args.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let tags: Vec<String> = tags_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let path = if path.starts_with("~/") {
            dirs::home_dir()
                .map(|d| d.join(&path[2..]))
                .unwrap_or_else(|| std::path::PathBuf::from(path))
        } else {
            std::path::PathBuf::from(path)
        };

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let mut count = 0;
            if path.is_dir() {
                for entry in std::fs::read_dir(&path)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().ends_with(".pdf") {
                        let id = name.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string();
                        // Simple heuristic: if filename contains arXiv format (e.g., 2507.03616)
                        let title =
                            if id.chars().filter(|c| c.is_numeric() || *c == '.').count() > 5 {
                                format!("arXiv:{}", id)
                            } else {
                                id.clone()
                            };
                        let paper = crate::registry::PaperEntry {
                            id: id.clone(),
                            title,
                            authors: None,
                            venue: None,
                            year: None,
                            pdf_path: Some(entry.path().to_string_lossy().to_string()),
                            bibtex: None,
                            tags: tags.clone(),
                            added_at: chrono::Utc::now(),
                        };
                        crate::registry::WorkspaceRegistry::save_paper(&conn, &paper)?;
                        count += 1;
                    }
                }
            }
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "indexed": count }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitExperimentLogTool;

impl McpTool for DevkitExperimentLogTool {
    fn name(&self) -> &'static str {
        "devkit_experiment_log"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Log a structured experiment entry in the devbase registry, linking it to a repository and optionally tagging the repo as experiment-active.

Use this when the user wants to:
- Record an experiment configuration and result
- Track which repos are currently being experimented on
- Maintain an audit trail of iterative changes

Do NOT use this for:
- General note-taking (use devkit_note or devkit_vault_write instead)
- Code changes (use devkit_sync or git directly)
- Paper tracking (use devkit_paper_index instead)

Parameters:
- id: Experiment identifier (e.g., "exp-2026-04-23-benchmark").
- repo_id: Associated repository ID.
- config: JSON object with experiment parameters.
- result: JSON object with experiment outcomes.
- tag_repo: If true, tags the repo with "experiment-active".

Returns: JSON with experiment log ID and success status."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Experiment identifier" },
                    "repo_id": { "type": "string" },
                    "paper_id": { "type": "string" },
                    "config_json": { "type": "string" },
                    "result_path": { "type": "string" },
                    "git_commit": { "type": "string" },
                    "syncthing_folder_id": { "type": "string" },
                    "status": { "type": "string", "default": "running" },
                    "tag_repo": { "type": "boolean", "default": false, "description": "Tag the associated repo with experiment-active" }
                },
                "required": ["id"]
            }
        })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let id = args.get("id").and_then(|v| v.as_str()).context("id required")?.to_string();
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).map(String::from);
        let tag_repo = args.get("tag_repo").and_then(|v| v.as_bool()).unwrap_or(false);
        let exp = crate::registry::ExperimentEntry {
            id,
            repo_id: repo_id.clone(),
            paper_id: args.get("paper_id").and_then(|v| v.as_str()).map(String::from),
            config_json: args.get("config_json").and_then(|v| v.as_str()).map(String::from),
            result_path: args.get("result_path").and_then(|v| v.as_str()).map(String::from),
            git_commit: args.get("git_commit").and_then(|v| v.as_str()).map(String::from),
            syncthing_folder_id: args
                .get("syncthing_folder_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            status: args.get("status").and_then(|v| v.as_str()).unwrap_or("running").to_string(),
            timestamp: chrono::Utc::now(),
        };
        tokio::task::spawn_blocking(move || {
            let mut conn = crate::registry::WorkspaceRegistry::init_db()?;
            crate::registry::WorkspaceRegistry::save_experiment(&conn, &exp)?;
            if tag_repo
                && let Some(ref rid) = repo_id {
                    let tx = conn.transaction()?;
                    tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1 AND tag = 'experiment-active'", [rid])?;
                    tx.execute("INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, 'experiment-active')", [rid])?;
                    tx.commit()?;
                }
            Ok::<_, anyhow::Error>(serde_json::json!({ "success": true }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitGithubInfoTool;

impl McpTool for DevkitGithubInfoTool {
    fn name(&self) -> &'static str {
        "devkit_github_info"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Fetch real-time metadata from the GitHub API for a registered repository, including stars, forks, open issues, description, and last push date. Optionally writes the GitHub description into the repo's local summary.

Use this when the user wants to:
- Check the current popularity (stars) of a tracked repo
- Compare upstream activity across multiple repos
- Update the local summary with the official GitHub description

Do NOT use this for:
- Querying local repo status (use devkit_health instead)
- Syncing code changes (use devkit_sync instead)
- Repos not hosted on GitHub (returns error)

Parameters:
- repo_id: Registered repository ID in devbase.
- write_summary: If true, writes the GitHub description into the local repo summary file.

Returns: JSON with stars, forks, open_issues, description, pushed_at, and updated summary path if write_summary was true."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Registered repository ID in devbase" },
                    "write_summary": { "type": "boolean", "description": "Write GitHub description into repo summary", "default": false }
                },
                "required": ["repo_id"]
            }
        })
    }
    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args
            .get("repo_id")
            .and_then(|v| v.as_str())
            .context("repo_id required")?
            .to_string();
        let write_summary = args.get("write_summary").and_then(|v| v.as_bool()).unwrap_or(false);

        let upstream_url = tokio::task::spawn_blocking({
            let repo_id = repo_id.clone();
            move || -> anyhow::Result<Option<String>> {
                let conn = crate::registry::WorkspaceRegistry::init_db()?;
                let mut stmt = conn.prepare("SELECT upstream_url FROM repo_remotes WHERE repo_id = ?1 AND remote_name = 'origin'")?;
                let url: Option<String> = stmt.query_row([&repo_id], |row| row.get(0)).optional()?;
                Ok(url)
            }
        }).await.map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;

        let upstream_url = upstream_url.context("No origin remote found for repo")?;
        let (owner, repo_name) = parse_github_repo(&upstream_url)
            .context("Failed to parse GitHub owner/repo from upstream_url")?;

        let config = crate::config::Config::load()?;
        let client = reqwest::Client::new();
        let mut req = client
            .get(format!("https://api.github.com/repos/{}/{}", owner, repo_name))
            .header("User-Agent", "devbase/0.1.0");
        if let Some(token) = config.github.token.as_deref() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Ok(
                serde_json::json!({ "success": false, "error": format!("GitHub API error {}: {}", status, text) }),
            );
        }
        let data: serde_json::Value = resp.json().await?;

        let stars = data.get("stargazers_count").and_then(|v| v.as_i64());
        let forks = data.get("forks_count").and_then(|v| v.as_i64());
        let description = data.get("description").and_then(|v| v.as_str()).map(String::from);
        let language = data.get("language").and_then(|v| v.as_str()).map(String::from);
        let open_issues = data.get("open_issues_count").and_then(|v| v.as_i64());
        let updated_at = data.get("updated_at").and_then(|v| v.as_str()).map(String::from);
        let html_url = data.get("html_url").and_then(|v| v.as_str()).map(String::from);

        if write_summary && let Some(ref desc) = description {
            let repo_id2 = repo_id.clone();
            let desc2 = desc.clone();
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                let conn = crate::registry::WorkspaceRegistry::init_db()?;
                crate::registry::WorkspaceRegistry::save_summary(&conn, &repo_id2, &desc2, "")?;
                Ok(())
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
        }

        Ok(serde_json::json!({
            "success": true,
            "owner": owner,
            "repo": repo_name,
            "stars": stars,
            "forks": forks,
            "description": description,
            "language": language,
            "open_issues": open_issues,
            "updated_at": updated_at,
            "html_url": html_url,
            "raw": data
        }))
    }
}

#[derive(Clone)]
pub struct DevkitCodeMetricsTool;

impl McpTool for DevkitCodeMetricsTool {
    fn name(&self) -> &'static str {
        "devkit_code_metrics"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Compute code metrics for registered repositories: total lines of code, file count, language breakdown, and rough complexity indicators (via tokei).

Use this when the user wants to:
- Compare the size of different projects
- Identify the primary language of a repo
- Find the largest or most complex codebase in the workspace

Do NOT use this for:
- Module-level structure analysis (use devkit_module_graph instead)
- Git status or health checks (use devkit_health instead)
- Searching code content (use devkit_natural_language_query instead)

Parameters:
- repo_id: Specific repo ID. If omitted, returns metrics for all registered repos.

Returns: JSON array of metric objects per repo: repo_id, total_lines, code_lines, comment_lines, blank_lines, file_count, and language_breakdown."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Specific repo ID; if omitted, returns all repos", "default": "" }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            if repo_id.is_empty() {
                let metrics = crate::registry::WorkspaceRegistry::list_code_metrics(&conn)?;
                let repos: Vec<serde_json::Value> = metrics.into_iter().map(|(id, m)| {
                    serde_json::json!({
                        "repo_id": id,
                        "total_lines": m.total_lines,
                        "source_lines": m.source_lines,
                        "test_lines": m.test_lines,
                        "comment_lines": m.comment_lines,
                        "file_count": m.file_count,
                        "language_breakdown": m.language_breakdown,
                        "updated_at": m.updated_at.to_rfc3339()
                    })
                }).collect();
                Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "count": repos.len(), "repos": repos }))
            } else {
                match crate::registry::WorkspaceRegistry::get_code_metrics(&conn, &repo_id)? {
                    Some(m) => Ok(serde_json::json!({
                        "success": true,
                        "repo_id": repo_id,
                        "total_lines": m.total_lines,
                        "source_lines": m.source_lines,
                        "test_lines": m.test_lines,
                        "comment_lines": m.comment_lines,
                        "file_count": m.file_count,
                        "language_breakdown": m.language_breakdown,
                        "updated_at": m.updated_at.to_rfc3339()
                    })),
                    None => Ok(serde_json::json!({ "success": false, "error": "No metrics found for repo" })),
                }
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitModuleGraphTool;

impl McpTool for DevkitModuleGraphTool {
    fn name(&self) -> &'static str {
        "devkit_module_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Extract the module and binary target structure from a Rust repository using cargo metadata. Returns crates, binaries, libraries, and their interdependencies.

Use this when the user wants to:
- Understand the architecture of a Rust workspace
- Find all binary targets (executables) in a project
- Map crate dependencies within a workspace

Do NOT use this for:
- Non-Rust repositories (returns empty or error)
- General code metrics like line counts (use devkit_code_metrics instead)
- Git operations (use devkit_health or devkit_sync instead)

Parameters:
- repo_id: Repository ID. If omitted, analyzes the current directory.

Returns: JSON with workspace_members, packages (name, version, targets), and dependency graph."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string", "description": "Repository ID", "default": "" }
                }
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            if repo_id.is_empty() {
                let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
                let mut all_modules = vec![];
                for repo in repos {
                    if repo.language.as_deref() == Some("Rust") {
                        let modules = crate::registry::WorkspaceRegistry::list_modules(&conn, &repo.id)?;
                        if !modules.is_empty() {
                            all_modules.push(serde_json::json!({
                                "repo_id": repo.id,
                                "modules": modules.iter().map(|(n, t, p)| serde_json::json!({
                                    "name": n, "type": t, "path": p
                                })).collect::<Vec<_>>()
                            }));
                        }
                    }
                }
                Ok::<_, anyhow::Error>(serde_json::json!({ "success": true, "count": all_modules.len(), "repos": all_modules }))
            } else {
                let modules = crate::registry::WorkspaceRegistry::list_modules(&conn, &repo_id)?;
                Ok(serde_json::json!({
                    "success": true,
                    "repo_id": repo_id,
                    "modules": modules.iter().map(|(n, t, p)| serde_json::json!({
                        "name": n, "type": t, "path": p
                    })).collect::<Vec<_>>()
                }))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

fn parse_github_repo(url: &str) -> Option<(String, String)> {
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

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let language = args.get("language").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tag = args.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let status = args.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as usize;

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;

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
                    let (st, ah, bh) =
                        match crate::registry::WorkspaceRegistry::get_health(&conn, &repo.id)? {
                            Some(health) => (health.status.clone(), health.ahead, health.behind),
                            None => {
                                let path = repo.local_path.to_string_lossy();
                                let primary = repo.primary_remote();
                                let upstream_url = primary.and_then(|r| r.upstream_url.as_deref());
                                let default_branch =
                                    primary.and_then(|r| r.default_branch.as_deref());
                                crate::health::analyze_repo(&path, upstream_url, default_branch)
                            }
                        };
                    let dirty = st == "dirty" || st == "changed";
                    (ah, bh, dirty)
                } else {
                    let dirty = match crate::health::compute_workspace_hash(&repo.local_path) {
                        Ok(current_hash) => {
                            match crate::registry::WorkspaceRegistry::get_latest_workspace_snapshot(
                                &conn, &repo.id,
                            )? {
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

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing required argument: query")?;
        let query = query.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let repos = crate::registry::WorkspaceRegistry::list_repos(&conn)?;
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

fn nl_filter_repos(
    query: &str,
    repos: &[crate::registry::RepoEntry],
    conn: &rusqlite::Connection,
) -> anyhow::Result<Vec<crate::registry::RepoEntry>> {
    let q = query.to_lowercase();
    let stars_cond = parse_stars_condition(&q);
    let explicit_tag = extract_tag_from_query(&q);

    let mut results = Vec::new();
    for repo in repos {
        // Language filter
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
        let mut lang_matched = true;
        for &(kw, expected) in &lang_keywords {
            if q.contains(kw) && repo.language.as_deref() != Some(expected) {
                lang_matched = false;
                break;
            }
        }
        if !lang_matched {
            continue;
        }

        // Tag filter
        if let Some(ref tag) = explicit_tag
            && !repo.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
        {
            continue;
        }

        // Stars filter
        if let Some((op, val)) = stars_cond {
            let stars = repo.stars.unwrap_or(0);
            match op {
                '>' => {
                    if stars <= val {
                        continue;
                    }
                }
                '<' => {
                    if stars >= val {
                        continue;
                    }
                }
                '=' => {
                    if stars != val {
                        continue;
                    }
                }
                _ => {}
            }
        }

        // Status filters (need health data)
        if q.contains("dirty")
            || q.contains("behind")
            || q.contains("ahead")
            || q.contains("diverged")
            || q.contains("up to date")
        {
            let (st, ah, bh) = match crate::registry::WorkspaceRegistry::get_health(conn, &repo.id)?
            {
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
                continue;
            }
            if q.contains("behind") && !q.contains("ahead") && bh == 0 {
                continue;
            }
            if q.contains("ahead") && !q.contains("behind") && ah == 0 {
                continue;
            }
            if q.contains("diverged") && (ah == 0 || bh == 0) {
                continue;
            }
            if (q.contains("up to date") || q.contains("uptodate")) && (dirty || ah > 0 || bh > 0) {
                continue;
            }
        }

        results.push(repo.clone());
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

#[derive(Clone)]
pub struct DevkitCodeSymbolsTool;

impl McpTool for DevkitCodeSymbolsTool {
    fn name(&self) -> &'static str {
        "devkit_code_symbols"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the semantic code symbol index for a repository. Returns functions, structs, enums, traits, impls, and modules extracted via tree-sitter AST parsing.

Use this when the user wants to:
- Find the definition of a specific function or struct
- Explore the API surface of a repository
- Answer questions like "what functions are in file X?" or "where is struct Y defined?"
- Understand the module structure at the symbol level

Do NOT use this for:
- Full-text search across code contents (use devkit_natural_language_query instead)
- Getting repo-level summaries (use devkit_query_repos instead)
- Code metrics like line counts (use devkit_code_metrics instead)

Parameters:
- repo_id: Registered repository ID to query.
- name_filter: Optional symbol name substring to filter results (case-insensitive).
- symbol_type: Optional filter by symbol type: "function", "struct", "enum", "trait", "impl", "module", "type_alias", "constant", "static".
- file_path: Optional file path substring to filter by source file.
- limit: Maximum results to return (default: 50, max: 200).

Returns: JSON array of symbols with file_path, name, symbol_type, line_start, line_end, and optional signature."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "name_filter": { "type": "string", "default": "" },
                    "symbol_type": { "type": "string", "default": "" },
                    "file_path": { "type": "string", "default": "" },
                    "limit": { "type": "integer", "default": 50 }
                },
                "required": ["repo_id"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let name_filter = args.get("name_filter").and_then(|v| v.as_str()).unwrap_or("");
        let symbol_type = args.get("symbol_type").and_then(|v| v.as_str()).unwrap_or("");
        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;

        let repo_id = repo_id.to_string();
        let name_filter = name_filter.to_string();
        let symbol_type = symbol_type.to_string();
        let file_path = file_path.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let mut sql = String::from(
                "SELECT file_path, symbol_type, name, line_start, line_end, signature \
                 FROM code_symbols WHERE repo_id = ?1"
            );
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.clone())];

            if !symbol_type.is_empty() {
                sql.push_str(" AND symbol_type = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(Box::new(symbol_type));
            }
            if !name_filter.is_empty() {
                sql.push_str(" AND name LIKE ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(Box::new(format!("%{}%", name_filter)));
            }
            if !file_path.is_empty() {
                sql.push_str(" AND file_path LIKE ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(Box::new(format!("%{}%", file_path)));
            }
            sql.push_str(&format!(" ORDER BY file_path, line_start LIMIT {}", limit));

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
                Ok(serde_json::json!({
                    "file_path": row.get::<_, String>(0)?,
                    "symbol_type": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "line_start": row.get::<_, i64>(3)?,
                    "line_end": row.get::<_, i64>(4)?,
                    "signature": row.get::<_, Option<String>>(5)?,
                }))
            })?;

            let mut symbols = Vec::new();
            for row in rows {
                symbols.push(row?);
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": symbols.len(),
                "symbols": symbols,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitDependencyGraphTool;

impl McpTool for DevkitDependencyGraphTool {
    fn name(&self) -> &'static str {
        "devkit_dependency_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the cross-repository dependency graph. Returns which local repos a given repo depends on, or which repos depend on it (reverse dependencies). Edges are discovered by parsing Cargo.toml, package.json, and go.mod manifest files.

Use this when the user wants to:
- Understand the impact of changing a shared library ("who depends on X?")
- Explore the architecture of a monorepo or multi-repo workspace
- Find all repos that use a specific local crate/package/module
- Plan refactoring or breaking changes across repo boundaries

Do NOT use this for:
- Code-level "who calls this function" (use devkit_code_symbols instead)
- Full-text search (use devkit_natural_language_query instead)
- Remote/external dependency analysis (this only tracks local repos)

Parameters:
- repo_id: Registered repository ID to query.
- direction: "outgoing" (repos this repo depends on) or "incoming" (repos that depend on this repo). Default: "outgoing".
- relation_type: Optional filter by relation type (default "depends_on").

Returns: JSON array of dependency edges with target_repo_id, relation_type, and confidence score (1.0 = verified local path dependency, 0.7-0.9 = name heuristic match)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "direction": { "type": "string", "default": "outgoing" },
                    "relation_type": { "type": "string", "default": "" }
                },
                "required": ["repo_id"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("outgoing");
        let relation_type = args.get("relation_type").and_then(|v| v.as_str()).unwrap_or("");

        let repo_id = repo_id.to_string();
        let direction = direction.to_string();
        let relation_type = relation_type.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;

            let mut results = Vec::new();
            if direction == "incoming" || direction == "reverse" {
                let rows = crate::dependency_graph::list_reverse_dependencies(&conn, &repo_id)?;
                for (from_id, rel, conf) in rows {
                    if !relation_type.is_empty() && rel != relation_type {
                        continue;
                    }
                    results.push(serde_json::json!({
                        "source_repo_id": from_id,
                        "target_repo_id": repo_id,
                        "relation_type": rel,
                        "confidence": conf,
                    }));
                }
            } else {
                let rows = crate::dependency_graph::list_dependencies(&conn, &repo_id)?;
                for (to_id, rel, conf) in rows {
                    if !relation_type.is_empty() && rel != relation_type {
                        continue;
                    }
                    results.push(serde_json::json!({
                        "source_repo_id": repo_id,
                        "target_repo_id": to_id,
                        "relation_type": rel,
                        "confidence": conf,
                    }));
                }
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "direction": direction,
                "count": results.len(),
                "dependencies": results,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitCallGraphTool;

impl McpTool for DevkitCallGraphTool {
    fn name(&self) -> &'static str {
        "devkit_call_graph"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Query the intra-repository call graph extracted by tree-sitter AST parsing. Answer "which functions call X" or "what does function Y call" within a single repo.

Use this when the user wants to:
- Find all call sites of a specific function inside a repo
- Understand the control flow impact of changing a function
- Discover unused functions (no incoming call edges)
- Trace how data flows through the codebase

Do NOT use this for:
- Cross-repo dependency questions (use devkit_dependency_graph instead)
- Finding symbol definitions (use devkit_code_symbols instead)
- Full-text search (use devkit_natural_language_query instead)

Parameters:
- repo_id: Registered repository ID to query.
- callee_name: Name of the called function to search for (required for "who calls X").
- caller_name: Name of the calling function to search for (required for "what does Y call").
- file_path: Optional file path substring to narrow scope.
- limit: Maximum results (default: 50, max: 200).

At least one of callee_name or caller_name must be provided.

Returns: JSON array of call edges with caller_file, caller_symbol, caller_line, callee_name."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "callee_name": { "type": "string", "default": "" },
                    "caller_name": { "type": "string", "default": "" },
                    "file_path": { "type": "string", "default": "" },
                    "limit": { "type": "integer", "default": 50 }
                },
                "required": ["repo_id"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let callee_name = args.get("callee_name").and_then(|v| v.as_str()).unwrap_or("");
        let caller_name = args.get("caller_name").and_then(|v| v.as_str()).unwrap_or("");
        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;

        if callee_name.is_empty() && caller_name.is_empty() {
            anyhow::bail!("At least one of callee_name or caller_name must be provided");
        }

        let repo_id = repo_id.to_string();
        let callee_name = callee_name.to_string();
        let caller_name = caller_name.to_string();
        let file_path = file_path.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let mut sql = String::from(
                "SELECT caller_file, caller_symbol, caller_line, callee_name \
                 FROM code_call_graph WHERE repo_id = ?1"
            );
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(repo_id.clone())];

            if !callee_name.is_empty() {
                sql.push_str(" AND callee_name = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(Box::new(callee_name));
            }
            if !caller_name.is_empty() {
                sql.push_str(" AND caller_symbol = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(Box::new(caller_name));
            }
            if !file_path.is_empty() {
                sql.push_str(" AND caller_file LIKE ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(Box::new(format!("%{}%", file_path)));
            }
            sql.push_str(&format!(" ORDER BY caller_file, caller_line LIMIT {}", limit));

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
                Ok(serde_json::json!({
                    "caller_file": row.get::<_, String>(0)?,
                    "caller_symbol": row.get::<_, String>(1)?,
                    "caller_line": row.get::<_, i64>(2)?,
                    "callee_name": row.get::<_, String>(3)?,
                }))
            })?;

            let mut calls = Vec::new();
            for row in rows {
                calls.push(row?);
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": calls.len(),
                "calls": calls,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitDeadCodeTool;

impl McpTool for DevkitDeadCodeTool {
    fn name(&self) -> &'static str {
        "devkit_dead_code"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Identify potentially dead (unused) functions in a repository by comparing the code symbol index against the call graph. Returns functions that are defined but never called within the same repo.

Use this when the user wants to:
- Clean up unused code in a repository
- Identify functions that may be safe to remove or deprecate
- Audit API surface for internal-only dead functions
- Reduce maintenance burden by eliminating unnecessary code

Do NOT use this for:
- Public API methods that are called by external consumers (devbase only sees intra-repo calls)
- Functions referenced by trait bounds or dynamic dispatch (may have false positives)
- Cross-repo usage analysis (use devkit_dependency_graph + devkit_call_graph instead)

Parameters:
- repo_id: Registered repository ID to analyze.
- limit: Maximum results (default: 50, max: 200).
- include_pub: Also report `pub fn` items (default: false; public functions may be called externally).

Returns: JSON array of potentially dead functions with file_path, name, and line_start."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 },
                    "include_pub": { "type": "boolean", "default": false }
                },
                "required": ["repo_id"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;
        let include_pub = args.get("include_pub").and_then(|v| v.as_bool()).unwrap_or(false);

        let repo_id = repo_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = crate::registry::WorkspaceRegistry::init_db()?;

            // Find all functions in the repo that have NO incoming call edges
            let mut sql = String::from(
                "SELECT cs.file_path, cs.name, cs.line_start \
                 FROM code_symbols cs \
                 WHERE cs.repo_id = ?1 AND cs.symbol_type = 'function' \
                 AND NOT EXISTS ( \
                     SELECT 1 FROM code_call_graph ccg \
                     WHERE ccg.repo_id = cs.repo_id AND ccg.callee_name = cs.name \
                 )"
            );

            if !include_pub {
                // Heuristic: exclude signatures that start with "pub fn"
                // (stored in code_symbols.signature if available)
                sql.push_str(" AND (cs.signature IS NULL OR cs.signature NOT LIKE 'pub fn%')");
            }

            sql.push_str(&format!(" ORDER BY cs.file_path, cs.line_start LIMIT {}", limit));

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([&repo_id], |row| {
                Ok(serde_json::json!({
                    "file_path": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "line_start": row.get::<_, i64>(2)?,
                }))
            })?;

            let mut dead = Vec::new();
            for row in rows {
                dead.push(row?);
            }

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "count": dead.len(),
                "note": "Results may include false positives: public APIs, trait methods, callback registrations, and dynamically dispatched functions are not visible in the intra-repo call graph.",
                "dead_functions": dead,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}

#[derive(Clone)]
pub struct DevkitSemanticSearchTool;

impl McpTool for DevkitSemanticSearchTool {
    fn name(&self) -> &'static str {
        "devkit_semantic_search"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Search for code symbols semantically similar to a natural language query. Uses local vector embeddings (via Ollama) to find function definitions that match the meaning of your query, not just the keywords.

Use this when the user wants to:
- Find code related to a concept (e.g., "authentication", "error handling", "config parsing")
- Discover functions by what they do, not what they're named
- Explore unfamiliar codebases using natural language

Do NOT use this for:
- Exact keyword searches (use devkit_natural_language_query or devkit_query instead)
- Finding symbol definitions by exact name (use devkit_code_symbols instead)
- When the embedding provider (Ollama) is not configured or available

Parameters:
- repo_id: Registered repository ID to search within.
- query: Natural language description of what you're looking for (e.g., "token validation logic").
- limit: Maximum results (default: 10, max: 50).

Returns: JSON array of matching symbols with file_path, name, line_start, and similarity_score (0.0-1.0). Requires [embedding] enabled in config.toml and Ollama running locally."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": { "type": "string" },
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "default": 10 }
                },
                "required": ["repo_id", "query"]
            }
        })
    }

    async fn invoke(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let query = args.get("query").and_then(|v| v.as_str()).context("query required")?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(50) as usize;

        let config = crate::config::Config::load()?;
        if !config.embedding.enabled {
            anyhow::bail!(
                "Semantic search is disabled. Enable it by setting [embedding] enabled = true in config.toml and ensure Ollama is running."
            );
        }

        let repo_id = repo_id.to_string();
        let query = query.to_string();
        let emb_config = config.embedding.clone();

        tokio::task::spawn_blocking(move || {
            // Generate query embedding
            let rt = tokio::runtime::Runtime::new()?;
            let query_embs = rt.block_on(crate::embedding::generate_embeddings(
                &[query.clone()],
                &emb_config,
            ));
            if query_embs.is_empty() {
                anyhow::bail!("Failed to generate query embedding. Is Ollama running with model {}?", emb_config.model);
            }
            let query_emb = &query_embs[0];

            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let results = crate::registry::WorkspaceRegistry::semantic_search_symbols(
                &conn, &repo_id, query_emb, limit,
            )?;

            let symbols: Vec<serde_json::Value> = results
                .into_iter()
                .map(|(_repo, name, path, line, sim)| {
                    serde_json::json!({
                        "name": name,
                        "file_path": path,
                        "line_start": line,
                        "similarity_score": sim,
                    })
                })
                .collect();

            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "repo_id": repo_id,
                "query": query,
                "count": symbols.len(),
                "symbols": symbols,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
