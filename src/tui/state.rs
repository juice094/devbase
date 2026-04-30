use crate::asyncgit::AsyncNotification;
use crate::registry::repo;
use crate::tui::{
    App, InputMode, ListState, MainView, NLPPopupMode, RepoItem, SearchPopupMode, SearchResult,
    SkillItem, SkillPopupMode, SortMode, SyncPopupMode, SyncPreviewItem, VaultItem,
    WorkflowPopupMode,
};
use chrono::Utc;
use crossbeam_channel::bounded;
use std::collections::HashSet;
use std::time::{Duration, Instant};

impl App {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let (async_tx, async_rx) = bounded::<AsyncNotification>(100);
        let repo_status_job = crate::asyncgit::AsyncSingleJob::new(async_tx.clone());

        let ctx = crate::storage::AppContext::with_defaults()?;
        let timeout_secs = ctx.config.sync.timeout_seconds;
        let mut app = Self {
            repos: Vec::new(),
            selected: 0,
            logs: Vec::new(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            list_state: ListState::default(),
            async_rx,
            async_tx: async_tx.clone(),
            repo_status_job,
            loading_repo_status: HashSet::new(),
            loading_sync: HashSet::new(),
            sync_orchestrator: crate::sync::SyncOrchestrator::new(
                ctx.config.sync.concurrency.max(1),
                ctx.i18n,
            ),
            sync_popup_mode: SyncPopupMode::Hidden,
            sync_preview_items: Vec::new(),
            sync_popup_results: Vec::new(),
            sync_total: 0,
            sync_start_time: None,
            sync_running: HashSet::new(),
            sync_timeout: Duration::from_secs(timeout_secs),
            sort_mode: SortMode::Status,
            ctx,
            dry_run: false,
            search_popup_mode: SearchPopupMode::Hidden,
            search_results: Vec::new(),
            search_selected: 0,
            search_pattern: String::new(),
            detail_tab: crate::tui::DetailTab::Overview,
            help_popup_mode: crate::tui::HelpPopupMode::Hidden,
            search_mode: crate::tui::SearchMode::Code,
            main_view: crate::tui::MainView::RepoList,
            vaults: Vec::new(),
            vault_selected: 0,
            vault_list_state: ListState::default(),
            skill_panel: crate::tui::SkillPanelState::default(),
            workflow_popup_mode: WorkflowPopupMode::Hidden,
            workflows: Vec::new(),
            workflow_selected: 0,
            workflow_list_state: ListState::default(),
            selected_workflow: None,
            workflow_execution_result: None,
            workflow_execution_error: None,
            nlp_popup_mode: NLPPopupMode::Hidden,
            nlp_query: String::new(),
            nlp_results: Vec::new(),
            nlp_selected: 0,
        };
        app.log_info(app.ctx.i18n.log.tui_started.to_string());
        app.load_repos()?;
        app.spawn_stars_refresh();
        app.spawn_vault_watcher();
        Ok(app)
    }

