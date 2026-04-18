use crate::tui::{App, InputMode, SortMode, SyncPopupMode, SearchPopupMode};
use ratatui::{{
    layout::{{Constraint, Direction, Layout}},
    style::{{Color, Modifier, Style}},
    text::{{Line, Span, Text}},
    widgets::{{Block, Borders, List, ListItem, Paragraph, Wrap}},
    Frame,
}};

pub(crate) fn ui(frame: &mut Frame, app: &mut App) {
    let bottom_height = if app.show_help
        || app.input_mode == InputMode::TagInput
        || app.input_mode == InputMode::SearchInput
        || app.search_popup_mode != SearchPopupMode::Hidden
    {
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

            // Star count indicator
            let star_indicator = if let Some(stars) = repo.stars {
                format!(" ★{}", stars)
            } else {
                String::new()
            };

            // Tag cluster indicator: show primary tag in muted color
            let tag_indicator = if let Some(first_tag) = repo.tags.first() {
                format!(" [{}]", first_tag)
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, repo.id), Style::default().fg(fg)),
                Span::styled(star_indicator, Style::default().fg(Color::Rgb(255, 215, 0))),
                Span::styled(status_suffix, Style::default().fg(Color::DarkGray)),
                Span::styled(tag_indicator, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list_title = format!(
        "{} [{}]",
        crate::i18n::current().tui.title_repos,
        match app.sort_mode {
            SortMode::Status => crate::i18n::current().tui.sort_status,
            SortMode::Stars => "Stars",
        }
    );

    let list = if items.is_empty() {
        let onboarding = vec![
            ListItem::new(Line::from(Span::styled("", Style::default()))),
            ListItem::new(Line::from(vec![
                Span::styled("  还没有注册任何仓库", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ])),
            ListItem::new(Line::from(Span::styled("", Style::default()))),
            ListItem::new(Line::from(vec![
                Span::styled("  运行: ", Style::default().fg(Color::DarkGray)),
                Span::styled("devbase scan <路径> --register", Style::default().fg(Color::Cyan)),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("  示例: ", Style::default().fg(Color::DarkGray)),
                Span::styled("devbase scan . --register", Style::default().fg(Color::Green)),
            ])),
        ];
        List::new(onboarding)
            .block(Block::default().borders(Borders::ALL).title(list_title))
    } else {
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(list_title))
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 80))
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ")
    };

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
            crate::sync::SyncPolicy::Mirror => Color::Red,
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
            if policy == crate::sync::SyncPolicy::Mirror {
                Line::from(vec![
                    Span::styled("  ⚠ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(crate::i18n::current().sync.mirror_policy_warning, Style::default().fg(Color::Red)),
                ])
            } else {
                Line::from("")
            },
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

    // Search popup
    match app.search_popup_mode {
        SearchPopupMode::Input => {
            let input_area = ratatui::layout::Rect {
                x: 0,
                y: frame.area().height.saturating_sub(1),
                width: frame.area().width,
                height: 1,
            };
            let input_text = Line::from(vec![
                Span::styled("/", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(&app.input_buffer),
            ]);
            frame.render_widget(Paragraph::new(input_text), input_area);
        }
        SearchPopupMode::Results => {
            let popup_area = centered_rect(80, 70, frame.area());
            let popup_inner = popup_area.inner(ratatui::layout::Margin {
                horizontal: 1,
                vertical: 1,
            });

            let title = if app.search_results.is_empty() {
                format!("{}: \"{}\" - {}", crate::i18n::current().tui.search_results_title, app.search_pattern, crate::i18n::current().tui.search_no_results)
            } else {
                format!("{}: \"{}\" ({} results)", crate::i18n::current().tui.search_results_title, app.search_pattern, app.search_results.len())
            };

            let items: Vec<ListItem> = app.search_results.iter().enumerate().map(|(i, result)| {
                let is_selected = i == app.search_selected;
                let repo_line = Span::styled(
                    format!("[{}] {}:{}", result.repo_id, result.file_path, result.line_number),
                    Style::default().fg(Color::Cyan),
                );
                let content_line = Span::styled(
                    format!("  > {}", result.line_content),
                    if is_selected {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                );
                ListItem::new(Text::from(vec![
                    Line::from(repo_line),
                    Line::from(content_line),
                ]))
            }).collect();

            let popup_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(40, 40, 80))
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            frame.render_widget(ratatui::widgets::Clear, popup_area);
            frame.render_widget(popup_list, popup_area);

            let hint = Paragraph::new(Span::styled(
                crate::i18n::current().tui.hint_search_results,
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
        SearchPopupMode::Hidden => {}
    }

    // Sync popup
    match app.sync_popup_mode {
        SyncPopupMode::Preview => {
            let popup_area = centered_rect(60, 50, frame.area());
            let popup_inner = popup_area.inner(ratatui::layout::Margin {
                horizontal: 1,
                vertical: 1,
            });

            let mut lines: Vec<Line> = Vec::new();

            if app.dry_run {
                lines.push(Line::from(Span::styled(
                    crate::i18n::current().sync.dry_run_badge,
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
            }

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
                let i18n = &crate::i18n::current().tui;
                let sort_label = match app.sort_mode {
                    SortMode::Status => i18n.sort_status,
                    SortMode::Stars => "Stars",
                };
                let mut spans = vec![
                    Span::styled("q", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={} ", i18n.help_quit)),
                    Span::styled("r", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={} ", i18n.help_refresh)),
                    Span::styled("s", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={} ", i18n.help_preview)),
                    Span::styled("S", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={} ", i18n.help_batch)),
                    Span::styled("t", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={} ", i18n.help_tag)),
                    Span::styled("o", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={}({}) ", i18n.help_sort, sort_label)),
                    Span::styled("h", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={} ", i18n.help_help)),
                    Span::styled("↑↓", Style::default().fg(Color::Cyan)),
                    Span::raw("/"),
                    Span::styled("PgUp/PgDn", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("={}", i18n.help_navigate)),
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
            InputMode::SearchInput => Line::from(vec![
                Span::styled(crate::i18n::current().tui.search_prompt, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::raw(&app.input_buffer),
                Span::styled(crate::i18n::current().tui.hint_tag_input, Style::default().fg(Color::DarkGray)),
            ]),
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
