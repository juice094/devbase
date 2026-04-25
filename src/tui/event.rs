use crate::tui::render::ui;
use crate::tui::{
    App, InputMode, MainView, NLPPopupMode, SearchPopupMode, SkillPopupMode, SortMode,
    SyncPopupMode, WorkflowPopupMode,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use std::io;
use std::time::Duration;

pub(crate) enum TuiAction {
    Quit,
    LaunchExternal { cmd: String, cwd: String },
}

pub(crate) async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<TuiAction> {
    loop {
        terminal.draw(|f| ui(f, app)).map_err(|e| io::Error::other(e.to_string()))?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
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
                        KeyCode::Esc | KeyCode::Enter => {
                            app.sync_popup_mode = SyncPopupMode::Hidden
                        }
                        _ => {}
                    }
                    continue; // 弹窗显示时不处理其他按键
                }
                SyncPopupMode::Hidden => {}
            }
            // Skill popup intercepts when visible
            match app.skill_popup_mode {
                SkillPopupMode::List => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.skill_popup_mode = SkillPopupMode::Hidden;
                        }
                        KeyCode::Down => app.next_skill(),
                        KeyCode::Up => app.previous_skill(),
                        KeyCode::Home | KeyCode::PageUp => app.jump_to_top_skill(),
                        KeyCode::End | KeyCode::PageDown => app.jump_to_bottom_skill(),
                        KeyCode::Enter => {
                            if let Some(skill_item) = app.current_skill().cloned() {
                                let skill_md = std::path::PathBuf::from(&skill_item.row.local_path)
                                    .join("SKILL.md");
                                app.selected_skill =
                                    crate::skill_runtime::parser::parse_skill_md(&skill_md).ok();
                                app.skill_popup_mode = SkillPopupMode::Detail;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                SkillPopupMode::Detail => {
                    match key.code {
                        KeyCode::Esc => app.skill_popup_mode = SkillPopupMode::List,
                        KeyCode::Enter => {
                            let has_inputs = app
                                .selected_skill
                                .as_ref()
                                .map(|m| !m.inputs.is_empty())
                                .unwrap_or(false);
                            if has_inputs {
                                app.skill_param_buffer.clear();
                                app.skill_popup_mode = SkillPopupMode::ParamInput;
                            } else {
                                app.run_selected_skill();
                                app.skill_popup_mode = SkillPopupMode::Hidden;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                SkillPopupMode::ParamInput => {
                    match key.code {
                        KeyCode::Enter => {
                            app.run_selected_skill();
                            app.skill_param_buffer.clear();
                            app.skill_popup_mode = SkillPopupMode::Hidden;
                        }
                        KeyCode::Esc => {
                            app.skill_param_buffer.clear();
                            app.skill_popup_mode = SkillPopupMode::Detail;
                        }
                        KeyCode::Char(c) => app.skill_param_buffer.push(c),
                        KeyCode::Backspace => {
                            app.skill_param_buffer.pop();
                        }
                        _ => {}
                    }
                    continue;
                }
                SkillPopupMode::Result => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            app.skill_popup_mode = SkillPopupMode::Hidden;
                            app.skill_execution_result = None;
                        }
                        _ => {}
                    }
                    continue;
                }
                SkillPopupMode::Hidden => {}
            }
            // Workflow popup intercepts when visible
            match app.workflow_popup_mode {
                WorkflowPopupMode::List => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.workflow_popup_mode = WorkflowPopupMode::Hidden;
                        }
                        KeyCode::Down => app.next_workflow(),
                        KeyCode::Up => app.previous_workflow(),
                        KeyCode::Enter => {
                            if let Some(wf) = app.current_workflow().cloned() {
                                app.selected_workflow = Some(wf);
                                app.workflow_popup_mode = WorkflowPopupMode::Detail;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                WorkflowPopupMode::Detail => {
                    match key.code {
                        KeyCode::Esc => app.workflow_popup_mode = WorkflowPopupMode::List,
                        KeyCode::Enter | KeyCode::Char('r') => {
                            app.run_selected_workflow();
                            app.workflow_popup_mode = WorkflowPopupMode::Hidden;
                        }
                        _ => {}
                    }
                    continue;
                }
                WorkflowPopupMode::Result => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            app.workflow_popup_mode = WorkflowPopupMode::Hidden;
                            app.workflow_execution_result = None;
                            app.workflow_execution_error = None;
                        }
                        _ => {}
                    }
                    continue;
                }
                WorkflowPopupMode::Hidden => {}
            }
            // NLP query popup intercepts when visible
            match app.nlp_popup_mode {
                NLPPopupMode::Input => {
                    match key.code {
                        KeyCode::Enter => {
                            let query = app.nlp_query.trim().to_string();
                            if !query.is_empty() {
                                app.run_nlp_query(query);
                            }
                            app.nlp_popup_mode = NLPPopupMode::Hidden;
                        }
                        KeyCode::Esc => {
                            app.nlp_query.clear();
                            app.nlp_popup_mode = NLPPopupMode::Hidden;
                        }
                        KeyCode::Char(c) => app.nlp_query.push(c),
                        KeyCode::Backspace => {
                            app.nlp_query.pop();
                        }
                        _ => {}
                    }
                    continue;
                }
                NLPPopupMode::Results => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.nlp_popup_mode = NLPPopupMode::Hidden;
                            app.nlp_results.clear();
                            app.nlp_selected = 0;
                        }
                        KeyCode::Enter => {
                            app.run_nlp_selected_skill();
                            app.nlp_popup_mode = NLPPopupMode::Hidden;
                            app.nlp_results.clear();
                            app.nlp_selected = 0;
                        }
                        KeyCode::Down => {
                            if app.nlp_selected + 1 < app.nlp_results.len() {
                                app.nlp_selected += 1;
                            }
                        }
                        KeyCode::Up => {
                            if app.nlp_selected > 0 {
                                app.nlp_selected -= 1;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                NLPPopupMode::Hidden => {}
            }
            // Help popup intercepts keys when visible
            if app.help_popup_mode == crate::tui::HelpPopupMode::Visible {
                match key.code {
                    KeyCode::Esc
                    | KeyCode::Char('q')
                    | KeyCode::Char('h')
                    | KeyCode::Char('?')
                    | KeyCode::F(1) => {
                        app.toggle_help();
                    }
                    _ => {}
                }
                continue;
            }
            // Search popup intercepts keys when visible
            if app.search_popup_mode == SearchPopupMode::Results {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.search_popup_mode = SearchPopupMode::Hidden;
                        app.search_results.clear();
                        app.search_selected = 0;
                    }
                    KeyCode::Down => {
                        if app.search_selected + 1 < app.search_results.len() {
                            app.search_selected += 1;
                        }
                    }
                    KeyCode::Up => {
                        if app.search_selected > 0 {
                            app.search_selected -= 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(result) = app.search_results.get(app.search_selected) {
                            let _ = std::process::Command::new("code")
                                .arg(format!("{}:{}", result.file_path, result.line_number))
                                .spawn();
                        }
                    }
                    _ => {}
                }
                continue; // 弹窗显示时不处理其他按键
            }
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(TuiAction::Quit),
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.toggle_search_mode();
                        let mode_label = match app.search_mode {
                            crate::tui::SearchMode::Repo => {
                                crate::i18n::current().tui.search_mode_repo
                            }
                            crate::tui::SearchMode::Code => {
                                crate::i18n::current().tui.search_mode_code
                            }
                        };
                        app.log_info(format!("搜索模式已切换为: {}", mode_label));
                    }
                    KeyCode::Char('r') => {
                        app.log_info(crate::i18n::current().log.refreshing.to_string());
                        if let Err(e) = app.load_repos() {
                            app.log_error(crate::i18n::current().log.refresh_failed(e));
                        }
                        if let Err(e) = app.load_vaults() {
                            app.log_error(format!("刷新 Vault 失败: {}", e));
                        }
                    }
                    KeyCode::Char('s') => app.fetch_all_and_preview(),
                    KeyCode::Char('S') => app.start_safe_sync(),
                    KeyCode::Char('t') => {
                        app.input_mode = InputMode::TagInput;
                        app.input_buffer.clear();
                    }
                    KeyCode::Char('/') => {
                        app.input_mode = InputMode::SearchInput;
                        app.input_buffer.clear();
                        app.search_popup_mode = SearchPopupMode::Input;
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
                    KeyCode::Char('k') => {
                        app.load_skills();
                        app.skill_popup_mode = SkillPopupMode::List;
                    }
                    KeyCode::Char('w') => {
                        app.load_workflows();
                        app.workflow_popup_mode = WorkflowPopupMode::List;
                    }
                    KeyCode::Char(':') => {
                        app.nlp_query.clear();
                        app.nlp_popup_mode = NLPPopupMode::Input;
                    }
                    KeyCode::Char('h') | KeyCode::Char('?') => app.toggle_help(),
                    KeyCode::F(1) => app.toggle_help(),
                    KeyCode::Tab => app.toggle_main_view(),
                    KeyCode::BackTab => app.toggle_main_view(),
                    KeyCode::Down => app.next(),
                    KeyCode::Up => app.previous(),
                    KeyCode::Home | KeyCode::PageUp => app.jump_to_top(),
                    KeyCode::End | KeyCode::PageDown => app.jump_to_bottom(),
                    KeyCode::Enter => match app.main_view {
                        MainView::RepoList => {
                            let cwd = app.repos.get(app.selected).map(|r| r.local_path.clone());
                            if let Some(cwd) = cwd {
                                let cmd = if which::which("gitui").is_ok() {
                                    Some("gitui")
                                } else if which::which("lazygit").is_ok() {
                                    Some("lazygit")
                                } else {
                                    app.log_warn(
                                        crate::i18n::current()
                                            .log
                                            .external_tui_not_found
                                            .to_string(),
                                    );
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
                        MainView::VaultList => {
                            if let Some(vault) = app.current_vault() {
                                let path = vault.path.clone();
                                let _ = std::process::Command::new("code").arg(&path).spawn();
                            }
                        }
                    },
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
                InputMode::SearchInput => match key.code {
                    KeyCode::Enter => {
                        let pattern = app.input_buffer.trim().to_string();
                        if !pattern.is_empty() {
                            app.search_pattern = pattern.clone();
                            app.execute_search();
                            app.search_popup_mode = SearchPopupMode::Results;
                        }
                        app.input_mode = InputMode::Normal;
                        app.input_buffer.clear();
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.input_buffer.clear();
                        app.search_popup_mode = SearchPopupMode::Hidden;
                        app.search_results.clear();
                        app.search_selected = 0;
                    }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    _ => {}
                },
            }
        }

        while let Ok(notification) = app.async_rx.try_recv() {
            app.update_async(notification);
        }
    }
}
