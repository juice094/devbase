use crate::asyncgit::AsyncNotification;
use crate::registry::WorkspaceRegistry;
use crate::tui::{App, InputMode, ListState, RepoItem, SortMode, SyncPopupMode, SyncPreviewItem, SearchPopupMode, SearchResult};
use crossbeam_channel::bounded;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use chrono::Utc;

impl App {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let (async_tx, async_rx) = bounded::<AsyncNotification>(100);
        let repo_status_job = crate::asyncgit::AsyncSingleJob::new(async_tx.clone());

        let config = crate::config::Config::load().unwrap_or_default();
        let timeout_secs = config.sync.timeout_seconds;
        let mut app = Self {
            repos: Vec::new(),
            selected: 0,
            logs: Vec::new(),
            show_help: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            list_state: ListState::default(),
            async_rx,
            async_tx: async_tx.clone(),
            repo_status_job,
            loading_repo_status: HashSet::new(),
            loading_sync: HashSet::new(),
            sync_orchestrator: crate::sync::SyncOrchestrator::new(config.sync.concurrency.max(1)),
            sync_popup_mode: SyncPopupMode::Hidden,
            sync_preview_items: Vec::new(),
            sync_popup_results: Vec::new(),
            sync_total: 0,
            sync_start_time: None,
            sync_running: HashSet::new(),
            sync_timeout: Duration::from_secs(timeout_secs),
            sort_mode: SortMode::Status,
            config,
            dry_run: false,
            search_popup_mode: SearchPopupMode::Hidden,
            search_results: Vec::new(),
            search_selected: 0,
            search_pattern: String::new(),
        };
        app.log_info(crate::i18n::current().log.tui_started.to_string());
        app.load_repos()?;
        app.spawn_stars_refresh();
        Ok(app)
    }

    pub(crate) fn load_repos(&mut self) -> anyhow::Result<()> {
        let conn = WorkspaceRegistry::init_db()?;
        let repos = WorkspaceRegistry::list_repos(&conn)?;

        self.repos.clear();
        for repo in repos {
            let primary = repo.primary_remote().cloned();
            self.repos.push(RepoItem {
                id: repo.id,
                local_path: repo.local_path.to_string_lossy().to_string(),
                upstream_url: primary.as_ref().and_then(|r| r.upstream_url.clone()),
                default_branch: primary.as_ref().and_then(|r| r.default_branch.clone()),
                tags: repo.tags.clone(),
                language: repo.language,
                status_dirty: None,
                status_ahead: None,
                status_behind: None,
                stars: repo.stars,
            });
        }
        // Mark all repos as loading status
        self.loading_repo_status.clear();
        for repo in &self.repos {
            self.loading_repo_status.insert(repo.id.clone());
        }

        // Batch async safety assessment
        for repo in &self.repos {
            let path = repo.local_path.clone();
            let id = repo.id.clone();
            let tags = repo.tags.join(",");
            let tx = self.async_tx.clone();
            std::thread::spawn(move || {
                let policy = crate::sync::SyncPolicy::from_tags(&tags);
                let (safety, ahead, behind) = crate::sync::assess_safety(&path, policy);
                let dirty = safety == crate::sync::SyncSafety::BlockedDirty;
                let _ = tx.send(crate::asyncgit::AsyncNotification::RepoStatus(
                    crate::asyncgit::RepoStatusNotification { repo_id: id, dirty, ahead, behind }
                ));
            });
        }

        // Initial sort by registry data only (tags alphabetical)
        self.sort_repos_by_registry();
        self.log_info(crate::i18n::current().log.loaded_repos(self.repos.len()));
        Ok(())
    }

    pub(crate) fn sort_repos_by_registry(&mut self) {
        self.repos.sort_by(|a, b| {
            let tag_a = a.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
            let tag_b = b.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
            tag_a.cmp(tag_b).then_with(|| a.id.cmp(&b.id))
        });
        self.sync_list_state();
    }

    pub(crate) fn sort_repos(&mut self) {
        match self.sort_mode {
            SortMode::Status => {
                self.repos.sort_by(|a, b| {
                    let priority = |repo: &RepoItem| -> i32 {
                        match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
                            (Some(true), _, _) => 0,
                            (Some(false), Some(a), Some(b)) if a > 0 && b > 0 => 1,
                            (Some(false), _, Some(b)) if b > 0 => 2,
                            (Some(false), Some(a), _) if a > 0 => 3,
                            _ => 4,
                        }
                    };
                    let pa = priority(a);
                    let pb = priority(b);
                    pa.cmp(&pb)
                        .then_with(|| {
                            let tag_a = a.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
                            let tag_b = b.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
                            tag_a.cmp(tag_b)
                        })
                        .then_with(|| a.id.cmp(&b.id))
                });
            }
            SortMode::Stars => {
                self.repos.sort_by(|a, b| {
                    b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0))
                        .then_with(|| a.id.cmp(&b.id))
                });
            }
        }
        self.sync_list_state();
    }

    fn sync_list_state(&mut self) {
        if self.selected >= self.repos.len() && !self.repos.is_empty() {
            self.selected = self.repos.len() - 1;
        }
        self.list_state.select(Some(self.selected));
    }

    pub(crate) fn log_info(&mut self, msg: String) {
        self.log_with_level("INFO", msg);
    }

    pub(crate) fn log_warn(&mut self, msg: String) {
        self.log_with_level("WARN", msg);
    }

    pub(crate) fn log_error(&mut self, msg: String) {
        self.log_with_level("ERROR", msg);
    }

    fn log_with_level(&mut self, level: &str, msg: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(format!("[{}] [{}] {}", time, level, msg));
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    pub(crate) fn spawn_stars_refresh(&mut self) {
        let repos: Vec<(String, Option<String>)> = self
            .repos
            .iter()
            .filter(|r| {
                r.upstream_url
                    .as_deref()
                    .map(|u| u.contains("github.com"))
                    .unwrap_or(false)
            })
            .map(|r| (r.id.clone(), r.upstream_url.clone()))
            .collect();
        if repos.is_empty() {
            return;
        }
        let tx = self.async_tx.clone();
        let github = self.config.github.clone();
        let ttl = self.config.cache.ttl_seconds;

        tokio::spawn(async move {
            let conn = match crate::registry::WorkspaceRegistry::init_db() {
                Ok(c) => c,
                Err(_) => return,
            };
            // Phase 1: check cache serially (conn is not Send)
            let mut needs_fetch = Vec::new();
            for (repo_id, upstream_url) in repos {
                let cache_hit = match crate::registry::WorkspaceRegistry::get_stars_cache(&conn, &repo_id) {
                    Ok(Some((stars, fetched_at))) => {
                        let elapsed = Utc::now().signed_duration_since(fetched_at).num_seconds();
                        if elapsed < ttl {
                            let _ = tx.send(AsyncNotification::StarsUpdated {
                                repo_id: repo_id.clone(),
                                stars: Some(stars),
                            });
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                };
                if !cache_hit {
                    if let Some(url) = upstream_url {
                        needs_fetch.push((repo_id, url));
                    }
                }
            }
            if needs_fetch.is_empty() {
                return;
            }
            // Phase 2: fetch concurrently with max 4 parallelism
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let mut handles = Vec::new();
            for (repo_id, url) in needs_fetch {
                let gh = github.clone();
                let permit = semaphore.clone().acquire_owned().await.ok();
                let handle = tokio::spawn(async move {
                    let _permit = permit;
                    tokio::task::spawn_blocking(move || {
                        crate::scan::fetch_github_stars(&url, Some(&gh))
                    })
                    .await
                    .ok()
                    .flatten()
                });
                handles.push((repo_id, handle));
            }
            // Phase 3: write back serially
            for (repo_id, handle) in handles {
                let stars = handle.await.ok().flatten();
                if let Some(s) = stars {
                    let _ = crate::registry::WorkspaceRegistry::save_stars_cache(&conn, &repo_id, s);
                }
                let _ = tx.send(AsyncNotification::StarsUpdated {
                    repo_id,
                    stars,
                });
            }
        });
    }

    pub(crate) fn spawn_repo_status_for_current(&mut self) {
        let repo = self.current_repo().cloned();
        if let Some(repo) = repo {
            if repo.status_dirty.is_none() {
                let id = repo.id.clone();
                self.loading_repo_status.insert(id);
                self.repo_status_job.spawn(crate::asyncgit::AsyncRepoStatus {
                    repo_id: repo.id,
                    local_path: repo.local_path,
                });
            }
        }
    }

    pub(crate) fn next(&mut self) {
        if !self.repos.is_empty() {
            self.selected = (self.selected + 1) % self.repos.len();
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    pub(crate) fn previous(&mut self) {
        if !self.repos.is_empty() {
            self.selected = (self.selected + self.repos.len() - 1) % self.repos.len();
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    pub(crate) fn jump_to_top(&mut self) {
        if !self.repos.is_empty() {
            self.selected = 0;
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    pub(crate) fn jump_to_bottom(&mut self) {
        if !self.repos.is_empty() {
            self.selected = self.repos.len() - 1;
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    pub(crate) fn current_repo(&self) -> Option<&RepoItem> {
        self.repos.get(self.selected)
    }

    pub(crate) fn update_async(&mut self, notification: AsyncNotification) {
        match notification {
            AsyncNotification::RepoStatus(n) => {
                self.loading_repo_status.remove(&n.repo_id);
                if let Some(repo) = self.repos.iter_mut().find(|r| r.id == n.repo_id) {
                    repo.status_dirty = Some(n.dirty);
                    repo.status_ahead = Some(n.ahead);
                    repo.status_behind = Some(n.behind);
                }
                self.log_info(crate::i18n::current().log.status_fmt(
                    &n.repo_id, n.dirty, n.ahead, n.behind
                ));
                // Trigger re-sort when all statuses are loaded
                if self.loading_repo_status.is_empty() && self.sort_mode == SortMode::Status {
                    self.sort_repos();
                }
            }
            AsyncNotification::SyncProgress(n) => {
                if n.repo_id == "__fetch_all_done__" && n.action == "FETCH_ALL_DONE" {
                    self.safe_sync_preview();
                    return;
                }
                if n.action == "RUNNING" || n.action == "FETCHING" {
                    self.loading_sync.remove(&n.repo_id);
                    self.sync_running.insert(n.repo_id.clone());
                } else {
                    self.sync_running.remove(&n.repo_id);
                }
                if let Some(entry) = self.sync_popup_results.iter_mut().find(|(id, _)| id == &n.repo_id) {
                    entry.1 = n.message.clone();
                } else {
                    self.sync_popup_results.push((n.repo_id.clone(), n.message.clone()));
                }
                self.log_info(crate::i18n::current().log.sync_progress_fmt(
                    &n.repo_id, &n.action, &n.message
                ));
            }
            AsyncNotification::StarsUpdated { repo_id, stars } => {
                if let Some(repo) = self.repos.iter_mut().find(|r| r.id == repo_id) {
                    repo.stars = stars;
                }
                if let Ok(conn) = crate::registry::WorkspaceRegistry::init_db() {
                    if let Some(s) = stars {
                        let _ = crate::registry::WorkspaceRegistry::save_stars_cache(&conn, &repo_id, s);
                    }
                }
                // Re-sort if currently sorting by stars
                if self.sort_mode == SortMode::Stars {
                    self.repos.sort_by(|a, b| {
                        b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0))
                            .then_with(|| a.id.cmp(&b.id))
                    });
                }
            }
        }
    }

    pub(crate) fn update_tags(&mut self, new_tags: &str) {
        let repo_id = match self.current_repo() {
            Some(r) => r.id.clone(),
            None => {
                self.log_warn(crate::i18n::current().log.no_repo_selected.to_string());
                return;
            }
        };

        match (|| -> anyhow::Result<()> {
            let mut conn = WorkspaceRegistry::init_db()?;
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1", [&repo_id])?;
            for tag in new_tags.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                tx.execute(
                    "INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                    rusqlite::params![&repo_id, tag],
                )?;
            }
            tx.commit()?;
            Ok(())
        })() {
            Ok(()) => {
                self.log_info(crate::i18n::current().log.updated_tags(&repo_id, new_tags));
                if let Err(e) = self.load_repos() {
                    self.log_error(crate::i18n::current().log.reload_repos_failed(e));
                }
            }
            Err(e) => self.log_error(crate::i18n::current().log.update_tags_failed(e)),
        }
    }

    pub(crate) fn safe_sync_preview(&mut self) {
        self.dry_run = true;
        self.sync_popup_mode = SyncPopupMode::Preview;
        self.sync_preview_items.clear();
        self.sync_popup_results.clear();
        self.sync_total = 0;
        self.sync_start_time = None;
        self.sync_running.clear();

        for repo in &self.repos {
            if repo.upstream_url.is_none() {
                continue;
            }
            let policy = crate::sync::SyncPolicy::from_tags(&repo.tags.join(","));
            let (safety, ahead, behind) = crate::sync::assess_safety(&repo.local_path, policy);
            let recommendation = crate::sync::recommend_sync_action(
                safety, ahead, behind, policy, repo.upstream_url.is_some()
            );
            self.sync_preview_items.push(SyncPreviewItem {
                repo_id: repo.id.clone(),
                safety,
                policy,
                ahead,
                behind,
                recommendation,
            });
        }

        if self.sync_preview_items.is_empty() {
            self.sync_popup_results.push(("system".to_string(), "No repositories eligible for safe sync.".to_string()));
            self.sync_popup_mode = SyncPopupMode::Progress;
        }
    }

    pub(crate) fn fetch_all_and_preview(&mut self) {
        self.dry_run = true;
        let tasks: Vec<_> = self.repos.iter()
            .filter(|r| r.upstream_url.is_some())
            .map(|repo| {
                let policy = crate::sync::SyncPolicy::from_tags(&repo.tags.join(","));
                crate::sync::RepoSyncTask {
                    id: repo.id.clone(),
                    path: repo.local_path.clone(),
                    upstream_url: repo.upstream_url.clone(),
                    default_branch: repo.default_branch.clone(),
                    policy,
                }
            })
            .collect();

        if tasks.is_empty() {
            self.log_info("No repositories with upstream to fetch.".to_string());
            return;
        }

        self.sync_popup_mode = SyncPopupMode::Preview;
        self.sync_preview_items.clear();
        self.sync_popup_results.clear();
        self.sync_total = tasks.len();
        self.sync_start_time = Some(Instant::now());
        self.sync_running.clear();
        self.loading_sync.clear();

        for t in &tasks {
            self.sync_popup_results.push((t.id.clone(), crate::i18n::current().log.status_queued.to_string()));
        }

        let sender = self.async_tx.clone();
        let orchestrator = self.sync_orchestrator.clone();
        let timeout = self.sync_timeout;

        tokio::spawn(async move {
            orchestrator
                .run_fetch_all(tasks, timeout, |id, summary| {
                    let _ = sender.send(AsyncNotification::SyncProgress(
                        crate::asyncgit::SyncProgressNotification {
                            repo_id: id,
                            action: summary.action,
                            message: summary.message,
                        },
                    ));
                })
                .await;
            // Signal completion to trigger preview refresh
            let _ = sender.send(AsyncNotification::SyncProgress(
                crate::asyncgit::SyncProgressNotification {
                    repo_id: "__fetch_all_done__".to_string(),
                    action: "FETCH_ALL_DONE".to_string(),
                    message: String::new(),
                },
            ));
        });
    }

    pub(crate) fn execute_search(&mut self) {
        self.search_results.clear();
        self.search_selected = 0;

        let repo_paths: Vec<(String, String)> = self.repos.iter()
            .map(|r| (r.id.clone(), r.local_path.clone()))
            .collect();

        let pattern = self.search_pattern.clone();

        for (repo_id, path) in repo_paths {
            if which::which("rg").is_ok() {
                if let Ok(output) = std::process::Command::new("rg")
                    .args(&["-n", "--no-heading", "--with-filename", "-C", "1", &pattern, &path])
                    .output()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        let parts: Vec<&str> = line.splitn(3, ':').collect();
                        if parts.len() >= 3 {
                            if let Ok(line_num) = parts[1].parse::<usize>() {
                                self.search_results.push(SearchResult {
                                    repo_id: repo_id.clone(),
                                    file_path: parts[0].to_string(),
                                    line_number: line_num,
                                    line_content: parts[2].to_string(),
                                });
                            }
                        }
                    }
                }
            } else {
                search_repo_fallback(&path, &pattern, &repo_id, &mut self.search_results);
            }

            if self.search_results.len() >= 200 {
                break;
            }
        }

        if self.search_results.len() > 200 {
            self.search_results.truncate(200);
            self.log_info("Search truncated to 200 results".to_string());
        }
    }

    pub(crate) fn start_safe_sync(&mut self) {
        self.dry_run = false;
        // If preview is stale/empty, regenerate it on-the-fly
        if self.sync_preview_items.is_empty() {
            self.safe_sync_preview();
        }

        let safe_items: Vec<crate::sync::RepoSyncTask> = self
            .sync_preview_items
            .iter()
            .filter(|item| item.safety == crate::sync::SyncSafety::Safe)
            .filter_map(|item| {
                self.repos.iter().find(|r| r.id == item.repo_id).map(|repo| {
                    let tags = repo.tags.join(",");
                    let policy = crate::sync::SyncPolicy::from_tags(&tags);
                    crate::sync::RepoSyncTask {
                        id: repo.id.clone(),
                        path: repo.local_path.clone(),
                        upstream_url: repo.upstream_url.clone(),
                        default_branch: repo.default_branch.clone(),
                        policy,
                    }
                })
            })
            .collect();

        self.sync_popup_mode = SyncPopupMode::Progress;
        self.sync_popup_results.clear();
        self.sync_total = safe_items.len();
        self.sync_start_time = Some(Instant::now());
        self.sync_running.clear();

        if safe_items.is_empty() {
            self.sync_popup_results.push(("system".to_string(), crate::i18n::current().sync.no_safe_repos.to_string()));
            return;
        }

        self.log_info(crate::i18n::current().log.batch_syncing(safe_items.len()));
        for r in &safe_items {
            self.loading_sync.insert(r.id.clone());
            self.sync_popup_results.push((r.id.clone(), crate::i18n::current().log.status_queued.to_string()));
        }

        let sender = self.async_tx.clone();
        let orchestrator = self.sync_orchestrator.clone();
        let timeout = self.sync_timeout;
        tokio::spawn(async move {
            orchestrator
                .run_sync(
                    safe_items,
                    crate::sync::SyncMode::BlockUi,
                    false,
                    timeout,
                    |id, summary| {
                        let _ = sender.send(AsyncNotification::SyncProgress(
                            crate::asyncgit::SyncProgressNotification {
                                repo_id: id,
                                action: summary.action,
                                message: summary.message,
                            },
                        ));
                    },
                )
                .await;
        });
    }

    pub(crate) fn generate_insights(&self, repo: &RepoItem) -> Vec<String> {
        let mut insights = vec![];

        // 1. 未同步检查
        if let (Some(ahead), Some(behind)) = (repo.status_ahead, repo.status_behind) {
            if ahead > 0 && behind > 0 {
                insights.push("⚠️ Local and remote have diverged — needs manual review".to_string());
            } else if behind > 0 {
                insights.push(format!("📥 Behind remote by {} commits — consider syncing", behind));
            } else if ahead > 0 {
                insights.push(format!("📤 Ahead of remote by {} commits — ready to push", ahead));
            }
        }

        // 2. Dirty 检查
        if repo.status_dirty == Some(true) {
            insights.push("📝 Working tree has uncommitted changes".to_string());
        }

        // 3. 无远程检查
        if repo.upstream_url.is_none() {
            insights.push("🔗 No upstream remote — local-only repository".to_string());
        }

        // 4. Stars 检查（如果有历史数据）
        if let Ok(conn) = crate::registry::WorkspaceRegistry::init_db() {
            if let Ok(history) = crate::registry::WorkspaceRegistry::get_stars_history(&conn, &repo.id, 7) {
                if history.len() >= 2 {
                    let first = history.first().map(|(s, _)| *s).unwrap_or(0);
                    let last = history.last().map(|(s, _)| *s).unwrap_or(0);
                    let delta = last as i64 - first as i64;
                    if delta > 0 {
                        insights.push(format!("⭐ Stars gained {} this week", delta));
                    } else if delta < 0 {
                        insights.push(format!("⭐ Stars lost {} this week", delta.abs()));
                    }
                }
            }
        }

        // 5. 策略检查
        let policy = crate::sync::SyncPolicy::from_tags(&repo.tags.join(","));
        if matches!(policy, crate::sync::SyncPolicy::Mirror) {
            insights.push("🛡️ Mirror policy — sync will never modify local branches".to_string());
        }

        insights
    }
}

fn search_repo_fallback(repo_path: &str, pattern: &str, repo_id: &str, results: &mut Vec<SearchResult>) {
    use std::collections::HashSet;

    let excluded_dirs: HashSet<&str> = [".git", "target", "node_modules", ".venv", "venv", "dist", "build"].iter().cloned().collect();

    for entry in walkdir::WalkDir::new(repo_path).max_depth(10) {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        if !entry.file_type().is_file() { continue; }

        let path = entry.path();
        if path.components().any(|c| {
            if let Some(name) = c.as_os_str().to_str() {
                excluded_dirs.contains(name)
            } else { false }
        }) { continue; }

        if let Ok(content) = std::fs::read(path) {
            if content.len() > 8 * 1024 * 1024 { continue; }
            if content.contains(&0) { continue; }

            let path_str = path.to_string_lossy().to_string();
            for (line_num, line) in content.split(|&b| b == b'\n').enumerate() {
                let line_str = String::from_utf8_lossy(line);
                if line_str.contains(pattern) {
                    results.push(SearchResult {
                        repo_id: repo_id.to_string(),
                        file_path: path_str.clone(),
                        line_number: line_num + 1,
                        line_content: line_str.to_string(),
                    });
                    if results.len() >= 200 { return; }
                }
            }
        }
    }
}
