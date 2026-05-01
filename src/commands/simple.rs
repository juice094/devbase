use devbase::*;
use devbase::mcp::clients::RegistryClient;
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

pub async fn run_vault(
    ctx: &mut crate::storage::AppContext,
    cmd: crate::VaultCommands,
) -> anyhow::Result<()> {
    match cmd {
        crate::VaultCommands::Scan { path } => {
            let dir = if path.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(path))
            };
            let pool = ctx.pool();
            let count = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get()?;
                vault::scanner::scan_vault(&mut conn, dir.as_deref())
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            println!("Synced {} vault notes.", count);
        }
        crate::VaultCommands::Reindex => {
            let pool = ctx.pool();
            tokio::task::spawn_blocking(move || {
                let conn = pool.get()?;
                vault::indexer::reindex_vault(&conn)
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            println!("Vault search index rebuilt.");
        }
        crate::VaultCommands::List { tag } => {
            let pool = ctx.pool();
            let notes = tokio::task::spawn_blocking(move || {
                let conn = pool.get()?;
                crate::registry::vault::list_vault_notes(&conn)
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            let filtered: Vec<_> = notes
                .into_iter()
                .filter(|n| {
                    tag.as_ref()
                        .map(|t| n.tags.iter().any(|nt| nt.eq_ignore_ascii_case(t)))
                        .unwrap_or(true)
                })
                .collect();
            if filtered.is_empty() {
                println!("No vault notes found.");
            } else {
                println!("{:<40} {:<20} {}", "PATH", "TITLE", "TAGS");
                for note in filtered {
                    let title = note.title.as_deref().unwrap_or("(no title)");
                    let tags = if note.tags.is_empty() {
                        "-".to_string()
                    } else {
                        note.tags.join(", ")
                    };
                    println!("{:<40} {:<20} {}", note.id, title, tags);
                }
            }
        }
        crate::VaultCommands::Read { path } => {
            let pool = ctx.pool();
            let note = tokio::task::spawn_blocking({
                let path = path.clone();
                move || {
                    let conn = pool.get()?;
                    crate::registry::vault::get_vault_note(&conn, &path)
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            match note {
                Some(n) => {
                    if let Some(content) = crate::vault::fs_io::read_note_content(&n.path) {
                        println!("{}", content);
                    } else {
                        anyhow::bail!("Failed to read note file: {}", n.path);
                    }
                }
                None => anyhow::bail!("Vault note not found: {}", path),
            }
        }
        crate::VaultCommands::Write { path, content, title } => {
            let vault_root = crate::registry::WorkspaceRegistry::workspace_dir()?.join("vault");
            let target = vault_root.join(&path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let body = match content {
                Some(c) if c == "-" => {
                    let mut stdin = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin)?;
                    stdin
                }
                Some(c) => c,
                None => String::new(),
            };
            let frontmatter_title = title.unwrap_or_else(|| {
                target
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string())
            });
            let full = if body.starts_with("---") {
                body
            } else {
                format!("---\ntitle: {}\n---\n\n{}", frontmatter_title, body)
            };
            std::fs::write(&target, full)?;
            let pool = ctx.pool();
            tokio::task::spawn_blocking(move || {
                let mut conn = pool.get()?;
                vault::scanner::scan_vault(&mut conn, Some(&vault_root))
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking failed: {}", e))??;
            println!("Wrote vault note: {}", path);
        }
        crate::VaultCommands::Search { query, limit } => {
            let results = crate::search::search_vault(&query, limit)
                .map_err(|e| anyhow::anyhow!("Vault search failed: {}", e))?;
            if results.is_empty() {
                println!("No vault notes found for '{}'.", query);
            } else {
                println!("Found {} note(s):", results.len());
                for (id, score) in results {
                    println!("  [{}] score={:.3}", id, score);
                }
            }
        }
    }
    Ok(())
}

pub fn run_clean(ctx: &mut crate::storage::AppContext) -> anyhow::Result<()> {
    info!("正在清理注册表中的备份条目");
    let conn = ctx.conn_mut()?;
    // Entities is the single source of truth.
    let deleted = conn.execute(
        &format!("DELETE FROM entities WHERE entity_type = '{}' AND (id LIKE 'Clarity_%' OR id LIKE 'clarity_backup%')", crate::registry::ENTITY_TYPE_REPO),
        [],
    )?;
    println!("已从 devbase 注册表中删除 {} 个备份条目。", deleted);
    println!("\n剩余已注册仓库:");
    let mut stmt = conn.prepare(&format!(
        "SELECT id, local_path FROM entities WHERE entity_type = '{}'",
        crate::registry::ENTITY_TYPE_REPO
    ))?;
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, path) = row?;
        println!("  [{}] {}", id, path);
    }
    Ok(())
}

pub fn run_tag(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    tags: &str,
) -> anyhow::Result<()> {
    info!("为 {} 打标签: {}", repo_id, tags);
    let mut conn = ctx.conn_mut()?;
    let tag_list: Vec<&str> = tags.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let tx = conn.transaction()?;
    let exists: bool = tx
        .query_row(
            &format!(
                "SELECT 1 FROM entities WHERE id = ?1 AND entity_type = '{}'",
                crate::registry::ENTITY_TYPE_REPO
            ),
            [&repo_id],
            |_| Ok(true),
        )
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
        crate::registry::repo::sync_repo_tags_to_entity(&tx, repo_id)?;
        tx.commit()?;
        println!("已为 '{}' 打上标签 '{}'。", repo_id, tags);
    }
    Ok(())
}

pub fn run_meta(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    tier: Option<String>,
    workspace_type: Option<String>,
) -> anyhow::Result<()> {
    info!("更新 {} 的元数据", repo_id);
    let conn = ctx.conn_mut()?;
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM entities WHERE id = ?1 AND entity_type = 'repo'",
            [&repo_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !exists {
        println!("注册表中未找到仓库 '{}'。", repo_id);
    } else {
        if let Some(ref t) = tier {
            crate::registry::repo::update_repo_tier(&conn, repo_id, t)?;
            println!("已将 '{}' 的数据分级设为 '{}'。", repo_id, t);
        }
        if let Some(ref wt) = workspace_type {
            crate::registry::repo::update_repo_workspace_type(&conn, repo_id, wt)?;
            println!("已将 '{}' 的工作区类型设为 '{}'。", repo_id, wt);
        }
        if tier.is_none() && workspace_type.is_none() {
            println!("未提供任何要更新的字段。使用 --tier 或 --workspace-type 指定。");
        }
    }
    Ok(())
}

#[cfg(feature = "watch")]
pub async fn run_watch(
    ctx: &mut crate::storage::AppContext,
    path: &str,
    duration: u64,
) -> anyhow::Result<()> {
    use std::time::Duration;
    use watch::{FolderScheduler, FsWatcher, WatchAggregator};

    let root = std::path::PathBuf::from(path);
    let watcher = FsWatcher::new(&root)?;
    let aggregator = WatchAggregator {
        max_files: ctx.config.watch.max_files,
        ..Default::default()
    };
    let mut scheduler = FolderScheduler::with_max_files(root.clone(), ctx.config.watch.max_files);

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

pub fn run_skill_sync(
    _ctx: &mut crate::storage::AppContext,
    output: &str,
    filter_tags: Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
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
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Skill sync failed: {}", e)),
    }
}

pub async fn run_digest(ctx: &mut crate::storage::AppContext) -> anyhow::Result<()> {
    let digest_config = ctx.config.digest.clone();
    let pool = ctx.pool();
    let i18n = ctx.i18n;
    match tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        let cfg = config::Config {
            general: config::GeneralConfig::default(),
            digest: digest_config,
            ..Default::default()
        };
        digest::generate_daily_digest(&conn, &cfg, &i18n)
    })
    .await
    {
        Ok(Ok(text)) => {
            println!("{}", text);
            Ok(())
        }
        Ok(Err(e)) => {
            println!("{}: {}", ctx.i18n.log.digest_failed, e);
            Ok(())
        }
        Err(e) => {
            println!("{}: {}", ctx.i18n.log.digest_panic, e);
            Ok(())
        }
    }
}

pub fn run_oplog(
    ctx: &mut crate::storage::AppContext,
    limit: i64,
    repo: Option<String>,
) -> anyhow::Result<()> {
    let conn = ctx.conn_mut()?;
    let entries = match repo {
        Some(ref r) => crate::registry::workspace::list_oplog_by_repo(&conn, r, limit)?,
        None => crate::registry::workspace::list_oplog(&conn, limit)?,
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

pub fn run_metrics(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    if repo_id.is_empty() {
        let val = ctx.list_code_metrics()?;
        let repos = val.get("repos").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if json {
            let output: Vec<serde_json::Value> = repos
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "repo_id": r.get("repo_id").cloned().unwrap_or(serde_json::Value::Null),
                        "total_lines": r.get("total_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "source_lines": r.get("source_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "test_lines": r.get("test_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "comment_lines": r.get("comment_lines").cloned().unwrap_or(serde_json::Value::Null),
                        "file_count": r.get("file_count").cloned().unwrap_or(serde_json::Value::Null),
                        "language_breakdown": r.get("language_breakdown").cloned().unwrap_or(serde_json::Value::Null),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("Code metrics for {} repo(s):", repos.len());
            for r in repos {
                let id = r.get("repo_id").and_then(|v| v.as_str()).unwrap_or("");
                let total = r.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let source = r.get("source_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let test = r.get("test_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let comment = r.get("comment_lines").and_then(|v| v.as_u64()).unwrap_or(0);
                let files = r.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0);
                println!(
                    "  [{}] total={} source={} test={} comment={} files={}",
                    id, total, source, test, comment, files
                );
            }
        }
    } else {
        let val = ctx.get_code_metrics(repo_id)?;
        let success = val.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if !success {
            println!("No metrics found for '{}'.", repo_id);
        } else if json {
            let output = serde_json::json!({
                "repo_id": repo_id,
                "total_lines": val.get("total_lines").cloned().unwrap_or(serde_json::Value::Null),
                "source_lines": val.get("source_lines").cloned().unwrap_or(serde_json::Value::Null),
                "test_lines": val.get("test_lines").cloned().unwrap_or(serde_json::Value::Null),
                "comment_lines": val.get("comment_lines").cloned().unwrap_or(serde_json::Value::Null),
                "file_count": val.get("file_count").cloned().unwrap_or(serde_json::Value::Null),
                "language_breakdown": val.get("language_breakdown").cloned().unwrap_or(serde_json::Value::Null),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            let total = val.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let source = val.get("source_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let test = val.get("test_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let comment = val.get("comment_lines").and_then(|v| v.as_u64()).unwrap_or(0);
            let files = val.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0);
            println!(
                "[{}] total={} source={} test={} comment={} files={}",
                repo_id, total, source, test, comment, files
            );
        }
    }
    Ok(())
}

pub fn run_module_graph(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    if repo_id.is_empty() {
        let repos_val = ctx.list_repos(None)?;
        let repos = repos_val.get("repos").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mut all = Vec::new();
        for repo in repos {
            let id = repo.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let language = repo.get("language").and_then(|v| v.as_str()).unwrap_or("");
            if language == "Rust" {
                let mod_val = ctx.list_modules(id)?;
                let modules = mod_val.get("modules").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                if !modules.is_empty() {
                    all.push((id.to_string(), modules));
                }
            }
        }
        if json {
            let out: Vec<serde_json::Value> = all
                .into_iter()
                .map(|(id, mods)| {
                    serde_json::json!({
                        "repo_id": id,
                        "modules": mods
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            println!("Module graph for {} Rust repo(s):", all.len());
            for (id, mods) in all {
                println!("  [{}] {} module(s)", id, mods.len());
                for m in mods {
                    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let ty = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    println!("    {} ({})  {}", name, ty, path);
                }
            }
        }
    } else {
        let mod_val = ctx.list_modules(repo_id)?;
        let modules = mod_val.get("modules").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if json {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": repo_id,
                "modules": modules
            }))?);
        } else {
            println!("Module graph for [{}]:", repo_id);
            for m in modules {
                let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let ty = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("");
                println!("  {} ({})  {}", name, ty, path);
            }
        }
    }
    Ok(())
}

pub fn run_call_graph(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    callee: Option<String>,
    caller: Option<String>,
    file: Option<String>,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let callee_s = callee.as_deref().unwrap_or("");
    let caller_s = caller.as_deref().unwrap_or("");
    if callee_s.is_empty() && caller_s.is_empty() {
        anyhow::bail!("At least one of --callee or --caller must be provided");
    }
    let val = ctx.query_call_graph(
        repo_id,
        Some(callee_s).filter(|s| !s.is_empty()),
        Some(caller_s).filter(|s| !s.is_empty()),
        file.as_deref().filter(|s| !s.is_empty()),
        limit,
    )?;
    let edges = val.get("calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "count": edges.len(),
            "calls": edges
        }))?);
    } else {
        println!("Call graph for [{}]: {} edge(s)", repo_id, edges.len());
        for e in edges {
            let caller_file = e.get("caller_file").and_then(|v| v.as_str()).unwrap_or("");
            let caller_symbol = e.get("caller_symbol").and_then(|v| v.as_str()).unwrap_or("");
            let caller_line = e.get("caller_line").and_then(|v| v.as_i64()).unwrap_or(0);
            let callee_name = e.get("callee_name").and_then(|v| v.as_str()).unwrap_or("");
            println!(
                "  {}:{}  {} -> {}",
                caller_file, caller_line, caller_symbol, callee_name
            );
        }
    }
    Ok(())
}

pub fn run_dependency_graph(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    direction: &str,
    relation_type: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let rel_filter = relation_type.as_deref().filter(|s| !s.is_empty());
    let val = ctx.query_dependencies(repo_id, direction, rel_filter)?;
    let label = val.get("label").and_then(|v| v.as_str()).unwrap_or("dependencies");
    let deps = val.get("dependencies").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "direction": direction,
            "count": deps.len(),
            "dependencies": deps
        }))?);
    } else {
        println!("{} for [{}]: {} edge(s)", label, repo_id, deps.len());
        for d in deps {
            let id = d.get("repo_id").and_then(|v| v.as_str()).unwrap_or("");
            let rel = d.get("relation_type").and_then(|v| v.as_str()).unwrap_or("");
            let conf = d.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!("  -> {} ({} conf={:.2})", id, rel, conf);
        }
    }
    Ok(())
}

