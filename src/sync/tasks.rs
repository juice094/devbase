use crate::registry::repo;
use git2::Repository;
use tracing::warn;

use super::SyncSummary;
use super::policy::{RepoSyncTask, SyncPolicy, SyncSafety, assess_safety, classify_sync_error};

pub(super) fn fetch_single_repo(
    path: &str,
    upstream_url: Option<&str>,
    i18n: crate::i18n::I18n,
) -> anyhow::Result<()> {
    let repo = Repository::open(path)?;

    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(_) => {
            let url = upstream_url.ok_or_else(|| anyhow::anyhow!("no upstream URL"))?;
            repo.remote("origin", url)?;
            repo.find_remote("origin")?
        }
    };

    if let Some(url) = upstream_url
        && remote.url() != Some(url)
    {
        repo.remote_set_url("origin", url)?;
        remote = repo.find_remote("origin")?;
    }

    let remote_url_str = remote.url().map(|s| s.to_string());
    let needs_auth = remote_url_str
        .as_deref()
        .map(|u| {
            u.contains("github.com")
                || u.contains("gitlab.com")
                || u.starts_with("git@")
                || u.starts_with("ssh://")
                || u.starts_with("https://")
        })
        .unwrap_or(false);

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
                crate::i18n::format_template(i18n.sync.fetch_failed, &[&e.to_string()])
            )
        })?;
    } else {
        remote.fetch(&[] as &[&str], None, None).map_err(|e| {
            anyhow::anyhow!(
                "{}",
                crate::i18n::format_template(i18n.sync.fetch_failed, &[&e.to_string()])
            )
        })?;
    }

    // Write syncdone marker so health indicators reflect the fetch
    let head_oid = repo.head().ok().and_then(|h| h.target());
    write_syncdone_marker(
        std::path::Path::new(path),
        "FETCH",
        head_oid.map(|o| o.to_string()).as_deref(),
    );

    Ok(())
}

