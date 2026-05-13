// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::tui::App;
use crate::tui::theme::Styles;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

/// Render the Session list (left panel) — contexts.
pub(crate) fn render_session_list(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|ctx| {
            let status_icon = match ctx.status.as_str() {
                "active" => "●",
                "archived" => "◌",
                _ => "?",
            };
            let status_color = match ctx.status.as_str() {
                "active" => Color::Green,
                "archived" => Color::Gray,
                _ => Color::Yellow,
            };
            let spans = vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(&ctx.id, styles.value.add_modifier(Modifier::BOLD)),
                Span::raw(" — "),
                Span::styled(&ctx.name, styles.dim),
            ];
            ListItem::new(Line::from(spans))
        })
        .collect();

    let title = format!(" Sessions ({}) ", app.sessions.len());
    let block = Block::default().title(title).borders(Borders::ALL).border_style(styles.border);

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default().bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut app.session_list_state);
}

/// Render the Session detail (right panel) — memories of selected context.
pub(crate) fn render_session_detail(frame: &mut Frame, app: &App, area: Rect, styles: &Styles) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Header: context name + intent
    let header_text = if let Some(ctx) = app.sessions.get(app.session_selected) {
        let intent = ctx.intent.as_deref().unwrap_or("no intent");
        format!("{} — {}", ctx.name, intent)
    } else {
        "No active context".to_string()
    };
    let header = Paragraph::new(header_text)
        .block(
            Block::default()
                .title(" Context ")
                .borders(Borders::ALL)
                .border_style(styles.border),
        )
        .style(styles.value);
    frame.render_widget(header, chunks[0]);

    // Memory list
    let mem_items: Vec<ListItem> = app
        .session_memories
        .iter()
        .map(|mem| {
            let type_icon = match mem.memory_type.as_str() {
                "decision" => "◆",
                "constraint" => "▪",
                "discovery" => "★",
                "error" => "✗",
                _ => "•",
            };
            let type_color = match mem.memory_type.as_str() {
                "decision" => Color::Cyan,
                "constraint" => Color::Yellow,
                "discovery" => Color::Green,
                "error" => Color::Red,
                _ => Color::Gray,
            };
            let indexed = mem.indexed_at.map(|_| " [indexed]").unwrap_or("");
            let model = mem.embedding_model.as_deref().unwrap_or("");
            let model_tag = if model.is_empty() {
                "".to_string()
            } else {
                format!(" ({}) ", model)
            };

            let spans = vec![
                Span::styled(format!("{} ", type_icon), Style::default().fg(type_color)),
                Span::styled(format!("[{}]", mem.memory_type), styles.dim),
                Span::raw(model_tag),
                Span::styled(indexed, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(&mem.content, styles.value),
            ];
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mem_block = Block::default()
        .title(format!(" Memories ({}) ", app.session_memories.len()))
        .borders(Borders::ALL)
        .border_style(styles.border);

    let mem_list = List::new(mem_items).block(mem_block);
    frame.render_widget(mem_list, chunks[1]);
}
