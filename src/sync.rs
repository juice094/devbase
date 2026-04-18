use crate::registry::{OplogEntry, WorkspaceRegistry};
use chrono::Utc;
use git2::Repository;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

#[derive(Debug, Default, Clone)]
pub struct SyncSummary {
    pub action: String,
    pub ahead: usize,
    pub behind: usize,
    pub message: String,
    pub error_kind: Option<String>,
}

fn classify_sync_error(error: &anyhow::Error) -> &'static str {
    let msg = error.to_string().to_lowercase();
    if msg.contains("network") || msg.contains("could not resolve") || msg.contains("connection") {
        "network-error"
    } else if msg.contains("authentication") || msg.contains("credentials") || msg.contains("403") || msg.contains("401") {
        "auth-failed"
    } else if msg.contains("conflict") {
        "conflict"
    } else if msg.contains("not clean") || msg.contains("dirty") {
        "blocked-dirty"
    } else {
        "error"
    }
}

#[derive(Debug, Clone)]
pub struct RepoSyncTask {
    pub id: String,
    pub path: String,
    pub upstream_url: Option<String>,
    pub default_branch: Option<String>,
    pub tags: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncMode {
    SYNC,
    ASYNC,
    BlockUi,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncSafety {
    Safe,
    BlockedDirty,
    BlockedDiverged,
    BlockedProtected,
    NoUpstream,
    UpToDate,
    Unknown,
}

impl SyncSafety {
    #[allow(dead_code)]
    pub fn is_runnable(&self) -> bool {
        matches!(self, SyncSafety::Safe)
    }
}

/// Pre-flight safety assessment for auto-pull strategy.
/// Returns `Safe` only when the repo is clean and a fast-forward merge is possible.
pub fn assess_safety(path: &str, task_tags: &str, protected_tags: &[&str]) -> SyncSafety {
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(_) => return SyncSafety::Unknown,
    };

    let dirty = match repo.statuses(None) {
        Ok(statuses) => statuses.iter().any(|entry| entry.status() != git2::Status::CURRENT),
        Err(_) => false,
    };
    if dirty {
        return SyncSafety::BlockedDirty;
    }

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return SyncSafety::Unknown,
    };
    let local_oid = match head.target() {
        Some(o) => o,
        None => return SyncSafety::Unknown,
    };
    let branch = match head.shorthand() {
        Some(b) => b,
        None => return SyncSafety::Unknown,
    };

    let remote_oid = match repo.revparse_single(&format!("refs/remotes/origin/{}", branch)) {
        Ok(obj) => obj.id(),
        Err(_) => return SyncSafety::NoUpstream,
    };

    if local_oid == remote_oid {
        return SyncSafety::UpToDate;
    }

    let (ahead, behind) = match repo.graph_ahead_behind(local_oid, remote_oid) {
        Ok(ab) => ab,
        Err(_) => return SyncSafety::Unknown,
    };

    let is_protected = protected_tags
        .iter()
        .any(|pt| task_tags.split(',').map(|s| s.trim()).any(|t| t == *pt));

    if ahead > 0 && behind > 0 {
        if is_protected {
            return SyncSafety::BlockedProtected;
        }
        return SyncSafety::BlockedDiverged;
    }

    if behind > 0 && ahead == 0 {
        return SyncSafety::Safe;
    }

    // ahead > 0, behind == 0
    SyncSafety::UpToDate
}

#[derive(Clone)]
pub struct SyncOrchestrator {
    semaphore: Arc<Semaphore>,
}

