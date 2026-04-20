use crate::tui::App;
use crate::tui::theme::Styles;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(crate) fn render_logs(frame: &mut Frame, app: &App, area: Rect, styles: &Styles) {
    let log_visible = area.height.saturating_sub(2) as usize;
    let log_start = app.logs.len().saturating_sub(log_visible);
    let log_lines: Vec<Line> =
        app.logs[log_start..].iter().map(|l| format_log_line(l, styles)).collect();

    let i18n = crate::i18n::current();
    let logs = Paragraph::new(ratatui::text::Text::from(log_lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(i18n.tui.title_logs)
                .border_style(styles.border),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(logs, area);
}

fn format_log_line<'a>(line: &'a str, styles: &'a Styles) -> Line<'a> {
    let mut spans = Vec::new();

    // Extract timestamp prefix [HH:MM:SS]
    if let Some(ts_end) = line.find("] ") {
        let ts = &line[..ts_end + 2];
        spans.push(Span::styled(ts, styles.dim));

        let rest = &line[ts_end + 2..];
        if let Some(stripped) = rest.strip_prefix("[ERROR] ") {
            spans.push(Span::styled("[ERROR] ", styles.log_error));
            spans.push(Span::raw(stripped));
        } else if let Some(stripped) = rest.strip_prefix("[WARN] ") {
            spans.push(Span::styled("[WARN] ", styles.log_warn));
            spans.push(Span::raw(stripped));
        } else if let Some(stripped) = rest.strip_prefix("[INFO] ") {
            spans.push(Span::styled("[INFO] ", styles.log_info));
            spans.push(Span::raw(stripped));
        } else {
            spans.push(Span::raw(rest));
        }
    } else {
        spans.push(Span::raw(line));
    }

    Line::from(spans)
}
