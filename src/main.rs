use clap::{Parser, Subcommand};

use devbase::*;

mod commands;

#[derive(Parser)]
#[command(name = "devbase", version)]
#[command(about = "Developer workspace database and knowledge-base manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
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
    #[cfg(feature = "tui")]
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
    #[cfg(feature = "watch")]
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
    /// Workflow Engine — orchestrate multi-Skill pipelines
    Workflow {
        #[command(subcommand)]
        cmd: WorkflowCommands,
    },
    /// Manage known system limits (L3 risk layer)
    Limit {
        #[command(subcommand)]
        cmd: LimitCommands,
    },
}

#[derive(Subcommand)]
pub(crate) enum SkillCommands {
    /// List installed skills
    List {
        /// Filter by skill type (builtin, custom, system)
        #[arg(long)]
        skill_type: Option<String>,
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
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
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Search skills by name or description
    Search {
        /// Query string
        query: String,
        /// Use semantic search (requires embeddings)
        #[arg(long)]
        semantic: bool,
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
        /// Maximum results
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
        /// Output full result as JSON
        #[arg(long)]
        json: bool,
    },
    /// Validate a local SKILL.md file
    Validate {
        /// Path to SKILL.md or skill directory
        path: String,
    },
    /// Validate and prepare a skill for publishing
    Publish {
        /// Path to skill directory (default: current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Dry-run: validate without creating tag
        #[arg(long)]
        dry_run: bool,
    },
    /// Sync skills to an external target (e.g. clarity)
    Sync {
        /// Target system to sync to
        #[arg(long)]
        target: String,
    },
    /// Discover and auto-package a project as a Skill
    Discover {
        /// Path to the project directory (or Git URL)
        path: String,
        /// Explicit skill ID (defaults to project name)
        #[arg(long)]
        skill_id: Option<String>,
        /// Dry-run: print generated files without installing
        #[arg(long)]
        dry_run: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Recalculate skill scores from execution history
    RecalcScores,
    /// Show top-rated skills
    Top {
        /// Maximum results
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Recommend skills based on execution scores
    Recommend {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
        /// Maximum results
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub(crate) enum WorkflowCommands {
    /// List registered workflows
    List,
    /// Show workflow definition
    Show {
        /// Workflow ID
        workflow_id: String,
    },
    /// Register a workflow from a YAML file
    Register {
        /// Path to workflow.yaml
        path: String,
    },
    /// Run a workflow
    Run {
        /// Workflow ID
        workflow_id: String,
        /// Workflow inputs as key=value pairs
        #[arg(long = "input")]
        inputs: Vec<String>,
    },
    /// Delete a workflow
    Delete {
        /// Workflow ID
        workflow_id: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum VaultCommands {
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
pub(crate) enum LimitCommands {
    /// Add or update a known limit
    Add {
        /// Unique identifier (kebab-case recommended)
        id: String,
        /// Category: hard-veto, known-bug, external-dep
        #[arg(long, default_value = "known-bug")]
        category: String,
        /// Description of the limit
        #[arg(long)]
        description: Option<String>,
        /// Source reference (e.g., AGENTS.md, oplog)
        #[arg(long)]
        source: Option<String>,
        /// Severity 1-5
        #[arg(long)]
        severity: Option<i32>,
    },
    /// List known limits
    List {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
        /// Filter by mitigated status
        #[arg(long)]
        mitigated: Option<bool>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Resolve (mitigate) a known limit
    Resolve {
        /// Limit ID
        id: String,
        /// Reason for resolution (optional, stored in L4 metacognition layer)
        #[arg(long)]
        reason: Option<String>,
    },
    /// Delete a known limit
    Delete {
        /// Limit ID
        id: String,
    },
    /// Seed hard vetoes from AGENTS.md into the registry
    Seed,
}

#[derive(Subcommand)]
pub(crate) enum RegistryCommands {
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

    let mut ctx = storage::AppContext::with_defaults()?;
    let lang = if ctx.config.general.language == "auto" || ctx.config.general.language.is_empty() {
        let detected = i18n::detect_system_language();
        ctx.config.general.language = detected.clone();
        if let Err(e) = ctx.config.save() {
            eprintln!("警告: 无法保存语言配置: {}", e);
        }
        detected
    } else {
        ctx.config.general.language.clone()
    };
    i18n::init(&lang);

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, register } => {
            commands::simple::run_scan(&mut ctx, &path, register).await?;
        }
        Commands::Health { detail, limit, page } => {
            commands::simple::run_health(&mut ctx, detail, limit, page).await?;
        }
        Commands::Sync {
            dry_run,
            filter_tags,
            exclude,
            json,
        } => {
            commands::simple::run_sync(&mut ctx, dry_run, filter_tags, exclude, json).await?;
        }
        Commands::Query { query, limit, page } => {
            commands::simple::run_query(&mut ctx, &query, limit, page).await?;
        }
        Commands::Index { path } => {
            commands::simple::run_index(&mut ctx, &path).await?;
        }
        Commands::Clean => {
            commands::simple::run_clean(&mut ctx)?;
        }
        Commands::Tag { repo_id, tags } => {
            commands::simple::run_tag(&mut ctx, &repo_id, &tags)?;
        }
        Commands::Meta { repo_id, tier, workspace_type } => {
            commands::simple::run_meta(&mut ctx, &repo_id, tier, workspace_type)?;
        }
        #[cfg(feature = "tui")]
        Commands::Tui => {
            commands::simple::run_tui(&mut ctx).await?;
        }
        Commands::Mcp { tools } => {
            commands::simple::run_mcp(&mut ctx, tools).await?;
        }
        Commands::Daemon { interval } => {
            commands::simple::run_daemon(&mut ctx, interval).await?;
        }
        #[cfg(feature = "watch")]
        Commands::Watch { path, duration } => {
            commands::simple::run_watch(&mut ctx, &path, duration).await?;
        }
        Commands::SkillSync { output, filter_tags, dry_run } => {
            commands::simple::run_skill_sync(&mut ctx, &output, filter_tags, dry_run)?;
        }
        Commands::SyncthingPush {
            api_url,
            api_key,
            filter_tags,
            experiment,
        } => {
            commands::simple::run_syncthing_push(&mut ctx, api_url, api_key, filter_tags, experiment)
                .await?;
        }
        Commands::Digest => {
            commands::simple::run_digest(&mut ctx).await?;
        }
        Commands::Oplog { limit, repo } => {
            commands::simple::run_oplog(&mut ctx, limit, repo)?;
        }
        Commands::Discover => {
            commands::simple::run_discover(&mut ctx)?;
        }
        Commands::Registry { cmd } => {
            commands::simple::run_registry(&mut ctx, cmd)?;
        }
        Commands::Vault { cmd } => {
            commands::simple::run_vault(&mut ctx, cmd).await?;
        }
        Commands::Skill { cmd } => {
            commands::skill::run_skill(&mut ctx, cmd)?;
        }
        Commands::Workflow { cmd } => {
            commands::workflow::run_workflow(&mut ctx, cmd)?;
        }
        Commands::Limit { cmd } => {
            commands::limit::run_limit(&mut ctx, cmd)?;
        }
    }

    Ok(())
}
