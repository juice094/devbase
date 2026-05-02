use crate::asyncgit::AsyncNotification;
use crate::tui::{App, RepoItem, SortMode};
use chrono::Utc;

impl App {
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
                    b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)).then_with(|| a.id.cmp(&b.id))
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

    pub(crate) fn spawn_stars_refresh(&mut self) {
        let repos: Vec<(String, Option<String>)> = self
            .repos
            .iter()
            .filter(|r| {
                r.upstream_url.as_deref().map(|u| u.contains("github.com")).unwrap_or(false)
            })
            .map(|r| (r.id.clone(), r.upstream_url.clone()))
            .collect();
        if repos.is_empty() {
            return;
        }
        let tx = self.async_tx.clone();
        let github = self.ctx.config.github.clone();
        let ttl = self.ctx.config.cache.ttl_seconds;

        let pool = self.ctx.pool();
        tokio::spawn(async move {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(_) => return,
            };
            // Phase 1: check cache serially (conn is not Send)
            let mut needs_fetch = Vec::new();
            for (repo_id, upstream_url) in repos {
                let cache_hit = match crate::registry::health::get_stars_cache(&conn, &repo_id) {
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
                if !cache_hit && let Some(url) = upstream_url {
                    needs_fetch.push((repo_id, url));
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
                    let _ = crate::registry::health::save_stars_cache(&conn, &repo_id, s);
                }
                let _ = tx.send(AsyncNotification::StarsUpdated { repo_id, stars });
            }
        });
    }

    pub(crate) fn spawn_repo_status_for_current(&mut self) {
        let repo = self.current_repo().cloned();
        if let Some(repo) = repo
            && repo.status_dirty.is_none()
        {
            let id = repo.id.clone();
            self.loading_repo_status.insert(id);
            self.repo_status_job.spawn(crate::asyncgit::AsyncRepoStatus {
                repo_id: repo.id,
                local_path: repo.local_path,
            });
        }
    }

    pub(crate) fn current_repo(&self) -> Option<&RepoItem> {
        self.repos.get(self.selected)
    }

    pub(crate) fn generate_insights(&self, repo: &RepoItem) -> Vec<String> {
        let mut insights = vec![];

        // 1. 未同步检查
        if let (Some(ahead), Some(behind)) = (repo.status_ahead, repo.status_behind) {
            if ahead > 0 && behind > 0 {
                insights
                    .push("⚠️ Local and remote have diverged — needs manual review".to_string());
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
        if let Ok(conn) = self.ctx.conn()
            && let Ok(history) = crate::registry::health::get_stars_history(&conn, &repo.id, 7)
            && history.len() >= 2
        {
            let first = history.first().map(|(s, _)| *s).unwrap_or(0);
            let last = history.last().map(|(s, _)| *s).unwrap_or(0);
            let delta = last as i64 - first as i64;
            if delta > 0 {
                insights.push(format!("⭐ Stars gained {} this week", delta));
            } else if delta < 0 {
                insights.push(format!("⭐ Stars lost {} this week", delta.abs()));
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