pub(super) async fn execute_task(
    task: &RepoSyncTask,
    dry_run: bool,
    i18n: crate::i18n::I18n,
) -> SyncSummary {
    if dry_run {
        let url = task.upstream_url.as_deref().unwrap_or("?");
        return SyncSummary {
            action: "DRY_RUN".to_string(),
            message: crate::i18n::format_template(i18n.sync.would_fetch, &[url, &task.path]),
            ..Default::default()
        };
    }

    // Pre-flight safety assessment based on per-repo policy
    let path = task.path.clone();
    let policy = task.policy;
    let (safety, _ahead, _behind) =
        tokio::task::spawn_blocking(move || assess_safety(&path, policy))
            .await
            .unwrap_or((SyncSafety::Unknown, 0, 0));

    match safety {
        SyncSafety::BlockedDirty => {
            return SyncSummary {
                action: "BLOCKED".to_string(),
                message: i18n.sync.blocked_dirty.to_string(),
                ..Default::default()
            };
        }
        SyncSafety::BlockedDiverged => {
            return SyncSummary {
                action: "BLOCKED".to_string(),
                message: "Diverged from upstream. Policy prevents auto-resolution.".to_string(),
                ..Default::default()
            };
        }
        SyncSafety::UpToDate => {
            return SyncSummary {
                action: "SKIP".to_string(),
                message: i18n.sync.already_up_to_date.to_string(),
                ..Default::default()
            };
        }
        SyncSafety::LocalAhead => {
            // Local ahead of remote; try to push if policy allows
            if policy.can_push() {
                match sync_repo_push(&task.path, &task.id).await {
                    Ok(summary) => return summary,
                    Err(e) => {
                        warn!("Failed to push {}: {}", task.id, e);
                        let kind = classify_sync_error(&e);
                        return SyncSummary {
                            action: "ERROR".to_string(),
                            message: e.to_string(),
                            error_kind: Some(kind.to_string()),
                            ..Default::default()
                        };
                    }
                }
            } else {
                return SyncSummary {
                    action: "SKIP".to_string(),
                    message: i18n.sync.already_up_to_date.to_string(),
                    ..Default::default()
                };
            }
        }
        SyncSafety::NoUpstream => {
            return SyncSummary {
                action: "SKIP".to_string(),
                message: i18n.sync.skip_no_upstream.to_string(),
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

    match sync_repo(
        &task.id,
        &task.path,
        task.upstream_url.as_deref(),
        task.default_branch.as_deref(),
        policy,
        i18n,
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

const MANAGED_TAGS: &[&str] = &[
    "mirror",
    "reference",
    "third-party",
    "collaborative",
    "team",
    "own-project",
    "tool",
    "active",
    "managed",
];

pub(super) async fn collect_tasks(
    conn: &rusqlite::Connection,
    filter_tags: Option<&str>,
    exclude: Option<&str>,
    exclude_paths: &[String],
) -> anyhow::Result<(Vec<RepoSyncTask>, usize)> {
    let repos = repo::list_repos(conn)?;

    let is_default_mode = filter_tags.is_none();
    let filter_list: Vec<&str> = filter_tags
        .map(|f| f.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let exclude_list: Vec<&str> = exclude
        .map(|e| e.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let mut skipped_unmanaged = 0usize;
    let tasks: Vec<RepoSyncTask> = repos
        .into_iter()
        .filter(|repo| {
            let tag_match = if is_default_mode {
                repo.tags.iter().any(|t| MANAGED_TAGS.contains(&t.as_str()))
            } else {
                filter_list.iter().any(|f| repo.tags.contains(&f.to_string()))
            };
            let not_excluded = !exclude_list.iter().any(|id| repo.id == *id);
            let not_path_excluded =
                !crate::scan::is_excluded_path(&repo.local_path, exclude_paths, None);
            let included = tag_match && not_excluded && not_path_excluded;
            if is_default_mode && !included && !tag_match {
                skipped_unmanaged += 1;
            }
            included
        })
        .map(|repo| {
            let primary = repo.primary_remote().cloned();
            let tags = repo.tags.join(",");
            let policy = SyncPolicy::from_tags(&tags);
            RepoSyncTask {
                id: repo.id,
                path: repo.local_path.to_string_lossy().to_string(),
                upstream_url: primary.as_ref().and_then(|r| r.upstream_url.clone()),
                default_branch: primary.as_ref().and_then(|r| r.default_branch.clone()),
                policy,
            }
        })
        .collect();

    Ok((tasks, skipped_unmanaged))
}

pub(super) fn map_action(action: &str, _message: &str) -> String {
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

pub(super) fn print_summary_table(results: &[serde_json::Value], i18n: &crate::i18n::I18n) {
    if results.is_empty() {
        println!("{}", i18n.sync.no_repos_processed);
        return;
    }

    println!("{:-<90}", "");
    println!(
        "{:<24} {:<10} {:>6} {:>7} {}",
        i18n.sync.header_repo,
        i18n.sync.header_action,
        i18n.sync.header_ahead,
        i18n.sync.header_behind,
        i18n.sync.header_message
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

async fn sync_repo_push(path: &str, _id: &str) -> anyhow::Result<SyncSummary> {
    let path = path.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let repo = Repository::open(&path)?;
        let head = repo.head()?;
        let branch = head.shorthand().unwrap_or("main");
        let local_ref = format!("refs/heads/{}", branch);

        let mut remote = repo.find_remote("origin")?;
        let mut push_opts = git2::PushOptions::new();
        remote
            .push(&[&local_ref], Some(&mut push_opts))
            .map_err(|e| anyhow::anyhow!("Push failed: {}", e))?;

        Ok::<SyncSummary, anyhow::Error>(SyncSummary {
            action: "PUSHED".to_string(),
            message: format!("Pushed {} to origin", branch),
            ..Default::default()
        })
    })
    .await;

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
            let e = anyhow::anyhow!("Push task panicked: {}", join_err);
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

async fn sync_repo(
    _id: &str,
    path: &str,
    upstream_url: Option<&str>,
    default_branch: Option<&str>,
    policy: SyncPolicy,
    i18n: crate::i18n::I18n,
) -> anyhow::Result<SyncSummary> {
    let path = path.to_string();
    let upstream_url = upstream_url.map(|s| s.to_string());
    let default_branch = default_branch.map(|s| s.to_string());

    let result = tokio::task::spawn_blocking(move || {
        let repo = Repository::open(&path)?;

        // Fetch latest remote state
        if let Err(e) = fetch_single_repo(&path, upstream_url.as_deref(), i18n) {
            return Ok(SyncSummary {
                action: "ERROR".to_string(),
                message: e.to_string(),
                ..Default::default()
            });
        }

        // Determine default branch
        let branch = default_branch
            .clone()
            .or_else(|| {
                repo.find_remote("origin").ok().and_then(|r| r.default_branch().ok()).and_then(
                    |b| b.as_str().map(|s| s.trim_start_matches("refs/heads/").to_string()),
                )
            })
            .unwrap_or_else(|| "main".to_string());

        // Check local vs remote
        let local_oid =
            repo.revparse_single(&format!("refs/heads/{}", branch)).ok().map(|obj| obj.id());
        let remote_oid = repo
            .revparse_single(&format!("refs/remotes/origin/{}", branch))
            .ok()
            .map(|obj| obj.id());

        let summary = match (local_oid, remote_oid) {
            (Some(local), Some(remote)) => {
                if local == remote {
                    SyncSummary {
                        action: "OK".to_string(),
                        message: crate::i18n::format_template(i18n.sync.up_to_date, &[&branch]),
                        ..Default::default()
                    }
                } else {
                    let (ahead, behind) = repo.graph_ahead_behind(local, remote)?;

                    if matches!(policy, SyncPolicy::Mirror) {
                        SyncSummary {
                            action: "FETCH".to_string(),
                            ahead,
                            behind,
                            message: "Fetched only".to_string(),
                            ..Default::default()
                        }
                    } else {
                        // Working directory cleanliness was already checked in assess_safety
                        if ahead > 0 && behind > 0 {
                            // Diverged: rebase or merge based on policy
                            if policy.can_rebase() {
                                perform_rebase(&repo, &branch, local, remote)?
                            } else {
                                perform_merge(&repo, &branch, local, remote, i18n)?
                            }
                        } else {
                            perform_merge(&repo, &branch, local, remote, i18n)?
                        }
                    }
                }
            }
            (None, Some(_)) => SyncSummary {
                action: "WARN".to_string(),
                message: crate::i18n::format_template(i18n.sync.local_branch_missing, &[&branch]),
                ..Default::default()
            },
            (Some(_), None) => SyncSummary {
                action: "WARN".to_string(),
                message: crate::i18n::format_template(i18n.sync.remote_branch_missing, &[&branch]),
                ..Default::default()
            },
            (None, None) => SyncSummary {
                action: "WARN".to_string(),
                message: crate::i18n::format_template(i18n.sync.neither_branch_exists, &[&branch]),
                ..Default::default()
            },
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
    })
    .await;

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

pub(super) fn write_syncdone_marker(
    path: &std::path::Path,
    action: &str,
    local_commit: Option<&str>,
) {
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

pub(super) fn perform_rebase(
    repo: &Repository,
    branch: &str,
    _local: git2::Oid,
    _remote: git2::Oid,
) -> anyhow::Result<SyncSummary> {
    let local_ref = format!("refs/heads/{}", branch);
    let remote_ref = format!("refs/remotes/origin/{}", branch);

    // Rebase current branch onto origin/branch
    let annotated_local = repo.reference_to_annotated_commit(&repo.find_reference(&local_ref)?)?;
    let annotated_remote =
        repo.reference_to_annotated_commit(&repo.find_reference(&remote_ref)?)?;

    let mut rebase = repo.rebase(Some(&annotated_local), Some(&annotated_remote), None, None)?;

    while let Some(op) = rebase.next() {
        let _op = op?;
        // Check for conflicts after each operation
        if repo.index()?.has_conflicts() {
            rebase.abort()?;
            return Ok(SyncSummary {
                action: "CONFLICT".to_string(),
                message: format!("Rebase conflict on {}. Aborted.", branch),
                ..Default::default()
            });
        }
        rebase.commit(None, &repo.signature()?, None)?;
    }

    // Finish rebase
    rebase.finish(None)?;

    Ok(SyncSummary {
        action: "REBASED".to_string(),
        message: format!("Rebased {} onto origin/{}", branch, branch),
        ..Default::default()
    })
}

pub(super) fn perform_merge(
    repo: &Repository,
    branch: &str,
    local: git2::Oid,
    remote: git2::Oid,
    i18n: crate::i18n::I18n,
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
            message: i18n.sync.merged_ff.to_string(),
            ..Default::default()
        })
    } else if analysis.is_normal() {
        repo.merge(&[&annotated], None, None)?;
        if repo.index()?.has_conflicts() {
            // Abort the merge to leave repo in a clean state
            let _ = repo.cleanup_state();
            Ok(SyncSummary {
                action: "CONFLICT".to_string(),
                message: i18n.sync.conflict.to_string(),
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
                message: i18n.sync.merged_commit.to_string(),
                ..Default::default()
            })
        }
    } else if analysis.is_up_to_date() {
        Ok(SyncSummary {
            action: "OK".to_string(),
            message: i18n.sync.already_up_to_date.to_string(),
            ..Default::default()
        })
    } else {
        Ok(SyncSummary {
            action: "SKIP".to_string(),
            message: i18n.sync.unhandled_merge_state.to_string(),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_action_known() {
        assert_eq!(map_action("SKIP", ""), "skipped");
        assert_eq!(map_action("FETCH", ""), "fetch_only");
        assert_eq!(map_action("BLOCKED", ""), "blocked_dirty");
        assert_eq!(map_action("MERGED_FF", ""), "merged_ff");
        assert_eq!(map_action("CONFLICT", ""), "conflict");
        assert_eq!(map_action("ERROR", ""), "error");
    }

    #[test]
    fn test_map_action_unknown() {
        assert_eq!(map_action("UNKNOWN", ""), "skipped");
        assert_eq!(map_action("", ""), "skipped");
    }

    #[test]
    fn test_write_syncdone_marker() {
        let dir = tempfile::tempdir().unwrap();
        write_syncdone_marker(dir.path(), "MERGED_FF", Some("abc123"));
        let path = dir.path().join(".devbase").join("syncdone");
        assert!(path.exists());
        let contents = std::fs::read_to_string(path).unwrap();
        assert!(contents.contains("MERGED_FF"));
        assert!(contents.contains("abc123"));
    }
}
