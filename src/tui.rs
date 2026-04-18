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
    // Fetch preview cache
    local_commit: Option<String>,
    remote_commit: Option<String>,
    last_preview_branch: Option<String>,
    last_preview_ahead: Option<usize>,
    last_preview_behind: Option<usize>,
    last_preview_synced: Option<bool>,
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
    fetch_preview_job: crate::asyncgit::AsyncSingleJob<crate::asyncgit::AsyncFetchPreview>,
    loading_repo_status: HashSet<String>,
    loading_preview: HashSet<String>,
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
        let fetch_preview_job = crate::asyncgit::AsyncSingleJob::new(async_tx.clone());

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
            fetch_preview_job,
            loading_repo_status: HashSet::new(),
            loading_preview: HashSet::new(),
            loading_sync: HashSet::new(),
            sync_orchestrator: crate::sync::SyncOrchestrator::new(4),
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
                local_commit: None,
                remote_commit: None,
                last_preview_branch: None,
                last_preview_ahead: None,
                last_preview_behind: None,
                last_preview_synced: None,
            });
        }
        // Sort: group by primary tag, then by id
        self.repos.sort_by(|a, b| {
            let tag_a = a.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
            let tag_b = b.tags.first().map(|s| s.as_str()).unwrap_or("zzz");
            let tag_cmp = tag_a.cmp(tag_b);
            if tag_cmp != std::cmp::Ordering::Equal {
                return tag_cmp;
            }
            a.id.cmp(&b.id)
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

    fn sync_preview(&mut self) {
        let repo = match self.current_repo().cloned() {
            Some(r) => r,
            None => {
                self.log_warn(crate::i18n::current().log.no_repo_selected.to_string());
                return;
            }
        };

        self.log_info(crate::i18n::current().log.fetching_preview(&repo.id));
        self.loading_preview.insert(repo.id.clone());

        self.fetch_preview_job
            .spawn(crate::asyncgit::AsyncFetchPreview {
                repo_id: repo.id,
                local_path: repo.local_path,
                upstream_url: repo.upstream_url,
                default_branch: repo.default_branch,
            });
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
            AsyncNotification::FetchPreview(n) => {
                self.loading_preview.remove(&n.repo_id);
                self.log_info(n.msg);
                if let Some(repo) = self.repos.iter_mut().find(|r| r.id == n.repo_id) {
                    repo.local_commit = n.local_commit;
                    repo.remote_commit = n.remote_commit;
                    repo.last_preview_branch = n.branch;
                    repo.last_preview_ahead = n.ahead;
                    repo.last_preview_behind = n.behind;
                    repo.last_preview_synced = n.is_synced;
                }
            }
            AsyncNotification::SyncProgress(n) => {
                if n.action == "RUNNING" {
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
                ahead,
                behind,
            });
        }

        if self.sync_preview_items.is_empty() {
            self.sync_popup_results.push(("system".to_string(), "No repositories eligible for safe sync.".to_string()));
            self.sync_popup_mode = SyncPopupMode::Progress;
        }
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
                            KeyCode::Char('s') => app.sync_preview(),
                            KeyCode::Char('S') => app.safe_sync_preview(),
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

    // Left: repo list (sorted by primary tag for implicit clustering)
    let items: Vec<ListItem> = app
        .repos
        .iter()
        .map(|repo| {
            let mut prefix = String::new();
            if repo.upstream_url.is_some() {
                prefix.push_str("🔗 ");
            } else {
                prefix.push_str("📁 ");
            }
            if app.loading_repo_status.contains(&repo.id)
                || app.loading_preview.contains(&repo.id)
                || app.loading_sync.contains(&repo.id)
            {
                prefix.push_str("⏳ ");
            }

            let base_fg = if repo.upstream_url.is_some() {
                Color::Cyan
            } else {
                Color::Yellow
            };

            let fg = if app.loading_repo_status.contains(&repo.id)
                || app.loading_preview.contains(&repo.id)
                || app.loading_sync.contains(&repo.id)
            {
                Color::LightCyan
            } else {
                base_fg
            };

            // Tag cluster indicator: show primary tag in muted color
            let tag_indicator = if let Some(first_tag) = repo.tags.first() {
                format!(" [{}]", first_tag)
            } else {
                String::new()
            };

            ListItem::new(Span::styled(
                format!("{}{}{}", prefix, repo.id, tag_indicator),
                Style::default().fg(fg),
            ))
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
            Span::styled("本地标签: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ];
        tag_line.extend(tag_spans(&repo.tags));

        let status_text = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
            (Some(d), Some(a), Some(b)) => format!("未提交={} 超前={} 落后={}", d, a, b),
            _ => crate::i18n::current().tui.status_loading.to_string(),
        };

        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled(crate::i18n::current().tui.label_id, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(&repo.id),
            ]),
            Line::from(vec![
                Span::styled(crate::i18n::current().tui.label_path, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(&repo.local_path),
            ]),
            Line::from(vec![
                Span::styled(crate::i18n::current().tui.label_branch, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(repo.default_branch.as_deref().unwrap_or(crate::i18n::current().tui.status_unknown)),
            ]),
            Line::from(tag_line),
            Line::from(vec![
                Span::styled(crate::i18n::current().tui.label_language, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(repo.language.as_deref().unwrap_or("—")),
            ]),
            Line::from(vec![
                Span::styled(crate::i18n::current().tui.label_upstream, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(
                    repo.upstream_url.as_deref().unwrap_or("(无)"),
                    if repo.upstream_url.is_some() {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Yellow)
                    },
                ),
            ]),
            Line::from(vec![
                Span::styled(crate::i18n::current().tui.label_status, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(status_text),
            ]),
        ];

        // Commit comparison line (from fetch preview)
        if let (Some(local), Some(remote)) = (&repo.local_commit, &repo.remote_commit) {
            let local_short = &local[..local.len().min(7)];
            let remote_short = &remote[..remote.len().min(7)];
            let synced = repo.last_preview_synced.unwrap_or(false);
            let cmp_text = if synced {
                format!("{} == {} ✓", local_short, remote_short)
            } else {
                format!("{} ≠ {}", local_short, remote_short)
            };
            let cmp_color = if synced { Color::Green } else { Color::Yellow };
            lines.push(Line::from(vec![
                Span::styled("版本对比: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(cmp_text, Style::default().fg(cmp_color)),
            ]));
            if let Some(branch) = &repo.last_preview_branch {
                lines.push(Line::from(vec![
                    Span::styled("对比分支: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(branch),
                ]));
            }
        } else if repo.upstream_url.is_some() {
            lines.push(Line::from(vec![
                Span::styled("版本对比: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("按 s 获取远程版本对比", Style::default().fg(Color::DarkGray)),
            ]));
        }

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
            let safe: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::Safe).collect();
            let diverged: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::BlockedDiverged).collect();
            let dirty: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::BlockedDirty).collect();
            let up_to_date: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::UpToDate).collect();
            let no_upstream: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::NoUpstream).collect();
            let unknown: Vec<_> = app.sync_preview_items.iter().filter(|i| i.safety == crate::sync::SyncSafety::Unknown).collect();

            if !safe.is_empty() {
                lines.push(Line::from(Span::styled(format!("将执行 ({})", safe.len()), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
                for item in safe {
                    lines.push(Line::from(format!("  [{}] behind={}", item.repo_id, item.behind)));
                }
                lines.push(Line::from(""));
            }
            if !diverged.is_empty() {
                lines.push(Line::from(Span::styled(format!("被阻塞 - 分叉 ({})", diverged.len()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                for item in diverged {
                    lines.push(Line::from(format!("  [{}] ahead={} behind={}", item.repo_id, item.ahead, item.behind)));
                }
                lines.push(Line::from(""));
            }
            if !dirty.is_empty() {
                lines.push(Line::from(Span::styled(format!("被阻塞 - 工作目录不干净 ({})", dirty.len()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                for item in dirty {
                    lines.push(Line::from(format!("  [{}]", item.repo_id)));
                }
                lines.push(Line::from(""));
            }
            if !up_to_date.is_empty() {
                lines.push(Line::from(Span::styled(format!("已最新 ({})", up_to_date.len()), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))));
                for item in up_to_date {
                    lines.push(Line::from(format!("  [{}]", item.repo_id)));
                }
                lines.push(Line::from(""));
            }
            if !no_upstream.is_empty() {
                lines.push(Line::from(Span::styled(format!("无远程 ({})", no_upstream.len()), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))));
                for item in no_upstream {
                    lines.push(Line::from(format!("  [{}]", item.repo_id)));
                }
                lines.push(Line::from(""));
            }
            if !unknown.is_empty() {
                lines.push(Line::from(Span::styled(format!("异常 ({})", unknown.len()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                for item in unknown {
                    lines.push(Line::from(format!("  [{}]", item.repo_id)));
                }
                lines.push(Line::from(""));
            }

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
