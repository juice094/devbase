use devbase::*;
use tracing::info;

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
                println!("{:<40} {:<20} TAGS", "PATH", "TITLE");
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

