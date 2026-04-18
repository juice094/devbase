use crossbeam_channel::Sender;
use git2::Repository;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct RepoStatusNotification {
    pub repo_id: String,
    pub dirty: bool,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug)]
pub struct FetchPreviewNotification {
    pub repo_id: String,
    pub msg: String,
    pub local_commit: Option<String>,
    pub remote_commit: Option<String>,
    pub branch: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub is_synced: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct SyncProgressNotification {
    pub repo_id: String,
    pub action: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum AsyncNotification {
    RepoStatus(RepoStatusNotification),
    FetchPreview(FetchPreviewNotification),
    SyncProgress(SyncProgressNotification),
}

pub trait AsyncJob: Send + Clone + 'static {
    fn run(&self) -> AsyncNotification;
}

pub struct AsyncSingleJob<J> {
    sender: Sender<AsyncNotification>,
    _phantom: PhantomData<J>,
}

impl<J> AsyncSingleJob<J> {
    pub fn new(sender: Sender<AsyncNotification>) -> Self {
        Self {
            sender,
            _phantom: PhantomData,
        }
    }

    pub fn spawn(&self, job: J)
    where
        J: AsyncJob,
    {
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let notification = job.run();
            let _ = sender.send(notification);
        });
    }
}

#[derive(Clone)]
pub struct AsyncRepoStatus {
    pub repo_id: String,
    pub local_path: String,
}

impl AsyncJob for AsyncRepoStatus {
    fn run(&self) -> AsyncNotification {
        let result = (|| -> anyhow::Result<(bool, usize, usize)> {
            let repo = Repository::open(&self.local_path)?;

            // Use default options (same as assess_safety) to include untracked files
            let statuses = repo.statuses(None)?;
            let dirty = statuses.iter().any(|entry| entry.status() != git2::Status::CURRENT);

            let (ahead, behind) = if let Ok(head) = repo.head() {
                let local_oid = head.target();
                let branch = head.shorthand();
                let upstream = head.resolve().ok().and_then(|local_ref| {
                    let name = local_ref.name()?;
                    repo.find_branch(name, git2::BranchType::Local)
                        .ok()?
                        .upstream()
                        .ok()
                });
                // Fall back to origin/{branch} if no tracking branch is set
                let upstream_oid = upstream.and_then(|up| up.get().target()).or_else(|| {
                    let b = branch?;
                    repo.revparse_single(&format!("refs/remotes/origin/{}", b)).ok().map(|o| o.id())
                });

                match (local_oid, upstream_oid) {
                    (Some(local), Some(up)) => {
                        repo.graph_ahead_behind(local, up).unwrap_or((0, 0))
                    }
                    _ => (0, 0),
                }
            } else {
                (0, 0)
            };

            Ok((dirty, ahead, behind))
        })();

        let (dirty, ahead, behind) = match result {
            Ok(v) => v,
            Err(_) => (false, 0, 0),
        };

        AsyncNotification::RepoStatus(RepoStatusNotification {
            repo_id: self.repo_id.clone(),
            dirty,
            ahead,
            behind,
        })
    }
}

#[derive(Clone)]
pub struct AsyncFetchPreview {
    pub repo_id: String,
    pub local_path: String,
    pub upstream_url: Option<String>,
    pub default_branch: Option<String>,
}

impl AsyncJob for AsyncFetchPreview {
    fn run(&self) -> AsyncNotification {
        let result = (|| -> anyhow::Result<(String, Option<String>, Option<String>, Option<String>, Option<usize>, Option<usize>, Option<bool>)> {
            let git_repo = Repository::open(&self.local_path)?;

            let upstream_url = self
                .upstream_url
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("no upstream URL configured"))?;

            let mut remote = match git_repo.find_remote("origin") {
                Ok(r) => r,
                Err(_) => {
                    git_repo.remote("origin", upstream_url)?;
                    git_repo.find_remote("origin")?
                }
            };
            if remote.url() != Some(upstream_url) {
                git_repo.remote_set_url("origin", upstream_url)?;
                remote = git_repo.find_remote("origin")?;
            }

            let mut callbacks = git2::RemoteCallbacks::new();
            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
            });
            let mut fetch_opts = git2::FetchOptions::new();
            fetch_opts.remote_callbacks(callbacks);
            remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)?;

            let branch = self
                .default_branch
                .clone()
                .or_else(|| {
                    git_repo
                        .find_remote("origin")
                        .ok()
                        .and_then(|r| r.default_branch().ok())
                        .and_then(|b| {
                            b.as_str()
                                .map(|s| s.trim_start_matches("refs/heads/").to_string())
                        })
                })
                .unwrap_or_else(|| "main".to_string());

            let local_oid = git_repo
                .revparse_single(&format!("refs/heads/{}", branch))
                .ok()
                .map(|o| o.id());
            let remote_oid = git_repo
                .revparse_single(&format!("refs/remotes/origin/{}", branch))
                .ok()
                .map(|o| o.id());

            let local_commit = local_oid.map(|o| o.to_string());
            let remote_commit = remote_oid.map(|o| o.to_string());

            match (local_oid, remote_oid) {
                (Some(local), Some(remote)) => {
                    if local == remote {
                        Ok((
                            format!("[{}] Up to date on {}", self.repo_id, branch),
                            local_commit, remote_commit,
                            Some(branch), Some(0), Some(0), Some(true),
                        ))
                    } else {
                        let (ahead, behind) = git_repo.graph_ahead_behind(local, remote)?;
                        Ok((
                            format!(
                                "[{}] {} ahead, {} behind origin/{}",
                                self.repo_id, ahead, behind, branch
                            ),
                            local_commit, remote_commit,
                            Some(branch), Some(ahead), Some(behind), Some(false),
                        ))
                    }
                }
                (None, Some(_)) => Ok((
                    format!("[{}] Local branch '{}' does not exist yet", self.repo_id, branch),
                    local_commit, remote_commit,
                    Some(branch), None, None, Some(false),
                )),
                (Some(_), None) => Ok((
                    format!("[{}] Remote branch 'origin/{}' not found after fetch", self.repo_id, branch),
                    local_commit, remote_commit,
                    Some(branch), None, None, Some(false),
                )),
                (None, None) => Ok((
                    format!("[{}] Neither local nor remote branch '{}' exists", self.repo_id, branch),
                    local_commit, remote_commit,
                    Some(branch), None, None, Some(false),
                )),
            }
        })();

        let (msg, local_commit, remote_commit, branch, ahead, behind, is_synced) = match result {
            Ok(v) => v,
            Err(e) => (format!("Sync preview failed: {}", e), None, None, None, None, None, None),
        };

        AsyncNotification::FetchPreview(FetchPreviewNotification {
            repo_id: self.repo_id.clone(),
            msg,
            local_commit,
            remote_commit,
            branch,
            ahead,
            behind,
            is_synced,
        })
    }
}
