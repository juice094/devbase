use crate::mcp::McpTool;
use crate::mcp::clients::KnowledgeClient;
use crate::mcp::tools::parse_github_repo;
use crate::storage::AppContext;
use anyhow::Context;
use rusqlite::OptionalExtension;

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
    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let repo_id = args
            .get("repo_id")
            .and_then(|v| v.as_str())
            .context("repo_id required")?
            .to_string();
        let write_summary = args.get("write_summary").and_then(|v| v.as_bool()).unwrap_or(false);

        let pool = ctx.pool();
        let upstream_url = tokio::task::spawn_blocking({
            let repo_id = repo_id.clone();
            move || -> anyhow::Result<Option<String>> {

            let conn = pool.get()?;
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
            KnowledgeClient::save_summary(ctx, &repo_id, desc, "")?;
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
pub struct DevkitArxivFetchTool;

impl McpTool for DevkitArxivFetchTool {
    fn name(&self) -> &'static str {
        "devkit_arxiv_fetch"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": "Fetch paper metadata from arXiv by ID. Returns title, authors, summary, published date, and primary category.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "arxiv_id": { "type": "string", "description": "e.g. 2401.12345" }
                },
                "required": ["arxiv_id"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let arxiv_id = args
            .get("arxiv_id")
            .and_then(|v| v.as_str())
            .context("Missing required argument: arxiv_id")?
            .to_string();

        let metadata =
            tokio::task::spawn_blocking(move || crate::arxiv::fetch_arxiv_metadata(&arxiv_id))
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))?;

        match metadata {
            Ok(m) => Ok(serde_json::json!({
                "success": true,
                "title": m.title,
                "authors": m.authors,
                "summary": m.summary,
                "published": m.published,
                "primary_category": m.primary_category,
            })),
            Err(e) => Ok(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            })),
        }
    }
}
