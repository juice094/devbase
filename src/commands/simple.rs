use devbase::*;
use tracing::{info, warn};

pub async fn run_scan(path: &str, register: bool) -> anyhow::Result<()> {
    info!("{}: {}", i18n::current().cli.scanning, path);
    scan::run(path, register).await
}

pub async fn run_health(
    detail: bool,
    limit: usize,
    page: usize,
    cache_ttl: i64,
) -> anyhow::Result<()> {
    info!("{}", i18n::current().cli.health_check);
    health::run(detail, limit, page, cache_ttl).await
}

pub async fn run_query(
    query: &str,
    limit: usize,
    page: usize,
    config: &config::Config,
) -> anyhow::Result<()> {
    info!("{}: {}", i18n::current().cli.querying, query);
    query::run(query, limit, page, config).await
}

pub async fn run_index(path: &str) -> anyhow::Result<()> {
    info!("{}: path='{}'", i18n::current().cli.indexing, path);
    let path = path.to_string();
    let count = tokio::task::spawn_blocking(move || knowledge_engine::run_index(&path))
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
    info!("已索引 {} 个仓库", count);
    Ok(())
}

#[cfg(feature = "tui")]
pub async fn run_tui() -> anyhow::Result<()> {
    info!("{}", i18n::current().cli.launching_tui);
    tui::run().await
}

pub async fn run_mcp(tools: Option<String>) -> anyhow::Result<()> {
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

pub async fn run_daemon(interval: Option<u64>, config: config::Config) -> anyhow::Result<()> {
    let interval = interval.unwrap_or(config.daemon.interval_seconds);
    let d = daemon::Daemon::new(interval, config);
    d.run().await
}

pub async fn run_vault(cmd: crate::VaultCommands) -> anyhow::Result<()> {
    match cmd {
        crate::VaultCommands::Scan { path } => {
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
        crate::VaultCommands::Reindex => {
            tokio::task::spawn_blocking(vault::indexer::reindex_vault)
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            println!("Vault search index rebuilt.");
        }
    }
    Ok(())
}

pub fn run_clean() -> anyhow::Result<()> {
    info!("正在清理注册表中的备份条目");
    let conn = crate::registry::WorkspaceRegistry::init_db()?;
    let deleted = conn
        .execute("DELETE FROM repos WHERE id LIKE 'Clarity_%' OR id LIKE 'clarity_backup%'", [])?;
    println!("已从 devbase 注册表中删除 {} 个备份条目。", deleted);
    println!("\n剩余已注册仓库:");
    let mut stmt = conn.prepare("SELECT id, local_path FROM repos")?;
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, path) = row?;
        println!("  [{}] {}", id, path);
    }
    Ok(())
}

pub fn run_tag(repo_id: &str, tags: &str) -> anyhow::Result<()> {
    info!("为 {} 打标签: {}", repo_id, tags);
    let mut conn = registry::WorkspaceRegistry::init_db()?;
    let tag_list: Vec<&str> = tags.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
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
    Ok(())
}

pub fn run_meta(
    repo_id: &str,
    tier: Option<String>,
    workspace_type: Option<String>,
) -> anyhow::Result<()> {
    info!("更新 {} 的元数据", repo_id);
    let conn = registry::WorkspaceRegistry::init_db()?;
    let exists: bool = conn
        .query_row("SELECT 1 FROM repos WHERE id = ?1", [&repo_id], |_| Ok(true))
        .unwrap_or(false);
    if !exists {
        println!("注册表中未找到仓库 '{}'。", repo_id);
    } else {
        if let Some(ref t) = tier {
            registry::WorkspaceRegistry::update_repo_tier(&conn, repo_id, t)?;
            println!("已将 '{}' 的数据分级设为 '{}'。", repo_id, t);
        }
        if let Some(ref wt) = workspace_type {
            registry::WorkspaceRegistry::update_repo_workspace_type(&conn, repo_id, wt)?;
            println!("已将 '{}' 的工作区类型设为 '{}'。", repo_id, wt);
        }
        if tier.is_none() && workspace_type.is_none() {
            println!("未提供任何要更新的字段。使用 --tier 或 --workspace-type 指定。");
        }
    }
    Ok(())
}

pub async fn run_watch(path: &str, duration: u64, config: &config::Config) -> anyhow::Result<()> {
    use std::time::Duration;
    use watch::{FolderScheduler, FsWatcher, WatchAggregator};

    let root = std::path::PathBuf::from(path);
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
                println!("Detected changes, actions: {:?}", actions);
            }
        }
    }

    println!("Watch completed for {}", path);
    Ok(())
}

pub fn run_skill_sync(output: &str, filter_tags: Option<String>, dry_run: bool) {
    let filter_tags: Vec<String> = filter_tags
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
        .unwrap_or_default();
    match skill_sync::run_sync(output, &filter_tags, dry_run) {
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

pub async fn run_digest(digest_config: config::DigestConfig) -> anyhow::Result<()> {
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
        Ok(Ok(text)) => {
            println!("{}", text);
            Ok(())
        }
        Ok(Err(e)) => {
            println!("{}: {}", i18n::current().log.digest_failed, e);
            Ok(())
        }
        Err(e) => {
            println!("{}: {}", i18n::current().log.digest_panic, e);
            Ok(())
        }
    }
}

pub fn run_oplog(limit: i64, repo: Option<String>) -> anyhow::Result<()> {
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
            let duration_display =
                entry.duration_ms.map(|d| format!(" | duration={}ms", d)).unwrap_or_default();
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
    Ok(())
}

pub fn run_discover() -> anyhow::Result<()> {
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
        WorkspaceRegistry::save_relation(&conn, &d.from, &d.to, &d.relation_type, d.confidence)?;
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
    Ok(())
}

pub async fn run_sync(
    dry_run: bool,
    filter_tags: Option<String>,
    exclude: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    if dry_run {
        warn!("Dry-run mode enabled");
    }
    info!("{}", i18n::current().cli.syncing);
    if json {
        let output = sync::run_json(dry_run, filter_tags.as_deref(), exclude.as_deref()).await?;
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        sync::run(dry_run, filter_tags.as_deref(), exclude.as_deref()).await?;
    }
    Ok(())
}

pub async fn run_syncthing_push(
    api_url: String,
    api_key: Option<String>,
    filter_tags: Option<String>,
    experiment: Option<String>,
) -> anyhow::Result<()> {
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
        let target_repo = exps.into_iter().find(|e| e.id == *exp_id).and_then(|e| e.repo_id);
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
    Ok(())
}

pub fn run_registry(cmd: crate::RegistryCommands) -> anyhow::Result<()> {
    match cmd {
        crate::RegistryCommands::Export { format, output } => {
            let out_path = output.as_deref().map(std::path::Path::new);
            backup::run_export(&format, out_path)?;
        }
        crate::RegistryCommands::Import { path, yes } => {
            let source = std::path::Path::new(&path);
            backup::run_import(source, yes)?;
        }
        crate::RegistryCommands::Backups => {
            backup::run_list()?;
        }
        crate::RegistryCommands::Clean => {
            backup::run_clean()?;
        }
    }
    Ok(())
}
