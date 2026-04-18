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
pub struct SyncProgressNotification {
    pub repo_id: String,
    pub action: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum AsyncNotification {
    RepoStatus(RepoStatusNotification),
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

            let mut opts = git2::StatusOptions::new();
            opts.include_untracked(false);
            let statuses = repo.statuses(Some(&mut opts))?;
            let dirty = statuses.iter().any(|entry| {
                let s = entry.status();
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
            });

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


