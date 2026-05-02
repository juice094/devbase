use crate::asyncgit::AsyncNotification;
use crate::tui::{App, SearchResult, SyncPopupMode, SyncPreviewItem};
use std::time::Instant;

impl App {
    pub(crate) fn toggle_search_mode(&mut self) {
        self.search_mode = match self.search_mode {
            crate::tui::SearchMode::Repo => crate::tui::SearchMode::Code,
            crate::tui::SearchMode::Code => crate::tui::SearchMode::Repo,
        };
    }

    pub(crate) fn run_nlp_query(&mut self, query: String) {
        self.nlp_query = query.clone();
        self.log_info(format!("NLQ: '{}' ...", query));
        let tx = self.async_tx.clone();
        let pool = self.ctx.pool();
        std::thread::spawn(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::NLPQueryFinished {
                        query,
                        skills: vec![],
                        error: Some(e.to_string()),
                    });
                    return;
                }
            };
            // Try semantic search first, fallback to text search if embedding unavailable
            let (skills, fallback) = match crate::embedding::generate_query_embedding(&query) {
                Ok(embedding) => {
                    match crate::skill_runtime::registry::search_skills_semantic(
                        &conn, &embedding, 10, None,
                    ) {
                        Ok(s) => (s, false),
                        Err(_) => {
                            match crate::skill_runtime::registry::search_skills_text(
                                &conn, &query, 10, None,
                            ) {
                                Ok(s) => (s, true),
                                Err(e) => {
                                    let _ = tx.send(
                                        crate::asyncgit::AsyncNotification::NLPQueryFinished {
                                            query,
                                            skills: vec![],
                                            error: Some(e.to_string()),
                                        },
                                    );
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    match crate::skill_runtime::registry::search_skills_text(
                        &conn, &query, 10, None,
                    ) {
                        Ok(s) => (s, true),
                        Err(e) => {
                            let _ = tx.send(crate::asyncgit::AsyncNotification::NLPQueryFinished {
                                query,
                                skills: vec![],
                                error: Some(e.to_string()),
                            });
                            return;
                        }
                    }
                }
            };
            let error = if fallback {
                Some("Semantic search unavailable; showing text search results".to_string())
            } else {
                None
            };
            let _ = tx.send(crate::asyncgit::AsyncNotification::NLPQueryFinished {
                query,
                skills,
                error,
            });
        });
    }

    pub(crate) fn execute_search(&mut self) {
        self.search_results.clear();
        self.search_selected = 0;
        let pattern = self.search_pattern.clone();

        match self.search_mode {
            crate::tui::SearchMode::Repo => {
                // Try Tantivy semantic/repo search first
                match crate::search::search_repos(&pattern, 50) {
                    Ok(results) => {
                        for (repo_id, score) in results {
                            self.search_results.push(SearchResult {
                                repo_id: repo_id.clone(),
                                file_path: format!("[score: {:.2}]", score),
                                line_number: 0,
                                line_content: repo_id,
                            });
                        }
                    }
                    Err(e) => {
                        self.log_warn(format!(
                            "Tantivy search failed, falling back to code search: {}",
                            e
                        ));
                        self.execute_code_search(&pattern);
                    }
                }
            }
            crate::tui::SearchMode::Code => {
                self.execute_code_search(&pattern);
            }
        }

        if self.search_results.len() > 200 {
            self.search_results.truncate(200);
            self.log_info("Search truncated to 200 results".to_string());
        }
    }

    fn execute_code_search(&mut self, pattern: &str) {
        let repo_paths: Vec<(String, String)> =
            self.repos.iter().map(|r| (r.id.clone(), r.local_path.clone())).collect();

        for (repo_id, path) in repo_paths {
            if which::which("rg").is_ok() {
                if let Ok(output) = std::process::Command::new("rg")
                    .args(["-n", "--no-heading", "--with-filename", "-C", "1", pattern, &path])
                    .output()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        let parts: Vec<&str> = line.splitn(3, ':').collect();
                        if parts.len() >= 3
                            && let Ok(line_num) = parts[1].parse::<usize>()
                        {
                            self.search_results.push(SearchResult {
                                repo_id: repo_id.clone(),
                                file_path: parts[0].to_string(),
                                line_number: line_num,
                                line_content: parts[2].to_string(),
                            });
                        }
                    }
                }
            } else {
                search_repo_fallback(&path, pattern, &repo_id, &mut self.search_results);
            }

            if self.search_results.len() >= 200 {
                break;
            }
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
                safety,
                ahead,
                behind,
                policy,
                repo.upstream_url.is_some(),
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
            self.sync_popup_results.push((
                "system".to_string(),
                "No repositories eligible for safe sync.".to_string(),
            ));
            self.sync_popup_mode = SyncPopupMode::Progress;
        }
    }

    pub(crate) fn fetch_all_and_preview(&mut self) {
        self.dry_run = true;
        let tasks: Vec<_> = self
            .repos
            .iter()
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
            self.sync_popup_results
                .push((t.id.clone(), self.ctx.i18n.log.status_queued.to_string()));
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
            self.sync_popup_results
                .push(("system".to_string(), self.ctx.i18n.sync.no_safe_repos.to_string()));
            return;
        }

        self.log_info(self.ctx.i18n.log.batch_syncing(safe_items.len()));
        for r in &safe_items {
            self.loading_sync.insert(r.id.clone());
            self.sync_popup_results
                .push((r.id.clone(), self.ctx.i18n.log.status_queued.to_string()));
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
}

fn search_repo_fallback(
    repo_path: &str,
    pattern: &str,
    repo_id: &str,
    results: &mut Vec<SearchResult>,
) {
    use std::collections::HashSet;

    let excluded_dirs: HashSet<&str> =
        [".git", "target", "node_modules", ".venv", "venv", "dist", "build", "__pycache__", ".cargo"]
            .iter()
            .cloned()
            .collect();

    for entry in walkdir::WalkDir::new(repo_path).max_depth(10) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path.components().any(|c| {
            if let Some(name) = c.as_os_str().to_str() {
                excluded_dirs.contains(name)
            } else {
                false
            }
        }) {
            continue;
        }

        if let Ok(content) = std::fs::read(path) {
            if content.len() > 8 * 1024 * 1024 {
                continue;
            }
            if content.contains(&0) {
                continue;
            }

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
                    if results.len() >= 200 {
                        return;
                    }
                }
            }
        }
    }
}
