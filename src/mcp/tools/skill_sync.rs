use anyhow::Context;

use crate::mcp::McpTool;
use crate::skill_runtime::sources::{GitHubSource, LocalFileSource, SkillSource};
use crate::skill_runtime::registry;

#[derive(Clone)]
pub struct DevkitSkillSyncTool;

impl McpTool for DevkitSkillSyncTool {
    fn name(&self) -> &'static str {
        "devkit_skill_sync"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Sync skills from external sources (GitHub repositories or local directories) into the devbase skill registry.
Each discovered SKILL.md file is parsed and installed. Sources are recorded and can be re-synced incrementally.

Source URL formats:
- GitHub: https://github.com/owner/repo/tree/branch/path or owner/repo
- Local: file:///absolute/path or /absolute/path

Requires DEVBASE_MCP_ENABLE_DESTRUCTIVE=1 since this modifies the skill registry."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source URL or path to sync skills from. GitHub repo URL or local directory path."
                    },
                    "source_path": {
                        "type": "string",
                        "description": "Path within the source to scan (default: '.' for local, 'skills' for GitHub)."
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "If true, list discovered skills without installing them.",
                        "default": false
                    }
                },
                "required": ["source"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        crate::mcp::check_destructive_enabled()?;

        let source_url = args
            .get("source")
            .and_then(|v| v.as_str())
            .context("Missing required argument: source")?;
        let source_path = args.get("source_path").and_then(|v| v.as_str());
        let dry_run = args
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let source: Box<dyn SkillSource> = if source_url.starts_with("https://github.com/")
            || source_url.starts_with("http://github.com/")
            || (source_url.contains('/') && !source_url.starts_with("/") && !source_url.contains("://"))
        {
            let (owner, repo) = parse_github_url(source_url)?;
            let path = source_path.unwrap_or("skills");
            Box::new(GitHubSource::new(&owner, &repo, path))
        } else {
            let path = source_url.strip_prefix("file://").unwrap_or(source_url);
            let name = source_path.unwrap_or(path);
            Box::new(LocalFileSource::new(name, std::path::Path::new(path)))
        };

        let skills = source.fetch().await?;
        let count = skills.len();

        if dry_run {
            let names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
            return Ok(serde_json::json!({
                "dry_run": true,
                "source": source.name(),
                "skills_found": count,
                "skill_names": names
            }));
        }

        let conn = ctx.conn_mut()?;
        let mut added = 0usize;
        let mut updated = 0usize;

        for skill in &skills {
            let exists = registry::get_skill(&conn, &skill.id)?.is_some();
            registry::install_skill(&conn, skill)?;
            if exists {
                updated += 1;
            } else {
                added += 1;
            }
        }

        // Record sync in audit log
        let _ = conn.execute(
            "INSERT INTO sync_log (source_name, status, skills_added, skills_updated, finished_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![source.name(), "success", added as i64, updated as i64],
        );

        // Update last_sync_at on the source record
        let _ = conn.execute(
            "INSERT INTO sync_sources (name, url, source_type)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET last_sync_at = datetime('now')",
            rusqlite::params![source.name(), source_url, source.name()],
        );

        Ok(serde_json::json!({
            "source": source.name(),
            "skills_found": count,
            "skills_added": added,
            "skills_updated": updated,
            "dry_run": false
        }))
    }
}

fn parse_github_url(url: &str) -> anyhow::Result<(String, String)> {
    let url = url.trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = url.strip_prefix("http://github.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }
    // Bare owner/repo format
    if let Some((owner, repo)) = url.split_once('/') {
        if !owner.is_empty() && !repo.is_empty()
            && !owner.contains("://")
            && !owner.contains('\\')
            && !owner.contains(' ')
        {
            return Ok((owner.to_string(), repo.to_string()));
        }
    }
    Err(anyhow::anyhow!(
        "Could not parse GitHub URL: {}. Expected format: owner/repo or https://github.com/owner/repo",
        url
    ))
}
