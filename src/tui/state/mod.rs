use crate::asyncgit::AsyncNotification;
use crate::tui::{
    App, InputMode, ListState, MainView, NLPPopupMode, RepoItem, SearchPopupMode, SkillItem,
    SkillPopupMode, SortMode, SyncPopupMode, WorkflowPopupMode,
};
use crossbeam_channel::bounded;
use std::collections::HashSet;
use std::time::Duration;

pub mod navigation;
pub mod panel;
pub mod repo;
pub mod search_sync;
pub mod vault;
pub mod view;

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
            main_view: MainView::RepoList,
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
        let mut repos = crate::registry::repo::list_repos(&conn)?;

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

    pub(crate) fn update_async(&mut self, notification: AsyncNotification) {
        match notification {
            AsyncNotification::RepoStatus(n) => {
                self.loading_repo_status.remove(&n.repo_id);
                if let Some(repo) = self.repos.iter_mut().find(|r| r.id == n.repo_id) {
                    repo.status_dirty = Some(n.dirty);
                    repo.status_ahead = Some(n.ahead);
                    repo.status_behind = Some(n.behind);
                }
                self.log_info(self.ctx.i18n.log.status_fmt(&n.repo_id, n.dirty, n.ahead, n.behind));
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
                    let _ = crate::registry::health::save_stars_cache(&conn, &repo_id, s);
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
            sync_orchestrator: crate::sync::SyncOrchestrator::new(
                1,
                crate::i18n::from_language("en"),
            ),
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
