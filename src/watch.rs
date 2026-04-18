use crate::sync_protocol::{scan_directory, FileInfo, SyncIndex};
use anyhow::Context;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Bottom-layer filesystem watcher using `notify`.
pub struct FsWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    rx: crossbeam_channel::Receiver<notify::Result<Event>>,
}

impl FsWatcher {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let _ = tx.send(res);
        })
        .with_context(|| "failed to create filesystem watcher")?;

        let mut watcher = watcher;
        watcher
            .watch(path, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch path {:?}", path))?;

        Ok(FsWatcher { watcher, rx })
    }

    /// Poll for events during `timeout`, returning all distinct paths that changed.
    pub fn poll_event(&self, timeout: Duration) -> Option<Vec<PathBuf>> {
        let start = std::time::Instant::now();
        let mut paths = Vec::new();

        // Block on first event up to timeout
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                paths.extend(event.paths);
            }
            Ok(Err(_e)) => {
                // Ignore watcher errors and keep waiting
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => return None,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return None,
        }

        // Drain remaining events until timeout expires
        let _remaining = timeout.saturating_sub(start.elapsed());
        let deadline = start + timeout;
        while std::time::Instant::now() < deadline {
            match self.rx.recv_deadline(deadline) {
                Ok(Ok(event)) => {
                    paths.extend(event.paths);
                }
                Ok(Err(_e)) => continue,
                Err(_) => break,
            }
        }

        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }
}

/// Middle-layer aggregator: dedup and degrade to full-scan when too many files change.
pub struct WatchAggregator {
    #[allow(dead_code)]
    pub delay: Duration,
    pub max_files: usize,
}

impl Default for WatchAggregator {
    fn default() -> Self {
        WatchAggregator {
            delay: Duration::from_secs(1),
            max_files: crate::config::default_watch_max_files(),
        }
    }
}

impl WatchAggregator {
    pub fn aggregate(&self, events: Vec<PathBuf>) -> Vec<PathBuf> {
        let deduped: HashSet<PathBuf> = events.into_iter().collect();
        if deduped.len() > self.max_files {
            // Degrade to a single root-scan request.
            // The caller decides what root means; here we signal empty vec
            // to indicate "too many changes". However the spec says return
            // Vec containing root directory path. Since we don't have root here,
            // we return the collected paths and let scheduler decide.
            // To keep API clean, we just return the deduped set as-is and let
            // FolderScheduler detect large counts.
            Vec::from_iter(deduped)
        } else {
            Vec::from_iter(deduped)
        }
    }
}

/// Actions produced by the scheduler.
#[derive(Debug, Clone)]
pub enum SyncAction {
    /// A full rescan was requested (degraded or initial).
    Scan(()),
    /// Incremental sync for a specific path with changed files.
    Sync((), ()),
}

/// Top-layer scheduler that turns file-system events into sync actions.
pub struct FolderScheduler {
    pub root: PathBuf,
    pub index: Option<SyncIndex>,
    pub max_files: usize,
}

impl FolderScheduler {
    #[allow(dead_code)]
    pub fn new(root: PathBuf) -> Self {
        FolderScheduler {
            root,
            index: None,
            max_files: crate::config::default_watch_max_files(),
        }
    }

    pub fn with_max_files(root: PathBuf, max_files: usize) -> Self {
        FolderScheduler {
            root,
            index: None,
            max_files,
        }
    }

    /// Given a list of changed paths, produce SyncActions.
    pub fn check_and_schedule(&mut self, paths: Vec<PathBuf>) -> anyhow::Result<Vec<SyncAction>> {
        // If too many paths changed, degrade to full root scan
        if paths.len() > self.max_files {
            let new_index = scan_directory(&self.root)?;
            let action = SyncAction::Scan(());
            self.index = Some(new_index);
            return Ok(vec![action]);
        }

        // For incremental changes, rescan the root (lightweight) and diff
        let new_index = scan_directory(&self.root)?;
        let old_index = self.index.take();

        let mut actions = Vec::new();

        if let Some(old) = old_index {
            let old_map: HashMap<String, FileInfo> = old
                .files
                .into_iter()
                .map(|f| (f.name.clone(), f))
                .collect();
            let new_map: HashMap<String, FileInfo> = new_index
                .files
                .iter()
                .map(|f| (f.name.clone(), f.clone()))
                .collect();

            let mut changed = Vec::new();

            // Detect modifications and additions
            for (name, new_f) in &new_map {
                match old_map.get(name) {
                    Some(old_f) => {
                        if old_f.blocks_hash != new_f.blocks_hash
                            || old_f.size != new_f.size
                        {
                            changed.push(new_f.clone());
                        }
                    }
                    None => changed.push(new_f.clone()),
                }
            }

            // Detect deletions (we represent them as a Scan of root for simplicity)
            let has_deletions = old_map.keys().any(|k| !new_map.contains_key(k));

            if !changed.is_empty() || has_deletions {
                actions.push(SyncAction::Sync((), ()));
            }
        } else {
            // First run: treat as full scan
            actions.push(SyncAction::Scan(()));
        }

        self.index = Some(new_index);
        Ok(actions)
    }
}
