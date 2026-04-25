use crate::tui::layout::AppLayout;
use crate::tui::theme::Styles;
use crate::tui::{App, SearchPopupMode, SkillPopupMode, SyncPopupMode, WorkflowPopupMode};
use ratatui::{
    Frame,
    layout::Margin,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub(crate) fn render_popups(frame: &mut Frame, app: &mut App, styles: &Styles) {
    match app.search_popup_mode {
        SearchPopupMode::Input => render_search_input(frame, app, styles),
        SearchPopupMode::Results => render_search_results(frame, app, styles),
        SearchPopupMode::Hidden => {}
    }

    match app.sync_popup_mode {
        SyncPopupMode::Preview => render_sync_preview(frame, app, styles),
        SyncPopupMode::Progress => render_sync_progress(frame, app, styles),
        SyncPopupMode::Hidden => {}
    }

    match app.skill_popup_mode {
        SkillPopupMode::List => render_skill_list(frame, app, styles),
        SkillPopupMode::Detail => render_skill_detail(frame, app, styles),
        SkillPopupMode::ParamInput => render_skill_param_input(frame, app, styles),
        SkillPopupMode::Result => render_skill_result(frame, app, styles),
        SkillPopupMode::Hidden => {}
    }

    match app.workflow_popup_mode {
        WorkflowPopupMode::List => render_workflow_list(frame, app, styles),
        WorkflowPopupMode::Detail => render_workflow_detail(frame, app, styles),
        WorkflowPopupMode::Hidden => {}
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

fn render_search_input(frame: &mut Frame, app: &App, styles: &Styles) {
    let area = ratatui::layout::Rect {
        x: 0,
        y: frame.area().height.saturating_sub(1),
        width: frame.area().width,
        height: 1,
    };
    let i18n = crate::i18n::current();
    let mode_label = match app.search_mode {
        crate::tui::SearchMode::Repo => i18n.tui.search_mode_repo,
        crate::tui::SearchMode::Code => i18n.tui.search_mode_code,
    };
    let input_text = Line::from(vec![
        Span::styled(
            format!("[{}] /", mode_label),
            Style::default().fg(styles.theme.warning).add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.input_buffer),
    ]);
    frame.render_widget(Paragraph::new(input_text), area);
}

fn render_search_results(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 80, 70);
    let popup_inner = popup_area.inner(Margin::new(1, 1));
    let i18n = crate::i18n::current();

    let title = if app.search_results.is_empty() {
        format!(
            "{}: \"{}\" - {}",
            i18n.tui.search_results_title, app.search_pattern, i18n.tui.search_no_results
        )
    } else {
        format!(
            "{}: \"{}\" ({} results)",
            i18n.tui.search_results_title,
            app.search_pattern,
            app.search_results.len()
        )
    };

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let is_selected = i == app.search_selected;
            let repo_line = Span::styled(
                format!("[{}] {}:{}", result.repo_id, result.file_path, result.line_number),
                Style::default().fg(styles.theme.primary),
            );
            let content_line = Span::styled(
                format!("  > {}", result.line_content),
                if is_selected {
                    Style::default().fg(styles.theme.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            );
            ListItem::new(Text::from(vec![Line::from(repo_line), Line::from(content_line)]))
        })
        .collect();

    let popup_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title).border_style(styles.border))
        .highlight_style(styles.highlight)
        .highlight_symbol("> ");

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_list, popup_area);

    let hint = Paragraph::new(Span::styled(i18n.tui.hint_search_results, styles.hint));
    let hint_height = 1;
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(hint_height),
        width: popup_inner.width,
        height: hint_height,
    };
    frame.render_widget(hint, hint_area);
}

// ---------------------------------------------------------------------------
// Skill Panel
// ---------------------------------------------------------------------------

fn render_skill_list(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 70, 60);
    let popup_inner = popup_area.inner(Margin::new(1, 1));
    let i18n = crate::i18n::current();

    let title = format!("{} ({})", i18n.tui.title_skills, app.skills.len());
    let items: Vec<ListItem> = app
        .skills
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = i == app.skill_selected;
            let cat_tag = s.row.category.as_deref().unwrap_or("uncategorized");
            let line = Span::styled(
                format!("[{}] {} [{}] — {}", s.row.id, s.row.name, cat_tag, s.row.description),
                if is_selected {
                    Style::default().fg(styles.theme.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            );
            ListItem::new(Line::from(line))
        })
        .collect();

    let popup_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title).border_style(styles.border))
        .highlight_style(styles.highlight)
        .highlight_symbol("> ");

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_list, popup_area);

    let hint = Paragraph::new(Span::styled(
        "[↑/↓] 选择  [Enter] 详情/执行  [Esc] 关闭",
        styles.hint,
    ));
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(1),
        width: popup_inner.width,
        height: 1,
    };
    frame.render_widget(hint, hint_area);
}