    pub(crate) fn load_repos(&mut self) -> anyhow::Result<()> {
        let conn = self.ctx.conn_mut()?;
        let mut repos = repo::list_repos(&conn)?;

        // P2-lite: apply static overrides from workspace/repos.toml
        if let Some(ot) = crate::registry::repos_toml::load_repos_toml() {
            for repo in &mut repos {
                if let Some(o) = ot.repos.iter().find(|o| {
                    repo.local_path.to_string_lossy().contains(&o.path) || repo.id.contains(&o.path)
                }) {
                    crate::registry::repos_toml::apply_overrides(repo, o);
                }
            }
        }

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
                    crate::asyncgit::RepoStatusNotification {
                        repo_id: id,
                        dirty,
                        ahead,
                        behind,
                    },
                ));
            });
        }

        // Initial sort by registry data only (tags alphabetical)
        self.sort_repos_by_registry();
        self.log_info(self.ctx.i18n.log.loaded_repos(self.repos.len()));
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
                let cache_hit =
                    match crate::registry::health::get_stars_cache(&conn, &repo_id) {
                        Ok(Some((stars, fetched_at))) => {
                            let elapsed =
                                Utc::now().signed_duration_since(fetched_at).num_seconds();
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
                    let _ =
                        crate::registry::health::save_stars_cache(&conn, &repo_id, s);
                }
                let _ = tx.send(AsyncNotification::StarsUpdated { repo_id, stars });
            }
        });
    }

    pub(crate) fn spawn_vault_watcher(&mut self) {
        let vault_path = match crate::registry::WorkspaceRegistry::workspace_dir() {
            Ok(ws) => ws.join("vault"),
            Err(_) => return,
        };
        if !vault_path.exists() {
            return;
        }
        let tx = self.async_tx.clone();
        std::thread::spawn(move || {
            let watcher = match crate::watch::FsWatcher::new(&vault_path) {
                Ok(w) => w,
                Err(_) => return,
            };
            loop {
                if watcher.poll_event(std::time::Duration::from_secs(2)).is_some() {
                    // Debounce: wait 500ms then drain remaining events
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = watcher.poll_event(std::time::Duration::from_millis(100));
                    let _ = tx.send(crate::asyncgit::AsyncNotification::VaultChanged);
                }
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

    pub(crate) fn next(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = (self.selected + 1) % self.repos.len();
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected = (self.vault_selected + 1) % self.vaults.len();
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn previous(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = (self.selected + self.repos.len() - 1) % self.repos.len();
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected =
                        (self.vault_selected + self.vaults.len() - 1) % self.vaults.len();
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn jump_to_top(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = 0;
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected = 0;
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn jump_to_bottom(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = self.repos.len() - 1;
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected = self.vaults.len() - 1;
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn toggle_main_view(&mut self) {
        self.main_view = self.main_view.toggle();
    }

    pub(crate) fn toggle_help(&mut self) {
        self.help_popup_mode = match self.help_popup_mode {
            crate::tui::HelpPopupMode::Hidden => crate::tui::HelpPopupMode::Visible,
            crate::tui::HelpPopupMode::Visible => crate::tui::HelpPopupMode::Hidden,
        };
    }

    pub(crate) fn toggle_search_mode(&mut self) {
        self.search_mode = match self.search_mode {
            crate::tui::SearchMode::Repo => crate::tui::SearchMode::Code,
            crate::tui::SearchMode::Code => crate::tui::SearchMode::Repo,
        };
    }

    pub(crate) fn current_repo(&self) -> Option<&RepoItem> {
        self.repos.get(self.selected)
    }

    pub(crate) fn current_vault(&self) -> Option<&VaultItem> {
        self.vaults.get(self.vault_selected)
    }

    pub(crate) fn load_vaults(&mut self) -> anyhow::Result<()> {
        let conn = self.ctx.conn()?;
        let notes = crate::registry::vault::list_vault_notes(&conn)?;
        self.vaults.clear();
        for note in notes {
            self.vaults.push(VaultItem {
                id: note.id,
                path: note.path,
                title: note.title,
                tags: note.tags,
                outgoing_links: note.outgoing_links,
            });
        }
        self.vault_selected = 0;
        self.vault_list_state.select(Some(0));
        self.log_info(self.ctx.i18n.log.loaded_vaults(self.vaults.len()));
        Ok(())
    }

    pub(crate) fn load_skills(&mut self) {
        let Ok(conn) = self.ctx.conn() else {
            return;
        };
        let rows = match crate::skill_runtime::registry::list_skills(&conn, None, None) {
            Ok(r) => r,
            Err(e) => {
                self.log_warn(format!("无法列出 Skills: {}", e));
                self.skill_panel.items.clear();
                self.skill_panel.selected = 0;
                self.skill_panel.list_state.select(Some(0));
                return;
            }
        };
        self.skill_panel.items = rows.into_iter().map(SkillItem::from).collect();
        self.skill_panel.selected = 0;
        self.skill_panel.list_state.select(Some(0));
        self.log_info(self.ctx.i18n.log.loaded_skills(self.skill_panel.items.len()));
    }

    pub(crate) fn next_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected =
                (self.skill_panel.selected + 1) % self.skill_panel.items.len();
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn previous_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected = (self.skill_panel.selected + self.skill_panel.items.len()
                - 1)
                % self.skill_panel.items.len();
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn jump_to_top_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected = 0;
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn jump_to_bottom_skill(&mut self) {
        if !self.skill_panel.items.is_empty() {
            self.skill_panel.selected = self.skill_panel.items.len() - 1;
            self.skill_panel.list_state.select(Some(self.skill_panel.selected));
        }
    }

    pub(crate) fn current_skill(&self) -> Option<&SkillItem> {
        self.skill_panel.items.get(self.skill_panel.selected)
    }

    pub(crate) fn load_workflows(&mut self) {
        let Ok(conn) = self.ctx.conn() else {
            return;
        };
        match crate::workflow::list_workflows(&conn) {
            Ok(rows) => {
                self.workflows = rows
                    .into_iter()
                    .filter_map(|(id, _, _)| {
                        crate::workflow::get_workflow(&conn, &id).ok().flatten()
                    })
                    .collect();
            }
            Err(e) => {
                self.log_warn(format!("无法列出 Workflow: {}", e));
                self.workflows.clear();
            }
        }
        self.workflow_selected = 0;
        self.workflow_list_state.select(Some(0));
        self.log_info(format!("已加载 {} 个 Workflow", self.workflows.len()));
    }

    pub(crate) fn next_workflow(&mut self) {
        if !self.workflows.is_empty() {
            self.workflow_selected = (self.workflow_selected + 1) % self.workflows.len();
            self.workflow_list_state.select(Some(self.workflow_selected));
        }
    }

    pub(crate) fn previous_workflow(&mut self) {
        if !self.workflows.is_empty() {
            self.workflow_selected =
                (self.workflow_selected + self.workflows.len() - 1) % self.workflows.len();
            self.workflow_list_state.select(Some(self.workflow_selected));
        }
    }

    pub(crate) fn current_workflow(&self) -> Option<&crate::workflow::WorkflowDefinition> {
        self.workflows.get(self.workflow_selected)
    }

    pub(crate) fn run_selected_workflow(&mut self) {
        let wf = match self.selected_workflow.clone() {
            Some(w) => w,
            None => {
                self.log_warn("未选择 Workflow".to_string());
                return;
            }
        };

        let mut inputs = std::collections::HashMap::new();
        for inp in &wf.inputs {
            if inp.required && inp.default.is_none() {
                self.log_warn(format!("Workflow '{}' 缺少必要输入: {}", wf.id, inp.name));
                return;
            }
            if let Some(default) = &inp.default {
                let val = match default {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
                };
                inputs.insert(inp.name.clone(), val);
            }
        }

        let tx = self.async_tx.clone();
        let pool = self.ctx.pool();
        std::thread::spawn(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::WorkflowRunFinished {
                        workflow_id: wf.id.clone(),
                        results: std::collections::HashMap::new(),
                        error: Some(e.to_string()),
                    });
                    return;
                }
            };
            let result = crate::workflow::execute_workflow(&conn, &pool, &wf, inputs);
            match result {
                Ok(results) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::WorkflowRunFinished {
                        workflow_id: wf.id,
                        results,
                        error: None,
                    });
                }
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::WorkflowRunFinished {
                        workflow_id: wf.id,
                        results: std::collections::HashMap::new(),
                        error: Some(e.to_string()),
                    });
                }
            }
        });
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

    pub(crate) fn run_selected_skill(&mut self) {
        let skill_item = match self.current_skill() {
            Some(s) => s.clone(),
            None => {
                self.log_warn("未选择 Skill".to_string());
                return;
            }
        };
        self.run_skill_item(
            skill_item,
            self.skill_panel
                .param_buffer
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
        );
    }

    pub(crate) fn run_nlp_selected_skill(&mut self) {
        let skill_item = match self.nlp_results.get(self.nlp_selected) {
            Some(s) => s.clone(),
            None => {
                self.log_warn("未选择 NLQ 结果".to_string());
                return;
            }
        };
        self.run_skill_item(skill_item, vec![]);
    }

    fn run_skill_item(&mut self, skill_item: SkillItem, args: Vec<String>) {
        let tx = self.async_tx.clone();
        let pool = self.ctx.pool();
        std::thread::spawn(move || {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(crate::asyncgit::AsyncNotification::SkillRunFinished(
                        crate::skill_runtime::ExecutionResult {
                            skill_id: skill_item.row.id.clone(),
                            status: crate::skill_runtime::ExecutionStatus::Failed,
                            stdout: String::new(),
                            stderr: e.to_string(),
                            exit_code: Some(1),
                            duration_ms: 0,
                        },
                    ));
                    return;
                }
            };
            let result = crate::skill_runtime::executor::run_skill(
                &conn,
                &skill_item.row,
                &args,
                std::time::Duration::from_secs(30),
            );
            let execution_result = match result {
                Ok(r) => r,
                Err(e) => crate::skill_runtime::ExecutionResult {
                    skill_id: skill_item.row.id.clone(),
                    status: crate::skill_runtime::ExecutionStatus::Failed,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    exit_code: Some(1),
                    duration_ms: 0,
                },
            };
            let _ = tx.send(crate::asyncgit::AsyncNotification::SkillRunFinished(execution_result));
        });
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
                self.log_info(
                    self.ctx.i18n.log.status_fmt(&n.repo_id, n.dirty, n.ahead, n.behind),
                );
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
                if let Some(entry) =
                    self.sync_popup_results.iter_mut().find(|(id, _)| id == &n.repo_id)
                {
                    entry.1 = n.message.clone();
                } else {
                    self.sync_popup_results.push((n.repo_id.clone(), n.message.clone()));
                }
                self.log_info(
                    self.ctx.i18n.log.sync_progress_fmt(&n.repo_id, &n.action, &n.message),
                );
            }
            AsyncNotification::StarsUpdated { repo_id, stars } => {
                if let Some(repo) = self.repos.iter_mut().find(|r| r.id == repo_id) {
                    repo.stars = stars;
                }
                if let Some(s) = stars
                    && let Ok(conn) = self.ctx.conn_mut()
                {
                    let _ =
                        crate::registry::health::save_stars_cache(&conn, &repo_id, s);
                }
                // Re-sort if currently sorting by stars
                if self.sort_mode == SortMode::Stars {
                    self.repos.sort_by(|a, b| {
                        b.stars
                            .unwrap_or(0)
                            .cmp(&a.stars.unwrap_or(0))
                            .then_with(|| a.id.cmp(&b.id))
                    });
                }
            }
            AsyncNotification::VaultChanged => {
                self.log_info("Vault changed, refreshing...".to_string());
                if let Err(e) = self.load_vaults() {
                    self.log_error(format!("Vault refresh failed: {}", e));
                }
            }
            AsyncNotification::SkillRunFinished(result) => {
                let status_label = match result.status {
                    crate::skill_runtime::ExecutionStatus::Success => "成功",
                    _ => "失败",
                };
                self.log_info(format!(
                    "Skill [{}] 执行{} (exit_code={:?}, {}ms)",
                    result.skill_id, status_label, result.exit_code, result.duration_ms
                ));
                self.skill_panel.execution_result = Some(result);
                self.skill_panel.popup_mode = SkillPopupMode::Result;
            }
            AsyncNotification::WorkflowRunFinished { workflow_id, results, error } => {
                if let Some(e) = error {
                    self.log_error(format!("Workflow [{}] 执行失败: {}", workflow_id, e));
                    self.workflow_execution_error = Some(e);
                } else {
                    self.log_info(format!(
                        "Workflow [{}] 执行完成 ({} steps)",
                        workflow_id,
                        results.len()
                    ));
                    self.workflow_execution_error = None;
                }
                self.workflow_execution_result = Some(results);
                self.workflow_popup_mode = crate::tui::WorkflowPopupMode::Result;
            }
            AsyncNotification::NLPQueryFinished { query, skills, error } => {
                if let Some(e) = error {
                    self.log_error(format!("NLQ '{}' 失败: {}", query, e));
                    self.nlp_results.clear();
                } else {
                    self.log_info(format!("NLQ '{}' 找到 {} 个 skill", query, skills.len()));
                    self.nlp_results = skills.into_iter().map(SkillItem::from).collect();
                }
                self.nlp_selected = 0;
                self.nlp_popup_mode = NLPPopupMode::Results;
            }
        }
    }

    pub(crate) fn update_tags(&mut self, new_tags: &str) {
        let repo_id = match self.current_repo() {
            Some(r) => r.id.clone(),
            None => {
                self.log_warn(self.ctx.i18n.log.no_repo_selected.to_string());
                return;
            }
        };

        match (|| -> anyhow::Result<()> {
            let mut conn = self.ctx.conn_mut()?;
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
                self.log_info(self.ctx.i18n.log.updated_tags(&repo_id, new_tags));
                if let Err(e) = self.load_repos() {
                    self.log_error(self.ctx.i18n.log.reload_repos_failed(e));
                }
            }
            Err(e) => self.log_error(self.ctx.i18n.log.update_tags_failed(e)),
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
            self.sync_popup_results.push((
                "system".to_string(),
                self.ctx.i18n.sync.no_safe_repos.to_string(),
            ));
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
            && let Ok(history) =
                crate::registry::health::get_stars_history(&conn, &repo.id, 7)
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

fn search_repo_fallback(
    repo_path: &str,
    pattern: &str,
    repo_id: &str,
    results: &mut Vec<SearchResult>,
) {
    use std::collections::HashSet;

    let excluded_dirs: HashSet<&str> =
        [".git", "target", "node_modules", ".venv", "venv", "dist", "build"]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_nlp_selected_skill_empty_results() {
        // Smoke test: calling run_nlp_selected_skill with empty results should not panic
        let (tx, _rx) = crossbeam_channel::bounded::<AsyncNotification>(1);
        let mut app = App {
            repos: vec![],
            selected: 0,
            logs: vec![],
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            list_state: ListState::default(),
            async_rx: _rx,
            async_tx: tx.clone(),
            repo_status_job: crate::asyncgit::AsyncSingleJob::new(tx),
            loading_repo_status: HashSet::new(),
            loading_sync: HashSet::new(),
            sync_orchestrator: crate::sync::SyncOrchestrator::new(1, crate::i18n::from_language("en")),
            sync_popup_mode: SyncPopupMode::Hidden,
            sync_preview_items: vec![],
            sync_popup_results: vec![],
            sync_total: 0,
            sync_start_time: None,
            sync_running: HashSet::new(),
            sync_timeout: std::time::Duration::from_secs(60),
            sort_mode: SortMode::Status,
            ctx: crate::storage::AppContext::with_defaults().unwrap(),
            dry_run: false,
            search_popup_mode: SearchPopupMode::Hidden,
            search_results: vec![],
            search_selected: 0,
            search_pattern: String::new(),
            detail_tab: crate::tui::DetailTab::Overview,
            help_popup_mode: crate::tui::HelpPopupMode::Hidden,
            search_mode: crate::tui::SearchMode::Code,
            main_view: MainView::RepoList,
            vaults: vec![],
            vault_selected: 0,
            vault_list_state: ListState::default(),
            skill_panel: crate::tui::SkillPanelState::default(),
            workflow_popup_mode: WorkflowPopupMode::Hidden,
            workflows: vec![],
            workflow_selected: 0,
            workflow_list_state: ListState::default(),
            selected_workflow: None,
            workflow_execution_result: None,
            workflow_execution_error: None,
            nlp_popup_mode: NLPPopupMode::Results,
            nlp_query: "test".to_string(),
            nlp_results: vec![],
            nlp_selected: 0,
        };
        app.run_nlp_selected_skill(); // should not panic
        assert!(app.logs.iter().any(|l| l.contains("未选择 NLQ 结果")));
    }
}