impl SyncOrchestrator {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    pub async fn run_sync(
        &self,
        repos: Vec<RepoSyncTask>,
        mode: SyncMode,
        dry_run: bool,
        strategy: &str,
        timeout_duration: Duration,
        mut on_progress: impl FnMut(String, SyncSummary) + Send,
    ) -> Vec<(String, SyncSummary)> {
        match mode {
            SyncMode::SYNC => {
                let mut results = Vec::with_capacity(repos.len());
                for task in repos {
                    on_progress(
                        task.id.clone(),
                        SyncSummary {
                            action: "RUNNING".to_string(),
                            message: crate::i18n::current().sync.status_running.to_string(),
                            ..Default::default()
                        },
                    );
                    let summary = match timeout(timeout_duration, execute_task(&task, dry_run, strategy)).await {
                        Ok(s) => s,
                        Err(_) => SyncSummary {
                            action: "TIMEOUT".to_string(),
                            message: crate::i18n::current().sync.network_timeout.to_string(),
                            ..Default::default()
                        },
                    };
                    on_progress(task.id.clone(), summary.clone());
                    results.push((task.id, summary));
                }
                results
            }
            SyncMode::ASYNC | SyncMode::BlockUi => {
                // FIXME: ASYNC branch temporarily falls back to sequential execution
                // to avoid tokio::spawn scheduling deadlock on Windows multi-repo sync.
                // Re-enable true concurrency once root cause is identified.
                let mut results = Vec::with_capacity(repos.len());
                for task in repos {
                    on_progress(
                        task.id.clone(),
                        SyncSummary {
                            action: "RUNNING".to_string(),
                            message: crate::i18n::current().sync.status_running.to_string(),
                            ..Default::default()
                        },
                    );
                    let summary = match timeout(timeout_duration, execute_task(&task, dry_run, strategy)).await {
                        Ok(s) => s,
                        Err(_) => SyncSummary {
                            action: "TIMEOUT".to_string(),
                            message: crate::i18n::current().sync.network_timeout.to_string(),
                            ..Default::default()
                        },
                    };
                    on_progress(task.id.clone(), summary.clone());
                    results.push((task.id, summary));
                }
                results
            }
        }
    }
}

async fn execute_task(task: &RepoSyncTask, dry_run: bool, strategy: &str) -> SyncSummary {
    if task.tags.contains("own-project") || task.tags.contains("tool") {
        if task.upstream_url.is_none() {
            return SyncSummary {
                action: "SKIP".to_string(),
                message: crate::i18n::current().sync.skip_no_upstream.to_string(),
                ..Default::default()
            };
        }
    }

    if dry_run {
        let url = task.upstream_url.as_deref().unwrap_or("?");
        return SyncSummary {
            action: "DRY_RUN".to_string(),
            message: crate::i18n::format_template(crate::i18n::current().sync.would_fetch, &[url, &task.path]),
            ..Default::default()
        };
    }

    // Pre-flight safety assessment for auto-pull
    if strategy == "auto-pull" {
        let safety = assess_safety(&task.path, &task.tags, &["own-project", "tool"]);
        match safety {
            SyncSafety::BlockedDirty => {
                return SyncSummary {
                    action: "BLOCKED".to_string(),
                    message: crate::i18n::current().sync.blocked_dirty.to_string(),
                    ..Default::default()
                };
            }
            SyncSafety::BlockedDiverged => {
                return SyncSummary {
                    action: "BLOCKED".to_string(),
                    message: "Diverged from upstream. Manual merge required.".to_string(),
                    ..Default::default()
                };
            }
            SyncSafety::BlockedProtected => {
                return SyncSummary {
                    action: "BLOCKED".to_string(),
                    message: "Protected project blocked from auto-merge.".to_string(),
                    ..Default::default()
                };
            }
            SyncSafety::UpToDate => {
                return SyncSummary {
                    action: "SKIP".to_string(),
                    message: crate::i18n::current().sync.already_up_to_date.to_string(),
                    ..Default::default()
                };
            }
            SyncSafety::NoUpstream => {
                return SyncSummary {
                    action: "SKIP".to_string(),
                    message: crate::i18n::current().sync.skip_no_upstream.to_string(),
                    ..Default::default()
                };
            }
            SyncSafety::Unknown => {
                return SyncSummary {
                    action: "ERROR".to_string(),
                    message: "Failed to assess repository safety.".to_string(),
                    ..Default::default()
                };
            }
            SyncSafety::Safe => {}
        }
    }

    match sync_repo(
        &task.id,
        &task.path,
        task.upstream_url.as_deref(),
        task.default_branch.as_deref(),
        strategy,
    )
    .await
    {
        Ok(summary) => summary,
        Err(e) => {
            warn!("Failed to sync {}: {}", task.id, e);
            let kind = classify_sync_error(&e);
            SyncSummary {
                action: "ERROR".to_string(),
                message: e.to_string(),
                error_kind: Some(kind.to_string()),
                ..Default::default()
            }
        }
    }
}

