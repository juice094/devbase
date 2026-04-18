use crate::tui::{App, InputMode, SortMode, SyncPopupMode};
use crate::tui::render::ui;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{backend::Backend, Terminal};
use std::io;
use std::time::Duration;

pub(crate) enum TuiAction {
    Quit,
    LaunchExternal { cmd: String, cwd: String },
}

pub(crate) async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<TuiAction> {
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
                            KeyCode::Char('q') => return Ok(TuiAction::Quit),
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
                            KeyCode::Char('o') => {
                                app.sort_mode = match app.sort_mode {
                                    SortMode::Status => SortMode::Stars,
                                    SortMode::Stars => SortMode::Status,
                                };
                                if let Err(e) = app.load_repos() {
                                    app.log_error(crate::i18n::current().log.refresh_failed(e));
                                }
                            }
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Down => app.next(),
                            KeyCode::Up => app.previous(),
                            KeyCode::Home | KeyCode::PageUp => app.jump_to_top(),
                            KeyCode::End | KeyCode::PageDown => app.jump_to_bottom(),
                            KeyCode::Enter => {
                                let cwd = app.repos.get(app.selected).map(|r| r.local_path.clone());
                                if let Some(cwd) = cwd {
                                    let cmd = if which::which("gitui").is_ok() {
                                        Some("gitui")
                                    } else if which::which("lazygit").is_ok() {
                                        Some("lazygit")
                                    } else {
                                        app.log_warn(crate::i18n::current().log.external_tui_not_found.to_string());
                                        None
                                    };
                                    if let Some(cmd) = cmd {
                                        return Ok(TuiAction::LaunchExternal {
                                            cmd: cmd.to_string(),
                                            cwd,
                                        });
                                    }
                                }
                            }
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

