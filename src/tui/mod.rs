use crate::asyncgit::AsyncNotification;
use crossbeam_channel::Receiver;
use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SortMode {
    Status,
    Stars,
}

#[derive(Clone)]
pub(crate) struct RepoItem {
    pub(crate) id: String,
    pub(crate) local_path: String,
    pub(crate) upstream_url: Option<String>,
    pub(crate) default_branch: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) language: Option<String>,
    pub(crate) status_dirty: Option<bool>,
    pub(crate) status_ahead: Option<usize>,
    pub(crate) status_behind: Option<usize>,
    pub(crate) stars: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum InputMode {
    Normal,
    TagInput,
    SearchInput,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SyncPopupMode {
    Hidden,
    Preview,
    Progress,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SearchPopupMode {
    Hidden,
    Input,
    Results,
}

#[derive(Clone)]
pub(crate) struct SearchResult {
    pub(crate) repo_id: String,
    pub(crate) file_path: String,
    pub(crate) line_number: usize,
    pub(crate) line_content: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SyncPreviewItem {
    pub(crate) repo_id: String,
    pub(crate) safety: crate::sync::SyncSafety,
    pub(crate) policy: crate::sync::SyncPolicy,
    pub(crate) ahead: usize,
    pub(crate) behind: usize,
    pub(crate) recommendation: Option<String>,
}

pub struct App {
    pub(crate) repos: Vec<RepoItem>,
    pub(crate) selected: usize,
    pub(crate) logs: Vec<String>,
    pub(crate) show_help: bool,
    pub(crate) input_mode: InputMode,
    pub(crate) input_buffer: String,
    pub(crate) list_state: ListState,
    pub(crate) async_rx: Receiver<AsyncNotification>,
    pub(crate) async_tx: crossbeam_channel::Sender<AsyncNotification>,
    pub(crate) repo_status_job: crate::asyncgit::AsyncSingleJob<crate::asyncgit::AsyncRepoStatus>,
    pub(crate) loading_repo_status: HashSet<String>,
    pub(crate) loading_sync: HashSet<String>,
    pub(crate) sync_orchestrator: crate::sync::SyncOrchestrator,
    pub(crate) sync_popup_mode: SyncPopupMode,
    pub(crate) sync_preview_items: Vec<SyncPreviewItem>,
    pub(crate) sync_popup_results: Vec<(String, String)>, // (repo_id, message)
    pub(crate) sync_total: usize,
    pub(crate) sync_start_time: Option<Instant>,
    pub(crate) sync_running: HashSet<String>,
    pub(crate) sync_timeout: Duration,
    pub(crate) sort_mode: SortMode,
    pub(crate) config: crate::config::Config,
    pub(crate) dry_run: bool,
    pub(crate) search_popup_mode: SearchPopupMode,
    pub(crate) search_results: Vec<SearchResult>,
    pub(crate) search_selected: usize,
    pub(crate) search_pattern: String,
}

pub mod state;
pub mod event;
pub mod render;

use self::event::{run_app, TuiAction};

pub async fn run() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    let mut app = App::new()?;
    let mut action = run_app(&mut terminal, &mut app).await;

    loop {
        match action {
            Ok(TuiAction::Quit) => break,
            Ok(TuiAction::LaunchExternal { cmd, cwd }) => {
                ratatui::restore();
                let _ = std::process::Command::new(&cmd).current_dir(&cwd).status();
                terminal = ratatui::init();
                terminal.clear()?;
                action = run_app(&mut terminal, &mut app).await;
            }
            Err(e) => {
                ratatui::restore();
                return Err(e.into());
            }
        }
    }

    ratatui::restore();
    Ok(())
}
