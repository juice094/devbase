use crate::tui::layout::AppLayout;
use crate::tui::theme::{Styles, Theme};
use crate::tui::{App, HelpPopupMode};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};

mod detail;
mod help;
mod list;
mod logs;
mod popups;

/// Main render dispatcher.
///
/// Delegates each screen region to its own submodule, keeping the top-level
/// render loop readable and maintainable.
pub(crate) fn ui(frame: &mut Frame, app: &mut App) {
    let theme = Theme::default();
    let styles = theme.styles();
    let layout = AppLayout::compute(frame.area());

    // Help overlay is modal — render it on top of everything else.
    if app.help_popup_mode == HelpPopupMode::Visible {
        // Dim the background
        let dim = ratatui::widgets::Block::default()
            .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));
        frame.render_widget(dim, frame.area());
        let popup = AppLayout::centered(frame.area(), 70, 75);
        help::render_help(frame, app, popup, &styles);
        return;
    }

    if layout.compact {
        // Compact mode: full-width list only
        list::render_list(frame, app, layout.list, &styles);
    } else {
        list::render_list(frame, app, layout.list, &styles);
        detail::render_detail(frame, app, layout.detail, &styles);
        logs::render_logs(frame, app, layout.logs, &styles);
    }

    // Popups (search / sync) overlay the main content
    popups::render_popups(frame, app, &styles);

    // Bottom bar (hints / input)
    render_bottom_bar(frame, app, layout.bottom, &styles);
}

fn render_bottom_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, styles: &Styles) {
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    if area.height == 0 {
        return;
    }

    let i18n = crate::i18n::current();
    let text = match app.input_mode {
        crate::tui::InputMode::TagInput => Line::from(vec![
            Span::styled("标签: ", styles.selected),
            Span::raw(&app.input_buffer),
            Span::styled(i18n.tui.hint_tag_input, styles.hint),
        ]),
        crate::tui::InputMode::SearchInput => Line::from(vec![
            Span::styled(i18n.tui.search_prompt, styles.selected),
            Span::raw(" "),
            Span::raw(&app.input_buffer),
            Span::styled(i18n.tui.hint_tag_input, styles.hint),
        ]),
        crate::tui::InputMode::Normal => {
            let view_label = match app.main_view {
                crate::tui::MainView::RepoList => "[Repos]",
                crate::tui::MainView::VaultList => "[Vault]",
            };
            let mut spans = vec![
                Span::styled(
                    view_label,
                    Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled("Tab", styles.selected),
                Span::raw("=切换 "),
                Span::styled("q", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_quit)),
                Span::styled("r", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_refresh)),
                Span::styled("s", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_preview)),
                Span::styled("S", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_batch)),
                Span::styled("t", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_tag)),
                Span::styled("o", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_sort)),
                Span::styled("k", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_skills)),
                Span::styled("h", styles.selected),
                Span::raw(format!("={} ", i18n.tui.help_help)),
            ];
            if app.sync_total > 0 {
                let queued = app.loading_sync.len();
                let running = app.sync_running.len();
                let completed = app.sync_total.saturating_sub(queued + running);
                spans.push(Span::raw(" | "));
                spans.push(Span::styled(
                    format!(
                        "{}{}/{}/{}",
                        i18n.tui.title_sync_progress, completed, running, app.sync_total
                    ),
                    styles.dry_run,
                ));
            }
            Line::from(spans)
        }
    };

    frame.render_widget(Paragraph::new(text), area);
}

// ---------------------------------------------------------------------------
// Data helpers (pure functions, isolated from rendering logic)
// ---------------------------------------------------------------------------

pub(crate) fn read_head_commit(path: &str) -> Option<String> {
    let repo = git2::Repository::open(path).ok()?;
    let head = repo.head().ok()?;
    let oid = head.target()?;
    Some(oid.to_string().chars().take(7).collect())
}

pub(crate) fn read_syncdone_info(path: &str) -> (String, String, String) {
    let default = || ("从未同步".to_string(), "—".to_string(), "—".to_string());

    let content =
        match std::fs::read_to_string(std::path::Path::new(path).join(".devbase").join("syncdone"))
        {
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

pub(crate) fn read_repo_summary(conn: &rusqlite::Connection, repo_id: &str) -> Option<String> {
    conn.query_row("SELECT summary FROM repo_summaries WHERE repo_id = ?1", [repo_id], |row| {
        row.get::<_, String>(0)
    })
    .ok()
}