fn render_skill_detail(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 60, 50);
    let popup_inner = popup_area.inner(Margin::new(1, 1));
    let i18n = crate::i18n::current();

    let mut lines: Vec<Line> = Vec::new();
    if let Some(skill) = &app.selected_skill {
        lines.push(Line::from(Span::styled(
            format!("{} v{}", skill.name, skill.version),
            Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(&skill.description, Style::default().fg(styles.theme.text))));
        lines.push(Line::from(vec![
            Span::styled("Category: ", Style::default().fg(styles.theme.info)),
            Span::styled(skill.category.as_deref().unwrap_or("uncategorized"), Style::default().fg(styles.theme.text)),
            Span::raw("  |  "),
            Span::styled("Type: ", Style::default().fg(styles.theme.info)),
            Span::styled(skill.skill_type.as_str(), Style::default().fg(styles.theme.text)),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Inputs:",
            Style::default().fg(styles.theme.info).add_modifier(Modifier::BOLD),
        )));
        if skill.inputs.is_empty() {
            lines.push(Line::from(Span::styled(i18n.tui.skill_no_params, Style::default().fg(Color::Gray))));
        } else {
            for inp in &skill.inputs {
                let required = if inp.required { "(required)" } else { "" };
                lines.push(Line::from(vec![
                    Span::raw(format!("  • {} ({}) ", inp.name, inp.input_type)),
                    Span::styled(required, Style::default().fg(styles.theme.warning)),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("    {}", inp.description),
                    Style::default().fg(Color::Gray),
                )));
            }
        }
    } else {
        lines.push(Line::from(Span::styled("无法加载 Skill 详情", Style::default().fg(styles.theme.danger))));
    }

    let popup_text = Text::from(lines);
    let popup_para = Paragraph::new(popup_text)
        .block(Block::default().borders(Borders::ALL).title("Skill Detail").border_style(styles.border))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_para, popup_area);

    let hint = Paragraph::new(Span::styled(
        "[Enter] 执行  [Esc] 返回列表",
        styles.hint,
    ));
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(1),
        width: popup_inner.width,
        height: 1,
    };
    frame.render_widget(hint, hint_area);
}

fn render_skill_param_input(frame: &mut Frame, app: &App, styles: &Styles) {
    let area = ratatui::layout::Rect {
        x: 0,
        y: frame.area().height.saturating_sub(1),
        width: frame.area().width,
        height: 1,
    };
    let i18n = crate::i18n::current();
    let input_text = Line::from(vec![
        Span::styled(
            format!("[{}] ", i18n.tui.skill_result_title),
            Style::default().fg(styles.theme.warning).add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.skill_param_buffer),
        Span::styled(i18n.tui.hint_skill_params, styles.hint),
    ]);
    frame.render_widget(Paragraph::new(input_text), area);
}

fn render_skill_result(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 80, 70);
    let popup_inner = popup_area.inner(Margin::new(1, 1));
    let i18n = crate::i18n::current();

    let mut lines: Vec<Line> = Vec::new();
    if let Some(result) = &app.skill_execution_result {
        let status_label = match result.status {
            crate::skill_runtime::ExecutionStatus::Success => ("✓ Success", styles.theme.success),
            crate::skill_runtime::ExecutionStatus::Failed => ("✗ Failed", styles.theme.danger),
            crate::skill_runtime::ExecutionStatus::Timeout => ("⏱ Timeout", styles.theme.warning),
            _ => ("… Running", styles.theme.warning),
        };
        lines.push(Line::from(Span::styled(
            format!("{} | exit_code={:?} | {}ms",
                status_label.0, result.exit_code, result.duration_ms),
            Style::default().fg(status_label.1).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        if !result.stdout.is_empty() {
            lines.push(Line::from(Span::styled("stdout:", Style::default().fg(styles.theme.info).add_modifier(Modifier::BOLD))));
            for line in result.stdout.lines() {
                lines.push(Line::from(Span::raw(line)));
            }
        }
        if !result.stderr.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("stderr:", Style::default().fg(styles.theme.danger).add_modifier(Modifier::BOLD))));
            for line in result.stderr.lines() {
                lines.push(Line::from(Span::raw(line)));
            }
        }
    } else {
        lines.push(Line::from(Span::styled("No result available", Style::default().fg(Color::Gray))));
    }

    let popup_text = Text::from(lines);
    let popup_para = Paragraph::new(popup_text)
        .block(Block::default().borders(Borders::ALL).title(i18n.tui.skill_result_title).border_style(styles.border))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_para, popup_area);

    let hint = Paragraph::new(Span::styled(i18n.tui.hint_popup_close, styles.hint));
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(1),
        width: popup_inner.width,
        height: 1,
    };
    frame.render_widget(hint, hint_area);
}

