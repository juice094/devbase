use crate::tui::theme::Styles;
use crate::tui::{App, MainView, SortMode};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub(crate) fn render_list(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    match app.main_view {
        MainView::RepoList => render_repo_list(frame, app, area, styles),
        MainView::VaultList => render_vault_list(frame, app, area, styles),
    }
}

fn render_repo_list(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    let items: Vec<ListItem> = app
        .repos
        .iter()
        .map(|repo| {
            let status_icon = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
                (None, _, _) => "⏳",
                (Some(true), _, _) => "●",
                (Some(false), Some(a), Some(b)) if a > 0 && b > 0 => "◆",
                (Some(false), _, Some(b)) if b > 0 => "▼",
                (Some(false), Some(a), _) if a > 0 => "▲",
                _ if repo.upstream_url.is_none() => "○",
                _ => "✓",
            };
            let mut prefix = format!("{} ", status_icon);
            if app.loading_repo_status.contains(&repo.id) || app.loading_sync.contains(&repo.id) {
                prefix.push_str("⟳ ");
            }

            let base_fg = match (repo.status_dirty, repo.status_ahead, repo.status_behind) {
                (Some(true), _, _) => styles.theme.danger,
                (Some(false), _, Some(b)) if b > 0 => styles.theme.warning,
                (Some(false), Some(a), _) if a > 0 => styles.theme.primary,
                _ if repo.upstream_url.is_none() => styles.theme.muted,
                _ => styles.theme.success,
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
                format!("  ★{stars}")
            } else {
                String::new()
            };

            // Tag cluster indicator
            let tag_indicator = if let Some(first_tag) = repo.tags.first() {
                format!("  [{first_tag}]")
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, repo.id), Style::default().fg(fg)),
                Span::styled(star_indicator, styles.star),
                Span::styled(status_suffix, styles.dim),
                Span::styled(tag_indicator, styles.dim),
            ]))
        })
        .collect();

    let i18n = &app.ctx.i18n;
    let list_title = format!(
        "{} [{}]",
        i18n.tui.title_repos,
        match app.sort_mode {
            SortMode::Status => i18n.tui.sort_status,
            SortMode::Stars => "Stars",
        }
    );

    let list = if items.is_empty() {
        let onboarding = vec![
            ListItem::new(Line::from(Span::styled("", Style::default()))),
            ListItem::new(Line::from(vec![Span::styled(
                "  还没有注册任何仓库",
                Style::default().fg(styles.theme.warning).add_modifier(Modifier::BOLD),
            )])),
            ListItem::new(Line::from(Span::styled("", Style::default()))),
            ListItem::new(Line::from(vec![
                Span::styled("  运行: ", Style::default().fg(styles.theme.muted)),
                Span::styled(
                    "devbase scan <路径> --register",
                    Style::default().fg(styles.theme.primary),
                ),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled("  示例: ", Style::default().fg(styles.theme.muted)),
                Span::styled(
                    "devbase scan . --register",
                    Style::default().fg(styles.theme.success),
                ),
            ])),
        ];
        List::new(onboarding).block(
            Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .border_style(styles.border),
        )
    } else {
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(list_title)
                    .border_style(styles.border)
                    .padding(ratatui::widgets::Padding::horizontal(1)),
            )
            .highlight_style(styles.highlight)
            .highlight_symbol("> ")
    };

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_vault_list(frame: &mut Frame, app: &mut App, area: Rect, styles: &Styles) {
    let items: Vec<ListItem> = app
        .vaults
        .iter()
        .map(|vault| {
            let title = vault.title.as_deref().unwrap_or(&vault.id);
            let tag_indicator = if let Some(first_tag) = vault.tags.first() {
                format!("  [{first_tag}]")
            } else {
                String::new()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("📄 {}", title), Style::default().fg(styles.theme.text)),
                Span::styled(tag_indicator, styles.dim),
            ]))
        })
        .collect();

    let list_title = format!("Vault [{}]", app.vaults.len());

    let list = if items.is_empty() {
        let onboarding = vec![
            ListItem::new(Line::from(Span::styled("", Style::default()))),
            ListItem::new(Line::from(vec![Span::styled(
                "  还没有 Vault 笔记",
                Style::default().fg(styles.theme.warning).add_modifier(Modifier::BOLD),
            )])),
            ListItem::new(Line::from(Span::styled("", Style::default()))),
            ListItem::new(Line::from(vec![
                Span::styled("  运行: ", Style::default().fg(styles.theme.muted)),
                Span::styled(
                    "devbase vault scan <路径>",
                    Style::default().fg(styles.theme.primary),
                ),
            ])),
        ];
        List::new(onboarding).block(
            Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .border_style(styles.border),
        )
    } else {
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(list_title)
                    .border_style(styles.border)
                    .padding(ratatui::widgets::Padding::horizontal(1)),
            )
            .highlight_style(styles.highlight)
            .highlight_symbol("> ")
    };

    frame.render_stateful_widget(list, area, &mut app.vault_list_state);
}
