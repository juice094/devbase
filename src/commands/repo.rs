use anyhow::Context;
use devbase::*;
use devbase::mcp::clients::RegistryClient;
use rusqlite::OptionalExtension;
use tracing::{info, warn};

pub async fn run_scan(
    ctx: &mut crate::storage::AppContext,
    path: &str,
    register: bool,
) -> anyhow::Result<()> {
    info!("{}: {}", ctx.i18n.cli.scanning, path);
    let pool = ctx.pool();
    scan::run(path, register, &pool).await
}

pub async fn run_health(
    ctx: &mut crate::storage::AppContext,
    detail: bool,
    limit: usize,
    page: usize,
) -> anyhow::Result<()> {
    info!("{}", ctx.i18n.cli.health_check);
    let conn = ctx.conn()?;
    health::run(&conn, detail, limit, page, ctx.config.cache.ttl_seconds, &ctx.i18n).await
}

pub async fn run_query(
    ctx: &mut crate::storage::AppContext,
    query: &str,
    limit: usize,
    page: usize,
) -> anyhow::Result<()> {
    info!("{}: {}", ctx.i18n.cli.querying, query);
    let conn = ctx.conn()?;
    query::run(&conn, query, limit, page, &ctx.config).await
}

pub async fn run_index(ctx: &mut crate::storage::AppContext, path: &str) -> anyhow::Result<()> {
    info!("{}: path='{}'", ctx.i18n.cli.indexing, path);
    let path = path.to_string();
    let pool = ctx.pool();
    let count = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        knowledge_engine::run_index(&mut conn, &path)
    })
    .await
    .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
    info!("已索引 {} 个仓库", count);
    Ok(())
}

#[cfg(feature = "tui")]
pub fn run_discover(ctx: &mut crate::storage::AppContext) -> anyhow::Result<()> {
    use discovery_engine::{Discovery, discover_dependencies, discover_similar_projects};
    use std::collections::HashMap;

    let conn = ctx.conn_mut()?;
    let repos = crate::registry::repo::list_repos(&conn)?;

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
        crate::registry::relation::save_relation(
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
    Ok(())
}

pub async fn run_sync(
    ctx: &mut crate::storage::AppContext,
    dry_run: bool,
    filter_tags: Option<String>,
    exclude: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    if dry_run {
        warn!("Dry-run mode enabled");
    }
    info!("{}", ctx.i18n.cli.syncing);
    let conn = ctx.conn()?;
    if json {
        let output =
            sync::run_json(&conn, dry_run, filter_tags.as_deref(), exclude.as_deref(), &ctx.i18n)
                .await?;
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        sync::run(&conn, dry_run, filter_tags.as_deref(), exclude.as_deref(), &ctx.i18n).await?;
    }
    Ok(())
}

pub async fn run_syncthing_push(
    ctx: &mut crate::storage::AppContext,
    api_url: String,
    api_key: Option<String>,
    filter_tags: Option<String>,
    experiment: Option<String>,
) -> anyhow::Result<()> {
    use registry::WorkspaceRegistry;
    use syncthing_client::SyncthingClient;

    let client = SyncthingClient::new(&api_url, api_key.as_deref());

    let conn = ctx.conn_mut()?;

    let repos = match crate::registry::repo::list_repos(&conn) {
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

pub fn run_registry(
    ctx: &mut crate::storage::AppContext,
    cmd: crate::RegistryCommands,
) -> anyhow::Result<()> {
    match cmd {
        crate::RegistryCommands::Export { format, output } => {
            let conn = ctx.conn()?;
            let out_path = output.as_deref().map(std::path::Path::new);
            backup::run_export(&conn, &format, out_path)?;
        }
        crate::RegistryCommands::Import { path, yes } => {
            let mut conn = ctx.conn_mut()?;
            let source = std::path::Path::new(&path);
            backup::run_import(&mut conn, source, yes)?;
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

