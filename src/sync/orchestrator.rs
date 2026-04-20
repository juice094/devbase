use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{Duration, timeout};

use super::SyncSummary;
use super::policy::{RepoSyncTask, SyncMode};
use super::tasks::{execute_task, fetch_single_repo};

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
        timeout_duration: Duration,
        mut on_progress: impl FnMut(String, SyncSummary) + Send,
    ) -> Vec<(String, SyncSummary)> {
        match mode {
            SyncMode::Sync => {
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
                    let summary =
                        match timeout(timeout_duration, execute_task(&task, dry_run)).await {
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
            SyncMode::Async | SyncMode::BlockUi => {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, SyncSummary)>();
                let total = repos.len();
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
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let summary = match timeout(timeout_duration, execute_task(&task, dry_run))
                            .await
                        {
                            Ok(s) => s,
                            Err(_) => SyncSummary {
                                action: "TIMEOUT".to_string(),
                                message: crate::i18n::current().sync.network_timeout.to_string(),
                                ..Default::default()
                            },
                        };
                        let _ = tx.send((task.id, summary));
                        drop(permit);
                    });
                }

                drop(tx);

                let mut results = Vec::with_capacity(total);
                while let Some((id, summary)) = rx.recv().await {
                    on_progress(id.clone(), summary.clone());
                    results.push((id, summary));
                }
                results
            }
        }
    }

    pub async fn run_fetch_all(
        &self,
        repos: Vec<RepoSyncTask>,
        _timeout_duration: Duration,
        mut on_progress: impl FnMut(String, SyncSummary) + Send,
    ) -> Vec<(String, SyncSummary)> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, SyncSummary)>();
        let total = repos.len();

        for task in repos {
            on_progress(
                task.id.clone(),
                SyncSummary {
                    action: "FETCHING".to_string(),
                    message: format!("Fetching {}", task.id),
                    ..Default::default()
                },
            );
            let permit = self
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("semaphore should not be closed");
            let path = task.path.clone();
            let upstream = task.upstream_url.clone();
            let id = task.id.clone();
            let tx = tx.clone();

            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let summary = match fetch_single_repo(&path, upstream.as_deref()) {
                    Ok(()) => SyncSummary {
                        action: "FETCHED".to_string(),
                        message: format!("Fetched {}", id),
                        ..Default::default()
                    },
                    Err(e) => SyncSummary {
                        action: "ERROR".to_string(),
                        message: e.to_string(),
                        ..Default::default()
                    },
                };
                let _ = tx.send((id, summary));
            });
        }

        drop(tx);

        let mut results = Vec::with_capacity(total);
        while let Some((id, summary)) = rx.recv().await {
            on_progress(id.clone(), summary.clone());
            results.push((id, summary));
        }
        results
    }
}