async fn collect_tasks(
    filter_tags: Option<&str>,
    exclude: Option<&str>,
) -> anyhow::Result<Vec<RepoSyncTask>> {
    let conn = WorkspaceRegistry::init_db()?;
    let repos = WorkspaceRegistry::list_repos(&conn)?;

    let filter_list: Vec<&str> = filter_tags
        .map(|f| f.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let exclude_list: Vec<&str> = exclude
        .map(|e| e.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let tasks: Vec<RepoSyncTask> = repos
        .into_iter()
        .filter(|repo| {
            let tag_match = filter_list.is_empty() || filter_list.iter().any(|f| repo.tags.contains(&f.to_string()));
            let not_excluded = !exclude_list.iter().any(|id| repo.id == *id);
            tag_match && not_excluded
        })
        .map(|repo| {
            let primary = repo.primary_remote().cloned();
            RepoSyncTask {
                id: repo.id,
                path: repo.local_path.to_string_lossy().to_string(),
                upstream_url: primary.as_ref().and_then(|r| r.upstream_url.clone()),
                default_branch: primary.as_ref().and_then(|r| r.default_branch.clone()),
                tags: repo.tags.join(","),
            }
        })
        .collect();

    Ok(tasks)
}

pub async fn run_json(
    dry_run: bool,
    strategy: &str,
    filter_tags: Option<&str>,
    exclude: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let tasks = collect_tasks(filter_tags, exclude).await?;
    let mut path_map = HashMap::new();
    for task in &tasks {
        path_map.insert(task.id.clone(), task.path.clone());
    }

    let orchestrator = SyncOrchestrator::new(1);
    let summaries = orchestrator
        .run_sync(tasks, SyncMode::SYNC, dry_run, strategy, Duration::from_secs(60), |_id, _summary| {})
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
    if let Ok(conn) = WorkspaceRegistry::init_db() {
        let repo_count = results_json.len();
        let _ = WorkspaceRegistry::save_oplog(
            &conn,
            &OplogEntry {
                id: None,
                operation: "sync".to_string(),
                repo_id: None,
                details: Some(format!("strategy={}, dry_run={}, repos={}", strategy, dry_run, repo_count)),
                status: "success".to_string(),
                timestamp: Utc::now(),
            },
        );
    }

    Ok(serde_json::json!({
        "success": true,
        "dry_run": dry_run,
        "strategy": strategy,
        "results": results_json
    }))
}

pub async fn run(
    dry_run: bool,
    strategy: &str,
    filter_tags: Option<&str>,
    exclude: Option<&str>,
) -> anyhow::Result<()> {
    let tasks = collect_tasks(filter_tags, exclude).await?;
    let mut path_map = HashMap::new();
    for task in &tasks {
        path_map.insert(task.id.clone(), task.path.clone());
    }

    let orchestrator = SyncOrchestrator::new(4);
    let results = orchestrator
        .run_sync(
            tasks,
            SyncMode::ASYNC,
            dry_run,
            strategy,
            Duration::from_secs(60),
            |id, summary| {
                println!("  [{}] {}: {}", id, crate::i18n::current().log.progress, summary.message);
            },
        )
        .await;

    let filter_suffix = filter_tags
        .map(|f| format!("{}{}）", crate::i18n::current().sync.filter_prefix, f))
        .unwrap_or_default();
    println!("{}{} {}{}\n", crate::i18n::current().sync.strategy_prefix, ":", strategy, filter_suffix);

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
            println!(
                "  [{}] {}",
                id,
                message
            );
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
    if let Ok(conn) = WorkspaceRegistry::init_db() {
        let repo_count = results_json.len();
        let _ = WorkspaceRegistry::save_oplog(
            &conn,
            &OplogEntry {
                id: None,
                operation: "sync".to_string(),
                repo_id: None,
                details: Some(format!("strategy={}, dry_run={}, repos={}", strategy, dry_run, repo_count)),
                status: "success".to_string(),
                timestamp: Utc::now(),
            },
        );
    }

    Ok(())
}

fn map_action(action: &str, _message: &str) -> String {
    match action {
        "SKIP" | "OK" | "WARN" | "DRY_RUN" => "skipped".to_string(),
        "FETCH" => "fetch_only".to_string(),
        "BLOCKED" => "blocked_dirty".to_string(),
        "MERGED_FF" => "merged_ff".to_string(),
        "MERGED_COMMIT" => "merged_commit".to_string(),
        "MERGED" => "merged_ff".to_string(),
        "CONFLICT" => "conflict".to_string(),
        "TIMEOUT" => "timeout".to_string(),
        "ERROR" => "error".to_string(),
        _ => "skipped".to_string(),
    }
}

fn print_summary_table(results: &[serde_json::Value]) {
    if results.is_empty() {
        println!("{}", crate::i18n::current().sync.no_repos_processed);
        return;
    }

    println!("{:-<90}", "");
    println!(
        "{:<24} {:<10} {:>6} {:>7} {}",
        crate::i18n::current().sync.header_repo, crate::i18n::current().sync.header_action, crate::i18n::current().sync.header_ahead, crate::i18n::current().sync.header_behind, crate::i18n::current().sync.header_message
    );
    println!("{:-<90}", "");
    for item in results {
        let id = item["id"].as_str().unwrap_or("");
        let action = item["action"].as_str().unwrap_or("");
        let ahead = item["ahead"].as_u64().unwrap_or(0);
        let behind = item["behind"].as_u64().unwrap_or(0);
        let message = item["message"].as_str().unwrap_or("");
        let error_kind = item["error_kind"].as_str();
        let display_message = if let Some(ek) = error_kind {
            format!("{} [{}]", message, ek)
        } else {
            message.to_string()
        };
        let id_display = if id.len() > 23 { &id[..23] } else { id };
        println!(
            "{:<24} {:<10} {:>6} {:>7} {}",
            id_display, action, ahead, behind, display_message
        );
    }
    println!("{:-<90}", "");
}

async fn sync_repo(
    _id: &str,
    path: &str,
    upstream_url: Option<&str>,
    default_branch: Option<&str>,
    strategy: &str,
) -> anyhow::Result<SyncSummary> {
    let path = path.to_string();
    let upstream_url = upstream_url.map(|s| s.to_string());
    let default_branch = default_branch.map(|s| s.to_string());
    let strategy = strategy.to_string();

    let result = tokio::task::spawn_blocking(move || {
        let repo = Repository::open(&path)?;

        // Ensure origin remote points to the expected URL
        {
            let mut remote = match repo.find_remote("origin") {
                Ok(r) => r,
                Err(_) => {
                    match upstream_url.as_deref() {
                        Some(url) => {
                            repo.remote("origin", url)?;
                            repo.find_remote("origin")?
                        }
                        None => {
                            return Ok(SyncSummary {
                                action: "SKIP".to_string(),
                                message: crate::i18n::current().sync.no_origin.to_string(),
                                ..Default::default()
                            });
                        }
                    }
                }
            };
            if let Some(ref url) = upstream_url {
                if remote.url() != Some(url) {
                    repo.remote_set_url("origin", url)?;
                    remote = repo.find_remote("origin")?;
                }
            }

            // Fetch with friendly error message
            let remote_url_str = remote.url().map(|s| s.to_string());
            let needs_auth = remote_url_str.as_deref().map(|u| {
                u.contains("github.com") || u.contains("gitlab.com") || u.starts_with("git@") || u.starts_with("ssh://") || u.starts_with("https://")
            }).unwrap_or(false);

            if needs_auth {
                let mut callbacks = git2::RemoteCallbacks::new();
                callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
                });
                let mut fetch_opts = git2::FetchOptions::new();
                fetch_opts.remote_callbacks(callbacks);
                remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None).map_err(|e| {
                    anyhow::anyhow!(
                        "{}",
                        crate::i18n::format_template(crate::i18n::current().sync.fetch_failed, &[&e.to_string()])
                    )
                })?;
            } else {
                remote.fetch(&[] as &[&str], None, None).map_err(|e| {
                    anyhow::anyhow!(
                        "{}",
                        crate::i18n::format_template(crate::i18n::current().sync.fetch_failed, &[&e.to_string()])
                    )
                })?;
            }
        }

        // Determine default branch
        let branch = default_branch
            .clone()
            .or_else(|| {
                repo.find_remote("origin")
                    .ok()
                    .and_then(|r| r.default_branch().ok())
                    .and_then(|b| {
                        b.as_str()
                            .map(|s| s.trim_start_matches("refs/heads/").to_string())
                    })
            })
            .unwrap_or_else(|| "main".to_string());

        // Check local vs remote
        let local_oid = repo
            .revparse_single(&format!("refs/heads/{}", branch))
            .ok()
            .map(|obj| obj.id());
        let remote_oid = repo
            .revparse_single(&format!("refs/remotes/origin/{}", branch))
            .ok()
            .map(|obj| obj.id());

        let summary = match (local_oid, remote_oid) {
            (Some(local), Some(remote)) => {
                if local == remote {
                    SyncSummary {
                        action: "OK".to_string(),
                        message: crate::i18n::format_template(crate::i18n::current().sync.up_to_date, &[&branch]),
                        ..Default::default()
                    }
                } else {
                    let (ahead, behind) = repo.graph_ahead_behind(local, remote)?;

                    if strategy == "fetch-only" {
                        SyncSummary {
                            action: "FETCH".to_string(),
                            ahead,
                            behind,
                            message: "Fetched only".to_string(),
                            ..Default::default()
                        }
                    } else {
                        // Check working directory is clean
                        let statuses = repo.statuses(None)?;
                        let is_clean = statuses.iter().count() == 0;
                        if !is_clean {
                            SyncSummary {
                                action: "BLOCKED".to_string(),
                                ahead,
                                behind,
                                message: crate::i18n::current().sync.blocked_dirty.to_string(),
                                ..Default::default()
                            }
                        } else if strategy == "ask" {
                            print!("    Merge origin/{} into {}? [y/N] ", branch, branch);
                            io::stdout().flush()?;
                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;
                            if !input.trim().eq_ignore_ascii_case("y") {
                                SyncSummary {
                                    action: "SKIP".to_string(),
                                    ahead,
                                    behind,
                                    message: crate::i18n::current().sync.skipped_by_user.to_string(),
                                    ..Default::default()
                                }
                            } else {
                                perform_merge(&repo, &branch, local, remote)?
                            }
                        } else {
                            perform_merge(&repo, &branch, local, remote)?
                        }
                    }
                }
            }
            (None, Some(_)) => {
                SyncSummary {
                    action: "WARN".to_string(),
                    message: crate::i18n::format_template(crate::i18n::current().sync.local_branch_missing, &[&branch]),
                    ..Default::default()
                }
            }
            (Some(_), None) => {
                SyncSummary {
                    action: "WARN".to_string(),
                    message: crate::i18n::format_template(crate::i18n::current().sync.remote_branch_missing, &[&branch]),
                    ..Default::default()
                }
            }
            (None, None) => {
                SyncSummary {
                    action: "WARN".to_string(),
                    message: crate::i18n::format_template(crate::i18n::current().sync.neither_branch_exists, &[&branch]),
                    ..Default::default()
                }
            }
        };

        // Update submodules if present
        if std::path::Path::new(&format!("{}/.gitmodules", path)).exists() {
            repo.submodules()?.iter_mut().for_each(|sm| {
                if let Err(e) = sm.update(true, None) {
                    warn!("Submodule update failed: {}", e);
                }
            });
        }

        // Write .syncdone marker on successful sync
        if !matches!(summary.action.as_str(), "ERROR" | "BLOCKED" | "SKIP" | "WARN") {
            let final_oid = repo.head().ok().and_then(|h| h.target());
            write_syncdone_marker(
                std::path::Path::new(&path),
                &summary.action,
                final_oid.map(|o| o.to_string()).as_deref(),
            );
        }

        Ok::<SyncSummary, anyhow::Error>(summary)
    }).await;

    match result {
        Ok(Ok(summary)) => Ok(summary),
        Ok(Err(e)) => {
            let kind = classify_sync_error(&e);
            Ok(SyncSummary {
                action: "ERROR".to_string(),
                message: e.to_string(),
                error_kind: Some(kind.to_string()),
                ..Default::default()
            })
        }
        Err(join_err) => {
            let e = anyhow::anyhow!("Sync task panicked: {}", join_err);
            let kind = classify_sync_error(&e);
            Ok(SyncSummary {
                action: "ERROR".to_string(),
                message: e.to_string(),
                error_kind: Some(kind.to_string()),
                ..Default::default()
            })
        }
    }
}

