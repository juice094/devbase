use clap::{Parser, Subcommand};
use tracing::{info, warn};

mod asyncgit;
mod config;
mod daemon;
mod digest;
mod health;
mod discovery_engine;
mod i18n;
mod knowledge_engine;
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
    /// Index repository summaries and module structures
    Index {
        /// Specific path to index; if omitted, index all registered repos
        #[arg(default_value = "")]
        path: String,
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
    /// Start the background daemon for knowledge maintenance
    Daemon {
        /// Tick interval in seconds
        #[arg(long)]
        interval: Option<u64>,
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
    /// Auto-discover relationships between registered repositories
    Discover,
    /// Generate daily knowledge digest
    Digest,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = config::Config::load()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, register } => {
            info!("{}: {}", i18n::cli::SCANNING, path);
            scan::run(&path, register).await?;
        }
        Commands::Health { detail } => {
            info!("{}", i18n::cli::HEALTH_CHECK);
            health::run(detail, config.cache.ttl_seconds).await?;
        }
        Commands::Sync {
            dry_run,
            strategy,
            filter_tags,
        } => {
            if dry_run {
                warn!("Dry-run mode enabled");
            }
            info!("{}: {}", i18n::cli::SYNCING, strategy);
            sync::run(dry_run, &strategy, filter_tags.as_deref()).await?;
        }
        Commands::Query { query } => {
            info!("{}: {}", i18n::cli::QUERYING, query);
            query::run(&query, &config).await?;
        }
        Commands::Index { path } => {
            info!("{}: path='{}'", i18n::cli::INDEXING, path);
            let path = path.clone();
            let count = tokio::task::spawn_blocking(move || crate::knowledge_engine::run_index(&path))
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            info!("已索引 {} 个仓库", count);
        }
        Commands::Clean => {
            info!("正在清理注册表中的备份条目");
            let conn = registry::WorkspaceRegistry::init_db()?;
            let deleted = conn.execute(
                "DELETE FROM repos WHERE id LIKE 'Clarity_%' OR id LIKE 'clarity_backup%'",
                [],
            )?;
            println!("已从 devbase 注册表中删除 {} 个备份条目。", deleted);
            println!("\n剩余已注册仓库:");
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
            info!("为 {} 打标签: {}", repo_id, tags);
            let mut conn = registry::WorkspaceRegistry::init_db()?;
            let tag_list: Vec<&str> = tags.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let tx = conn.transaction()?;
            let exists: bool = tx.query_row("SELECT 1 FROM repos WHERE id = ?1", [&repo_id], |_| Ok(true)).unwrap_or(false);
            if !exists {
                println!("注册表中未找到仓库 '{}'。", repo_id);
            } else {
                tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1", [&repo_id])?;
                for tag in &tag_list {
                    tx.execute(
                        "INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                        rusqlite::params![&repo_id, tag],
                    )?;
                }
                tx.commit()?;
                println!("已为 '{}' 打上标签 '{}'。", repo_id, tags);
            }
        }
        Commands::Tui => {
            info!("{}", i18n::cli::LAUNCHING_TUI);
            tui::run().await?;
        }
        Commands::Mcp { transport } => {
            if transport == "stdio" {
                mcp::run_stdio().await?;
            } else {
                anyhow::bail!("Unsupported transport: {}", transport);
            }
        }
        Commands::Daemon { interval } => {
            let interval = interval.unwrap_or(config.daemon.interval_seconds);
            let d = daemon::Daemon::new(interval, config.clone());
            d.run().await?;
        }
        Commands::Watch { path, duration } => {
            use std::time::Duration;
            use watch::{FolderScheduler, FsWatcher, WatchAggregator};

            let root = std::path::PathBuf::from(&path);
            let watcher = FsWatcher::new(&root)?;
            let aggregator = WatchAggregator {
                max_files: config.watch.max_files,
                ..Default::default()
            };
            let mut scheduler = FolderScheduler::with_max_files(root.clone(), config.watch.max_files);

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
                    filter_list.is_empty() || filter_list.iter().any(|f| repo.tags.contains(&f.to_string()))
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
        Commands::Digest => {
            let digest_config = config.digest.clone();
            match tokio::task::spawn_blocking(move || {
                let conn = registry::WorkspaceRegistry::init_db()?;
                let cfg = crate::config::Config {
                    digest: digest_config,
                    ..Default::default()
                };
                digest::generate_daily_digest(&conn, &cfg)
            })
            .await
            {
                Ok(Ok(text)) => println!("{}", text),
                Ok(Err(e)) => println!("生成日报失败: {}", e),
                Err(e) => println!("日报任务崩溃: {}", e),
            }
        }
        Commands::Discover => {
            use discovery_engine::{discover_dependencies, discover_similar_projects, Discovery};
            use registry::WorkspaceRegistry;
            use std::collections::HashMap;

            let conn = WorkspaceRegistry::init_db()?;
            let repos = WorkspaceRegistry::list_repos(&conn)?;

            let deps = discover_dependencies(&repos);
            let sims = discover_similar_projects(&conn)?;

            let mut merged: HashMap<(String, String, String), Discovery> = HashMap::new();
            for d in deps.into_iter().chain(sims.into_iter()) {
                let key = (d.from.clone(), d.to.clone(), d.relation_type.clone());
                if let Some(existing) = merged.get(&key) {
                    if d.confidence > existing.confidence {
                        merged.insert(key, d);
                    }
                } else {
                    merged.insert(key, d);
                }
            }

            for d in merged.values() {
                WorkspaceRegistry::save_relation(&conn, &d.from, &d.to, &d.relation_type, d.confidence)?;
            }

            let mut all: Vec<Discovery> = merged.into_values().collect();
            all.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

            let dep_count = all.iter().filter(|d| d.relation_type == "depends_on").count();
            let sim_count = all.iter().filter(|d| d.relation_type == "similar_to").count();
            println!("Found {} dependencies and {} similarities.", dep_count, sim_count);

            println!("Top 10 discoveries:");
            for d in all.iter().take(10) {
                println!(
                    "  [{}] {} -> {} (confidence={:.2}): {}",
                    d.relation_type, d.from, d.to, d.confidence, d.description
                );
            }
        }
    }

    Ok(())
}
