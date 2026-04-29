use crate::registry::{OplogEntry, WorkspaceRegistry};
use chrono::Utc;
use std::collections::HashMap;
use tokio::time::Duration;
use tracing::info;

mod orchestrator;
mod policy;
mod tasks;

pub use orchestrator::SyncOrchestrator;
pub use policy::{
    RepoSyncTask, SyncMode, SyncPolicy, SyncSafety, assess_safety, recommend_sync_action,
};

use tasks::{collect_tasks, map_action, print_summary_table};

#[derive(Debug, Default, Clone)]
pub struct SyncSummary {
    pub action: String,
    pub ahead: usize,
    pub behind: usize,
    pub message: String,
    pub error_kind: Option<String>,
}

pub async fn run_json(
    conn: &rusqlite::Connection,
    dry_run: bool,
    filter_tags: Option<&str>,
    exclude: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let start = std::time::Instant::now();
    let config = crate::config::Config::load().unwrap_or_default();
    let tasks = collect_tasks(conn, filter_tags, exclude, &config.scan.exclude_paths).await?;
    let mut path_map = HashMap::new();
    for task in &tasks {
        path_map.insert(task.id.clone(), task.path.clone());
    }

    let orchestrator = SyncOrchestrator::new(config.sync.concurrency.max(1));
    let timeout = Duration::from_secs(config.sync.timeout_seconds);
    let summaries = orchestrator
        .run_sync(tasks, SyncMode::Sync, dry_run, timeout, |_id, _summary| {})
        .await;

    let results_json: Vec<serde_json::Value> = summaries
        .into_iter()
        .map(|(id, summary)| {
            let path = path_map.get(&id).cloned().unwrap_or_default();
            serde_json::json!({
                "id": id,
                "path": path,
                "action": map_action(&summary.action, &summary.message),
                "ahead": summary.ahead,
                "behind": summary.behind,
                "message": summary.message,
                "error_kind": summary.error_kind
            })
        })
        .collect();

    info!("{}", crate::i18n::current().log.sync_finished);

    // Log to oplog
    let duration_ms = start.elapsed().as_millis() as i64;
    let repo_count = results_json.len();
    let details = serde_json::json!({
        "dry_run": dry_run,
        "repo_count": repo_count
    });
    let _ = WorkspaceRegistry::save_oplog(
        conn,
        &OplogEntry {
            id: None,
            event_type: crate::registry::OplogEventType::Sync,
            repo_id: None,
            details: Some(details.to_string()),
            status: "success".to_string(),
            timestamp: Utc::now(),
            duration_ms: Some(duration_ms),
            event_version: 1,
        },
    );

    Ok(serde_json::json!({
        "success": true,
        "dry_run": dry_run,
        "results": results_json
    }))
}

pub async fn run(
    conn: &rusqlite::Connection,
    dry_run: bool,
    filter_tags: Option<&str>,
    exclude: Option<&str>,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    let config = crate::config::Config::load().unwrap_or_default();
    let tasks = collect_tasks(conn, filter_tags, exclude, &config.scan.exclude_paths).await?;
    let mut path_map = HashMap::new();
    for task in &tasks {
        path_map.insert(task.id.clone(), task.path.clone());
    }

    let orchestrator = SyncOrchestrator::new(config.sync.concurrency.max(1));
    let timeout = Duration::from_secs(config.sync.timeout_seconds);
    let results = orchestrator
        .run_sync(tasks, SyncMode::Async, dry_run, timeout, |id, summary| {
            println!("  [{}] {}: {}", id, crate::i18n::current().log.progress, summary.message);
        })
        .await;

    let filter_suffix = filter_tags
        .map(|f| format!("{}{}）", crate::i18n::current().sync.filter_prefix, f))
        .unwrap_or_default();
    println!(
        "{}: policy-per-repo{}\n",
        crate::i18n::current().sync.strategy_prefix,
        filter_suffix
    );

    let results_json: Vec<serde_json::Value> = results
        .iter()
        .map(|(id, summary)| {
            serde_json::json!({
                "id": id,
                "path": path_map.get(id).cloned().unwrap_or_default(),
                "action": map_action(&summary.action, &summary.message),
                "ahead": summary.ahead,
                "behind": summary.behind,
                "message": summary.message,
                "error_kind": summary.error_kind
            })
        })
        .collect();

    for item in &results_json {
        let id = item["id"].as_str().unwrap_or("");
        let path = item["path"].as_str().unwrap_or("");
        let action = item["action"].as_str().unwrap_or("");
        let message = item["message"].as_str().unwrap_or("");

        if action == "skipped" && message == crate::i18n::current().sync.skip_no_upstream {
            println!("  [{}] {}", id, crate::i18n::current().sync.skip_no_upstream);
        } else if action == "skipped" && dry_run {
            println!("  [{}] {}", id, message);
        } else {
            println!("  [{}] {} {}...", id, crate::i18n::current().sync.checking, path);
            if action == "error" || action == "timeout" {
                println!("    [{}] {}", crate::i18n::current().sync.error_prefix, message);
            } else if action == "fetch_only" {
                println!("    -> {}", crate::i18n::current().sync.fetched_only);
            } else if action == "blocked_dirty" {
                println!("    {}", crate::i18n::current().sync.blocked_dirty);
            } else if action == "merged_ff" {
                println!("    {}", crate::i18n::current().sync.merged_ff);
            } else if action == "merged_commit" {
                println!("    {}", crate::i18n::current().sync.merged_commit);
            } else if action == "conflict" {
                println!("    {}", crate::i18n::current().sync.conflict);
            }
        }
    }

    print_summary_table(&results_json);

    if dry_run {
        println!("\n{}", crate::i18n::current().sync.dry_run_complete);
    } else {
        println!("\n{}", crate::i18n::current().sync.sync_complete);
    }

    // Log to oplog
    let duration_ms = start.elapsed().as_millis() as i64;
    let repo_count = results_json.len();
    let details = serde_json::json!({
        "dry_run": dry_run,
        "repo_count": repo_count
    });
    let _ = WorkspaceRegistry::save_oplog(
        conn,
        &OplogEntry {
            id: None,
            event_type: crate::registry::OplogEventType::Sync,
            repo_id: None,
            details: Some(details.to_string()),
            status: "success".to_string(),
            timestamp: Utc::now(),
            duration_ms: Some(duration_ms),
            event_version: 1,
        },
    );

    Ok(())
}

#[cfg(test)]
mod tests;