// ---------------------------------------------------------------------------
// Sync Preview
// ---------------------------------------------------------------------------

fn render_sync_preview(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 60, 50);
    let popup_inner = popup_area.inner(Margin::new(1, 1));
    let i18n = crate::i18n::current();

    let mut lines: Vec<Line> = Vec::new();

    if app.dry_run {
        lines.push(Line::from(Span::styled(i18n.sync.dry_run_badge, styles.dry_run)));
        lines.push(Line::from(""));
    }

    // If preview items are empty but popup results exist, we're in fetch-progress mode
    if app.sync_preview_items.is_empty() && !app.sync_popup_results.is_empty() {
        lines.push(Line::from(Span::styled(
            "正在获取远程状态...",
            Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        for (repo_id, msg) in &app.sync_popup_results {
            if repo_id == "system" {
                continue;
            }
            let color = if msg.contains("Fetched") {
                styles.theme.success
            } else if msg.contains("Error") || msg.contains("TIMEOUT") {
                styles.theme.danger
            } else {
                styles.theme.warning
            };
            lines.push(Line::from(vec![
                Span::raw(format!("  [{}] ", repo_id)),
                Span::styled(msg, Style::default().fg(color)),
            ]));
        }

        let popup_text = Text::from(lines);
        let popup_para = Paragraph::new(popup_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Safe Sync Preview")
                    .border_style(styles.border),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup_para, popup_area);
        return;
    }

    let safe: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::Safe)
        .collect();
    let diverged: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::BlockedDiverged)
        .collect();
    let dirty: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::BlockedDirty)
        .collect();
    let local_ahead: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::LocalAhead)
        .collect();
    let up_to_date: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::UpToDate)
        .collect();
    let no_upstream: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::NoUpstream)
        .collect();
    let unknown: Vec<_> = app
        .sync_preview_items
        .iter()
        .filter(|i| i.safety == crate::sync::SyncSafety::Unknown)
        .collect();

    append_group(&mut lines, "将执行", &safe, styles.theme.success, styles);
    append_group(&mut lines, "被阻塞 - 分叉", &diverged, styles.theme.danger, styles);
    append_group(&mut lines, "被阻塞 - 工作目录不干净", &dirty, styles.theme.danger, styles);
    append_group(&mut lines, "本地超前 - 将推送", &local_ahead, styles.theme.warning, styles);
    append_group(&mut lines, "已最新", &up_to_date, styles.theme.muted, styles);
    append_group(&mut lines, "无远程", &no_upstream, styles.theme.muted, styles);
    append_group(&mut lines, "异常", &unknown, styles.theme.danger, styles);

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "注：基于本地缓存评估，同步前会重新获取远程状态",
        styles.hint,
    )));

    let popup_text = Text::from(lines);
    let popup_para = Paragraph::new(popup_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Safe Sync Preview")
                .border_style(styles.border),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_para, popup_area);

    let hint = Paragraph::new(Span::styled("[Enter] 确认执行  [Esc] 取消", styles.hint));
    let hint_height = 1;
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(hint_height),
        width: popup_inner.width,
        height: hint_height,
    };
    frame.render_widget(hint, hint_area);
}

fn append_group<'a>(
    lines: &mut Vec<Line<'a>>,
    label: &str,
    items: &[&crate::tui::SyncPreviewItem],
    color: Color,
    _styles: &Styles,
) {
    if items.is_empty() {
        return;
    }
    lines.push(Line::from(Span::styled(
        format!("{} ({})", label, items.len()),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )));
    for item in items {
        let detail = if item.ahead > 0 && item.behind > 0 {
            format!(
                "  [{}] {:?} ahead={} behind={}",
                item.repo_id, item.policy, item.ahead, item.behind
            )
        } else if item.behind > 0 {
            format!("  [{}] {:?} behind={}", item.repo_id, item.policy, item.behind)
        } else if item.ahead > 0 {
            format!("  [{}] {:?} ahead={}", item.repo_id, item.policy, item.ahead)
        } else {
            format!("  [{}] {:?}", item.repo_id, item.policy)
        };
        lines.push(Line::from(detail));
        if let Some(rec) = &item.recommendation {
            lines.push(Line::from(Span::styled(
                format!("    → {}", rec),
                Style::default().fg(color),
            )));
        }
    }
    lines.push(Line::from(""));
}

