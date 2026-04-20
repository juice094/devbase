use crate::tui::App;
use crate::tui::theme::Styles;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub(crate) fn render_help(frame: &mut Frame, _app: &App, area: Rect, styles: &Styles) {
    let i18n = crate::i18n::current();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(i18n.tui.help_title)
        .border_style(styles.border)
        .title_style(Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into columns: Navigation | Repo / Sync | Search / System
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1)])
        .split(inner);

    let nav_lines = help_section(
        i18n.tui.help_category_nav,
        &[
            ("↑ / ↓", "选择上一个 / 下一个"),
            ("PgUp / PgDn", "快速翻页"),
            ("Home / End", "跳到顶部 / 底部"),
            ("Tab", "切换详情标签页"),
        ],
        styles,
    );
    frame.render_widget(
        Paragraph::new(ratatui::text::Text::from(nav_lines)).wrap(Wrap { trim: true }),
        columns[0],
    );

    let repo_lines = help_section(
        i18n.tui.help_category_repo,
        &[("Enter", "在 gitui/lazygit 中打开"), ("t", "编辑标签"), ("o", "切换排序模式")],
        styles,
    );
    let sync_lines = help_section(
        i18n.tui.help_category_sync,
        &[("s", "获取并预览同步"), ("S", "执行安全同步"), ("r", "刷新仓库列表")],
        styles,
    );
    let mut repo_sync = repo_lines;
    repo_sync.push(Line::from(""));
    repo_sync.extend(sync_lines);
    frame.render_widget(
        Paragraph::new(ratatui::text::Text::from(repo_sync)).wrap(Wrap { trim: true }),
        columns[1],
    );

    let search_lines = help_section(
        i18n.tui.help_category_search,
        &[
            ("/", "开始搜索"),
            ("Ctrl+R", "切换搜索模式"),
            ("↑/↓", "导航结果"),
            ("Enter", "在编辑器中打开"),
        ],
        styles,
    );
    let system_lines = help_section(
        i18n.tui.help_category_system,
        &[("h / ? / F1", "显示帮助"), ("q / Esc", "退出 / 关闭弹窗")],
        styles,
    );
    let mut search_system = search_lines;
    search_system.push(Line::from(""));
    search_system.extend(system_lines);
    frame.render_widget(
        Paragraph::new(ratatui::text::Text::from(search_system)).wrap(Wrap { trim: true }),
        columns[2],
    );
}

fn help_section<'a>(
    title: &'a str,
    bindings: &[(&'a str, &'a str)],
    styles: &Styles,
) -> Vec<Line<'a>> {
    let mut lines = vec![Line::from(Span::styled(
        title,
        Style::default().fg(styles.theme.primary).add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(""));
    for (key, desc) in bindings {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<14}", key),
                Style::default().fg(styles.theme.info).add_modifier(Modifier::BOLD),
            ),
            Span::styled(*desc, Style::default().fg(styles.theme.text)),
        ]));
    }
    lines
}
