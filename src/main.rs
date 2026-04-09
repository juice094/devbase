use clap::{Parser, Subcommand};
use tracing::{info, warn};

mod asyncgit;
mod health;
mod mcp;
mod query;
mod registry;
mod scan;
mod sync;
mod sync_protocol;
mod syncthing_client;
mod tui;
mod watch;

#[derive(Parser)]
#[command(name = "devbase")]
#[command(about = "Developer workspace database and knowledge-base manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a directory for Git repositories and register them
    Scan {
        /// Directory to scan (defaults to workspace root)
        #[arg(default_value = ".")]
        path: String,
        /// Register discovered repos into the database
        #[arg(long)]
        register: bool,
    },
    /// Check the health of registered repositories and the environment
    Health {
        /// Show detailed per-repo status
        #[arg(long)]
        detail: bool,
    },
    /// Sync registered repositories with their upstream remotes
    Sync {
        /// Dry-run: show what would be updated without applying
        #[arg(long)]
        dry_run: bool,
        /// Sync strategy: auto-pull, fetch-only, or ask
        #[arg(long, default_value = "fetch-only")]
        strategy: String,
        /// Comma-separated list of tags to filter repositories (OR logic)
        #[arg(long)]
        filter_tags: Option<String>,
    },
    /// Query the knowledge base
    Query {
        /// Query expression, e.g. "lang:rust stale:>30"
        query: String,
    },
    /// Remove archive/backup entries from registry
    Clean,
    /// Tag a registered repository
    Tag {
        /// Repository ID
        repo_id: String,
        /// Comma-separated tags
        tags: String,
    },
    /// Launch interactive TUI
    Tui,
    /// Run as an MCP server
    Mcp {
        /// Transport protocol: stdio or sse
        #[arg(long, default_value = "stdio")]
        transport: String,
    },
    /// Watch a directory for changes and schedule sync actions
    Watch {
        /// Directory to watch
        #[arg(default_value = ".")]
        path: String,
        /// Watch duration in seconds
        #[arg(long, default_value = "10")]
        duration: u64,
    },
    /// Push registered repositories to Syncthing as sync folders
    SyncthingPush {
        /// Syncthing REST API base URL
        #[arg(long, default_value = "http://127.0.0.1:8384")]
        api_url: String,
        /// Syncthing API key (optional if no auth)
        #[arg(long)]
        api_key: Option<String>,
        /// Only push repos matching these tags (comma-separated, OR logic)
        #[arg(long)]
        filter_tags: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, register } => {
            info!("Scanning directory: {}", path);
            scan::run(&path, register).await?;
        }
        Commands::Health { detail } => {
            info!("Running health check...");
            health::run(detail).await?;
        }
        Commands::Sync {
            dry_run,
            strategy,
            filter_tags,
        } => {
            if dry_run {
                warn!("Dry-run mode enabled");
            }
            info!("Syncing with strategy: {}", strategy);
            sync::run(dry_run, &strategy, filter_tags.as_deref()).await?;
        }
        Commands::Query { query } => {
            info!("Querying: {}", query);
            query::run(&query).await?;
        }
        Commands::Clean => {
            info!("Cleaning backup entries from registry");
            let conn = registry::WorkspaceRegistry::init_db()?;
            let deleted = conn.execute(
                "DELETE FROM repos WHERE id LIKE 'Clarity_%' OR id LIKE 'clarity_backup%'",
                [],
            )?;
            println!("Deleted {} backup entries from devbase registry.", deleted);
            println!("\nRemaining registered repos:");
            let mut stmt = conn.prepare("SELECT id, local_path FROM repos")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (id, path) = row?;
                println!("  [{}] {}", id, path);
            }
        }
        Commands::Tag { repo_id, tags } => {
            info!("Tagging {} with {}", repo_id, tags);
            let conn = registry::WorkspaceRegistry::init_db()?;
            let updated = conn.execute(
                "UPDATE repos SET tags = ?1 WHERE id = ?2",
                rusqlite::params![tags, repo_id],
            )?;
            if updated == 0 {
                println!("Repository '{}' not found in registry.", repo_id);
            } else {
                println!("Tagged '{}' with '{}'.", repo_id, tags);
            }
        }
        Commands::Tui => {
            info!("Launching TUI");
            tui::run().await?;
        }
        Commands::Mcp { transport } => {
            if transport == "stdio" {
                mcp::run_stdio().await?;
            } else {
                anyhow::bail!("Unsupported transport: {}", transport);
            }
        }
        Commands::Watch { path, duration } => {
            use std::time::Duration;
            use watch::{FolderScheduler, FsWatcher, WatchAggregator};

            let root = std::path::PathBuf::from(&path);
            let watcher = FsWatcher::new(&root)?;
            let aggregator = WatchAggregator::default();
            let mut scheduler = FolderScheduler::new(root.clone());

            println!("Watching {} for {} seconds...", path, duration);
            let start = std::time::Instant::now();
            let total_duration = Duration::from_secs(duration);

            while start.elapsed() < total_duration {
                let remaining = total_duration.saturating_sub(start.elapsed());
                if let Some(events) = watcher.poll_event(remaining) {
                    let aggregated = aggregator.aggregate(events);
                    let actions = scheduler.check_and_schedule(aggregated)?;
                    if !actions.is_empty() {
                        println!(
                            "Detected changes, actions: {:?}",
                            actions
                        );
                    }
                }
            }

            println!("Watch completed for {}", path);
        }
        Commands::SyncthingPush {
            api_url,
            api_key,
            filter_tags,
        } => {
            use registry::WorkspaceRegistry;
            use syncthing_client::SyncthingClient;

            let client = SyncthingClient::new(&api_url, api_key.as_deref());

            let conn = match WorkspaceRegistry::init_db() {
                Ok(c) => c,
                Err(e) => {
                    println!("无法初始化数据库: {}", e);
                    return Ok(());
                }
            };

            let repos = match WorkspaceRegistry::list_repos(&conn) {
                Ok(r) => r,
                Err(e) => {
                    println!("无法读取仓库列表: {}", e);
                    return Ok(());
                }
            };

            let filter_list: Vec<&str> = filter_tags
                .as_deref()
                .map(|f| f.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default();

            let filtered_repos: Vec<_> = repos
                .into_iter()
                .filter(|repo| {
                    let tags = repo.tags.join(",");
                    filter_list.is_empty() || filter_list.iter().any(|f| tags.contains(f))
                })
                .collect();

            if filtered_repos.is_empty() {
                println!("没有符合条件的仓库需要推送。");
                return Ok(());
            }

            let mut pushed = Vec::new();
            let mut connection_failed = false;
            for repo in &filtered_repos {
                let folder_id = format!("devbase-{}", repo.id);
                let path = repo.local_path.to_string_lossy().to_string();
                match client.create_or_update_folder(&folder_id, &path, &[]).await {
                    Ok(()) => {
                        println!("  [{}] Pushed {} -> {}", repo.id, folder_id, path);
                        pushed.push((repo.id.clone(), folder_id));
                    }
                    Err(e) => {
                        let msg = e.to_string().to_lowercase();
                        if msg.contains("connection") || msg.contains("connect") || msg.contains("error sending request") {
                            if !connection_failed {
                                println!("无法连接到 Syncthing API，请确认 Syncthing 正在运行且 API 地址正确。");
                                connection_failed = true;
                            }
                        } else {
                            println!("  [{}] ERROR: {}", repo.id, e);
                        }
                    }
                }
            }

            // Try to fetch folder status for successfully pushed repos
            if !pushed.is_empty() && !connection_failed {
                println!("\nQuerying folder status from Syncthing...");
                for (repo_id, folder_id) in &pushed {
                    match client.get_folder_status(folder_id).await {
                        Ok(status) => {
                            let state = status.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
                            println!("  [{}] state: {}", repo_id, state);
                        }
                        Err(e) => {
                            let msg = e.to_string().to_lowercase();
                            if msg.contains("connection") || msg.contains("connect") || msg.contains("error sending request") {
                                println!("无法连接到 Syncthing API，请确认 Syncthing 正在运行且 API 地址正确。");
                                break;
                            } else {
                                println!("  [{}] status query failed: {}", repo_id, e);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