pub fn run_code_symbols(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    name: Option<String>,
    symbol_type: Option<String>,
    file: Option<String>,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let val = ctx.query_code_symbols(
        repo_id,
        name.as_deref().filter(|s| !s.is_empty()),
        symbol_type.as_deref().filter(|s| !s.is_empty()),
        file.as_deref().filter(|s| !s.is_empty()),
        limit,
    )?;
    let symbols = val.get("symbols").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "count": symbols.len(),
            "symbols": symbols
        }))?);
    } else {
        println!("Code symbols for [{}]: {} result(s)", repo_id, symbols.len());
        for s in symbols {
            let fp = s.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let st = s.get("symbol_type").and_then(|v| v.as_str()).unwrap_or("");
            let n = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ls = s.get("line_start").and_then(|v| v.as_i64()).unwrap_or(0);
            let sig = s.get("signature").and_then(|v| v.as_str());
            let sig_str = sig.map(|s| format!("  {}", s)).unwrap_or_default();
            println!("  {}:{} {} {} {}", fp, ls, st, n, sig_str);
        }
    }
    Ok(())
}

pub fn run_dead_code(
    ctx: &mut crate::storage::AppContext,
    repo_id: &str,
    include_pub: bool,
    limit: usize,
    json: bool,
) -> anyhow::Result<()> {
    let val = ctx.query_dead_code(repo_id, include_pub, limit)?;
    let dead = val.get("dead_functions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "repo_id": repo_id,
            "count": dead.len(),
            "dead_functions": dead
        }))?);
    } else {
        println!("Potentially dead functions in [{}]: {}", repo_id, dead.len());
        for d in dead {
            let fp = d.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let n = d.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let line = d.get("line_start").and_then(|v| v.as_i64()).unwrap_or(0);
            let sig = d.get("signature").and_then(|v| v.as_str());
            let sig_str = sig.map(|s| format!("  {}", s)).unwrap_or_default();
            println!("  {}:{} {}{}", fp, line, n, sig_str);
        }
    }
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{AppContext, StorageBackend};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TempStorage {
        dir: tempfile::TempDir,
    }

    impl TempStorage {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
            }
        }
    }

    impl StorageBackend for TempStorage {
        fn db_path(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("db"))
        }
        fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
            let ws = self.dir.path().join("ws");
            std::fs::create_dir_all(&ws)?;
            Ok(ws)
        }
        fn index_path(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("idx"))
        }
        fn backup_dir(&self) -> anyhow::Result<PathBuf> {
            Ok(self.dir.path().join("bk"))
        }
    }

    #[tokio::test]
    async fn test_run_vault_list_empty() {
        let storage = Arc::new(TempStorage::new());
        let mut ctx = AppContext::with_storage(storage).unwrap();
        let result = run_vault(&mut ctx, crate::VaultCommands::List { tag: None }).await;
        assert!(result.is_ok());
    }
}
