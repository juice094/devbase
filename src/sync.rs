use crate::registry::WorkspaceRegistry;
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
                    let summary = match timeout(Duration::from_secs(30), execute_task(&task, dry_run, strategy)).await {
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
                let mut handles = Vec::with_capacity(repos.len());
                for task in repos {
                    on_progress(
                        task.id.clone(),
                        SyncSummary {
                            action: "RUNNING".to_string(),
                            message: crate::i18n::current().sync.status_running.to_string(),
                            ..Default::default()
                        },
                    );
                    let permit = self
                        .semaphore
                        .clone()
                        .acquire_owned()
                        .await
                        .expect("semaphore should not be closed");
                    let strategy = strategy.to_string();
                    let handle = tokio::spawn(async move {
                        let summary = match timeout(Duration::from_secs(30), execute_task(&task, dry_run, &strategy)).await {
                            Ok(s) => s,
                            Err(_) => SyncSummary {
                                action: "TIMEOUT".to_string(),
                                message: crate::i18n::current().sync.network_timeout.to_string(),
                                ..Default::default()
                            },
                        };
                        (task.id, summary, permit)
                    });
                    handles.push(handle);
                }

                let mut results = Vec::with_capacity(handles.len());
                for handle in handles {
                    let (id, summary, _permit) = handle.await.unwrap();
                    on_progress(id.clone(), summary.clone());
                    results.push((id, summary));
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
        .run_sync(tasks, SyncMode::SYNC, dry_run, strategy, |_id, _summary| {})
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
