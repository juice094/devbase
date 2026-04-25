use clap::{Parser, Subcommand};
use tracing::{info, warn};

use devbase::*;

#[derive(Parser)]
#[command(name = "devbase", version)]
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
        /// Maximum number of repos to display per page (0 = unlimited)
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Page number (1-based)
        #[arg(long, default_value_t = 1)]
        page: usize,
    },
    /// Sync registered repositories with their upstream remotes
    Sync {
        /// Dry-run: show what would be updated without applying
        #[arg(long)]
        dry_run: bool,
        /// Comma-separated list of tags to filter repositories (OR logic)
        #[arg(long)]
        filter_tags: Option<String>,
        /// Comma-separated list of repo IDs to exclude from sync
        #[arg(long)]
        exclude: Option<String>,
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },
    /// Query the knowledge base
    Query {
        /// Query expression, e.g. "lang:rust stale:>30"
        query: String,
        /// Maximum number of results per page (0 = unlimited)
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Page number (1-based)
        #[arg(long, default_value_t = 1)]
        page: usize,
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
    /// Update metadata (tier / workspace type) of a registered repository
    Meta {
        /// Repository ID
        repo_id: String,
        /// Data tier: public, cooperative, or private
        #[arg(long)]
        tier: Option<String>,
        /// Workspace type: git, openclaw, or generic
        #[arg(long)]
        workspace_type: Option<String>,
    },
    /// Launch interactive TUI
    Tui,
    /// Run as an MCP server (stdio transport)
    Mcp {
        /// Comma-separated tool tiers to expose (stable,beta,experimental).
        /// Defaults to all tiers if omitted.
        #[arg(long)]
        tools: Option<String>,
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
    /// Sync vault notes with ai_context=true to Clarity SKILL.md format
    SkillSync {
        /// Output directory for generated SKILL.md files
        #[arg(long, default_value = "skills")]
        output: String,
        /// Only sync notes matching specific tags (comma-separated)
        #[arg(long)]
        filter_tags: Option<String>,
        /// Preview mode: list what would be synced without writing files
        #[arg(long)]
        dry_run: bool,
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
        /// Only push the repo associated with this experiment ID
        #[arg(long)]
        experiment: Option<String>,
    },
    /// Auto-discover relationships between registered repositories
    Discover,
    /// Generate daily knowledge digest
    Digest,
    /// View the operation log
    Oplog {
        /// Limit number of entries (default: 20)
        #[arg(long, default_value_t = 20)]
        limit: i64,
        /// Filter by repo ID
        #[arg(long)]
        repo: Option<String>,
    },
    /// Registry backup and restore operations
    Registry {
        #[command(subcommand)]
        cmd: RegistryCommands,
    },
    /// Vault note management
    Vault {
        #[command(subcommand)]
        cmd: VaultCommands,
    },
    /// Skill Runtime — install, discover, and execute AI skills
    Skill {
        #[command(subcommand)]
        cmd: SkillCommands,
    },
}

#[derive(Subcommand)]
enum SkillCommands {
    /// List installed skills
    List {
        /// Filter by skill type (builtin, custom, system)
        #[arg(long)]
        skill_type: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Install a skill from a local path or Git URL
    Install {
        /// Path to the skill directory or Git URL (must contain SKILL.md)
        source: String,
        /// Force treating source as a Git URL
        #[arg(long)]
        git: bool,
    },
    /// Uninstall a skill
    Uninstall {
        /// Skill ID to remove
        skill_id: String,
    },
    /// Show skill details
    Info {
        /// Skill ID
        skill_id: String,
    },
    /// Search skills by name or description
    Search {
        /// Query string
        query: String,
        /// Use semantic search (requires embeddings)
        #[arg(long)]
        semantic: bool,
        /// Maximum results
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Execute a skill
    Run {
        /// Skill ID
        skill_id: String,
        /// Arguments as key=value pairs
        #[arg(long = "arg")]
        args: Vec<String>,
        /// Timeout in seconds
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    /// Validate a local SKILL.md file
    Validate {
        /// Path to SKILL.md or skill directory
        path: String,
    },
}

#[derive(Subcommand)]
enum VaultCommands {
    /// Scan a directory for Markdown notes and sync into the vault
    Scan {
        /// Directory to scan (defaults to default vault dir)
        #[arg(default_value = "")]
        path: String,
    },
    /// Rebuild the Tantivy search index for all vault notes
    Reindex,
}

#[derive(Subcommand)]
enum RegistryCommands {
    /// Export registry to a backup file
    Export {
        /// Output format: sqlite or json
        #[arg(long, default_value = "sqlite")]
        format: String,
        /// Output file path (optional, defaults to backup dir with timestamp)
        #[arg(long)]
        output: Option<String>,
    },
    /// Import registry from a backup SQLite file
    Import {
        /// Source backup file path
        path: String,
        /// Skip dry-run and execute immediately
        #[arg(long)]
        yes: bool,
    },
    /// List existing registry backups
    Backups,
    /// Clean old backups, keeping only the most recent ones
    Clean,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut config = config::Config::load()?;
    let lang = if config.general.language == "auto" || config.general.language.is_empty() {
        let detected = i18n::detect_system_language();
        config.general.language = detected.clone();
        if let Err(e) = config.save() {
            eprintln!("警告: 无法保存语言配置: {}", e);
        }
        detected
    } else {
        config.general.language.clone()
    };
    i18n::init(&lang);

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, register } => {
            info!("{}: {}", i18n::current().cli.scanning, path);
            scan::run(&path, register).await?;
        }
        Commands::Health { detail, limit, page } => {
            info!("{}", i18n::current().cli.health_check);
            health::run(detail, limit, page, config.cache.ttl_seconds).await?;
        }
        Commands::Sync {
            dry_run,
            filter_tags,
            exclude,
            json,
        } => {
            if dry_run {
                warn!("Dry-run mode enabled");
            }
            info!("{}", i18n::current().cli.syncing);
            if json {
                let output =
                    sync::run_json(dry_run, filter_tags.as_deref(), exclude.as_deref()).await?;
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                sync::run(dry_run, filter_tags.as_deref(), exclude.as_deref()).await?;
            }
        }
        Commands::Query { query, limit, page } => {
            info!("{}: {}", i18n::current().cli.querying, query);
            query::run(&query, limit, page, &config).await?;
        }
        Commands::Index { path } => {
            info!("{}: path='{}'", i18n::current().cli.indexing, path);
            let path = path.clone();
            let count = tokio::task::spawn_blocking(move || knowledge_engine::run_index(&path))
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            info!("已索引 {} 个仓库", count);
        }
        Commands::Clean => {
            info!("正在清理注册表中的备份条目");
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            let deleted = conn.execute(
                "DELETE FROM repos WHERE id LIKE 'Clarity_%' OR id LIKE 'clarity_backup%'",
                [],
            )?;
            println!("已从 devbase 注册表中删除 {} 个备份条目。", deleted);
            println!("\n剩余已注册仓库:");
            let mut stmt = conn.prepare("SELECT id, local_path FROM repos")?;
            let rows =
                stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
            for row in rows {
                let (id, path) = row?;
                println!("  [{}] {}", id, path);
            }
        }
        Commands::Tag { repo_id, tags } => {
            info!("为 {} 打标签: {}", repo_id, tags);
            let mut conn = registry::WorkspaceRegistry::init_db()?;
            let tag_list: Vec<&str> =
                tags.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let tx = conn.transaction()?;
            let exists: bool = tx
                .query_row("SELECT 1 FROM repos WHERE id = ?1", [&repo_id], |_| Ok(true))
                .unwrap_or(false);
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
        Commands::Meta { repo_id, tier, workspace_type } => {
            info!("更新 {} 的元数据", repo_id);
            let conn = registry::WorkspaceRegistry::init_db()?;
            let exists: bool = conn
                .query_row("SELECT 1 FROM repos WHERE id = ?1", [&repo_id], |_| Ok(true))
                .unwrap_or(false);
            if !exists {
                println!("注册表中未找到仓库 '{}'。", repo_id);
            } else {
                if let Some(ref t) = tier {
                    registry::WorkspaceRegistry::update_repo_tier(&conn, &repo_id, t)?;
                    println!("已将 '{}' 的数据分级设为 '{}'。", repo_id, t);
                }
                if let Some(ref wt) = workspace_type {
                    registry::WorkspaceRegistry::update_repo_workspace_type(&conn, &repo_id, wt)?;
                    println!("已将 '{}' 的工作区类型设为 '{}'。", repo_id, wt);
                }
                if tier.is_none() && workspace_type.is_none() {
                    println!("未提供任何要更新的字段。使用 --tier 或 --workspace-type 指定。");
                }
            }
        }
        Commands::Tui => {
            info!("{}", i18n::current().cli.launching_tui);
            tui::run().await?;
        }
        Commands::Mcp { tools } => {
            if let Some(tiers) = tools {
                // SAFETY: set_var is called once at program startup before any
                // threads read the environment. The MCP server runs in a single
                // subprocess, so concurrent reads are not possible.
                unsafe {
                    std::env::set_var("DEVBASE_MCP_TOOL_TIERS", tiers);
                }
            }
            mcp::run_stdio().await?;
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
            let mut scheduler =
                FolderScheduler::with_max_files(root.clone(), config.watch.max_files);

            println!("Watching {} for {} seconds...", path, duration);
            let start = std::time::Instant::now();
            let total_duration = Duration::from_secs(duration);

            while start.elapsed() < total_duration {
                let remaining = total_duration.saturating_sub(start.elapsed());
                if let Some(events) = watcher.poll_event(remaining) {
                    let aggregated = aggregator.aggregate(events);
                    let actions = scheduler.check_and_schedule(aggregated)?;
                    if !actions.is_empty() {
                        println!("Detected changes, actions: {:?}", actions);
                    }
                }
            }

            println!("Watch completed for {}", path);
        }
        Commands::SkillSync { output, filter_tags, dry_run } => {
            let filter_tags: Vec<String> = filter_tags
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_default();
            match skill_sync::run_sync(&output, &filter_tags, dry_run) {
                Ok(count) => {
                    if dry_run {
                        println!("Would sync {} vault notes to {}", count, output);
                    } else {
                        println!("Synced {} vault notes to {}", count, output);
                    }
                }
                Err(e) => {
                    eprintln!("Skill sync failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::SyncthingPush {
            api_url,
            api_key,
            filter_tags,
            experiment,
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

            let filtered_repos: Vec<_> = if let Some(ref exp_id) = experiment {
                let exps = WorkspaceRegistry::list_experiments(&conn).unwrap_or_default();
                let target_repo =
                    exps.into_iter().find(|e| e.id == *exp_id).and_then(|e| e.repo_id);
                match target_repo {
                    Some(repo_id) => repos.into_iter().filter(|r| r.id == repo_id).collect(),
                    None => {
                        println!("未找到实验 '{}' 关联的仓库。", exp_id);
                        return Ok(());
                    }
                }
            } else {
                repos
                    .into_iter()
                    .filter(|repo| {
                        filter_list.is_empty()
                            || filter_list.iter().any(|f| repo.tags.contains(&f.to_string()))
                    })
                    .collect()
            };

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
                        // Update experiment record if --experiment was provided
                        if let Some(ref exp_id) = experiment
                            && let Ok(mut exps) = WorkspaceRegistry::list_experiments(&conn)
                            && let Some(exp) = exps.iter_mut().find(|e| e.id == *exp_id)
                        {
                            exp.syncthing_folder_id = Some(folder_id.clone());
                            let _ = WorkspaceRegistry::save_experiment(&conn, exp);
                        }
                        pushed.push((repo.id.clone(), folder_id));
                    }
                    Err(e) => {
                        let msg = e.to_string().to_lowercase();
                        if msg.contains("connection")
                            || msg.contains("connect")
                            || msg.contains("error sending request")
                        {
                            if !connection_failed {
                                println!(
                                    "无法连接到 Syncthing API，请确认 Syncthing 正在运行且 API 地址正确。"
                                );
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
                            let state =
                                status.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
                            println!("  [{}] state: {}", repo_id, state);
                        }
                        Err(e) => {
                            let msg = e.to_string().to_lowercase();
                            if msg.contains("connection")
                                || msg.contains("connect")
                                || msg.contains("error sending request")
                            {
                                println!(
                                    "无法连接到 Syncthing API，请确认 Syncthing 正在运行且 API 地址正确。"
                                );
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
                let cfg = config::Config {
                    general: config::GeneralConfig::default(),
                    digest: digest_config,
                    ..Default::default()
                };
                digest::generate_daily_digest(&conn, &cfg)
            })
            .await
            {
                Ok(Ok(text)) => println!("{}", text),
                Ok(Err(e)) => println!("{}: {}", i18n::current().log.digest_failed, e),
                Err(e) => println!("{}: {}", i18n::current().log.digest_panic, e),
            }
        }
        Commands::Oplog { limit, repo } => {
            let conn = registry::WorkspaceRegistry::init_db()?;
            let entries = match repo {
                Some(ref r) => registry::WorkspaceRegistry::list_oplog_by_repo(&conn, r, limit)?,
                None => registry::WorkspaceRegistry::list_oplog(&conn, limit)?,
            };
            if entries.is_empty() {
                println!("操作日志为空。");
            } else {
                println!("最近 {} 条操作日志:", entries.len());
                for entry in entries {
                    let ts = entry.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
                    let repo = entry.repo_id.as_deref().unwrap_or("-");
                    let details_display = if entry.event_version >= 1 {
                        match serde_json::from_str::<serde_json::Value>(
                            entry.details.as_deref().unwrap_or("{}"),
                        ) {
                            Ok(val) => {
                                if let Some(obj) = val.as_object() {
                                    obj.iter()
                                        .map(|(k, v)| format!("{}={}", k, v))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                } else {
                                    entry.details.as_deref().unwrap_or("").to_string()
                                }
                            }
                            Err(_) => entry.details.as_deref().unwrap_or("").to_string(),
                        }
                    } else {
                        entry.details.as_deref().unwrap_or("").to_string()
                    };
                    let duration_display = entry
                        .duration_ms
                        .map(|d| format!(" | duration={}ms", d))
                        .unwrap_or_default();
                    println!(
                        "  [{}] {} | repo={} | status={}{}{}",
                        ts,
                        entry.event_type.as_str(),
                        repo,
                        entry.status,
                        duration_display,
                        if details_display.is_empty() {
                            "".to_string()
                        } else {
                            format!(" | {}", details_display)
                        }
                    );
                }
            }
        }
        Commands::Discover => {
            use discovery_engine::{Discovery, discover_dependencies, discover_similar_projects};
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
                WorkspaceRegistry::save_relation(
                    &conn,
                    &d.from,
                    &d.to,
                    &d.relation_type,
                    d.confidence,
                )?;
            }

            let mut all: Vec<Discovery> = merged.into_values().collect();
            all.sort_by(|a, b| {
                b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
            });

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
        Commands::Registry { cmd } => match cmd {
            RegistryCommands::Export { format, output } => {
                let out_path = output.as_deref().map(std::path::Path::new);
                backup::run_export(&format, out_path)?;
            }
            RegistryCommands::Import { path, yes } => {
                let source = std::path::Path::new(&path);
                backup::run_import(source, yes)?;
            }
            RegistryCommands::Backups => {
                backup::run_list()?;
            }
            RegistryCommands::Clean => {
                backup::run_clean()?;
            }
        },
        Commands::Vault { cmd } => match cmd {
            VaultCommands::Scan { path } => {
                let dir = if path.is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(path))
                };
                let count =
                    tokio::task::spawn_blocking(move || vault::scanner::scan_vault(dir.as_deref()))
                        .await
                        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
                println!("Synced {} vault notes.", count);
            }
            VaultCommands::Reindex => {
                tokio::task::spawn_blocking(vault::indexer::reindex_vault)
                    .await
                    .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
                println!("Vault search index rebuilt.");
            }
        },
        Commands::Skill { cmd } => {
            use skill_runtime::{parser, registry};
            let conn = crate::registry::WorkspaceRegistry::init_db()?;
            match cmd {
                SkillCommands::List { skill_type, json } => {
                    let st = skill_type.as_deref().and_then(|s| s.parse().ok());
                    let skills = registry::list_skills(&conn, st)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&skills)?);
                    } else {
                        if skills.is_empty() {
                            println!("No skills found.");
                        } else {
                            println!("{:<24} {:<10} {:<12} {}", "ID", "Type", "Version", "Description");
                            for s in &skills {
                                println!(
                                    "{:<24} {:<10} {:<12} {}",
                                    s.id, s.skill_type.as_str(), s.version, s.description
                                );
                            }
                        }
                    }
                }
                SkillCommands::Install { source, git } => {
                    let is_git = git || source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@");
                    if is_git {
                        let skill = registry::install_skill_from_git(&conn, &source, None)?;
                        println!("Installed skill '{}' ({}) from {}", skill.name, skill.id, source);
                    } else {
                        let p = std::path::PathBuf::from(&source);
                        let skill_md = if p.is_dir() {
                            p.join("SKILL.md")
                        } else {
                            p.clone()
                        };
                        if !skill_md.exists() {
                            println!("SKILL.md not found at: {}", skill_md.display());
                            return Ok(());
                        }
                        let skill = parser::parse_skill_md(&skill_md)?;
                        registry::install_skill(&conn, &skill)?;
                        println!("Installed skill '{}' ({})", skill.name, skill.id);
                    }
                }
                SkillCommands::Uninstall { skill_id } => {
                    let removed = registry::uninstall_skill(&conn, &skill_id)?;
                    if removed {
                        println!("Uninstalled skill '{}'.", skill_id);
                    } else {
                        println!("Skill '{}' not found.", skill_id);
                    }
                }
                SkillCommands::Info { skill_id } => {
                    match registry::get_skill(&conn, &skill_id)? {
                        Some(s) => {
                            println!("ID:          {}", s.id);
                            println!("Name:        {}", s.name);
                            println!("Version:     {}", s.version);
                            println!("Type:        {}", s.skill_type.as_str());
                            println!("Author:      {}", s.author.as_deref().unwrap_or("-"));
                            println!("Tags:        {}", s.tags.join(", "));
                            println!("Path:        {}", s.local_path);
                            println!("Installed:   {}", s.installed_at.format("%Y-%m-%d %H:%M:%S"));
                            println!("Description: {}", s.description);
                        }
                        None => println!("Skill '{}' not found.", skill_id),
                    }
                }
                SkillCommands::Search { query, semantic: _, limit } => {
                    let results = registry::search_skills_text(&conn, &query, limit)?;
                    if results.is_empty() {
                        println!("No skills matching '{}'.", query);
                    } else {
                        println!("Found {} skill(s):", results.len());
                        for s in &results {
                            println!("  [{}] {} — {}", s.id, s.name, s.description);
                        }
                    }
                }
                SkillCommands::Run { skill_id, args, timeout } => {
                    match registry::get_skill(&conn, &skill_id)? {
                        Some(skill) => {
                            let result = skill_runtime::executor::run_skill(
                                &skill, &args, std::time::Duration::from_secs(timeout),
                            )?;
                            let exec_id = registry::record_execution_start(&conn, &skill_id, &serde_json::to_string(&args).unwrap_or_default())?;
                            registry::record_execution_finish(&conn, exec_id, &result)?;
                            println!("Exit code: {:?}", result.exit_code);
                            if !result.stdout.is_empty() {
                                println!("--- stdout ---\n{}", result.stdout);
                            }
                            if !result.stderr.is_empty() {
                                eprintln!("--- stderr ---\n{}", result.stderr);
                            }
                        }
                        None => println!("Skill '{}' not found.", skill_id),
                    }
                }
                SkillCommands::Validate { path } => {
                    let p = std::path::PathBuf::from(&path);
                    let skill_md = if p.is_dir() { p.join("SKILL.md") } else { p };
                    match parser::parse_skill_md(&skill_md) {
                        Ok(skill) => {
                            println!("✓ Valid SKILL.md: '{}' ({})", skill.name, skill.id);
                            if !skill.inputs.is_empty() {
                                println!("  Inputs:  {}", skill.inputs.len());
                            }
                            if !skill.outputs.is_empty() {
                                println!("  Outputs: {}", skill.outputs.len());
                            }
                        }
                        Err(e) => {
                            println!("✗ Invalid SKILL.md: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