// ---------------------------------------------------------------------------
// Sync Progress
// ---------------------------------------------------------------------------

fn render_sync_progress(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 60, 40);
    let popup_inner = popup_area.inner(Margin::new(1, 1));
    let i18n = crate::i18n::current();

    let queued = app.loading_sync.len();
    let running = app.sync_running.len();
    let completed = app.sync_total.saturating_sub(queued + running);
    let elapsed_secs = app.sync_start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);

    let popup_title = Line::from(vec![
        Span::raw(i18n.tui.title_sync_progress),
        Span::raw(" | "),
        Span::styled(format!("{}{}", completed, i18n.tui.sync_done), styles.log_info),
        Span::styled(
            format!("{}{}", running, i18n.tui.sync_running),
            Style::default().fg(styles.theme.warning),
        ),
        Span::styled(format!("{}{}", queued, i18n.tui.sync_queued), styles.dim),
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
            let is_pending =
                message == i18n.log.status_queued || message == i18n.sync.status_running;
            let color = if is_error {
                styles.theme.danger
            } else if is_pending {
                styles.theme.warning
            } else {
                styles.theme.success
            };
            ListItem::new(Span::styled(
                format!("[{}] {}", repo_id, message),
                Style::default().fg(color),
            ))
        })
        .collect();

    let popup_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(popup_title)
            .border_style(styles.border),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_list, popup_area);

    let hint = Paragraph::new(Span::styled(i18n.tui.hint_popup_close, styles.hint));
    let hint_height = 1;
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(hint_height),
        width: popup_inner.width,
        height: hint_height,
    };
    frame.render_widget(hint, hint_area);
}


fn render_workflow_list(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 70, 60);
    let popup_inner = popup_area.inner(Margin::new(1, 1));

    let title = format!("Workflows ({})", app.workflows.len());
    let items: Vec<ListItem> = app
        .workflows
        .iter()
        .enumerate()
        .map(|(i, wf)| {
            let is_selected = i == app.workflow_selected;
            let line = Span::styled(
                format!("[{}] {} — {}", wf.id, wf.name, wf.description.as_deref().unwrap_or("").chars().take(60).collect::<String>()),
                if is_selected {
                    Style::default().fg(styles.theme.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            );
            ListItem::new(Line::from(line))
        })
        .collect();

    let popup_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title).border_style(styles.border))
        .highlight_style(styles.highlight)
        .highlight_symbol("> ");

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_list, popup_area);

    let hint = Paragraph::new(Span::styled(
        "[↑/↓] 选择  [Enter] 详情  [Esc] 关闭",
        styles.hint,
    ));
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(1),
        width: popup_inner.width,
        height: 1,
    };
    frame.render_widget(hint, hint_area);
}

fn render_workflow_detail(frame: &mut Frame, app: &App, styles: &Styles) {
    let popup_area = AppLayout::centered(frame.area(), 60, 50);
    let popup_inner = popup_area.inner(Margin::new(1, 1));

    let mut lines: Vec<Line> = Vec::new();
    if let Some(wf) = &app.selected_workflow {
        lines.push(Line::from(Span::styled(
            format!("{} v{}", wf.name, wf.version),
            Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD),
        )));
        if let Some(desc) = &wf.description {
            lines.push(Line::from(Span::styled(desc, Style::default().fg(styles.theme.text))));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Steps:",
            Style::default().fg(styles.theme.info).add_modifier(Modifier::BOLD),
        )));
        for step in &wf.steps {
            let deps = if step.depends_on.is_empty() {
                "".to_string()
            } else {
                format!(" [→ {}]", step.depends_on.join(", "))
            };
            lines.push(Line::from(Span::styled(
                format!("  • {}{}", step.id, deps),
                Style::default().fg(styles.theme.text),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled("无法加载 Workflow 详情", Style::default().fg(styles.theme.danger))));
    }

    let popup_text = Text::from(lines);
    let popup_para = Paragraph::new(popup_text)
        .block(Block::default().borders(Borders::ALL).title("Workflow Detail").border_style(styles.border))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, popup_area);
    frame.render_widget(popup_para, popup_area);

    let hint = Paragraph::new(Span::styled(
        "[Enter] 关闭  [Esc] 返回列表",
        styles.hint,
    ));
    let hint_area = ratatui::layout::Rect {
        x: popup_inner.x,
        y: popup_inner.y + popup_inner.height.saturating_sub(1),
        width: popup_inner.width,
        height: 1,
    };
    frame.render_widget(hint, hint_area);
}
