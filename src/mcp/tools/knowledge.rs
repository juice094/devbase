use crate::mcp::McpTool;
use crate::mcp::clients::{DigestClient, KnowledgeClient};
use crate::registry::{ExperimentEntry, PaperEntry};
use crate::repository::knowledge::KnowledgeRepository;
use crate::storage::AppContext;
use anyhow::Context;

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

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).context("repo_id required")?;
        let text = args.get("text").and_then(|v| v.as_str()).context("text required")?;
        let author = args.get("author").and_then(|v| v.as_str()).unwrap_or("ai");
        KnowledgeClient::save_note(ctx, repo_id, text, author)
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
    async fn invoke(
        &self,
        _args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        DigestClient::generate_daily_digest(ctx)
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
    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
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

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
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
                        let paper = PaperEntry {
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
                        KnowledgeRepository::new(&conn).save_paper_entry(&paper)?;
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
    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let id = args.get("id").and_then(|v| v.as_str()).context("id required")?.to_string();
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).map(String::from);
        let tag_repo = args.get("tag_repo").and_then(|v| v.as_bool()).unwrap_or(false);
        let exp = ExperimentEntry {
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
        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {

            let mut conn = pool.get()?;
            KnowledgeRepository::new(&conn).save_experiment_entry(&exp)?;
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
pub struct DevkitKnowledgeReportTool;

impl McpTool for DevkitKnowledgeReportTool {
    fn name(&self) -> &'static str {
        "devkit_knowledge_report"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Generate a knowledge coverage report for the workspace or a single repository. Shows symbol counts, embedding coverage, call graph density, health summary, and recent activity.

Use this when the user wants to:
- Get an overview of how well the workspace is indexed
- Check which repos have embeddings and which don't
- See recent scan/index/sync activity
- Identify knowledge gaps (repos with many symbols but zero embeddings)

Parameters:
- repo_id: Optional specific repository ID. If omitted, reports on the entire workspace.
- activity_limit: Number of recent OpLog events to include (default: 20, max: 100).

Returns: JSON object with repo_count, total_symbols, total_embeddings, total_calls, overall_coverage_pct, per-repo breakdown, health_summary, and recent_activity."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Optional specific repository to report on"
                    },
                    "activity_limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Number of recent activity events to include"
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
        let repo_id = args.get("repo_id").and_then(|v| v.as_str()).map(String::from);
        let activity_limit =
            args.get("activity_limit").and_then(|v| v.as_u64()).unwrap_or(20).min(100) as usize;

        let pool = ctx.pool();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let report =
                crate::oplog_analytics::generate_report(&conn, repo_id.as_deref(), activity_limit)?;

            let json = serde_json::to_value(report)?;
            Ok::<_, anyhow::Error>(serde_json::json!({
                "success": true,
                "report": json,
            }))
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?
    }
}
