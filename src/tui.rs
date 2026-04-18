use crate::asyncgit::AsyncNotification;
use crate::registry::WorkspaceRegistry;
use crossbeam_channel::{Receiver, bounded};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashSet;
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct RepoItem {
    id: String,
    local_path: String,
    upstream_url: Option<String>,
    default_branch: Option<String>,
    tags: Vec<String>,
    language: Option<String>,
    status_dirty: Option<bool>,
    status_ahead: Option<usize>,
    status_behind: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum InputMode {
    Normal,
    TagInput,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SyncPopupMode {
    Hidden,
    Preview,
    Progress,
}

#[derive(Debug, Clone)]
struct SyncPreviewItem {
    repo_id: String,
    safety: crate::sync::SyncSafety,
    policy: crate::sync::SyncPolicy,
    ahead: usize,
    behind: usize,
}

pub struct App {
    repos: Vec<RepoItem>,
    selected: usize,
    logs: Vec<String>,
    show_help: bool,
    input_mode: InputMode,
    input_buffer: String,
    list_state: ListState,
    async_rx: Receiver<AsyncNotification>,
    async_tx: crossbeam_channel::Sender<AsyncNotification>,
    repo_status_job: crate::asyncgit::AsyncSingleJob<crate::asyncgit::AsyncRepoStatus>,
    loading_repo_status: HashSet<String>,
    loading_sync: HashSet<String>,
    sync_orchestrator: crate::sync::SyncOrchestrator,
    sync_popup_mode: SyncPopupMode,
    sync_preview_items: Vec<SyncPreviewItem>,
    sync_popup_results: Vec<(String, String)>, // (repo_id, message)
    sync_total: usize,
    sync_start_time: Option<Instant>,
    sync_running: HashSet<String>,
    sync_timeout: Duration,
}

impl App {
    fn new() -> anyhow::Result<Self> {
        let (async_tx, async_rx) = bounded::<AsyncNotification>(100);
        let repo_status_job = crate::asyncgit::AsyncSingleJob::new(async_tx.clone());

        let timeout_secs = crate::config::Config::load()
            .map(|c| c.sync.timeout_seconds)
            .unwrap_or(60);
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
            sync_orchestrator: crate::sync::SyncOrchestrator::new(8),
            sync_popup_mode: SyncPopupMode::Hidden,
            sync_preview_items: Vec::new(),
            sync_popup_results: Vec::new(),
            sync_total: 0,
            sync_start_time: None,
            sync_running: HashSet::new(),
            sync_timeout: Duration::from_secs(timeout_secs),
        };
        app.log_info(crate::i18n::current().log.tui_started.to_string());
        app.load_repos()?;
        Ok(app)
    }

    fn load_repos(&mut self) -> anyhow::Result<()> {
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
            });
        }
        // Initial status assessment for better sorting and display
        for repo in &mut self.repos {
            if repo.upstream_url.is_some() {
                let policy = crate::sync::SyncPolicy::from_tags(&repo.tags.join(","));
                let (safety, ahead, behind) = crate::sync::assess_safety(&repo.local_path, policy);
                repo.status_dirty = Some(safety == crate::sync::SyncSafety::BlockedDirty);
                repo.status_ahead = Some(ahead);
                repo.status_behind = Some(behind);
            }
        }

        // Sort by status priority: needs action first, then by tag, then id
        self.repos.sort_by(|a, b| {
            let priority = |repo: &RepoItem| -> i32 {
                match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
                    (Some(true), _, _) => 0,                              // dirty
                    (Some(false), Some(a), Some(b)) if a > 0 && b > 0 => 1, // diverged
                    (Some(false), _, Some(b)) if b > 0 => 2,              // behind
                    (Some(false), Some(a), _) if a > 0 => 3,              // ahead
                    _ => 4,                                               // up-to-date / unknown
                }
            };
            let pa = priority(a);
            let pb = priority(b);
            pa.cmp(&pb).then_with(|| {
                let tag_a = a.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
                let tag_b = b.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
                tag_a.cmp(tag_b)
            }).then_with(|| a.id.cmp(&b.id))
        });
        if self.selected >= self.repos.len() && !self.repos.is_empty() {
            self.selected = self.repos.len() - 1;
        }
        self.list_state.select(Some(self.selected));
        self.log_info(crate::i18n::current().log.loaded_repos(self.repos.len()));
        self.spawn_repo_status_for_current();
        Ok(())
    }

    fn log_info(&mut self, msg: String) {
        self.log_with_level("INFO", msg);
    }

    fn log_warn(&mut self, msg: String) {
        self.log_with_level("WARN", msg);
    }

    fn log_error(&mut self, msg: String) {
        self.log_with_level("ERROR", msg);
    }

    fn log_with_level(&mut self, level: &str, msg: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(format!("[{}] [{}] {}", time, level, msg));
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    fn spawn_repo_status_for_current(&mut self) {
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

    fn next(&mut self) {
        if !self.repos.is_empty() {
            self.selected = (self.selected + 1) % self.repos.len();
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    fn previous(&mut self) {
        if !self.repos.is_empty() {
            self.selected = (self.selected + self.repos.len() - 1) % self.repos.len();
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    fn jump_to_top(&mut self) {
        if !self.repos.is_empty() {
            self.selected = 0;
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    fn jump_to_bottom(&mut self) {
        if !self.repos.is_empty() {
            self.selected = self.repos.len() - 1;
            self.list_state.select(Some(self.selected));
            self.spawn_repo_status_for_current();
        }
    }

    fn current_repo(&self) -> Option<&RepoItem> {
        self.repos.get(self.selected)
    }

    fn update_async(&mut self, notification: AsyncNotification) {
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
        }
    }

    fn update_tags(&mut self, new_tags: &str) {
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

    fn safe_sync_preview(&mut self) {
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
            self.sync_preview_items.push(SyncPreviewItem {
                repo_id: repo.id.clone(),
                safety,
                policy,
                ahead,
                behind,
            });
        }

        if self.sync_preview_items.is_empty() {
            self.sync_popup_results.push(("system".to_string(), "No repositories eligible for safe sync.".to_string()));
            self.sync_popup_mode = SyncPopupMode::Progress;
        }
    }

    fn fetch_all_and_preview(&mut self) {
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

    fn start_safe_sync(&mut self) {
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
            self.sync_popup_results.push(("system".to_string(), "No safe repositories to sync.".to_string()));
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
}

pub async fn run() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    let mut app = App::new()?;
    let res = run_app(&mut terminal, &mut app).await;

    ratatui::restore();
    Ok(res?)
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app)).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.sync_popup_mode {
                        SyncPopupMode::Preview => {
                            match key.code {
                                KeyCode::Enter => app.start_safe_sync(),
                                KeyCode::Esc => app.sync_popup_mode = SyncPopupMode::Hidden,
                                _ => {}
                            }
                            continue; // 弹窗显示时不处理其他按键
                        }
                        SyncPopupMode::Progress => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.sync_popup_mode = SyncPopupMode::Hidden,
                                _ => {}
                            }
                            continue; // 弹窗显示时不处理其他按键
                        }
                        SyncPopupMode::Hidden => {}
                    }
                    match app.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('r') => {
                                app.log_info(crate::i18n::current().log.refreshing.to_string());
                                if let Err(e) = app.load_repos() {
                                    app.log_error(crate::i18n::current().log.refresh_failed(e));
                                }
                            }
                            KeyCode::Char('s') => app.fetch_all_and_preview(),
                            KeyCode::Char('S') => app.start_safe_sync(),
                            KeyCode::Char('t') => {
                                app.input_mode = InputMode::TagInput;
                                app.input_buffer.clear();
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Down => app.next(),
                            KeyCode::Up => app.previous(),
                            KeyCode::Home | KeyCode::PageUp => app.jump_to_top(),
                            KeyCode::End | KeyCode::PageDown => app.jump_to_bottom(),
                            _ => {}
                        },
                        InputMode::TagInput => match key.code {
                            KeyCode::Enter => {
                                let tags = app.input_buffer.trim().to_string();
                                if !tags.is_empty() {
                                    app.update_tags(&tags);
                                } else {
                                    app.log_warn(crate::i18n::current().log.empty_tag_ignored.to_string());
                                }
                                app.input_mode = InputMode::Normal;
                                app.input_buffer.clear();
                            }
                            KeyCode::Esc => {
                                app.input_mode = InputMode::Normal;
                                app.input_buffer.clear();
                                app.log_info(crate::i18n::current().log.tag_input_cancelled.to_string());
                            }
                            KeyCode::Char(c) => app.input_buffer.push(c),
                            KeyCode::Backspace => {
                                app.input_buffer.pop();
                            }
                            _ => {}
                        },
                    }
                }
            }
        }

        while let Ok(notification) = app.async_rx.try_recv() {
            app.update_async(notification);
        }
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    let bottom_height = if app.show_help || app.input_mode == InputMode::TagInput {
        1
    } else {
        0
    };

    let main_vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(bottom_height),
        ])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(main_vertical[0]);

    // Left: repo list (sorted by status priority)
    let items: Vec<ListItem> = app
        .repos
        .iter()
        .map(|repo| {
            let status_icon = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
                (Some(true), _, _) => "🔴",
                (Some(false), Some(a), Some(b)) if a > 0 && b > 0 => "🟡",
                (Some(false), _, Some(b)) if b > 0 => "🟡",
                (Some(false), Some(a), _) if a > 0 => "🔵",
                _ if repo.upstream_url.is_none() => "⚪",
                _ => "🟢",
            };
            let mut prefix = format!("{} ", status_icon);
            if app.loading_repo_status.contains(&repo.id)
                || app.loading_sync.contains(&repo.id)
            {
                prefix.push_str("⏳ ");
            }

            let base_fg = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
                (Some(true), _, _) => Color::Red,
                (Some(false), _, Some(b)) if b > 0 => Color::Yellow,
                (Some(false), Some(a), _) if a > 0 => Color::Blue,
                _ if repo.upstream_url.is_none() => Color::DarkGray,
                _ => Color::Green,
            };

            let fg = if app.loading_repo_status.contains(&repo.id)
                || app.loading_sync.contains(&repo.id)
            {
                Color::LightCyan
            } else {
                base_fg
            };

            // Status suffix: behind/ahead count
            let status_suffix = match (repo.status_ahead, repo.status_behind) {
                (Some(_), Some(b)) if b > 0 => format!(" ↓{}", b),
                (Some(a), _) if a > 0 => format!(" ↑{}", a),
                _ => String::new(),
            };

            // Tag cluster indicator: show primary tag in muted color
            let tag_indicator = if let Some(first_tag) = repo.tags.first() {
                format!(" [{}]", first_tag)
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, repo.id), Style::default().fg(fg)),
                Span::styled(status_suffix, Style::default().fg(Color::DarkGray)),
                Span::styled(tag_indicator, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(crate::i18n::current().tui.title_repos))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 80))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // Right: detail + logs
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[1]);

    // Detail panel
    let detail_text = if let Some(repo) = app.current_repo() {
        let mut tag_line = vec![
            Span::styled("标签: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ];
        tag_line.extend(tag_spans(&repo.tags));

        // ── Core status block ──
        let (dirty, ahead, behind) = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
            (Some(d), Some(a), Some(b)) => (d, a, b),
            _ => (false, 0, 0),
        };
        let status_color = if dirty {
            Color::Red
        } else if behind > 0 || ahead > 0 {
            Color::Yellow
        } else {
            Color::Green
        };
        let status_icon = if dirty { "⚠" } else if behind > 0 || ahead > 0 { "●" } else { "✓" };
        let status_desc = if dirty {
            "工作目录不干净".to_string()
        } else if behind > 0 && ahead > 0 {
            format!("分叉  ahead={} behind={}", ahead, behind)
        } else if behind > 0 {
            format!("落后远程 {} commit", behind)
        } else if ahead > 0 {
            format!("超前远程 {} commit", ahead)
        } else {
            "已最新".to_string()
        };

        // Git HEAD + sync history
        let head_short = read_head_commit(&repo.local_path).unwrap_or_else(|| "—".to_string());
        let (last_sync_human, last_sync_action, last_sync_commit) = read_syncdone_info(&repo.local_path);
        let summary_text = read_repo_summary(&repo.id).unwrap_or_else(|| "暂无描述".to_string());

        let policy = crate::sync::SyncPolicy::from_tags(&repo.tags.join(","));
        let policy_text = format!("{:?}", policy);
        let policy_color = match policy {
            crate::sync::SyncPolicy::Mirror => Color::Blue,
            crate::sync::SyncPolicy::Conservative => Color::Yellow,
            crate::sync::SyncPolicy::Rebase => Color::Green,
            crate::sync::SyncPolicy::Merge => Color::Magenta,
        };

        let lines: Vec<Line> = vec![
            // === Layer 1: Core status (human decision-making) ===
            Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
                Span::styled(&repo.id, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(status_desc, Style::default().fg(status_color)),
            ]),
            Line::from(vec![
                Span::styled("HEAD: ", Style::default().fg(Color::DarkGray)),
                Span::styled(head_short.clone(), Style::default().fg(Color::White)),
                Span::styled("  策略: ", Style::default().fg(Color::DarkGray)),
                Span::styled(policy_text, Style::default().fg(policy_color)),
            ]),
            Line::from(""),

            // === Layer 1.5: What is this repo? ===
            Line::from(vec![
                Span::styled("描述: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(summary_text.clone(), Style::default().fg(Color::White)),
            ]),
            Line::from(""),

            // === Layer 2: Connection metadata ===
            Line::from(vec![
                Span::styled("分支: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(repo.default_branch.as_deref().unwrap_or("—")),
                Span::styled("  语言: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(repo.language.as_deref().unwrap_or("—")),
            ]),
            Line::from(vec![
                Span::styled("远程: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(
                    repo.upstream_url.as_deref().unwrap_or("(无)"),
                    if repo.upstream_url.is_some() { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Yellow) },
                ),
            ]),
            Line::from(tag_line),
            Line::from(""),

            // === Layer 3: Sync history ===
            Line::from(vec![
                Span::styled("上次同步: ", Style::default().fg(Color::DarkGray)),
                Span::styled(last_sync_human.clone(), Style::default().fg(Color::White)),
                Span::styled(format!(" ({}) ", last_sync_action.clone()), Style::default().fg(Color::DarkGray)),
                Span::styled(last_sync_commit.clone(), Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),

            // === Layer 4: Action hint ===
            Line::from(vec![
                Span::styled("操作: ", Style::default().fg(Color::DarkGray)),
                Span::styled("s 预览  S 执行  r 刷新", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        Text::from(lines)
    } else {
        Text::raw(crate::i18n::current().log.no_repos_registered)
    };

    let detail = Paragraph::new(detail_text)
        .block(Block::default().borders(Borders::ALL).title(crate::i18n::current().tui.title_details))
        .wrap(Wrap { trim: true });

    frame.render_widget(detail, right_chunks[0]);

    // Logs panel
    let log_visible = right_chunks[1].height.saturating_sub(2) as usize;
    let log_start = app.logs.len().saturating_sub(log_visible);
    let log_lines: Vec<Line> = app.logs[log_start..].iter().map(|l| format_log_line(l)).collect();
    let log_text = Text::from(log_lines);
    let logs = Paragraph::new(log_text)
        .block(Block::default().borders(Borders::ALL).title(crate::i18n::current().tui.title_logs))
        .wrap(Wrap { trim: true });

    frame.render_widget(logs, right_chunks[1]);

    // Sync popup
    match app.sync_popup_mode {
        SyncPopupMode::Preview => {
            let popup_area = centered_rect(60, 50, frame.area());
            let popup_inner = popup_area.inner(ratatui::layout::Margin {
                horizontal: 1,
                vertical: 1,
            });

            let mut lines: Vec<Line> = Vec::new();

            // If preview items are empty but popup results exist, we're in fetch-progress mode
            if app.sync_preview_items.is_empty() && !app.sync_popup_results.is_empty() {
                lines.push(Line::from(Span::styled(
                    "正在获取远程状态...",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
                for (repo_id, msg) in &app.sync_popup_results {
                    if repo_id == "system" { continue; }
                    let color = if msg.contains("Fetched") {
                        Color::Green
                    } else if msg.contains("Error") || msg.contains("TIMEOUT") {
                        Color::Red
                    } else {
                        Color::Yellow
                    };
                    lines.push(Line::from(vec![
                        Span::raw(format!("  [{}] ", repo_id)),
                        Span::styled(msg, Style::default().fg(color)),
                    ]));
                }

                let popup_text = Text::from(lines);
                let popup_para = Paragraph::new(popup_text)
                    .block(Block::default().borders(Borders::ALL).title("Safe Sync Preview"))
                    .wrap(Wrap { trim: true });

                frame.render_widget(ratatui::widgets::Clear, popup_area);
                frame.render_widget(popup_para, popup_area);
                return;
            }

            let safe: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::Safe).collect();
            let diverged: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::BlockedDiverged).collect();
            let dirty: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::BlockedDirty).collect();
            let local_ahead: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::LocalAhead).collect();
            let up_to_date: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::UpToDate).collect();
            let no_upstream: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::NoUpstream).collect();
            let unknown: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::Unknown).collect();

            if !safe.is_empty() {
                lines.push(Line::from(Span::styled(format!("将执行 ({})", safe.len()), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
                for item in safe {
                    lines.push(Line::from(format!("  [{}] {:?} behind={}", item.repo_id, item.policy, item.behind)));
                }
                lines.push(Line::from(""));
            }
            if !diverged.is_empty() {
                lines.push(Line::from(Span::styled(format!("被阻塞 - 分叉 ({})", diverged.len()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                for item in diverged {
                    lines.push(Line::from(format!("  [{}] {:?} ahead={} behind={}", item.repo_id, item.policy, item.ahead, item.behind)));
                }
                lines.push(Line::from(""));
            }
            if !dirty.is_empty() {
                lines.push(Line::from(Span::styled(format!("被阻塞 - 工作目录不干净 ({})", dirty.len()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                for item in dirty {
                    lines.push(Line::from(format!("  [{}] {:?}", item.repo_id, item.policy)));
                }
                lines.push(Line::from(""));
            }
            if !local_ahead.is_empty() {
                lines.push(Line::from(Span::styled(format!("本地超前 - 将推送 ({})", local_ahead.len()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
                for item in local_ahead {
                    lines.push(Line::from(format!("  [{}] {:?} ahead={}", item.repo_id, item.policy, item.ahead)));
                }
                lines.push(Line::from(""));
            }
            if !up_to_date.is_empty() {
                lines.push(Line::from(Span::styled(format!("已最新 ({})", up_to_date.len()), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))));
                for item in up_to_date {
                    lines.push(Line::from(format!("  [{}] {:?}", item.repo_id, item.policy)));
                }
                lines.push(Line::from(""));
            }
            if !no_upstream.is_empty() {
                lines.push(Line::from(Span::styled(format!("无远程 ({})", no_upstream.len()), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))));
                for item in no_upstream {
                    lines.push(Line::from(format!("  [{}] {:?}", item.repo_id, item.policy)));
                }
                lines.push(Line::from(""));
            }
            if !unknown.is_empty() {
                lines.push(Line::from(Span::styled(format!("异常 ({})", unknown.len()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                for item in unknown {
                    lines.push(Line::from(format!("  [{}] {:?}", item.repo_id, item.policy)));
                }
                lines.push(Line::from(""));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "注：基于本地缓存评估，同步前会重新获取远程状态",
                Style::default().fg(Color::DarkGray),
            )));

            let popup_text = Text::from(lines);
            let popup_para = Paragraph::new(popup_text)
                .block(Block::default().borders(Borders::ALL).title("Safe Sync Preview"))
                .wrap(Wrap { trim: true });

            frame.render_widget(ratatui::widgets::Clear, popup_area);
            frame.render_widget(popup_para, popup_area);

            let hint = Paragraph::new(Span::styled(
                "[Enter] 确认执行  [Esc] 取消",
                Style::default().fg(Color::DarkGray),
            ));
            let hint_height = 1;
            let hint_area = ratatui::layout::Rect {
                x: popup_inner.x,
                y: popup_inner.y + popup_inner.height.saturating_sub(hint_height),
                width: popup_inner.width,
                height: hint_height,
            };
            frame.render_widget(hint, hint_area);
        }
        SyncPopupMode::Progress => {
            let popup_area = centered_rect(60, 40, frame.area());
            let popup_inner = popup_area.inner(ratatui::layout::Margin {
                horizontal: 1,
                vertical: 1,
            });

            let queued = app.loading_sync.len();
            let running = app.sync_running.len();
            let completed = app.sync_total.saturating_sub(queued + running);
            let elapsed_secs = app
                .sync_start_time
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            let i18n = crate::i18n::current();
            let popup_title = Line::from(vec![
                Span::raw(i18n.tui.title_sync_progress),
                Span::raw(" | "),
                Span::styled(
                    format!("{}{}", completed, i18n.tui.sync_done),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(
                    format!("{}{}", running, i18n.tui.sync_running),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{}{}", queued, i18n.tui.sync_queued),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(format!(" | {}{}s", i18n.tui.elapsed, elapsed_secs)),
            ]);

            let items: Vec<ListItem> = app
                .sync_popup_results
                .iter()
                .map(|(repo_id, message)| {
                    let msg_lower = message.to_lowercase();
                    let is_error = msg_lower.contains("failed")
                        || msg_lower.contains("error")
                        || msg_lower.contains("timeout")
                        || msg_lower.contains("超时");
                    let is_pending = message == crate::i18n::current().log.status_queued
                        || message == crate::i18n::current().sync.status_running;
                    let color = if is_error {
                        Color::Red
                    } else if is_pending {
                        Color::Yellow
                    } else {
                        Color::Green
                    };
                    ListItem::new(Span::styled(
                        format!("[{}] {}", repo_id, message),
                        Style::default().fg(color),
                    ))
                })
                .collect();

            let popup_list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(popup_title),
                );

            frame.render_widget(ratatui::widgets::Clear, popup_area);
            frame.render_widget(popup_list, popup_area);

            let hint = Paragraph::new(Span::styled(
                crate::i18n::current().tui.hint_popup_close,
                Style::default().fg(Color::DarkGray),
            ));
            let hint_height = 1;
            let hint_area = ratatui::layout::Rect {
                x: popup_inner.x,
                y: popup_inner.y + popup_inner.height.saturating_sub(hint_height),
                width: popup_inner.width,
                height: hint_height,
            };
            frame.render_widget(hint, hint_area);
        }
        SyncPopupMode::Hidden => {}
    }

    // Bottom bar
    if bottom_height > 0 {
        let bottom_text = match app.input_mode {
            InputMode::TagInput => Line::from(vec![
                Span::styled("标签: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(&app.input_buffer),
                Span::styled(crate::i18n::current().tui.hint_tag_input, Style::default().fg(Color::DarkGray)),
            ]),
            InputMode::Normal => {
                let mut spans = vec![
                    Span::styled("q", Style::default().fg(Color::Cyan)),
                    Span::raw("=退出 "),
                    Span::styled("r", Style::default().fg(Color::Cyan)),
                    Span::raw("=刷新 "),
                    Span::styled("s", Style::default().fg(Color::Cyan)),
                    Span::raw("=获取预览 "),
                    Span::styled("S", Style::default().fg(Color::Cyan)),
                    Span::raw("=批量同步 "),
                    Span::styled("t", Style::default().fg(Color::Cyan)),
                    Span::raw("=编辑标签 "),
                    Span::styled("h", Style::default().fg(Color::Cyan)),
                    Span::raw("=帮助 "),
                    Span::styled("↑↓", Style::default().fg(Color::Cyan)),
                    Span::raw("/"),
                    Span::styled("PgUp/PgDn", Style::default().fg(Color::Cyan)),
                    Span::raw("=首末"),
                ];
                if app.sync_total > 0 {
                    let queued = app.loading_sync.len();
                    let running = app.sync_running.len();
                    let completed = app.sync_total.saturating_sub(queued + running);
                    spans.push(Span::raw(" | "));
                    spans.push(Span::styled(
                        format!("{}{}/{}/{}", crate::i18n::current().tui.title_sync_progress, completed, running, app.sync_total),
                        Style::default().fg(Color::Yellow),
                    ));
                }
                Line::from(spans)
            }
        };
        let bottom_bar = Paragraph::new(bottom_text);
        frame.render_widget(bottom_bar, main_vertical[1]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn tag_spans(tags: &[String]) -> Vec<Span<'_>> {
    let palette = [
        Color::Magenta,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Cyan,
        Color::Red,
    ];
    let mut spans = Vec::new();
    for (i, tag) in tags.iter().enumerate() {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if i > 0 {
            spans.push(Span::raw(", "));
        }
        let color = palette[i % palette.len()];
        spans.push(Span::styled(tag, Style::default().fg(color).add_modifier(Modifier::BOLD)));
    }
    if spans.is_empty() {
        spans.push(Span::raw("(无)"));
    }
    spans
}

fn read_head_commit(path: &str) -> Option<String> {
    let repo = git2::Repository::open(path).ok()?;
    let head = repo.head().ok()?;
    let oid = head.target()?;
    Some(oid.to_string().chars().take(7).collect())
}

fn read_syncdone_info(path: &str) -> (String, String, String) {
    let default = || ("从未同步".to_string(), "—".to_string(), "—".to_string());

    let content = match std::fs::read_to_string(std::path::Path::new(path).join(".devbase").join("syncdone")) {
        Ok(c) => c,
        Err(_) => return default(),
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return default(),
    };

    let timestamp_str = match json.get("timestamp").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return default(),
    };
    let action = json.get("action").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let commit = json.get("local_commit").and_then(|v| v.as_str()).unwrap_or("—").to_string();
    let commit_short: String = commit.chars().take(7).collect();

    let dt = match chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        Ok(d) => d.with_timezone(&chrono::Utc),
        Err(_) => return default(),
    };
    let duration = chrono::Utc::now().signed_duration_since(dt);

    let human = if duration.num_seconds() < 60 {
        "刚刚".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}分钟前", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}小时前", duration.num_hours())
    } else if duration.num_days() < 7 {
        format!("{}天前", duration.num_days())
    } else {
        format!("{}周前", duration.num_days() / 7)
    };

    (human, action, commit_short)
}

fn read_repo_summary(repo_id: &str) -> Option<String> {
    let conn = crate::registry::WorkspaceRegistry::init_db().ok()?;
    conn.query_row(
        "SELECT summary FROM repo_summaries WHERE repo_id = ?1",
        [repo_id],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

fn format_log_line(line: &str) -> Line<'_> {
    let mut spans = Vec::new();

    // Extract timestamp prefix [HH:MM:SS]
    if let Some(ts_end) = line.find("] ") {
        let ts = &line[..ts_end + 2];
        spans.push(Span::styled(ts, Style::default().fg(Color::DarkGray)));

        let rest = &line[ts_end + 2..];
        if rest.starts_with("[ERROR] ") {
            spans.push(Span::styled(
                "[ERROR] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(&rest[8..]));
        } else if rest.starts_with("[WARN] ") {
            spans.push(Span::styled(
                "[WARN] ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(&rest[7..]));
        } else if rest.starts_with("[INFO] ") {
            spans.push(Span::styled("[INFO] ", Style::default().fg(Color::Green)));
            spans.push(Span::raw(&rest[7..]));
        } else {
            spans.push(Span::raw(rest));
        }
    } else {
        spans.push(Span::raw(line));
    }

    Line::from(spans)
}
