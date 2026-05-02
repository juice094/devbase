use anyhow::Context;
use devbase::*;
use devbase::mcp::clients::RegistryClient;
use rusqlite::OptionalExtension;
use tracing::{info, warn};

pub async fn run_tui(ctx: &mut crate::storage::AppContext) -> anyhow::Result<()> {
    info!("{}", ctx.i18n.cli.launching_tui);
    tui::run().await
}

pub async fn run_mcp(
    _ctx: &mut crate::storage::AppContext,
    tools: Option<String>,
) -> anyhow::Result<()> {
    if let Some(tiers) = tools {
        // SAFETY: set_var is called once at program startup before any
        // threads read the environment. The MCP server runs in a single
        // subprocess, so concurrent reads are not possible.
        unsafe {
            std::env::set_var("DEVBASE_MCP_TOOL_TIERS", tiers);
        }
    }
    mcp::run_stdio().await
}

pub async fn run_daemon(
    ctx: &mut crate::storage::AppContext,
    interval: Option<u64>,
) -> anyhow::Result<()> {
    let interval = interval.unwrap_or(ctx.config.daemon.interval_seconds);
    let config = ctx.config.clone();
    let pool = ctx.pool();
    let d = daemon::Daemon::new(interval, config, pool);
    d.run().await
}

pub async fn run_github_info(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    write_summary: bool,
    json: bool,
) -> anyhow::Result<()> {
    let conn = ctx.conn()?;
    let url: Option<String> = conn
        .query_row(
            "SELECT upstream_url FROM repo_remotes WHERE repo_id = ?1 AND remote_name = 'origin'",
            [repo_id],
            |row| row.get(0),
        )
        .optional()?;
    let upstream_url = url.context("No origin remote found for repo")?;

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
        anyhow::bail!("GitHub API error {}: {}", status, text);
    }
    let data: serde_json::Value = resp.json().await?;

    let stars = data.get("stargazers_count").and_then(|v| v.as_i64());
    let forks = data.get("forks_count").and_then(|v| v.as_i64());
    let description = data.get("description").and_then(|v| v.as_str());
    let language = data.get("language").and_then(|v| v.as_str());
    let open_issues = data.get("open_issues_count").and_then(|v| v.as_i64());
    let updated_at = data.get("updated_at").and_then(|v| v.as_str());
    let html_url = data.get("html_url").and_then(|v| v.as_str());

    if write_summary && let Some(desc) = description {
        crate::registry::knowledge::save_summary(&conn, repo_id, desc, "github-info")?;
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "owner": owner,
                "repo": repo_name,
                "stars": stars,
                "forks": forks,
                "description": description,
                "language": language,
                "open_issues": open_issues,
                "updated_at": updated_at,
                "html_url": html_url,
            }))?
        );
    } else {
        println!("GitHub: {}/{}", owner, repo_name);
        println!("  URL: {}", html_url.unwrap_or("N/A"));
        println!("  Stars: {} | Forks: {} | Issues: {}",
            stars.map_or("N/A".to_string(), |s| s.to_string()),
            forks.map_or("N/A".to_string(), |s| s.to_string()),
            open_issues.map_or("N/A".to_string(), |s| s.to_string()),
        );
        println!("  Language: {} | Updated: {}",
            language.unwrap_or("N/A"),
            updated_at.unwrap_or("N/A"),
        );
        if let Some(d) = description {
            println!("  Description: {}", d);
        }
    }
    Ok(())
}

