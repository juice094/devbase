use git2::Repository;

pub(super) fn classify_sync_error(error: &anyhow::Error) -> &'static str {
    let msg = error.to_string().to_lowercase();
    if msg.contains("network") || msg.contains("could not resolve") || msg.contains("connection") {
        "network-error"
    } else if msg.contains("authentication")
        || msg.contains("credentials")
        || msg.contains("403")
        || msg.contains("401")
    {
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
    pub policy: SyncPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncMode {
    Sync,
    Async,
    BlockUi,
}

/// Per-repository sync policy. Determined by repository tags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncPolicy {
    /// Only fetch; never modify local branches or push.
    Mirror,
    /// Fast-forward merge only; block on diverge or local-ahead.
    Conservative,
    /// Rebase local commits onto remote, then push.
    Rebase,
    /// Merge remote into local (merge commit), then push.
    Merge,
}

impl SyncPolicy {
    /// Infer policy from repository tags.
    pub fn from_tags(tags: &str) -> Self {
        let tags: Vec<&str> = tags.split(',').map(|s| s.trim()).collect();
        if tags.contains(&"mirror") || tags.contains(&"reference") || tags.contains(&"third-party")
        {
            SyncPolicy::Mirror
        } else if tags.contains(&"collaborative") || tags.contains(&"team") {
            SyncPolicy::Merge
        } else if tags.contains(&"own-project")
            || tags.contains(&"tool")
            || tags.contains(&"active")
        {
            SyncPolicy::Rebase
        } else {
            // Default: conservative for unknown tags
            SyncPolicy::Conservative
        }
    }

    pub fn can_push(&self) -> bool {
        matches!(self, SyncPolicy::Rebase | SyncPolicy::Merge)
    }

    pub fn can_rebase(&self) -> bool {
        matches!(self, SyncPolicy::Rebase)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncSafety {
    Safe,
    BlockedDirty,
    BlockedDiverged,
    NoUpstream,
    UpToDate,
    LocalAhead,
    Unknown,
}

/// Pre-flight safety assessment. Returns (safety, ahead, behind).
pub fn assess_safety(path: &str, policy: SyncPolicy) -> (SyncSafety, usize, usize) {
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(_) => return (SyncSafety::Unknown, 0, 0),
    };

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false);
    let dirty = match repo.statuses(Some(&mut opts)) {
        Ok(statuses) => statuses.iter().any(|entry| {
            let s = entry.status();
            // Only care about tracked-file changes that would block merge/rebase.
            // Untracked files (WT_NEW) are ignored — they don't block ff-merge.
            s.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE
                    | git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED
                    | git2::Status::WT_RENAMED
                    | git2::Status::WT_TYPECHANGE
                    | git2::Status::CONFLICTED,
            )
        }),
        Err(_) => false,
    };
    if dirty {
        return (SyncSafety::BlockedDirty, 0, 0);
    }

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return (SyncSafety::Unknown, 0, 0),
    };
    let local_oid = match head.target() {
        Some(o) => o,
        None => return (SyncSafety::Unknown, 0, 0),
    };
    let branch = match head.shorthand() {
        Some(b) => b,
        None => return (SyncSafety::Unknown, 0, 0),
    };

    let remote_oid = match repo.revparse_single(&format!("refs/remotes/origin/{}", branch)) {
        Ok(obj) => obj.id(),
        Err(_) => return (SyncSafety::NoUpstream, 0, 0),
    };

    if local_oid == remote_oid {
        return (SyncSafety::UpToDate, 0, 0);
    }

    let (ahead, behind) = match repo.graph_ahead_behind(local_oid, remote_oid) {
        Ok(ab) => ab,
        Err(_) => return (SyncSafety::Unknown, 0, 0),
    };

    let safety = if ahead > 0 && behind > 0 {
        // Diverged: only safe if policy allows rebase or merge
        match policy {
            SyncPolicy::Mirror | SyncPolicy::Conservative => SyncSafety::BlockedDiverged,
            SyncPolicy::Rebase | SyncPolicy::Merge => SyncSafety::Safe,
        }
    } else if behind > 0 && ahead == 0 {
        // Local behind remote: always safe to fast-forward
        SyncSafety::Safe
    } else {
        // ahead > 0, behind == 0: local ahead of remote
        if policy.can_push() {
            SyncSafety::LocalAhead
        } else {
            SyncSafety::UpToDate
        }
    };
    (safety, ahead, behind)
}

pub fn recommend_sync_action(
    safety: SyncSafety,
    ahead: usize,
    behind: usize,
    _policy: SyncPolicy,
    has_upstream: bool,
) -> Option<String> {
    if !has_upstream {
        return Some("No remote — cannot sync".to_string());
    }
    match safety {
        SyncSafety::BlockedDirty => {
            Some("Working tree dirty — commit or stash before sync".to_string())
        }
        SyncSafety::BlockedDiverged => Some(format!(
            "Diverged ({} ahead, {} behind) — switch to Rebase/Merge policy",
            ahead, behind
        )),
        SyncSafety::Safe if behind > 0 && ahead == 0 => {
            Some(format!("Safe to fast-forward {} commit(s)", behind))
        }
        SyncSafety::Safe if behind > 0 && ahead > 0 => {
            Some(format!("Can rebase/merge {} commit(s)", behind))
        }
        SyncSafety::LocalAhead if ahead > 0 => {
            Some(format!("Local ahead by {} — ready to push", ahead))
        }
        SyncSafety::UpToDate => Some("Up to date — nothing to do".to_string()),
        _ => None,
    }
}