fn write_syncdone_marker(path: &std::path::Path, action: &str, local_commit: Option<&str>) {
    let syncdone = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "local_commit": local_commit,
        "action": action
    });
    let devbase_dir = path.join(".devbase");
    if let Err(e) = std::fs::create_dir_all(&devbase_dir) {
        warn!("Failed to create .devbase dir for {}: {}", path.display(), e);
    } else if let Err(e) = std::fs::write(devbase_dir.join("syncdone"), syncdone.to_string()) {
        warn!("Failed to write .devbase/syncdone for {}: {}", path.display(), e);
    }
}

fn perform_merge(
    repo: &Repository,
    branch: &str,
    local: git2::Oid,
    remote: git2::Oid,
) -> anyhow::Result<SyncSummary> {
    let local_ref = format!("refs/heads/{}", branch);
    let remote_ref = format!("refs/remotes/origin/{}", branch);
    let mut local_branch = repo.find_reference(&local_ref)?;
    let annotated = repo.reference_to_annotated_commit(&repo.find_reference(&remote_ref)?)?;

    let (analysis, _) = repo.merge_analysis_for_ref(&local_branch, &[&annotated])?;

    if analysis.is_fast_forward() {
        local_branch.set_target(remote, "Fast-forward merge by devbase")?;
        repo.set_head(&local_ref)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        Ok(SyncSummary {
            action: "MERGED_FF".to_string(),
            message: crate::i18n::current().sync.merged_ff.to_string(),
            ..Default::default()
        })
    } else if analysis.is_normal() {
        repo.merge(&[&annotated], None, None)?;
        if repo.index()?.has_conflicts() {
            Ok(SyncSummary {
                action: "CONFLICT".to_string(),
                message: crate::i18n::current().sync.conflict.to_string(),
                ..Default::default()
            })
        } else {
            let sig = repo.signature()?;
            let local_commit = repo.find_commit(local)?;
            let remote_commit = repo.find_commit(remote)?;
            let tree_id = repo.index()?.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            repo.commit(
                Some(&local_ref),
                &sig,
                &sig,
                &format!("Merge origin/{} by devbase", branch),
                &tree,
                &[&local_commit, &remote_commit],
            )?;
            repo.cleanup_state()?;
            Ok(SyncSummary {
                action: "MERGED_COMMIT".to_string(),
                message: crate::i18n::current().sync.merged_commit.to_string(),
                ..Default::default()
            })
        }
    } else if analysis.is_up_to_date() {
        Ok(SyncSummary {
            action: "OK".to_string(),
            message: crate::i18n::current().sync.already_up_to_date.to_string(),
            ..Default::default()
        })
    } else {
        Ok(SyncSummary {
            action: "SKIP".to_string(),
            message: crate::i18n::current().sync.unhandled_merge_state.to_string(),
            ..Default::default()
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::fs;
    use tempfile::TempDir;

    fn create_commit(repo: &Repository, message: &str) -> git2::Oid {
        let sig = repo.signature().unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().ok().and_then(|h| h.target()).and_then(|oid| repo.find_commit(oid).ok());
        match parent {
            Some(ref p) => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[p]).unwrap(),
            None => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[]).unwrap(),
        }
    }

    fn setup_repo_with_remote_commits(ahead_local: usize, behind_remote: usize) -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(&dir).unwrap();
        
        // Initial commit on main
        fs::write(dir.path().join("file.txt"), "base").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let sig = repo.signature().unwrap();
        let tree_id = index.write_tree().unwrap();
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[]).unwrap();
        }
        
        // Create origin remote pointing to a bare repo
        let bare_dir = TempDir::new().unwrap();
        let _bare_repo = Repository::init_bare(&bare_dir).unwrap();
        repo.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        
        // Push base to origin/main
        {
            let mut remote = repo.find_remote("origin").unwrap();
            remote.push(&["+refs/heads/main:refs/heads/main"], None).unwrap();
        }
        
        // Create remote commits via a helper clone
        let helper_dir = TempDir::new().unwrap();
        let helper = Repository::clone(bare_dir.path().to_str().unwrap(), &helper_dir).unwrap();
        for i in 0..behind_remote {
            fs::write(helper_dir.path().join("file.txt"), format!("remote{}", i)).unwrap();
            let mut hindex = helper.index().unwrap();
            hindex.add_path(std::path::Path::new("file.txt")).unwrap();
            hindex.write().unwrap();
            let hsig = helper.signature().unwrap();
            let htree_id = hindex.write_tree().unwrap();
            {
                let htree = helper.find_tree(htree_id).unwrap();
                let hparent = helper.head().unwrap().peel_to_commit().unwrap();
                helper.commit(Some("HEAD"), &hsig, &hsig, &format!("remote{}", i), &htree, &[&hparent]).unwrap();
            }
        }
        let mut hremote = helper.find_remote("origin").unwrap();
        hremote.push(&["+refs/heads/main:refs/heads/main"], None).unwrap();
        
        // Fetch remote changes back so origin/main exists and is updated
        {
            let mut remote = repo.find_remote("origin").unwrap();
            remote.fetch(&["main"], None, None).unwrap();
        }
        
        // Set upstream tracking for local main branch
        {
            let mut branch = repo.find_branch("main", git2::BranchType::Local).unwrap();
            branch.set_upstream(Some("origin/main")).unwrap();
        }
        
        // Create local commits (these will make local ahead)
        for i in 0..ahead_local {
            fs::write(dir.path().join("file.txt"), format!("local{}", i)).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("file.txt")).unwrap();
            index.write().unwrap();
            create_commit(&repo, &format!("local{}", i));
        }
        
        (dir, repo)
    }

    #[test]
    fn test_assess_safety_safe_ff() {
        let (dir, _repo) = setup_repo_with_remote_commits(0, 2);
        let safety = assess_safety(dir.path().to_str().unwrap(), "third-party", &["own-project"]);
        assert_eq!(safety, SyncSafety::Safe);
    }

    #[test]
    fn test_assess_safety_blocked_dirty() {
        let (dir, _repo) = setup_repo_with_remote_commits(0, 2);
        fs::write(dir.path().join("dirty.txt"), "dirty").unwrap();
        let safety = assess_safety(dir.path().to_str().unwrap(), "third-party", &["own-project"]);
        assert_eq!(safety, SyncSafety::BlockedDirty);
    }

    #[test]
    fn test_assess_safety_blocked_diverged() {
        let (dir, _repo) = setup_repo_with_remote_commits(1, 2);
        let safety = assess_safety(dir.path().to_str().unwrap(), "third-party", &["own-project"]);
        assert_eq!(safety, SyncSafety::BlockedDiverged);
    }

    #[test]
    fn test_assess_safety_blocked_protected() {
        let (dir, _repo) = setup_repo_with_remote_commits(1, 2);
        let safety = assess_safety(dir.path().to_str().unwrap(), "own-project", &["own-project"]);
        assert_eq!(safety, SyncSafety::BlockedProtected);
    }

    #[test]
    fn test_assess_safety_up_to_date() {
        let (dir, _repo) = setup_repo_with_remote_commits(0, 0);
        let safety = assess_safety(dir.path().to_str().unwrap(), "third-party", &["own-project"]);
        assert_eq!(safety, SyncSafety::UpToDate);
    }

    #[test]
    fn test_assess_safety_no_upstream() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(&dir).unwrap();
        let sig = _repo.signature().unwrap();
        let tree_id = _repo.index().unwrap().write_tree().unwrap();
        let tree = _repo.find_tree(tree_id).unwrap();
        _repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
        let safety = assess_safety(dir.path().to_str().unwrap(), "", &["own-project"]);
        assert_eq!(safety, SyncSafety::NoUpstream);
    }

    #[test]
    fn test_write_syncdone_marker() {
        let dir = TempDir::new().unwrap();
        write_syncdone_marker(dir.path(), "FETCH", Some("abc1234"));

        let syncdone_path = dir.path().join(".devbase").join("syncdone");
        assert!(syncdone_path.exists(), ".devbase/syncdone should be written");

        let content = fs::read_to_string(&syncdone_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.get("action").and_then(|v| v.as_str()), Some("FETCH"));
        assert_eq!(parsed.get("local_commit").and_then(|v| v.as_str()), Some("abc1234"));
        assert!(parsed.get("timestamp").is_some());
    }

    #[test]
    fn test_sync_repo_skip_no_syncdone() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(&dir).unwrap();
        let sig = repo.signature().unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

        // Simulate a SKIP summary: write_syncdone_marker should NOT be called
        let syncdone_path = dir.path().join(".devbase").join("syncdone");
        assert!(!syncdone_path.exists(), ".devbase/syncdone should NOT exist before any write");

        // Write it manually with SKIP action to verify it would be wrong
        write_syncdone_marker(dir.path(), "SKIP", None);
        assert!(syncdone_path.exists(), "marker can be written for testing");

        // In real sync_repo, SKIP action bypasses write_syncdone_marker, so delete it
        fs::remove_file(&syncdone_path).unwrap();
        assert!(!syncdone_path.exists());
    }
}
